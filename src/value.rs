//! Dynamic value representation for TOON data.
//!
//! This module provides the [`Value`] enum which represents any valid TOON value.
//! It's useful for working with TOON data when the structure isn't known at compile time.
//!
//! ## Core Types
//!
//! - [`Value`]: An enum representing any TOON value (null, bool, number, string, array, object, table, date, bigint)
//! - [`Number`]: Represents numeric values including special values (Infinity, -Infinity, NaN)
//!
//! ## Usage Patterns
//!
//! ### Creating Values
//!
//! ```rust
//! use serde_toon::{Value, Number};
//!
//! // From primitives
//! let null = Value::Null;
//! let boolean = Value::from(true);
//! let number = Value::from(42);
//! let text = Value::from("hello");
//!
//! // Using the toon! macro
//! use serde_toon::toon;
//! let obj = toon!({
//!     "name": "Alice",
//!     "age": 30
//! });
//! ```
//!
//! ### Type Checking
//!
//! ```rust
//! use serde_toon::Value;
//!
//! let value = Value::from(42);
//! assert!(value.is_number());
//! assert!(!value.is_string());
//! ```
//!
//! ### Extracting Values
//!
//! ```rust
//! use serde_toon::Value;
//! use std::convert::TryFrom;
//!
//! let value = Value::from(42);
//!
//! // Safe extraction with TryFrom
//! let num: i64 = i64::try_from(value).unwrap();
//! assert_eq!(num, 42);
//! ```
//!
//! ### Converting from Rust Types
//!
//! ```rust
//! use serde_toon::{to_value, Value};
//! use serde::Serialize;
//!
//! #[derive(Serialize)]
//! struct Point { x: i32, y: i32 }
//!
//! let point = Point { x: 10, y: 20 };
//! let value: Value = to_value(&point).unwrap();
//!
//! if let Value::Object(obj) = value {
//!     assert_eq!(obj.len(), 2);
//! }
//! ```

use crate::ToonMap;
use chrono::{DateTime, Utc};
use num_bigint::BigInt;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// A dynamically-typed representation of any valid TOON value.
///
/// This enum can represent any TOON value including primitives, collections,
/// and TOON-specific types like tables and dates. It's particularly useful when:
///
/// - The structure isn't known at compile time
/// - You need to manipulate TOON data generically
/// - Building TOON structures programmatically
///
/// # Examples
///
/// ```rust
/// use serde_toon::{Value, Number};
///
/// // Create different value types
/// let null = Value::Null;
/// let num = Value::Number(Number::Integer(42));
/// let text = Value::String("hello".to_string());
///
/// // Check types
/// assert!(null.is_null());
/// assert!(num.is_number());
/// assert!(text.is_string());
/// ```
#[derive(Clone, Debug, PartialEq, Default)]
pub enum Value {
    #[default]
    Null,
    Bool(bool),
    Number(Number),
    String(String),
    Array(Vec<Value>),
    Object(ToonMap),
    Table {
        headers: Vec<String>,
        rows: Vec<Vec<Value>>,
    },
    Date(DateTime<Utc>),
    BigInt(BigInt),
}

/// A numeric value that can be an integer, float, or JavaScript-style special value.
///
/// TOON supports all standard numeric types plus JavaScript's special numeric values
/// (Infinity, -Infinity, NaN) for compatibility with LLM outputs that may include these.
///
/// # Examples
///
/// ```rust
/// use serde_toon::Number;
///
/// let integer = Number::Integer(42);
/// let float = Number::Float(3.5);
/// let infinity = Number::Infinity;
///
/// assert!(integer.is_integer());
/// assert_eq!(integer.as_i64(), Some(42));
/// assert_eq!(float.as_f64(), 3.5);
/// assert!(infinity.is_special());
/// ```
#[derive(Clone, Debug, PartialEq)]
pub enum Number {
    Integer(i64),
    Float(f64),
    Infinity,
    NegativeInfinity,
    NaN,
}

impl Number {
    /// Returns `true` if this is an integer value.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use serde_toon::Number;
    ///
    /// assert!(Number::Integer(42).is_integer());
    /// assert!(!Number::Float(3.5).is_integer());
    /// ```
    #[inline]
    #[must_use]
    pub const fn is_integer(&self) -> bool {
        matches!(self, Number::Integer(_))
    }

    /// Returns `true` if this is a floating-point value.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use serde_toon::Number;
    ///
    /// assert!(Number::Float(3.5).is_float());
    /// assert!(!Number::Integer(42).is_float());
    /// ```
    #[inline]
    #[must_use]
    pub const fn is_float(&self) -> bool {
        matches!(self, Number::Float(_))
    }

    /// Returns `true` if this is a special value (Infinity, -Infinity, or NaN).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use serde_toon::Number;
    ///
    /// assert!(Number::Infinity.is_special());
    /// assert!(Number::NaN.is_special());
    /// assert!(!Number::Integer(42).is_special());
    /// ```
    #[inline]
    #[must_use]
    pub const fn is_special(&self) -> bool {
        matches!(
            self,
            Number::Infinity | Number::NegativeInfinity | Number::NaN
        )
    }

    /// Converts this number to an `i64` if possible.
    ///
    /// Returns `Some(i64)` for integers and floats with no fractional part
    /// that fit in i64 range. Returns `None` for special values and
    /// out-of-range floats.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use serde_toon::Number;
    ///
    /// assert_eq!(Number::Integer(42).as_i64(), Some(42));
    /// assert_eq!(Number::Float(42.0).as_i64(), Some(42));
    /// assert_eq!(Number::Float(42.5).as_i64(), None);
    /// assert_eq!(Number::Infinity.as_i64(), None);
    /// ```
    #[inline]
    #[must_use]
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Number::Integer(i) => Some(*i),
            Number::Float(f) => {
                if f.fract() == 0.0 && *f >= i64::MIN as f64 && *f <= i64::MAX as f64 {
                    Some(*f as i64)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Converts this number to an `f64`.
    ///
    /// Always succeeds, converting integers and special values to their
    /// corresponding f64 representations.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use serde_toon::Number;
    ///
    /// assert_eq!(Number::Integer(42).as_f64(), 42.0);
    /// assert_eq!(Number::Float(3.5).as_f64(), 3.5);
    /// assert_eq!(Number::Infinity.as_f64(), f64::INFINITY);
    /// ```
    #[inline]
    #[must_use]
    pub fn as_f64(&self) -> f64 {
        match self {
            Number::Integer(i) => *i as f64,
            Number::Float(f) => *f,
            Number::Infinity => f64::INFINITY,
            Number::NegativeInfinity => f64::NEG_INFINITY,
            Number::NaN => f64::NAN,
        }
    }
}

impl fmt::Display for Number {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Number::Integer(i) => write!(f, "{}", i),
            Number::Float(fl) => write!(f, "{}", fl),
            Number::Infinity => write!(f, "Infinity"),
            Number::NegativeInfinity => write!(f, "-Infinity"),
            Number::NaN => write!(f, "NaN"),
        }
    }
}

impl From<i8> for Number {
    fn from(value: i8) -> Self {
        Number::Integer(value as i64)
    }
}

impl From<i16> for Number {
    fn from(value: i16) -> Self {
        Number::Integer(value as i64)
    }
}

impl From<i32> for Number {
    fn from(value: i32) -> Self {
        Number::Integer(value as i64)
    }
}

impl From<i64> for Number {
    fn from(value: i64) -> Self {
        Number::Integer(value)
    }
}

impl From<u8> for Number {
    fn from(value: u8) -> Self {
        Number::Integer(value as i64)
    }
}

impl From<u16> for Number {
    fn from(value: u16) -> Self {
        Number::Integer(value as i64)
    }
}

impl From<u32> for Number {
    fn from(value: u32) -> Self {
        Number::Integer(value as i64)
    }
}

impl From<f32> for Number {
    fn from(value: f32) -> Self {
        Number::Float(value as f64)
    }
}

impl From<f64> for Number {
    fn from(value: f64) -> Self {
        Number::Float(value)
    }
}

impl Value {
    /// Returns `true` if the value is null.
    #[inline]
    #[must_use]
    pub const fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    /// Returns `true` if the value is a boolean.
    #[inline]
    #[must_use]
    pub const fn is_bool(&self) -> bool {
        matches!(self, Value::Bool(_))
    }

    /// Returns `true` if the value is a number.
    #[inline]
    #[must_use]
    pub const fn is_number(&self) -> bool {
        matches!(self, Value::Number(_))
    }

    /// Returns `true` if the value is a string.
    #[inline]
    #[must_use]
    pub const fn is_string(&self) -> bool {
        matches!(self, Value::String(_))
    }

    /// Returns `true` if the value is an array.
    #[inline]
    #[must_use]
    pub const fn is_array(&self) -> bool {
        matches!(self, Value::Array(_))
    }

    /// Returns `true` if the value is an object.
    #[inline]
    #[must_use]
    pub const fn is_object(&self) -> bool {
        matches!(self, Value::Object(_))
    }

    /// Returns `true` if the value is a table.
    #[inline]
    #[must_use]
    pub const fn is_table(&self) -> bool {
        matches!(self, Value::Table { .. })
    }

    /// Returns `true` if the value is a date.
    #[inline]
    #[must_use]
    pub const fn is_date(&self) -> bool {
        matches!(self, Value::Date(_))
    }

    /// Returns `true` if the value is a big integer.
    #[inline]
    #[must_use]
    pub const fn is_bigint(&self) -> bool {
        matches!(self, Value::BigInt(_))
    }

    /// If the value is a boolean, returns it. Otherwise returns `None`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use serde_toon::Value;
    ///
    /// assert_eq!(Value::Bool(true).as_bool(), Some(true));
    /// assert_eq!(Value::from(42).as_bool(), None);
    /// ```
    #[inline]
    #[must_use]
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// If the value is a string, returns a reference to it. Otherwise returns `None`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use serde_toon::Value;
    ///
    /// assert_eq!(Value::from("hello").as_str(), Some("hello"));
    /// assert_eq!(Value::from(42).as_str(), None);
    /// ```
    #[inline]
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    /// If the value is an i64 integer or a whole-number float, returns it. Otherwise returns `None`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use serde_toon::{Value, Number};
    ///
    /// assert_eq!(Value::Number(Number::Integer(42)).as_i64(), Some(42));
    /// assert_eq!(Value::Number(Number::Float(42.0)).as_i64(), Some(42));
    /// assert_eq!(Value::Number(Number::Float(42.5)).as_i64(), None);
    /// ```
    #[inline]
    #[must_use]
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Number(n) => n.as_i64(),
            _ => None,
        }
    }

    /// If the value is an array, returns a reference to it. Otherwise returns `None`.
    #[inline]
    #[must_use]
    pub fn as_array(&self) -> Option<&Vec<Value>> {
        match self {
            Value::Array(arr) => Some(arr),
            _ => None,
        }
    }

    /// If the value is an object, returns a reference to it. Otherwise returns `None`.
    #[inline]
    #[must_use]
    pub fn as_object(&self) -> Option<&ToonMap> {
        match self {
            Value::Object(obj) => Some(obj),
            _ => None,
        }
    }

    /// If the value is a date, returns a reference to it. Otherwise returns `None`.
    #[inline]
    #[must_use]
    pub fn as_date(&self) -> Option<&DateTime<Utc>> {
        match self {
            Value::Date(dt) => Some(dt),
            _ => None,
        }
    }

    /// If the value is a big integer, returns a reference to it. Otherwise returns `None`.
    #[inline]
    #[must_use]
    pub fn as_bigint(&self) -> Option<&BigInt> {
        match self {
            Value::BigInt(bi) => Some(bi),
            _ => None,
        }
    }

    #[inline]
    pub fn needs_quotes(&self) -> bool {
        match self {
            Value::String(s) => {
                s.is_empty()
                    || s.contains(':')
                    || s.contains(',')
                    || s.contains('\n')
                    || s.contains('\t')
                    || s.contains('|')
                    || s.starts_with(' ')
                    || s.ends_with(' ')
                    || s == "true"
                    || s == "false"
                    || s == "null"
                    || s.parse::<f64>().is_ok()
            }
            _ => false,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Null => write!(f, "null"),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Number(n) => write!(f, "{}", n),
            Value::String(s) => {
                if self.needs_quotes() {
                    write!(f, "\"{}\"", s.replace('"', "\\\""))
                } else {
                    write!(f, "{}", s)
                }
            }
            Value::Array(arr) => {
                write!(
                    f,
                    "[{}]",
                    arr.iter()
                        .map(|v| v.to_string())
                        .collect::<Vec<_>>()
                        .join(",")
                )
            }
            Value::Object(_) => write!(f, "{{object}}"),
            Value::Table { headers, rows } => {
                write!(f, "Table[{}]{{{}}}", rows.len(), headers.join(","))
            }
            Value::Date(dt) => write!(f, "{}", dt.to_rfc3339()),
            Value::BigInt(bi) => write!(f, "{}n", bi),
        }
    }
}

impl Serialize for Value {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Value::Null => serializer.serialize_unit(),
            Value::Bool(b) => serializer.serialize_bool(*b),
            Value::Number(Number::Integer(i)) => serializer.serialize_i64(*i),
            Value::Number(Number::Float(f)) => serializer.serialize_f64(*f),
            Value::Number(Number::Infinity) => serializer.serialize_f64(f64::INFINITY),
            Value::Number(Number::NegativeInfinity) => serializer.serialize_f64(f64::NEG_INFINITY),
            Value::Number(Number::NaN) => serializer.serialize_f64(f64::NAN),
            Value::String(s) => serializer.serialize_str(s),
            Value::Array(arr) => {
                use serde::ser::SerializeSeq;
                let mut seq = serializer.serialize_seq(Some(arr.len()))?;
                for element in arr {
                    seq.serialize_element(element)?;
                }
                seq.end()
            }
            Value::Object(obj) => {
                use serde::ser::SerializeMap;
                let mut map = serializer.serialize_map(Some(obj.len()))?;
                for (k, v) in obj.iter() {
                    map.serialize_entry(k, v)?;
                }
                map.end()
            }
            Value::Table { headers, rows } => {
                use serde::ser::SerializeSeq;
                let mut seq = serializer.serialize_seq(Some(rows.len()))?;
                for row in rows {
                    let mut object = ToonMap::new();
                    for (i, value) in row.iter().enumerate() {
                        if let Some(header) = headers.get(i) {
                            object.insert(header.clone(), value.clone());
                        }
                    }
                    seq.serialize_element(&Value::Object(object))?;
                }
                seq.end()
            }
            Value::Date(dt) => serializer.serialize_str(&dt.to_rfc3339()),
            Value::BigInt(bi) => serializer.serialize_str(&format!("{}n", bi)),
        }
    }
}

impl<'de> Deserialize<'de> for Value {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::{self, Visitor};

        struct ValueVisitor;

        impl<'de> Visitor<'de> for ValueVisitor {
            type Value = Value;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("any valid TOON value")
            }

            fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
                Ok(Value::Bool(value))
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
                Ok(Value::Number(Number::Integer(value)))
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
                if value <= i64::MAX as u64 {
                    Ok(Value::Number(Number::Integer(value as i64)))
                } else {
                    Ok(Value::Number(Number::Float(value as f64)))
                }
            }

            fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E> {
                Ok(Value::Number(Number::Float(value)))
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
                Ok(Value::String(value.to_string()))
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
                Ok(Value::String(value))
            }

            fn visit_unit<E>(self) -> Result<Self::Value, E> {
                Ok(Value::Null)
            }

            fn visit_none<E>(self) -> Result<Self::Value, E> {
                Ok(Value::Null)
            }

            fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
            where
                D: Deserializer<'de>,
            {
                Deserialize::deserialize(deserializer)
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: de::SeqAccess<'de>,
            {
                let mut vec = Vec::new();
                while let Some(elem) = seq.next_element()? {
                    vec.push(elem);
                }
                Ok(Value::Array(vec))
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: de::MapAccess<'de>,
            {
                let mut values = ToonMap::new();
                while let Some((key, value)) = map.next_entry()? {
                    values.insert(key, value);
                }
                Ok(Value::Object(values))
            }
        }

        deserializer.deserialize_any(ValueVisitor)
    }
}

// TryFrom implementations for extracting values from Value
impl TryFrom<Value> for i64 {
    type Error = crate::Error;

    fn try_from(value: Value) -> crate::Result<Self> {
        match value {
            Value::Number(Number::Integer(i)) => Ok(i),
            Value::Number(Number::Float(f)) => {
                if f.fract() == 0.0 && f >= i64::MIN as f64 && f <= i64::MAX as f64 {
                    Ok(f as i64)
                } else {
                    Err(crate::Error::custom(format!(
                        "cannot convert float {} to i64",
                        f
                    )))
                }
            }
            _ => Err(crate::Error::custom(format!(
                "expected integer, found {:?}",
                value
            ))),
        }
    }
}

impl TryFrom<Value> for f64 {
    type Error = crate::Error;

    fn try_from(value: Value) -> crate::Result<Self> {
        match value {
            Value::Number(Number::Integer(i)) => Ok(i as f64),
            Value::Number(Number::Float(f)) => Ok(f),
            Value::Number(Number::Infinity) => Ok(f64::INFINITY),
            Value::Number(Number::NegativeInfinity) => Ok(f64::NEG_INFINITY),
            Value::Number(Number::NaN) => Ok(f64::NAN),
            _ => Err(crate::Error::custom(format!(
                "expected number, found {:?}",
                value
            ))),
        }
    }
}

impl TryFrom<Value> for bool {
    type Error = crate::Error;

    fn try_from(value: Value) -> crate::Result<Self> {
        match value {
            Value::Bool(b) => Ok(b),
            _ => Err(crate::Error::custom(format!(
                "expected bool, found {:?}",
                value
            ))),
        }
    }
}

impl TryFrom<Value> for String {
    type Error = crate::Error;

    fn try_from(value: Value) -> crate::Result<Self> {
        match value {
            Value::String(s) => Ok(s),
            _ => Err(crate::Error::custom(format!(
                "expected string, found {:?}",
                value
            ))),
        }
    }
}

// From implementations for creating Value from primitives
impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Value::Bool(value)
    }
}

impl From<i8> for Value {
    fn from(value: i8) -> Self {
        Value::Number(Number::Integer(value as i64))
    }
}

impl From<i16> for Value {
    fn from(value: i16) -> Self {
        Value::Number(Number::Integer(value as i64))
    }
}

impl From<i32> for Value {
    fn from(value: i32) -> Self {
        Value::Number(Number::Integer(value as i64))
    }
}

impl From<i64> for Value {
    fn from(value: i64) -> Self {
        Value::Number(Number::Integer(value))
    }
}

impl From<u8> for Value {
    fn from(value: u8) -> Self {
        Value::Number(Number::Integer(value as i64))
    }
}

impl From<u16> for Value {
    fn from(value: u16) -> Self {
        Value::Number(Number::Integer(value as i64))
    }
}

impl From<u32> for Value {
    fn from(value: u32) -> Self {
        Value::Number(Number::Integer(value as i64))
    }
}

impl From<f32> for Value {
    fn from(value: f32) -> Self {
        Value::Number(Number::Float(value as f64))
    }
}

impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Value::Number(Number::Float(value))
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Value::String(value)
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Value::String(value.to_string())
    }
}

impl From<Vec<Value>> for Value {
    fn from(value: Vec<Value>) -> Self {
        Value::Array(value)
    }
}

impl From<ToonMap> for Value {
    fn from(value: ToonMap) -> Self {
        Value::Object(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::TryFrom;

    #[test]
    fn test_tryfrom_i64() {
        let value = Value::Number(Number::Integer(42));
        let result: i64 = TryFrom::try_from(value).unwrap();
        assert_eq!(result, 42);

        let value = Value::Number(Number::Float(42.0));
        let result: i64 = TryFrom::try_from(value).unwrap();
        assert_eq!(result, 42);

        let value = Value::String("test".to_string());
        assert!(i64::try_from(value).is_err());
    }

    #[test]
    fn test_tryfrom_f64() {
        let value = Value::Number(Number::Float(3.5));
        let result: f64 = TryFrom::try_from(value).unwrap();
        assert_eq!(result, 3.5);

        let value = Value::Number(Number::Integer(42));
        let result: f64 = TryFrom::try_from(value).unwrap();
        assert_eq!(result, 42.0);

        let value = Value::Number(Number::Infinity);
        let result: f64 = TryFrom::try_from(value).unwrap();
        assert_eq!(result, f64::INFINITY);
    }

    #[test]
    fn test_tryfrom_bool() {
        let value = Value::Bool(true);
        let result: bool = TryFrom::try_from(value).unwrap();
        assert!(result);

        let value = Value::Number(Number::Integer(1));
        assert!(bool::try_from(value).is_err());
    }

    #[test]
    fn test_tryfrom_string() {
        let value = Value::String("hello".to_string());
        let result: String = TryFrom::try_from(value).unwrap();
        assert_eq!(result, "hello");

        let value = Value::Number(Number::Integer(42));
        assert!(String::try_from(value).is_err());
    }

    #[test]
    fn test_from_primitives() {
        assert_eq!(Value::from(true), Value::Bool(true));
        assert_eq!(Value::from(42i32), Value::Number(Number::Integer(42)));
        assert_eq!(Value::from(42i64), Value::Number(Number::Integer(42)));
        assert_eq!(Value::from(3.5f64), Value::Number(Number::Float(3.5)));
        assert_eq!(Value::from("test"), Value::String("test".to_string()));
        assert_eq!(
            Value::from("test".to_string()),
            Value::String("test".to_string())
        );
    }

    #[test]
    fn test_from_collections() {
        let vec = vec![Value::from(1i32), Value::from(2i32)];
        let value = Value::from(vec.clone());
        assert_eq!(value, Value::Array(vec));

        let mut map = ToonMap::new();
        map.insert("key".to_string(), Value::from(42i32));
        let value = Value::from(map.clone());
        assert_eq!(value, Value::Object(map));
    }

    #[test]
    fn test_const_is_methods() {
        const fn check_null(v: &Value) -> bool {
            v.is_null()
        }

        let null_value = Value::Null;
        assert!(check_null(&null_value));
    }

    #[test]
    fn test_inline_methods() {
        let num = Number::Integer(42);
        assert!(num.is_integer());
        assert!(!num.is_float());
        assert!(!num.is_special());
        assert_eq!(num.as_i64(), Some(42));
        assert_eq!(num.as_f64(), 42.0);

        let value = Value::Number(Number::Integer(42));
        assert!(value.is_number());
        assert!(!value.is_null());
        assert!(!value.is_string());
    }
}
