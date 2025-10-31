//! Tabular array format for homogeneous structs.
//!
//! Run with: cargo run --example tabular_arrays

use serde::{Deserialize, Serialize};
use serde_toon::{from_str, to_string_pretty};
use std::error::Error;

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Product {
    sku: String,
    name: String,
    price: f64,
    in_stock: bool,
}

fn main() -> Result<(), Box<dyn Error>> {
    let products = vec![
        Product {
            sku: "WIDGET-001".into(),
            name: "Super Widget".into(),
            price: 29.99,
            in_stock: true,
        },
        Product {
            sku: "GADGET-002".into(),
            name: "Mega Gadget".into(),
            price: 49.99,
            in_stock: false,
        },
        Product {
            sku: "TOOL-003".into(),
            name: "Ultra Tool".into(),
            price: 19.99,
            in_stock: true,
        },
    ];

    // Serialize to tabular format
    let toon = to_string_pretty(&products)?;
    println!("TOON tabular output:\n{}\n", toon);

    // Deserialize back to verify
    let products_back: Vec<Product> = from_str(&toon)?;
    assert_eq!(products, products_back);
    println!("✓ Round-trip successful");

    Ok(())
}
