//! # serde_toon
//!
//! A Serde-compatible serialization library for the TOON (Token-Oriented Object Notation) format.
//!
//! ## What is TOON?
//!
//! TOON is a compact, human-readable data format specifically designed for efficient communication
//! with Large Language Models (LLMs). It achieves 30-60% fewer tokens than equivalent JSON while
//! maintaining readability and structure.
//!
//! ## Key Features
//!
//! - **Token-Efficient**: Minimalist syntax reduces token count by eliminating unnecessary braces,
//!   brackets, and quotes
//! - **Tabular Arrays**: Homogeneous object arrays serialize as compact tables with headers
//! - **Serde Compatible**: Works seamlessly with existing Rust types via `#[derive(Serialize, Deserialize)]`
//! - **Type Safe**: Statically typed with comprehensive error reporting
//! - **No Unsafe Code**: Written entirely in safe Rust with zero unsafe blocks
//!
//! ## Quick Start
//!
//! Add this to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! serde = { version = "1.0", features = ["derive"] }
//! serde_toon = "0.2"
//! ```
//!
//! ### Basic Serialization and Deserialization
//!
//! ```rust
//! use serde::{Deserialize, Serialize};
//! use serde_toon::{to_string, from_str};
//!
//! #[derive(Serialize, Deserialize, PartialEq, Debug)]
//! struct User {
//!     id: u32,
//!     name: String,
//!     active: bool,
//! }
//!
//! let user = User {
//!     id: 123,
//!     name: "Alice".to_string(),
//!     active: true,
//! };
//!
//! // Serialize to TOON format
//! let toon_string = to_string(&user).unwrap();
//! assert_eq!(toon_string, "id: 123\nname: Alice\nactive: true");
//!
//! // Deserialize back
//! let user_back: User = from_str(&toon_string).unwrap();
//! assert_eq!(user, user_back);
//! ```
//!
//! ### Working with Arrays (Tabular Format)
//!
//! Arrays of homogeneous objects automatically serialize as space-efficient tables:
//!
//! ```rust
//! use serde::{Deserialize, Serialize};
//! use serde_toon::to_string;
//!
//! #[derive(Serialize, Deserialize)]
//! struct Product {
//!     id: u32,
//!     name: String,
//!     price: f64,
//! }
//!
//! let products = vec![
//!     Product { id: 1, name: "Widget".to_string(), price: 9.99 },
//!     Product { id: 2, name: "Gadget".to_string(), price: 14.99 },
//! ];
//!
//! let toon = to_string(&products).unwrap();
//! assert_eq!(toon, "[2]{id,name,price}:\n  1,Widget,9.99\n  2,Gadget,14.99");
//! ```
//!
//! ### Dynamic Values with toon! Macro
//!
//! ```rust
//! use serde_toon::{toon, Value};
//!
//! let data = toon!({
//!     "name": "Alice",
//!     "age": 30,
//!     "tags": ["rust", "serde", "llm"]
//! });
//!
//! if let Value::Object(obj) = data {
//!     assert_eq!(obj.get("name").and_then(|v| v.as_str()), Some("Alice"));
//! }
//! ```
//!
//! ## Performance Characteristics
//!
//! - **Serialization**: O(n) where n is the number of fields/elements
//! - **Deserialization**: O(n) with single-pass parsing
//! - **Memory**: Pre-allocated buffers minimize reallocations
//! - **Token Count**: 30-60% reduction vs JSON for typical structured data
//!
//! ## Safety Guarantees
//!
//! - No `unsafe` code blocks
//! - All array indexing is bounds-checked
//! - Proper error propagation with `Result` types
//! - No panics in public API (except for logic errors that indicate bugs)
//!
//! ## Format Specification
//!
//! For the complete TOON format specification, see the [`spec`] module documentation.
//!
//! External reference: <https://github.com/johannschopplich/toon>
//!
//! ## Examples
//!
//! See the `examples/` directory for focused, production-ready examples:
//!
//! - **`simple.rs`** - Your first TOON experience (basic serialization)
//! - **`macro.rs`** - Building values with the toon! macro
//! - **`tabular_arrays.rs`** - TOON's tabular feature for repeated structures
//! - **`dynamic_values.rs`** - Working with Value dynamically
//! - **`custom_options.rs`** - Customizing delimiters and formatting
//! - **`token_efficiency.rs`** - TOON vs JSON comparison
//!
//! Run any example with: `cargo run --example <name>`

pub mod de;
pub mod error;
mod lexical;
pub mod macros;
pub mod map;
pub mod options;
pub mod ser;
pub mod spec;
pub mod value;

pub use de::Deserializer;
pub use error::{Error, Result};
pub use map::ToonMap;
pub use options::{DecodeOptions, Delimiter, ToonOptions};
pub use ser::{Serializer, ValueSerializer};
pub use value::{Number, Value};

use serde::{Deserialize, Serialize};
use std::io;

/// The version of the [TOON specification](https://github.com/toon-format/spec)
/// this crate targets.
///
/// # Examples
///
/// ```rust
/// assert_eq!(serde_toon::SPEC_VERSION, "4.1");
/// ```
pub const SPEC_VERSION: &str = "4.1";

/// Serialize any `T: Serialize` to a TOON string, using the default options
/// (2-space indentation, comma delimiter).
///
/// See the [`ser`] module for how Rust values map to TOON.
///
/// # Examples
///
/// ```rust
/// use serde_toon::to_string;
/// use serde::Serialize;
///
/// #[derive(Serialize)]
/// struct Point { x: i32, y: i32 }
///
/// let point = Point { x: 1, y: 2 };
/// assert_eq!(to_string(&point)?, "x: 1\ny: 2");
/// # Ok::<(), serde_toon::Error>(())
/// ```
///
/// # Errors
///
/// Returns an error if the value's `Serialize` impl fails, or if a map key
/// is not a string, char, integer, or boolean.
#[must_use = "this returns the result of the operation, errors must be handled"]
pub fn to_string<T>(value: &T) -> Result<String>
where
    T: ?Sized + Serialize,
{
    to_string_with_options(value, ToonOptions::default())
}

/// Serialize any `T: Serialize` to a TOON string using [`ToonOptions::pretty`].
///
/// TOON output is always multi-line and indented, so this returns the same
/// text as [`to_string`]; it is kept for compatibility.
///
/// # Examples
///
/// ```rust
/// use serde_toon::{to_string, to_string_pretty};
/// use serde::Serialize;
///
/// #[derive(Serialize)]
/// struct Point { x: i32, y: i32 }
///
/// let point = Point { x: 1, y: 2 };
/// assert_eq!(to_string_pretty(&point)?, to_string(&point)?);
/// # Ok::<(), serde_toon::Error>(())
/// ```
///
/// # Errors
///
/// Returns an error in the same cases as [`to_string`].
#[must_use = "this returns the result of the operation, errors must be handled"]
pub fn to_string_pretty<T>(value: &T) -> Result<String>
where
    T: ?Sized + Serialize,
{
    to_string_with_options(value, ToonOptions::pretty())
}

/// Serialize any `T: Serialize` to a TOON string with custom options.
///
/// The options set the indentation width and the document delimiter, which
/// every array header declares.
///
/// # Examples
///
/// ```rust
/// use serde_toon::{to_string_with_options, ToonOptions, Delimiter};
/// use serde::Serialize;
///
/// #[derive(Serialize)]
/// struct Point { x: i32, y: i32 }
///
/// #[derive(Serialize)]
/// struct Shape { points: Vec<Point> }
///
/// let shape = Shape { points: vec![Point { x: 1, y: 2 }, Point { x: 3, y: 4 }] };
/// let options = ToonOptions::new()
///     .with_delimiter(Delimiter::Pipe)
///     .with_indent(4);
/// assert_eq!(
///     to_string_with_options(&shape, options)?,
///     "points[2|]{x|y}:\n    1|2\n    3|4"
/// );
/// # Ok::<(), serde_toon::Error>(())
/// ```
///
/// # Errors
///
/// Returns an error if `options.indent` is 0, or in the same cases as
/// [`to_string`].
#[must_use = "this returns the result of the operation, errors must be handled"]
pub fn to_string_with_options<T>(value: &T, options: ToonOptions) -> Result<String>
where
    T: ?Sized + Serialize,
{
    let mut serializer = Serializer::new(options);
    value.serialize(&mut serializer)?;
    Ok(serializer.into_inner())
}

/// Convert any `T: Serialize` to a `Value`.
///
/// Useful for working with TOON data dynamically when the structure isn't known at compile time.
///
/// # Examples
///
/// ```rust
/// use serde_toon::{to_value, Value};
/// use serde::Serialize;
///
/// #[derive(Serialize)]
/// struct Point { x: i32, y: i32 }
///
/// let point = Point { x: 1, y: 2 };
/// let value: Value = to_value(&point).unwrap();
/// assert!(value.is_object());
/// ```
///
/// # Errors
///
/// Returns an error if the value cannot be serialized.
#[must_use = "this returns the result of the operation, errors must be handled"]
pub fn to_value<T>(value: &T) -> Result<Value>
where
    T: ?Sized + Serialize,
{
    value.serialize(crate::ser::ValueSerializer)
}

/// Serialize any `T: Serialize` to a writer in TOON format.
///
/// # Examples
///
/// ```rust
/// use serde_toon::to_writer;
/// use serde::Serialize;
///
/// #[derive(Serialize)]
/// struct Point { x: i32, y: i32 }
///
/// let point = Point { x: 1, y: 2 };
/// let mut buffer = Vec::new();
/// to_writer(&mut buffer, &point)?;
/// assert_eq!(buffer, b"x: 1\ny: 2");
/// # Ok::<(), serde_toon::Error>(())
/// ```
///
/// # Errors
///
/// Returns an error if serialization fails (see [`to_string`]) or writing to
/// the writer fails.
#[must_use = "this returns the result of the operation, errors must be handled"]
pub fn to_writer<W, T>(writer: W, value: &T) -> Result<()>
where
    W: io::Write,
    T: ?Sized + Serialize,
{
    to_writer_with_options(writer, value, ToonOptions::default())
}

/// Serialize any `T: Serialize` to a writer in TOON format with custom options.
///
/// The document is built in memory and written with a single `write_all`.
///
/// # Examples
///
/// ```rust
/// use serde_toon::{to_writer_with_options, Delimiter, ToonOptions};
///
/// let mut buffer = Vec::new();
/// let options = ToonOptions::new().with_delimiter(Delimiter::Tab);
/// to_writer_with_options(&mut buffer, &["a", "b"], options)?;
/// assert_eq!(buffer, b"[2\t]: a\tb");
/// # Ok::<(), serde_toon::Error>(())
/// ```
///
/// # Errors
///
/// Returns an error if serialization fails (see [`to_string_with_options`])
/// or writing to the writer fails.
#[must_use = "this returns the result of the operation, errors must be handled"]
pub fn to_writer_with_options<W, T>(mut writer: W, value: &T, options: ToonOptions) -> Result<()>
where
    W: io::Write,
    T: ?Sized + Serialize,
{
    let toon_string = to_string_with_options(value, options)?;
    writer
        .write_all(toon_string.as_bytes())
        .map_err(|e| Error::io(&e.to_string()))?;
    Ok(())
}

/// Deserialize an instance of type `T` from a string of TOON text.
///
/// # Examples
///
/// ```rust
/// use serde_toon::from_str;
/// use serde::Deserialize;
///
/// #[derive(Deserialize, PartialEq, Debug)]
/// struct Point { x: i32, y: i32 }
///
/// let toon = "x: 1\ny: 2";
/// let point: Point = from_str(toon).unwrap();
/// assert_eq!(point, Point { x: 1, y: 2 });
/// ```
///
/// # Errors
///
/// Returns an error if the input is not valid TOON format or cannot be deserialized to type `T`.
/// Error messages include line and column information.
#[must_use = "this returns the result of the operation, errors must be handled"]
pub fn from_str<'a, T>(s: &'a str) -> Result<T>
where
    T: Deserialize<'a>,
{
    let mut deserializer = Deserializer::from_str(s);
    T::deserialize(&mut deserializer)
}

/// Deserialize an instance of type `T` from a string of TOON text, using the
/// given [`DecodeOptions`].
///
/// # Examples
///
/// ```rust
/// use serde_toon::{from_str_with_options, DecodeOptions};
/// use serde::Deserialize;
///
/// #[derive(Deserialize, PartialEq, Debug)]
/// struct Point { x: i32, y: i32 }
///
/// let point: Point = from_str_with_options("x: 1\ny: 2", DecodeOptions::strict())?;
/// assert_eq!(point, Point { x: 1, y: 2 });
///
/// // Strict mode rejects duplicate keys.
/// let dup = from_str_with_options::<Point>("x: 1\nx: 2\ny: 3", DecodeOptions::strict());
/// assert!(dup.is_err());
/// # Ok::<(), serde_toon::Error>(())
/// ```
///
/// # Errors
///
/// Returns an error if the input is not valid TOON under the chosen options,
/// or cannot be deserialized to type `T`. Syntax errors carry line and column
/// information.
#[must_use = "this returns the result of the operation, errors must be handled"]
pub fn from_str_with_options<'a, T>(s: &'a str, options: DecodeOptions) -> Result<T>
where
    T: Deserialize<'a>,
{
    let mut deserializer = Deserializer::from_str_with_options(s, options);
    T::deserialize(&mut deserializer)
}

/// Deserialize an instance of type `T` from an I/O stream of TOON.
///
/// # Examples
///
/// ```rust
/// use serde_toon::from_reader;
/// use serde::Deserialize;
/// use std::io::Cursor;
///
/// #[derive(Deserialize, PartialEq, Debug)]
/// struct Point { x: i32, y: i32 }
///
/// let toon_bytes = b"x: 1\ny: 2";
/// let cursor = Cursor::new(toon_bytes);
/// let point: Point = from_reader(cursor).unwrap();
/// assert_eq!(point, Point { x: 1, y: 2 });
/// ```
///
/// # Errors
///
/// Returns an error if reading from the reader fails, the input is not valid TOON,
/// or the data cannot be deserialized to type `T`.
#[must_use = "this returns the result of the operation, errors must be handled"]
pub fn from_reader<R, T>(mut reader: R) -> Result<T>
where
    R: io::Read,
    T: for<'de> Deserialize<'de>,
{
    let mut string = String::new();
    reader
        .read_to_string(&mut string)
        .map_err(|e| Error::io(&e.to_string()))?;
    from_str(&string)
}

/// Deserialize an instance of type `T` from an I/O stream of TOON, using the
/// given [`DecodeOptions`].
///
/// # Examples
///
/// ```rust
/// use serde_toon::{from_reader_with_options, DecodeOptions};
/// use std::collections::BTreeMap;
///
/// let map: BTreeMap<String, i32> =
///     from_reader_with_options(&b"a: 1"[..], DecodeOptions::strict())?;
/// assert_eq!(map["a"], 1);
/// # Ok::<(), serde_toon::Error>(())
/// ```
///
/// # Errors
///
/// Returns an error if reading fails, the input is not valid TOON under the
/// chosen options, or it cannot be deserialized to type `T`.
#[must_use = "this returns the result of the operation, errors must be handled"]
pub fn from_reader_with_options<R, T>(mut reader: R, options: DecodeOptions) -> Result<T>
where
    R: io::Read,
    T: for<'de> Deserialize<'de>,
{
    let mut string = String::new();
    reader
        .read_to_string(&mut string)
        .map_err(|e| Error::io(&e.to_string()))?;
    from_str_with_options(&string, options)
}

/// Deserialize an instance of type `T` from bytes of TOON text.
///
/// # Examples
///
/// ```rust
/// use serde_toon::from_slice;
/// use serde::Deserialize;
///
/// #[derive(Deserialize, PartialEq, Debug)]
/// struct Point { x: i32, y: i32 }
///
/// let toon_bytes = b"x: 1\ny: 2";
/// let point: Point = from_slice(toon_bytes).unwrap();
/// assert_eq!(point, Point { x: 1, y: 2 });
/// ```
///
/// # Errors
///
/// Returns an error if the bytes are not valid UTF-8, not valid TOON format,
/// or cannot be deserialized to type `T`.
#[must_use = "this returns the result of the operation, errors must be handled"]
pub fn from_slice<'a, T>(v: &'a [u8]) -> Result<T>
where
    T: Deserialize<'a>,
{
    let s = std::str::from_utf8(v).map_err(|e| Error::custom(e.to_string()))?;
    from_str(s)
}

/// Deserialize an instance of type `T` from bytes of TOON text, using the
/// given [`DecodeOptions`].
///
/// # Examples
///
/// ```rust
/// use serde_toon::{from_slice_with_options, DecodeOptions};
/// use std::collections::BTreeMap;
///
/// let map: BTreeMap<String, bool> = from_slice_with_options(b"ok: true", DecodeOptions::strict())?;
/// assert!(map["ok"]);
/// # Ok::<(), serde_toon::Error>(())
/// ```
///
/// # Errors
///
/// Returns an error if the bytes are not valid UTF-8, not valid TOON under the
/// chosen options, or cannot be deserialized to type `T`.
#[must_use = "this returns the result of the operation, errors must be handled"]
pub fn from_slice_with_options<'a, T>(v: &'a [u8], options: DecodeOptions) -> Result<T>
where
    T: Deserialize<'a>,
{
    let s = std::str::from_utf8(v).map_err(|e| Error::custom(e.to_string()))?;
    from_str_with_options(s, options)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct Point {
        x: i32,
        y: i32,
    }

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct User {
        id: u32,
        name: String,
        active: bool,
        tags: Vec<String>,
    }

    #[test]
    fn test_serialize_deserialize_point() {
        let point = Point { x: 1, y: 2 };
        let toon = to_string(&point).unwrap();
        let point_back: Point = from_str(&toon).unwrap();
        assert_eq!(point, point_back);
    }

    #[test]
    #[ignore = "pending decoder rewrite"]
    fn test_serialize_deserialize_user() {
        let user = User {
            id: 123,
            name: "Alice".to_string(),
            active: true,
            tags: vec!["admin".to_string(), "user".to_string()],
        };

        let toon = to_string(&user).unwrap();
        let user_back: User = from_str(&toon).unwrap();
        assert_eq!(user, user_back);
    }

    #[test]
    #[ignore = "pending decoder rewrite"]
    fn test_pretty_printing() {
        let user = User {
            id: 123,
            name: "Alice".to_string(),
            active: true,
            tags: vec!["admin".to_string(), "user".to_string()],
        };

        let toon = to_string_pretty(&user).unwrap();
        let user_back: User = from_str(&toon).unwrap();
        assert_eq!(user, user_back);
    }

    #[test]
    fn test_to_value() {
        let point = Point { x: 1, y: 2 };
        let value = to_value(&point).unwrap();

        match value {
            Value::Object(obj) => {
                assert_eq!(obj.get("x"), Some(&Value::Number(Number::Integer(1))));
                assert_eq!(obj.get("y"), Some(&Value::Number(Number::Integer(2))));
            }
            _ => panic!("Expected object"),
        }
    }

    #[test]
    fn test_arrays() {
        let numbers = vec![1, 2, 3, 4, 5];
        let toon = to_string(&numbers).unwrap();
        let numbers_back: Vec<i32> = from_str(&toon).unwrap();
        assert_eq!(numbers, numbers_back);
    }

    #[test]
    #[ignore = "pending decoder rewrite"]
    #[allow(deprecated)] // the length marker is accepted and ignored
    fn test_custom_options() {
        let user = User {
            id: 123,
            name: "Alice".to_string(),
            active: true,
            tags: vec!["admin".to_string(), "user".to_string()],
        };

        let options = ToonOptions::new()
            .with_delimiter(Delimiter::Tab)
            .with_length_marker('#');

        let toon = to_string_with_options(&user, options).unwrap();
        let user_back: User = from_str(&toon).unwrap();
        assert_eq!(user, user_back);
    }
}
