//! TOON Format Specification
//!
//! This module summarizes the TOON (Token-Oriented Object Notation) format as
//! implemented by this library. The normative text is the
//! [TOON specification v4.1](https://github.com/toon-format/spec)
//! ([`SPEC_VERSION`](crate::SPEC_VERSION)); section numbers below (§) refer
//! to it.
//!
//! # Overview
//!
//! TOON is a line-oriented, indentation-based encoding of the JSON data model
//! designed for efficient token usage in Large Language Model (LLM) contexts.
//! Uniform arrays of objects collapse into tables whose field names appear
//! once, which typically saves 30-60% of the tokens of equivalent JSON.
//!
//! # Objects (§8)
//!
//! ```text
//! name: Alice
//! age: 30
//! user:
//!   id: 1
//!   tags[2]: admin,ops
//! empty:
//! ```
//!
//! - One `key: value` per line, exactly one space after the colon.
//! - A nested object is `key:` with its fields one level deeper (2 spaces by
//!   default); an empty object is `key:` alone. An empty root object is the
//!   empty document.
//! - Fields keep the order in which they are serialized.
//! - Keys are unquoted only when they match `^[A-Za-z_][A-Za-z0-9_.]*$`
//!   (§7.3); otherwise they are quoted: `"user-id": 1`, `"2nd": x`.
//!
//! # Primitives (§2, §7)
//!
//! | Type | Output |
//! |------|--------|
//! | Null | `null` |
//! | Boolean | `true`, `false` |
//! | Integer | exact decimal of any width: `18446744073709551615` |
//! | Float | shortest round-trip decimal without exponent when `1e-6 <= abs(n) < 1e21` (`1.5`, `1` for `1.0`, `0` for `-0.0`); otherwise exponent form `1e+21`, `1e-7` |
//! | NaN, +/-Infinity | `null` |
//! | String | unquoted when safe, otherwise `"quoted"` |
//!
//! A string value is quoted (§7.2) when it is empty; has leading or trailing
//! spaces or tabs; equals `true`, `false`, or `null`; looks numeric (`42`,
//! `-3.14`, `1e-6`, `05`, `+1`); contains `:`, `"`, `\`, `[`, `]`, `{`, `}`,
//! a control character, or the relevant delimiter; or starts with `-` or
//! `#`. Beyond §7.2, a string starting with U+FEFF is also quoted, since a
//! decoder would strip it as a byte-order mark. Quoted strings escape only `\\`, `\"`, `\n`, `\r`, `\t`, and other
//! control characters as `\u00XX` (§7.1).
//!
//! ```text
//! note: hello world
//! data: "a,b"
//! flag: "true"
//! id: "42"
//! item: "- x"
//! ```
//!
//! # Arrays (§9)
//!
//! Every array header declares its length, and every header declares the
//! document delimiter (see below).
//!
//! **Inline**: all elements primitive:
//!
//! ```text
//! tags[3]: admin,ops,dev
//! empty: []
//! ```
//!
//! **Tabular**: all elements non-empty objects with the same keys, every
//! value primitive or a uniform nested object. The header lists the fields
//! in the first element's order; uniform nested objects become nested field
//! groups:
//!
//! ```text
//! items[2]{sku,qty,price}:
//!   A1,2,9.99
//!   B2,1,14.5
//! orders[2]{id,customer{name,country}}:
//!   1,Ada,DK
//!   2,Bob,UK
//! ```
//!
//! **List**: anything else. Objects put their first field on the hyphen
//! line; nested arrays use a keyless header:
//!
//! ```text
//! items[4]:
//!   - 1
//!   - id: 2
//!     name: Ada
//!   - [2]: a,b
//!   -
//! ```
//!
//! A tabular array that is the first field of a list-item object keeps its
//! header on the hyphen line, with its rows two levels deeper (§10):
//!
//! ```text
//! items[1]:
//!   - users[2]{id,name}:
//!       1,Ada
//!       2,Bob
//!     status: active
//! ```
//!
//! # Keyed tabular objects (§9.5)
//!
//! An object (in field or root position) with two or more entries whose
//! values are uniform non-empty objects is written as a table whose rows
//! carry their keys:
//!
//! ```text
//! users[2:]{age,city}:
//!   alice: 30,Berlin
//!   bob: 25,Oslo
//! ```
//!
//! # Delimiters (§11)
//!
//! | Delimiter | Header | Example |
//! |-----------|--------|---------|
//! | Comma (default) | `[N]` | `tags[3]: a,b,c` |
//! | Tab | `[N<TAB>]` | `tags[3<TAB>]: a<TAB>b<TAB>c` |
//! | Pipe | `[N\|]` | `tags[3\|]: a\|b\|c` |
//!
//! The same delimiter separates field names inside `{...}`. Strings containing
//! the chosen delimiter are quoted; other delimiter characters are safe.
//!
//! Length markers (`[#3]`) were removed in TOON v2 and are never emitted;
//! [`ToonOptions::length_marker`](crate::ToonOptions::length_marker) is
//! ignored.
//!
//! # Whitespace (§12)
//!
//! Indentation uses spaces only (2 per level by default, configurable with
//! [`ToonOptions::with_indent`](crate::ToonOptions::with_indent)); lines end
//! with LF; there are no trailing spaces and no trailing newline.
//!
//! # Decoding
//!
//! The decoder reads everything above, including keyed tabular objects,
//! nested field groups, and headers that declare a tab or pipe delimiter:
//!
//! ```rust
//! use serde::Deserialize;
//! use std::collections::BTreeMap;
//!
//! #[derive(Deserialize, Debug, PartialEq)]
//! struct Person { age: u32, city: String }
//!
//! let toon = "users[2:]{age,city}:\n  alice: 30,Berlin\n  bob: 25,Oslo";
//! let doc: BTreeMap<String, BTreeMap<String, Person>> = serde_toon::from_str(toon)?;
//! assert_eq!(doc["users"]["bob"], Person { age: 25, city: "Oslo".into() });
//! # Ok::<(), serde_toon::Error>(())
//! ```
//!
//! Input may also contain what an encoder never writes (§5.1, §12):
//!
//! - **Comments**: a line whose first non-space character is `#` is ignored,
//!   at any indentation. `#` after a value (`a: 1 # x`) is part of the value,
//!   and a `#` line indented with tabs is not a comment.
//! - A leading byte-order mark (U+FEFF), CRLF line endings, trailing spaces,
//!   and blank lines between fields.
//!
//! Trailing content after a complete root array, keyed root object or root
//! primitive is an error in every mode.
//!
//! ## Strictness (§14)
//!
//! [`DecodeOptions`](crate::DecodeOptions) selects one of three modes:
//!
//! - **Strict** ([`DecodeOptions::strict`](crate::DecodeOptions::strict)):
//!   every §14 violation is an error with its line and column: an array
//!   length or tabular row count that differs from the header, a row with
//!   the wrong number of cells, indentation that is not a multiple of the
//!   indent size, tabs used for indentation, a blank line inside an array or
//!   keyed object, and duplicate sibling keys.
//! - **Lenient** ([`DecodeOptions::lenient`](crate::DecodeOptions::lenient)):
//!   the spec's non-strict mode. Declared lengths are not enforced, missing
//!   cells leave fields absent and extra cells are ignored, indentation is rounded down to a level (a
//!   leading tab counts as one level), and duplicate keys are passed to the
//!   visitor in document order (maps keep the last value).
//! - **Compatible** ([`DecodeOptions::compatible`](crate::DecodeOptions::compatible),
//!   the default and what [`from_str`](crate::from_str) uses): lenient, plus
//!   the syntax serde_toon 0.2 wrote: `[#N]` length markers, tab headers
//!   written with spaces (`[2    ]`), headers after a key's colon
//!   (`key: [2]: a,b`), `NaN`/`Infinity`/`inf` float tokens and the 0.2 root
//!   enum layout.
//!
//! The [`de`](crate::de) module documents the remaining
//! implementation-defined choices (number conversion, typed hints, nesting
//! limits).
//!
//! # Rust types
//!
//! The mapping from Rust and serde types to TOON (enums, maps, `Option`,
//! bytes, 128-bit integers, [`Value`](crate::Value) variants) is documented
//! in the [`ser`](crate::ser) module.
//!
//! # Format comparison
//!
//! **JSON** (126 chars):
//! ```json
//! [
//!   {"id":1,"name":"Alice","email":"alice@ex.com","active":true},
//!   {"id":2,"name":"Bob","email":"bob@ex.com","active":true}
//! ]
//! ```
//!
//! **TOON** (78 chars):
//! ```text
//! [2]{id,name,email,active}:
//!   1,Alice,alice@ex.com,true
//!   2,Bob,bob@ex.com,true
//! ```

// This module contains only documentation; no implementation code
