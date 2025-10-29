# serde_toon

A Serde-compatible [TOON](https://github.com/johannschopplich/toon) serialization library for Rust.

[![Crates.io](https://img.shields.io/crates/v/serde_toon.svg)](https://crates.io/crates/serde_toon) [![Documentation](https://docs.rs/serde_toon/badge.svg)](https://docs.rs/serde_toon) [![License](https://img.shields.io/crates/l/serde_toon.svg)](LICENSE-MIT)

## What is TOON?

TOON (Token-Oriented Object Notation) is a compact format for LLMs, using **30-60% fewer tokens than JSON**.

**Example - Same data, different size:**
```rust
// JSON (191 chars)
[
  {
    "id":1,
    "name":"Alice",
     "email":"alice@example.com",
     "active":true
  },
  {
    "id":2,
    "name":"Bob",
    "email":"bob@example.com",
    "active":true
  }
]

// TOON (83 chars - 56% smaller)
[2]{active,email,id,name}:
  true,alice@example.com,1,Alice
  true,bob@example.com,2,Bob
```

*See [`examples/token_efficiency.rs`](examples/token_efficiency.rs)*

## Quick Start

```toml
[dependencies]
serde_toon = "0.1"
serde = { version = "1.0", features = ["derive"] }
```

```rust
use serde::{Deserialize, Serialize};
use serde_toon::{to_string, from_str};

#[derive(Serialize, Deserialize)]
struct User { id: u32, name: String }

let user = User { id: 123, name: "Alice".into() };
let toon = to_string(&user)?;
let back: User = from_str(&toon)?;
```

## Features

- Full Serde integration
- Zero-copy deserialization
- Comprehensive error messages
- Multiple array formats
- No unsafe code

## Documentation

See https://docs.rs/serde_toon

## License

MIT OR Apache-2.0
