#[macro_export]
macro_rules! toon {
    // Handle null
    (null) => {
        $crate::Value::Null
    };

    // Handle true
    (true) => {
        $crate::Value::Bool(true)
    };

    // Handle false
    (false) => {
        $crate::Value::Bool(false)
    };

    // Handle empty array
    ([]) => {
        $crate::Value::Array(vec![])
    };

    // Handle non-empty array
    ([ $($elem:tt),* $(,)? ]) => {
        $crate::Value::Array(vec![$($crate::toon!($elem)),*])
    };

    // Handle empty object
    ({}) => {
        $crate::Value::Object($crate::ToonMap::new())
    };

    // Handle non-empty object
    ({ $($key:literal : $value:tt),* $(,)? }) => {{
        let mut object = $crate::ToonMap::new();
        $(
            object.insert($key.to_string(), $crate::toon!($value));
        )*
        $crate::Value::Object(object)
    }};

    // Handle different literal types explicitly

    // String literals (quoted)
    ($s:expr) => {{
        // This is a fallback for any expression
        $crate::to_value(&$s).unwrap_or($crate::Value::Null)
    }};
}

#[cfg(test)]
mod tests {
    use crate::{Number, ToonMap, Value};

    #[test]
    fn test_toon_macro_primitives() {
        assert_eq!(toon!(null), Value::Null);
        assert_eq!(toon!(true), Value::Bool(true));
        assert_eq!(toon!(false), Value::Bool(false));
        assert_eq!(toon!(42), Value::Number(Number::Integer(42)));
        assert_eq!(toon!(3.5), Value::Number(Number::Float(3.5)));
        assert_eq!(toon!("hello"), Value::String("hello".to_string()));
    }

    #[test]
    fn test_toon_macro_arrays() {
        assert_eq!(toon!([]), Value::Array(vec![]));

        let arr = toon!([1, 2, 3]);
        match arr {
            Value::Array(vec) => {
                assert_eq!(vec.len(), 3);
                assert_eq!(vec[0], Value::Number(Number::Integer(1)));
                assert_eq!(vec[1], Value::Number(Number::Integer(2)));
                assert_eq!(vec[2], Value::Number(Number::Integer(3)));
            }
            _ => panic!("Expected array"),
        }
    }

    #[test]
    fn test_toon_macro_objects() {
        assert_eq!(toon!({}), Value::Object(ToonMap::new()));

        let obj = toon!({
            "name": "Alice",
            "age": 30
        });

        match obj {
            Value::Object(map) => {
                assert_eq!(map.len(), 2);
                assert_eq!(map.get("name"), Some(&Value::String("Alice".to_string())));
                assert_eq!(map.get("age"), Some(&Value::Number(Number::Integer(30))));
            }
            _ => panic!("Expected object"),
        }
    }
}
