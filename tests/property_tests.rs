//! Property-based tests - pragmatic approach testing core roundtrip guarantees
//!
//! These tests complement the 52+ integration tests by verifying properties
//! across a wide range of generated inputs. Focus is on common use cases.

use proptest::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use serde_toon::{
    from_str, from_str_with_options, to_string, to_string_with_options, DecodeOptions, Delimiter,
    ToonOptions,
};

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

// ---------------------------------------------------------------------------
// Regressions from tests/property_tests.proptest-regressions
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
struct Flagged {
    id: u32,
    name: String,
    active: bool,
}

/// Decode side of the saved regressions: the spec-conformant TOON for each
/// shrunk value decodes back to it.
#[test]
fn regressions_decode() {
    let flagged = |name: &str| Flagged {
        id: 0,
        name: name.to_string(),
        active: false,
    };
    // `|` needs no quoting with the comma document delimiter (§7.2).
    let got: Flagged = from_str("id: 0\nname: |\nactive: false").unwrap();
    assert_eq!(got, flagged("|"));
    let got: Flagged = from_str("id: 0\nname: \"|\"\nactive: false").unwrap();
    assert_eq!(got, flagged("|"));
    // A key or value starting with `n` is not `null`.
    let got: Flagged = from_str("id: 0\nname: n\nactive: false").unwrap();
    assert_eq!(got, flagged("n"));
    // Controls are escaped as \uXXXX (§7.1).
    let got: String = from_str(r#""\u000b""#).unwrap();
    assert_eq!(got, "\u{b}");
    // Large magnitudes use exponent notation (§2).
    let got: f64 = from_str("-9.742640468003355e+303").unwrap();
    assert_eq!(got, -9.742640468003355e303);
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

    /// Everything the encoder emits is valid under strict decoding and
    /// decodes back to the same JSON value (numbers compared by value;
    /// NaN/infinities are normalized to null by the encoder).
    #[test]
    fn prop_json_round_trips_through_strict_decoder(value in arb_json()) {
        for delimiter in [Delimiter::Comma, Delimiter::Tab, Delimiter::Pipe] {
            let options = ToonOptions::new().with_delimiter(delimiter);
            let text = to_string_with_options(&value, options)
                .map_err(|e| TestCaseError::fail(e.to_string()))?;
            let back: Json = from_str_with_options(&text, DecodeOptions::strict())
                .map_err(|e| TestCaseError::fail(format!("{e}\n--- input ---\n{text}")))?;
            prop_assert!(json_eq(&normalize(&value), &back), "{:?}\n--- toon ---\n{}\n--- back ---\n{:?}", value, text, back);
        }
    }

    fn prop_f64(n in any::<f64>().prop_filter("finite", |f| f.is_finite())) {
        prop_assert!(roundtrip(&n));
    }

    #[test]
    fn prop_string(s in any::<String>()) {
        prop_assert!(roundtrip(&s));
    }

    #[test]
    fn prop_struct(id in any::<u32>(), name in any::<String>(), active in any::<bool>()) {
        let value = Flagged { id, name, active };
        prop_assert!(roundtrip(&value));
    }
}

/// The encoder's §3 normalization: non-finite floats become null.
fn normalize(v: &Json) -> Json {
    match v {
        Json::Number(n) if n.as_f64().is_some_and(|f| !f.is_finite()) => Json::Null,
        Json::Array(items) => Json::Array(items.iter().map(normalize).collect()),
        Json::Object(map) => {
            Json::Object(map.iter().map(|(k, v)| (k.clone(), normalize(v))).collect())
        }
        other => other.clone(),
    }
}

/// JSON-model equality (spec §2): numbers by mathematical value. Objects are
/// compared as key sets, since tabular and keyed tabular forms legitimately
/// reorder keys to the header's field order.
fn json_eq(a: &Json, b: &Json) -> bool {
    match (a, b) {
        (Json::Number(x), Json::Number(y)) => {
            match (x.as_i64(), y.as_i64(), x.as_u64(), y.as_u64()) {
                (Some(p), Some(q), _, _) => p == q,
                (_, _, Some(p), Some(q)) => p == q,
                _ => x.as_f64() == y.as_f64(),
            }
        }
        (Json::Array(x), Json::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(p, q)| json_eq(p, q))
        }
        (Json::Object(x), Json::Object(y)) => {
            x.len() == y.len()
                && x.iter()
                    .all(|(k, vx)| y.get(k).is_some_and(|vy| json_eq(vx, vy)))
        }
        _ => a == b,
    }
}
