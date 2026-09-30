//! TOON serialization (encoder for TOON spec v4.1).
//!
//! This module provides the [`Serializer`] that turns any `Serialize` value
//! into TOON text, and the [`ValueSerializer`] that turns it into a
//! [`Value`](crate::Value). Most users should call the crate-root functions
//! ([`to_string`](crate::to_string), [`to_string_with_options`](crate::to_string_with_options),
//! [`to_writer`](crate::to_writer), [`to_value`](crate::to_value)).
//!
//! ## Output forms
//!
//! The encoder picks each value's form from its shape and position, as the
//! specification requires (§9, §10):
//!
//! - **Inline arrays** for primitives: `tags[3]: a,b,c`.
//! - **Tabular arrays** for uniform objects, with nested field groups for
//!   uniform nested objects: `orders[2]{id,customer{name,country}}:`.
//! - **Keyed tabular objects** for objects of two or more uniform objects:
//!   `users[2:]{age,city}:` followed by `alice: 30,Berlin` rows.
//! - **List form** (`- item`) for everything else.
//! - Strings are quoted only when §7.2 requires it; keys only when §7.3 does.
//!
//! ## Serde data model mapping (§3 host-type normalization)
//!
//! | Rust / serde                                | TOON (JSON data model)                          |
//! |---------------------------------------------|-------------------------------------------------|
//! | `bool`                                      | `true` / `false`                                |
//! | `i8`-`i128`, `u8`-`u128`                    | integer, written exactly in decimal             |
//! | `f32`, `f64` (finite)                       | number, canonical decimal (§2); exponent form such as `1e+21` / `1e-7` outside `1e-6 <= abs(n) < 1e21`; `-0.0` as `0`; `f32` uses its own shortest digits |
//! | `f32`, `f64` NaN / +/-infinity                | `null`                                          |
//! | `char`, `&str`, `String`                    | string                                          |
//! | `&[u8]` via `serialize_bytes`               | array of numbers                                |
//! | `None`, `()`, unit struct                   | `null`                                          |
//! | `Some(v)`, newtype struct                   | `v`                                             |
//! | unit variant `E::A`                         | string `A`                                      |
//! | newtype / tuple / struct variant `E::A(..)` | single-key object `{A: value}` (externally tagged), at any depth |
//! | sequence, tuple, tuple struct, set          | array                                           |
//! | struct, map                                 | object, fields in serialization order           |
//! | map key                                     | strings and chars as-is; integers and booleans as their decimal / literal text; any other key type is an error |
//! | [`Value::Table`](crate::Value::Table)       | array of objects (tabular when uniform)         |
//! | [`Value::Date`](crate::Value::Date)         | RFC 3339 string                                 |
//! | [`Value::BigInt`](crate::Value::BigInt)     | string `"<digits>n"`                            |
//! | [`Number::NaN`](crate::Number::NaN), `Infinity`, `NegativeInfinity` | `null`                  |
//!
//! Object keys are written in the order serde provides them; duplicate keys
//! produced by a hand-written `Serialize` impl are written as given and
//! disable the tabular forms for that value.
//!
//! [`ToonOptions`] fields: `indent` (spaces per level, at least 1) and
//! `delimiter` (the document delimiter, declared in every header). The
//! deprecated `length_marker` and the `pretty` flag have no effect.
//!
//! ## Usage
//!
//! ```rust
//! use serde::Serialize;
//! use serde_toon::{to_string, Serializer, ToonOptions};
//!
//! #[derive(Serialize)]
//! struct Item { sku: &'static str, qty: u32 }
//!
//! let items = [Item { sku: "A1", qty: 2 }, Item { sku: "B2", qty: 1 }];
//! assert_eq!(to_string(&items)?, "[2]{sku,qty}:\n  A1,2\n  B2,1");
//!
//! // The serializer can also be driven directly.
//! let mut serializer = Serializer::new(ToonOptions::new());
//! vec![1, 2, 3, 4, 5].serialize(&mut serializer)?;
//! assert_eq!(serializer.into_inner(), "[5]: 1,2,3,4,5");
//! # Ok::<(), serde_toon::Error>(())
//! ```

mod emit;
mod ir;
mod number;
mod value;

pub use self::value::{SerializeMap, SerializeVec, ValueSerializer};

use self::ir::{to_key, to_node, value_without_key, Entry, Node, Text};
use crate::{Delimiter, Error, Result, ToonOptions};
use serde::{ser, Serialize};
use std::num::NonZeroUsize;

/// Spaces per indentation level; never zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Indent(NonZeroUsize);

/// [`ToonOptions`] after validation: what the encoder actually uses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EncodeConfig {
    indent: Indent,
    delimiter: Delimiter,
}

/// Why a [`ToonOptions`] cannot be used for encoding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum InvalidOptions {
    ZeroIndent,
}

impl From<InvalidOptions> for Error {
    fn from(invalid: InvalidOptions) -> Self {
        match invalid {
            InvalidOptions::ZeroIndent => {
                Error::custom("invalid ToonOptions: indent must be at least 1 space")
            }
        }
    }
}

impl TryFrom<&ToonOptions> for EncodeConfig {
    type Error = InvalidOptions;

    fn try_from(options: &ToonOptions) -> std::result::Result<Self, InvalidOptions> {
        let indent = NonZeroUsize::new(options.indent).ok_or(InvalidOptions::ZeroIndent)?;
        Ok(EncodeConfig {
            indent: Indent(indent),
            delimiter: options.delimiter.clone(),
        })
    }
}

impl EncodeConfig {
    pub(crate) fn indent(&self) -> usize {
        self.indent.0.get()
    }

    pub(crate) fn delimiter(&self) -> &Delimiter {
        &self.delimiter
    }

    pub(crate) fn delimiter_char(&self) -> char {
        char::from(self.delimiter.as_byte())
    }

    pub(crate) fn header_symbol(&self) -> &'static str {
        self.delimiter.header_symbol()
    }
}

/// The TOON serializer.
///
/// Serializing a value through `&mut Serializer` appends its TOON document
/// to the serializer's output; [`Serializer::into_inner`] returns the text.
/// The [module documentation](self) describes how Rust values map to TOON.
///
/// # Errors
///
/// Serializing returns an error if the options have `indent == 0`, if a map
/// key is not a string, char, integer, or boolean, or if a `Serialize` impl
/// reports one.
///
/// # Examples
///
/// ```rust
/// use serde::Serialize;
/// use serde_toon::{Delimiter, Serializer, ToonOptions};
///
/// let mut serializer = Serializer::new(ToonOptions::new().with_delimiter(Delimiter::Pipe));
/// ["a", "b"].serialize(&mut serializer)?;
/// assert_eq!(serializer.into_inner(), "[2|]: a|b");
///
/// let mut invalid = Serializer::new(ToonOptions::new().with_indent(0));
/// assert!(1.serialize(&mut invalid).is_err());
/// # Ok::<(), serde_toon::Error>(())
/// ```
#[derive(Debug)]
pub struct Serializer {
    output: String,
    /// Backing storage for the strings of the value being serialized.
    arena: String,
    config: std::result::Result<EncodeConfig, InvalidOptions>,
}

impl Serializer {
    /// Creates a serializer with the given options.
    ///
    /// The options are validated here; invalid options (`indent == 0`) make
    /// every serialization through this serializer return an error.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use serde_toon::{Serializer, ToonOptions};
    ///
    /// let serializer = Serializer::new(ToonOptions::new().with_indent(4));
    /// assert_eq!(serializer.into_inner(), "");
    /// ```
    pub fn new(options: ToonOptions) -> Self {
        Serializer {
            output: String::new(),
            arena: String::new(),
            config: EncodeConfig::try_from(&options),
        }
    }

    /// Consumes the serializer and returns the TOON text written so far.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use serde::Serialize;
    /// use serde_toon::{Serializer, ToonOptions};
    ///
    /// let mut serializer = Serializer::new(ToonOptions::new());
    /// true.serialize(&mut serializer)?;
    /// assert_eq!(serializer.into_inner(), "true");
    /// # Ok::<(), serde_toon::Error>(())
    /// ```
    pub fn into_inner(self) -> String {
        self.output
    }

    /// Fails early when the options are invalid.
    fn check(&self) -> Result<()> {
        match &self.config {
            Ok(_) => Ok(()),
            Err(invalid) => Err((*invalid).into()),
        }
    }

    /// Records a value into a node, using this serializer's string arena.
    fn node<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<Node> {
        to_node(&mut self.arena, value)
    }

    /// Writes a root string document without recording it first.
    fn emit_str(&mut self, s: &str) -> Result<()> {
        let config = self
            .config
            .as_ref()
            .map_err(|invalid| Error::from(*invalid))?;
        emit::Writer::new(&mut self.output, &self.arena, config).root_string(s);
        Ok(())
    }

    /// Writes one field of a root object in nested form, whose document
    /// began at output length `start`.
    fn write_root_field(&mut self, start: usize, key: &Text, value: &Node) -> Result<()> {
        let config = self
            .config
            .as_ref()
            .map_err(|invalid| Error::from(*invalid))?;
        let mut writer = emit::Writer::new(&mut self.output, &self.arena, config);
        writer.root_field(start, key, value);
        Ok(())
    }

    /// Writes `root` as a document and resets the arena.
    fn emit(&mut self, root: Node) -> Result<()> {
        let config = self
            .config
            .as_ref()
            .map_err(|invalid| Error::from(*invalid))?;
        self.output.reserve(self.arena.len() + 64);
        emit::Writer::new(&mut self.output, &self.arena, config).document(&root);
        self.arena.clear();
        Ok(())
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

    fn serialize_bool(self, v: bool) -> Result<()> {
        self.emit(Node::Bool(v))
    }

    fn serialize_i8(self, v: i8) -> Result<()> {
        self.emit(Node::Int(v.into()))
    }

    fn serialize_i16(self, v: i16) -> Result<()> {
        self.emit(Node::Int(v.into()))
    }

    fn serialize_i32(self, v: i32) -> Result<()> {
        self.emit(Node::Int(v.into()))
    }

    fn serialize_i64(self, v: i64) -> Result<()> {
        self.emit(Node::Int(v))
    }

    fn serialize_i128(self, v: i128) -> Result<()> {
        let node = self.node(&v)?;
        self.emit(node)
    }

    fn serialize_u8(self, v: u8) -> Result<()> {
        self.emit(Node::UInt(v.into()))
    }

    fn serialize_u16(self, v: u16) -> Result<()> {
        self.emit(Node::UInt(v.into()))
    }

    fn serialize_u32(self, v: u32) -> Result<()> {
        self.emit(Node::UInt(v.into()))
    }

    fn serialize_u64(self, v: u64) -> Result<()> {
        self.emit(Node::UInt(v))
    }

    fn serialize_u128(self, v: u128) -> Result<()> {
        let node = self.node(&v)?;
        self.emit(node)
    }

    fn serialize_f32(self, v: f32) -> Result<()> {
        self.emit(Node::Float32(v))
    }

    fn serialize_f64(self, v: f64) -> Result<()> {
        self.emit(Node::Float(v))
    }

    fn serialize_char(self, v: char) -> Result<()> {
        self.emit_str(v.encode_utf8(&mut [0u8; 4]))
    }

    fn serialize_str(self, v: &str) -> Result<()> {
        self.emit_str(v)
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<()> {
        self.emit(Node::Array(
            v.iter().map(|&b| Node::UInt(b.into())).collect(),
        ))
    }

    fn serialize_none(self) -> Result<()> {
        self.emit(Node::Null)
    }

    fn serialize_some<T>(self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<()> {
        self.emit(Node::Null)
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> {
        self.emit(Node::Null)
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<()> {
        self.emit(Node::Str(Text::Static(variant)))
    }

    fn serialize_newtype_struct<T>(self, _name: &'static str, value: &T) -> Result<()>
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
    ) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.check()?;
        let inner = self.node(value)?;
        self.emit(Node::Object(vec![(Text::Static(variant), inner)]))
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<SeqSerializer<'a>> {
        self.check()?;
        Ok(SeqSerializer {
            items: Vec::with_capacity(len.unwrap_or(0)),
            ser: self,
        })
    }

    fn serialize_tuple(self, len: usize) -> Result<TupleSerializer<'a>> {
        self.check()?;
        Ok(TupleSerializer {
            items: Vec::with_capacity(len),
            ser: self,
        })
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<TupleStructSerializer<'a>> {
        self.check()?;
        Ok(TupleStructSerializer {
            items: Vec::with_capacity(len),
            ser: self,
        })
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<TupleVariantSerializer<'a>> {
        self.check()?;
        Ok(TupleVariantSerializer {
            variant,
            items: Vec::with_capacity(len),
            ser: self,
        })
    }

    fn serialize_map(self, len: Option<usize>) -> Result<MapSerializer<'a>> {
        self.check()?;
        Ok(MapSerializer {
            object: RootObject::new(&mut self.output, len.unwrap_or(0)),
            key: None,
            ser: self,
        })
    }

    fn serialize_struct(self, _name: &'static str, len: usize) -> Result<StructSerializer<'a>> {
        self.check()?;
        Ok(StructSerializer {
            object: RootObject::new(&mut self.output, len),
            ser: self,
        })
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<StructVariantSerializer<'a>> {
        self.check()?;
        Ok(StructVariantSerializer {
            variant,
            entries: Vec::with_capacity(len),
            ser: self,
        })
    }
}

/// Serializes a sequence as a TOON document. Returned by
/// `serialize_seq` on [`&mut Serializer`](Serializer).
#[derive(Debug)]
pub struct SeqSerializer<'a> {
    ser: &'a mut Serializer,
    items: Vec<Node>,
}

/// Serializes a tuple as a TOON array document. Returned by
/// `serialize_tuple` on [`&mut Serializer`](Serializer).
#[derive(Debug)]
pub struct TupleSerializer<'a> {
    ser: &'a mut Serializer,
    items: Vec<Node>,
}

/// Serializes a tuple struct as a TOON array document. Returned by
/// `serialize_tuple_struct` on [`&mut Serializer`](Serializer).
#[derive(Debug)]
pub struct TupleStructSerializer<'a> {
    ser: &'a mut Serializer,
    items: Vec<Node>,
}

/// Serializes a tuple variant as the single-key object `Variant[N]: ...`.
/// Returned by `serialize_tuple_variant` on [`&mut Serializer`](Serializer).
#[derive(Debug)]
pub struct TupleVariantSerializer<'a> {
    ser: &'a mut Serializer,
    variant: &'static str,
    items: Vec<Node>,
}

/// Serializes a map as a TOON object document. Returned by
/// `serialize_map` on [`&mut Serializer`](Serializer).
#[derive(Debug)]
pub struct MapSerializer<'a> {
    ser: &'a mut Serializer,
    object: RootObject,
    key: Option<Text>,
}

/// Serializes a struct as a TOON object document. Returned by
/// `serialize_struct` on [`&mut Serializer`](Serializer).
#[derive(Debug)]
pub struct StructSerializer<'a> {
    ser: &'a mut Serializer,
    object: RootObject,
}

/// A root object being serialized.
///
/// Only an object whose values are all non-empty objects can take the keyed
/// tabular form (§9.5), so entries are buffered only while that is still
/// possible. From the first other value on, the object is known to be in
/// nested form (§8) and each field is written as soon as it is complete,
/// without keeping its node.
#[derive(Debug)]
enum RootObject {
    /// Every value so far is a non-empty object.
    Buffering(Vec<Entry>),
    /// The object is in nested form; fields are written directly. `start` is
    /// the output length where this document began.
    Streaming { start: usize },
}

impl RootObject {
    /// Starts a root object of about `len` fields, reserving room in
    /// `output` for a typical line per field. The buffer allocates only if an
    /// object value arrives first.
    fn new(output: &mut String, len: usize) -> Self {
        output.reserve(len.saturating_mul(24).clamp(64, 4096));
        RootObject::Buffering(Vec::new())
    }

    fn field(&mut self, ser: &mut Serializer, key: Text, value: Node) -> Result<()> {
        match self {
            RootObject::Buffering(entries) if value.as_nonempty_object().is_some() => {
                entries.push((key, value));
                Ok(())
            }
            RootObject::Buffering(entries) => {
                let start = ser.output.len();
                let buffered = std::mem::take(entries);
                *self = RootObject::Streaming { start };
                for (key, value) in &buffered {
                    ser.write_root_field(start, key, value)?;
                }
                ser.write_root_field(start, &key, &value)?;
                ser.arena.clear();
                Ok(())
            }
            RootObject::Streaming { start } => {
                ser.write_root_field(*start, &key, &value)?;
                ser.arena.clear();
                Ok(())
            }
        }
    }

    fn end(self, ser: &mut Serializer) -> Result<()> {
        match self {
            RootObject::Buffering(entries) => ser.emit(Node::Object(entries)),
            RootObject::Streaming { .. } => Ok(()),
        }
    }
}

/// Serializes a struct variant as the single-key object `Variant:` with the
/// fields nested beneath. Returned by `serialize_struct_variant` on
/// [`&mut Serializer`](Serializer).
#[derive(Debug)]
pub struct StructVariantSerializer<'a> {
    ser: &'a mut Serializer,
    variant: &'static str,
    entries: Vec<Entry>,
}

/// Implements the element-collecting half of a sequence-like serializer.
macro_rules! seq_like {
    ($ty:ident, $trait:ident, $method:ident) => {
        impl ser::$trait for $ty<'_> {
            type Ok = ();
            type Error = Error;

            fn $method<T>(&mut self, value: &T) -> Result<()>
            where
                T: ?Sized + Serialize,
            {
                let node = self.ser.node(value)?;
                self.items.push(node);
                Ok(())
            }

            fn end(self) -> Result<()> {
                self.ser.emit(Node::Array(self.items))
            }
        }
    };
}

seq_like!(SeqSerializer, SerializeSeq, serialize_element);
seq_like!(TupleSerializer, SerializeTuple, serialize_element);
seq_like!(TupleStructSerializer, SerializeTupleStruct, serialize_field);

impl ser::SerializeTupleVariant for TupleVariantSerializer<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        let node = self.ser.node(value)?;
        self.items.push(node);
        Ok(())
    }

    fn end(self) -> Result<()> {
        let array = Node::Array(self.items);
        self.ser
            .emit(Node::Object(vec![(Text::Static(self.variant), array)]))
    }
}

impl ser::SerializeMap for MapSerializer<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_key<T>(&mut self, key: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.key = Some(to_key(&mut self.ser.arena, key)?);
        Ok(())
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        let key = self.key.take().ok_or_else(value_without_key)?;
        let node = self.ser.node(value)?;
        self.object.field(self.ser, key, node)
    }

    fn end(self) -> Result<()> {
        self.object.end(self.ser)
    }
}

impl ser::SerializeStruct for StructSerializer<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        let node = self.ser.node(value)?;
        self.object.field(self.ser, Text::Static(key), node)
    }

    fn end(self) -> Result<()> {
        self.object.end(self.ser)
    }
}

impl ser::SerializeStructVariant for StructVariantSerializer<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        let node = self.ser.node(value)?;
        self.entries.push((Text::Static(key), node));
        Ok(())
    }

    fn end(self) -> Result<()> {
        let object = Node::Object(self.entries);
        self.ser
            .emit(Node::Object(vec![(Text::Static(self.variant), object)]))
    }
}

#[cfg(test)]
mod tests;
