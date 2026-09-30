# Changelog

## [0.3.0] - 2026-09-30

The encoder and decoder were rewritten to target the
[TOON specification v4.1](https://github.com/toon-format/spec)
(`serde_toon::SPEC_VERSION`). The crate passes all 538 official conformance
fixtures (179 encode, 359 decode) from spec tag v4.1.1, vendored in
`tests/spec`.

**The serialized text changes** (see "Migrating from 0.2" below). The Rust API
is source compatible: no public item was removed or changed signature, and
`cargo semver-checks check-release --baseline-version 0.2.0 --release-type minor`
reports no breaking change. Documents written by 0.2 still decode with
`from_str`.

### Added

- `DecodeOptions` with three modes: `strict()` (spec §14 validation),
  `lenient()` (spec non-strict mode) and `compatible()` (the default:
  lenient plus the syntax serde_toon 0.2 wrote), and `with_indent_size`.
- `from_str_with_options`, `from_slice_with_options`,
  `from_reader_with_options` and `Deserializer::from_str_with_options`.
- `SPEC_VERSION` constant (`"4.1"`).
- Encoder: nested field groups in tabular headers
  (`orders[2]{id,customer{name,country}}:`), keyed tabular objects
  (`users[2:]{age,city}:` with `alice: 30,Berlin` rows), delimiter-declaring
  headers (`[N\t]`, `[N|]`) and the §10 list-item layout.
- Encoder: newtype, tuple and struct enum variants now serialize at any depth
  (externally tagged, `Variant: value`); 0.2 returned "Unsupported type".
- Encoder: exact `u64`, `i128` and `u128` output.
- Decoder: comments (`#`), a leading byte-order mark, CRLF line endings,
  keyed tabular objects, nested field groups, tab and pipe delimiters.
- Decoder: zero-copy strings: `&str` and `Cow<str>` fields borrow from the
  input when the value contains no escape sequences.
- Decoder: typed `i128`/`u128` targets decode exactly; `Value` decodes
  integers above `i64::MAX` (up to `u64::MAX`) as `Value::BigInt`.
- `Debug` for `Serializer` and `Deserializer`; `Eq` and `Hash` for
  `Delimiter`; `PartialEq` for `ToonOptions`.
- `ValueSerializer`/`to_value` handle nested enum variants and wide integers.
- Crate docs: serde data model mapping table in the `ser` module and a
  decoding section in the `spec` module.
- `examples/strict_decoding.rs`.

### Changed

- **Output format** (TOON v4.1). Length markers (`[#N]`) are never written;
  tab headers are `[N\t]` (0.2 wrote four spaces); empty arrays are
  `key: []`; array headers follow the key directly (`tags[2]: a,b`, 0.2 wrote
  `tags: [2]: a,b`); struct fields keep declaration order instead of being
  sorted; keys are quoted when §7.3 requires it (`"user-id": 1`); strings
  are quoted per §7.2, and also when they start with U+FEFF; nested arrays
  in list items are written `- [M]: ...`.
- **Numbers are canonical** (§2): NaN and +/-infinity are written as `null`,
  `-0.0` as `0`, and exponent form is used outside `1e-6 <= |n| < 1e21`
  (`1e+21`, `1e-7`).
- Tabular form is used whenever the spec requires it, including rows with
  uniform nested objects.
- The decoder is a streaming `serde::Deserializer` over the input's lines;
  it no longer builds an intermediate `Value` tree.
- Type errors (such as a string where a number is expected) now carry the
  line and column of the offending token, like syntax errors.
- Nesting deeper than 128 objects/arrays (64 for field groups within one
  header) is an error instead of exhausting the stack.
- `ToonOptions::indent == 0` is now an error when serializing.
- `ToonOptions::pretty` / `to_string_pretty` produce the same text as the
  defaults (TOON output is always indented).

### Deprecated

- `ToonOptions::length_marker` and `ToonOptions::with_length_marker`: TOON v2+
  forbids length markers, so the setting is ignored.

### Fixed

- Decoding of unquoted strings starting with `t`, `f` or `n` (`name: frank`
  failed with "Expected null").
- Decoding of exponent numbers (`1e5` decoded as `1.0`).
- Decoding of tab- and pipe-delimited tables, quoted strings inside delimited
  arrays, lists of objects, `Option` fields, unit variants inside sequences
  and nested primitive arrays in lists.
- `from_str` rejects trailing content after a complete root value
  (`[2]: 1,2\ngarbage` was accepted).
- Out-of-range integers are a type error instead of being truncated
  (`"300"` as `u8` decoded as `44`).
- The decoder never panics on arbitrary input (property-tested in
  `tests/decode_robustness.rs`).

### Performance

Criterion benchmarks on one machine; indicative only.

| Benchmark (`benches/serialization.rs`) | 0.2.0 | 0.3.0 |
|---|---|---|
| serialize simple struct | 292 ns | 149 ns |
| serialize array/500 | 583 µs | 133 µs |
| serialize nested struct | 1.42 µs | 0.50 µs |
| deserialize simple struct | 901 ns | 380 ns |
| deserialize array/500 | 531 µs | 117 µs |
| deserialize nested struct | 1.86 µs | 0.86 µs |

Against the official `toon-format` 0.5.0 crate (`benches/vs_toon_format.rs`):

| Workload | serde_toon encode | toon-format encode | serde_toon decode | toon-format decode |
|---|---|---|---|---|
| tabular, 1k rows | 0.28 ms | 2.46 ms | 0.29 ms | 1.44 ms |
| tabular, 10k rows | 3.3 ms | 32.7 ms | 2.7 ms | 17.9 ms |
| nested | 0.17 ms | 1.43 ms | 0.19 ms | 0.87 ms |
| strings | 0.11 ms | 0.59 ms | 0.15 ms | 0.68 ms |

### Migrating from 0.2

- **Reading old data needs no change.** `from_str`, `from_slice` and
  `from_reader` use `DecodeOptions::compatible()`, which still accepts what
  0.2 wrote: `[#N]` length markers, four-space tab headers, `key: [N]: ...`
  headers, `NaN`/`inf`/`-inf` tokens and the 0.2 root enum layout
  (`Rect:w: 2`). 87 golden documents produced by 0.2.0 are checked in
  `tests/legacy_compat.rs`.
- **Written text differs.** If you compare serialized output byte for byte
  (snapshots, caches, hashes), regenerate it. Tools that parse the output
  must accept TOON v4.1: `[]` for empty arrays, `[N\t]`/`[N|]` headers,
  quoted keys, `null` for non-finite floats and exponent-form numbers.
- **Field order.** Fields are written in declaration order; 0.2 sorted them
  alphabetically. Reorder struct fields if a particular column order matters.
- **Non-finite floats** now round-trip as `None`/`null`, not `NaN`/`inf`;
  decoding `null` into an `f64` field is an error, so use `Option<f64>` if
  such values occur.
- **Validation.** To reject malformed input (wrong lengths, bad
  indentation, duplicate keys), decode with `DecodeOptions::strict()`.
- Remove calls to `with_length_marker`; they have no effect.
- `Delimiter` gained `Eq` and `Hash` but is deliberately not `Copy`.

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
