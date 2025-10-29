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
//! serde_toon = "0.1"
//! serde = { version = "1.0", features = ["derive"] }
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
//! // Output: "id: 123\nname: Alice\nactive: true"
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
//! // Output: "[2]{id,name,price}:\n  1,Widget,9.99\n  2,Gadget,14.99"
//! ```
//!
//! ### Dynamic Values with toon! Macro
//!
//! ```rust
//! use serde_toon::{toon, ToonValue};
//!
//! let data = toon!({
//!     "name": "Alice",
//!     "age": 30,
//!     "tags": ["rust", "serde", "llm"]
//! });
//!
//! if let ToonValue::Object(obj) = data {
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
//! For the complete TOON format specification, see:
//! <https://github.com/johannschopplich/toon>
//!
//! ## Examples
//!
//! See the `examples/` directory for focused, production-ready examples:
//!
//! - **`simple.rs`** - Your first TOON experience (basic serialization)
//! - **`macro.rs`** - Building values with the toon! macro
//! - **`tabular_arrays.rs`** - TOON's killer feature for repeated structures
//! - **`dynamic_values.rs`** - Working with ToonValue dynamically
//! - **`custom_options.rs`** - Customizing delimiters and formatting
//! - **`token_efficiency.rs`** - TOON vs JSON comparison
//!
//! Run any example with: `cargo run --example <name>`

pub mod de;
pub mod error;
pub mod macros;
pub mod map;
pub mod options;
pub mod ser;
pub mod value;

pub use de::Deserializer;
pub use error::{Error, Result};
pub use map::ToonMap;
pub use options::{Delimiter, ToonOptions};
pub use ser::{Serializer, ToonValueSerializer};
pub use value::{Number, ToonValue};

use serde::{Deserialize, Serialize};
use std::io;

/// Serialize any `T: Serialize` to a TOON string.
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
/// let toon = to_string(&point).unwrap();
/// ```
///
/// # Errors
///
/// Returns an error if the value cannot be serialized (e.g., unsupported types).
#[must_use = "this returns the result of the operation, errors must be handled"]
pub fn to_string<T>(value: &T) -> Result<String>
where
    T: ?Sized + Serialize,
{
    to_string_with_options(value, ToonOptions::default())
}

/// Serialize any `T: Serialize` to a pretty-printed TOON string.
///
/// Pretty-printing adds newlines and indentation for readability.
///
/// # Examples
///
/// ```rust
/// use serde_toon::to_string_pretty;
/// use serde::Serialize;
///
/// #[derive(Serialize)]
/// struct Point { x: i32, y: i32 }
///
/// let point = Point { x: 1, y: 2 };
/// let toon = to_string_pretty(&point).unwrap();
/// ```
///
/// # Errors
///
/// Returns an error if the value cannot be serialized.
#[must_use = "this returns the result of the operation, errors must be handled"]
pub fn to_string_pretty<T>(value: &T) -> Result<String>
where
    T: ?Sized + Serialize,
{
    to_string_with_options(value, ToonOptions::pretty())
}

/// Serialize any `T: Serialize` to a TOON string with custom options.
///
/// Allows customization of delimiters, indentation, and length markers.
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
/// let point = Point { x: 1, y: 2 };
/// let options = ToonOptions::new()
///     .with_delimiter(Delimiter::Tab)
///     .with_length_marker('#');
/// let toon = to_string_with_options(&point, options).unwrap();
/// ```
///
/// # Errors
///
/// Returns an error if the value cannot be serialized.
#[must_use = "this returns the result of the operation, errors must be handled"]
pub fn to_string_with_options<T>(value: &T, options: ToonOptions) -> Result<String>
where
    T: ?Sized + Serialize,
{
    let mut serializer = Serializer::new(options);
    value.serialize(&mut serializer)?;
    Ok(serializer.into_inner())
}

/// Convert any `T: Serialize` to a `ToonValue`.
///
/// Useful for working with TOON data dynamically when the structure isn't known at compile time.
///
/// # Examples
///
/// ```rust
/// use serde_toon::{to_value, ToonValue};
/// use serde::Serialize;
///
/// #[derive(Serialize)]
/// struct Point { x: i32, y: i32 }
///
/// let point = Point { x: 1, y: 2 };
/// let value: ToonValue = to_value(&point).unwrap();
/// assert!(value.is_object());
/// ```
///
/// # Errors
///
/// Returns an error if the value cannot be serialized.
#[must_use = "this returns the result of the operation, errors must be handled"]
pub fn to_value<T>(value: &T) -> Result<ToonValue>
where
    T: ?Sized + Serialize,
{
    value.serialize(crate::ser::ToonValueSerializer)
}

/// Serialize any `T: Serialize` to a writer in TOON format.
///
/// # Examples
///
/// ```rust
/// use serde_toon::to_writer;
/// use serde::Serialize;
/// use std::io::Cursor;
///
/// #[derive(Serialize)]
/// struct Point { x: i32, y: i32 }
///
/// let point = Point { x: 1, y: 2 };
/// let mut buffer = Vec::new();
/// to_writer(&mut buffer, &point).unwrap();
/// ```
///
/// # Errors
///
/// Returns an error if serialization fails or writing to the writer fails.
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
/// # Errors
///
/// Returns an error if serialization fails or writing to the writer fails.
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
            ToonValue::Object(obj) => {
                assert_eq!(obj.get("x"), Some(&ToonValue::Number(Number::Integer(1))));
                assert_eq!(obj.get("y"), Some(&ToonValue::Number(Number::Integer(2))));
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
