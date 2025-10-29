#[macro_export]
macro_rules! toon {
    // Handle null
    (null) => {
        $crate::ToonValue::Null
    };

    // Handle true
    (true) => {
        $crate::ToonValue::Bool(true)
    };

    // Handle false
    (false) => {
        $crate::ToonValue::Bool(false)
    };

    // Handle empty array
    ([]) => {
        $crate::ToonValue::Array(vec![])
    };

    // Handle non-empty array
    ([ $($elem:tt),* $(,)? ]) => {
        $crate::ToonValue::Array(vec![$($crate::toon!($elem)),*])
    };

    // Handle empty object
    ({}) => {
        $crate::ToonValue::Object($crate::ToonMap::new())
    };

    // Handle non-empty object
    ({ $($key:literal : $value:tt),* $(,)? }) => {{
        let mut object = $crate::ToonMap::new();
        $(
            object.insert($key.to_string(), $crate::toon!($value));
        )*
        $crate::ToonValue::Object(object)
    }};

    // Handle different literal types explicitly

    // String literals (quoted)
    ($s:expr) => {{
        // This is a fallback for any expression
        $crate::to_value(&$s).unwrap_or($crate::ToonValue::Null)
    }};
}

#[cfg(test)]
mod tests {
    use crate::{Number, ToonMap, ToonValue};

    #[test]
    fn test_toon_macro_primitives() {
        assert_eq!(toon!(null), ToonValue::Null);
        assert_eq!(toon!(true), ToonValue::Bool(true));
        assert_eq!(toon!(false), ToonValue::Bool(false));
        assert_eq!(toon!(42), ToonValue::Number(Number::Integer(42)));
        assert_eq!(toon!(3.5), ToonValue::Number(Number::Float(3.5)));
        assert_eq!(toon!("hello"), ToonValue::String("hello".to_string()));
    }

    #[test]
    fn test_toon_macro_arrays() {
        assert_eq!(toon!([]), ToonValue::Array(vec![]));

        let arr = toon!([1, 2, 3]);
        match arr {
            ToonValue::Array(vec) => {
                assert_eq!(vec.len(), 3);
                assert_eq!(vec[0], ToonValue::Number(Number::Integer(1)));
                assert_eq!(vec[1], ToonValue::Number(Number::Integer(2)));
                assert_eq!(vec[2], ToonValue::Number(Number::Integer(3)));
            }
            _ => panic!("Expected array"),
        }
    }

    #[test]
    fn test_toon_macro_objects() {
        assert_eq!(toon!({}), ToonValue::Object(ToonMap::new()));

        let obj = toon!({
            "name": "Alice",
            "age": 30
        });

        match obj {
            ToonValue::Object(map) => {
                assert_eq!(map.len(), 2);
                assert_eq!(
                    map.get("name"),
                    Some(&ToonValue::String("Alice".to_string()))
                );
                assert_eq!(
                    map.get("age"),
                    Some(&ToonValue::Number(Number::Integer(30)))
                );
            }
            _ => panic!("Expected object"),
        }
    }
}
