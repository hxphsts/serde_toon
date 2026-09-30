//! The encoder's private intermediate representation.
//!
//! Serde drives a [`NodeSerializer`] that records the value as a lean
//! [`Node`] tree in a single pass. TOON's forms (tabular arrays, keyed
//! tabular objects) depend on the shape of whole subtrees, so the encoder
//! needs this lookahead before it can write anything; [`super::emit`] then
//! classifies and writes the tree in one pass.
//!
//! Strings are not allocated individually: their bytes are appended to one
//! arena `String` and nodes refer to them by range ([`Text::Arena`]). Struct
//! field names and variant names are `&'static str` and are never copied
//! ([`Text::Static`]).

use super::number;
use crate::{Error, Result};
use serde::{ser, Serialize};
use std::fmt::Write as _;

/// A string recorded by the builder: either borrowed for `'static` or a byte
/// range of the arena.
#[derive(Debug, Clone)]
pub(crate) enum Text {
    Static(&'static str),
    Arena { start: usize, end: usize },
}

impl Text {
    /// Appends `s` to `arena` and returns a reference to it.
    pub(crate) fn push(arena: &mut String, s: &str) -> Text {
        if arena.capacity() == 0 {
            // Skip the smallest growth steps: most documents hold at least
            // a few strings.
            arena.reserve(s.len().max(128));
        }
        let start = arena.len();
        arena.push_str(s);
        Text::Arena {
            start,
            end: arena.len(),
        }
    }

    /// Resolves the text against the arena it was recorded in.
    pub(crate) fn resolve<'a>(&'a self, arena: &'a str) -> &'a str {
        match *self {
            Text::Static(s) => s,
            Text::Arena { start, end } => &arena[start..end],
        }
    }
}

/// An object entry: key and value.
pub(crate) type Entry = (Text, Node);

/// A value normalized to the JSON data model (spec §2, §3).
#[derive(Debug)]
pub(crate) enum Node {
    Null,
    Bool(bool),
    Int(i64),
    UInt(u64),
    Float(f64),
    Float32(f32),
    Str(Text),
    /// An integer too wide for `i64`/`u64`, already rendered in decimal.
    BigInt(Text),
    Array(Vec<Node>),
    Object(Vec<Entry>),
}

impl Node {
    /// Primitive per spec §1.6: string, number, boolean, or null.
    pub(crate) fn is_primitive(&self) -> bool {
        !matches!(self, Node::Array(_) | Node::Object(_))
    }

    /// The entries of a non-empty object, the only kind of value that can
    /// be a tabular row or a nested field group.
    pub(crate) fn as_nonempty_object(&self) -> Option<&[Entry]> {
        match self {
            Node::Object(entries) if !entries.is_empty() => Some(entries),
            _ => None,
        }
    }

    /// Wraps `self` as the externally tagged `{variant: self}` object.
    fn tagged(self, variant: Option<&'static str>) -> Node {
        match variant {
            Some(v) => Node::Object(vec![(Text::Static(v), self)]),
            None => self,
        }
    }
}

fn i128_node(arena: &mut String, v: i128) -> Node {
    if let Ok(v) = i64::try_from(v) {
        Node::Int(v)
    } else if let Ok(v) = u64::try_from(v) {
        Node::UInt(v)
    } else {
        let start = arena.len();
        let _ = write!(arena, "{v}");
        Node::BigInt(Text::Arena {
            start,
            end: arena.len(),
        })
    }
}

fn u128_node(arena: &mut String, v: u128) -> Node {
    match u64::try_from(v) {
        Ok(v) => Node::UInt(v),
        Err(_) => {
            let start = arena.len();
            let _ = write!(arena, "{v}");
            Node::BigInt(Text::Arena {
                start,
                end: arena.len(),
            })
        }
    }
}

/// Builds a [`Node`] from any `Serialize` value.
pub(crate) fn to_node<T: ?Sized + Serialize>(arena: &mut String, value: &T) -> Result<Node> {
    value.serialize(NodeSerializer { arena })
}

/// Records a map key (spec §3: map-like collections become objects with
/// string keys). Strings and chars are used as-is; integers and booleans are
/// written in their TOON primitive form.
pub(crate) fn to_key<T: ?Sized + Serialize>(arena: &mut String, key: &T) -> Result<Text> {
    key.serialize(KeySerializer { arena })
}

/// Serde serializer producing a [`Node`].
pub(crate) struct NodeSerializer<'b> {
    arena: &'b mut String,
}

impl<'b> ser::Serializer for NodeSerializer<'b> {
    type Ok = Node;
    type Error = Error;
    type SerializeSeq = SeqBuilder<'b>;
    type SerializeTuple = SeqBuilder<'b>;
    type SerializeTupleStruct = SeqBuilder<'b>;
    type SerializeTupleVariant = SeqBuilder<'b>;
    type SerializeMap = MapBuilder<'b>;
    type SerializeStruct = MapBuilder<'b>;
    type SerializeStructVariant = MapBuilder<'b>;

    fn serialize_bool(self, v: bool) -> Result<Node> {
        Ok(Node::Bool(v))
    }
    fn serialize_i8(self, v: i8) -> Result<Node> {
        Ok(Node::Int(v.into()))
    }
    fn serialize_i16(self, v: i16) -> Result<Node> {
        Ok(Node::Int(v.into()))
    }
    fn serialize_i32(self, v: i32) -> Result<Node> {
        Ok(Node::Int(v.into()))
    }
    fn serialize_i64(self, v: i64) -> Result<Node> {
        Ok(Node::Int(v))
    }
    fn serialize_i128(self, v: i128) -> Result<Node> {
        Ok(i128_node(self.arena, v))
    }
    fn serialize_u8(self, v: u8) -> Result<Node> {
        Ok(Node::UInt(v.into()))
    }
    fn serialize_u16(self, v: u16) -> Result<Node> {
        Ok(Node::UInt(v.into()))
    }
    fn serialize_u32(self, v: u32) -> Result<Node> {
        Ok(Node::UInt(v.into()))
    }
    fn serialize_u64(self, v: u64) -> Result<Node> {
        Ok(Node::UInt(v))
    }
    fn serialize_u128(self, v: u128) -> Result<Node> {
        Ok(u128_node(self.arena, v))
    }
    fn serialize_f32(self, v: f32) -> Result<Node> {
        Ok(Node::Float32(v))
    }
    fn serialize_f64(self, v: f64) -> Result<Node> {
        Ok(Node::Float(v))
    }
    fn serialize_char(self, v: char) -> Result<Node> {
        let mut buf = [0u8; 4];
        Ok(Node::Str(Text::push(self.arena, v.encode_utf8(&mut buf))))
    }
    fn serialize_str(self, v: &str) -> Result<Node> {
        Ok(Node::Str(Text::push(self.arena, v)))
    }
    fn serialize_bytes(self, v: &[u8]) -> Result<Node> {
        Ok(Node::Array(
            v.iter().map(|&b| Node::UInt(b.into())).collect(),
        ))
    }
    fn serialize_none(self) -> Result<Node> {
        Ok(Node::Null)
    }
    fn serialize_some<T: ?Sized + Serialize>(self, value: &T) -> Result<Node> {
        value.serialize(self)
    }
    fn serialize_unit(self) -> Result<Node> {
        Ok(Node::Null)
    }
    fn serialize_unit_struct(self, _name: &'static str) -> Result<Node> {
        Ok(Node::Null)
    }
    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
    ) -> Result<Node> {
        Ok(Node::Str(Text::Static(variant)))
    }
    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Node> {
        value.serialize(self)
    }
    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Node> {
        Ok(value.serialize(self)?.tagged(Some(variant)))
    }
    fn serialize_seq(self, len: Option<usize>) -> Result<SeqBuilder<'b>> {
        Ok(SeqBuilder::new(self.arena, len.unwrap_or(0), None))
    }
    fn serialize_tuple(self, len: usize) -> Result<SeqBuilder<'b>> {
        Ok(SeqBuilder::new(self.arena, len, None))
    }
    fn serialize_tuple_struct(self, _name: &'static str, len: usize) -> Result<SeqBuilder<'b>> {
        Ok(SeqBuilder::new(self.arena, len, None))
    }
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<SeqBuilder<'b>> {
        Ok(SeqBuilder::new(self.arena, len, Some(variant)))
    }
    fn serialize_map(self, len: Option<usize>) -> Result<MapBuilder<'b>> {
        Ok(MapBuilder::new(self.arena, len.unwrap_or(0), None))
    }
    fn serialize_struct(self, _name: &'static str, len: usize) -> Result<MapBuilder<'b>> {
        Ok(MapBuilder::new(self.arena, len, None))
    }
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<MapBuilder<'b>> {
        Ok(MapBuilder::new(self.arena, len, Some(variant)))
    }
}

/// Collects the elements of a sequence, tuple, or tuple variant.
pub(crate) struct SeqBuilder<'b> {
    arena: &'b mut String,
    items: Vec<Node>,
    variant: Option<&'static str>,
}

impl<'b> SeqBuilder<'b> {
    fn new(arena: &'b mut String, len: usize, variant: Option<&'static str>) -> Self {
        SeqBuilder {
            arena,
            items: Vec::with_capacity(len),
            variant,
        }
    }

    fn push<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<()> {
        self.items.push(to_node(self.arena, value)?);
        Ok(())
    }

    fn finish(self) -> Result<Node> {
        Ok(Node::Array(self.items).tagged(self.variant))
    }
}

impl ser::SerializeSeq for SeqBuilder<'_> {
    type Ok = Node;
    type Error = Error;
    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<()> {
        self.push(value)
    }
    fn end(self) -> Result<Node> {
        self.finish()
    }
}

impl ser::SerializeTuple for SeqBuilder<'_> {
    type Ok = Node;
    type Error = Error;
    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<()> {
        self.push(value)
    }
    fn end(self) -> Result<Node> {
        self.finish()
    }
}

impl ser::SerializeTupleStruct for SeqBuilder<'_> {
    type Ok = Node;
    type Error = Error;
    fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<()> {
        self.push(value)
    }
    fn end(self) -> Result<Node> {
        self.finish()
    }
}

impl ser::SerializeTupleVariant for SeqBuilder<'_> {
    type Ok = Node;
    type Error = Error;
    fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<()> {
        self.push(value)
    }
    fn end(self) -> Result<Node> {
        self.finish()
    }
}

/// Collects the entries of a map, struct, or struct variant.
pub(crate) struct MapBuilder<'b> {
    arena: &'b mut String,
    entries: Vec<Entry>,
    key: Option<Text>,
    variant: Option<&'static str>,
}

impl<'b> MapBuilder<'b> {
    fn new(arena: &'b mut String, len: usize, variant: Option<&'static str>) -> Self {
        MapBuilder {
            arena,
            entries: Vec::with_capacity(len),
            key: None,
            variant,
        }
    }

    fn field<T: ?Sized + Serialize>(&mut self, key: &'static str, value: &T) -> Result<()> {
        let value = to_node(self.arena, value)?;
        self.entries.push((Text::Static(key), value));
        Ok(())
    }

    fn finish(self) -> Result<Node> {
        Ok(Node::Object(self.entries).tagged(self.variant))
    }
}

impl ser::SerializeMap for MapBuilder<'_> {
    type Ok = Node;
    type Error = Error;
    fn serialize_key<T: ?Sized + Serialize>(&mut self, key: &T) -> Result<()> {
        self.key = Some(to_key(self.arena, key)?);
        Ok(())
    }
    fn serialize_value<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<()> {
        let key = self.key.take().ok_or_else(value_without_key)?;
        let value = to_node(self.arena, value)?;
        self.entries.push((key, value));
        Ok(())
    }
    fn end(self) -> Result<Node> {
        self.finish()
    }
}

impl ser::SerializeStruct for MapBuilder<'_> {
    type Ok = Node;
    type Error = Error;
    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<()> {
        self.field(key, value)
    }
    fn end(self) -> Result<Node> {
        self.finish()
    }
}

impl ser::SerializeStructVariant for MapBuilder<'_> {
    type Ok = Node;
    type Error = Error;
    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<()> {
        self.field(key, value)
    }
    fn end(self) -> Result<Node> {
        self.finish()
    }
}

pub(crate) fn value_without_key() -> Error {
    Error::custom("serialize_value called without serialize_key")
}

fn invalid_key(kind: &str) -> Error {
    Error::custom(format_args!(
        "map keys must be strings, chars, integers, or booleans; found {kind}"
    ))
}

/// Serde serializer for map keys, producing the key text.
struct KeySerializer<'b> {
    arena: &'b mut String,
}

impl KeySerializer<'_> {
    fn integer(self, v: i64) -> Text {
        let start = self.arena.len();
        number::push_i64(self.arena, v);
        Text::Arena {
            start,
            end: self.arena.len(),
        }
    }

    fn unsigned(self, v: u64) -> Text {
        let start = self.arena.len();
        number::push_u64(self.arena, v);
        Text::Arena {
            start,
            end: self.arena.len(),
        }
    }

    fn display(self, v: impl std::fmt::Display) -> Text {
        let start = self.arena.len();
        let _ = write!(self.arena, "{v}");
        Text::Arena {
            start,
            end: self.arena.len(),
        }
    }
}

impl ser::Serializer for KeySerializer<'_> {
    type Ok = Text;
    type Error = Error;
    type SerializeSeq = ser::Impossible<Text, Error>;
    type SerializeTuple = ser::Impossible<Text, Error>;
    type SerializeTupleStruct = ser::Impossible<Text, Error>;
    type SerializeTupleVariant = ser::Impossible<Text, Error>;
    type SerializeMap = ser::Impossible<Text, Error>;
    type SerializeStruct = ser::Impossible<Text, Error>;
    type SerializeStructVariant = ser::Impossible<Text, Error>;

    fn serialize_bool(self, v: bool) -> Result<Text> {
        Ok(Text::Static(if v { "true" } else { "false" }))
    }
    fn serialize_i8(self, v: i8) -> Result<Text> {
        Ok(self.integer(v.into()))
    }
    fn serialize_i16(self, v: i16) -> Result<Text> {
        Ok(self.integer(v.into()))
    }
    fn serialize_i32(self, v: i32) -> Result<Text> {
        Ok(self.integer(v.into()))
    }
    fn serialize_i64(self, v: i64) -> Result<Text> {
        Ok(self.integer(v))
    }
    fn serialize_i128(self, v: i128) -> Result<Text> {
        Ok(self.display(v))
    }
    fn serialize_u8(self, v: u8) -> Result<Text> {
        Ok(self.unsigned(v.into()))
    }
    fn serialize_u16(self, v: u16) -> Result<Text> {
        Ok(self.unsigned(v.into()))
    }
    fn serialize_u32(self, v: u32) -> Result<Text> {
        Ok(self.unsigned(v.into()))
    }
    fn serialize_u64(self, v: u64) -> Result<Text> {
        Ok(self.unsigned(v))
    }
    fn serialize_u128(self, v: u128) -> Result<Text> {
        Ok(self.display(v))
    }
    fn serialize_f32(self, _v: f32) -> Result<Text> {
        Err(invalid_key("a float"))
    }
    fn serialize_f64(self, _v: f64) -> Result<Text> {
        Err(invalid_key("a float"))
    }
    fn serialize_char(self, v: char) -> Result<Text> {
        let mut buf = [0u8; 4];
        Ok(Text::push(self.arena, v.encode_utf8(&mut buf)))
    }
    fn serialize_str(self, v: &str) -> Result<Text> {
        Ok(Text::push(self.arena, v))
    }
    fn serialize_bytes(self, _v: &[u8]) -> Result<Text> {
        Err(invalid_key("bytes"))
    }
    fn serialize_none(self) -> Result<Text> {
        Err(invalid_key("null"))
    }
    fn serialize_some<T: ?Sized + Serialize>(self, value: &T) -> Result<Text> {
        value.serialize(self)
    }
    fn serialize_unit(self) -> Result<Text> {
        Err(invalid_key("null"))
    }
    fn serialize_unit_struct(self, _name: &'static str) -> Result<Text> {
        Err(invalid_key("null"))
    }
    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
    ) -> Result<Text> {
        Ok(Text::Static(variant))
    }
    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Text> {
        value.serialize(self)
    }
    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<Text> {
        Err(invalid_key("an enum variant with data"))
    }
    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq> {
        Err(invalid_key("a sequence"))
    }
    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple> {
        Err(invalid_key("a tuple"))
    }
    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        Err(invalid_key("a tuple struct"))
    }
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        Err(invalid_key("an enum variant with data"))
    }
    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap> {
        Err(invalid_key("a map"))
    }
    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<Self::SerializeStruct> {
        Err(invalid_key("a struct"))
    }
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        Err(invalid_key("an enum variant with data"))
    }
}
