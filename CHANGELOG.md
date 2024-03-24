# Changelog

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
