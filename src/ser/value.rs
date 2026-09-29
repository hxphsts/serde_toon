//! [`ValueSerializer`]: converts any `Serialize` value into a [`Value`].

use crate::{Error, Number, Result, ToonMap, Value};
use num_bigint::BigInt;
use serde::{ser, Serialize};

/// Serializer whose output is a [`Value`], used by [`to_value`](crate::to_value).
///
/// The mapping is the one documented in the [module docs](crate::ser), with
/// these `Value`-specific choices:
///
/// - Integers that fit in `i64` become [`Number::Integer`]; wider `u64`,
///   `i128`, and `u128` values become [`Value::BigInt`], without loss.
/// - Enum variants with data become a single-key [`Value::Object`]
///   `{Variant: value}` at any depth.
/// - Map keys must serialize as strings (or chars).
///
/// # Examples
///
/// ```rust
/// use serde::Serialize;
/// use serde_toon::{Value, ValueSerializer};
///
/// let value = vec![1, 2].serialize(ValueSerializer)?;
/// assert_eq!(value, Value::Array(vec![Value::from(1), Value::from(2)]));
/// # Ok::<(), serde_toon::Error>(())
/// ```
#[derive(Debug)]
pub struct ValueSerializer;

/// Collects the elements of a sequence, tuple, or tuple variant for
/// [`ValueSerializer`].
#[derive(Debug)]
pub struct SerializeVec {
    vec: Vec<Value>,
    variant: Option<&'static str>,
}

/// Collects the entries of a map, struct, or struct variant for
/// [`ValueSerializer`].
#[derive(Debug)]
pub struct SerializeMap {
    map: ToonMap,
    current_key: Option<String>,
    variant: Option<&'static str>,
}

fn to_toon_value<T: Serialize + ?Sized>(value: &T) -> Result<Value> {
    value.serialize(ValueSerializer)
}

/// Wraps `value` as the externally tagged `{variant: value}` object.
fn tagged(variant: Option<&'static str>, value: Value) -> Value {
    match variant {
        Some(variant) => {
            let mut map = ToonMap::with_capacity(1);
            map.insert(variant.to_string(), value);
            Value::Object(map)
        }
        None => value,
    }
}

impl ser::Serializer for ValueSerializer {
    type Ok = Value;
    type Error = Error;

    type SerializeSeq = SerializeVec;
    type SerializeTuple = SerializeVec;
    type SerializeTupleStruct = SerializeVec;
    type SerializeTupleVariant = SerializeVec;
    type SerializeMap = SerializeMap;
    type SerializeStruct = SerializeMap;
    type SerializeStructVariant = SerializeMap;

    fn serialize_bool(self, v: bool) -> Result<Value> {
        Ok(Value::Bool(v))
    }

    fn serialize_i8(self, v: i8) -> Result<Value> {
        Ok(Value::Number(Number::Integer(v.into())))
    }

    fn serialize_i16(self, v: i16) -> Result<Value> {
        Ok(Value::Number(Number::Integer(v.into())))
    }

    fn serialize_i32(self, v: i32) -> Result<Value> {
        Ok(Value::Number(Number::Integer(v.into())))
    }

    fn serialize_i64(self, v: i64) -> Result<Value> {
        Ok(Value::Number(Number::Integer(v)))
    }

    fn serialize_i128(self, v: i128) -> Result<Value> {
        Ok(match i64::try_from(v) {
            Ok(v) => Value::Number(Number::Integer(v)),
            Err(_) => Value::BigInt(BigInt::from(v)),
        })
    }

    fn serialize_u8(self, v: u8) -> Result<Value> {
        Ok(Value::Number(Number::Integer(v.into())))
    }

    fn serialize_u16(self, v: u16) -> Result<Value> {
        Ok(Value::Number(Number::Integer(v.into())))
    }

    fn serialize_u32(self, v: u32) -> Result<Value> {
        Ok(Value::Number(Number::Integer(v.into())))
    }

    fn serialize_u64(self, v: u64) -> Result<Value> {
        Ok(match i64::try_from(v) {
            Ok(v) => Value::Number(Number::Integer(v)),
            Err(_) => Value::BigInt(BigInt::from(v)),
        })
    }

    fn serialize_u128(self, v: u128) -> Result<Value> {
        Ok(match i64::try_from(v) {
            Ok(v) => Value::Number(Number::Integer(v)),
            Err(_) => Value::BigInt(BigInt::from(v)),
        })
    }

    fn serialize_f32(self, v: f32) -> Result<Value> {
        Ok(Value::Number(Number::Float(v.into())))
    }

    fn serialize_f64(self, v: f64) -> Result<Value> {
        Ok(Value::Number(Number::Float(v)))
    }

    fn serialize_char(self, v: char) -> Result<Value> {
        Ok(Value::String(v.to_string()))
    }

    fn serialize_str(self, v: &str) -> Result<Value> {
        Ok(Value::String(v.to_string()))
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<Value> {
        let vec = v
            .iter()
            .map(|&b| Value::Number(Number::Integer(b.into())))
            .collect();
        Ok(Value::Array(vec))
    }

    fn serialize_none(self) -> Result<Value> {
        Ok(Value::Null)
    }

    fn serialize_some<T>(self, value: &T) -> Result<Value>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Value> {
        Ok(Value::Null)
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Value> {
        Ok(Value::Null)
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Value> {
        Ok(Value::String(variant.to_string()))
    }

    fn serialize_newtype_struct<T>(self, _name: &'static str, value: &T) -> Result<Value>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Value>
    where
        T: ?Sized + Serialize,
    {
        Ok(tagged(Some(variant), to_toon_value(value)?))
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<SerializeVec> {
        Ok(SerializeVec::new(len.unwrap_or(0), None))
    }

    fn serialize_tuple(self, len: usize) -> Result<SerializeVec> {
        Ok(SerializeVec::new(len, None))
    }

    fn serialize_tuple_struct(self, _name: &'static str, len: usize) -> Result<SerializeVec> {
        Ok(SerializeVec::new(len, None))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<SerializeVec> {
        Ok(SerializeVec::new(len, Some(variant)))
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<SerializeMap> {
        Ok(SerializeMap::new(None))
    }

    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<SerializeMap> {
        Ok(SerializeMap::new(None))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<SerializeMap> {
        Ok(SerializeMap::new(Some(variant)))
    }
}

impl SerializeVec {
    fn new(len: usize, variant: Option<&'static str>) -> Self {
        SerializeVec {
            vec: Vec::with_capacity(len),
            variant,
        }
    }

    fn push<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<()> {
        self.vec.push(to_toon_value(value)?);
        Ok(())
    }

    fn finish(self) -> Value {
        tagged(self.variant, Value::Array(self.vec))
    }
}

impl SerializeMap {
    fn new(variant: Option<&'static str>) -> Self {
        SerializeMap {
            map: ToonMap::new(),
            current_key: None,
            variant,
        }
    }

    fn finish(self) -> Value {
        tagged(self.variant, Value::Object(self.map))
    }
}

impl ser::SerializeSeq for SerializeVec {
    type Ok = Value;
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.push(value)
    }

    fn end(self) -> Result<Value> {
        Ok(self.finish())
    }
}

impl ser::SerializeTuple for SerializeVec {
    type Ok = Value;
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.push(value)
    }

    fn end(self) -> Result<Value> {
        Ok(self.finish())
    }
}

impl ser::SerializeTupleStruct for SerializeVec {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.push(value)
    }

    fn end(self) -> Result<Value> {
        Ok(self.finish())
    }
}

impl ser::SerializeTupleVariant for SerializeVec {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.push(value)
    }

    fn end(self) -> Result<Value> {
        Ok(self.finish())
    }
}

impl ser::SerializeMap for SerializeMap {
    type Ok = Value;
    type Error = Error;

    fn serialize_key<T>(&mut self, key: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        match to_toon_value(key)? {
            Value::String(s) => {
                self.current_key = Some(s);
                Ok(())
            }
            _ => Err(Error::custom("Map keys must be strings")),
        }
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        let key = self
            .current_key
            .take()
            .ok_or_else(super::ir::value_without_key)?;
        self.map.insert(key, to_toon_value(value)?);
        Ok(())
    }

    fn end(self) -> Result<Value> {
        Ok(self.finish())
    }
}

impl ser::SerializeStruct for SerializeMap {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.map.insert(key.to_string(), to_toon_value(value)?);
        Ok(())
    }

    fn end(self) -> Result<Value> {
        Ok(self.finish())
    }
}

impl ser::SerializeStructVariant for SerializeMap {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.map.insert(key.to_string(), to_toon_value(value)?);
        Ok(())
    }

    fn end(self) -> Result<Value> {
        Ok(self.finish())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::to_value;

    #[test]
    fn wide_integers_are_lossless() {
        assert_eq!(
            to_value(&u64::MAX).unwrap(),
            Value::BigInt(BigInt::from(u64::MAX))
        );
        assert_eq!(
            to_value(&(i64::MAX as u64)).unwrap(),
            Value::Number(Number::Integer(i64::MAX))
        );
        assert_eq!(
            to_value(&-5i128).unwrap(),
            Value::Number(Number::Integer(-5))
        );
        assert_eq!(
            to_value(&i128::MIN).unwrap(),
            Value::BigInt(BigInt::from(i128::MIN))
        );
        assert_eq!(
            to_value(&u128::MAX).unwrap(),
            Value::BigInt(BigInt::from(u128::MAX))
        );
    }

    #[derive(Serialize)]
    enum Shape {
        Unit,
        Circle(f64),
        Pair(i32, i32),
        Rect { w: i32 },
    }

    fn object(key: &str, value: Value) -> Value {
        let mut map = ToonMap::new();
        map.insert(key.to_string(), value);
        Value::Object(map)
    }

    #[test]
    fn enum_variants_are_externally_tagged() {
        assert_eq!(to_value(&Shape::Unit).unwrap(), Value::from("Unit"));
        assert_eq!(
            to_value(&Shape::Circle(1.5)).unwrap(),
            object("Circle", Value::from(1.5))
        );
        assert_eq!(
            to_value(&vec![Shape::Pair(1, 2)]).unwrap(),
            Value::Array(vec![object(
                "Pair",
                Value::Array(vec![Value::from(1), Value::from(2)])
            )])
        );
        assert_eq!(
            to_value(&Shape::Rect { w: 3 }).unwrap(),
            object("Rect", object("w", Value::from(3)))
        );
    }
}
