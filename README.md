# Simple thread-safe cell for Rust

[`PtrCell`][1] is an atomic cell type that allows safe, concurrent access to shared data. No
[`std`][2], no data races, no [nasal demons][3] (undefined behavior), and most importantly, no locks

This type is only useful in scenarios where you need to update a shared value by moving in and out
of it. If you want to concurrently update a value through mutable references and don't require
support for `no_std`, take a look at the standard [`Mutex`][4] and [`RwLock`][5] instead

#### Offers:

- **Familiarity**: `PtrCell`'s API was modelled after `std`'s [Cell][6]

- **Easy Concurrency**: No more `Arc<Mutex<T>>`, `Arc::clone()`, and `Mutex::lock().expect()`! Leave
the data static and then point to it when you need to. It's a _single instruction_ on most modern
platforms

#### Limitations:

- **Heap Allocation**: Every value you insert into `PtrCell` must first be allocated using
[`Box`][7]. Allocating on the heap is, computationally, a moderately expensive operation. To address
this, the cell exposes a pointer API that can be used to avoid allocating the same values multiple
times. Future releases will primarily rely on the stack

## Table of Contents
- [Installation](#installation)
- [Usage](#usage)
- [Semantics](#semantics)
- [Examples](#examples)
- [Contributing](#contributing)
- [License](#license)

## Installation

Just add the crate using Cargo:

```shell
cargo add ptr_cell
```

## Usage

```rust
use ptr_cell::{PtrCell, Semantics::Relaxed};

let cell: PtrCell<u16> = 0x81D.into();

assert_eq!(cell.replace(Some(2047), Relaxed), Some(0x81D));
assert_eq!(cell.is_empty(Relaxed), false);
assert_eq!(cell.take(Relaxed), Some(2047))
```

## Semantics

`PtrCell` allows you to specify memory ordering semantics for its internal atomic operations through
the [`Semantics`][8] enum. Each variant is different in how it balances synchronization and
performace. Here's a comparison of the available semantics:

| Variant | Overhead | Synchronization |
|---|---|---|
| [`Relaxed`][9] | Negligible | None |
| [`Coupled`][10] | Acceptable | Intuitive |
| [`Ordered`][11] | Noticeable | Strict |

`Coupled` is what you'd typically use. However, other orderings have their use cases too. For
example, the `Relaxed` semantics could be useful when the operations are already synchronized
through other means, like [fences][12]. As always, the documentation for each item contains more
details

## Examples

The code below finds the maximum value of a sequence by concurrently processing its halves. Notice
how the code doesn't read the shared value. Instead, it uses moves and corrects previous operations
as new data comes in

```rust
use ptr_cell::{PtrCell, Semantics};
use std::sync::Arc;

fn main() {
    const VALUES: [u8; 11] = [47, 12, 88, 45, 67, 34, 78, 90, 11, 77, 33];

    let cell = PtrCell::default();
    let maximum = Arc::new(cell);

    let (left, right) = VALUES.split_at(VALUES.len() / 2);

    let handles = [left, right].map(|half| {
        let maximum = Arc::clone(&maximum);

        std::thread::spawn(move || maximize_in(half, &maximum))
    });

    for worker in handles {
        if let Err(payload) = worker.join() {
            std::panic::resume_unwind(payload)
        }
    }

    assert_eq!(maximum.take(), Some(90))
}

fn maximize_in<T>(sequence: &[T], buffer: &PtrCell<T>)
where
    T: Ord + Copy,
{
    for &item in sequence {
        let mut slot = Some(item);

        loop {
            let previous = buffer.replace(slot, Semantics::Relaxed);

            match slot < previous {
                true => slot = previous,
                false => break,
            }
        }
    }
}
```

## Contributing

Yes, please! See [CONTRIBUTING.md](CONTRIBUTING.md)

Authors of merged pull requests will be rewarded with snacks

## License

Copyright 2024 Nikolay Levkovsky

Individual contributions are copyright by the respective contributors

---

This project is licensed under Creative Commons CC0 1.0 Universal (CC0 1.0) as found in
[LICENSE.txt](LICENSE.txt). CC0 is a public domain dedication tool provided by Creative Commons

[1]: https://docs.rs/ptr_cell/latest/ptr_cell/struct.PtrCell.html
[2]: https://doc.rust-lang.org/std/index.html
[3]: https://groups.google.com/g/comp.std.c/c/ycpVKxTZkgw/m/S2hHdTbv4d8J?hl=en
[4]: https://doc.rust-lang.org/std/sync/struct.Mutex.html
[5]: https://doc.rust-lang.org/std/sync/struct.RwLock.html
[6]: https://doc.rust-lang.org/std/cell/struct.Cell.html
[7]: https://doc.rust-lang.org/std/boxed/struct.Box.html
[8]: https://docs.rs/ptr_cell/latest/ptr_cell/enum.Semantics.html
[9]: https://docs.rs/ptr_cell/latest/ptr_cell/enum.Semantics.html#variant.Relaxed
[10]: https://docs.rs/ptr_cell/latest/ptr_cell/enum.Semantics.html#variant.Coupled
[11]: https://docs.rs/ptr_cell/latest/ptr_cell/enum.Semantics.html#variant.Ordered
[12]: https://doc.rust-lang.org/std/sync/atomic/fn.fence.html
