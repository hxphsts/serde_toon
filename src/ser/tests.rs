use crate::{to_string, to_string_with_options, to_value, Delimiter, ToonOptions, Value};
use serde::Serialize;
use serde_json::json;
use std::collections::BTreeMap;

fn enc<T: ?Sized + Serialize>(value: &T) -> String {
    to_string(value).unwrap()
}

fn enc_with<T: ?Sized + Serialize>(value: &T, options: ToonOptions) -> String {
    to_string_with_options(value, options).unwrap()
}

#[derive(Serialize)]
struct X<T> {
    x: T,
}

#[test]
fn float_edge_cases() {
    assert_eq!(enc(&X { x: 0.1 }), "x: 0.1");
    assert_eq!(enc(&X { x: 1.0 }), "x: 1");
    assert_eq!(enc(&X { x: 1e21 }), "x: 1e+21");
    assert_eq!(enc(&X { x: 1e-7 }), "x: 1e-7");
    assert_eq!(enc(&X { x: 1e-6 }), "x: 0.000001");
    assert_eq!(enc(&X { x: 123456789.125 }), "x: 123456789.125");
    assert_eq!(enc(&X { x: -0.0 }), "x: 0");
    assert_eq!(enc(&f64::MAX), "1.7976931348623157e+308");
    assert_eq!(enc(&f64::MIN_POSITIVE), "2.2250738585072014e-308");
    assert_eq!(enc(&[f64::NAN, f64::INFINITY, 2.5]), "[3]: null,null,2.5");
}

#[test]
fn f32_prints_shortest_f32_digits() {
    assert_eq!(enc(&0.1f32), "0.1");
    assert_eq!(enc(&X { x: 3.3f32 }), "x: 3.3");
    assert_eq!(enc(&vec![0.1f32, 1e-6, 1e-7]), "[3]: 0.1,0.000001,1e-7");
    assert_eq!(enc(&f32::MAX), "3.4028235e+38");
    assert_eq!(enc(&X { x: f32::NAN }), "x: null");
}

#[test]
fn wide_integers_are_exact() {
    assert_eq!(enc(&u64::MAX), "18446744073709551615");
    assert_eq!(enc(&i128::MIN), "-170141183460469231731687303715884105728");
    assert_eq!(
        enc(&X { x: u128::MAX }),
        "x: 340282366920938463463374607431768211455"
    );
    assert_eq!(enc(&vec![-1i128, 2]), "[2]: -1,2");
    assert_eq!(enc(&X { x: 5u128 }), "x: 5");
}

#[test]
fn keys_are_quoted_per_7_3() {
    let v = json!({"a.b": 1, "_x": 2, "my-key": 3, "1a": 4, "": 5, "a b": 6, "é": 7, "q\"k": 8});
    assert_eq!(
        enc(&v),
        "a.b: 1\n_x: 2\n\"my-key\": 3\n\"1a\": 4\n\"\": 5\n\"a b\": 6\n\"é\": 7\n\"q\\\"k\": 8"
    );
    assert_eq!(enc(&json!({"my-key": [1, 2]})), "\"my-key\"[2]: 1,2");
    assert_eq!(
        enc(&json!({"x-items": [{"id": 1}, {"id": 2}]})),
        "\"x-items\"[2]{id}:\n  1\n  2"
    );
}

#[test]
fn string_values_are_quoted_per_7_2() {
    let v = json!({"a": "-x", "b": "#tag", "c": "05", "d": "a b", "e": "[x]", "f": "x|y"});
    assert_eq!(
        enc(&v),
        "a: \"-x\"\nb: \"#tag\"\nc: \"05\"\nd: a b\ne: \"[x]\"\nf: x|y"
    );
    assert_eq!(enc(&'c'), "c");
    assert_eq!(enc(&'1'), "\"1\"");
    assert_eq!(enc(""), "\"\"");
}

#[test]
fn tab_and_pipe_headers() {
    let v = json!({"rows": [{"a": 1, "b": "x,y"}, {"a": 2, "b": "p|q"}], "tags": ["a", "b c"]});
    assert_eq!(
        enc_with(&v, ToonOptions::new().with_delimiter(Delimiter::Tab)),
        "rows[2\t]{a\tb}:\n  1\tx,y\n  2\tp|q\ntags[2\t]: a\tb c"
    );
    assert_eq!(
        enc_with(&v, ToonOptions::new().with_delimiter(Delimiter::Pipe)),
        "rows[2|]{a|b}:\n  1|x,y\n  2|\"p|q\"\ntags[2|]: a|b c"
    );
    // Object field values use the document delimiter.
    assert_eq!(
        enc_with(
            &json!({"k": "a|b"}),
            ToonOptions::new().with_delimiter(Delimiter::Pipe)
        ),
        "k: \"a|b\""
    );
    assert_eq!(
        enc_with(
            &json!({"k": "a\tb"}),
            ToonOptions::new().with_delimiter(Delimiter::Tab)
        ),
        "k: \"a\\tb\""
    );
}

#[derive(Serialize)]
struct Customer {
    name: &'static str,
    country: &'static str,
}

#[derive(Serialize)]
struct Order {
    id: u32,
    customer: Customer,
    total: u32,
}

#[test]
fn nested_field_groups() {
    let orders = vec![
        Order {
            id: 1,
            customer: Customer {
                name: "Ada",
                country: "DK",
            },
            total: 99,
        },
        Order {
            id: 2,
            customer: Customer {
                name: "Bob",
                country: "UK",
            },
            total: 149,
        },
    ];
    assert_eq!(
        enc(&X { x: &orders }),
        "x[2]{id,customer{name,country},total}:\n  1,Ada,DK,99\n  2,Bob,UK,149"
    );
    // Nested group rows with a different key order are placed by name.
    let v = json!([
        {"id": 1, "c": {"a": 1, "b": 2}},
        {"c": {"b": 4, "a": 3}, "id": 2}
    ]);
    assert_eq!(enc(&v), "[2]{id,c{a,b}}:\n  1,1,2\n  2,3,4");
}

#[test]
fn tabular_exclusions() {
    // Null mixed with objects in a column.
    assert_eq!(
        enc(&json!([{"c": {"a": 1}}, {"c": null}])),
        "[2]:\n  - c:\n      a: 1\n  - c: null"
    );
    // Array-valued column.
    assert_eq!(
        enc(&json!([{"a": [1]}, {"a": [2]}])),
        "[2]:\n  - a[1]: 1\n  - a[1]: 2"
    );
    // Empty-object element.
    assert_eq!(enc(&json!([{"a": 1}, {}])), "[2]:\n  - a: 1\n  -");
    // Different key sets of the same size.
    assert_eq!(
        enc(&json!([{"a": 1, "b": 2}, {"a": 1, "c": 2}])),
        "[2]:\n  - a: 1\n    b: 2\n  - a: 1\n    c: 2"
    );
}

#[test]
fn tabular_detects_permuted_large_key_sets() {
    // More keys than the linear-search threshold, in reversed order.
    let first: serde_json::Map<String, serde_json::Value> =
        (0..20).map(|i| (format!("k{i}"), json!(i))).collect();
    let second: serde_json::Map<String, serde_json::Value> = (0..20)
        .rev()
        .map(|i| (format!("k{i}"), json!(i + 100)))
        .collect();
    let out = enc(&vec![first, second]);
    let mut lines = out.lines();
    let header: Vec<String> = (0..20).map(|i| format!("k{i}")).collect();
    assert_eq!(
        lines.next().unwrap(),
        format!("[2]{{{}}}:", header.join(","))
    );
    let row0: Vec<String> = (0..20).map(|i| i.to_string()).collect();
    let row1: Vec<String> = (0..20).map(|i| (i + 100).to_string()).collect();
    assert_eq!(lines.next().unwrap(), format!("  {}", row0.join(",")));
    assert_eq!(lines.next().unwrap(), format!("  {}", row1.join(",")));
}

/// A hand-written map with a repeated key.
struct Dup;

impl Serialize for Dup {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut m = s.serialize_map(Some(2))?;
        m.serialize_entry("a", &1)?;
        m.serialize_entry("a", &2)?;
        m.end()
    }
}

#[test]
fn duplicate_keys_never_form_a_header() {
    assert_eq!(
        enc(&[Dup, Dup]),
        "[2]:\n  - a: 1\n    a: 2\n  - a: 1\n    a: 2"
    );
}

#[test]
fn keyed_tabular() {
    let mut servers = BTreeMap::new();
    servers.insert(
        "alpha",
        Customer {
            name: "a",
            country: "x",
        },
    );
    servers.insert(
        "beta",
        Customer {
            name: "b",
            country: "y",
        },
    );
    assert_eq!(
        enc(&X { x: &servers }),
        "x[2:]{name,country}:\n  alpha: a,x\n  beta: b,y"
    );
    // Root position omits the key.
    assert_eq!(
        enc(&servers),
        "[2:]{name,country}:\n  alpha: a,x\n  beta: b,y"
    );
    assert_eq!(
        enc_with(&servers, ToonOptions::new().with_delimiter(Delimiter::Pipe)),
        "[2:|]{name|country}:\n  alpha: a|x\n  beta: b|y"
    );
    // A single entry stays nested.
    servers.remove("beta");
    assert_eq!(enc(&servers), "alpha:\n  name: a\n  country: x");
    // Entry keys are quoted per §7.3; integer map keys are stringified.
    let mut ids = BTreeMap::new();
    ids.insert(1, X { x: 1 });
    ids.insert(2, X { x: 2 });
    assert_eq!(enc(&ids), "[2:]{x}:\n  \"1\": 1\n  \"2\": 2");
}

#[test]
fn list_item_first_field_tabular_layout() {
    let v = json!({"items": [
        {"users": [{"id": 1}, {"id": 2}], "status": "ok"},
        {"cfg": {"a": {"x": 1}, "b": {"x": 2}}, "n": 1}
    ]});
    assert_eq!(
        enc(&v),
        "items[2]:\n  - users[2]{id}:\n      1\n      2\n    status: ok\n  - cfg[2:]{x}:\n      a: 1\n      b: 2\n    n: 1"
    );
    // Tabular form is not available directly as a list item.
    assert_eq!(
        enc(&json!([[{"x": 1}, {"x": 2}], []])),
        "[2]:\n  - [2]:\n    - x: 1\n    - x: 2\n  - [0]:"
    );
}

#[test]
fn custom_indent() {
    let v = json!({"a": {"b": [{"x": 1}, {"y": 2}]}});
    assert_eq!(
        enc_with(&v, ToonOptions::new().with_indent(4)),
        "a:\n    b[2]:\n        - x: 1\n        - y: 2"
    );
    // Deeper than the static space buffer.
    let deep = json!({"a": {"b": {"c": 1}}});
    assert_eq!(
        enc_with(&deep, ToonOptions::new().with_indent(40)),
        format!("a:\n{}b:\n{}c: 1", " ".repeat(40), " ".repeat(80))
    );
}

#[test]
fn indent_zero_is_an_error() {
    let options = ToonOptions::new().with_indent(0);
    assert!(to_string_with_options(&1, options.clone()).is_err());
    assert!(to_string_with_options(&X { x: 1 }, options.clone()).is_err());
    let err = to_string_with_options(&vec![1], options).unwrap_err();
    assert!(err.to_string().contains("indent"), "{err}");
}

#[test]
#[allow(deprecated)]
fn length_marker_is_ignored() {
    let options = ToonOptions::new().with_length_marker('#');
    assert_eq!(enc_with(&X { x: [1, 2] }, options), "x[2]: 1,2");
    let mut options = ToonOptions::new();
    options.length_marker = Some('#');
    assert_eq!(enc_with(&[X { x: 1 }], options), "[1]{x}:\n  1");
}

#[test]
fn pretty_has_no_effect() {
    let v = json!({"a": {"b": 1}, "c": [1, 2]});
    assert_eq!(enc_with(&v, ToonOptions::pretty()), enc(&v));
}

#[derive(Serialize)]
enum Shape {
    Unit,
    Circle(f64),
    Pair(i32, i32),
    Rect { w: u32, h: u32 },
}

#[derive(Serialize)]
struct Drawing {
    main: Shape,
    shapes: Vec<Shape>,
}

#[test]
fn enums_are_externally_tagged() {
    assert_eq!(enc(&Shape::Unit), "Unit");
    assert_eq!(enc(&Shape::Circle(1.5)), "Circle: 1.5");
    assert_eq!(enc(&Shape::Pair(1, 2)), "Pair[2]: 1,2");
    assert_eq!(enc(&Shape::Rect { w: 1, h: 2 }), "Rect:\n  w: 1\n  h: 2");
    let d = Drawing {
        main: Shape::Rect { w: 3, h: 4 },
        shapes: vec![Shape::Unit, Shape::Circle(2.0), Shape::Pair(5, 6)],
    };
    assert_eq!(
        enc(&d),
        "main:\n  Rect:\n    w: 3\n    h: 4\nshapes[3]:\n  - Unit\n  - Circle: 2\n  - Pair[2]: 5,6"
    );
    // Uniform struct variants form a table with a nested field group.
    assert_eq!(
        enc(&[Shape::Rect { w: 1, h: 2 }, Shape::Rect { w: 3, h: 4 }]),
        "[2]{Rect{w,h}}:\n  1,2\n  3,4"
    );
}

#[test]
fn serde_mapping() {
    assert_eq!(enc(&()), "null");
    assert_eq!(enc(&None::<i32>), "null");
    assert_eq!(enc(&Some(3)), "3");
    assert_eq!(
        enc(&X {
            x: serde_bytes_like(&[1, 2])
        }),
        "x[2]: 1,2"
    );
    assert_eq!(
        enc(&X {
            x: Vec::<i32>::new()
        }),
        "x: []"
    );
    assert_eq!(enc(&Vec::<i32>::new()), "[]");
    assert_eq!(enc(&BTreeMap::<String, i32>::new()), "");
    assert_eq!(
        enc(&X {
            x: BTreeMap::<String, i32>::new()
        }),
        "x:"
    );
    let mut bools = BTreeMap::new();
    bools.insert(true, 1);
    // `true` matches the unquoted-key pattern (§7.3).
    assert_eq!(enc(&bools), "true: 1");
    let mut chars = BTreeMap::new();
    chars.insert('k', 1);
    assert_eq!(enc(&chars), "k: 1");
    let mut bad = BTreeMap::new();
    bad.insert(vec![1], 1);
    assert!(to_string(&bad).is_err());
}

/// Serializes via `serialize_bytes`.
struct Bytes<'a>(&'a [u8]);

impl Serialize for Bytes<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_bytes(self.0)
    }
}

fn serde_bytes_like(b: &[u8]) -> Bytes<'_> {
    Bytes(b)
}

#[test]
fn value_types() {
    let table = Value::Table {
        headers: vec!["a".into(), "b".into()],
        rows: vec![
            vec![Value::from(1), Value::from("x")],
            vec![Value::from(2), Value::from("y")],
        ],
    };
    assert_eq!(enc(&table), "[2]{a,b}:\n  1,x\n  2,y");
    let date = chrono::DateTime::parse_from_rfc3339("2024-01-02T03:04:05Z")
        .unwrap()
        .with_timezone(&chrono::Utc);
    assert_eq!(enc(&Value::Date(date)), "\"2024-01-02T03:04:05+00:00\"");
    assert_eq!(enc(&Value::Number(crate::Number::NaN)), "null");
    assert_eq!(enc(&Value::Number(crate::Number::Infinity)), "null");
    assert_eq!(enc(&Value::Number(crate::Number::NegativeInfinity)), "null");
    // to_value keeps working and re-encodes identically.
    let v = json!({"a": [{"x": 1}, {"x": 2}], "b": {"c": "d"}});
    assert_eq!(enc(&to_value(&v).unwrap()), enc(&v));
}

#[test]
fn needs_quotes_follows_lexical_rules() {
    for s in ["", "a,b", "-x", "#x", "true", "42", "[x", " a"] {
        assert!(Value::from(s).needs_quotes(), "{s:?}");
    }
    for s in ["hello world", "a|b", "café"] {
        assert!(!Value::from(s).needs_quotes(), "{s:?}");
    }
    assert!(!Value::from(1).needs_quotes());
}

#[test]
fn serializer_reuse_and_debug() {
    let mut ser = super::Serializer::new(ToonOptions::new());
    X { x: "a" }.serialize(&mut ser).unwrap();
    assert!(format!("{ser:?}").contains("Serializer"));
    assert_eq!(ser.into_inner(), "x: a");
    assert_eq!(format!("{:?}", crate::ValueSerializer), "ValueSerializer");
}

#[test]
fn root_objects_stream_once_keyed_form_is_ruled_out() {
    // Buffered object values, then a primitive: flushed in order.
    assert_eq!(
        enc(&json!({"a": {"x": 1}, "b": {"x": 2}, "c": 3})),
        "a:\n  x: 1\nb:\n  x: 2\nc: 3"
    );
    // Streaming from the first field, with later object and array values.
    assert_eq!(
        enc(&json!({"n": 1, "o": {"p": {"x": 1}, "q": {"x": 2}}, "e": {}, "l": [1]})),
        "n: 1\no[2:]{x}:\n  p: 1\n  q: 2\ne:\nl[1]: 1"
    );
    // An empty object value rules out the keyed form too.
    assert_eq!(enc(&json!({"a": {"x": 1}, "b": {}})), "a:\n  x: 1\nb:");
    // Map keys recorded while streaming.
    let mut m = BTreeMap::new();
    m.insert(2, "b");
    m.insert(1, "a");
    assert_eq!(enc(&m), "\"1\": a\n\"2\": b");
}
