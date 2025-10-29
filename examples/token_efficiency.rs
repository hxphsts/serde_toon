//! TOON vs JSON token efficiency comparison.
//!
//! Run with: cargo run --example token_efficiency

use serde::{Deserialize, Serialize};
use serde_toon::to_string_pretty;
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
                name: "Alice Johnson".to_string(),
                email: "alice@example.com".to_string(),
                active: true,
            },
            User {
                id: 2,
                name: "Bob Smith".to_string(),
                email: "bob@example.com".to_string(),
                active: true,
            },
            User {
                id: 3,
                name: "Charlie Brown".to_string(),
                email: "charlie@example.com".to_string(),
                active: false,
            },
        ],
        total: 3,
        page: 1,
    };

    // Serialize to JSON
    let json = serde_json::to_string_pretty(&response)?;
    println!("JSON ({} chars):\n{}\n", json.len(), json);

    // Serialize to TOON
    let toon = to_string_pretty(&response)?;
    println!("TOON ({} chars):\n{}\n", toon.len(), toon);

    // Calculate token savings
    let savings = ((json.len() - toon.len()) as f64 / json.len() as f64) * 100.0;
    println!(
        "✓ Token savings: {:.1}% ({} → {} chars)",
        savings,
        json.len(),
        toon.len()
    );

    Ok(())
}
