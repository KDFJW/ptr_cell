//! # Simple thread-safe cell
//!
//! [`PtrCell`] is an atomic cell type that allows safe, concurrent access to shared data. No
//! [`std`][1], no [data races][2], no [nasal demons (UB)][3], and most importantly, no [locks][4]
//!
//! This type is only useful in scenarios where you need to update a shared value by moving in and
//! out of it. If you want to concurrently update a value through mutable references and don't
//! require support for environments without the standard library ([`no_std`][5]), take a look at
//! the standard [`Mutex`][6] and [`RwLock`][7] instead
//!
//! #### Offers:
//! - **Ease of use**: The API is fairly straightforward
//! - **Performance**: Core algorithms are at most a couple of instructions long
//!
//! #### Limits:
//! - **Access**: To see what's stored inside a cell, you must either take the value out of it or
//! provide exclusive access (`&mut`) to the cell
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
//! other means, like [fences](core::sync::atomic::fence). As always, the documentation for each
//! item contains more details
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
//!     let cell = ptr_cell::PtrCell::default();
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
//! [1]: https://doc.rust-lang.org/std
//! [2]: https://en.wikipedia.org/wiki/Race_condition#In_software
//! [3]: https://en.wikipedia.org/wiki/Undefined_behavior
//! [4]: https://en.wikipedia.org/wiki/Lock_(computer_science)
//! [5]: https://docs.rust-embedded.org/book/intro/no-std.html
//! [6]: https://doc.rust-lang.org/std/sync/struct.Mutex.html
//! [7]: https://doc.rust-lang.org/std/sync/struct.RwLock.html

#![no_std]
#![warn(missing_docs)]

extern crate alloc;

use alloc::boxed::Box;
use core::sync::atomic::Ordering;

// Add "virtually" to "no locks" in the top-level docs (3.0.0)
// Update the "Limits" section in the top-level docs (3.0.0)
// Implement get by using brief spinlocking (3.0.0)

// Add `set`, `set_ptr`, `swap` and the inverse of `map_owner` (2.2.0)
// Explain how the `map_*` methods are equivalent to the `push` and `pop` of a linked list (2.2.0)

// VVVVVVVVVVVVVVVVVVVVVVVV
// **SPIN HINT!!! (2.2.0)**
// AAAAAAAAAAAAAAAAAAAAAAAA

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
///
/// # Semantics
///
/// All methods that access this cell's data through `&self` inherently require a [`Semantics`]
/// variant, as this is the only way to load the underlying atomic pointer. This parameter is
/// omitted from the documentation of individual methods due to its universal applicability
///
/// # Pointer Safety
///
/// When dereferencing a pointer to the cell's value, you must ensure that the memory it points to
/// hasn't been [reclaimed](Self::heap_reclaim). Notice that calls to [`replace`](Self::replace) and
/// its derivatives ([`set`](Self::set) and [`take`](Self::take)) automatically reclaim memory. This
/// includes any calls made from other threads
///
/// This also applies to external pointers that the cell now manages, like the `ptr` parameter in
/// [`from_ptr`](Self::from_ptr)
#[repr(transparent)]
pub struct PtrCell<T> {
    /// Pointer to the contained value
    ///
    /// # Invariants
    ///
    /// - **If non-null**: Must point to memory that conforms to the [memory layout][1] used by
    ///   [`Box`]
    ///
    /// [1]: https://doc.rust-lang.org/std/boxed/index.html#memory-layout
    value: core::sync::atomic::AtomicPtr<T>,
}

impl<T> PtrCell<T> {
    /// Replaces the cell's value with a new one, constructed from the cell itself using the
    /// provided `new` function
    ///
    /// Despite the fact that this operation is somewhat complex, it's still entirely atomic. This
    /// allows it to be safely used in implementations of shared linked-list-like data structures
    ///
    /// # Usage
    ///
    /// ```rust
    /// fn main() {
    ///     use ptr_cell::Semantics;
    ///
    ///     // Initialize a test sentence
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
    ///     // Check that there were no errors
    ///     assert_eq!(decoded, SENTENCE)
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

            match value_ptr_result {
                Ok(_same) => break,
                Err(modified) => *value_ptr = modified,
            }
        }
    }

    /// Returns the cell's value, replacing it with [`None`]
    ///
    /// This is an alias for `self.replace(None, order)`
    ///
    /// # Usage
    ///
    /// ```rust
    /// use ptr_cell::Semantics;
    ///
    /// // Initialize a test value
    /// const VALUE: Option<u8> = Some(0b01000101);
    ///
    /// // Wrap the value in a cell
    /// let cell = ptr_cell::PtrCell::new(VALUE);
    ///
    /// // Take the value out
    /// assert_eq!(cell.take(Semantics::Relaxed), VALUE);
    ///
    /// // Verify that the cell is now empty
    /// assert_eq!(cell.take(Semantics::Relaxed), None)
    /// ```
    #[inline(always)]
    pub fn take(&self, order: Semantics) -> Option<T> {
        self.replace(None, order)
    }

    /// Returns the pointer to the cell's value, replacing the pointer with a null one
    ///
    /// This is an alias for `self.replace_ptr(core::ptr::null_mut(), order)`
    ///
    /// # Usage
    ///
    /// ```rust
    /// use ptr_cell::Semantics;
    ///
    /// // Initialize a test value
    /// const VALUE: Option<u8> = Some(0b01000101);
    ///
    /// // Wrap the value in a cell
    /// let cell = ptr_cell::PtrCell::new(VALUE);
    ///
    /// // Take the pointer out
    /// let ptr = cell.take_ptr(Semantics::Relaxed);
    ///
    /// // Get the value back
    /// assert_eq!(unsafe { ptr_cell::PtrCell::heap_reclaim(ptr) }, VALUE)
    /// ```
    #[inline(always)]
    pub fn take_ptr(&self, order: Semantics) -> *mut T {
        self.replace_ptr(core::ptr::null_mut(), order)
    }

    /// Returns the cell's value, replacing it with `slot`
    ///
    /// # Usage
    ///
    /// ```rust
    /// use ptr_cell::Semantics;
    ///
    /// // Initialize a pair of test values
    /// const SEMI: Option<char> = Some(';');
    /// const COLON: Option<char> = Some(':');
    ///
    /// // Wrap one of the values in a cell
    /// let cell = ptr_cell::PtrCell::new(SEMI);
    ///
    /// // Replace the value
    /// assert_eq!(cell.replace(COLON, Semantics::Relaxed), SEMI);
    ///
    /// // ...and get one back
    /// assert_eq!(cell.replace(None, Semantics::Relaxed), COLON)
    /// ```
    ///
    /// **Note**: For taking the value out of a cell, using [`take`](Self::take) is recommended
    #[inline(always)]
    pub fn replace(&self, slot: Option<T>, order: Semantics) -> Option<T> {
        let new_leak = Self::heap_leak(slot);
        let old_leak = self.replace_ptr(new_leak, order);

        unsafe { Self::heap_reclaim(old_leak) }
    }

    /// Returns the pointer to the cell's value, replacing the pointer with `ptr`
    ///
    /// **WARNING: THIS FUNCTION WAS ERRONEOUSLY MARKED AS SAFE. IT SHOULD BE UNSAFE AND WILL BE
    /// MARKED AS SUCH IN THE NEXT MAJOR RELEASE**
    ///
    /// # Safety
    ///
    /// The memory `ptr` points to must conform to the [memory layout][1] used by [`Box`]
    ///
    /// # Usage
    ///
    /// ```rust
    /// use ptr_cell::{PtrCell, Semantics};
    ///
    /// unsafe {
    ///     // Allocate a pair of test values on the heap
    ///     let semi = PtrCell::heap_leak(Some(';'));
    ///     let colon = PtrCell::heap_leak(Some(':'));
    ///
    ///     // Construct a cell from one of the allocations
    ///     let cell = PtrCell::from_ptr(semi);
    ///
    ///     // Replace the pointer to the allocation
    ///     assert_eq!(cell.replace_ptr(colon, Semantics::Relaxed), semi);
    ///
    ///     // ...and get one back
    ///     let null = std::ptr::null_mut();
    ///     assert_eq!(cell.replace_ptr(null, Semantics::Relaxed), colon);
    ///
    ///     // Clean up
    ///     PtrCell::heap_reclaim(semi);
    ///     PtrCell::heap_reclaim(colon);
    /// }
    /// ```
    ///
    /// **Note**: For taking the pointer out of a cell, using [`take_ptr`](Self::take_ptr) is
    /// recommended
    ///
    /// [1]: https://doc.rust-lang.org/std/boxed/index.html#memory-layout
    #[inline(always)]
    pub fn replace_ptr(&self, ptr: *mut T, order: Semantics) -> *mut T {
        self.value.swap(ptr, order.read_write())
    }

    /// Mutably borrows the cell's value
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

    /// Returns a pointer to the cell's value
    ///
    /// # Safety
    ///
    /// The cell's value may get deallocated at any moment. Because of this, it's hard to safely
    /// dereference the resulting pointer. Refer to the [Pointer Safety][1] section for more details
    ///
    /// # Usage
    ///
    /// ```rust
    /// use ptr_cell::Semantics;
    ///
    /// // Construct an empty cell
    /// let cell: ptr_cell::PtrCell<[u8; 3]> = Default::default();
    ///
    /// // Get the cell's pointer
    /// assert_eq!(cell.get_ptr(Semantics::Relaxed), std::ptr::null_mut())
    /// ```
    ///
    /// [1]: https://docs.rs/ptr_cell/2.2.0/ptr_cell/struct.PtrCell.html#pointer-safety
    #[inline(always)]
    pub fn get_ptr(&self, order: Semantics) -> *mut T {
        self.value.load(order.read())
    }

    /// Determines whether this cell is empty
    ///
    /// # Usage
    ///
    /// ```rust
    /// use ptr_cell::Semantics;
    ///
    /// // Construct an empty cell
    /// let cell: ptr_cell::PtrCell<[char; 3]> = Default::default();
    ///
    /// // Check that the cell's default value is None (empty)
    /// assert!(cell.is_empty(Semantics::Relaxed))
    /// ```
    #[inline(always)]
    pub fn is_empty(&self, order: Semantics) -> bool {
        self.get_ptr(order).is_null()
    }

    /// Constructs a cell with `slot` inside
    ///
    /// # Usage
    ///
    /// ```rust
    /// use ptr_cell::Semantics;
    ///
    /// // Initialize a test value
    /// const VALUE: Option<u16> = Some(0xFAA);
    ///
    /// // Wrap the value in a cell
    /// let cell = ptr_cell::PtrCell::new(VALUE);
    ///
    /// // Take the value out
    /// assert_eq!(cell.take(Semantics::Relaxed), VALUE)
    /// ```
    #[inline(always)]
    pub fn new(slot: Option<T>) -> Self {
        let ptr = Self::heap_leak(slot);

        unsafe { Self::from_ptr(ptr) }
    }

    /// Constructs a cell that owns the memory to which `ptr` points
    ///
    /// **A null pointer represents [None]**
    ///
    /// # Safety
    ///
    /// The memory must conform the [memory layout][1] used by [`Box`]
    ///
    /// # Usage
    ///
    /// ```rust
    /// use ptr_cell::Semantics;
    ///
    /// // Initialize a test value
    /// const VALUE: Option<u16> = Some(0xFAA);
    ///
    /// // Allocate the value on the heap and get a pointer to it
    /// let value_ptr = ptr_cell::PtrCell::heap_leak(VALUE);
    ///
    /// // Construct a cell from the pointer
    /// let cell = unsafe { ptr_cell::PtrCell::from_ptr(value_ptr) };
    ///
    /// // Take the value out
    /// assert_eq!(cell.take(Semantics::Relaxed), VALUE)
    /// ```
    ///
    /// [1]: https://doc.rust-lang.org/std/boxed/index.html#memory-layout
    #[inline(always)]
    pub const unsafe fn from_ptr(ptr: *mut T) -> Self {
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
    /// The memory must conform to the [memory layout][1] used by [`Box`]
    ///
    /// Dereferencing `ptr` after this function has been called is undefined behavior
    ///
    /// # Usage
    ///
    /// ```rust
    /// // Initialize a test value
    /// const VALUE: Option<u16> = Some(1155);
    ///
    /// // Give up ownership of the value
    /// let ptr = ptr_cell::PtrCell::heap_leak(VALUE);
    ///
    /// // Get ownership of the value back
    /// assert_eq!(unsafe { ptr_cell::PtrCell::heap_reclaim(ptr) }, VALUE)
    /// ```
    ///
    /// [1]: https://doc.rust-lang.org/std/boxed/index.html#memory-layout
    #[inline(always)]
    pub unsafe fn heap_reclaim(ptr: *mut T) -> Option<T> {
        non_null(ptr).map(|ptr| *Box::from_raw(ptr))
    }

    /// Leaks `slot` to the heap and returns a raw pointer to it
    ///
    /// **[None] is represented by a null pointer**
    ///
    /// The memory will be allocated in accordance with the [memory layout][1] used by [`Box`]
    ///
    /// # Usage
    ///
    /// ```rust
    /// use ptr_cell::Semantics;
    ///
    /// // Allocate a value
    /// let ptr = ptr_cell::PtrCell::heap_leak(1031_u16.into());
    ///
    /// // Transfer ownership of the allocation to a new cell
    /// let cell = unsafe { ptr_cell::PtrCell::from_ptr(ptr) };
    ///
    /// // Check that the cell uses the same allocation
    /// assert_eq!(cell.get_ptr(Semantics::Relaxed), ptr)
    /// ```
    ///
    /// [1]: https://doc.rust-lang.org/std/boxed/index.html#memory-layout
    #[inline(always)]
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
    #[inline(always)]
    fn default() -> Self {
        Self::new(None)
    }
}

impl<T> Drop for PtrCell<T> {
    #[inline(always)]
    fn drop(&mut self) {
        let ptr = *self.value.get_mut();

        unsafe { Self::heap_reclaim(ptr) };
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
/// ### Explanations
///
/// Lock-free programming is not easy to grasp. What's more, resources explaining Rust's atomic
/// orderings in depth are pretty sparse. However, this is not really an issue. Atomics in Rust are
/// almost identical to their C++ counterparts, of which there exist abundant explanations
///
/// Here are just some of them:
///
/// - Although not meant as an introduction to the release-acquire semantics, this [fantastic
/// article][1] by Jeff Preshing definitely provides the much-needed clarification
///
/// - Another [great article][2] by Preshing, but this time dedicated entirely to the concept of the
/// release-acquire semantics
///
/// - [Memory order][3] from the C++ standards. Way more technical, but has all contracts organized
/// in a single place. Please note that Rust lacks a direct analog to C++'s `memory_order_consume`
///
/// If you're still not sure what semantics to use, choose [`Coupled`](Semantics::Coupled)
///
/// [1]: https://preshing.com/20131125/acquire-and-release-fences-dont-work-the-way-youd-expect/
/// [2]: https://preshing.com/20120913/acquire-and-release-semantics/
/// [3]: https://en.cppreference.com/w/cpp/atomic/memory_order
#[non_exhaustive]
#[derive(Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub enum Semantics {
    /// [`Relaxed`](Ordering::Relaxed) semantics
    ///
    /// No synchronization constraints and the best performance
    Relaxed,

    /// [`Release`](Ordering::Release) - [`Acquire`](Ordering::Acquire) coupling semantics
    ///
    /// Mild synchronization constraints and fair performance
    ///
    /// A read will always see the preceding write (if one exists). Any operations that take place
    /// before the write will also be seen, regardless of their semantics. See the documentation for
    /// `Release` and `Acquire`
    ///
    /// A common assumption is that this is how memory operations naturally behave. While it's true
    /// on some platforms (namely, x86 and x86-64), this behavior is not universal. Thus, this is
    /// likely the semantics you want to use
    Coupled,

    /// [`SeqCst`](Ordering::SeqCst) semantics
    ///
    /// Maximum synchronization constraints and the worst performance
    ///
    /// All memory operations will appear to be executed in a single, total order. See the
    /// documentation for `SeqCst`
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
            /// use std::sync::atomic::Ordering;
            ///
            /// // Copy a variant of Semantics
            /// let semantics = ptr_cell::Semantics::Coupled;
            ///
            /// // Get the corresponding Ordering
            $($assert)*
            /// ```
            #[inline(always)]
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
