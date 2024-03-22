# Changelog

## 1.0.1 - 3/22/24

### Changed
- The top-level example now uses `Semantics::Relaxed` instead of `Semantics::Coupled`

## 1.0.0 - 3/21/24

### Added
- `PtrCell`: Thread-safe cell based on atomic pointers
- `Semantics`: Memory ordering semantics for `PtrCell`'s atomic operations
