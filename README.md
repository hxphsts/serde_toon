# serde_toon

A [Serde](https://serde.rs) encoder and decoder for [TOON](https://github.com/toon-format/spec), the Token-Oriented Object Notation.

[![Crates.io](https://img.shields.io/crates/v/serde_toon.svg)](https://crates.io/crates/serde_toon) [![Documentation](https://docs.rs/serde_toon/badge.svg)](https://docs.rs/serde_toon) [![License](https://img.shields.io/crates/l/serde_toon.svg)](LICENSE-MIT)

## What is TOON?

TOON is a compact encoding of the JSON data model built for LLM prompts: arrays of uniform objects become tables whose field names appear once. serde_toon implements [spec v4.1](https://github.com/toon-format/spec).

JSON, 155 characters:

```json
[
  { "id": 1, "name": "Alice", "email": "alice@example.com", "active": true },
  { "id": 2, "name": "Bob", "email": "bob@example.com", "active": false }
]
```

TOON, 89 characters:

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

## Dynamic values

```rust
use serde_toon::toon;

fn main() -> Result<(), serde_toon::Error> {
    let data = toon!({
        "users": [
            {"id": 1, "name": "Alice"},
            {"id": 2, "name": "Bob"}
        ],
        "active": true
    });
    println!("{}", serde_toon::to_string(&data)?);
    Ok(())
}
```

*See [`examples/dynamic_values.rs`](examples/dynamic_values.rs) and [`examples/macro.rs`](examples/macro.rs).*

## Strict decoding

`from_str` is lenient. For untrusted or LLM-generated input, strict mode enforces the spec's validation rules and reports where the input went wrong:

```rust
use serde_toon::{from_str_with_options, DecodeOptions, Value};

fn main() {
    // The header declares 3 rows but only 2 follow.
    let input = "users[3]{id,name}:\n  1,Ada\n  2,Bob";
    let err = from_str_with_options::<Value>(input, DecodeOptions::strict()).unwrap_err();
    assert!(err.to_string().contains("line 1"));
}
```

*See [`examples/strict_decoding.rs`](examples/strict_decoding.rs).*

## Features

- TOON spec v4.1, passing all 538 official conformance tests
- Full Serde integration: derives, enums, maps, `Option`, 128-bit integers
- Zero-copy decoding: `&str` fields borrow from the input
- Three decoding modes: strict, lenient, and compatible with 0.2 output
- Configurable delimiter (comma, tab, pipe) and indentation
- Errors with line and column
- No unsafe code; MSRV 1.70

## Compatibility with 0.2

The Rust API is unchanged, but 0.3 writes spec-compliant TOON, so the serialized text differs: there are no `[#N]` markers, fields keep declaration order, and NaN becomes `null`. `from_str` still reads everything 0.2 wrote. See the [changelog](CHANGELOG.md#migrating-from-02).

## Performance

Encoding and decoding are 2-4.6x faster than 0.2 ([changelog](CHANGELOG.md)). Compared with the official [`toon-format`](https://crates.io/crates/toon-format) crate (`benches/vs_toon_format.rs`):

| Workload | Encode | Decode |
|---|---|---|
| Tabular, 10k rows | 3.3 ms vs 32.7 ms | 2.7 ms vs 17.9 ms |
| Nested | 0.17 ms vs 1.43 ms | 0.19 ms vs 0.87 ms |
| Strings | 0.11 ms vs 0.59 ms | 0.15 ms vs 0.68 ms |

## Documentation

<https://docs.rs/serde_toon>

## License

MIT OR Apache-2.0
