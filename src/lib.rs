//! # Simple thread-safe cell
//!
//! [`PtrCell`] is an atomic cell type that allows safe, concurrent access to shared data. No
//! [`std`], no [data races][1], no [nasal demons (UB)][2], and most importantly, no [locks][3]
//
//! This type is only useful in scenarios where you need to update a shared value by moving in and
//! out of it. If you want to concurrently update a value through mutable references and don't
//! require support for environments without the standard library ([`no_std`][4]), take a look at
//! the standard [`Mutex`](std::sync::Mutex) and [`RwLock`](std::sync::RwLock) instead
//!
//! #### Offers:
//! - **Ease of use**: The API is fairly straightforward
//! - **Performance**: The algorithms are at most a couple of instructions long
//!
//! #### Limits:
//! - **Access to the cell's value**: To see what's stored inside a cell, you must either take the
//! value out of it or have exclusive access to the cell
//!
//! ## Usage
//!
//! ```rust
//! use ptr_cell::Semantics;
//!
//! // Construct a cell
//! let cell: ptr_cell::PtrCell<u16> = 0x81D.into();
//!
//! // Replace the value inside the cell
//! assert_eq!(cell.replace(Some(2047), Semantics::Relaxed), Some(0x81D));
//!
//! // Check whether the cell is empty
//! assert_eq!(cell.is_empty(Semantics::Relaxed), false);
//!
//! // Take the value out of the cell
//! assert_eq!(cell.take(Semantics::Relaxed), Some(2047))
//! ```
//!
//! ## Semantics
//!
//! `PtrCell` allows you to specify memory ordering semantics for its internal atomic operations
//! through the [`Semantics`] enum. Choosing appropriate semantics is crucial for achieving the
//! desired level of synchronization and performance. The available semantics are:
//!
//! - [`Ordered`](Semantics::Ordered): Noticeable overhead, strict
//! - [`Coupled`](Semantics::Coupled): Acceptable overhead, intuitive
//! - [`Relaxed`](Semantics::Relaxed): Little overhead, unconstrained
//!
//! `Coupled` is what you'd typically use. However, other orderings have their use cases too. For
//! example, the `Relaxed` semantics could be useful when the operations are already ordered through
//! other means, like [fences](std::sync::atomic::fence). As always, the documentation for each item
//! contains more details
//!
//! ## Examples
//!
//! Find the maximum value of a sequence of numbers by concurrently processing both of the
//! sequence's halves
//!
//! ```rust
//! use ptr_cell::Semantics;
//!
//! fn main() {
//!     // Initialize an array of random numbers
//!     const VALUES: [u8; 11] = [47, 12, 88, 45, 67, 34, 78, 90, 11, 77, 33];
//!
//!     // Construct a cell to hold the current maximum value
//!     let cell = ptr_cell::PtrCell::new(None);
//!     let maximum = std::sync::Arc::new(cell);
//!
//!     // Slice the array in two
//!     let (left, right) = VALUES.split_at(VALUES.len() / 2);
//!
//!     // Start a worker thread for each half
//!     let handles = [left, right].map(|half| {
//!         // Clone `maximum` to move it into the worker
//!         let maximum = std::sync::Arc::clone(&maximum);
//!
//!         // Spawn a thread to run the maximizer
//!         std::thread::spawn(move || maximize_in(half, &maximum))
//!     });
//!
//!     // Wait for the workers to finish
//!     for worker in handles {
//!         // Check whether a panic occured
//!         if let Err(payload) = worker.join() {
//!             // Thread panicked, propagate the panic
//!             std::panic::resume_unwind(payload)
//!         }
//!     }
//!
//!     // Check the found maximum
//!     assert_eq!(maximum.take(Semantics::Relaxed), Some(90))
//! }
//!
//! /// Inserts the maximum of `sequence` and `buffer` into `buffer`
//! ///
//! /// At least one swap takes place for each value of `sequence`
//! fn maximize_in<T>(sequence: &[T], buffer: &ptr_cell::PtrCell<T>)
//! where
//!     T: Ord + Copy,
//! {
//!     // Iterate over the slice
//!     for &item in sequence {
//!         // Wrap the item to make the cell accept it
//!         let mut slot = Some(item);
//!
//!         // Try to insert the value into the cell
//!         loop {
//!             // Replace the cell's value
//!             let previous = buffer.replace(slot, Semantics::Relaxed);
//!
//!             // Determine whether the swap resulted in a decrease of the buffer's value
//!             match slot < previous {
//!                 // It did, insert the old value back
//!                 true => slot = previous,
//!                 // It didn't, move on to the next item
//!                 false => break,
//!             }
//!         }
//!     }
//! }
//! ```
//!
//! [1]: https://en.wikipedia.org/wiki/Race_condition#In_software
//! [2]: https://en.wikipedia.org/wiki/Undefined_behavior
//! [3]: https://en.wikipedia.org/wiki/Lock_(computer_science)
//! [4]: https://docs.rust-embedded.org/book/intro/no-std.html

#![no_std]
#![warn(missing_docs)]

extern crate alloc;

use alloc::boxed::Box;
use core::sync::atomic::Ordering;

// As far as I can tell, accessing the cell's value is only safe when you have exclusive access to
// the pointer. In other words, either after replacing the pointer, or when working with a &mut or
// an owned cell. The next comment follows from this

// Do NOT ever refactor this to use None instead of null pointers. No pointer and a pointer to
// nothing are vastly different concepts. In this case, only the absence of a pointer is safe to use

/// Thread-safe cell based on atomic pointers
///
/// This cell type stores its data externally: instead of owning values directly, it holds pointers
/// to *leaked* values allocated by [`Box`]. Synchronization is achieved by atomically manipulating
/// these pointers
///
/// This type has the same in-memory representation as a `*mut T`
///
/// # Usage
///
/// ```rust
/// use ptr_cell::Semantics;
///
/// // Construct a cell
/// let cell: ptr_cell::PtrCell<u16> = 0x81D.into();
///
/// // Replace the value inside the cell
/// assert_eq!(cell.replace(Some(2047), Semantics::Relaxed), Some(0x81D));
///
/// // Check whether the cell is empty
/// assert_eq!(cell.is_empty(Semantics::Relaxed), false);
///
/// // Take the value out of the cell
/// assert_eq!(cell.take(Semantics::Relaxed), Some(2047))
/// ```
#[repr(transparent)]
pub struct PtrCell<T> {
    /// Pointer to the contained value
    value: core::sync::atomic::AtomicPtr<T>,
}

impl<T> PtrCell<T> {
    /// Returns a mutable reference to the cell's value
    ///
    /// # Usage
    ///
    /// ```rust
    /// use ptr_cell::Semantics;
    ///
    /// // Construct a cell with a String inside
    /// let mut text: ptr_cell::PtrCell<_> = "Punto aquí".to_string().into();
    ///
    /// // Modify the String
    /// text.get_mut()
    ///     .expect("The cell should contain a value")
    ///     .push_str(" con un puntero");
    ///
    /// // Check the String's value
    /// let sentence = "Punto aquí con un puntero".to_string();
    /// assert_eq!(text.take(Semantics::Relaxed), Some(sentence))
    /// ```
    #[inline(always)]
    pub fn get_mut(&mut self) -> Option<&mut T> {
        let leak = *self.value.get_mut();

        non_null(leak).map(|ptr| unsafe { &mut *ptr })
    }

    /// Replaces the cell's value with a new one, constructed from the cell itself using the
    /// provided `new` function
    ///
    /// Despite the operation being somewhat complex, it's still entirely atomic. This allows it to
    /// be safely used in implementations of shared linked-list-like data structures
    ///
    /// # Usage
    ///
    /// ```rust
    /// fn main() {
    ///     use ptr_cell::Semantics;
    ///
    ///     // Initialize a sample sentence
    ///     const SENTENCE: &str = "Hachó en México";
    ///
    ///     // Construct an empty cell
    ///     let cell = ptr_cell::PtrCell::default();
    ///
    ///     // "encode" the sentence into the cell
    ///     for word in SENTENCE.split_whitespace().rev() {
    ///         // Make the new node set its value to the current word
    ///         let value = word;
    ///
    ///         // Replace the node with a new one pointing to it
    ///         cell.map_owner(|next| Node { value, next }, Semantics::Relaxed);
    ///     }
    ///
    ///     // Take the first node out of the cell and destructure it
    ///     let Node { value, mut next } = cell
    ///         .take(Semantics::Relaxed)
    ///         .expect("Values should have been inserted into the cell");
    ///
    ///     // Initialize the "decoded" sentence with the first word
    ///     let mut decoded = value.to_string();
    ///
    ///     // Iterate over each remaining node
    ///     while let Some(node) = next.take(Semantics::Relaxed) {
    ///         // Append the word to the sentence
    ///         decoded += " ";
    ///         decoded += node.value;
    ///
    ///         // Set the value to process next
    ///         next = node.next
    ///     }
    ///
    ///     assert_eq!(SENTENCE, decoded)
    /// }
    ///
    /// /// Unit of a linked list
    /// struct Node<T> {
    ///     pub value: T,
    ///     pub next: ptr_cell::PtrCell<Self>,
    /// }
    ///
    /// impl<T> AsMut<ptr_cell::PtrCell<Self>> for Node<T> {
    ///     fn as_mut(&mut self) -> &mut ptr_cell::PtrCell<Self> {
    ///         &mut self.next
    ///     }
    /// }
    /// ```
    pub fn map_owner<F>(&self, new: F, order: Semantics)
    where
        F: FnOnce(Self) -> T,
        T: AsMut<Self>,
    {
        let value_ptr = self.value.load(order.read());
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

            match value_ptr_result {
                Ok(_same) => break,
                Err(modified) => *value_ptr = modified,
            }
        }
    }

    /// Returns the cell's value, leaving [`None`] in its place
    ///
    /// This is an alias for `self.replace(None, order)`
    ///
    /// # Usage
    ///
    /// ```rust
    /// use ptr_cell::Semantics;
    ///
    /// // Initialize a sample number
    /// const VALUE: Option<u8> = Some(0b01000101);
    ///
    /// // Wrap the number in a cell
    /// let cell = ptr_cell::PtrCell::new(VALUE);
    ///
    /// // Take the number out
    /// assert_eq!(cell.take(Semantics::Relaxed), VALUE);
    ///
    /// // Verify that the cell is now empty
    /// assert_eq!(cell.take(Semantics::Relaxed), None)
    /// ```
    #[inline(always)]
    pub fn take(&self, order: Semantics) -> Option<T> {
        self.replace(None, order)
    }

    /// Returns the cell's value, replacing it with `slot`
    ///
    /// # Usage
    ///
    /// ```rust
    /// use ptr_cell::Semantics;
    ///
    /// // Construct an empty cell
    /// let cell = ptr_cell::PtrCell::default();
    ///
    /// // Initialize a pair of values
    /// let odd = Some(vec![1, 3, 5]);
    /// let even = Some(vec![2, 4, 6]);
    ///
    /// // Replace the value multiple times
    /// assert_eq!(cell.replace(odd.clone(), Semantics::Relaxed), None);
    /// assert_eq!(cell.replace(even.clone(), Semantics::Relaxed), odd);
    /// assert_eq!(cell.replace(None, Semantics::Relaxed), even)
    /// ```
    #[inline(always)]
    pub fn replace(&self, slot: Option<T>, order: Semantics) -> Option<T> {
        let new_leak = Self::heap_leak(slot);
        let old_leak = self.value.swap(new_leak, order.read_write());

        unsafe { Self::heap_reclaim(old_leak) }
    }

    /// Determines whether this cell is empty
    ///
    /// # Usage
    ///
    /// ```rust
    /// use ptr_cell::Semantics;
    /// use std::collections::HashMap;
    ///
    /// // Construct an empty cell
    /// let cell: ptr_cell::PtrCell<HashMap<u16, String>> = Default::default();
    ///
    /// // Check that the cell's default value is None (empty)
    /// assert!(
    ///     cell.is_empty(Semantics::Relaxed),
    ///     "The cell should be empty by default"
    /// )
    /// ```
    #[inline(always)]
    pub fn is_empty(&self, order: Semantics) -> bool {
        self.value.load(order.read()).is_null()
    }

    /// Constructs a cell with `slot` inside
    ///
    /// # Usage
    ///
    /// ```rust
    /// use ptr_cell::Semantics;
    ///
    /// // Initialize a sample number
    /// const VALUE: Option<u16> = Some(0xFAA);
    ///
    /// // Wrap the number in a cell
    /// let cell = ptr_cell::PtrCell::new(VALUE);
    ///
    /// // Take the number out
    /// assert_eq!(cell.take(Semantics::Relaxed), VALUE)
    /// ```
    #[inline(always)]
    pub fn new(slot: Option<T>) -> Self {
        let ptr = Self::heap_leak(slot);

        unsafe { Self::from_ptr(ptr) }
    }

    /// Constructs a cell that owns the allocation to which `ptr` points
    ///
    /// Passing in a null `ptr` is perfectly valid, as it represents [`None`]. Conversely, a
    /// non-null `ptr` is treated as [`Some`]
    ///
    /// # Safety
    /// The memory pointed to by `ptr` must have been allocated in accordance with the [memory
    /// layout][1] used by [`Box`]
    ///
    /// Dereferencing `ptr` after this function has been called can result in undefined behavior
    ///
    /// # Usage
    ///
    /// ```rust, ignore
    /// use ptr_cell::Semantics;
    ///
    /// // Initialize a sample number
    /// const VALUE: Option<u16> = Some(0xFAA);
    ///
    /// // Allocate the number on the heap and get a pointer to the allocation
    /// let value_ptr = ptr_cell::PtrCell::heap_leak(VALUE);
    ///
    /// // Construct a cell from the pointer
    /// let cell = unsafe { ptr_cell::PtrCell::from_ptr(value_ptr) };
    ///
    /// // Take the number out
    /// assert_eq!(cell.take(Semantics::Relaxed), VALUE)
    /// ```
    ///
    /// [1]: https://doc.rust-lang.org/std/boxed/index.html#memory-layout
    #[inline(always)]
    const unsafe fn from_ptr(ptr: *mut T) -> Self {
        let value = core::sync::atomic::AtomicPtr::new(ptr);

        Self { value }
    }

    /// Reclaims ownership of the memory pointed to by `ptr` and returns the contained value
    ///
    /// **A null pointer represents [None]**
    ///
    /// This function is intended to be the inverse of [`heap_leak`](Self::heap_leak)
    ///
    /// # Safety
    ///
    /// If `ptr` is non-null, the memory it points to must have been allocated in accordance with
    /// the [memory layout][1] used by [`Box`]
    ///
    /// Dereferencing `ptr` after this function has been called is undefined behavior
    ///
    /// [1]: https://doc.rust-lang.org/std/boxed/index.html#memory-layout
    #[inline(always)]
    unsafe fn heap_reclaim(ptr: *mut T) -> Option<T> {
        non_null(ptr).map(|ptr| *Box::from_raw(ptr))
    }

    /// Leaks `slot` to the heap and returns a raw pointer to it
    ///
    /// **[None] is represented by a null pointer**
    ///
    /// The memory will be allocated in accordance with the [memory layout][1] used by [`Box`]
    ///
    /// [1]: https://doc.rust-lang.org/std/boxed/index.html#memory-layout
    #[inline(always)]
    fn heap_leak(slot: Option<T>) -> *mut T {
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
    #[inline(always)]
    fn default() -> Self {
        Self::new(None)
    }
}

impl<T> Drop for PtrCell<T> {
    #[inline(always)]
    fn drop(&mut self) {
        let ptr = *self.value.get_mut();

        let _drop = unsafe { Self::heap_reclaim(ptr) };
    }
}

impl<T> From<T> for PtrCell<T> {
    #[inline(always)]
    fn from(value: T) -> Self {
        Self::new(Some(value))
    }
}

/// Returns `ptr` if it's non-null
#[inline(always)]
fn non_null<T>(ptr: *mut T) -> Option<*mut T> {
    match ptr.is_null() {
        true => None,
        false => Some(ptr),
    }
}

/// Memory ordering semantics for atomic operations. Determines how value updates are synchronized
/// between threads
///
/// If you're not sure what semantics to use, choose [`Coupled`](Semantics::Coupled)
#[non_exhaustive]
#[derive(Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub enum Semantics {
    /// [`SeqCst`](Ordering::SeqCst) semantics
    ///
    /// All memory operations will appear to be executed in a single, total order. See the
    /// documentation for `SeqCst`
    ///
    /// Maximum synchronization constraints and the worst performance
    Ordered = 2,

    /// [`Release`](Ordering::Release)-[`Acquire`](Ordering::Acquire) coupling semantics
    ///
    /// A write that has happened before a read will always be visible to the said read. See the
    /// documentation for `Release` and `Acquire`
    ///
    /// A common assumption is that this is how memory operations naturally behave. Thus, this is
    /// likely the semantics you want to use
    ///
    /// Mild synchronization constraints and fair performance
    Coupled = 1,

    /// [`Relaxed`](Ordering::Relaxed) semantics
    ///
    /// No synchronization constraints and the best performance
    Relaxed = 0,
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
            /// - [`SeqCst`](Ordering::SeqCst) for [`Ordered`](Semantics::Ordered) semantics
            $($returns)*
            /// - [`Relaxed`](Ordering::Relaxed) for [`Relaxed`](Semantics::Relaxed) semantics
            ///
            /// # Usage
            ///
            /// ```rust
            /// use std::sync::atomic::Ordering;
            ///
            /// let semantics = ptr_cell::Semantics::Coupled;
            ///
            $($assert)*
            /// ```
            #[inline(always)]
            pub const fn $name(&self) -> Ordering {
                match self {
                    Self::Ordered => Ordering::SeqCst,
                    Self::Coupled => $coupled,
                    Self::Relaxed => Ordering::Relaxed,
                }
            }
        }
    };
}

operation!(read_write with Ordering::AcqRel: {
    /// Returns the memory ordering for read-write operations with these semantics
}, {
    /// - [`AcqRel`](Ordering::AcqRel) for [`Coupled`](Semantics::Coupled) semantics
}, {
    /// assert_eq!(semantics.read_write(), Ordering::AcqRel)
});

operation!(write with Ordering::Release: {
    /// Returns the memory ordering for write operations with these semantics
}, {
    /// - [`Release`](Ordering::Release) for [`Coupled`](Semantics::Coupled) semantics
}, {
    /// assert_eq!(semantics.write(), Ordering::Release)
});

operation!(read with Ordering::Acquire: {
    /// Returns the memory ordering for read operations with these semantics
}, {
    /// - [`Acquire`](Ordering::Acquire) for [`Coupled`](Semantics::Coupled) semantics
}, {
    /// assert_eq!(semantics.read(), Ordering::Acquire)
});

impl Default for Semantics {
    #[inline(always)]
    fn default() -> Self {
        Self::Coupled
    }
}
