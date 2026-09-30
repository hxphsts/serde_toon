//! Decoder behavior beyond the spec fixtures: serde integration (typed
//! hints, borrowing, enums, options), error positions, limits and the
//! serde_toon 0.2 compatibility boundary.

use serde::Deserialize;
use serde_toon::{from_slice, from_str, from_str_with_options, DecodeOptions, Error, Value};
use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};

fn strict<'a, T: Deserialize<'a>>(s: &'a str) -> Result<T, Error> {
    from_str_with_options(s, DecodeOptions::strict())
}

fn lenient<'a, T: Deserialize<'a>>(s: &'a str) -> Result<T, Error> {
    from_str_with_options(s, DecodeOptions::lenient())
}

/// The line number an error reports, if any.
fn line_of(err: &Error) -> Option<usize> {
    match err {
        Error::Syntax { line, .. }
        | Error::TypeMismatch { line, .. }
        | Error::IndentationError { line, .. }
        | Error::InvalidFormat { line, .. }
        | Error::UnexpectedEof { line, .. } => Some(*line),
        _ => None,
    }
}

#[track_caller]
fn assert_error_line<T: std::fmt::Debug>(result: Result<T, Error>, line: usize) {
    match result {
        Ok(v) => panic!("expected an error at line {line}, got {v:?}"),
        Err(e) => assert_eq!(line_of(&e), Some(line), "wrong position in: {e}"),
    }
}

type Map = BTreeMap<String, Value>;

// ---------------------------------------------------------------------------
// Strict-mode errors carry line numbers
// ---------------------------------------------------------------------------

#[test]
fn strict_errors_report_their_line() {
    let cases: &[(&str, usize)] = &[
        ("a: 1\nxs[3]: 1,2", 2),                 // inline count
        ("a: 1\nitems[2]:\n  - x", 2),           // list count
        ("t[2]{a,b}:\n  1,2\n  3", 3),           // row width
        ("t[1]{a}:\n  1\n  2", 1),               // row count
        ("m[2:]{v}:\n  a: 1", 1),                // keyed count
        ("a: 1\nb:\n   c: 2", 3),                // indent multiple
        ("a:\n\tb: 1", 2),                       // tab indent
        ("a:\n  b: 1\n      c: 2", 3),           // over-indented
        ("a:\n      b: 1", 2),                   // depth jump
        ("x: 1\ny: 2\nx: 3", 3),                 // duplicate key
        ("items[3]:\n  - a\n\n  - b\n  - c", 4), // blank in span: line after the gap
        ("t[2]{a}:\n  1\n\n# note\n\n  2", 6),   // gap of blanks and comments
        ("a: 1\n[2]: x,y", 2),                   // keyless header
        ("k[1]:\n  - [2]{x}:\n    1\n    2", 2), // keyless tabular item
        ("items[03]: a", 1),                     // leading zero
        ("foo [2]: a,b", 1),                     // space before bracket
        ("m[2:]:\n  a: 1\n  b: 2", 1),           // keyed w/o fields
        ("t[1]{a,a}:\n  1,2", 1),                // duplicate field
        ("t[1]{a|b}:\n  1|2", 1),                // delimiter mismatch
        ("t[2]{a,b}: 1,2", 1),                   // inline after fields
        ("m[2:]{v}:\n  a: 1\n  5", 3),           // entry w/o colon
        ("a: \"x\\qy\"", 1),                     // bad escape
        ("a: 1\nb: \"x", 2),                     // unterminated
        ("a: 1\nb: \"x\" y", 2),                 // text after quote
        ("[2]: 1,2\nx: 1", 2),                   // trailing content
        ("a:\n  user", 2),                       // missing colon
    ];
    for &(input, line) in cases {
        let result = strict::<Value>(input);
        assert!(result.is_err(), "{input:?} should fail");
        assert_eq!(
            line_of(result.as_ref().unwrap_err()),
            Some(line),
            "{input:?}: {}",
            result.unwrap_err()
        );
    }
}

#[test]
fn lenient_accepts_what_strict_rejects() {
    let v: Map = lenient("xs[3]: 1,2\na:\n   b: 1\nx: 1\nx: 2").unwrap();
    assert_eq!(v["xs"], Value::Array(vec![1.into(), 2.into()]));
    assert_eq!(v["x"], Value::from(2)); // last write wins
    assert!(v["a"].is_object());
}

#[test]
fn scalar_line_outside_root_is_an_error_in_every_mode() {
    assert_error_line(lenient::<Value>("k[1]:\n  - x: 1\n  bare"), 3);
    assert_error_line(from_str::<Value>("hello\nworld"), 1);
}

#[test]
fn zero_indent_size_is_rejected() {
    let r = from_str_with_options::<Value>("a: 1", DecodeOptions::strict().with_indent_size(0));
    assert!(r.is_err());
}

#[test]
fn custom_indent_size() {
    let v: Map = from_str_with_options(
        "a:\n    b:\n        c: 1",
        DecodeOptions::strict().with_indent_size(4),
    )
    .unwrap();
    let b = v["a"].as_object().unwrap().get("b").unwrap();
    assert_eq!(b.as_object().unwrap().get("c"), Some(&Value::from(1)));
}

// ---------------------------------------------------------------------------
// Strings: borrowing, typed hints, unquoted edge cases
// ---------------------------------------------------------------------------

#[derive(Deserialize, Debug, PartialEq)]
struct Borrowed<'a> {
    #[serde(borrow)]
    name: &'a str,
    #[serde(borrow)]
    note: Cow<'a, str>,
    tags: Vec<&'a str>,
}

#[test]
fn zero_copy_strings_borrow_from_the_input() {
    let input = "name: Ada Lovelace\nnote: \"quoted, no escapes\"\ntags[2]: x,\"y z\"";
    let v: Borrowed = strict(input).unwrap();
    assert_eq!(v.name, "Ada Lovelace");
    assert!(matches!(v.note, Cow::Borrowed("quoted, no escapes")));
    assert_eq!(v.tags, ["x", "y z"]);
    // The borrowed slices point into the input.
    let range = input.as_bytes().as_ptr_range();
    assert!(range.contains(&v.name.as_ptr()));

    // An escaped value must be owned: Cow works, &str cannot.
    let v: Borrowed = strict("name: n\nnote: \"a\\nb\"\ntags[0]:").unwrap();
    assert!(matches!(v.note, Cow::Owned(ref s) if s == "a\nb"));
    assert!(strict::<Borrowed>("name: \"a\\tb\"\nnote: x\ntags: []").is_err());
}

#[test]
fn tabular_field_names_and_keys_borrow() {
    #[derive(Deserialize)]
    struct Row<'a> {
        id: u32,
        name: &'a str,
    }
    let map: HashMap<&str, Vec<Row>> = strict("rows[2]{id,name}:\n  1,Ada\n  2,Bob").unwrap();
    let rows = &map["rows"];
    assert_eq!((rows[1].id, rows[1].name), (2, "Bob"));
}

#[derive(Deserialize, Debug, PartialEq)]
struct Person {
    name: String,
    title: String,
    nullable: String,
    flag: bool,
}

#[test]
fn unquoted_strings_starting_with_t_f_n() {
    let p: Person = strict("name: frank\ntitle: nothing\nnullable: nope\nflag: false").unwrap();
    assert_eq!(p.name, "frank");
    assert_eq!(p.title, "nothing");
    assert_eq!(p.nullable, "nope");
    assert!(!p.flag);
    let v: Map = strict("n: nan\nt: truthy\nf: falsey").unwrap();
    assert_eq!(v["n"], Value::String("nan".into()));
    assert_eq!(v["t"], Value::String("truthy".into()));
}

#[derive(Deserialize, Debug, PartialEq)]
struct Record {
    date: String,
    zip: String,
    code: String,
    yes: String,
    version: String,
}

#[test]
fn string_targets_accept_unquoted_numbers_and_booleans() {
    let r: Record =
        strict("date: 2024-01-01\nzip: 01234\ncode: 123\nyes: true\nversion: 1.10").unwrap();
    assert_eq!(r.date, "2024-01-01");
    assert_eq!(r.zip, "01234");
    assert_eq!(r.code, "123");
    assert_eq!(r.yes, "true");
    assert_eq!(r.version, "1.10"); // the text, not a re-formatted float
                                   // deserialize_any still types tokens exactly (§4).
    let v: Map = strict("code: 123\nzip: 01234").unwrap();
    assert_eq!(v["code"], Value::from(123));
    assert_eq!(v["zip"], Value::String("01234".into()));
}

#[test]
fn chars_accept_any_single_scalar() {
    assert_eq!(strict::<char>("é").unwrap(), 'é');
    assert_eq!(
        strict::<Vec<char>>("[3]: a,\"b\",7").unwrap(),
        ['a', 'b', '7']
    );
    assert!(strict::<char>("ab").is_err());
}

#[test]
fn escapes_and_unicode() {
    let s: String = strict(r#""tab\there \u00e9 \"q\" \\ 🚀""#).unwrap();
    assert_eq!(s, "tab\there é \"q\" \\ 🚀");
    assert!(strict::<String>(r#""\ud83d\ude80""#).is_err()); // surrogate escapes
}

// ---------------------------------------------------------------------------
// Numbers
// ---------------------------------------------------------------------------

#[test]
fn exponent_forms() {
    assert_eq!(strict::<f64>("1e5").unwrap(), 100000.0);
    assert_eq!(strict::<u32>("1e5").unwrap(), 100000); // integral float
    assert_eq!(strict::<f32>("-2.5E-1").unwrap(), -0.25);
    assert!(strict::<u32>("1.5").is_err());
    assert_eq!(strict::<Value>("1e5").unwrap(), Value::from(100000.0));
    let neg_zero: f64 = strict("-0").unwrap();
    assert!(neg_zero == 0.0 && neg_zero.is_sign_positive());
}

#[test]
fn integer_overflow_is_an_error_with_position() {
    #[derive(Deserialize, Debug)]
    #[allow(dead_code)]
    struct S {
        a: u8,
        b: i64,
    }
    assert_error_line(strict::<S>("a: 1\nb: 9223372036854775808"), 2);
    assert_error_line(strict::<S>("a: 300\nb: 1"), 1);
    assert_error_line(strict::<S>("a: -1\nb: 1"), 1);
    assert!(matches!(
        strict::<S>("a: 256\nb: 1"),
        Err(Error::TypeMismatch {
            line: 1,
            col: 4,
            ..
        })
    ));
}

#[test]
fn wide_integers() {
    assert_eq!(strict::<u64>("18446744073709551615").unwrap(), u64::MAX);
    assert_eq!(strict::<i64>("-9223372036854775808").unwrap(), i64::MIN);
    assert_eq!(
        strict::<i128>("-170141183460469231731687303715884105728").unwrap(),
        i128::MIN
    );
    assert_eq!(
        strict::<u128>("340282366920938463463374607431768211455").unwrap(),
        u128::MAX
    );
    assert!(strict::<u64>("18446744073709551616").is_err());
}

#[test]
fn value_numbers_are_lossless() {
    use num_bigint::BigInt;
    let v: Value = strict("18446744073709551615").unwrap();
    assert_eq!(v, Value::BigInt(BigInt::from(u64::MAX)));
    // Beyond the u64 range, untyped decoding approximates with f64 so that
    // targets such as serde_json::Value keep working; typed i128/u128
    // targets remain exact (see `wide_integers` above).
    let v: Value = strict("-170141183460469231731687303715884105728").unwrap();
    assert_eq!(v, Value::from(i128::MIN as f64));
    let j: serde_json::Value = strict("340282366920938463463374607431768211455").unwrap();
    assert_eq!(j.as_f64(), Some(u128::MAX as f64));
    let v: Value = strict("42").unwrap();
    assert_eq!(v, Value::from(42));
    // Objects decode to Value::Object, tabular arrays to arrays of objects.
    let v: Value = strict("t[1]{a}:\n  1").unwrap();
    let t = v.as_object().unwrap().get("t").unwrap();
    assert!(t.as_array().unwrap()[0].is_object());
}

#[test]
fn non_finite_tokens_only_in_compatible_mode() {
    assert!(from_str::<f64>("NaN").unwrap().is_nan());
    assert_eq!(from_str::<f64>("-inf").unwrap(), f64::NEG_INFINITY);
    assert_eq!(
        from_str::<Vec<f64>>("[2]: Infinity,1").unwrap()[0],
        f64::INFINITY
    );
    assert!(strict::<f64>("NaN").is_err());
    assert!(lenient::<f64>("Infinity").is_err());
    // As a dynamic value it is a string in every mode (§4).
    assert_eq!(
        from_str::<Value>("NaN").unwrap(),
        Value::String("NaN".into())
    );
}

// ---------------------------------------------------------------------------
// Line handling
// ---------------------------------------------------------------------------

#[test]
fn crlf_bom_and_comments() {
    let input = "\u{feff}# header comment\r\nid: 1\r\n  # indented comment\r\nname: Ada  \r\n";
    let v: Map = strict(input).unwrap();
    assert_eq!(v["id"], Value::from(1));
    assert_eq!(v["name"], Value::String("Ada".into()));
}

#[test]
fn invalid_utf8_reports_position() {
    let err = from_slice::<Value>(b"a: 1\nb: \xff").unwrap_err();
    assert_eq!(line_of(&err), Some(2));
}

#[test]
fn trailing_content_after_root_is_rejected_in_every_mode() {
    assert!(from_str::<Vec<i32>>("[2]: 1,2\nx: 1").is_err());
    assert!(from_str::<Vec<i32>>("[]\nx").is_err());
    assert!(from_str::<i32>("1\n2").is_err());
}

// ---------------------------------------------------------------------------
// Options, enums and structure
// ---------------------------------------------------------------------------

#[derive(Deserialize, Debug, PartialEq)]
struct WithOptions {
    a: Option<i32>,
    b: Option<String>,
    c: Option<Vec<i32>>,
    d: Option<Inner>,
}

#[derive(Deserialize, Debug, PartialEq, Default)]
struct Inner {
    x: Option<i32>,
}

#[test]
fn option_fields() {
    let v: WithOptions = strict("a: 1\nb: null\nd:\n  x: 5").unwrap();
    assert_eq!(
        v,
        WithOptions {
            a: Some(1),
            b: None,
            c: None,
            d: Some(Inner { x: Some(5) }),
        }
    );
    let v: WithOptions = strict("a: null\nb: \"null\"\nc: []\nd:").unwrap();
    assert_eq!(v.b.as_deref(), Some("null"));
    assert_eq!(v.c, Some(vec![]));
    assert_eq!(v.d, Some(Inner::default()));
    let v: Vec<Option<i32>> = strict("[3]: 1,null,3").unwrap();
    assert_eq!(v, [Some(1), None, Some(3)]);
}

#[derive(Deserialize, Debug, PartialEq)]
enum Shape {
    Empty,
    Circle(f64),
    Pair(i32, i32),
    Rect { w: f64, h: f64 },
}

#[derive(Deserialize, Debug, PartialEq)]
struct Drawing {
    name: String,
    main: Shape,
    shapes: Vec<Shape>,
}

#[test]
fn nested_enums() {
    let input = "\
name: d
main:
  Rect:
    w: 2
    h: 3.5
shapes[4]:
  - Empty
  - Circle: 1.5
  - Pair[2]: 1,2
  - Rect:
      w: 1
      h: 1";
    let d: Drawing = strict(input).unwrap();
    assert_eq!(d.main, Shape::Rect { w: 2.0, h: 3.5 });
    assert_eq!(
        d.shapes,
        [
            Shape::Empty,
            Shape::Circle(1.5),
            Shape::Pair(1, 2),
            Shape::Rect { w: 1.0, h: 1.0 }
        ]
    );
    assert_eq!(strict::<Shape>("Circle: 2").unwrap(), Shape::Circle(2.0));
    assert_eq!(strict::<Shape>("Empty").unwrap(), Shape::Empty);
    // More than one key is not an enum.
    assert!(strict::<Shape>("Circle: 1\nEmpty: null").is_err());
}

#[test]
fn unit_enums_in_rows_and_inline_arrays() {
    #[derive(Deserialize, Debug, PartialEq)]
    enum Color {
        Red,
        Green,
    }
    #[derive(Deserialize, Debug, PartialEq)]
    struct Px {
        c: Color,
        n: u8,
    }
    assert_eq!(
        strict::<Vec<Color>>("[2]: Red,Green").unwrap(),
        [Color::Red, Color::Green]
    );
    let px: Vec<Px> = strict("[1]{c,n}:\n  Green,3").unwrap();
    assert_eq!(
        px,
        [Px {
            c: Color::Green,
            n: 3
        }]
    );
}

#[derive(Deserialize, Debug, PartialEq)]
struct Customer {
    name: String,
    country: String,
}

#[derive(Deserialize, Debug, PartialEq)]
struct Order {
    id: u32,
    customer: Customer,
    total: f64,
}

#[test]
fn tabular_with_nested_groups() {
    let orders: Vec<Order> =
        strict("[2]{id,customer{name,country},total}:\n  1,Ada,DK,99\n  2,Bob,UK,149.5").unwrap();
    assert_eq!(orders[1].customer.country, "UK");
    assert_eq!(orders[1].total, 149.5);
}

#[derive(Deserialize, Debug, PartialEq)]
struct Server {
    host: String,
    port: u16,
}

#[test]
fn keyed_tabular_into_maps() {
    let input = "servers[2:|]{host|port}:\n  alpha: a.example.com|8080\n  \"beta-1\": b|9090";
    let h: HashMap<String, HashMap<String, Server>> = strict(input).unwrap();
    assert_eq!(h["servers"]["beta-1"].port, 9090);
    let b: BTreeMap<String, BTreeMap<String, Server>> = strict(input).unwrap();
    assert_eq!(b["servers"].keys().collect::<Vec<_>>(), ["alpha", "beta-1"]);
    // Root keyed object.
    let root: BTreeMap<String, Server> = strict("[1:]{host,port}:\n  a: h,1").unwrap();
    assert_eq!(
        root["a"],
        Server {
            host: "h".into(),
            port: 1
        }
    );
}

#[test]
fn list_item_object_with_tabular_first_field() {
    #[derive(Deserialize, Debug, PartialEq)]
    struct Group {
        users: Vec<Customer>,
        status: String,
    }
    let input = "\
groups[2]:
  - users[2]{name,country}:
      Ada,DK
      Bob,UK
    status: active
  - users: []
    status: empty";
    let g: HashMap<String, Vec<Group>> = strict(input).unwrap();
    let g = &g["groups"];
    assert_eq!(g[0].users.len(), 2);
    assert_eq!(g[0].status, "active");
    assert_eq!(g[1].status, "empty");
}

#[test]
fn map_keys_of_other_types() {
    let m: HashMap<u32, bool> = strict("1: true\n\"2\": false").unwrap();
    assert!(m[&1] && !m[&2]);
}

#[test]
fn unit_and_empty_structs_from_empty_document() {
    #[derive(Deserialize, Debug, PartialEq)]
    struct Unit;
    #[derive(Deserialize, Debug, PartialEq)]
    struct Empty {}
    assert_eq!(strict::<Unit>("").unwrap(), Unit);
    assert_eq!(strict::<Empty>("# only a comment").unwrap(), Empty {});
    assert!(strict::<BTreeMap<String, i32>>("").unwrap().is_empty());
    assert!(strict::<Vec<i32>>("").is_err()); // an empty document is `{}`
}

#[test]
fn tuples_check_their_length() {
    assert_eq!(
        strict::<(i32, String)>("[2]: 1,one").unwrap(),
        (1, "one".into())
    );
    assert!(strict::<(i32, i32)>("[3]: 1,2,3").is_err());
    assert!(strict::<(i32, i32, i32)>("[2]: 1,2").is_err());
}

#[test]
fn visitor_errors_carry_positions() {
    #[derive(Deserialize, Debug)]
    #[allow(dead_code)]
    struct S {
        a: Vec<i32>,
    }
    // A string where a sequence is expected.
    assert_error_line(strict::<S>("\na: nope"), 2);
}

// ---------------------------------------------------------------------------
// Limits
// ---------------------------------------------------------------------------

fn nested(levels: usize) -> String {
    let mut s = String::new();
    for i in 0..levels {
        s.push_str(&"  ".repeat(i));
        s.push_str("a:\n");
    }
    s.push_str(&"  ".repeat(levels));
    s.push_str("b: 1");
    s
}

#[test]
fn deep_nesting_is_limited_not_a_stack_overflow() {
    assert!(strict::<Value>(&nested(100)).is_ok());
    let err = strict::<Value>(&nested(1000)).unwrap_err();
    assert!(err.to_string().contains("maximum depth"), "{err}");
    // Deeply nested field groups in a header are limited too.
    let header = format!("t[1]{}:\n  1", "{a".repeat(500) + &"}".repeat(500));
    assert!(strict::<Value>(&header).is_err());
}

#[test]
fn declared_lengths_do_not_preallocate() {
    // A huge declared N with few actual items must not allocate for N.
    let v: Vec<i32> = lenient("[18446744073709551615]: 1,2").unwrap();
    assert_eq!(v, [1, 2]);
}

// ---------------------------------------------------------------------------
// serde_toon 0.2 syntax
// ---------------------------------------------------------------------------

#[test]
fn legacy_syntax_is_confined_to_compatible_mode() {
    for input in ["[#2]: 1,2", "xs: [2]: 1,2", "[2    ]: 1\t2"] {
        assert!(from_str::<Value>(input).is_ok(), "compatible: {input}");
    }
    assert!(strict::<Vec<i32>>("[#2]: 1,2").is_err());
    let err = lenient::<Vec<i32>>("[#2]: 1,2").unwrap_err();
    assert!(err.to_string().contains("compatible"), "{err}");
    // Spec modes read `key: [2]: …` as a string value.
    let v: Map = strict("xs: [2]: 1,2").unwrap();
    assert_eq!(v["xs"], Value::String("[2]: 1,2".into()));
}

#[test]
fn debug_output() {
    let de = serde_toon::Deserializer::from_str("a: 1\nb: 2");
    let s = format!("{de:?}");
    assert!(
        s.starts_with("Deserializer") && s.contains("lines: 2"),
        "{s}"
    );
}

// ---------------------------------------------------------------------------
// Structure edge cases
// ---------------------------------------------------------------------------

#[test]
fn list_item_object_scopes() {
    let input = "\
items[2]:
  - a: 1
    b:
      c: 2
    d[2]: x,y
  - matrix[1]:
      - [2]: 1,2
    e: end";
    let v: Map = strict(input).unwrap();
    let json = serde_json::to_string(&v).unwrap();
    assert_eq!(
        json,
        r#"{"items":[{"a":1,"b":{"c":2},"d":["x","y"]},{"matrix":[[1,2]],"e":"end"}]}"#
    );
}

#[test]
fn non_strict_adopts_a_deeper_block_indentation() {
    // Four-space indentation decoded with the default indent size of 2.
    let v: Map = from_str("a:\n    b: 1\n    c:\n        d: 2\ne: 3").unwrap();
    let json = serde_json::to_string(&v).unwrap();
    assert_eq!(json, r#"{"a":{"b":1,"c":{"d":2}},"e":3}"#);
    // Strict mode reports the depth jump.
    assert_error_line(strict::<Map>("a:\n    b: 1"), 2);
}

#[test]
fn rows_end_at_a_key_value_line() {
    // §9.3: at row depth, colon-before-delimiter ends the rows; in a
    // list-item object the line is the next field.
    let input = "\
items[1]:
  - t[2]{a,b}:
      1,2
      3,4
    n: 1";
    let v: Map = strict(input).unwrap();
    let json = serde_json::to_string(&v).unwrap();
    assert_eq!(
        json,
        r#"{"items":[{"t":[{"a":1,"b":2},{"a":3,"b":4}],"n":1}]}"#
    );
}

#[test]
fn ignored_fields_are_skipped_whole() {
    #[derive(Deserialize, Debug, PartialEq)]
    struct Only {
        keep: u8,
    }
    let input = "\
skip:
  deep[2]{a,b{c,d}}:
    1,2,3
    4,5,6
  list[1]:
    - x: 1
keep: 7
rows[1]{a}:
  1";
    assert_eq!(strict::<Only>(input).unwrap(), Only { keep: 7 });
    // Ignored nested groups still consume their cells.
    #[derive(Deserialize, Debug, PartialEq)]
    struct Row {
        z: u8,
    }
    let rows: Vec<Row> = strict("[1]{a,b{c,d},z}:\n  1,2,3,9").unwrap();
    assert_eq!(rows, [Row { z: 9 }]);
}
