# Simple thread-safe cell

[`PtrCell`][1] is an atomic cell type that allows safe, concurrent access to shared data. No [data
races][2], no [nasal demons (UB)][3], and most importantly, no [locks][4]

This type is only useful in scenarios where you need to update a shared value by moving in and out
of it. If you want to concurrently update a value through mutable references, take a look at the
standard [`Mutex`][5] and [`RwLock`][6] instead

#### Offers:
- **Ease of use**: The API is fairly straightforward
- **Performance**: The algorithms are at most a couple of instructions long

#### Limits:
- **Access to the cell's value**: To see what's stored inside a cell, you must either take the value
out of it or have exclusive access to the cell

## Table of Contents
- [Installation](#installation)
- [Usage](#usage)
- [Semantics](#semantics)
- [Examples](#examples)
- [Contributing](#contributing)
- [License](#license)

## Installation

To add `ptr_cell` to your crate's dependencies, run the following command in your project directory:

```shell
cargo add ptr_cell
```

This will add `ptr_cell` to your Cargo.toml file, allowing you to use it in your crate

## Usage

```rust
// Construct a new cell with default coupled semantics
let cell: ptr_cell::PtrCell<u16> = 0x81D.into();

// Replace the value inside the cell
assert_eq!(cell.replace(Some(2047)), Some(0x81D));

// Check whether the cell is empty
assert_eq!(cell.is_empty(), false);

// Take the value out of the cell
assert_eq!(cell.take(), Some(2047))
```

## Semantics

`PtrCell` allows you to specify memory ordering semantics for its internal atomic operations through
the [`Semantics`][7] enum. Choosing appropriate semantics is crucial for achieving the desired level
of synchronization and performance. The available semantics are:

- [`Ordered`][8]: Noticeable overhead, strict
- [`Coupled`][9]: Acceptable overhead, intuitive
- [`Relaxed`][10]: Little overhead, unconstrained

`Coupled` is what you'd typically use. However, other orderings have their use cases too. For
example, the `Relaxed` semantics could be useful when the operations are already ordered through
other means, like [fences][11]. As always, the documentation for each item contains more details

## Examples

Find the maximum value of a sequence of numbers by concurrently processing both of the sequence's
halves

```rust
fn main() {
    // Initialize an array of random numbers
    const VALUES: [u8; 11] = [47, 12, 88, 45, 67, 34, 78, 90, 11, 77, 33];

    // Construct a cell to hold the current maximum value
    let cell = ptr_cell::PtrCell::new(None, ptr_cell::Semantics::Relaxed);
    let maximum = std::sync::Arc::new(cell);

    // Slice the array in two
    let (left, right) = VALUES.split_at(VALUES.len() / 2);

    // Start a worker thread for each half
    let handles = [left, right].map(|half| {
        // Clone `maximum` to move it into the worker
        let maximum = std::sync::Arc::clone(&maximum);

        // Spawn a thread to run the maximizer
        std::thread::spawn(move || maximize_in(half, &maximum))
    });

    // Wait for the workers to finish
    for worker in handles {
        // Check whether a panic occured
        if let Err(payload) = worker.join() {
            // Thread panicked, propagate the panic
            std::panic::resume_unwind(payload)
        }
    }

    // Check the found maximum
    assert_eq!(maximum.take(), Some(90))
}

/// Inserts the maximum of `sequence` and `buffer` into `buffer`
///
/// At least one swap takes place for each value of `sequence`
fn maximize_in<T>(sequence: &[T], buffer: &ptr_cell::PtrCell<T>)
where
    T: Ord + Copy,
{
    // Iterate over the slice
    for &item in sequence {
        // Wrap the item to make the cell accept it
        let mut slot = Some(item);

        // Try to insert the value into the cell
        loop {
            // Replace the cell's value
            let previous = buffer.replace(slot);

            // Determine whether the swap resulted in a decrease of the buffer's value
            match slot < previous {
                // It did, insert the old value back
                true => slot = previous,
                // It didn't, move on to the next item
                false => break,
            }
        }
    }
}
```

## Contributing

Yes, please! See [CONTRIBUTING.md][12]

## License

Either CC0 1.0 Universal or the Apache License 2.0. See [LICENSE.md][13] for more details

<!-- References -->
[1]: https://docs.rs/ptr_cell/latest/ptr_cell/struct.PtrCell.html
[2]: https://en.wikipedia.org/wiki/Race_condition#In_software
[3]: https://en.wikipedia.org/wiki/Undefined_behavior
[4]: https://en.wikipedia.org/wiki/Lock_(computer_science)
[5]: https://doc.rust-lang.org/std/sync/struct.Mutex.html
[6]: https://doc.rust-lang.org/std/sync/struct.RwLock.html
[7]: https://docs.rs/ptr_cell/latest/ptr_cell/enum.Semantics.html
[8]: https://docs.rs/ptr_cell/latest/ptr_cell/enum.Semantics.html#variant.Ordered
[9]: https://docs.rs/ptr_cell/latest/ptr_cell/enum.Semantics.html#variant.Coupled
[10]: https://docs.rs/ptr_cell/latest/ptr_cell/enum.Semantics.html#variant.Relaxed
[11]: https://doc.rust-lang.org/std/sync/atomic/fn.fence.html
[12]: CONTRIBUTING.md
[13]: LICENSE.md
