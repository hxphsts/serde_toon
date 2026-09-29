//! Compile-time guard of the public API.
//!
//! IF THIS FILE STOPS COMPILING, A PUBLIC API WAS BROKEN.
//!
//! Every public item that serde_toon 0.2.0 shipped is named here with its
//! exact signature: function items are coerced to fn-pointer types, generic
//! functions are called from wrappers carrying exactly the published bounds
//! (so tightening a bound fails), enums are matched exhaustively without
//! wildcard arms (so adding *or* removing a variant fails, since none of them
//! is `#[non_exhaustive]`), structs with public fields are built with struct
//! literals and destructured without `..` (so adding a field fails), and
//! trait implementations and auto traits are asserted through bounds.
//!
//! The 0.3.0 additions (`from_*_with_options`, `DecodeOptions`,
//! `SPEC_VERSION`, `Deserializer::from_str_with_options`, and the new derives
//! on `Delimiter` and `ToonOptions`) are pinned in a separate section.
//!
//! Do not "fix" a compile error here by editing this file: restore the API
//! instead, or make the change in a new major version. `cargo-semver-checks`
//! (CI job `semver`) covers the same ground against the crates.io release;
//! this file makes the check local and immediate. Most tests here only need
//! to compile; the few runtime assertions are cheap sanity checks.

#![allow(deprecated)] // `ToonOptions::length_marker` / `with_length_marker` are part of the 0.2 API.
#![allow(clippy::type_complexity)]

use chrono::{DateTime, Utc};
use num_bigint::BigInt;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::convert::TryFrom;
use std::fmt::{Debug, Display};
use std::io;
use std::panic::{RefUnwindSafe, UnwindSafe};

use serde_toon::{
    DecodeOptions, Delimiter, Deserializer, Error, Number, Result, Serializer, ToonMap,
    ToonOptions, Value, ValueSerializer,
};

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct Foo {
    x: i32,
}

// ---------------------------------------------------------------------------
// Assertion helpers
// ---------------------------------------------------------------------------

fn assert_send_sync<T: Send + Sync>() {}
fn assert_unwind_safe<T: UnwindSafe + RefUnwindSafe>() {}
fn assert_static<T: 'static>() {}

// ---------------------------------------------------------------------------
// Public modules and re-export paths
// ---------------------------------------------------------------------------

#[allow(unused_imports)]
mod paths {
    // Every public module is reachable.
    use serde_toon::{de, error, macros, map, options, ser, spec, value};

    // Items are reachable both at the crate root and at their module path.
    use serde_toon::de::Deserializer;
    use serde_toon::error::{Error, Result};
    use serde_toon::map::ToonMap;
    use serde_toon::options::{Delimiter, ToonOptions};
    use serde_toon::ser::{
        MapSerializer, SeqSerializer, SerializeMap, SerializeVec, Serializer, StructSerializer,
        StructVariantSerializer, TupleSerializer, TupleStructSerializer, TupleVariantSerializer,
        ValueSerializer,
    };
    use serde_toon::value::{Number, Value};
    use serde_toon::{
        from_reader, from_slice, from_str, to_string, to_string_pretty, to_string_with_options,
        to_value, to_writer, to_writer_with_options, toon,
    };

    // 0.3.0 additions.
    use serde_toon::options::DecodeOptions;
    use serde_toon::{
        from_reader_with_options, from_slice_with_options, from_str_with_options, SPEC_VERSION,
    };
}

#[test]
fn root_reexports_are_the_module_items() {
    fn same<T>(_: T, _: T) {}
    same::<Option<serde_toon::Deserializer<'static>>>(None, None::<serde_toon::de::Deserializer>);
    same::<Option<serde_toon::Error>>(None, None::<serde_toon::error::Error>);
    same::<Option<serde_toon::Result<u8>>>(None, None::<serde_toon::error::Result<u8>>);
    same::<Option<serde_toon::ToonMap>>(None, None::<serde_toon::map::ToonMap>);
    same::<Option<serde_toon::Delimiter>>(None, None::<serde_toon::options::Delimiter>);
    same::<Option<serde_toon::ToonOptions>>(None, None::<serde_toon::options::ToonOptions>);
    same::<Option<serde_toon::Serializer>>(None, None::<serde_toon::ser::Serializer>);
    same::<Option<serde_toon::ValueSerializer>>(None, None::<serde_toon::ser::ValueSerializer>);
    same::<Option<serde_toon::Number>>(None, None::<serde_toon::value::Number>);
    same::<Option<serde_toon::Value>>(None, None::<serde_toon::value::Value>);
    same::<Option<serde_toon::DecodeOptions>>(None, None::<serde_toon::options::DecodeOptions>);
}

// ---------------------------------------------------------------------------
// Top-level functions
// ---------------------------------------------------------------------------

#[test]
fn top_level_function_signatures() {
    // Exact signatures, with the generic parameter order (turbofish) pinned.
    let _: fn(&Foo) -> Result<String> = serde_toon::to_string::<Foo>;
    let _: fn(&str) -> Result<String> = serde_toon::to_string::<str>;
    let _: fn(&Foo) -> Result<String> = serde_toon::to_string_pretty::<Foo>;
    let _: fn(&[i32]) -> Result<String> = serde_toon::to_string_pretty::<[i32]>;
    let _: fn(&Foo, ToonOptions) -> Result<String> = serde_toon::to_string_with_options::<Foo>;
    let _: fn(&str, ToonOptions) -> Result<String> = serde_toon::to_string_with_options::<str>;
    let _: fn(&Foo) -> Result<Value> = serde_toon::to_value::<Foo>;
    let _: fn(&str) -> Result<Value> = serde_toon::to_value::<str>;
    let _: fn(Vec<u8>, &Foo) -> Result<()> = serde_toon::to_writer::<Vec<u8>, Foo>;
    let _: fn(Vec<u8>, &str) -> Result<()> = serde_toon::to_writer::<Vec<u8>, str>;
    let _: fn(Vec<u8>, &Foo, ToonOptions) -> Result<()> =
        serde_toon::to_writer_with_options::<Vec<u8>, Foo>;
    let _: fn(&'static str) -> Result<Foo> = serde_toon::from_str::<Foo>;
    let _: fn(&'static [u8]) -> Result<Foo> = serde_toon::from_slice::<Foo>;
    let _: fn(&'static [u8]) -> Result<Foo> = serde_toon::from_reader::<&'static [u8], Foo>;
    let _: fn(io::Empty) -> Result<Foo> = serde_toon::from_reader::<io::Empty, Foo>;
}

// Wrappers carrying exactly the published bounds: if a bound is tightened,
// these stop compiling.
fn generic_to_string<T: ?Sized + Serialize>(v: &T) -> Result<String> {
    serde_toon::to_string(v)
}
fn generic_to_string_pretty<T: ?Sized + Serialize>(v: &T) -> Result<String> {
    serde_toon::to_string_pretty(v)
}
fn generic_to_string_with_options<T: ?Sized + Serialize>(v: &T, o: ToonOptions) -> Result<String> {
    serde_toon::to_string_with_options(v, o)
}
fn generic_to_value<T: ?Sized + Serialize>(v: &T) -> Result<Value> {
    serde_toon::to_value(v)
}
fn generic_to_writer<W: io::Write, T: ?Sized + Serialize>(w: W, v: &T) -> Result<()> {
    serde_toon::to_writer(w, v)
}
fn generic_to_writer_with_options<W: io::Write, T: ?Sized + Serialize>(
    w: W,
    v: &T,
    o: ToonOptions,
) -> Result<()> {
    serde_toon::to_writer_with_options(w, v, o)
}
fn generic_from_str<'a, T: Deserialize<'a>>(s: &'a str) -> Result<T> {
    serde_toon::from_str(s)
}
fn generic_from_slice<'a, T: Deserialize<'a>>(s: &'a [u8]) -> Result<T> {
    serde_toon::from_slice(s)
}
fn generic_from_reader<R: io::Read, T: DeserializeOwned>(r: R) -> Result<T> {
    serde_toon::from_reader(r)
}
// Zero-copy deserialization borrows from the input.
fn borrowed_from_str(s: &str) -> Result<&str> {
    serde_toon::from_str(s)
}

#[test]
fn top_level_functions_accept_published_bounds() {
    let foo = Foo { x: 1 };
    let text = generic_to_string(&foo).unwrap();
    assert_eq!(generic_from_str::<Foo>(&text).unwrap(), foo);
    assert_eq!(generic_from_slice::<Foo>(text.as_bytes()).unwrap(), foo);
    assert_eq!(generic_from_reader::<_, Foo>(text.as_bytes()).unwrap(), foo);
    generic_to_string_pretty(&foo).unwrap();
    generic_to_string_with_options(&foo, ToonOptions::new()).unwrap();
    generic_to_value(&foo).unwrap();
    generic_to_writer(Vec::new(), &foo).unwrap();
    generic_to_writer(&mut Vec::new(), "unsized").unwrap();
    generic_to_writer_with_options(io::sink(), &foo, ToonOptions::new()).unwrap();
    let _: fn(&str) -> Result<&str> = borrowed_from_str;
}

// ---------------------------------------------------------------------------
// Error and Result
// ---------------------------------------------------------------------------

#[test]
fn result_alias() {
    fn same(r: Result<u8>) -> std::result::Result<u8, Error> {
        r
    }
    fn back(r: std::result::Result<u8, Error>) -> Result<u8> {
        r
    }
    assert_eq!(back(same(Ok(1))).unwrap(), 1);
}

/// Exhaustive, wildcard-free match with every field named and typed.
fn error_variants(e: Error) {
    match e {
        Error::Io(msg) => {
            let _: String = msg;
        }
        Error::Syntax {
            line,
            col,
            msg,
            context,
            suggestion,
        } => {
            let _: (usize, usize, String, String, String) = (line, col, msg, context, suggestion);
        }
        Error::TypeMismatch {
            line,
            col,
            expected,
            found,
        } => {
            let _: (usize, usize, String, String) = (line, col, expected, found);
        }
        Error::IndentationError {
            line,
            col,
            expected,
            found,
            context,
        } => {
            let _: (usize, usize, usize, usize, String) = (line, col, expected, found, context);
        }
        Error::UnsupportedType(msg) => {
            let _: String = msg;
        }
        Error::InvalidFormat { line, col, msg } => {
            let _: (usize, usize, String) = (line, col, msg);
        }
        Error::UnexpectedEof {
            line,
            col,
            expected,
            context,
        } => {
            let _: (usize, usize, String, String) = (line, col, expected, context);
        }
        Error::Custom(msg) => {
            let _: String = msg;
        }
        Error::Message(msg) => {
            let _: String = msg;
        }
    }
}

#[test]
fn error_variants_and_constructors() {
    let _: fn(Error) = error_variants;

    // Every variant is constructible with its public fields.
    let s = String::new;
    let all = vec![
        Error::Io(s()),
        Error::Syntax {
            line: 1,
            col: 1,
            msg: s(),
            context: s(),
            suggestion: s(),
        },
        Error::TypeMismatch {
            line: 1,
            col: 1,
            expected: s(),
            found: s(),
        },
        Error::IndentationError {
            line: 1,
            col: 1,
            expected: 2,
            found: 3,
            context: s(),
        },
        Error::UnsupportedType(s()),
        Error::InvalidFormat {
            line: 1,
            col: 1,
            msg: s(),
        },
        Error::UnexpectedEof {
            line: 1,
            col: 1,
            expected: s(),
            context: s(),
        },
        Error::Custom(s()),
        Error::Message(s()),
    ];
    for e in all {
        error_variants(e);
    }

    // Inherent constructors.
    let _: fn(usize, usize, &str) -> Error = Error::syntax;
    let _: fn(usize, usize, &str, &str, Option<&str>) -> Error = Error::syntax_with_context;
    let _: fn(usize, usize, &str, &str) -> Error = Error::type_mismatch;
    let _: fn(usize, usize, usize, usize, &str) -> Error = Error::indentation_error;
    let _: fn(usize, usize, &str) -> Error = Error::invalid_format;
    let _: fn(usize, usize, &str, &str) -> Error = Error::unexpected_eof;
    let _: fn(&str) -> Error = Error::unsupported_type;
    let _: fn(String) -> Error = Error::custom::<String>;
    let _: fn(&'static str) -> Error = Error::custom::<&'static str>;
    let _: fn(&str) -> Error = Error::io;
    fn custom_any<T: Display>(msg: T) -> Error {
        Error::custom(msg)
    }
    assert!(custom_any(42).to_string().contains("42"));

    // serde error traits.
    let _: Error = <Error as serde::ser::Error>::custom("x");
    let _: Error = <Error as serde::de::Error>::custom("x");
    let _: Error = <Error as serde::de::Error>::missing_field("x");
}

fn error_traits<
    E: std::error::Error
        + Debug
        + Display
        + Clone
        + serde::ser::Error
        + serde::de::Error
        + Send
        + Sync
        + 'static,
>() {
}

#[test]
fn error_trait_impls() {
    error_traits::<Error>();
    assert_unwind_safe::<Error>();
    // `Box<dyn Error + Send + Sync>` conversion via `?` keeps working.
    fn boxed() -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Err(Error::custom("x"))?;
        Ok(())
    }
    assert!(boxed().is_err());
}

// ---------------------------------------------------------------------------
// Value and Number
// ---------------------------------------------------------------------------

fn value_variants(v: Value) {
    match v {
        Value::Null => {}
        Value::Bool(b) => {
            let _: bool = b;
        }
        Value::Number(n) => {
            let _: Number = n;
        }
        Value::String(s) => {
            let _: String = s;
        }
        Value::Array(a) => {
            let _: Vec<Value> = a;
        }
        Value::Object(o) => {
            let _: ToonMap = o;
        }
        Value::Table { headers, rows } => {
            let _: (Vec<String>, Vec<Vec<Value>>) = (headers, rows);
        }
        Value::Date(d) => {
            let _: DateTime<Utc> = d;
        }
        Value::BigInt(b) => {
            let _: BigInt = b;
        }
    }
}

fn number_variants(n: Number) {
    match n {
        Number::Integer(i) => {
            let _: i64 = i;
        }
        Number::Float(f) => {
            let _: f64 = f;
        }
        Number::Infinity => {}
        Number::NegativeInfinity => {}
        Number::NaN => {}
    }
}

#[test]
fn value_and_number_variants() {
    let _: fn(Value) = value_variants;
    let _: fn(Number) = number_variants;
    let _ = |d: DateTime<Utc>, b: BigInt| {
        vec![
            Value::Null,
            Value::Bool(true),
            Value::Number(Number::Integer(1)),
            Value::String(String::new()),
            Value::Array(Vec::new()),
            Value::Object(ToonMap::new()),
            Value::Table {
                headers: Vec::new(),
                rows: Vec::new(),
            },
            Value::Date(d),
            Value::BigInt(b),
        ]
    };
    for n in [
        Number::Integer(1),
        Number::Float(1.5),
        Number::Infinity,
        Number::NegativeInfinity,
        Number::NaN,
    ] {
        number_variants(n);
    }
}

// `const fn`s stay `const`.
const _: bool = Value::Null.is_null();
const _: bool = Value::Bool(true).is_bool();
const _: bool = Number::Integer(1).is_integer();
const _: bool = Number::Float(1.0).is_float();
const _: bool = Number::NaN.is_special();

#[test]
fn value_methods() {
    let _: fn(&Value) -> bool = Value::is_null;
    let _: fn(&Value) -> bool = Value::is_bool;
    let _: fn(&Value) -> bool = Value::is_number;
    let _: fn(&Value) -> bool = Value::is_string;
    let _: fn(&Value) -> bool = Value::is_array;
    let _: fn(&Value) -> bool = Value::is_object;
    let _: fn(&Value) -> bool = Value::is_table;
    let _: fn(&Value) -> bool = Value::is_date;
    let _: fn(&Value) -> bool = Value::is_bigint;
    let _: fn(&Value) -> Option<bool> = Value::as_bool;
    let _: fn(&Value) -> Option<&str> = Value::as_str;
    let _: fn(&Value) -> Option<i64> = Value::as_i64;
    let _: fn(&Value) -> Option<&Vec<Value>> = Value::as_array;
    let _: fn(&Value) -> Option<&ToonMap> = Value::as_object;
    let _: fn(&Value) -> Option<&DateTime<Utc>> = Value::as_date;
    let _: fn(&Value) -> Option<&BigInt> = Value::as_bigint;
    let _: fn(&Value) -> bool = Value::needs_quotes;

    let _: fn(&Number) -> bool = Number::is_integer;
    let _: fn(&Number) -> bool = Number::is_float;
    let _: fn(&Number) -> bool = Number::is_special;
    let _: fn(&Number) -> Option<i64> = Number::as_i64;
    let _: fn(&Number) -> f64 = Number::as_f64;
}

fn value_traits<
    T: Clone
        + Debug
        + Display
        + PartialEq
        + Default
        + Serialize
        + DeserializeOwned
        + Send
        + Sync
        + UnwindSafe
        + RefUnwindSafe
        + 'static,
>() {
}

fn number_traits<
    T: Clone + Debug + Display + PartialEq + Send + Sync + UnwindSafe + RefUnwindSafe + 'static,
>() {
}

#[test]
fn value_and_number_trait_impls() {
    value_traits::<Value>();
    number_traits::<Number>();
    assert_eq!(Value::default(), Value::Null);
}

#[test]
fn value_conversions() {
    // TryFrom<Value>, with serde_toon::Error as the error type.
    let _: fn(Value) -> Result<i64> = <i64 as TryFrom<Value>>::try_from;
    let _: fn(Value) -> Result<f64> = <f64 as TryFrom<Value>>::try_from;
    let _: fn(Value) -> Result<bool> = <bool as TryFrom<Value>>::try_from;
    let _: fn(Value) -> Result<String> = <String as TryFrom<Value>>::try_from;

    // From<primitive> for Value.
    let _: fn(bool) -> Value = Value::from;
    let _: fn(i8) -> Value = Value::from;
    let _: fn(i16) -> Value = Value::from;
    let _: fn(i32) -> Value = Value::from;
    let _: fn(i64) -> Value = Value::from;
    let _: fn(u8) -> Value = Value::from;
    let _: fn(u16) -> Value = Value::from;
    let _: fn(u32) -> Value = Value::from;
    let _: fn(f32) -> Value = Value::from;
    let _: fn(f64) -> Value = Value::from;
    let _: fn(String) -> Value = Value::from;
    let _: fn(&'static str) -> Value = Value::from;
    let _ = |s: &str| -> Value { Value::from(s) };
    let _: fn(Vec<Value>) -> Value = Value::from;
    let _: fn(ToonMap) -> Value = Value::from;

    // From<primitive> for Number.
    let _: fn(i8) -> Number = Number::from;
    let _: fn(i16) -> Number = Number::from;
    let _: fn(i32) -> Number = Number::from;
    let _: fn(i64) -> Number = Number::from;
    let _: fn(u8) -> Number = Number::from;
    let _: fn(u16) -> Number = Number::from;
    let _: fn(u32) -> Number = Number::from;
    let _: fn(f32) -> Number = Number::from;
    let _: fn(f64) -> Number = Number::from;

    // Literal inference that downstream code relies on (`Value::from(42)`
    // picks i32; adding a competing impl could make it ambiguous).
    assert_eq!(Value::from(42), Value::Number(Number::Integer(42)));
    assert_eq!(Value::from(1.5), Value::Number(Number::Float(1.5)));
    assert_eq!(Number::from(7), Number::Integer(7));
    assert_eq!(i64::try_from(Value::from(3)).unwrap(), 3);
}

// ---------------------------------------------------------------------------
// ToonMap
// ---------------------------------------------------------------------------

#[test]
fn toon_map_methods() {
    let _: fn() -> ToonMap = ToonMap::new;
    let _: fn(usize) -> ToonMap = ToonMap::with_capacity;
    let _: fn(&mut ToonMap, String, Value) -> Option<Value> = ToonMap::insert;
    let _: for<'a> fn(&'a ToonMap, &str) -> Option<&'a Value> = ToonMap::get;
    let _: fn(&ToonMap) -> usize = ToonMap::len;
    let _: fn(&ToonMap) -> bool = ToonMap::is_empty;
    let _: for<'a> fn(&'a ToonMap) -> indexmap::map::Keys<'a, String, Value> = ToonMap::keys;
    let _: for<'a> fn(&'a ToonMap) -> indexmap::map::Values<'a, String, Value> = ToonMap::values;
    let _: for<'a> fn(&'a ToonMap) -> indexmap::map::Iter<'a, String, Value> = ToonMap::iter;
}

fn toon_map_traits<
    T: Debug
        + Clone
        + PartialEq
        + Default
        + From<HashMap<String, Value>>
        + Into<HashMap<String, Value>>
        + IntoIterator<Item = (String, Value), IntoIter = indexmap::map::IntoIter<String, Value>>
        + FromIterator<(String, Value)>
        + Send
        + Sync
        + UnwindSafe
        + RefUnwindSafe
        + 'static,
>() {
}

#[test]
fn toon_map_trait_impls() {
    toon_map_traits::<ToonMap>();
    let map: ToonMap = vec![("a".to_string(), Value::from(1))]
        .into_iter()
        .collect();
    let hash: HashMap<String, Value> = map.clone().into();
    assert_eq!(ToonMap::from(hash), map);
    assert_eq!(map.into_iter().count(), 1);
}

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

fn delimiter_variants(d: Delimiter) {
    match d {
        Delimiter::Comma => {}
        Delimiter::Tab => {}
        Delimiter::Pipe => {}
    }
}

const _: &str = Delimiter::Comma.as_str();

#[test]
fn delimiter() {
    let _: fn(Delimiter) = delimiter_variants;
    let _: fn(&Delimiter) -> &'static str = Delimiter::as_str;
    fn traits<T: Clone + Debug + PartialEq + Default + Send + Sync + UnwindSafe + 'static>() {}
    traits::<Delimiter>();
    assert_eq!(Delimiter::default(), Delimiter::Comma);
    for d in [Delimiter::Comma, Delimiter::Tab, Delimiter::Pipe] {
        delimiter_variants(d);
    }
}

#[test]
fn toon_options() {
    // Struct literal with every public field and no `..`: adding a public
    // field (without `#[non_exhaustive]`, which would itself be breaking)
    // fails here.
    let opts = ToonOptions {
        indent: 2,
        delimiter: Delimiter::Comma,
        length_marker: None,
        pretty: false,
    };
    let ToonOptions {
        indent,
        delimiter,
        length_marker,
        pretty,
    } = opts.clone();
    let _: (usize, Delimiter, Option<char>, bool) = (indent, delimiter, length_marker, pretty);

    let _: fn() -> ToonOptions = ToonOptions::new;
    let _: fn() -> ToonOptions = ToonOptions::pretty;
    let _: fn(ToonOptions, usize) -> ToonOptions = ToonOptions::with_indent;
    let _: fn(ToonOptions, Delimiter) -> ToonOptions = ToonOptions::with_delimiter;
    let _: fn(ToonOptions, char) -> ToonOptions = ToonOptions::with_length_marker;

    fn traits<T: Clone + Debug + Default + Send + Sync + UnwindSafe + RefUnwindSafe + 'static>() {}
    traits::<ToonOptions>();
    let _ = ToonOptions::default();
    let _ = ToonOptions::new()
        .with_indent(4)
        .with_delimiter(Delimiter::Pipe)
        .with_length_marker('#');
    assert_eq!(opts.indent, ToonOptions::new().indent);
}

// ---------------------------------------------------------------------------
// The toon! macro
// ---------------------------------------------------------------------------

#[test]
fn toon_macro_forms() {
    let n = 5;
    let values: Vec<Value> = vec![
        serde_toon::toon!(null),
        serde_toon::toon!(true),
        serde_toon::toon!(false),
        serde_toon::toon!([]),
        serde_toon::toon!([1, "two", null]),
        serde_toon::toon!([1, 2,]),
        serde_toon::toon!({}),
        serde_toon::toon!({"a": 1, "b": [true, false], "c": {"d": null}}),
        serde_toon::toon!({"a": 1,}),
        serde_toon::toon!(42),
        serde_toon::toon!(3.5),
        serde_toon::toon!("text"),
        serde_toon::toon!(n),
        serde_toon::toon!(vec![1, 2]),
    ];
    assert_eq!(values.len(), 14);
    assert_eq!(serde_toon::toon!(null), Value::Null);
}

// ---------------------------------------------------------------------------
// Serializer types (serde_toon::ser)
// ---------------------------------------------------------------------------

/// `&mut Serializer` implements `serde::Serializer` with exactly these
/// associated types.
fn toon_serializer<'a, S>(_: S)
where
    S: serde::Serializer<
        Ok = (),
        Error = Error,
        SerializeSeq = serde_toon::ser::SeqSerializer<'a>,
        SerializeTuple = serde_toon::ser::TupleSerializer<'a>,
        SerializeTupleStruct = serde_toon::ser::TupleStructSerializer<'a>,
        SerializeTupleVariant = serde_toon::ser::TupleVariantSerializer<'a>,
        SerializeMap = serde_toon::ser::MapSerializer<'a>,
        SerializeStruct = serde_toon::ser::StructSerializer<'a>,
        SerializeStructVariant = serde_toon::ser::StructVariantSerializer<'a>,
    >,
{
}

fn value_serializer<S>(_: S)
where
    S: serde::Serializer<
        Ok = Value,
        Error = Error,
        SerializeSeq = serde_toon::ser::SerializeVec,
        SerializeTuple = serde_toon::ser::SerializeVec,
        SerializeTupleStruct = serde_toon::ser::SerializeVec,
        SerializeTupleVariant = serde_toon::ser::SerializeVec,
        SerializeMap = serde_toon::ser::SerializeMap,
        SerializeStruct = serde_toon::ser::SerializeMap,
        SerializeStructVariant = serde_toon::ser::SerializeMap,
    >,
{
}

#[test]
fn serializer_types() {
    let _: fn(ToonOptions) -> Serializer = Serializer::new;
    let _: fn(Serializer) -> String = Serializer::into_inner;

    let mut ser = Serializer::new(ToonOptions::new());
    toon_serializer(&mut ser);
    Foo { x: 1 }.serialize(&mut ser).unwrap();
    assert_eq!(ser.into_inner(), "x: 1");

    // `ValueSerializer` is a unit struct, constructible by name.
    let vs: ValueSerializer = ValueSerializer;
    value_serializer(vs);
    assert!(Foo { x: 1 }.serialize(ValueSerializer).unwrap().is_object());

    assert_send_sync::<Serializer>();
    assert_send_sync::<ValueSerializer>();
    assert_send_sync::<serde_toon::ser::SerializeVec>();
    assert_send_sync::<serde_toon::ser::SerializeMap>();
    assert_send_sync::<serde_toon::ser::SeqSerializer<'static>>();
    assert_send_sync::<serde_toon::ser::TupleSerializer<'static>>();
    assert_send_sync::<serde_toon::ser::TupleStructSerializer<'static>>();
    assert_send_sync::<serde_toon::ser::TupleVariantSerializer<'static>>();
    assert_send_sync::<serde_toon::ser::MapSerializer<'static>>();
    assert_send_sync::<serde_toon::ser::StructSerializer<'static>>();
    assert_send_sync::<serde_toon::ser::StructVariantSerializer<'static>>();
    assert_unwind_safe::<Serializer>();
    assert_unwind_safe::<ValueSerializer>();
}

// ---------------------------------------------------------------------------
// Deserializer (serde_toon::de)
// ---------------------------------------------------------------------------

fn toon_deserializer<'de, D: serde::Deserializer<'de, Error = Error>>(_: D) {}

#[test]
fn deserializer_types() {
    let _: fn(&'static str) -> Deserializer<'static> = Deserializer::from_str;
    fn from_str_any<'de>(s: &'de str) -> Deserializer<'de> {
        Deserializer::from_str(s)
    }

    let mut de = from_str_any("x: 1");
    toon_deserializer(&mut de);
    assert_eq!(Foo::deserialize(&mut de).unwrap(), Foo { x: 1 });

    assert_send_sync::<Deserializer<'static>>();
    assert_unwind_safe::<Deserializer<'static>>();
}

// ---------------------------------------------------------------------------
// Auto traits on owned public types
// ---------------------------------------------------------------------------

#[test]
fn auto_traits() {
    assert_send_sync::<Value>();
    assert_send_sync::<Number>();
    assert_send_sync::<ToonMap>();
    assert_send_sync::<Error>();
    assert_send_sync::<ToonOptions>();
    assert_send_sync::<Delimiter>();
    assert_send_sync::<Result<Value>>();
    assert_unwind_safe::<Value>();
    assert_unwind_safe::<Number>();
    assert_unwind_safe::<ToonMap>();
    assert_unwind_safe::<Delimiter>();
    assert_static::<Value>();
    assert_static::<Error>();
}

// ---------------------------------------------------------------------------
// 0.3.0 additions
// ---------------------------------------------------------------------------

const _: &str = serde_toon::SPEC_VERSION;
const _: DecodeOptions = DecodeOptions::strict();
const _: DecodeOptions = DecodeOptions::lenient();
const _: DecodeOptions = DecodeOptions::compatible().with_indent_size(4);
const _: bool = DecodeOptions::strict().is_strict();
const _: bool = DecodeOptions::strict().is_compatible();
const _: usize = DecodeOptions::strict().indent_size();

fn generic_from_str_with_options<'a, T: Deserialize<'a>>(
    s: &'a str,
    o: DecodeOptions,
) -> Result<T> {
    serde_toon::from_str_with_options(s, o)
}
fn generic_from_slice_with_options<'a, T: Deserialize<'a>>(
    s: &'a [u8],
    o: DecodeOptions,
) -> Result<T> {
    serde_toon::from_slice_with_options(s, o)
}
fn generic_from_reader_with_options<R: io::Read, T: DeserializeOwned>(
    r: R,
    o: DecodeOptions,
) -> Result<T> {
    serde_toon::from_reader_with_options(r, o)
}

#[test]
fn additions_0_3() {
    let _: fn(&'static str, DecodeOptions) -> Result<Foo> =
        serde_toon::from_str_with_options::<Foo>;
    let _: fn(&'static [u8], DecodeOptions) -> Result<Foo> =
        serde_toon::from_slice_with_options::<Foo>;
    let _: fn(&'static [u8], DecodeOptions) -> Result<Foo> =
        serde_toon::from_reader_with_options::<&'static [u8], Foo>;
    let _: fn(&'static str, DecodeOptions) -> Deserializer<'static> =
        Deserializer::from_str_with_options;

    let _: fn() -> DecodeOptions = DecodeOptions::strict;
    let _: fn() -> DecodeOptions = DecodeOptions::lenient;
    let _: fn() -> DecodeOptions = DecodeOptions::compatible;
    let _: fn(DecodeOptions, usize) -> DecodeOptions = DecodeOptions::with_indent_size;
    let _: fn(&DecodeOptions) -> bool = DecodeOptions::is_strict;
    let _: fn(&DecodeOptions) -> bool = DecodeOptions::is_compatible;
    let _: fn(&DecodeOptions) -> usize = DecodeOptions::indent_size;

    fn decode_options_traits<
        T: Clone
            + Copy
            + Debug
            + PartialEq
            + Eq
            + std::hash::Hash
            + Default
            + Send
            + Sync
            + UnwindSafe
            + RefUnwindSafe
            + 'static,
    >() {
    }
    decode_options_traits::<DecodeOptions>();
    assert_eq!(DecodeOptions::default(), DecodeOptions::compatible());

    // Derives added to existing types in 0.3.0.
    fn delimiter_traits<T: Copy + Eq + std::hash::Hash>() {}
    delimiter_traits::<Delimiter>();
    fn options_traits<T: PartialEq>() {}
    options_traits::<ToonOptions>();

    let foo = Foo { x: 1 };
    let o = DecodeOptions::strict();
    assert_eq!(
        generic_from_str_with_options::<Foo>("x: 1", o).unwrap(),
        foo
    );
    assert_eq!(
        generic_from_slice_with_options::<Foo>(b"x: 1", o).unwrap(),
        foo
    );
    assert_eq!(
        generic_from_reader_with_options::<_, Foo>(&b"x: 1"[..], o).unwrap(),
        foo
    );
}
