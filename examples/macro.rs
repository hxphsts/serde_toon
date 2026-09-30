//! Using the toon! macro for dynamic value construction.
//!
//! Run with: cargo run --example macro

use serde_toon::{to_string, toon, Value};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let null_val = toon!(null);
    let bool_val = toon!(true);
    let number = toon!(42);
    let text = toon!("Hello, TOON!");

    println!("Primitives:");
    println!("  null:   {}", to_string(&null_val)?);
    println!("  bool:   {}", to_string(&bool_val)?);
    println!("  number: {}", to_string(&number)?);
    println!("  text:   {}\n", to_string(&text)?);

    let numbers = toon!([1, 2, 3, 4, 5]);
    let mixed = toon!([1, "two", true, null]);

    println!("Arrays:");
    println!("  Numbers: {}", to_string(&numbers)?);
    println!("  Mixed:   {}\n", to_string(&mixed)?);

    let user = toon!({
        "id": 123,
        "name": "Alice",
        "email": "alice@example.com",
        "active": true
    });

    println!("Objects:");
    println!("{}\n", to_string(&user)?);

    let config = toon!({
        "app": {
            "name": "MyApp",
            "version": "1.0.0"
        },
        "database": {
            "host": "localhost",
            "port": 5432,
            "name": "mydb"
        },
        "features": ["auth", "logging", "metrics"],
        "debug": true
    });

    println!("Nested structures:");
    println!("{}\n", to_string(&config)?);

    let items = vec![
        toon!({"id": 1, "status": "active"}),
        toon!({"id": 2, "status": "pending"}),
        toon!({"id": 3, "status": "completed"}),
    ];

    let summary = toon!({
        "total": 3,
        "items": items
    });

    println!("Dynamic construction:");
    println!("{}\n", to_string(&summary)?);

    if let Value::Object(obj) = &config {
        if let Some(Value::Object(app)) = obj.get("app") {
            if let Some(name) = app.get("name").and_then(|v| v.as_str()) {
                println!("Accessing values:");
                println!("  App name: {}", name);
            }
        }

        if let Some(Value::Array(features)) = obj.get("features") {
            println!("  Features: {}", features.len());
        }
    }

    Ok(())
}
