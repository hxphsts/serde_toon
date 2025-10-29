//! Dynamic value representation for TOON data.
//!
//! This module provides the [`ToonValue`] enum which represents any valid TOON value.
//! It's useful for working with TOON data when the structure isn't known at compile time.
//!
//! ## Core Types
//!
//! - [`ToonValue`]: An enum representing any TOON value (null, bool, number, string, array, object, table, date, bigint)
//! - [`Number`]: Represents numeric values including special values (Infinity, -Infinity, NaN)
//!
//! ## Usage Patterns
//!
//! ### Creating Values
//!
//! ```rust
//! use serde_toon::{ToonValue, Number};
//!
//! // From primitives
//! let null = ToonValue::Null;
//! let boolean = ToonValue::from(true);
//! let number = ToonValue::from(42);
//! let text = ToonValue::from("hello");
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
//! use serde_toon::ToonValue;
//!
//! let value = ToonValue::from(42);
//! assert!(value.is_number());
//! assert!(!value.is_string());
//! ```
//!
//! ### Extracting Values
//!
//! ```rust
//! use serde_toon::ToonValue;
//! use std::convert::TryFrom;
//!
//! let value = ToonValue::from(42);
//!
//! // Safe extraction with TryFrom
//! let num: i64 = i64::try_from(value).unwrap();
//! assert_eq!(num, 42);
//! ```
//!
//! ### Converting from Rust Types
//!
//! ```rust
//! use serde_toon::{to_value, ToonValue};
//! use serde::Serialize;
//!
//! #[derive(Serialize)]
//! struct Point { x: i32, y: i32 }
//!
//! let point = Point { x: 10, y: 20 };
//! let value: ToonValue = to_value(&point).unwrap();
//!
//! if let ToonValue::Object(obj) = value {
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
/// use serde_toon::{ToonValue, Number};
///
/// // Create different value types
/// let null = ToonValue::Null;
/// let num = ToonValue::Number(Number::Integer(42));
/// let text = ToonValue::String("hello".to_string());
///
/// // Check types
/// assert!(null.is_null());
/// assert!(num.is_number());
/// assert!(text.is_string());
/// ```
#[derive(Clone, Debug, PartialEq, Default)]
pub enum ToonValue {
    #[default]
    Null,
    Bool(bool),
    Number(Number),
    String(String),
    Array(Vec<ToonValue>),
    Object(ToonMap),
    Table {
        headers: Vec<String>,
        rows: Vec<Vec<ToonValue>>,
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

impl ToonValue {
    /// Returns `true` if the value is null.
    #[inline]
    #[must_use]
    pub const fn is_null(&self) -> bool {
        matches!(self, ToonValue::Null)
    }

    /// Returns `true` if the value is a boolean.
    #[inline]
    #[must_use]
    pub const fn is_bool(&self) -> bool {
        matches!(self, ToonValue::Bool(_))
    }

    /// Returns `true` if the value is a number.
    #[inline]
    #[must_use]
    pub const fn is_number(&self) -> bool {
        matches!(self, ToonValue::Number(_))
    }

    /// Returns `true` if the value is a string.
    #[inline]
    #[must_use]
    pub const fn is_string(&self) -> bool {
        matches!(self, ToonValue::String(_))
    }

    /// Returns `true` if the value is an array.
    #[inline]
    #[must_use]
    pub const fn is_array(&self) -> bool {
        matches!(self, ToonValue::Array(_))
    }

    /// Returns `true` if the value is an object.
    #[inline]
    #[must_use]
    pub const fn is_object(&self) -> bool {
        matches!(self, ToonValue::Object(_))
    }

    /// Returns `true` if the value is a table.
    #[inline]
    #[must_use]
    pub const fn is_table(&self) -> bool {
        matches!(self, ToonValue::Table { .. })
    }

    /// Returns `true` if the value is a date.
    #[inline]
    #[must_use]
    pub const fn is_date(&self) -> bool {
        matches!(self, ToonValue::Date(_))
    }

    /// Returns `true` if the value is a big integer.
    #[inline]
    #[must_use]
    pub const fn is_bigint(&self) -> bool {
        matches!(self, ToonValue::BigInt(_))
    }

    /// If the value is a boolean, returns it. Otherwise returns `None`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use serde_toon::ToonValue;
    ///
    /// assert_eq!(ToonValue::Bool(true).as_bool(), Some(true));
    /// assert_eq!(ToonValue::from(42).as_bool(), None);
    /// ```
    #[inline]
    #[must_use]
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            ToonValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// If the value is a string, returns a reference to it. Otherwise returns `None`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use serde_toon::ToonValue;
    ///
    /// assert_eq!(ToonValue::from("hello").as_str(), Some("hello"));
    /// assert_eq!(ToonValue::from(42).as_str(), None);
    /// ```
    #[inline]
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            ToonValue::String(s) => Some(s),
            _ => None,
        }
    }

    /// If the value is an i64 integer or a whole-number float, returns it. Otherwise returns `None`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use serde_toon::{ToonValue, Number};
    ///
    /// assert_eq!(ToonValue::Number(Number::Integer(42)).as_i64(), Some(42));
    /// assert_eq!(ToonValue::Number(Number::Float(42.0)).as_i64(), Some(42));
    /// assert_eq!(ToonValue::Number(Number::Float(42.5)).as_i64(), None);
    /// ```
    #[inline]
    #[must_use]
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            ToonValue::Number(n) => n.as_i64(),
            _ => None,
        }
    }

    /// If the value is an array, returns a reference to it. Otherwise returns `None`.
    #[inline]
    #[must_use]
    pub fn as_array(&self) -> Option<&Vec<ToonValue>> {
        match self {
            ToonValue::Array(arr) => Some(arr),
            _ => None,
        }
    }

    /// If the value is an object, returns a reference to it. Otherwise returns `None`.
    #[inline]
    #[must_use]
    pub fn as_object(&self) -> Option<&ToonMap> {
        match self {
            ToonValue::Object(obj) => Some(obj),
            _ => None,
        }
    }

    /// If the value is a date, returns a reference to it. Otherwise returns `None`.
    #[inline]
    #[must_use]
    pub fn as_date(&self) -> Option<&DateTime<Utc>> {
        match self {
            ToonValue::Date(dt) => Some(dt),
            _ => None,
        }
    }

    /// If the value is a big integer, returns a reference to it. Otherwise returns `None`.
    #[inline]
    #[must_use]
    pub fn as_bigint(&self) -> Option<&BigInt> {
        match self {
            ToonValue::BigInt(bi) => Some(bi),
            _ => None,
        }
    }

    #[inline]
    pub fn needs_quotes(&self) -> bool {
        match self {
            ToonValue::String(s) => {
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

impl fmt::Display for ToonValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ToonValue::Null => write!(f, "null"),
            ToonValue::Bool(b) => write!(f, "{}", b),
            ToonValue::Number(n) => write!(f, "{}", n),
            ToonValue::String(s) => {
                if self.needs_quotes() {
                    write!(f, "\"{}\"", s.replace('"', "\\\""))
                } else {
                    write!(f, "{}", s)
                }
            }
            ToonValue::Array(arr) => {
                write!(
                    f,
                    "[{}]",
                    arr.iter()
                        .map(|v| v.to_string())
                        .collect::<Vec<_>>()
                        .join(",")
                )
            }
            ToonValue::Object(_) => write!(f, "{{object}}"),
            ToonValue::Table { headers, rows } => {
                write!(f, "Table[{}]{{{}}}", rows.len(), headers.join(","))
            }
            ToonValue::Date(dt) => write!(f, "{}", dt.to_rfc3339()),
            ToonValue::BigInt(bi) => write!(f, "{}n", bi),
        }
    }
}

impl Serialize for ToonValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            ToonValue::Null => serializer.serialize_unit(),
            ToonValue::Bool(b) => serializer.serialize_bool(*b),
            ToonValue::Number(Number::Integer(i)) => serializer.serialize_i64(*i),
            ToonValue::Number(Number::Float(f)) => serializer.serialize_f64(*f),
            ToonValue::Number(Number::Infinity) => serializer.serialize_f64(f64::INFINITY),
            ToonValue::Number(Number::NegativeInfinity) => {
                serializer.serialize_f64(f64::NEG_INFINITY)
            }
            ToonValue::Number(Number::NaN) => serializer.serialize_f64(f64::NAN),
            ToonValue::String(s) => serializer.serialize_str(s),
            ToonValue::Array(arr) => {
                use serde::ser::SerializeSeq;
                let mut seq = serializer.serialize_seq(Some(arr.len()))?;
                for element in arr {
                    seq.serialize_element(element)?;
                }
                seq.end()
            }
            ToonValue::Object(obj) => {
                use serde::ser::SerializeMap;
                let mut map = serializer.serialize_map(Some(obj.len()))?;
                for (k, v) in obj.iter() {
                    map.serialize_entry(k, v)?;
                }
                map.end()
            }
            ToonValue::Table { headers, rows } => {
                use serde::ser::SerializeSeq;
                let mut seq = serializer.serialize_seq(Some(rows.len()))?;
                for row in rows {
                    let mut object = ToonMap::new();
                    for (i, value) in row.iter().enumerate() {
                        if let Some(header) = headers.get(i) {
                            object.insert(header.clone(), value.clone());
                        }
                    }
                    seq.serialize_element(&ToonValue::Object(object))?;
                }
                seq.end()
            }
            ToonValue::Date(dt) => serializer.serialize_str(&dt.to_rfc3339()),
            ToonValue::BigInt(bi) => serializer.serialize_str(&format!("{}n", bi)),
        }
    }
}

impl<'de> Deserialize<'de> for ToonValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::{self, Visitor};

        struct ToonValueVisitor;

        impl<'de> Visitor<'de> for ToonValueVisitor {
            type Value = ToonValue;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("any valid TOON value")
            }

            fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
                Ok(ToonValue::Bool(value))
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
                Ok(ToonValue::Number(Number::Integer(value)))
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
                if value <= i64::MAX as u64 {
                    Ok(ToonValue::Number(Number::Integer(value as i64)))
                } else {
                    Ok(ToonValue::Number(Number::Float(value as f64)))
                }
            }

            fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E> {
                Ok(ToonValue::Number(Number::Float(value)))
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
                Ok(ToonValue::String(value.to_string()))
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
                Ok(ToonValue::String(value))
            }

            fn visit_unit<E>(self) -> Result<Self::Value, E> {
                Ok(ToonValue::Null)
            }

            fn visit_none<E>(self) -> Result<Self::Value, E> {
                Ok(ToonValue::Null)
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
                Ok(ToonValue::Array(vec))
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: de::MapAccess<'de>,
            {
                let mut values = ToonMap::new();
                while let Some((key, value)) = map.next_entry()? {
                    values.insert(key, value);
                }
                Ok(ToonValue::Object(values))
            }
        }

        deserializer.deserialize_any(ToonValueVisitor)
    }
}

// TryFrom implementations for extracting values from ToonValue
impl TryFrom<ToonValue> for i64 {
    type Error = crate::Error;

    fn try_from(value: ToonValue) -> crate::Result<Self> {
        match value {
            ToonValue::Number(Number::Integer(i)) => Ok(i),
            ToonValue::Number(Number::Float(f)) => {
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

impl TryFrom<ToonValue> for f64 {
    type Error = crate::Error;

    fn try_from(value: ToonValue) -> crate::Result<Self> {
        match value {
            ToonValue::Number(Number::Integer(i)) => Ok(i as f64),
            ToonValue::Number(Number::Float(f)) => Ok(f),
            ToonValue::Number(Number::Infinity) => Ok(f64::INFINITY),
            ToonValue::Number(Number::NegativeInfinity) => Ok(f64::NEG_INFINITY),
            ToonValue::Number(Number::NaN) => Ok(f64::NAN),
            _ => Err(crate::Error::custom(format!(
                "expected number, found {:?}",
                value
            ))),
        }
    }
}

impl TryFrom<ToonValue> for bool {
    type Error = crate::Error;

    fn try_from(value: ToonValue) -> crate::Result<Self> {
        match value {
            ToonValue::Bool(b) => Ok(b),
            _ => Err(crate::Error::custom(format!(
                "expected bool, found {:?}",
                value
            ))),
        }
    }
}

impl TryFrom<ToonValue> for String {
    type Error = crate::Error;

    fn try_from(value: ToonValue) -> crate::Result<Self> {
        match value {
            ToonValue::String(s) => Ok(s),
            _ => Err(crate::Error::custom(format!(
                "expected string, found {:?}",
                value
            ))),
        }
    }
}

// From implementations for creating ToonValue from primitives
impl From<bool> for ToonValue {
    fn from(value: bool) -> Self {
        ToonValue::Bool(value)
    }
}

impl From<i8> for ToonValue {
    fn from(value: i8) -> Self {
        ToonValue::Number(Number::Integer(value as i64))
    }
}

impl From<i16> for ToonValue {
    fn from(value: i16) -> Self {
        ToonValue::Number(Number::Integer(value as i64))
    }
}

impl From<i32> for ToonValue {
    fn from(value: i32) -> Self {
        ToonValue::Number(Number::Integer(value as i64))
    }
}

impl From<i64> for ToonValue {
    fn from(value: i64) -> Self {
        ToonValue::Number(Number::Integer(value))
    }
}

impl From<u8> for ToonValue {
    fn from(value: u8) -> Self {
        ToonValue::Number(Number::Integer(value as i64))
    }
}

impl From<u16> for ToonValue {
    fn from(value: u16) -> Self {
        ToonValue::Number(Number::Integer(value as i64))
    }
}

impl From<u32> for ToonValue {
    fn from(value: u32) -> Self {
        ToonValue::Number(Number::Integer(value as i64))
    }
}

impl From<f32> for ToonValue {
    fn from(value: f32) -> Self {
        ToonValue::Number(Number::Float(value as f64))
    }
}

impl From<f64> for ToonValue {
    fn from(value: f64) -> Self {
        ToonValue::Number(Number::Float(value))
    }
}

impl From<String> for ToonValue {
    fn from(value: String) -> Self {
        ToonValue::String(value)
    }
}

impl From<&str> for ToonValue {
    fn from(value: &str) -> Self {
        ToonValue::String(value.to_string())
    }
}

impl From<Vec<ToonValue>> for ToonValue {
    fn from(value: Vec<ToonValue>) -> Self {
        ToonValue::Array(value)
    }
}

impl From<ToonMap> for ToonValue {
    fn from(value: ToonMap) -> Self {
        ToonValue::Object(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::TryFrom;

    #[test]
    fn test_tryfrom_i64() {
        let value = ToonValue::Number(Number::Integer(42));
        let result: i64 = TryFrom::try_from(value).unwrap();
        assert_eq!(result, 42);

        let value = ToonValue::Number(Number::Float(42.0));
        let result: i64 = TryFrom::try_from(value).unwrap();
        assert_eq!(result, 42);

        let value = ToonValue::String("test".to_string());
        assert!(i64::try_from(value).is_err());
    }

    #[test]
    fn test_tryfrom_f64() {
        let value = ToonValue::Number(Number::Float(3.5));
        let result: f64 = TryFrom::try_from(value).unwrap();
        assert_eq!(result, 3.5);

        let value = ToonValue::Number(Number::Integer(42));
        let result: f64 = TryFrom::try_from(value).unwrap();
        assert_eq!(result, 42.0);

        let value = ToonValue::Number(Number::Infinity);
        let result: f64 = TryFrom::try_from(value).unwrap();
        assert_eq!(result, f64::INFINITY);
    }

    #[test]
    fn test_tryfrom_bool() {
        let value = ToonValue::Bool(true);
        let result: bool = TryFrom::try_from(value).unwrap();
        assert!(result);

        let value = ToonValue::Number(Number::Integer(1));
        assert!(bool::try_from(value).is_err());
    }

    #[test]
    fn test_tryfrom_string() {
        let value = ToonValue::String("hello".to_string());
        let result: String = TryFrom::try_from(value).unwrap();
        assert_eq!(result, "hello");

        let value = ToonValue::Number(Number::Integer(42));
        assert!(String::try_from(value).is_err());
    }

    #[test]
    fn test_from_primitives() {
        assert_eq!(ToonValue::from(true), ToonValue::Bool(true));
        assert_eq!(
            ToonValue::from(42i32),
            ToonValue::Number(Number::Integer(42))
        );
        assert_eq!(
            ToonValue::from(42i64),
            ToonValue::Number(Number::Integer(42))
        );
        assert_eq!(
            ToonValue::from(3.5f64),
            ToonValue::Number(Number::Float(3.5))
        );
        assert_eq!(
            ToonValue::from("test"),
            ToonValue::String("test".to_string())
        );
        assert_eq!(
            ToonValue::from("test".to_string()),
            ToonValue::String("test".to_string())
        );
    }

    #[test]
    fn test_from_collections() {
        let vec = vec![ToonValue::from(1i32), ToonValue::from(2i32)];
        let value = ToonValue::from(vec.clone());
        assert_eq!(value, ToonValue::Array(vec));

        let mut map = ToonMap::new();
        map.insert("key".to_string(), ToonValue::from(42i32));
        let value = ToonValue::from(map.clone());
        assert_eq!(value, ToonValue::Object(map));
    }

    #[test]
    fn test_const_is_methods() {
        const fn check_null(v: &ToonValue) -> bool {
            v.is_null()
        }

        let null_value = ToonValue::Null;
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

        let value = ToonValue::Number(Number::Integer(42));
        assert!(value.is_number());
        assert!(!value.is_null());
        assert!(!value.is_string());
    }
}
