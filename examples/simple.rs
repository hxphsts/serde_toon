//! Basic TOON serialization and deserialization.
//!
//! Run with: cargo run --example simple

use serde::{Deserialize, Serialize};
use serde_toon::{from_str, to_string};
use std::error::Error;

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct User {
    id: u32,
    name: String,
    email: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    let users = vec![
        User {
            id: 42,
            name: "Alice Johnson".to_string(),
            email: "alice@example.com".to_string(),
        },
        User {
            id: 43,
            name: "Bob Smith".to_string(),
            email: "bob@example.com".to_string(),
        },
    ];

    // Serialize to TOON
    let toon = to_string(&users)?;
    println!("TOON output:\n{}\n", toon);

    // Deserialize back to struct
    let users_back: Vec<User> = from_str(&toon)?;
    assert_eq!(users, users_back);
    println!("✓ Round-trip successful");

    Ok(())
}
