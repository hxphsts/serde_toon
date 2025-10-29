//! TOON serialization.
//!
//! This module provides the [`Serializer`] implementation that converts
//! Rust data structures into TOON format strings.
//!
//! ## Overview
//!
//! The serializer automatically applies TOON's space-saving optimizations:
//!
//! - **Tabular arrays**: Homogeneous object arrays serialize as compact tables
//! - **Inline primitives**: Simple arrays serialize inline (e.g., `[3]: 1,2,3`)
//! - **List format**: Complex arrays use list syntax with `- ` prefixes
//! - **Quote minimization**: Strings are unquoted when safe
//!
//! ## Usage
//!
//! Most users should use the high-level functions in the crate root:
//!
//! ```rust
//! use serde_toon::{to_string, to_string_pretty, ToonOptions};
//! use serde::Serialize;
//!
//! #[derive(Serialize)]
//! struct Data { x: i32, y: i32 }
//!
//! let data = Data { x: 1, y: 2 };
//!
//! // Compact format
//! let compact = to_string(&data).unwrap();
//!
//! // Pretty format
//! let pretty = to_string_pretty(&data).unwrap();
//! ```
//!
//! ## Direct Serializer Usage
//!
//! For advanced use cases, you can use the serializer directly:
//!
//! ```rust
//! use serde_toon::{Serializer, ToonOptions};
//! use serde::Serialize;
//!
//! let options = ToonOptions::new();
//! let mut serializer = Serializer::new(options);
//!
//! let data = vec![1, 2, 3, 4, 5];
//! data.serialize(&mut serializer).unwrap();
//!
//! let toon_string = serializer.into_inner();
//! assert_eq!(toon_string, "[5]: 1,2,3,4,5");
//! ```

use crate::{Error, Number, Result, ToonMap, ToonOptions, ToonValue};
use serde::ser::SerializeSeq;
use serde::{ser, Serialize};

/// The TOON serializer.
///
/// Converts Rust values implementing `Serialize` into TOON format strings.
/// Created via [`Serializer::new`] with customizable options.
pub struct Serializer {
    output: String,
    options: ToonOptions,
    indent_level: usize,
}

impl Serializer {
    pub fn new(options: ToonOptions) -> Self {
        // Pre-allocate with reasonable capacity to reduce reallocations
        // 256 bytes is a good starting point for typical structs
        Serializer {
            output: String::with_capacity(256),
            options,
            indent_level: 0,
        }
    }

    pub fn into_inner(self) -> String {
        self.output
    }

    fn write_newline(&mut self) {
        if self.options.pretty {
            self.output.push('\n');
        }
    }

    #[inline]
    fn needs_quotes(s: &str) -> bool {
        s.is_empty()
            || s.contains(':')
            || s.contains(',')
            || s.contains('\n')
            || s.contains('\t')
            || s.contains('|')
            || s.contains('"')
            || s.contains('\\')
            || s.contains('\0')
            || s.starts_with(' ')
            || s.ends_with(' ')
            || s == "true"
            || s == "false"
            || s == "null"
            || s.parse::<f64>().is_ok()
    }

    #[inline]
    fn write_string(&mut self, s: &str) {
        if Self::needs_quotes(s) {
            self.output.push('"');
            for ch in s.chars() {
                match ch {
                    '"' => self.output.push_str("\\\""),
                    '\\' => self.output.push_str("\\\\"),
                    '\n' => self.output.push_str("\\n"),
                    '\r' => self.output.push_str("\\r"),
                    '\t' => self.output.push_str("\\t"),
                    '\u{0008}' => self.output.push_str("\\b"), // backspace
                    '\u{000C}' => self.output.push_str("\\f"), // form feed
                    '\0' => self.output.push_str("\\0"),
                    _ => self.output.push(ch),
                }
            }
            self.output.push('"');
        } else {
            self.output.push_str(s);
        }
    }
}

impl<'a> ser::Serializer for &'a mut Serializer {
    type Ok = ();
    type Error = Error;

    type SerializeSeq = SeqSerializer<'a>;
    type SerializeTuple = TupleSerializer<'a>;
    type SerializeTupleStruct = TupleStructSerializer<'a>;
    type SerializeTupleVariant = TupleVariantSerializer<'a>;
    type SerializeMap = MapSerializer<'a>;
    type SerializeStruct = StructSerializer<'a>;
    type SerializeStructVariant = StructVariantSerializer<'a>;

    fn serialize_bool(self, v: bool) -> Result<Self::Ok> {
        self.output.push_str(if v { "true" } else { "false" });
        Ok(())
    }

    fn serialize_i8(self, v: i8) -> Result<Self::Ok> {
        self.serialize_i64(v as i64)
    }

    fn serialize_i16(self, v: i16) -> Result<Self::Ok> {
        self.serialize_i64(v as i64)
    }

    fn serialize_i32(self, v: i32) -> Result<Self::Ok> {
        self.serialize_i64(v as i64)
    }

    fn serialize_i64(self, v: i64) -> Result<Self::Ok> {
        self.output.push_str(&v.to_string());
        Ok(())
    }

    fn serialize_u8(self, v: u8) -> Result<Self::Ok> {
        self.serialize_u64(v as u64)
    }

    fn serialize_u16(self, v: u16) -> Result<Self::Ok> {
        self.serialize_u64(v as u64)
    }

    fn serialize_u32(self, v: u32) -> Result<Self::Ok> {
        self.serialize_u64(v as u64)
    }

    fn serialize_u64(self, v: u64) -> Result<Self::Ok> {
        self.output.push_str(&v.to_string());
        Ok(())
    }

    fn serialize_f32(self, v: f32) -> Result<Self::Ok> {
        self.serialize_f64(v as f64)
    }

    fn serialize_f64(self, v: f64) -> Result<Self::Ok> {
        self.output.push_str(&v.to_string());
        Ok(())
    }

    fn serialize_char(self, v: char) -> Result<Self::Ok> {
        self.serialize_str(&v.to_string())
    }

    fn serialize_str(self, v: &str) -> Result<Self::Ok> {
        self.write_string(v);
        Ok(())
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<Self::Ok> {
        use ser::SerializeSeq;
        let mut seq = self.serialize_seq(Some(v.len()))?;
        for byte in v {
            seq.serialize_element(byte)?;
        }
        seq.end()
    }

    fn serialize_none(self) -> Result<Self::Ok> {
        self.serialize_unit()
    }

    fn serialize_some<T>(self, value: &T) -> Result<Self::Ok>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Self::Ok> {
        self.output.push_str("null");
        Ok(())
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok> {
        self.serialize_unit()
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok> {
        self.serialize_str(variant)
    }

    fn serialize_newtype_struct<T>(self, _name: &'static str, value: &T) -> Result<Self::Ok>
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
    ) -> Result<Self::Ok>
    where
        T: ?Sized + Serialize,
    {
        self.output.push_str(variant);
        self.output.push(':');
        if self.options.pretty {
            self.output.push(' ');
        }
        value.serialize(self)
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq> {
        Ok(SeqSerializer {
            ser: self,
            elements: Vec::new(),
        })
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple> {
        Ok(TupleSerializer {
            ser: self,
            elements: Vec::new(),
        })
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        Ok(TupleStructSerializer {
            ser: self,
            elements: Vec::new(),
        })
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        Ok(TupleVariantSerializer {
            ser: self,
            variant: variant.to_string(),
            elements: Vec::new(),
        })
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap> {
        Ok(MapSerializer {
            ser: self,
            entries: Vec::new(),
            current_key: None,
        })
    }

    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<Self::SerializeStruct> {
        Ok(StructSerializer {
            ser: self,
            entries: Vec::new(),
        })
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        Ok(StructVariantSerializer {
            ser: self,
            variant: variant.to_string(),
            entries: Vec::new(),
        })
    }
}

pub struct SeqSerializer<'a> {
    ser: &'a mut Serializer,
    elements: Vec<ToonValue>,
}

impl<'a> ser::SerializeSeq for SeqSerializer<'a> {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        let toon_value = to_toon_value(value)?;
        self.elements.push(toon_value);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok> {
        if self.elements.is_empty() {
            self.ser.output.push_str("[0]:");
            return Ok(());
        }

        let tabular = can_be_tabular(&self.elements);

        if let Some((headers, rows)) = tabular {
            // Tabular format: [N]{field1,field2}:
            write_tabular_array(
                &mut self.ser.output,
                &headers,
                &rows,
                &self.ser.options,
                self.ser.indent_level,
            );
        } else {
            // Check if all elements are primitives for inline format
            let all_primitives = self.elements.iter().all(is_primitive_value);

            if all_primitives {
                // Inline format: [N]: val1,val2,val3
                write_inline_array(&mut self.ser.output, &self.elements, &self.ser.options);
            } else {
                // List format with "- " prefix
                write_list_array(
                    &mut self.ser.output,
                    &self.elements,
                    &self.ser.options,
                    self.ser.indent_level,
                );
            }
        }

        Ok(())
    }
}

pub struct TupleSerializer<'a> {
    ser: &'a mut Serializer,
    elements: Vec<ToonValue>,
}

impl<'a> ser::SerializeTuple for TupleSerializer<'a> {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        let toon_value = to_toon_value(value)?;
        self.elements.push(toon_value);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok> {
        let seq_ser = SeqSerializer {
            ser: self.ser,
            elements: self.elements,
        };
        seq_ser.end()
    }
}

pub struct TupleStructSerializer<'a> {
    ser: &'a mut Serializer,
    elements: Vec<ToonValue>,
}

impl<'a> ser::SerializeTupleStruct for TupleStructSerializer<'a> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        let toon_value = to_toon_value(value)?;
        self.elements.push(toon_value);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok> {
        let seq_ser = SeqSerializer {
            ser: self.ser,
            elements: self.elements,
        };
        seq_ser.end()
    }
}

pub struct TupleVariantSerializer<'a> {
    ser: &'a mut Serializer,
    variant: String,
    elements: Vec<ToonValue>,
}

impl<'a> ser::SerializeTupleVariant for TupleVariantSerializer<'a> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        let toon_value = to_toon_value(value)?;
        self.elements.push(toon_value);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok> {
        self.ser.output.push_str(&self.variant);
        self.ser.output.push(':');
        if self.ser.options.pretty {
            self.ser.output.push(' ');
        }

        let seq_ser = SeqSerializer {
            ser: self.ser,
            elements: self.elements,
        };
        seq_ser.end()
    }
}

pub struct MapSerializer<'a> {
    ser: &'a mut Serializer,
    entries: Vec<(String, ToonValue)>,
    current_key: Option<String>,
}

impl<'a> ser::SerializeMap for MapSerializer<'a> {
    type Ok = ();
    type Error = Error;

    fn serialize_key<T>(&mut self, key: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        let key_value = to_toon_value(key)?;
        match key_value {
            ToonValue::String(s) => {
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
            .ok_or_else(|| Error::custom("serialize_value called without serialize_key"))?;
        let toon_value = to_toon_value(value)?;
        self.entries.push((key, toon_value));
        Ok(())
    }

    fn end(self) -> Result<Self::Ok> {
        write_object(
            &mut self.ser.output,
            &self.entries,
            &self.ser.options,
            self.ser.indent_level,
        );
        Ok(())
    }
}

pub struct StructSerializer<'a> {
    ser: &'a mut Serializer,
    entries: Vec<(String, ToonValue)>,
}

impl<'a> ser::SerializeStruct for StructSerializer<'a> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        let toon_value = to_toon_value(value)?;
        self.entries.push((key.to_string(), toon_value));
        Ok(())
    }

    fn end(self) -> Result<Self::Ok> {
        write_object(
            &mut self.ser.output,
            &self.entries,
            &self.ser.options,
            self.ser.indent_level,
        );
        Ok(())
    }
}

pub struct StructVariantSerializer<'a> {
    ser: &'a mut Serializer,
    variant: String,
    entries: Vec<(String, ToonValue)>,
}

impl<'a> ser::SerializeStructVariant for StructVariantSerializer<'a> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        let toon_value = to_toon_value(value)?;
        self.entries.push((key.to_string(), toon_value));
        Ok(())
    }

    fn end(self) -> Result<Self::Ok> {
        self.ser.output.push_str(&self.variant);
        self.ser.output.push(':');

        if self.ser.options.pretty {
            self.ser.write_newline();
            self.ser.indent_level += 1;
        }

        write_object(
            &mut self.ser.output,
            &self.entries,
            &self.ser.options,
            self.ser.indent_level,
        );

        if self.ser.options.pretty {
            self.ser.indent_level -= 1;
        }

        Ok(())
    }
}

pub struct ToonValueSerializer;

pub struct SerializeVec {
    vec: Vec<ToonValue>,
}

pub struct SerializeMap {
    map: ToonMap,
    current_key: Option<String>,
}

impl ser::Serializer for ToonValueSerializer {
    type Ok = ToonValue;
    type Error = Error;

    type SerializeSeq = SerializeVec;
    type SerializeTuple = SerializeVec;
    type SerializeTupleStruct = SerializeVec;
    type SerializeTupleVariant = SerializeVec;
    type SerializeMap = SerializeMap;
    type SerializeStruct = SerializeMap;
    type SerializeStructVariant = SerializeMap;

    fn serialize_bool(self, v: bool) -> Result<ToonValue> {
        Ok(ToonValue::Bool(v))
    }

    fn serialize_i8(self, v: i8) -> Result<ToonValue> {
        Ok(ToonValue::Number(Number::Integer(v as i64)))
    }

    fn serialize_i16(self, v: i16) -> Result<ToonValue> {
        Ok(ToonValue::Number(Number::Integer(v as i64)))
    }

    fn serialize_i32(self, v: i32) -> Result<ToonValue> {
        Ok(ToonValue::Number(Number::Integer(v as i64)))
    }

    fn serialize_i64(self, v: i64) -> Result<ToonValue> {
        Ok(ToonValue::Number(Number::Integer(v)))
    }

    fn serialize_u8(self, v: u8) -> Result<ToonValue> {
        Ok(ToonValue::Number(Number::Integer(v as i64)))
    }

    fn serialize_u16(self, v: u16) -> Result<ToonValue> {
        Ok(ToonValue::Number(Number::Integer(v as i64)))
    }

    fn serialize_u32(self, v: u32) -> Result<ToonValue> {
        Ok(ToonValue::Number(Number::Integer(v as i64)))
    }

    fn serialize_u64(self, v: u64) -> Result<ToonValue> {
        if v <= i64::MAX as u64 {
            Ok(ToonValue::Number(Number::Integer(v as i64)))
        } else {
            Ok(ToonValue::Number(Number::Float(v as f64)))
        }
    }

    fn serialize_f32(self, v: f32) -> Result<ToonValue> {
        Ok(ToonValue::Number(Number::Float(v as f64)))
    }

    fn serialize_f64(self, v: f64) -> Result<ToonValue> {
        Ok(ToonValue::Number(Number::Float(v)))
    }

    fn serialize_char(self, v: char) -> Result<ToonValue> {
        Ok(ToonValue::String(v.to_string()))
    }

    fn serialize_str(self, v: &str) -> Result<ToonValue> {
        Ok(ToonValue::String(v.to_string()))
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<ToonValue> {
        let vec = v
            .iter()
            .map(|&b| ToonValue::Number(Number::Integer(b as i64)))
            .collect();
        Ok(ToonValue::Array(vec))
    }

    fn serialize_none(self) -> Result<ToonValue> {
        Ok(ToonValue::Null)
    }

    fn serialize_some<T>(self, value: &T) -> Result<ToonValue>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<ToonValue> {
        Ok(ToonValue::Null)
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<ToonValue> {
        Ok(ToonValue::Null)
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<ToonValue> {
        Ok(ToonValue::String(variant.to_string()))
    }

    fn serialize_newtype_struct<T>(self, _name: &'static str, value: &T) -> Result<ToonValue>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<ToonValue>
    where
        T: ?Sized + Serialize,
    {
        Err(Error::unsupported_type("newtype variants"))
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<SerializeVec> {
        Ok(SerializeVec::new())
    }

    fn serialize_tuple(self, _len: usize) -> Result<SerializeVec> {
        Ok(SerializeVec::new())
    }

    fn serialize_tuple_struct(self, _name: &'static str, _len: usize) -> Result<SerializeVec> {
        Ok(SerializeVec::new())
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<SerializeVec> {
        Err(Error::unsupported_type("tuple variants"))
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<SerializeMap> {
        Ok(SerializeMap::new())
    }

    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<SerializeMap> {
        Ok(SerializeMap::new())
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<SerializeMap> {
        Err(Error::unsupported_type("struct variants"))
    }
}

impl SerializeVec {
    fn new() -> Self {
        SerializeVec { vec: Vec::new() }
    }
}

impl SerializeMap {
    fn new() -> Self {
        SerializeMap {
            map: ToonMap::new(),
            current_key: None,
        }
    }
}

impl ser::SerializeSeq for SerializeVec {
    type Ok = ToonValue;
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.vec.push(to_toon_value(value)?);
        Ok(())
    }

    fn end(self) -> Result<ToonValue> {
        Ok(ToonValue::Array(self.vec))
    }
}

impl ser::SerializeTuple for SerializeVec {
    type Ok = ToonValue;
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.vec.push(to_toon_value(value)?);
        Ok(())
    }

    fn end(self) -> Result<ToonValue> {
        Ok(ToonValue::Array(self.vec))
    }
}

impl ser::SerializeTupleStruct for SerializeVec {
    type Ok = ToonValue;
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.vec.push(to_toon_value(value)?);
        Ok(())
    }

    fn end(self) -> Result<ToonValue> {
        Ok(ToonValue::Array(self.vec))
    }
}

impl ser::SerializeTupleVariant for SerializeVec {
    type Ok = ToonValue;
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.vec.push(to_toon_value(value)?);
        Ok(())
    }

    fn end(self) -> Result<ToonValue> {
        Ok(ToonValue::Array(self.vec))
    }
}

impl ser::SerializeMap for SerializeMap {
    type Ok = ToonValue;
    type Error = Error;

    fn serialize_key<T>(&mut self, key: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        match to_toon_value(key)? {
            ToonValue::String(s) => {
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
            .ok_or_else(|| Error::custom("serialize_value called without serialize_key"))?;
        self.map.insert(key, to_toon_value(value)?);
        Ok(())
    }

    fn end(self) -> Result<ToonValue> {
        Ok(ToonValue::Object(self.map))
    }
}

impl ser::SerializeStruct for SerializeMap {
    type Ok = ToonValue;
    type Error = Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.map.insert(key.to_string(), to_toon_value(value)?);
        Ok(())
    }

    fn end(self) -> Result<ToonValue> {
        Ok(ToonValue::Object(self.map))
    }
}

impl ser::SerializeStructVariant for SerializeMap {
    type Ok = ToonValue;
    type Error = Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.map.insert(key.to_string(), to_toon_value(value)?);
        Ok(())
    }

    fn end(self) -> Result<ToonValue> {
        Ok(ToonValue::Object(self.map))
    }
}

fn to_toon_value<T: Serialize + ?Sized>(value: &T) -> Result<ToonValue> {
    value.serialize(ToonValueSerializer)
}

fn can_be_tabular(elements: &[ToonValue]) -> Option<(Vec<String>, Vec<Vec<ToonValue>>)> {
    if elements.is_empty() {
        return None;
    }

    // All elements must be objects with identical primitive fields
    let first_headers = match &elements[0] {
        ToonValue::Object(obj) => {
            // Check that all values are primitives (not objects or arrays)
            for value in obj.values() {
                if !is_primitive_value(value) {
                    return None;
                }
            }

            let mut headers: Vec<_> = obj.keys().cloned().collect();
            headers.sort(); // TOON spec: fields are sorted alphabetically
            headers
        }
        _ => return None,
    };

    let mut rows = Vec::new();

    for element in elements {
        match element {
            ToonValue::Object(obj) => {
                // Check that this object has the same structure
                let mut element_headers: Vec<_> = obj.keys().cloned().collect();
                element_headers.sort();

                if element_headers != first_headers {
                    return None;
                }

                // Check that all values are still primitives
                for value in obj.values() {
                    if !is_primitive_value(value) {
                        return None;
                    }
                }

                let row: Vec<_> = first_headers
                    .iter()
                    .map(|key| obj.get(key).cloned().unwrap_or(ToonValue::Null))
                    .collect();
                rows.push(row);
            }
            _ => return None,
        }
    }

    Some((first_headers, rows))
}

#[inline]
fn is_primitive_value(value: &ToonValue) -> bool {
    match value {
        ToonValue::Null
        | ToonValue::Bool(_)
        | ToonValue::Number(_)
        | ToonValue::String(_)
        | ToonValue::Date(_)
        | ToonValue::BigInt(_) => true,
        ToonValue::Array(_) | ToonValue::Object(_) | ToonValue::Table { .. } => false,
    }
}

fn write_tabular_array(
    output: &mut String,
    headers: &[String],
    rows: &[Vec<ToonValue>],
    options: &ToonOptions,
    indent_level: usize,
) {
    // Format header: [N]{field1,field2}: or [N|]{field1|field2}: or [N    ]{field1    field2}:
    // Cache delimiter string to avoid repeated method calls in loop
    let delimiter_str = options.delimiter.as_str();
    let len_marker = if let Some(marker) = options.length_marker {
        format!("{}{}", marker, rows.len())
    } else {
        rows.len().to_string()
    };

    // Encode delimiter in header according to TOON spec
    // Use &str to avoid unnecessary String allocations
    let header_suffix = match options.delimiter {
        crate::Delimiter::Comma => "",   // implicit for comma
        crate::Delimiter::Tab => "    ", // show tabs as spaces in header
        crate::Delimiter::Pipe => "|",
    };

    let headers_str = match options.delimiter {
        crate::Delimiter::Comma => headers.join(","),
        crate::Delimiter::Tab => headers.join("    "), // tabs shown as spaces in header
        crate::Delimiter::Pipe => headers.join("|"),
    };

    output.push_str(&format!(
        "[{}{}]{{{}}}:",
        len_marker, header_suffix, headers_str
    ));

    // Write rows
    for row in rows {
        output.push('\n');
        output.push_str(&" ".repeat((indent_level + 1) * options.indent));

        for (i, value) in row.iter().enumerate() {
            if i > 0 {
                output.push_str(delimiter_str);
            }
            write_toon_value_quoted(output, value, options);
        }
    }
}

fn write_inline_array(output: &mut String, elements: &[ToonValue], options: &ToonOptions) {
    // Cache delimiter string for loop performance
    let delimiter_str = options.delimiter.as_str();
    let len_marker = if let Some(marker) = options.length_marker {
        format!("{}{}", marker, elements.len())
    } else {
        elements.len().to_string()
    };

    // Encode delimiter in header
    // Use &str to avoid unnecessary String allocations
    let header_suffix = match options.delimiter {
        crate::Delimiter::Comma => "",
        crate::Delimiter::Tab => "    ",
        crate::Delimiter::Pipe => "|",
    };

    output.push_str(&format!("[{}{}]: ", len_marker, header_suffix));

    for (i, element) in elements.iter().enumerate() {
        if i > 0 {
            output.push_str(delimiter_str);
        }
        write_toon_value_quoted(output, element, options);
    }
}

fn write_list_array(
    output: &mut String,
    elements: &[ToonValue],
    options: &ToonOptions,
    indent_level: usize,
) {
    let len_marker = if let Some(marker) = options.length_marker {
        format!("{}{}", marker, elements.len())
    } else {
        elements.len().to_string()
    };

    output.push_str(&format!("[{}]:", len_marker));

    for element in elements {
        output.push('\n');
        output.push_str(&" ".repeat((indent_level + 1) * options.indent));
        output.push_str("- ");

        match element {
            ToonValue::Object(obj) => {
                // For objects in list format, sort keys alphabetically for deterministic output
                let mut sorted_entries: Vec<_> = obj.iter().collect();
                sorted_entries.sort_by(|(k1, _), (k2, _)| k1.cmp(k2));

                let mut iter = sorted_entries.into_iter();

                if let Some((first_key, first_value)) = iter.next() {
                    output.push_str(first_key);
                    output.push_str(": ");
                    write_toon_value_quoted(output, first_value, options);

                    // Remaining fields at same indentation level as the "- "
                    for (key, value) in iter {
                        output.push('\n');
                        output.push_str(&" ".repeat((indent_level + 1) * options.indent));
                        output.push_str("  "); // align with content after "- "
                        output.push_str(key);
                        output.push_str(": ");
                        write_toon_value_quoted(output, value, options);
                    }
                }
            }
            _ => {
                write_toon_value_quoted(output, element, options);
            }
        }
    }
}

fn write_array_toon(
    output: &mut String,
    arr: &[ToonValue],
    options: &ToonOptions,
    indent_level: usize,
) {
    if arr.is_empty() {
        output.push_str("[0]:");
        return;
    }

    // Check if array can be tabular
    if let Some((headers, rows)) = can_be_tabular(arr) {
        write_tabular_array(output, &headers, &rows, options, indent_level);
    } else if arr.iter().all(is_primitive_value) {
        // Inline format for all primitives
        write_inline_array(output, arr, options);
    } else {
        // List format for mixed content
        write_list_array(output, arr, options, indent_level);
    }
}

fn write_object(
    output: &mut String,
    entries: &[(String, ToonValue)],
    options: &ToonOptions,
    indent_level: usize,
) {
    for (i, (key, value)) in entries.iter().enumerate() {
        if i > 0 {
            output.push('\n');
        }

        // Add indentation for nested objects or pretty mode
        if indent_level > 0 {
            // Nested objects always get indented
            output.push_str(&" ".repeat(indent_level * options.indent));
        } else if i > 0 && options.pretty {
            // Top-level objects only get indented in pretty mode and after first field
            output.push_str(&" ".repeat(indent_level * options.indent));
        }

        output.push_str(key);
        output.push(':');

        match value {
            ToonValue::Array(arr) => {
                // Arrays get special TOON formatting
                output.push(' ');
                write_array_toon(output, arr, options, indent_level);
            }
            ToonValue::Object(obj) => {
                // For nested objects, handle indentation properly
                output.push('\n');
                let entries: Vec<_> = obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                write_object(output, &entries, options, indent_level + 1);
            }
            ToonValue::Table { .. } => {
                // For tables, no space after colon
                output.push('\n');
                output.push_str(&" ".repeat((indent_level + 1) * options.indent));
                write_toon_value_quoted(output, value, options);
            }
            _ => {
                // For primitives, space after colon
                output.push(' ');
                write_toon_value_quoted(output, value, options);
            }
        }
    }
}

fn write_toon_value_quoted(output: &mut String, value: &ToonValue, options: &ToonOptions) {
    match value {
        ToonValue::Null => output.push_str("null"),
        ToonValue::Bool(b) => output.push_str(if *b { "true" } else { "false" }),
        ToonValue::Number(n) => output.push_str(&n.to_string()),
        ToonValue::String(s) => {
            if needs_quotes_toon(s, options) {
                output.push('"');
                for ch in s.chars() {
                    match ch {
                        '"' => output.push_str("\\\""),
                        '\\' => output.push_str("\\\\"),
                        '\n' => output.push_str("\\n"),
                        '\r' => output.push_str("\\r"),
                        '\t' => output.push_str("\\t"),
                        '\u{0008}' => output.push_str("\\b"), // backspace
                        '\u{000C}' => output.push_str("\\f"), // form feed
                        '\0' => output.push_str("\\0"),
                        _ => output.push(ch),
                    }
                }
                output.push('"');
            } else {
                output.push_str(s);
            }
        }
        ToonValue::Array(arr) => {
            // Arrays should be handled by their containing context
            output.push('[');
            for (i, elem) in arr.iter().enumerate() {
                if i > 0 {
                    output.push_str(options.delimiter.as_str());
                }
                write_toon_value_quoted(output, elem, options);
            }
            output.push(']');
        }
        ToonValue::Object(obj) => {
            let entries: Vec<_> = obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
            write_object(output, &entries, options, 0);
        }
        ToonValue::Table { headers, rows } => {
            write_tabular_array(output, headers, rows, options, 0);
        }
        ToonValue::Date(dt) => {
            let s = dt.to_rfc3339();
            if needs_quotes_toon(&s, options) {
                output.push('"');
                output.push_str(&s);
                output.push('"');
            } else {
                output.push_str(&s);
            }
        }
        ToonValue::BigInt(bi) => {
            let s = format!("{}n", bi);
            if needs_quotes_toon(&s, options) {
                output.push('"');
                output.push_str(&s);
                output.push('"');
            } else {
                output.push_str(&s);
            }
        }
    }
}

fn needs_quotes_toon(s: &str, options: &ToonOptions) -> bool {
    if s.is_empty() {
        return true;
    }

    // Leading or trailing spaces
    if s.starts_with(' ') || s.ends_with(' ') {
        return true;
    }

    // Contains active delimiter, colon, quote, backslash, or control chars
    let active_delimiter = options.delimiter.as_str();
    if s.contains(':')
        || s.contains('"')
        || s.contains('\\')
        || s.contains('\n')
        || s.contains('\r')
        || s.contains('\t')
        || s.contains('\0')
    {
        return true;
    }

    // Contains active delimiter
    if s.contains(active_delimiter) {
        return true;
    }

    // Looks like boolean/number/null
    if s == "true" || s == "false" || s == "null" {
        return true;
    }

    // Looks like a number
    if s.parse::<f64>().is_ok() || s.parse::<i64>().is_ok() {
        return true;
    }

    // Starts with "- " (list-like)
    if s.starts_with("- ") {
        return true;
    }

    // Looks like structural token
    if s.starts_with('[') && s.contains(']') {
        return true;
    }
    if s.starts_with('{') && s.contains('}') {
        return true;
    }

    false
}
