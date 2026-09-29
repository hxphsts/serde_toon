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

    // Tabular format, fields in declaration order (TOON spec §9.3).
    assert_eq!(toon, "[2]{id,name,role}:\n  1,Alice,admin\n  2,Bob,user");
}

#[test]
fn test_inline_primitive_array() {
    let tags = vec!["admin", "developer", "ops"];
    let toon = to_string(&tags).unwrap();
    println!("Inline array:\n{}", toon);

    // Inline format (TOON spec §9.1).
    assert_eq!(toon, "[3]: admin,developer,ops");
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

    // The header declares the tab delimiter with a literal HTAB inside the
    // brackets, and the field list uses it too (TOON spec §6, §11).
    assert_eq!(
        toon,
        "[2\t]{sku\tqty\tprice}:\n  A1\t2\t9.99\n  B2\t1\t14.5"
    );
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
    assert_eq!(toon, "[2|]{sku|qty|price}:\n  A1|2|9.99\n  B2|1|14.5");
}

#[test]
#[allow(deprecated)] // `with_length_marker` is deprecated and ignored
fn test_length_marker() {
    let tags = vec!["rust", "serde", "toon"];

    let options = ToonOptions::new().with_length_marker('#');
    let toon = to_string_with_options(&tags, options).unwrap();
    println!("With length marker:\n{}", toon);

    // TOON v2+ forbids length markers, so the option is ignored.
    assert_eq!(toon, "[3]: rust,serde,toon");
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

    // List format with "- " prefix; object fields keep their encounter
    // order, the first on the hyphen line (TOON spec §9.4, §10).
    assert_eq!(toon, "[3]:\n  - 1\n  - name: Alice\n    age: 30\n  - text");
}

#[test]
fn test_empty_array() {
    let empty: Vec<String> = vec![];
    let toon = to_string(&empty).unwrap();
    println!("Empty array:\n{}", toon);

    // Empty root arrays are written `[]` (TOON spec §9.1).
    assert_eq!(toon, "[]");
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
