# Changelog

## [0.2.0] - 2025-01-31

### Breaking Changes

- Renamed `ToonValue` to `Value` throughout the crate for consistency with `serde_json`
- Renamed `ToonValueSerializer` to `ValueSerializer`

### Added

- Comprehensive TOON format specification documentation in new `spec` module

### Changed

- Updated all examples to use `Value` instead of `ToonValue`
- Updated all tests to use new `Value` naming

## [0.1.1] - 2025-01-30

### Added

- Initial public release
- Full Serde integration for TOON serialization/deserialization
- Support for tabular array format (TOON's signature feature)
- `toon!` macro for building `Value` objects
- Custom delimiter support (comma, tab, pipe)
- Comprehensive test suite
- Examples demonstrating key features

### Fixed

- Updated crate documentation
- Improved error messages

[0.2.0]: https://github.com/yourusername/serde_toon/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/yourusername/serde_toon/releases/tag/v0.1.1
