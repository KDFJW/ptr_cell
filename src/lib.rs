//! # Simple thread-safe cell
//!
//! [`PtrCell`] is an atomic cell type that allows safe, concurrent access to shared data. No
//! [`std`][1], no data races, no [nasal demons][2] (undefined behavior), and most importantly, no
//! locks
//!
//! This type is only useful in scenarios where you need to update a shared value by moving in and
//! out of it. If you want to concurrently update a value through mutable references and don't
//! require support for `no_std`, take a look at the standard [`Mutex`][3] and [`RwLock`][4] instead
//!
//! #### Offers:
//!
//! - **Familiarity**: `PtrCell`'s API was modelled after `std`'s [Cell](core::cell::Cell)
//!
//! - **Easy Concurrency**: No more `Arc<Mutex<T>>`, `Arc::clone()`, and `Mutex::lock().expect()`!
//! Leave the data static and then point to it when you need to. It's a _single instruction_ on most
//! modern platforms
//!
//! #### Limitations:
//!
//! - **Heap Allocation**: Every value you insert into `PtrCell` must first be allocated using
//! [`Box`]. Allocating on the heap is, computationally, a moderately expensive operation. To
//! address this, the cell exposes a pointer API that can be used to avoid allocating the same
//! values multiple times. Future releases will primarily rely on the stack
//!
//! ## Usage
//!
//! ```rust
//! use ptr_cell::{PtrCell, Semantics::Relaxed};
//!
//! let cell: PtrCell<u16> = 0x81D.into();
//!
//! assert_eq!(cell.replace(Some(2047), Relaxed), Some(0x81D));
//! assert_eq!(cell.is_empty(Relaxed), false);
//! assert_eq!(cell.take(Relaxed), Some(2047))
//! ```
//!
//! ## Semantics
//!
//! [`PtrCell`] allows you to specify memory ordering semantics for its internal atomic operations
//! through the [`Semantics`] enum. Each variant is different in how it balances synchronization and
//! performace. Here's a comparison of the available semantics:
//!
//! | Variant | Overhead | Synchronization |
//! |---|---|---|
//! | [`Relaxed`](Semantics::Relaxed) | Negligible | None |
//! | [`Coupled`](Semantics::Coupled) | Acceptable | Intuitive |
//! | [`Ordered`](Semantics::Ordered) | Noticeable | Strict |
//!
//! `Coupled` is what you'd typically use. However, other orderings have their use cases too. For
//! example, the `Relaxed` semantics could be useful when the operations are already synchronized
//! through other means, like [fences](core::sync::atomic::fence). As always, the documentation for
//! each item contains more details
//!
//! ## Examples
//!
//! The code below finds the maximum value of a sequence by concurrently processing its halves.
//! Notice how the code doesn't read the shared value. Instead, it uses moves and corrects previous
//! operations as new data comes in
//!
//! ```rust
//! use ptr_cell::{PtrCell, Semantics};
//! use std::sync::Arc;
//!
//! fn main() {
//!     const VALUES: [u8; 11] = [47, 12, 88, 45, 67, 34, 78, 90, 11, 77, 33];
//!
//!     let cell = PtrCell::default();
//!     let maximum = Arc::new(cell);
//!
//!     let (left, right) = VALUES.split_at(VALUES.len() / 2);
//!
//!     let handles = [left, right].map(|half| {
//!         let maximum = Arc::clone(&maximum);
//!
//!         std::thread::spawn(move || maximize_in(half, &maximum))
//!     });
//!
//!     for worker in handles {
//!         if let Err(payload) = worker.join() {
//!             std::panic::resume_unwind(payload)
//!         }
//!     }
//!
//!     assert_eq!(maximum.take(Semantics::Relaxed), Some(90))
//! }
//!
//! fn maximize_in<T>(sequence: &[T], buffer: &PtrCell<T>)
//! where
//!     T: Ord + Copy,
//! {
//!     for &item in sequence {
//!         let mut slot = Some(item);
//!
//!         loop {
//!             let previous = buffer.replace(slot, Semantics::Relaxed);
//!
//!             match slot < previous {
//!                 true => slot = previous,
//!                 false => break,
//!             }
//!         }
//!     }
//! }
//! ```
//!
//! [1]: https://doc.rust-lang.org/std/index.html
//! [2]: https://groups.google.com/g/comp.std.c/c/ycpVKxTZkgw/m/S2hHdTbv4d8J?hl=en
//! [3]: https://doc.rust-lang.org/std/sync/struct.Mutex.html
//! [4]: https://doc.rust-lang.org/std/sync/struct.RwLock.html

#![no_std]
#![warn(missing_docs, clippy::all, clippy::pedantic, clippy::cargo)]
#![allow(clippy::must_use_candidate)]
#![forbid(unsafe_op_in_unsafe_fn)]

extern crate alloc;

use alloc::boxed::Box;
use core::sync::atomic::Ordering;

// 3.0.0:
// - Just fix `replace_ptr` already!!! \
// - Make `Semantics` exhaustive       |
// - Add the default `std` feature     /
// - Figure out how to properly generalize to the stack (see notes below)
// - Implement `get`, `update`, and some traits by using brief spinlocking
// - Add "virtually" to "no locks" in the top-level docs (very important)
// - Add `from_mut` like on std's Cell

// It's possible to ditch heap allocation entirely if we pre-allocate a buffer of type T.
// Pre-allocating an array of N buffers (const N: usize) could amortize performance losses during
// periods of high contention

// Top-level:
//
// ## Features
//
// - **`std`**: Enables everything that may depend on the standard library. Currently, there are no
// such items. Could optimize performace in future updates

/// Thread-safe cell based on atomic pointers
///
/// This type stores its data externally by _leaking_ it with [`Box`]. Synchronization is achieved
/// by atomically manipulating pointers to the data
///
/// # Usage
///
/// ```rust
/// use ptr_cell::{PtrCell, Semantics::Relaxed};
///
/// let cell: PtrCell<u16> = 0x81D.into();
///
/// assert_eq!(cell.replace(Some(2047), Relaxed), Some(0x81D));
/// assert_eq!(cell.is_empty(Relaxed), false);
/// assert_eq!(cell.take(Relaxed), Some(2047))
/// ```
///
/// # Pointer Safety
///
/// When dereferencing a pointer to the cell's value, you must ensure that the pointed-to memory
/// hasn't been [reclaimed](Self::heap_reclaim). For example, [`replace`](Self::replace) and its
/// derivatives ([`set`](Self::set) and [`take`](Self::take)) automatically reclaim memory. Be
/// careful not to miss any calls to such functions made from other threads
///
/// This also applies to externally-sourced pointers, like the `ptr` parameter in
/// [`from_ptr`](Self::from_ptr)
#[repr(transparent)]
pub struct PtrCell<T> {
    /// Pointer to the contained value
    ///
    /// #### Invariants
    ///
    /// - **If non-null**: Must point to memory that conforms to the [memory layout][1] used by
    ///   [`Box`]
    ///
    /// [1]: https://doc.rust-lang.org/std/boxed/index.html#memory-layout
    value: core::sync::atomic::AtomicPtr<T>,
}

impl<T> PtrCell<T> {
    /// Inserts the value constructed from this cell by `new` into the cell itself
    ///
    /// Think of this like the `push` method of a linked list, where each node contains a `PtrCell`
    ///
    /// # Examples
    ///
    /// The code below turns a sentence into a naive linked list of words, which is then assembled
    /// back into a [`String`][1]
    ///
    /// ```rust
    /// use ptr_cell::{PtrCell, Semantics};
    ///
    /// struct Node<T> {
    ///     pub value: T,
    ///     pub next: PtrCell<Self>,
    /// }
    ///
    /// impl<T> AsMut<PtrCell<Self>> for Node<T> {
    ///     fn as_mut(&mut self) -> &mut ptr_cell::PtrCell<Self> {
    ///         &mut self.next
    ///     }
    /// }
    ///
    /// let cell = PtrCell::default();
    ///
    /// for value in "Hachó en México".split_whitespace().rev() {
    ///     cell.map_owner(|next| Node { value, next }, Semantics::Relaxed);
    /// }
    ///
    /// let Node { value, mut next } = cell
    ///     .take(Semantics::Relaxed)
    ///     .expect("Some values should've been inserted into the cell");
    ///
    /// let mut decoded = value.to_string();
    /// while let Some(node) = next.take(Semantics::Relaxed) {
    ///     decoded.extend([" ", node.value]);
    ///     next = node.next
    /// }
    ///
    /// assert_eq!(decoded, "Hachó en México")
    /// ```
    ///
    /// [1]: https://doc.rust-lang.org/std/string/struct.String.html
    pub fn map_owner<F>(&self, new: F, order: Semantics)
    where
        F: FnOnce(Self) -> T,
        T: AsMut<Self>,
    {
        let value_ptr = self.get_ptr(order);
        let value = unsafe { Self::from_ptr(value_ptr) };

        let owner_slot = Some(new(value));
        let owner_ptr = Self::heap_leak(owner_slot);

        let owner = unsafe { &mut *owner_ptr };
        let value_ptr = owner.as_mut().value.get_mut();

        loop {
            let value_ptr_result = self.value.compare_exchange_weak(
                *value_ptr,
                owner_ptr,
                order.read_write(),
                order.read(),
            );

            let Err(modified) = value_ptr_result else {
                break;
            };

            *value_ptr = modified;
            core::hint::spin_loop();
        }
    }

    /// Swaps the values of two cells
    ///
    /// # Usage
    ///
    /// ```rust
    /// use ptr_cell::{PtrCell, Semantics::Relaxed};
    ///
    /// let one: PtrCell<u8> = 1.into();
    /// let mut two: PtrCell<u8> = 2.into();
    ///
    /// one.swap(&mut two, Relaxed);
    ///
    /// assert_eq!(two.take(Relaxed), Some(1));
    /// assert_eq!(one.take(Relaxed), Some(2))
    /// ```
    #[inline]
    pub fn swap(&self, other: &mut Self, order: Semantics) {
        let other_ptr = other.get_ptr(Semantics::Relaxed);

        unsafe {
            let ptr = self.replace_ptr(other_ptr, order);
            other.set_ptr(ptr, Semantics::Relaxed);
        }
    }

    /// Takes out the cell's value
    ///
    /// # Usage
    ///
    /// ```rust
    /// use ptr_cell::{PtrCell, Semantics::Relaxed};
    ///
    /// let cell: PtrCell<u8> = 45.into();
    ///
    /// assert_eq!(cell.take(Relaxed), Some(45));
    /// assert_eq!(cell.take(Relaxed), None)
    /// ```
    #[inline]
    pub fn take(&self, order: Semantics) -> Option<T> {
        self.replace(None, order)
    }

    /// Takes out the cell's pointer
    ///
    /// # Safety
    ///
    /// Not inherently unsafe. See [Pointer Safety][1]
    ///
    /// # Usage
    ///
    /// ```rust
    /// use ptr_cell::{PtrCell, Semantics::Relaxed};
    ///
    /// let cell: PtrCell<u8> = 45.into();
    /// let ptr = cell.take_ptr(Relaxed);
    ///
    /// assert_eq!(unsafe { ptr_cell::PtrCell::heap_reclaim(ptr) }, Some(45));
    /// assert_eq!(cell.take_ptr(Relaxed), std::ptr::null_mut())
    /// ```
    ///
    /// [1]: https://docs.rs/ptr_cell/latest/ptr_cell/struct.PtrCell.html#pointer-safety
    #[inline]
    pub fn take_ptr(&self, order: Semantics) -> *mut T {
        self.replace_ptr(core::ptr::null_mut(), order)
    }

    /// Inserts a value into the cell
    ///
    /// # Usage
    ///
    /// ```rust
    /// use ptr_cell::{PtrCell, Semantics::Relaxed};
    ///
    /// let cell = PtrCell::default();
    /// cell.set(Some(1776), Relaxed);
    ///
    /// assert_eq!(cell.take(Relaxed), Some(1776))
    /// ```
    #[inline]
    pub fn set(&self, slot: Option<T>, order: Semantics) {
        let _ = self.replace(slot, order);
    }

    /// Inserts a pointer into the cell
    ///
    /// # Safety
    ///
    /// The pointed-to memory must conform to the [memory layout][1] used by [`Box`]
    ///
    /// # Usage
    ///
    /// ```rust
    /// use ptr_cell::{PtrCell, Semantics::Relaxed};
    ///
    /// let cell = PtrCell::default();
    ///
    /// let ptr = PtrCell::heap_leak(Some(1776));
    /// unsafe { cell.set_ptr(ptr, Relaxed) };
    ///
    /// assert_eq!(cell.take(Relaxed), Some(1776))
    /// ```
    ///
    /// [1]: https://doc.rust-lang.org/std/boxed/index.html#memory-layout
    #[inline]
    pub unsafe fn set_ptr(&self, ptr: *mut T, order: Semantics) {
        self.value.store(ptr, order.write());
    }

    /// Replaces the cell's value
    ///
    /// # Usage
    ///
    /// ```rust
    /// use ptr_cell::{PtrCell, Semantics::Relaxed};
    ///
    /// let cell = PtrCell::from('a');
    ///
    /// assert_eq!(cell.replace(Some('b'), Relaxed), Some('a'));
    /// assert_eq!(cell.take(Relaxed), Some('b'))
    /// ```
    #[inline]
    #[must_use = "use `.set()` if you don't need the old value"]
    pub fn replace(&self, slot: Option<T>, order: Semantics) -> Option<T> {
        let new_leak = Self::heap_leak(slot);

        unsafe {
            let old_leak = self.replace_ptr(new_leak, order);
            Self::heap_reclaim(old_leak)
        }
    }

    /// Replaces the cell's pointer
    ///
    /// **WARNING: THIS FUNCTION WAS ERRONEOUSLY LEFT SAFE. IT'S UNSAFE AND WILL BE MARKED AS SUCH
    /// IN THE NEXT MAJOR RELEASE**
    ///
    /// # Safety
    ///
    /// The pointed-to memory must conform to the [memory layout][1] used by [`Box`]
    ///
    /// See also: [Pointer Safety][2]
    ///
    /// # Usage
    ///
    /// ```rust
    /// use ptr_cell::{PtrCell, Semantics::Relaxed};
    ///
    /// unsafe {
    ///     let a = PtrCell::heap_leak(Some('a'));
    ///     let b = PtrCell::heap_leak(Some('b'));
    ///
    ///     let cell = PtrCell::from_ptr(a);
    ///
    ///     assert_eq!(cell.replace_ptr(b, Relaxed), a);
    ///     assert_eq!(cell.take_ptr(Relaxed), b);
    ///
    ///     PtrCell::heap_reclaim(a);
    ///     PtrCell::heap_reclaim(b);
    /// }
    /// ```
    ///
    /// [1]: https://doc.rust-lang.org/std/boxed/index.html#memory-layout
    /// [2]: https://docs.rs/ptr_cell/latest/ptr_cell/struct.PtrCell.html#pointer-safety
    #[inline]
    #[must_use = "use `.set_ptr()` if you don't need the old pointer"]
    pub fn replace_ptr(&self, ptr: *mut T, order: Semantics) -> *mut T {
        self.value.swap(ptr, order.read_write())
    }

    /// Mutably borrows the cell's value
    ///
    /// # Usage
    ///
    /// ```rust
    /// use ptr_cell::{PtrCell, Semantics::Relaxed};
    ///
    /// let mut text = PtrCell::from("Point".to_string());
    /// text.get_mut()
    ///     .expect("The cell should contain a value")
    ///     .push_str("er");
    ///
    /// assert_eq!(text.take(Relaxed), Some("Pointer".to_string()))
    /// ```
    #[inline]
    pub fn get_mut(&mut self) -> Option<&mut T> {
        let leak = *self.value.get_mut();

        non_null(leak).map(|ptr| unsafe { &mut *ptr })
    }

    /// Returns a pointer to the cell's value
    ///
    /// # Safety
    ///
    /// Not inherently unsafe. See [Pointer Safety][1]
    ///
    /// # Usage
    ///
    /// ```rust
    /// use ptr_cell::{PtrCell, Semantics::Relaxed};
    ///
    /// let cell = PtrCell::<[u8; 3]>::default();
    ///
    /// assert_eq!(cell.get_ptr(Relaxed), std::ptr::null_mut())
    /// ```
    ///
    /// [1]: https://docs.rs/ptr_cell/latest/ptr_cell/struct.PtrCell.html#pointer-safety
    #[inline]
    pub fn get_ptr(&self, order: Semantics) -> *mut T {
        self.value.load(order.read())
    }

    /// Determines whether this cell is empty
    ///
    /// # Usage
    ///
    /// ```rust
    /// use ptr_cell::{PtrCell, Semantics::Relaxed};
    ///
    /// let cell = PtrCell::<[u8; 3]>::default();
    ///
    /// assert!(cell.is_empty(Relaxed))
    /// ```
    #[inline]
    pub fn is_empty(&self, order: Semantics) -> bool {
        self.get_ptr(order).is_null()
    }

    /// Constructs a cell
    ///
    /// # Usage
    ///
    /// ```rust
    /// use ptr_cell::{PtrCell, Semantics::Relaxed};
    ///
    /// let cell = PtrCell::new(Some(0xFAA));
    ///
    /// assert_eq!(cell.take(Relaxed), Some(0xFAA));
    /// assert!(cell.is_empty(Relaxed))
    /// ```
    #[inline]
    #[must_use]
    pub fn new(slot: Option<T>) -> Self {
        let ptr = Self::heap_leak(slot);

        unsafe { Self::from_ptr(ptr) }
    }

    /// Constructs a cell that owns [leaked](Self::heap_leak) memory
    ///
    /// A null pointer represents [`None`]
    ///
    /// # Safety
    ///
    /// The memory must conform to the [memory layout][1] used by [`Box`]
    ///
    /// # Usage
    ///
    /// ```rust
    /// use ptr_cell::{PtrCell, Semantics::Relaxed};
    ///
    /// let ptr = PtrCell::heap_leak(Some(0xFAA));
    /// let cell = unsafe { PtrCell::from_ptr(ptr) };
    ///
    /// assert_eq!(cell.take(Relaxed), Some(0xFAA));
    /// assert!(cell.is_empty(Relaxed))
    /// ```
    ///
    /// [1]: https://doc.rust-lang.org/std/boxed/index.html#memory-layout
    #[inline]
    pub const unsafe fn from_ptr(ptr: *mut T) -> Self {
        let value = core::sync::atomic::AtomicPtr::new(ptr);

        Self { value }
    }

    /// Reclaims ownership of [leaked](Self::heap_leak) memory
    ///
    /// A null pointer represents [`None`]
    ///
    /// # Safety
    ///
    /// The memory must conform to the [memory layout][1] used by [`Box`]
    ///
    /// Dereferencing `ptr` after this function has been called may cause undefined behavior
    ///
    /// # Usage
    ///
    /// ```rust
    /// use ptr_cell::PtrCell;
    ///
    /// let ptr = PtrCell::heap_leak(Some(1155));
    ///
    /// assert_eq!(unsafe { PtrCell::heap_reclaim(ptr) }, Some(1155))
    /// ```
    ///
    /// [1]: https://doc.rust-lang.org/std/boxed/index.html#memory-layout
    #[inline]
    pub unsafe fn heap_reclaim(ptr: *mut T) -> Option<T> {
        non_null(ptr).map(|ptr| *unsafe { Box::from_raw(ptr) })
    }

    /// Leaks a value to the heap
    ///
    /// [`None`] is represented by a null pointer
    ///
    /// The memory will conform to the [memory layout][1] used by [`Box`]
    ///
    /// # Usage
    ///
    /// ```rust
    /// use ptr_cell::PtrCell;
    ///
    /// let ptr = PtrCell::heap_leak(Some(1155));
    ///
    /// assert_eq!(unsafe { PtrCell::heap_reclaim(ptr) }, Some(1155))
    /// ```
    ///
    /// [1]: https://doc.rust-lang.org/std/boxed/index.html#memory-layout
    #[inline]
    #[must_use]
    pub fn heap_leak(slot: Option<T>) -> *mut T {
        match slot {
            Some(value) => Box::into_raw(Box::new(value)),
            None => core::ptr::null_mut(),
        }
    }
}

impl<T> core::fmt::Debug for PtrCell<T> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter) -> core::fmt::Result {
        formatter
            .debug_struct("PtrCell")
            .field("value", &self.value)
            .finish()
    }
}

impl<T> Default for PtrCell<T> {
    /// Constructs an empty cell
    #[inline]
    fn default() -> Self {
        Self::new(None)
    }
}

impl<T> Drop for PtrCell<T> {
    #[inline]
    fn drop(&mut self) {
        let ptr = *self.value.get_mut();

        unsafe { Self::heap_reclaim(ptr) };
    }
}

impl<T> From<T> for PtrCell<T> {
    #[inline]
    fn from(value: T) -> Self {
        Self::new(Some(value))
    }
}

/// Returns `ptr` if it's non-null
#[inline]
fn non_null<T>(ptr: *mut T) -> Option<*mut T> {
    if ptr.is_null() {
        None
    } else {
        Some(ptr)
    }
}

/// Memory ordering semantics for atomic operations
///
/// Each variant represents a group of compatible [orderings](Ordering). They determine how value
/// updates are synchronized between threads
#[non_exhaustive]
#[derive(Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Default)]
pub enum Semantics {
    /// [`Relaxed`](Ordering::Relaxed) semantics
    ///
    /// No synchronization constraints and the best performance
    ///
    /// Set this when using a value in only one thread
    Relaxed,

    /// [`Release`](Ordering::Release) - [`Acquire`](Ordering::Acquire) coupling semantics
    ///
    /// Mild synchronization constraints and fair performance
    ///
    /// A read will always see the preceding write (if one exists). Any operations that take place
    /// before the write will also be seen, regardless of their semantics
    ///
    /// Set this when using a value in multiple threads, unless certain that you need different
    /// semantics
    #[default]
    Coupled,

    /// [`SeqCst`](Ordering::SeqCst) semantics
    ///
    /// Maximum synchronization constraints and the worst performance
    ///
    /// All memory operations will appear to be executed in a single, total order
    Ordered,
}

/// Implements a method on [`Semantics`] that returns the appropriate [`Ordering`] for a type of
/// operations
macro_rules! operation {
    ($name:ident with $coupled:path:
        { $($overview:tt)* }, { $($returns:tt)* }, { $($assert:tt)* }
    $(,)? ) => {
        impl Semantics {
            $($overview)*
            ///
            /// # Returns
            /// - [`Relaxed`](Ordering::Relaxed) for [`Relaxed`](Semantics::Relaxed) semantics
            $($returns)*
            /// - [`SeqCst`](Ordering::SeqCst) for [`Ordered`](Semantics::Ordered) semantics
            ///
            /// # Usage
            ///
            /// ```rust
            /// use ptr_cell::Semantics::Coupled;
            /// use std::sync::atomic::Ordering;
            ///
            $($assert)*
            /// ```
            #[inline]
            pub const fn $name(&self) -> Ordering {
                match self {
                    Self::Relaxed => Ordering::Relaxed,
                    Self::Coupled => $coupled,
                    Self::Ordered => Ordering::SeqCst,
                }
            }
        }
    };
}

// Asserts are missing a space on purpose. All whitespace after `///` seems to be carried over to
// the example

operation!(read_write with Ordering::AcqRel: {
    /// Returns the memory ordering for read-write operations with these semantics
}, {
    /// - [`AcqRel`](Ordering::AcqRel) for [`Coupled`](Semantics::Coupled) semantics
}, {
    ///assert_eq!(Coupled.read_write(), Ordering::AcqRel)
});

operation!(write with Ordering::Release: {
    /// Returns the memory ordering for write operations with these semantics
}, {
    /// - [`Release`](Ordering::Release) for [`Coupled`](Semantics::Coupled) semantics
}, {
    ///assert_eq!(Coupled.write(), Ordering::Release)
});

operation!(read with Ordering::Acquire: {
    /// Returns the memory ordering for read operations with these semantics
}, {
    /// - [`Acquire`](Ordering::Acquire) for [`Coupled`](Semantics::Coupled) semantics
}, {
    ///assert_eq!(Coupled.read(), Ordering::Acquire)
});
