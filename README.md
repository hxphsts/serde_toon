# serde_toon

A [Serde](https://serde.rs) encoder and decoder for
[TOON](https://github.com/toon-format/spec) (Token-Oriented Object Notation).

toon-spec: 4.1

[![Crates.io](https://img.shields.io/crates/v/serde_toon.svg)](https://crates.io/crates/serde_toon) [![Documentation](https://docs.rs/serde_toon/badge.svg)](https://docs.rs/serde_toon) [![License](https://img.shields.io/crates/l/serde_toon.svg)](LICENSE-MIT)

TOON is a compact, line-oriented encoding of the JSON data model for LLM
prompts: uniform arrays of objects become tables whose field names appear once.
serde_toon implements [TOON spec v4.1](https://github.com/toon-format/spec)
(`serde_toon::SPEC_VERSION`) and passes all 538 official conformance fixtures
(179 encode, 359 decode, spec tag v4.1.1).

JSON (`serde_json::to_string_pretty`), 191 characters:

```json
[
  {
    "id": 1,
    "name": "Alice",
    "email": "alice@example.com",
    "active": true
  },
  {
    "id": 2,
    "name": "Bob",
    "email": "bob@example.com",
    "active": false
  }
]
```

TOON (`serde_toon::to_string`), 89 characters:

```text
[2]{id,name,email,active}:
  1,Alice,alice@example.com,true
  2,Bob,bob@example.com,false
```

*See [`examples/token_efficiency.rs`](examples/token_efficiency.rs).*

## Quick start

```toml
[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_toon = "0.3"
```

```rust
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct User {
    id: u32,
    name: String,
    tags: Vec<String>,
}

fn main() -> Result<(), serde_toon::Error> {
    let user = User { id: 7, name: "Ada".into(), tags: vec!["admin".into(), "ops".into()] };

    let toon = serde_toon::to_string(&user)?;
    assert_eq!(toon, "id: 7\nname: Ada\ntags[2]: admin,ops");

    let back: User = serde_toon::from_str(&toon)?;
    assert_eq!(back, user);
    Ok(())
}
```

## Output

Fields keep their declaration order. The encoder picks the form the
specification requires for each value:

```rust
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Serialize)]
struct Customer { name: &'static str, country: &'static str }
#[derive(Serialize)]
struct Order { id: u32, customer: Customer, total: f64 }
#[derive(Serialize)]
struct Stats { age: u32, city: &'static str }
#[derive(Serialize)]
enum Status { Active, Suspended { reason: &'static str } }
#[derive(Serialize)]
struct Report {
    orders: Vec<Order>,
    users: BTreeMap<&'static str, Stats>,
    tags: Vec<&'static str>,
    empty: Vec<u32>,
    status: Status,
    big: f64,
    label: &'static str,
}

fn main() -> Result<(), serde_toon::Error> {
    let report = Report {
        orders: vec![
            Order { id: 1, customer: Customer { name: "Ada", country: "DK" }, total: 12.5 },
            Order { id: 2, customer: Customer { name: "Bob", country: "UK" }, total: 7.0 },
        ],
        users: BTreeMap::from([
            ("alice", Stats { age: 30, city: "Berlin" }),
            ("bob", Stats { age: 25, city: "Oslo" }),
        ]),
        tags: vec!["rust", "serde"],
        empty: vec![],
        status: Status::Suspended { reason: "unpaid" },
        big: 1e21,
        label: "true",
    };
    let expected = "\
orders[2]{id,customer{name,country},total}:
  1,Ada,DK,12.5
  2,Bob,UK,7
users[2:]{age,city}:
  alice: 30,Berlin
  bob: 25,Oslo
tags[2]: rust,serde
empty: []
status:
  Suspended:
    reason: unpaid
big: 1e+21
label: \"true\"";
    assert_eq!(serde_toon::to_string(&report)?, expected);
    Ok(())
}
```

- Uniform arrays of objects are tables; uniform nested objects become field
  groups (`customer{name,country}`), and maps of uniform objects become keyed
  tables (`users[2:]{...}`).
- Numbers are canonical: `1.0` is `1`, `-0.0` is `0`, very large or small
  floats use exponent form (`1e+21`, `1e-7`), and NaN/+/-infinity are `null`.
  `u64`, `i128` and `u128` are written exactly.
- Strings and keys are quoted only when needed (`"true"`, `"a,b"`, `"user-id"`).
- Enums are externally tagged: `Unit`, `Variant: value`, or `Variant:` with
  nested fields.

`ToonOptions` sets the indentation and the delimiter, which every array
header declares:

```rust
use serde_toon::{to_string_with_options, Delimiter, ToonOptions};

fn main() -> Result<(), serde_toon::Error> {
    let options = ToonOptions::new().with_delimiter(Delimiter::Pipe);
    assert_eq!(to_string_with_options(&["a", "b"], options)?, "[2|]: a|b");

    let options = ToonOptions::new().with_delimiter(Delimiter::Tab);
    assert_eq!(to_string_with_options(&["a", "b"], options)?, "[2\t]: a\tb");
    Ok(())
}
```

## Decoding

The decoder drives serde's visitors directly over the input's lines, without
an intermediate tree; `&str` fields borrow from the input when the value
contains no escapes. It accepts full-line comments (`# ...`), a byte-order mark
and CRLF line endings; errors report line and column. It is property-tested
not to panic on arbitrary input, and nesting is limited to 128 levels.

`DecodeOptions` chooses how strictly the specification is enforced:

| Mode | Spec validation (§14) | serde_toon 0.2 syntax |
|------|-----------------------|-----------------------|
| `DecodeOptions::strict()` | enforced | rejected |
| `DecodeOptions::lenient()` | relaxed | rejected |
| `DecodeOptions::compatible()` (default, used by `from_str`) | relaxed | accepted |

Strict mode checks declared lengths, row widths, indentation, blank lines and
duplicate keys. Use it for untrusted or LLM-generated input:

```rust
use serde::Deserialize;
use serde_toon::{from_str_with_options, DecodeOptions};

#[derive(Deserialize)]
#[allow(dead_code)]
struct Row { id: u32, name: String }

#[derive(Deserialize)]
struct Doc { users: Vec<Row> }

fn main() -> Result<(), serde_toon::Error> {
    // The header declares 3 rows but only 2 follow.
    let input = "users[3]{id,name}:\n  1,Ada\n  2,Bob";

    // The default (compatible) mode accepts it...
    let doc: Doc = serde_toon::from_str(input)?;
    assert_eq!(doc.users.len(), 2);

    // ...strict mode reports the mismatch with its position.
    match from_str_with_options::<Doc>(input, DecodeOptions::strict()) {
        Err(err) => assert_eq!(
            err.to_string(),
            "Invalid TOON format at line 1, column 1: \
             tabular row count mismatch: header declares 3, found 2"
        ),
        Ok(_) => unreachable!("strict mode checks declared lengths"),
    }
    Ok(())
}
```

*See [`examples/strict_decoding.rs`](examples/strict_decoding.rs).*

## Compatibility with 0.2

0.3 writes TOON v4.1, so serialized text differs from 0.2 (no `[#N]` length
markers, `[]` for empty arrays, fields in declaration order instead of sorted,
`null` for NaN and infinities, exponent-form floats). The Rust API is source
compatible. `from_str` still decodes documents written by 0.2; this is covered
by 87 golden tests. See the [changelog](CHANGELOG.md#migrating-from-02).

## Performance

Criterion benchmarks on one machine; indicative only.

| `benches/serialization.rs` | 0.2.0 | 0.3.0 |
|---|---|---|
| serialize simple struct | 292 ns | 149 ns |
| serialize array/500 | 583 µs | 133 µs |
| serialize nested struct | 1.42 µs | 0.50 µs |
| deserialize simple struct | 901 ns | 380 ns |
| deserialize array/500 | 531 µs | 117 µs |
| deserialize nested struct | 1.86 µs | 0.86 µs |

Against the official [`toon-format`](https://crates.io/crates/toon-format)
0.5.0 crate (`benches/vs_toon_format.rs`):

| Workload | encode: serde_toon | encode: toon-format | decode: serde_toon | decode: toon-format |
|---|---|---|---|---|
| tabular, 1k rows | 0.28 ms | 2.46 ms | 0.29 ms | 1.44 ms |
| tabular, 10k rows | 3.3 ms | 32.7 ms | 2.7 ms | 17.9 ms |
| nested | 0.17 ms | 1.43 ms | 0.19 ms | 0.87 ms |
| strings | 0.11 ms | 0.59 ms | 0.15 ms | 0.68 ms |

## Also included

- `Value` and the `toon!` macro for dynamic data
  (see [`examples/dynamic_values.rs`](examples/dynamic_values.rs),
  [`examples/macro.rs`](examples/macro.rs)).
- No `unsafe` code.
- Minimum supported Rust version: 1.70.

## Documentation

API docs: <https://docs.rs/serde_toon>. Specification:
<https://github.com/toon-format/spec>.

## License

MIT OR Apache-2.0
