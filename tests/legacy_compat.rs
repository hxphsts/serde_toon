//! Golden backward-compatibility tests for serde_toon 0.2.0 output.
//!
//! Every TOON string in this file was produced verbatim by the released
//! serde_toon 0.2.0 serializer (commit 66eab33), using `to_string` and
//! `to_string_with_options` with the option set named in each `check` call:
//!
//! | label              | options                                                    |
//! |--------------------|------------------------------------------------------------|
//! | `default`          | `ToonOptions::new()`                                       |
//! | `pretty`           | `ToonOptions::pretty()`                                    |
//! | `tab`              | `.with_delimiter(Delimiter::Tab)`                          |
//! | `pipe`             | `.with_delimiter(Delimiter::Pipe)`                         |
//! | `hash`             | `.with_length_marker('#')`                                 |
//! | `tab_hash`         | `.with_delimiter(Delimiter::Tab).with_length_marker('#')`  |
//! | `pipe_hash`        | `.with_delimiter(Delimiter::Pipe).with_length_marker('#')` |
//! | `pretty_pipe_hash` | `ToonOptions::pretty()` plus pipe and `'#'`                |
//! | `indent4`          | `.with_indent(4)`                                          |
//!
//! A variant is listed only when its text differs from an earlier variant of
//! the same case, and only when serde_toon 0.2.0's own `from_str` decoded it
//! back to the original value. Documents that 0.2.0 wrote but could not read
//! itself carry no compatibility promise and are deliberately absent.
//!
//! The contract: `serde_toon::from_str` (the default,
//! `DecodeOptions::compatible()` mode) must keep decoding every golden below
//! to the same value. If one of these tests fails after a decoder change, the
//! change broke documents that users have already stored. Never edit a golden
//! string to make a test pass.
//!
//! ## What 0.2.0 wrote but could not decode itself (not covered here)
//!
//! Found while generating these goldens; a record of 0.2.0 decoder bugs:
//!
//! - An object whose first key starts with `n`, `t` or `f` (`name`, `title`,
//!   `nan`) was parsed as `null`/`true`/`false` ("Expected null").
//! - `Option<T>` fields and `Vec<Option<T>>` elements holding `Some`
//!   ("invalid type: integer, expected option"); only a root `Option` worked.
//! - Unit enum variants inside sequences or struct fields
//!   (`[3]: Red,Green,Blue`); only a root unit variant worked.
//! - Nested primitive arrays as list items (`- [1,2]`, "Expected ']'").
//! - Tabular arrays with a tab or pipe delimiter (`[2|]{a|b}:`); 0.2.0 also
//!   wrote the tab form's header with four spaces (`[2    ]{a    b}`).
//! - A tabular array field followed by another array field
//!   (`items: [1]{..}:` then `scores: [2]: ..`, "missing field").
//! - Quoted strings inside delimited arrays (`"x,y"`, `"p|q"`), and, with the
//!   tab delimiter, a quoted string value following an unquoted one with `, `.
//! - `u64` values above `i64::MAX`, floats of 1e21 and above (written without
//!   an exponent), and `NaN`, `inf`, `-inf` (written, but never readable).
//! - A root non-ASCII `char` (`'é'`), and the root string `"[x]"` (decoded as
//!   `"[x"`).
#![allow(clippy::approx_constant)]

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_toon::{toon, Value};
use std::collections::{BTreeMap, HashMap};
use std::fmt::Debug;

/// Asserts that `golden` (written by serde_toon 0.2.0 with the options named
/// by `variant`) still decodes with the default `from_str` to `expected`.
#[track_caller]
fn check<T>(expected: &T, variant: &str, golden: &str)
where
    T: DeserializeOwned + PartialEq + Debug,
{
    match serde_toon::from_str::<T>(golden) {
        Ok(got) => assert_eq!(
            &got, expected,
            "0.2.0 golden [{variant}] decodes to a different value:\n---\n{golden}\n---"
        ),
        Err(err) => panic!("0.2.0 golden [{variant}] no longer decodes: {err}\n---\n{golden}\n---"),
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
struct Point {
    x: i32,
    y: i32,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
struct WithOptions {
    a: Option<i32>,
    b: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Copy)]
enum Color {
    Red,
    Green,
    Blue,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
enum Shape {
    Circle(f64),
    Pair(i32, i32),
    Rect { w: f64, h: f64 },
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
struct Wrapper(i32);

// tests/integration_tests.rs
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
struct User {
    id: u32,
    name: String,
    active: bool,
    tags: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
struct Product {
    sku: String,
    price: f64,
    quantity: u32,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
struct Order {
    order_id: u32,
    customer: User,
    items: Vec<Product>,
    total: f64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
struct Empty {}

// examples/custom_options.rs
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
struct DataRow {
    id: u32,
    value: String,
    active: bool,
}

// examples/dynamic_values.rs
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
struct RoleUser {
    id: u32,
    name: String,
    roles: Vec<String>,
}

// examples/simple.rs
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
struct EmailUser {
    id: u32,
    name: String,
    email: String,
}

// examples/tabular_arrays.rs
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
struct StockItem {
    sku: String,
    name: String,
    price: f64,
    in_stock: bool,
}

// examples/token_efficiency.rs
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
struct ActiveUser {
    id: u32,
    name: String,
    email: String,
    active: bool,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
struct ApiResponse {
    users: Vec<ActiveUser>,
    total: u32,
    page: u32,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
struct Labeled {
    label: String,
    origin: Point,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
struct Crew {
    label: String,
    members: Vec<Point>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
struct Note {
    body: String,
    author: String,
}

#[test]
fn root_i8_min() {
    let v: i8 = i8::MIN;
    check(&v, "default", r#"-128"#);
}

#[test]
fn root_i8_max() {
    let v: i8 = i8::MAX;
    check(&v, "default", r#"127"#);
}

#[test]
fn root_i16_min() {
    let v: i16 = i16::MIN;
    check(&v, "default", r#"-32768"#);
}

#[test]
fn root_i16_max() {
    let v: i16 = i16::MAX;
    check(&v, "default", r#"32767"#);
}

#[test]
fn root_i32_min() {
    let v: i32 = i32::MIN;
    check(&v, "default", r#"-2147483648"#);
}

#[test]
fn root_i32_max() {
    let v: i32 = i32::MAX;
    check(&v, "default", r#"2147483647"#);
}

#[test]
fn root_i64_min() {
    let v: i64 = i64::MIN;
    check(&v, "default", r#"-9223372036854775808"#);
}

#[test]
fn root_i64_max() {
    let v: i64 = i64::MAX;
    check(&v, "default", r#"9223372036854775807"#);
}

#[test]
fn root_i64_zero() {
    let v: i64 = 0;
    check(&v, "default", r#"0"#);
}

#[test]
fn root_u8_max() {
    let v: u8 = u8::MAX;
    check(&v, "default", r#"255"#);
}

#[test]
fn root_u16_max() {
    let v: u16 = u16::MAX;
    check(&v, "default", r#"65535"#);
}

#[test]
fn root_u32_max() {
    let v: u32 = u32::MAX;
    check(&v, "default", r#"4294967295"#);
}

#[test]
fn root_u64_zero() {
    let v: u64 = 0;
    check(&v, "default", r#"0"#);
}

#[test]
fn root_u64_i64_max() {
    let v: u64 = i64::MAX as u64;
    check(&v, "default", r#"9223372036854775807"#);
}

#[test]
fn root_f64_half() {
    let v: f64 = 0.5;
    check(&v, "default", r#"0.5"#);
}

#[test]
fn root_f64_neg() {
    let v: f64 = -1.25;
    check(&v, "default", r#"-1.25"#);
}

#[test]
fn root_f64_1e10() {
    let v: f64 = 1e10;
    check(&v, "default", r#"10000000000"#);
}

#[test]
fn root_f64_zero() {
    let v: f64 = 0.0;
    check(&v, "default", r#"0"#);
}

#[test]
fn root_f64_pi() {
    let v: f64 = 3.14159;
    check(&v, "default", r#"3.14159"#);
}

#[test]
fn root_f64_tiny() {
    let v: f64 = 1e-7;
    check(&v, "default", r#"0.0000001"#);
}

#[test]
fn root_f32() {
    let v: f32 = 2.5;
    check(&v, "default", r#"2.5"#);
}

#[test]
fn root_bool_true() {
    let v: bool = true;
    check(&v, "default", r#"true"#);
}

#[test]
fn root_bool_false() {
    let v: bool = false;
    check(&v, "default", r#"false"#);
}

#[test]
fn root_unit() {
    let v: () = ();
    check(&v, "default", r#"null"#);
}

#[test]
fn root_option_some() {
    let v: Option<i32> = Some(7);
    check(&v, "default", r#"7"#);
}

#[test]
fn root_option_none() {
    let v: Option<i32> = None;
    check(&v, "default", r#"null"#);
}

#[test]
fn root_char() {
    let v: char = 'x';
    check(&v, "default", r#"x"#);
}

#[test]
fn root_string_plain() {
    let v: String = "hello".to_string();
    check(&v, "default", r#"hello"#);
}

#[test]
fn root_string_spaces() {
    let v: String = "hello world".to_string();
    check(&v, "default", r#"hello world"#);
}

#[test]
fn root_string_empty() {
    let v: String = String::new();
    check(&v, "default", r#""""#);
}

#[test]
fn root_string_comma() {
    let v: String = "a,b".to_string();
    check(&v, "default", r#""a,b""#);
}

#[test]
fn root_string_colon() {
    let v: String = "key: value".to_string();
    check(&v, "default", r#""key: value""#);
}

#[test]
fn root_string_true() {
    let v: String = "true".to_string();
    check(&v, "default", r#""true""#);
}

#[test]
fn root_string_null() {
    let v: String = "null".to_string();
    check(&v, "default", r#""null""#);
}

#[test]
fn root_string_number() {
    let v: String = "123".to_string();
    check(&v, "default", r#""123""#);
}

#[test]
fn root_string_float() {
    let v: String = "3.5".to_string();
    check(&v, "default", r#""3.5""#);
}

#[test]
fn root_string_newline() {
    let v: String = "line1\nline2".to_string();
    check(&v, "default", r#""line1\nline2""#);
}

#[test]
fn root_string_tab() {
    let v: String = "tab\there".to_string();
    check(&v, "default", r#""tab\there""#);
}

#[test]
fn root_string_quote() {
    let v: String = "say \"hi\"".to_string();
    check(&v, "default", r#""say \"hi\"""#);
}

#[test]
fn root_string_backslash() {
    let v: String = "C:\\path".to_string();
    check(&v, "default", r#""C:\\path""#);
}

#[test]
fn root_string_leading_space() {
    let v: String = " padded".to_string();
    check(&v, "default", r#"" padded""#);
}

#[test]
fn root_string_trailing_space() {
    let v: String = "padded ".to_string();
    check(&v, "default", r#""padded ""#);
}

#[test]
fn root_string_pipe() {
    let v: String = "pipe|here".to_string();
    check(&v, "default", r#""pipe|here""#);
}

#[test]
fn root_string_dash() {
    let v: String = "-dash".to_string();
    check(&v, "default", r#"-dash"#);
}

#[test]
fn root_string_unicode() {
    let v: String = "café ☕".to_string();
    check(&v, "default", r#"café ☕"#);
}

#[test]
fn vec_i32() {
    let v: Vec<i32> = vec![1, 2, 3, 4, 5];
    check(&v, "default", r#"[5]: 1,2,3,4,5"#);
    check(&v, "tab", r#"[5    ]: 1	2	3	4	5"#);
    check(&v, "pipe", r#"[5|]: 1|2|3|4|5"#);
    check(&v, "hash", r#"[#5]: 1,2,3,4,5"#);
    check(&v, "tab_hash", r#"[#5    ]: 1	2	3	4	5"#);
    check(&v, "pipe_hash", r#"[#5|]: 1|2|3|4|5"#);
}

#[test]
fn vec_i32_negative() {
    let v: Vec<i32> = vec![-1, 0, 1];
    check(&v, "default", r#"[3]: -1,0,1"#);
    check(&v, "tab", r#"[3    ]: -1	0	1"#);
    check(&v, "pipe", r#"[3|]: -1|0|1"#);
    check(&v, "hash", r#"[#3]: -1,0,1"#);
    check(&v, "tab_hash", r#"[#3    ]: -1	0	1"#);
    check(&v, "pipe_hash", r#"[#3|]: -1|0|1"#);
}

#[test]
fn vec_f64() {
    let v: Vec<f64> = vec![0.5, -1.25, 1e10];
    check(&v, "default", r#"[3]: 0.5,-1.25,10000000000"#);
    check(&v, "tab", r#"[3    ]: 0.5	-1.25	10000000000"#);
    check(&v, "pipe", r#"[3|]: 0.5|-1.25|10000000000"#);
    check(&v, "hash", r#"[#3]: 0.5,-1.25,10000000000"#);
    check(&v, "tab_hash", r#"[#3    ]: 0.5	-1.25	10000000000"#);
    check(&v, "pipe_hash", r#"[#3|]: 0.5|-1.25|10000000000"#);
}

#[test]
fn vec_bool() {
    let v: Vec<bool> = vec![true, false];
    check(&v, "default", r#"[2]: true,false"#);
    check(&v, "tab", r#"[2    ]: true	false"#);
    check(&v, "pipe", r#"[2|]: true|false"#);
    check(&v, "hash", r#"[#2]: true,false"#);
    check(&v, "tab_hash", r#"[#2    ]: true	false"#);
    check(&v, "pipe_hash", r#"[#2|]: true|false"#);
}

#[test]
fn vec_string() {
    let v: Vec<String> = vec!["a".to_string(), "b c".to_string(), "d".to_string()];
    check(&v, "default", r#"[3]: a,b c,d"#);
    check(&v, "tab", r#"[3    ]: a	b c	d"#);
    check(&v, "pipe", r#"[3|]: a|b c|d"#);
    check(&v, "hash", r#"[#3]: a,b c,d"#);
    check(&v, "tab_hash", r#"[#3    ]: a	b c	d"#);
    check(&v, "pipe_hash", r#"[#3|]: a|b c|d"#);
}

#[test]
fn vec_empty() {
    let v: Vec<i32> = Vec::new();
    check(&v, "default", r#"[0]:"#);
}

#[test]
fn vec_single() {
    let v: Vec<i32> = vec![42];
    check(&v, "default", r#"[1]: 42"#);
    check(&v, "tab", r#"[1    ]: 42"#);
    check(&v, "pipe", r#"[1|]: 42"#);
    check(&v, "hash", r#"[#1]: 42"#);
    check(&v, "tab_hash", r#"[#1    ]: 42"#);
    check(&v, "pipe_hash", r#"[#1|]: 42"#);
}

#[test]
fn tuple_pair() {
    let v: (i32, String) = (1, "one".to_string());
    check(&v, "default", r#"[2]: 1,one"#);
    check(&v, "tab", r#"[2    ]: 1	one"#);
    check(&v, "pipe", r#"[2|]: 1|one"#);
    check(&v, "hash", r#"[#2]: 1,one"#);
    check(&v, "tab_hash", r#"[#2    ]: 1	one"#);
    check(&v, "pipe_hash", r#"[#2|]: 1|one"#);
}

#[test]
fn tuple_triple() {
    let v: (i32, bool, f64) = (1, true, 2.5);
    check(&v, "default", r#"[3]: 1,true,2.5"#);
    check(&v, "tab", r#"[3    ]: 1	true	2.5"#);
    check(&v, "pipe", r#"[3|]: 1|true|2.5"#);
    check(&v, "hash", r#"[#3]: 1,true,2.5"#);
    check(&v, "tab_hash", r#"[#3    ]: 1	true	2.5"#);
    check(&v, "pipe_hash", r#"[#3|]: 1|true|2.5"#);
}

#[test]
fn point() {
    let v: Point = Point { x: 1, y: 2 };
    check(
        &v,
        "default",
        r#"x: 1
y: 2"#,
    );
}

#[test]
fn point_negative() {
    let v: Point = Point { x: -10, y: 0 };
    check(
        &v,
        "default",
        r#"x: -10
y: 0"#,
    );
}

#[test]
fn vec_point() {
    let v: Vec<Point> = vec![
        Point { x: 1, y: 2 },
        Point { x: 3, y: 4 },
        Point { x: 5, y: 6 },
    ];
    check(
        &v,
        "default",
        r#"[3]{x,y}:
  1,2
  3,4
  5,6"#,
    );
    check(
        &v,
        "hash",
        r#"[#3]{x,y}:
  1,2
  3,4
  5,6"#,
    );
    check(
        &v,
        "indent4",
        r#"[3]{x,y}:
    1,2
    3,4
    5,6"#,
    );
}

#[test]
fn with_options_none() {
    let v: WithOptions = WithOptions { a: None, b: None };
    check(
        &v,
        "default",
        r#"a: null
b: null"#,
    );
}

#[test]
fn enum_unit_red() {
    let v: Color = Color::Red;
    check(&v, "default", r#"Red"#);
}

#[test]
fn enum_unit_blue() {
    let v: Color = Color::Blue;
    check(&v, "default", r#"Blue"#);
}

#[test]
fn enum_newtype() {
    let v: Shape = Shape::Circle(1.5);
    check(&v, "default", r#"Circle:1.5"#);
    check(&v, "pretty", r#"Circle: 1.5"#);
}

#[test]
fn enum_tuple() {
    let v: Shape = Shape::Pair(1, 2);
    check(&v, "default", r#"Pair:[2]: 1,2"#);
    check(&v, "pretty", r#"Pair: [2]: 1,2"#);
    check(&v, "tab", r#"Pair:[2    ]: 1	2"#);
    check(&v, "pipe", r#"Pair:[2|]: 1|2"#);
    check(&v, "hash", r#"Pair:[#2]: 1,2"#);
    check(&v, "tab_hash", r#"Pair:[#2    ]: 1	2"#);
    check(&v, "pipe_hash", r#"Pair:[#2|]: 1|2"#);
    check(&v, "pretty_pipe_hash", r#"Pair: [#2|]: 1|2"#);
}

#[test]
fn enum_struct() {
    let v: Shape = Shape::Rect { w: 2.0, h: 3.5 };
    check(
        &v,
        "default",
        r#"Rect:w: 2
h: 3.5"#,
    );
    check(
        &v,
        "pretty",
        r#"Rect:
  w: 2
  h: 3.5"#,
    );
}

#[test]
fn newtype_struct() {
    let v: Wrapper = Wrapper(5);
    check(&v, "default", r#"5"#);
}

#[test]
fn empty_struct() {
    let v: Empty = Empty {};
    check(&v, "default", r#""#);
}

#[test]
fn btreemap() {
    let v: BTreeMap<String, i32> = [("a".to_string(), 1), ("b".to_string(), 2)]
        .into_iter()
        .collect();
    check(
        &v,
        "default",
        r#"a: 1
b: 2"#,
    );
}

#[test]
fn hashmap_single() {
    let v: HashMap<String, String> = [("key".to_string(), "value".to_string())]
        .into_iter()
        .collect();
    check(&v, "default", r#"key: value"#);
}

#[test]
fn btreemap_vec() {
    let v: BTreeMap<String, Vec<i32>> =
        [("xs".to_string(), vec![1, 2]), ("ys".to_string(), vec![])]
            .into_iter()
            .collect();
    check(
        &v,
        "default",
        r#"xs: [2]: 1,2
ys: [0]:"#,
    );
    check(
        &v,
        "tab",
        r#"xs: [2    ]: 1	2
ys: [0]:"#,
    );
    check(
        &v,
        "pipe",
        r#"xs: [2|]: 1|2
ys: [0]:"#,
    );
    check(
        &v,
        "hash",
        r#"xs: [#2]: 1,2
ys: [0]:"#,
    );
    check(
        &v,
        "tab_hash",
        r#"xs: [#2    ]: 1	2
ys: [0]:"#,
    );
    check(
        &v,
        "pipe_hash",
        r#"xs: [#2|]: 1|2
ys: [0]:"#,
    );
}

#[test]
fn btreemap_points() {
    let v: BTreeMap<String, Point> = [("p".to_string(), Point { x: 1, y: 2 })]
        .into_iter()
        .collect();
    check(
        &v,
        "default",
        r#"p:
  x: 1
  y: 2"#,
    );
    check(
        &v,
        "indent4",
        r#"p:
    x: 1
    y: 2"#,
    );
}

#[test]
fn integration_user() {
    let v: User = User {
        id: 123,
        name: "Alice".to_string(),
        active: true,
        tags: vec!["admin".to_string(), "developer".to_string()],
    };
    check(
        &v,
        "default",
        r#"id: 123
name: Alice
active: true
tags: [2]: admin,developer"#,
    );
    check(
        &v,
        "tab",
        r#"id: 123
name: Alice
active: true
tags: [2    ]: admin	developer"#,
    );
    check(
        &v,
        "pipe",
        r#"id: 123
name: Alice
active: true
tags: [2|]: admin|developer"#,
    );
    check(
        &v,
        "hash",
        r#"id: 123
name: Alice
active: true
tags: [#2]: admin,developer"#,
    );
    check(
        &v,
        "tab_hash",
        r#"id: 123
name: Alice
active: true
tags: [#2    ]: admin	developer"#,
    );
    check(
        &v,
        "pipe_hash",
        r#"id: 123
name: Alice
active: true
tags: [#2|]: admin|developer"#,
    );
}

#[test]
fn integration_order() {
    let v: Order = Order {
        order_id: 12345,
        customer: User {
            id: 123,
            name: "Alice".to_string(),
            active: true,
            tags: vec!["vip".to_string()],
        },
        items: vec![
            Product {
                sku: "WIDGET-1".to_string(),
                price: 9.99,
                quantity: 2,
            },
            Product {
                sku: "GADGET-2".to_string(),
                price: 14.5,
                quantity: 1,
            },
        ],
        total: 34.48,
    };
    check(
        &v,
        "default",
        r#"order_id: 12345
customer:
  id: 123
  name: Alice
  active: true
  tags: [1]: vip
items: [2]{price,quantity,sku}:
  9.99,2,WIDGET-1
  14.5,1,GADGET-2
total: 34.48"#,
    );
    check(
        &v,
        "hash",
        r#"order_id: 12345
customer:
  id: 123
  name: Alice
  active: true
  tags: [#1]: vip
items: [#2]{price,quantity,sku}:
  9.99,2,WIDGET-1
  14.5,1,GADGET-2
total: 34.48"#,
    );
    check(
        &v,
        "indent4",
        r#"order_id: 12345
customer:
    id: 123
    name: Alice
    active: true
    tags: [1]: vip
items: [2]{price,quantity,sku}:
    9.99,2,WIDGET-1
    14.5,1,GADGET-2
total: 34.48"#,
    );
}

#[test]
fn integration_products() {
    let v: Vec<Product> = vec![
        Product {
            sku: "A1".to_string(),
            price: 1.5,
            quantity: 10,
        },
        Product {
            sku: "B2".to_string(),
            price: 2.0,
            quantity: 0,
        },
    ];
    check(
        &v,
        "default",
        r#"[2]{price,quantity,sku}:
  1.5,10,A1
  2,0,B2"#,
    );
    check(
        &v,
        "hash",
        r#"[#2]{price,quantity,sku}:
  1.5,10,A1
  2,0,B2"#,
    );
    check(
        &v,
        "indent4",
        r#"[2]{price,quantity,sku}:
    1.5,10,A1
    2,0,B2"#,
    );
}

#[test]
fn example_custom_options() {
    let v: Vec<DataRow> = vec![
        DataRow {
            id: 1,
            value: "alpha".to_string(),
            active: true,
        },
        DataRow {
            id: 2,
            value: "beta gamma".to_string(),
            active: false,
        },
    ];
    check(
        &v,
        "default",
        r#"[2]{active,id,value}:
  true,1,alpha
  false,2,beta gamma"#,
    );
    check(
        &v,
        "hash",
        r#"[#2]{active,id,value}:
  true,1,alpha
  false,2,beta gamma"#,
    );
    check(
        &v,
        "indent4",
        r#"[2]{active,id,value}:
    true,1,alpha
    false,2,beta gamma"#,
    );
}

#[test]
fn example_dynamic_values() {
    let v: RoleUser = RoleUser {
        id: 123,
        name: "Alice".to_string(),
        roles: vec!["admin".to_string(), "developer".to_string()],
    };
    check(
        &v,
        "default",
        r#"id: 123
name: Alice
roles: [2]: admin,developer"#,
    );
    check(
        &v,
        "tab",
        r#"id: 123
name: Alice
roles: [2    ]: admin	developer"#,
    );
    check(
        &v,
        "pipe",
        r#"id: 123
name: Alice
roles: [2|]: admin|developer"#,
    );
    check(
        &v,
        "hash",
        r#"id: 123
name: Alice
roles: [#2]: admin,developer"#,
    );
    check(
        &v,
        "tab_hash",
        r#"id: 123
name: Alice
roles: [#2    ]: admin	developer"#,
    );
    check(
        &v,
        "pipe_hash",
        r#"id: 123
name: Alice
roles: [#2|]: admin|developer"#,
    );
}

#[test]
fn example_simple() {
    let v: Vec<EmailUser> = vec![
        EmailUser {
            id: 1,
            name: "Alice".to_string(),
            email: "alice@example.com".to_string(),
        },
        EmailUser {
            id: 2,
            name: "Bob".to_string(),
            email: "bob@example.com".to_string(),
        },
    ];
    check(
        &v,
        "default",
        r#"[2]{email,id,name}:
  alice@example.com,1,Alice
  bob@example.com,2,Bob"#,
    );
    check(
        &v,
        "hash",
        r#"[#2]{email,id,name}:
  alice@example.com,1,Alice
  bob@example.com,2,Bob"#,
    );
    check(
        &v,
        "indent4",
        r#"[2]{email,id,name}:
    alice@example.com,1,Alice
    bob@example.com,2,Bob"#,
    );
}

#[test]
fn example_tabular_arrays() {
    let v: Vec<StockItem> = vec![
        StockItem {
            sku: "A-100".to_string(),
            name: "Widget".to_string(),
            price: 9.99,
            in_stock: true,
        },
        StockItem {
            sku: "B-200".to_string(),
            name: "Gadget Pro".to_string(),
            price: 24.5,
            in_stock: false,
        },
    ];
    check(
        &v,
        "default",
        r#"[2]{in_stock,name,price,sku}:
  true,Widget,9.99,A-100
  false,Gadget Pro,24.5,B-200"#,
    );
    check(
        &v,
        "hash",
        r#"[#2]{in_stock,name,price,sku}:
  true,Widget,9.99,A-100
  false,Gadget Pro,24.5,B-200"#,
    );
    check(
        &v,
        "indent4",
        r#"[2]{in_stock,name,price,sku}:
    true,Widget,9.99,A-100
    false,Gadget Pro,24.5,B-200"#,
    );
}

#[test]
fn example_token_efficiency() {
    let v: ApiResponse = ApiResponse {
        users: vec![
            ActiveUser {
                id: 1,
                name: "Alice".to_string(),
                email: "alice@example.com".to_string(),
                active: true,
            },
            ActiveUser {
                id: 2,
                name: "Bob".to_string(),
                email: "bob@example.com".to_string(),
                active: false,
            },
        ],
        total: 2,
        page: 1,
    };
    check(
        &v,
        "default",
        r#"users: [2]{active,email,id,name}:
  true,alice@example.com,1,Alice
  false,bob@example.com,2,Bob
total: 2
page: 1"#,
    );
    check(
        &v,
        "hash",
        r#"users: [#2]{active,email,id,name}:
  true,alice@example.com,1,Alice
  false,bob@example.com,2,Bob
total: 2
page: 1"#,
    );
    check(
        &v,
        "indent4",
        r#"users: [2]{active,email,id,name}:
    true,alice@example.com,1,Alice
    false,bob@example.com,2,Bob
total: 2
page: 1"#,
    );
}

#[test]
fn value_array() {
    let v: Value = toon!([1, 2, 3]);
    check(&v, "default", r#"[3]: 1,2,3"#);
    check(&v, "tab", r#"[3    ]: 1	2	3"#);
    check(&v, "pipe", r#"[3|]: 1|2|3"#);
    check(&v, "hash", r#"[#3]: 1,2,3"#);
    check(&v, "tab_hash", r#"[#3    ]: 1	2	3"#);
    check(&v, "pipe_hash", r#"[#3|]: 1|2|3"#);
}

#[test]
fn value_records() {
    let v: Value = toon!([{"id": 1, "status": "active"}, {"id": 2, "status": "pending"}]);
    check(
        &v,
        "default",
        r#"[2]{id,status}:
  1,active
  2,pending"#,
    );
    check(
        &v,
        "hash",
        r#"[#2]{id,status}:
  1,active
  2,pending"#,
    );
    check(
        &v,
        "indent4",
        r#"[2]{id,status}:
    1,active
    2,pending"#,
    );
}

#[test]
fn labeled() {
    let v: Labeled = Labeled {
        label: "home".to_string(),
        origin: Point { x: 5, y: -5 },
    };
    check(
        &v,
        "default",
        r#"label: home
origin:
  x: 5
  y: -5"#,
    );
    check(
        &v,
        "indent4",
        r#"label: home
origin:
    x: 5
    y: -5"#,
    );
}

#[test]
fn crew() {
    let v: Crew = Crew {
        label: "core".to_string(),
        members: vec![Point { x: 1, y: 2 }, Point { x: 3, y: 4 }],
    };
    check(
        &v,
        "default",
        r#"label: core
members: [2]{x,y}:
  1,2
  3,4"#,
    );
    check(
        &v,
        "hash",
        r#"label: core
members: [#2]{x,y}:
  1,2
  3,4"#,
    );
    check(
        &v,
        "indent4",
        r#"label: core
members: [2]{x,y}:
    1,2
    3,4"#,
    );
}

#[test]
fn note_comma() {
    let v: Note = Note {
        body: "hello, world".to_string(),
        author: "a: b".to_string(),
    };
    check(
        &v,
        "default",
        r#"body: "hello, world"
author: "a: b""#,
    );
}

#[test]
fn note_quotes() {
    let v: Note = Note {
        body: "say \"hi\"".to_string(),
        author: "".to_string(),
    };
    check(
        &v,
        "default",
        r#"body: "say \"hi\""
author: """#,
    );
}

#[test]
fn note_keywords() {
    let v: Note = Note {
        body: "true".to_string(),
        author: "42".to_string(),
    };
    check(
        &v,
        "default",
        r#"body: "true"
author: "42""#,
    );
}

#[test]
fn vec_note() {
    let v: Vec<Note> = vec![
        Note {
            body: "x".to_string(),
            author: "y".to_string(),
        },
        Note {
            body: "p q".to_string(),
            author: "r".to_string(),
        },
    ];
    check(
        &v,
        "default",
        r#"[2]{author,body}:
  y,x
  r,p q"#,
    );
    check(
        &v,
        "hash",
        r#"[#2]{author,body}:
  y,x
  r,p q"#,
    );
    check(
        &v,
        "indent4",
        r#"[2]{author,body}:
    y,x
    r,p q"#,
    );
}

#[test]
fn value_object_safe_keys() {
    let v: Value = toon!({"id": 1, "label": "x", "xs": [1, 2], "ok": true});
    check(
        &v,
        "default",
        r#"id: 1
label: x
xs: [2]: 1,2
ok: true"#,
    );
    check(
        &v,
        "tab",
        r#"id: 1
label: x
xs: [2    ]: 1	2
ok: true"#,
    );
    check(
        &v,
        "pipe",
        r#"id: 1
label: x
xs: [2|]: 1|2
ok: true"#,
    );
    check(
        &v,
        "hash",
        r#"id: 1
label: x
xs: [#2]: 1,2
ok: true"#,
    );
    check(
        &v,
        "tab_hash",
        r#"id: 1
label: x
xs: [#2    ]: 1	2
ok: true"#,
    );
    check(
        &v,
        "pipe_hash",
        r#"id: 1
label: x
xs: [#2|]: 1|2
ok: true"#,
    );
}

#[test]
fn value_nested_object() {
    let v: Value = toon!({"outer": {"inner": 1}, "list": ["a", "b"]});
    check(
        &v,
        "default",
        r#"outer:
  inner: 1
list: [2]: a,b"#,
    );
    check(
        &v,
        "tab",
        r#"outer:
  inner: 1
list: [2    ]: a	b"#,
    );
    check(
        &v,
        "pipe",
        r#"outer:
  inner: 1
list: [2|]: a|b"#,
    );
    check(
        &v,
        "hash",
        r#"outer:
  inner: 1
list: [#2]: a,b"#,
    );
    check(
        &v,
        "tab_hash",
        r#"outer:
  inner: 1
list: [#2    ]: a	b"#,
    );
    check(
        &v,
        "pipe_hash",
        r#"outer:
  inner: 1
list: [#2|]: a|b"#,
    );
    check(
        &v,
        "indent4",
        r#"outer:
    inner: 1
list: [2]: a,b"#,
    );
}
