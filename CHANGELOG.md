# Changelog

## 2.2.1 - 6/17/24

### Changed
- Overhauled the _entire_ documentation
- PtrCell::{heap_leak, new, replace_ptr, replace} were annotated with `must_use` to prevent bugs

## 2.2.0 - 4/14/24

### Added
- `PtrCell::swap`: Method for swapping the values of two cells
- `PtrCell::{set, set_ptr}`: Methods for overwriting the cell's value
- A section discussing the pointer API of `PtrCell` in the cell's documentation
- A warning about the safety of `PtrCell::replace_ptr`

### Changed
- Updated the documentation of `PtrCell::map_owner`

## 2.1.1 - 4/10/24

### Fixed
- README now contains the correct version number

## 2.1.0 - 4/10/24

### Added
- `PtrCell::heap_leak`: Associated function for giving up ownership of data
- `PtrCell::heap_reclaim`: Inverse of `PtrCell::heap_leak`
- `PtrCell::{from_ptr, replace_ptr, take_ptr}`: Pointer-based alternatives to some existing methods
- `PtrCell::get_ptr`: Getter for the pointer of `PtrCell`
- A section on the relationship between `PtrCell` and `Semantics` in the cell's documentation
- Comments in the usage of `Semantics::{read, read_write, write}`

### Changed
- `PtrCell::{is_empty, replace}` got better examples

### Fixed
- Fixed broken links in the documentation

## 2.0.0 - 4/6/24

![will it affect me? yes][yes]

### Added
- Links to helpful resources in the documentation for `Semantics`

### Changed
- `PtrCell` now has the same in-memory representation as a `*mut T`
- `PtrCell::new` doesn't require a `Semantics` variant anymore
- `PtrCell::{is_empty, replace, take, map_owner}` now require a `Semantics` variant

### Fixed
- The documentation for `Semantics::Coupled` is now closer to reality

## 1.2.1 - 3/25/24

### Fixed
- Removed the unnecessary `T: Debug` bound in `PtrCell`'s `Debug` implementation

## 1.2.0 - 3/24/24

### Changed
- The library now links against `core` and `alloc` instead of `std`

## 1.1.0 - 3/23/24

### Added
- `PtrCell::map_owner`: Method for linked lists based on `PtrCell`

## 1.0.1 - 3/22/24

### Fixed
- The top-level example now uses `Semantics::Relaxed` instead of `Semantics::Coupled`

## 1.0.0 - 3/21/24

Initial release

[yes]: https://img.shields.io/badge/will%20it%20affect%20me%3F-yes-red.svg
