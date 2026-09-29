//! The decoder must never panic, whatever the input: TOON is often produced
//! by LLMs or other untrusted sources. Every mode must return `Ok` or `Err`.

use proptest::prelude::*;
use serde_json::Value as Json;
use serde_toon::{from_slice_with_options, from_str_with_options, DecodeOptions, Value};

fn modes() -> [DecodeOptions; 4] {
    [
        DecodeOptions::strict(),
        DecodeOptions::lenient(),
        DecodeOptions::compatible(),
        DecodeOptions::strict().with_indent_size(4),
    ]
}

/// Strings built from TOON's structural alphabet, so the generator reaches
/// headers, rows, list items, quotes and escapes far more often than
/// uniformly random text would.
fn toonish() -> impl Strategy<Value = String> {
    let token = prop_oneof![
        Just("\n".to_string()),
        Just("  ".to_string()),
        Just("- ".to_string()),
        Just(": ".to_string()),
        Just(":".to_string()),
        Just(",".to_string()),
        Just("|".to_string()),
        Just("\t".to_string()),
        Just("[".to_string()),
        Just("]".to_string()),
        Just("{".to_string()),
        Just("}".to_string()),
        Just("\"".to_string()),
        Just("\\".to_string()),
        Just("\\u00".to_string()),
        Just("#".to_string()),
        Just("[2]".to_string()),
        Just("[#3]".to_string()),
        Just("[2:]".to_string()),
        Just("{a,b{c}}".to_string()),
        Just("null".to_string()),
        Just("-0".to_string()),
        Just("1e400".to_string()),
        Just("99999999999999999999999".to_string()),
        "[a-z]{1,3}",
        "[0-9]{1,3}",
    ];
    prop::collection::vec(token, 0..40).prop_map(|t| t.concat())
}

proptest! {
    #[test]
    fn arbitrary_text_never_panics(s in any::<String>()) {
        for mode in modes() {
            let _ = from_str_with_options::<Json>(&s, mode);
        }
    }

    #[test]
    fn structural_text_never_panics(s in toonish()) {
        for mode in modes() {
            let _ = from_str_with_options::<Json>(&s, mode);
            let _ = from_str_with_options::<Value>(&s, mode);
            let _ = from_str_with_options::<Vec<std::collections::BTreeMap<String, String>>>(&s, mode);
        }
    }

    #[test]
    fn arbitrary_bytes_never_panic(b in prop::collection::vec(any::<u8>(), 0..64)) {
        for mode in modes() {
            let _ = from_slice_with_options::<Json>(&b, mode);
        }
    }
}

#[test]
fn deep_nesting_is_an_error_not_a_stack_overflow() {
    let mut doc = String::new();
    for depth in 0..10_000 {
        doc.push_str(&" ".repeat(depth * 2));
        doc.push_str("a:\n");
    }
    for mode in modes() {
        assert!(from_str_with_options::<Json>(&doc, mode).is_err());
    }

    let list = "- [1]:\n".to_string()
        + &(1..10_000)
            .map(|d| format!("{}- [1]:\n", "  ".repeat(d)))
            .collect::<String>();
    assert!(from_str_with_options::<Json>(&list, DecodeOptions::lenient()).is_err());
}
