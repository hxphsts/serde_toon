use serde_toon::{toon, Number, ToonMap, Value};

#[test]
fn test_toon_macro_null() {
    let value = toon!(null);
    assert_eq!(value, Value::Null);
}

#[test]
fn test_toon_macro_booleans() {
    let true_val = toon!(true);
    assert_eq!(true_val, Value::Bool(true));

    let false_val = toon!(false);
    assert_eq!(false_val, Value::Bool(false));
}

#[test]
fn test_toon_macro_numbers() {
    let int_val = toon!(42);
    assert_eq!(int_val, Value::Number(Number::Integer(42)));

    let float_val = toon!(3.5);
    assert_eq!(float_val, Value::Number(Number::Float(3.5)));

    let negative_val = toon!(-123);
    assert_eq!(negative_val, Value::Number(Number::Integer(-123)));
}

#[test]
fn test_toon_macro_strings() {
    let string_val = toon!("hello world");
    assert_eq!(string_val, Value::String("hello world".to_string()));

    let empty_string = toon!("");
    assert_eq!(empty_string, Value::String("".to_string()));
}

#[test]
fn test_toon_macro_arrays() {
    let empty_array = toon!([]);
    assert_eq!(empty_array, Value::Array(vec![]));

    let number_array = toon!([1, 2, 3]);
    assert_eq!(
        number_array,
        Value::Array(vec![
            Value::Number(Number::Integer(1)),
            Value::Number(Number::Integer(2)),
            Value::Number(Number::Integer(3)),
        ])
    );

    let mixed_array = toon!([1, "hello", true, null]);
    assert_eq!(
        mixed_array,
        Value::Array(vec![
            Value::Number(Number::Integer(1)),
            Value::String("hello".to_string()),
            Value::Bool(true),
            Value::Null,
        ])
    );
}

#[test]
fn test_toon_macro_objects() {
    let empty_object = toon!({});
    assert_eq!(empty_object, Value::Object(ToonMap::new()));

    let simple_object = toon!({
        "name": "Alice",
        "age": 30
    });

    match simple_object {
        Value::Object(ref obj) => {
            assert_eq!(obj.len(), 2);
            assert_eq!(obj.get("name"), Some(&Value::String("Alice".to_string())));
            assert_eq!(obj.get("age"), Some(&Value::Number(Number::Integer(30))));
        }
        _ => panic!("Expected object"),
    }
}

#[test]
fn test_toon_macro_nested() {
    let nested = toon!({
        "user": {
            "id": 123,
            "name": "Bob",
            "active": true
        },
        "tags": ["admin", "developer"],
        "count": 42
    });

    match nested {
        Value::Object(ref obj) => {
            assert_eq!(obj.len(), 3);

            // Check user object
            if let Some(Value::Object(user)) = obj.get("user") {
                assert_eq!(user.get("id"), Some(&Value::Number(Number::Integer(123))));
                assert_eq!(user.get("name"), Some(&Value::String("Bob".to_string())));
                assert_eq!(user.get("active"), Some(&Value::Bool(true)));
            } else {
                panic!("Expected user to be an object");
            }

            // Check tags array
            if let Some(Value::Array(tags)) = obj.get("tags") {
                assert_eq!(tags.len(), 2);
                assert_eq!(tags[0], Value::String("admin".to_string()));
                assert_eq!(tags[1], Value::String("developer".to_string()));
            } else {
                panic!("Expected tags to be an array");
            }

            // Check count
            assert_eq!(obj.get("count"), Some(&Value::Number(Number::Integer(42))));
        }
        _ => panic!("Expected object"),
    }
}

#[test]
fn test_toon_value_methods() {
    let null_val = toon!(null);
    assert!(null_val.is_null());
    assert!(!null_val.is_bool());
    assert!(!null_val.is_number());
    assert!(!null_val.is_string());
    assert!(!null_val.is_array());
    assert!(!null_val.is_object());
    assert!(!null_val.is_table());

    let bool_val = toon!(true);
    assert!(bool_val.is_bool());
    assert_eq!(bool_val.as_bool(), Some(true));

    let str_val = toon!("hello");
    assert!(str_val.is_string());
    assert_eq!(str_val.as_str(), Some("hello"));

    let array_val = toon!([1, 2, 3]);
    assert!(array_val.is_array());
    assert_eq!(array_val.as_array().unwrap().len(), 3);

    let obj_val = toon!({"key": "value"});
    assert!(obj_val.is_object());
    assert_eq!(obj_val.as_object().unwrap().len(), 1);
}

#[test]
fn test_string_quoting_needs() {
    let normal = Value::String("hello".to_string());
    assert!(!normal.needs_quotes());

    let with_comma = Value::String("hello,world".to_string());
    assert!(with_comma.needs_quotes());

    let with_colon = Value::String("key:value".to_string());
    assert!(with_colon.needs_quotes());

    let empty = Value::String("".to_string());
    assert!(empty.needs_quotes());

    let boolean_like = Value::String("true".to_string());
    assert!(boolean_like.needs_quotes());

    let number_like = Value::String("123".to_string());
    assert!(number_like.needs_quotes());
}
