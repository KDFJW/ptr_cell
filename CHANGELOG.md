# Changelog

## 2.1.0 - 4/10/24

### Added
- `PtrCell::heap_leak`: Associated function for giving up ownership of data
- `PtrCell::heap_reclaim`: Inverse of `PtrCell::heap_leak`
- `PtrCell::{from_ptr, replace_ptr, take_ptr}`: Pointer-based alternatives to some existing methods
- `PtrCell::get_ptr`: Getter for the pointer of `PtrCell`
- A section on `Semantics` in `PtrCell`'s methods to the cell's documentation
- Comments to the usage of `Semantics::{read, read_write, write}`

### Changed
- Used cleaner examples for `PtrCell::{is_empty, replace}`

### Fixed
- Broken links in the documentation

## 2.0.0 - 4/6/24

![will it affect me? yes][yes]

### Added
- Links to helpful resources in the documentation for `Semantics`

### Changed
- `PtrCell` now has the same in-memory representation as a `*mut T`
- `PtrCell::new` doesn't require a `Semantics` variant anymore
- `PtrCell::{is_empty, replace, take, map_owner}` now require a `Semantics` variant
- The documentation for `Semantics::Coupled` now better reflects the reality

## 1.2.1 - 3/25/24

### Fixed
- Removed the unnecessary `T: Debug` bound from the `PtrCell`'s `Debug` implementation

## 1.2.0 - 3/24/24

### Changed
- The library now links against `core` and `alloc` instead of `std`

## 1.1.0 - 3/23/24

### Added
- `PtrCell::map_owner`: Method for linked lists based on `PtrCell`

## 1.0.1 - 3/22/24

### Changed
- The top-level example now uses `Semantics::Relaxed` instead of `Semantics::Coupled`

## 1.0.0 - 3/21/24

### Added
- `PtrCell`: Thread-safe cell based on atomic pointers
- `Semantics`: Memory ordering semantics for `PtrCell`'s atomic operations

<!-- References -->
[yes]: https://img.shields.io/badge/will%20it%20affect%20me%3F-yes-red.svg
