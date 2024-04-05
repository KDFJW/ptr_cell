# Simple thread-safe cell

[`PtrCell`][1] is an atomic cell type that allows safe, concurrent access to shared data. No
[`std`][2], no [data races][3], no [nasal demons (UB)][4], and most importantly, no [locks][5]

This type is only useful in scenarios where you need to update a shared value by moving in and out
of it. If you want to concurrently update a value through mutable references and don't require
support for environments without the standard library ([`no_std`][6]), take a look at the standard
[`Mutex`][7] and [`RwLock`][8] instead

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

This will add `ptr_cell` to your Cargo.toml file, allowing you to use the library in your crate.
Alternatively, you can do this by manually adding the following lines to the file:

```toml
[dependencies.ptr_cell]
version = "2.0.0"
```

## Usage

```rust
use ptr_cell::Semantics;

// Construct a cell
let cell: ptr_cell::PtrCell<u16> = 0x81D.into();

// Replace the value inside the cell
assert_eq!(cell.replace(Some(2047), Semantics::Relaxed), Some(0x81D));

// Check whether the cell is empty
assert_eq!(cell.is_empty(Semantics::Relaxed), false);

// Take the value out of the cell
assert_eq!(cell.take(Semantics::Relaxed), Some(2047))
```

## Semantics

`PtrCell` allows you to specify memory ordering semantics for its internal atomic operations through
the [`Semantics`][9] enum. Choosing appropriate semantics is crucial for achieving the desired level
of synchronization and performance. The available semantics are:

- [`Ordered`][10]: Noticeable overhead, strict
- [`Coupled`][11]: Acceptable overhead, intuitive
- [`Relaxed`][12]: Little overhead, unconstrained

`Coupled` is what you'd typically use. However, other orderings have their use cases too. For
example, the `Relaxed` semantics could be useful when the operations are already ordered through
other means, like [fences][13]. As always, the documentation for each item contains more details

## Examples

Find the maximum value of a sequence of numbers by concurrently processing both of the sequence's
halves

```rust
fn main() {
    // Initialize an array of random numbers
    const VALUES: [u8; 11] = [47, 12, 88, 45, 67, 34, 78, 90, 11, 77, 33];

    // Construct a cell to hold the current maximum value
    let cell = ptr_cell::PtrCell::new(None);
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
            let previous = buffer.replace(slot, ptr_cell::Semantics::Relaxed);

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

Yes, please! See [CONTRIBUTING.md][14]

## License

Either CC0 1.0 Universal or the Apache License 2.0. See [LICENSE.md][15] for more details

<!-- References -->
[1]: https://docs.rs/ptr_cell/latest/ptr_cell/struct.PtrCell.html
[2]: https://doc.rust-lang.org/std/
[3]: https://en.wikipedia.org/wiki/Race_condition#In_software
[4]: https://en.wikipedia.org/wiki/Undefined_behavior
[5]: https://en.wikipedia.org/wiki/Lock_(computer_science)
[6]: https://docs.rust-embedded.org/book/intro/no-std.html
[7]: https://doc.rust-lang.org/std/sync/struct.Mutex.html
[8]: https://doc.rust-lang.org/std/sync/struct.RwLock.html
[9]: https://docs.rs/ptr_cell/latest/ptr_cell/enum.Semantics.html
[10]: https://docs.rs/ptr_cell/latest/ptr_cell/enum.Semantics.html#variant.Ordered
[11]: https://docs.rs/ptr_cell/latest/ptr_cell/enum.Semantics.html#variant.Coupled
[12]: https://docs.rs/ptr_cell/latest/ptr_cell/enum.Semantics.html#variant.Relaxed
[13]: https://doc.rust-lang.org/std/sync/atomic/fn.fence.html
[14]: CONTRIBUTING.md
[15]: LICENSE.md
