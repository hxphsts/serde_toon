//! TOON vs JSON size comparison.
//!
//! Character counts are a rough proxy for tokens; the savings come mostly
//! from the tabular form, which writes each field name once.
//!
//! Run with: cargo run --example token_efficiency

use serde::{Deserialize, Serialize};
use serde_toon::to_string;
use std::error::Error;

#[derive(Debug, Serialize, Deserialize)]
struct User {
    id: u32,
    name: String,
    email: String,
    active: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct ApiResponse {
    users: Vec<User>,
    total: u32,
    page: u32,
}

fn main() -> Result<(), Box<dyn Error>> {
    let response = ApiResponse {
        users: vec![
            User {
                id: 1,
                name: "Alice Johnson".into(),
                email: "alice@example.com".into(),
                active: true,
            },
            User {
                id: 2,
                name: "Bob Smith".into(),
                email: "bob@example.com".into(),
                active: true,
            },
            User {
                id: 3,
                name: "Charlie Brown".into(),
                email: "charlie@example.com".into(),
                active: false,
            },
        ],
        total: 3,
        page: 1,
    };

    // Serialize to JSON (pretty-printed, as often pasted into prompts)
    let json = serde_json::to_string_pretty(&response)?;
    println!("JSON ({} chars):\n{}\n", json.len(), json);
    let compact = serde_json::to_string(&response)?;
    println!("Compact JSON: {} chars\n", compact.len());

    // Serialize to TOON
    let toon = to_string(&response)?;
    println!("TOON ({} chars):\n{}\n", toon.len(), toon);

    // Compare sizes
    let savings = ((json.len() - toon.len()) as f64 / json.len() as f64) * 100.0;
    println!(
        "✓ TOON is {:.1}% smaller ({} → {} chars)",
        savings,
        json.len(),
        toon.len()
    );
    let vs_compact = ((compact.len() - toon.len()) as f64 / compact.len() as f64) * 100.0;
    println!("  vs compact JSON: {:.1}% smaller", vs_compact);

    Ok(())
}
