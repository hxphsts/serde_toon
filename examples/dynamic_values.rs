//! Working with Value for runtime flexibility.
//!
//! Run with: cargo run --example dynamic_values

use serde::{Deserialize, Serialize};
use serde_toon::{to_string_pretty, to_value, toon, Value};
use std::error::Error;

#[derive(Debug, Serialize, Deserialize)]
struct User {
    id: u32,
    name: String,
    roles: Vec<String>,
}

fn main() -> Result<(), Box<dyn Error>> {
    // Build config dynamically with toon! macro
    let config = toon!({
        "host": "localhost",
        "port": 8080,
        "features": ["auth", "logging", "metrics"],
        "debug": true
    });

    println!("Config as TOON:\n{}\n", to_string_pretty(&config)?);

    // Access values dynamically
    if let Value::Object(obj) = &config {
        if let Some(Value::String(host)) = obj.get("host") {
            println!("Accessing field 'host': {}", host);
        }

        if let Some(port) = obj.get("port").and_then(|v| v.as_i64()) {
            println!("Accessing field 'port': {}", port);
        }

        if let Some(Value::Array(features)) = obj.get("features") {
            println!("Accessing field 'features': {} items\n", features.len());
        }
    }

    // Convert existing struct to Value
    let user = User {
        id: 123,
        name: "Alice".to_string(),
        roles: vec!["admin".to_string(), "developer".to_string()],
    };

    let user_value = to_value(&user)?;
    println!("User as Value:\n{}\n", to_string_pretty(&user_value)?);

    // Runtime type checking
    println!("Type checks:");
    println!("  is_object: {}", user_value.is_object());
    println!("  is_array:  {}", user_value.is_array());
    println!("  is_string: {}", user_value.is_string());

    Ok(())
}
