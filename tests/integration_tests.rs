use serde::{Deserialize, Serialize};
use serde_toon::{
    from_str, to_string, to_string_pretty, to_value, Delimiter, Number, ToonOptions, Value,
};

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct User {
    id: u32,
    name: String,
    active: bool,
    tags: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct Product {
    sku: String,
    price: f64,
    quantity: u32,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct Order {
    order_id: u32,
    customer: User,
    items: Vec<Product>,
    total: f64,
}

#[test]
fn test_simple_struct() {
    let user = User {
        id: 123,
        name: "Alice".to_string(),
        active: true,
        tags: vec!["admin".to_string(), "developer".to_string()],
    };

    let toon = to_string(&user).unwrap();
    println!("User TOON: {}", toon);

    let user_back: User = from_str(&toon).unwrap();
    assert_eq!(user, user_back);
}

#[test]
fn test_nested_struct() {
    let order = Order {
        order_id: 12345,
        customer: User {
            id: 123,
            name: "Alice".to_string(),
            active: true,
            tags: vec!["vip".to_string()],
        },
        items: vec![
            Product {
                sku: "WIDGET-001".to_string(),
                price: 29.99,
                quantity: 2,
            },
            Product {
                sku: "GADGET-002".to_string(),
                price: 49.99,
                quantity: 1,
            },
        ],
        total: 109.97,
    };

    let toon = to_string_pretty(&order).unwrap();
    println!("Order TOON:\n{}", toon);

    let order_back: Order = from_str(&toon).unwrap();
    assert_eq!(order, order_back);
}

#[test]
fn test_array_of_objects() {
    let products = vec![
        Product {
            sku: "A001".to_string(),
            price: 10.99,
            quantity: 5,
        },
        Product {
            sku: "B002".to_string(),
            price: 15.99,
            quantity: 3,
        },
        Product {
            sku: "C003".to_string(),
            price: 20.99,
            quantity: 1,
        },
    ];

    let toon = to_string_pretty(&products).unwrap();
    println!("Products TOON:\n{}", toon);

    let products_back: Vec<Product> = from_str(&toon).unwrap();
    assert_eq!(products, products_back);
}

#[test]
fn test_primitives() {
    // Test various primitive types
    assert_roundtrip(&42i32);
    assert_roundtrip(&3.5f64);
    assert_roundtrip(&true);
    assert_roundtrip(&false);
    assert_roundtrip(&"hello world".to_string());
    assert_roundtrip(&vec![1, 2, 3, 4, 5]);
}

#[test]
fn test_options() {
    let user = User {
        id: 123,
        name: "Alice".to_string(),
        active: true,
        tags: vec!["admin".to_string(), "developer".to_string()],
    };

    // Test with tab delimiter
    let options = ToonOptions::new().with_delimiter(Delimiter::Tab);
    let toon = serde_toon::to_string_with_options(&user, options).unwrap();
    println!("Tab-delimited TOON: {}", toon);

    let user_back: User = from_str(&toon).unwrap();
    assert_eq!(user, user_back);

    // Test with pipe delimiter
    let options = ToonOptions::new().with_delimiter(Delimiter::Pipe);
    let toon = serde_toon::to_string_with_options(&user, options).unwrap();
    println!("Pipe-delimited TOON: {}", toon);

    let user_back: User = from_str(&toon).unwrap();
    assert_eq!(user, user_back);

    // Test with length marker
    let options = ToonOptions::new().with_length_marker('#');
    let toon = serde_toon::to_string_with_options(&user, options).unwrap();
    println!("Length-marked TOON: {}", toon);

    let user_back: User = from_str(&toon).unwrap();
    assert_eq!(user, user_back);
}

#[test]
fn test_to_value() {
    let user = User {
        id: 123,
        name: "Alice".to_string(),
        active: true,
        tags: vec!["admin".to_string()],
    };

    let value = to_value(&user).unwrap();

    match value {
        Value::Object(obj) => {
            assert_eq!(obj.get("id"), Some(&Value::Number(Number::Integer(123))));
            assert_eq!(obj.get("name"), Some(&Value::String("Alice".to_string())));
            assert_eq!(obj.get("active"), Some(&Value::Bool(true)));

            if let Some(Value::Array(tags)) = obj.get("tags") {
                assert_eq!(tags.len(), 1);
                assert_eq!(tags[0], Value::String("admin".to_string()));
            } else {
                panic!("Expected tags to be an array");
            }
        }
        _ => panic!("Expected object"),
    }
}

#[test]
fn test_empty_collections() {
    let empty_vec: Vec<i32> = vec![];
    assert_roundtrip(&empty_vec);

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct Empty {}

    let empty = Empty {};
    assert_roundtrip(&empty);
}

#[test]
fn test_special_strings() {
    let special_strings = vec![
        "".to_string(),                // empty
        "hello, world".to_string(),    // comma
        "line1\nline2".to_string(),    // newline
        "tab\there".to_string(),       // tab
        "pipe|here".to_string(),       // pipe
        " leading space".to_string(),  // leading space
        "trailing space ".to_string(), // trailing space
        "true".to_string(),            // boolean literal
        "false".to_string(),           // boolean literal
        "null".to_string(),            // null literal
        "123".to_string(),             // number literal
        "3.5".to_string(),             // float literal
        "\"quoted\"".to_string(),      // already quoted
    ];

    for s in special_strings {
        println!("Testing string: {:?}", s);
        assert_roundtrip(&s);
    }
}

#[test]
fn test_numbers() {
    // Test various number types
    assert_roundtrip(&0i8);
    assert_roundtrip(&127i8);
    assert_roundtrip(&-128i8);
    assert_roundtrip(&0i16);
    assert_roundtrip(&32767i16);
    assert_roundtrip(&-32768i16);
    assert_roundtrip(&0i32);
    assert_roundtrip(&2147483647i32);
    assert_roundtrip(&-2147483648i32);
    assert_roundtrip(&0i64);
    assert_roundtrip(&9223372036854775807i64);
    assert_roundtrip(&-9223372036854775808i64);

    assert_roundtrip(&0u8);
    assert_roundtrip(&255u8);
    assert_roundtrip(&0u16);
    assert_roundtrip(&65535u16);
    assert_roundtrip(&0u32);
    assert_roundtrip(&4294967295u32);

    assert_roundtrip(&0.0f32);
    assert_roundtrip(&3.5f32);
    assert_roundtrip(&-2.5f32);
    assert_roundtrip(&0.0f64);
    assert_roundtrip(&4.25f64);
    assert_roundtrip(&-5.75f64);
}

fn assert_roundtrip<T>(original: &T)
where
    T: Serialize + for<'de> Deserialize<'de> + PartialEq + std::fmt::Debug,
{
    let toon = to_string(original).unwrap();
    let deserialized: T = from_str(&toon).unwrap();
    assert_eq!(*original, deserialized);
}
