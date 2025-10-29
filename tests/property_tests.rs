//! Property-based tests - pragmatic approach testing core roundtrip guarantees
//!
//! These tests complement the 52+ integration tests by verifying properties
//! across a wide range of generated inputs. Focus is on common use cases.

use proptest::prelude::*;
use serde::{Deserialize, Serialize};
use serde_toon::{from_str, to_string};

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
