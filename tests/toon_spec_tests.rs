use serde::{Deserialize, Serialize};
use serde_toon::{to_string, to_string_with_options, Delimiter, ToonOptions};

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct User {
    id: u32,
    name: String,
    role: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct Product {
    sku: String,
    qty: u32,
    price: f64,
}

#[test]
fn test_tabular_array_format() {
    let users = vec![
        User {
            id: 1,
            name: "Alice".to_string(),
            role: "admin".to_string(),
        },
        User {
            id: 2,
            name: "Bob".to_string(),
            role: "user".to_string(),
        },
    ];

    let toon = to_string(&users).unwrap();
    println!("Tabular format:\n{}", toon);

    // Should be in tabular format: [2]{id,name,role}:
    assert!(toon.contains("[2]{"));
    assert!(toon.contains("id,name,role"));
    assert!(toon.contains("1,Alice,admin"));
    assert!(toon.contains("2,Bob,user"));
}

#[test]
fn test_inline_primitive_array() {
    let tags = vec!["admin", "developer", "ops"];
    let toon = to_string(&tags).unwrap();
    println!("Inline array:\n{}", toon);

    // Should be inline format: [3]: admin,developer,ops
    assert!(toon.starts_with("[3]: "));
    assert!(toon.contains("admin,developer,ops"));
}

#[test]
fn test_tab_delimiter() {
    let products = vec![
        Product {
            sku: "A1".to_string(),
            qty: 2,
            price: 9.99,
        },
        Product {
            sku: "B2".to_string(),
            qty: 1,
            price: 14.5,
        },
    ];

    let options = ToonOptions::new().with_delimiter(Delimiter::Tab);
    let toon = to_string_with_options(&products, options).unwrap();
    println!("Tab-delimited tabular:\n{}", toon);

    // Should show tab delimiter in header
    assert!(toon.contains("[2    ]{"));
    assert!(toon.contains("price    qty    sku"));
}

#[test]
fn test_pipe_delimiter() {
    let products = vec![
        Product {
            sku: "A1".to_string(),
            qty: 2,
            price: 9.99,
        },
        Product {
            sku: "B2".to_string(),
            qty: 1,
            price: 14.5,
        },
    ];

    let options = ToonOptions::new().with_delimiter(Delimiter::Pipe);
    let toon = to_string_with_options(&products, options).unwrap();
    println!("Pipe-delimited tabular:\n{}", toon);

    // Should show pipe delimiter in header
    assert!(toon.contains("[2|]{"));
    assert!(toon.contains("price|qty|sku"));
}

#[test]
fn test_length_marker() {
    let tags = vec!["rust", "serde", "toon"];

    let options = ToonOptions::new().with_length_marker('#');
    let toon = to_string_with_options(&tags, options).unwrap();
    println!("With length marker:\n{}", toon);

    // Should have # prefix in length
    assert!(toon.starts_with("[#3]: "));
}

#[test]
fn test_mixed_array_list_format() {
    use serde_json::json;

    let mixed = json!([
        1,
        {"name": "Alice", "age": 30},
        "text"
    ]);

    let toon = to_string(&mixed).unwrap();
    println!("Mixed array (list format):\n{}", toon);

    // Should use list format with "- " prefix
    assert!(toon.contains("[3]:"));
    assert!(toon.contains("- 1"));
    // Fields are sorted alphabetically, so "age" comes before "name"
    assert!(toon.contains("- age: 30"));
    assert!(toon.contains("name: Alice"));
    assert!(toon.contains("- text"));
}

#[test]
fn test_empty_array() {
    let empty: Vec<String> = vec![];
    let toon = to_string(&empty).unwrap();
    println!("Empty array:\n{}", toon);

    assert_eq!(toon, "[0]:");
}

#[test]
fn test_quoting_rules() {
    use std::collections::HashMap;

    let mut data = HashMap::new();
    data.insert("normal", "hello world".to_string());
    data.insert("with_comma", "hello,world".to_string());
    data.insert("with_spaces", " padded ".to_string());
    data.insert("boolean_like", "true".to_string());
    data.insert("number_like", "123".to_string());
    data.insert("empty", "".to_string());

    let toon = to_string(&data).unwrap();
    println!("Quoting test:\n{}", toon);

    // Check that appropriate strings are quoted
    assert!(toon.contains("\"hello,world\"")); // comma needs quoting
    assert!(toon.contains("\" padded \"")); // spaces need quoting
    assert!(toon.contains("\"true\"")); // boolean-like needs quoting
    assert!(toon.contains("\"123\"")); // number-like needs quoting
    assert!(toon.contains("\"\"")); // empty needs quoting
    assert!(toon.contains("hello world")); // normal doesn't need quoting
}
