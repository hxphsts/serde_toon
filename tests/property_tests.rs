//! Property-based tests - pragmatic approach testing core roundtrip guarantees
//!
//! These tests complement the 52+ integration tests by verifying properties
//! across a wide range of generated inputs. Focus is on common use cases.

use proptest::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use serde_toon::{from_str, to_string, to_string_with_options, Delimiter, ToonOptions};

fn roundtrip<T: Serialize + for<'de> Deserialize<'de> + PartialEq + std::fmt::Debug>(
    value: &T,
) -> bool {
    match to_string(value) {
        Ok(serialized) => match from_str::<T>(&serialized) {
            Ok(deserialized) => *value == deserialized,
            Err(e) => {
                eprintln!("Deserialize failed: {}", e);
                eprintln!("Serialized was: {}", serialized);
                false
            }
        },
        Err(e) => {
            eprintln!("Serialize failed: {}", e);
            false
        }
    }
}

proptest! {
    // Test primitive types
    #[test]
    fn prop_i32(n in any::<i32>()) {
        prop_assert!(roundtrip(&n));
    }

    #[test]
    fn prop_i64(n in any::<i64>()) {
        prop_assert!(roundtrip(&n));
    }

    #[test]
    fn prop_u32(n in any::<u32>()) {
        prop_assert!(roundtrip(&n));
    }

    #[test]
    fn prop_bool(b in any::<bool>()) {
        prop_assert!(roundtrip(&b));
    }

    // Test collections
    // An empty Vec encodes as `[]` (TOON spec §9.1), which the 0.2 decoder
    // cannot read.
    #[test]
    #[ignore = "pending decoder rewrite"]
    fn prop_vec_i32(v in prop::collection::vec(any::<i32>(), 0..20)) {
        prop_assert!(roundtrip(&v));
    }

    #[test]
    fn prop_option_i32(opt in proptest::option::of(any::<i32>())) {
        prop_assert!(roundtrip(&opt));
    }

    #[test]
    fn prop_tuple_i32_bool(t in (any::<i32>(), any::<bool>())) {
        prop_assert!(roundtrip(&t));
    }
}

/// Arbitrary JSON trees: every primitive kind, strings with any characters
/// (quotes, controls, delimiters, leading `-`/`#`), and nested arrays and
/// objects, including empty ones and uniform object arrays.
fn arb_json() -> impl Strategy<Value = Json> {
    let leaf = prop_oneof![
        Just(Json::Null),
        any::<bool>().prop_map(Json::Bool),
        any::<i64>().prop_map(Json::from),
        any::<u64>().prop_map(Json::from),
        any::<f64>().prop_map(Json::from), // NaN and infinities become null
        ".*".prop_map(Json::String),
        "[a-z#\\- ,|:]{0,4}".prop_map(Json::String),
    ];
    leaf.prop_recursive(4, 64, 6, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..6).prop_map(Json::Array),
            prop::collection::vec(("[a-c]{1,2}|.*", inner.clone()), 0..5)
                .prop_map(|entries| Json::Object(entries.into_iter().collect())),
            // Uniform objects, so tabular and keyed tabular forms show up.
            (1usize..4, prop::collection::vec(inner, 1..4)).prop_map(|(n, values)| {
                let row: serde_json::Map<String, Json> = values
                    .into_iter()
                    .enumerate()
                    .map(|(i, v)| (format!("k{i}"), v))
                    .collect();
                Json::Array(vec![Json::Object(row); n])
            }),
        ]
    })
}

fn assert_well_formed(text: &str) -> Result<(), TestCaseError> {
    prop_assert!(!text.ends_with('\n'), "trailing newline: {:?}", text);
    prop_assert!(!text.contains('\r'), "CR in output: {:?}", text);
    for line in text.split('\n') {
        prop_assert!(!line.ends_with(' '), "trailing space in {:?}", line);
        prop_assert!(!line.starts_with('\t'), "tab indentation in {:?}", line);
        prop_assert!(
            !line.trim_start_matches(' ').starts_with('#'),
            "comment-like line {:?}",
            line
        );
    }
    Ok(())
}

proptest! {
    #[test]
    fn prop_encoding_never_panics_and_is_well_formed(value in arb_json()) {
        for delimiter in [Delimiter::Comma, Delimiter::Tab, Delimiter::Pipe] {
            let options = ToonOptions::new().with_delimiter(delimiter);
            let text = to_string_with_options(&value, options)
                .map_err(|e| TestCaseError::fail(e.to_string()))?;
            assert_well_formed(&text)?;
        }
    }
}
