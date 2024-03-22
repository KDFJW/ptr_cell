//! # Simple thread-safe cell
//!
//! [`PtrCell`] is an atomic cell type that allows safe, concurrent access to shared data. No [data
//! races][1], no [nasal demons (UB)][2], and most importantly, no [locks][3]
//!
//! This type is only useful in scenarios where you need to update a shared value by moving in and
//! out of it. If you want to concurrently update a value through mutable references, take a look at
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
//! // Construct a new cell with default coupled semantics
//! let cell: ptr_cell::PtrCell<u16> = 0x81D.into();
//!
//! // Replace the value inside the cell
//! assert_eq!(cell.replace(Some(2047)), Some(0x81D));
//!
//! // Check whether the cell is empty
//! assert_eq!(cell.is_empty(), false);
//!
//! // Take the value out of the cell
//! assert_eq!(cell.take(), Some(2047))
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
//! fn main() {
//!     // Initialize an array of random numbers
//!     const VALUES: [u8; 11] = [47, 12, 88, 45, 67, 34, 78, 90, 11, 77, 33];
//!
//!     // Construct a cell to hold the current maximum value
//!     let cell = ptr_cell::PtrCell::new(None, ptr_cell::Semantics::Relaxed);
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
//!     assert_eq!(maximum.take(), Some(90))
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
//!             let previous = buffer.replace(slot);
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

// You WILL document your code and you WILL like it
#![warn(missing_docs)]

use std::sync::atomic::Ordering;

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
/// # Usage
///
/// ```rust
/// // Construct a new cell with default coupled semantics
/// let cell: ptr_cell::PtrCell<u16> = 0x81D.into();
///
/// // Replace the value inside the cell
/// assert_eq!(cell.replace(Some(2047)), Some(0x81D));
///
/// // Check whether the cell is empty
/// assert_eq!(cell.is_empty(), false);
///
/// // Take the value out of the cell
/// assert_eq!(cell.take(), Some(2047))
/// ```
#[derive(Debug)]
pub struct PtrCell<T> {
    /// Pointer to the contained value
    value: std::sync::atomic::AtomicPtr<T>,
    /// Group of memory orderings for internal atomic operations
    order: Semantics,
}

impl<T> PtrCell<T> {
    /// Returns a mutable reference to the cell's value
    ///
    /// # Usage
    ///
    /// ```rust
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
    /// assert_eq!(text.take(), Some(sentence))
    /// ```
    #[inline(always)]
    pub fn get_mut(&mut self) -> Option<&mut T> {
        let read = self.order.read();
        let leaked = self.value.load(read);

        non_null(leaked).map(|ptr| unsafe { &mut *ptr })
    }

    /// Returns the cell's value, leaving [`None`] in its place
    ///
    /// This is an alias for `self.replace(None)`
    ///
    /// # Usage
    ///
    /// ```rust
    /// // Initialize a sample number
    /// const VALUE: Option<u8> = Some(0b01000101);
    ///
    /// // Wrap the number in a cell
    /// let ordered = ptr_cell::Semantics::Ordered;
    /// let cell = ptr_cell::PtrCell::new(VALUE, ordered);
    ///
    /// // Take the number out
    /// assert_eq!(cell.take(), VALUE);
    ///
    /// // Verify that the cell is now empty
    /// assert_eq!(cell.take(), None)
    /// ```
    #[inline(always)]
    pub fn take(&self) -> Option<T> {
        self.replace(None)
    }

    /// Returns the cell's value, replacing it with `slot`
    ///
    /// # Usage
    ///
    /// ```rust
    /// // Construct an empty cell
    /// let cell = ptr_cell::PtrCell::new(None, Default::default());
    ///
    /// // Initialize a pair of values
    /// let odd = Some(vec![1, 3, 5]);
    /// let even = Some(vec![2, 4, 6]);
    ///
    /// // Replace the value multiple times
    /// assert_eq!(cell.replace(odd.clone()), None);
    /// assert_eq!(cell.replace(even.clone()), odd);
    /// assert_eq!(cell.replace(None), even)
    /// ```
    #[inline(always)]
    pub fn replace(&self, slot: Option<T>) -> Option<T> {
        let read_write = self.order.read_write();

        let new_leaked = Self::heap_leak(slot);
        let old_leaked = self.value.swap(new_leaked, read_write);

        non_null(old_leaked).map(|ptr| *unsafe { Box::from_raw(ptr) })
    }

    /// Determines whether this cell is empty
    ///
    /// # Usage
    ///
    /// ```rust
    /// use ptr_cell::PtrCell;
    /// use std::collections::HashMap;
    ///
    /// // Construct an empty cell
    /// let cell: PtrCell<HashMap<u16, String>> = PtrCell::default();
    ///
    /// // The cell's default value is None (empty)
    /// assert!(cell.is_empty(), "The cell should be empty by default")
    /// ```
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        let read = self.order.read();

        self.value.load(read).is_null()
    }

    /// Constructs a cell with `slot` inside and `order` as its memory ordering
    ///
    /// # Usage
    ///
    /// ```rust
    /// // Initialize a sample number
    /// const VALUE: Option<u16> = Some(0xFAA);
    ///
    /// // Wrap the number in a cell
    /// let ordered = ptr_cell::Semantics::Ordered;
    /// let cell = ptr_cell::PtrCell::new(VALUE, ordered);
    ///
    /// // Take the number out
    /// assert_eq!(cell.take(), VALUE)
    /// ```
    #[inline(always)]
    pub fn new(slot: Option<T>, order: Semantics) -> Self {
        let leaked = Self::heap_leak(slot);
        let value = std::sync::atomic::AtomicPtr::new(leaked);

        Self { value, order }
    }

    /// Sets the memory ordering of this cell to `order`
    ///
    /// # Usage
    ///
    /// ```rust
    /// use ptr_cell::{PtrCell, Semantics};
    ///
    /// // Construct a cell with relaxed semantics
    /// let mut cell: PtrCell<Vec<u8>> = PtrCell::new(None, Semantics::Relaxed);
    ///
    /// // Change the semantics to coupled
    /// cell.set_order(Semantics::Coupled);
    ///
    /// // Check the updated semantics
    /// assert_eq!(cell.get_order(), Semantics::Coupled)
    /// ```
    #[inline(always)]
    pub fn set_order(&mut self, order: Semantics) {
        self.order = order
    }

    /// Returns the current memory ordering of this cell
    ///
    /// # Usage
    ///
    /// ```rust
    /// use ptr_cell::PtrCell;
    ///
    /// // Construct a cell with relaxed semantics
    /// let relaxed = ptr_cell::Semantics::Relaxed;
    /// let cell: PtrCell<String> = PtrCell::new(None, relaxed);
    ///
    /// // Check the cell's semantics
    /// assert_eq!(cell.get_order(), relaxed)
    /// ```
    #[inline(always)]
    pub fn get_order(&self) -> Semantics {
        self.order
    }

    /// Returns a raw pointer to the value contained within `slot`
    ///
    /// Works differently depending on the `slot`'s variant:
    /// - [`Some(T)`]: allocates `T` on the heap using [`Box`] and leaks it
    /// - [`None`]: creates a null pointer
    #[inline(always)]
    fn heap_leak(slot: Option<T>) -> *mut T {
        let Some(value) = slot else {
            return std::ptr::null_mut();
        };

        let allocation = Box::new(value);
        Box::into_raw(allocation)
    }
}

impl<T> Default for PtrCell<T> {
    /// Constructs an empty cell with the memory ordering of [`Coupled`](Semantics::Coupled)
    #[inline(always)]
    fn default() -> Self {
        Self::new(None, Default::default())
    }
}

impl<T> Drop for PtrCell<T> {
    #[inline(always)]
    fn drop(&mut self) {
        let _drop = self.take();
    }
}

impl<T> From<T> for PtrCell<T> {
    #[inline(always)]
    fn from(value: T) -> Self {
        Self::new(Some(value), Default::default())
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
