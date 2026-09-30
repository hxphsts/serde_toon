//! TOON deserialization.
//!
//! This module provides the [`Deserializer`], a direct
//! [`serde::Deserializer`] over TOON text (specification v4.1). It never
//! builds an intermediate tree: one pre-pass splits the input into typed
//! lines, and serde's visitors then walk those lines by depth, borrowing
//! strings from the input whenever they contain no escapes.
//!
//! Most users should use the functions in the crate root:
//!
//! ```rust
//! use serde::Deserialize;
//!
//! #[derive(Deserialize, Debug, PartialEq)]
//! struct User<'a> {
//!     id: u32,
//!     name: &'a str, // borrowed from the input: no allocation
//!     tags: Vec<String>,
//! }
//!
//! let toon = "id: 7\nname: Ada\ntags[2]: admin,ops";
//! let user: User = serde_toon::from_str(toon)?;
//! assert_eq!(user, User { id: 7, name: "Ada", tags: vec!["admin".into(), "ops".into()] });
//! # Ok::<(), serde_toon::Error>(())
//! ```
//!
//! # Decoding modes
//!
//! See [`DecodeOptions`]: `strict` enforces every check of spec §14,
//! `lenient` is the spec's non-strict mode, and `compatible` (the default,
//! used by [`from_str`](crate::from_str)) additionally reads everything
//! serde_toon 0.2 wrote: `[#N]` length markers, tab headers written as
//! spaces (`[2    ]`), headers after a key's colon (`key: [2]: a,b`),
//! `NaN`/`Infinity`/`inf` float tokens and the 0.2 enum layout.
//!
//! # Implementation-defined behavior (spec §4, §12, §13.2, §15)
//!
//! - **Numbers.** Unquoted tokens matching the §4 grammar decode losslessly
//!   as `i64`, then `u64`, then `i128`/`u128`; other numbers (fractions,
//!   exponents, or integers beyond 128 bits) decode as the nearest `f64`.
//!   `-0` decodes as `0`. Integer targets use checked conversion: an
//!   out-of-range value is an [`Error::TypeMismatch`], never truncated.
//! - **Typed hints are accommodating.** A string target accepts the text of
//!   an unquoted number or boolean (`date: 2024-01-01`, `zip: 01234`,
//!   `code: 123`), and float targets accept integers. `deserialize_any`
//!   (e.g. [`Value`](crate::Value), `serde_json::Value`) types tokens exactly
//!   as §4 prescribes.
//! - **Key order and duplicates.** Keys are handed to the visitor in document
//!   order. In strict mode duplicate sibling keys are an error; otherwise
//!   every occurrence is passed on and the visitor decides (maps keep the
//!   last value; `#[derive(Deserialize)]` structs report a duplicate field).
//! - **Tabs.** Non-strict modes accept tabs in indentation; each leading tab
//!   counts as one indentation level (`indent_size` spaces).
//! - **Indentation.** Non-strict modes treat the first line's indentation as
//!   the root level and, when a nested block is indented deeper than one
//!   level, adopt that deeper level for the whole block.
//! - **Nesting limit.** Nesting deeper than 128 objects/arrays is reported
//!   as an error instead of exhausting the stack.
//! - **Trailing content** after a complete root array, keyed root object or
//!   root primitive is an error in every mode.

mod header;
mod legacy;
mod line;
mod scalar;

use self::header::{classify, fallback, parse_legacy_value_header, Field, Header, LineKind};
use self::line::{find_unquoted, find_unquoted2, trim_spaces, Line};
use self::scalar::{decode_key, locate, KeyDeserializer, ScalarDeserializer, Token};
use crate::{DecodeOptions, Error, Result};
use serde::de::{self, DeserializeSeed, IgnoredAny, Visitor};
use std::borrow::Cow;
use std::collections::HashSet;
use std::fmt;
use std::marker::PhantomData;

/// Maximum nesting depth of objects and arrays.
const MAX_DEPTH: usize = 128;

/// The TOON deserializer.
///
/// Created with [`Deserializer::from_str`] (default,
/// [`DecodeOptions::compatible`]) or [`Deserializer::from_str_with_options`];
/// most code uses [`from_str`](crate::from_str) instead.
///
/// # Examples
///
/// ```rust
/// use serde::Deserialize;
/// use serde_toon::{DecodeOptions, Deserializer};
///
/// #[derive(Deserialize, Debug, PartialEq)]
/// struct Point { x: i32, y: i32 }
///
/// let mut de = Deserializer::from_str_with_options("x: 1\ny: 2", DecodeOptions::strict());
/// assert_eq!(Point::deserialize(&mut de)?, Point { x: 1, y: 2 });
/// # Ok::<(), serde_toon::Error>(())
/// ```
pub struct Deserializer<'de> {
    lines: Vec<Line<'de>>,
    pos: usize,
    options: DecodeOptions,
    /// An error found by the pre-pass, reported by the first `deserialize_*`.
    pending_error: Option<Error>,
    remaining_depth: usize,
    /// Whether the cursor is inside a header span (§12), where blank lines
    /// are a strict-mode error.
    in_span: bool,
}

impl fmt::Debug for Deserializer<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Deserializer")
            .field("options", &self.options)
            .field("lines", &self.lines.len())
            .field("position", &self.pos)
            .field("next_line", &self.peek().map(|l| l.number))
            .finish()
    }
}

impl<'de> Deserializer<'de> {
    /// Creates a deserializer for `input` with the default
    /// [`DecodeOptions`] ([`DecodeOptions::compatible`]).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use serde::Deserialize;
    ///
    /// let mut de = serde_toon::Deserializer::from_str("[3]: 1,2,3");
    /// assert_eq!(Vec::<u8>::deserialize(&mut de)?, [1, 2, 3]);
    /// # Ok::<(), serde_toon::Error>(())
    /// ```
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(input: &'de str) -> Self {
        Self::from_str_with_options(input, DecodeOptions::default())
    }

    /// Creates a deserializer that decodes `input` using the given
    /// [`DecodeOptions`].
    ///
    /// Problems found while splitting the input into lines (for example an
    /// `indent_size` of zero, or a tab used for indentation in strict mode)
    /// are reported by the first `deserialize_*` call.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use serde::Deserialize;
    /// use serde_toon::{DecodeOptions, Deserializer};
    ///
    /// let mut de = Deserializer::from_str_with_options("xs[3]: 1,2", DecodeOptions::strict());
    /// assert!(Vec::<u8>::deserialize(&mut de).is_err()); // declared 3, found 2
    /// ```
    pub fn from_str_with_options(input: &'de str, options: DecodeOptions) -> Self {
        let (lines, pending_error) = match line::split_lines(input, &options) {
            Ok(lines) => (lines, None),
            Err(e) => (Vec::new(), Some(e)),
        };
        Deserializer {
            lines,
            pos: 0,
            options,
            pending_error,
            remaining_depth: MAX_DEPTH,
            in_span: false,
        }
    }

    fn strict(&self) -> bool {
        self.options.is_strict()
    }

    fn compat(&self) -> bool {
        self.options.is_compatible()
    }

    fn peek(&self) -> Option<&Line<'de>> {
        self.lines.get(self.pos)
    }

    /// Consumes the next line, enforcing the §12 header-span blank-line rule.
    fn next_line(&mut self) -> Result<Line<'de>> {
        let line = self.lines[self.pos];
        self.pos += 1;
        if line.after_blank && self.in_span && self.strict() {
            // Report the line after the gap: it is what the context shows,
            // and the gap may span several blank or comment lines.
            let (number, col) = line.pos();
            return Err(Error::syntax_with_context(
                number,
                col,
                "blank line before this line, inside an array or keyed object",
                line.content,
                Some("remove the blank line; blank lines are only allowed between fields"),
            ));
        }
        Ok(line)
    }

    fn enter(&mut self, line: &Line<'_>) -> Result<()> {
        if self.remaining_depth == 0 {
            return Err(Error::invalid_format(
                line.number,
                line.lead + 1,
                &format!("nesting exceeds the maximum depth of {MAX_DEPTH}"),
            ));
        }
        self.remaining_depth -= 1;
        Ok(())
    }

    fn leave(&mut self) {
        self.remaining_depth += 1;
    }

    /// Content depth of the scope opened by a line standing at `parent`, or
    /// `None` if the scope is empty (§8).
    fn child_depth(&self, parent: usize) -> Result<Option<usize>> {
        match self.peek() {
            Some(line) if line.depth > parent => {
                if self.strict() && line.depth != parent + 1 {
                    return Err(self.indentation(
                        line,
                        parent + 1,
                        "indentation jumps more than one level",
                    ));
                }
                Ok(Some(line.depth))
            }
            _ => Ok(None),
        }
    }

    fn indentation(&self, line: &Line<'_>, depth: usize, msg: &str) -> Error {
        let size = self.options.indent_size();
        let mut err = Error::indentation_error(
            line.number,
            line.lead + 1,
            depth * size,
            line.lead,
            line.content,
        );
        if let Error::IndentationError { context, .. } = &mut err {
            *context = format!("{msg}: {}", line.content);
        }
        err
    }

    /// Handles the next line, which is deeper than the content depth
    /// `depth` of its enclosing scope and belongs to no scope (§8).
    fn orphan(&mut self, depth: usize) -> Result<()> {
        let line = self.lines[self.pos];
        if self.strict() {
            return Err(self.indentation(
                &line,
                depth,
                "line is indented deeper than its enclosing block",
            ));
        }
        // Non-strict decoders may skip it, but a scalar line outside the
        // root position is an error in every mode (§14.2).
        if line.list_item().is_none()
            && matches!(
                classify(line.content, &line, &self.options)?,
                LineKind::Scalar
            )
        {
            return Err(missing_colon(&line));
        }
        self.pos += 1;
        Ok(())
    }

    /// Discovers the root form (§5) and returns it as a node.
    fn root(&mut self) -> Result<Node<'de>> {
        if let Some(err) = self.pending_error.take() {
            return Err(err);
        }
        let Some(&first) = self.peek() else {
            return Ok(Node::EmptyObject((1, 1)));
        };
        if first.depth != 0 {
            return Err(self.indentation(&first, 0, "the first line must not be indented"));
        }
        self.pos += 1;
        if first.content == "[]" {
            return Ok(Node::EmptyArray(first.pos()));
        }
        let kind = classify(first.content, &first, &self.options)?;
        Ok(match kind {
            LineKind::Header(header) if header.key.is_none() => Node::array(header, 0, first),
            LineKind::Scalar if self.pos == self.lines.len() => {
                Node::Scalar(Token::new(first.content, &first))
            }
            kind => Node::Fields {
                depth: 0,
                first: (first, kind),
            },
        })
    }

    /// Errors if any line is left after the root value (§5).
    fn end(&mut self) -> Result<()> {
        match self.peek() {
            None => Ok(()),
            Some(line) => Err(Error::syntax_with_context(
                line.number,
                line.lead + 1,
                "unexpected content after the end of the document's root value",
                line.content,
                Some("a root array, keyed object or primitive must be the whole document"),
            )),
        }
    }

    /// The node for the value of a key-value line whose key stands at
    /// `depth`.
    fn value_node(&self, value: &'de str, depth: usize, line: Line<'de>) -> Node<'de> {
        if value.is_empty() {
            return Node::Block {
                parent: depth,
                line,
            };
        }
        if value == "[]" {
            return Node::EmptyArray((line.number, line.col_of(value)));
        }
        if self.compat() && value.starts_with('[') {
            if let Some(header) = parse_legacy_value_header(value, &line, &self.options) {
                return Node::array(Box::new(header), depth, line);
            }
        }
        Node::Scalar(Token::new(value, &line))
    }

    /// The node for a list item whose text after the marker is `rest`, on
    /// `line` at item depth `depth` (§9.2, §9.4, §10).
    fn item_node(&self, rest: &'de str, depth: usize, line: Line<'de>) -> Result<Node<'de>> {
        if rest.is_empty() {
            return Ok(if self.strict() {
                Node::EmptyObject(line.pos())
            } else {
                Node::Block {
                    parent: depth,
                    line,
                }
            });
        }
        if rest == "[]" {
            return Ok(Node::EmptyArray((line.number, line.col_of(rest))));
        }
        Ok(match classify(rest, &line, &self.options)? {
            LineKind::Header(header) if header.key.is_none() => {
                if self.strict() && header.fields.is_some() {
                    return Err(Error::syntax_with_context(
                        line.number,
                        line.col_of(rest),
                        "a keyless header with a field list is only valid at the document root",
                        line.content,
                        Some("use `- [N]:` with list items, or give the header a key"),
                    ));
                }
                // A keyless header is the item itself: items at depth + 1.
                Node::array(header, depth, line)
            }
            LineKind::Scalar => Node::Scalar(Token::new(rest, &line)),
            // An object whose first field is on the hyphen line: its fields
            // stand at depth + 1 (§10).
            kind => Node::Fields {
                depth: depth + 1,
                first: (line, kind),
            },
        })
    }
}

/// Decodes byte input as UTF-8 (§4), reporting the line and column of the
/// first ill-formed sequence.
pub(crate) fn str_from_utf8(bytes: &[u8]) -> Result<&str> {
    std::str::from_utf8(bytes).map_err(|e| {
        let valid = &bytes[..e.valid_up_to()];
        let line = valid.iter().filter(|&&b| b == b'\n').count() + 1;
        let line_start = valid.iter().rposition(|&b| b == b'\n').map_or(0, |i| i + 1);
        let col = String::from_utf8_lossy(&valid[line_start..])
            .chars()
            .count()
            + 1;
        Error::syntax(line, col, &format!("invalid UTF-8: {e}"))
    })
}

fn missing_colon(line: &Line<'_>) -> Error {
    Error::syntax_with_context(
        line.number,
        line.lead + 1,
        "expected `key: value`, found a bare value (missing colon)",
        line.content,
        Some("a bare value is only valid as the entire document"),
    )
}

fn count_error(line: &Line<'_>, what: &str, declared: usize, found: usize) -> Error {
    Error::invalid_format(
        line.number,
        line.lead + 1,
        &format!("{what}: header declares {declared}, found {found}"),
    )
}

// ---------------------------------------------------------------------------
// Nodes: what the next value is
// ---------------------------------------------------------------------------

/// A 1-based line and column.
type Pos = (usize, usize);

/// A value located in the document, not yet deserialized.
enum Node<'de> {
    /// A primitive token.
    Scalar(Token<'de>),
    /// `key: []`, `- []` or a root `[]`.
    EmptyArray(Pos),
    /// An empty object (empty document, bare `-` list item).
    EmptyObject(Pos),
    /// `key:` with nothing after the colon: an object whose fields are the
    /// lines below `parent`, if any.
    Block { parent: usize, line: Line<'de> },
    /// An object whose fields stand at `depth`, the first already
    /// classified (root object, list-item object).
    Fields {
        depth: usize,
        first: (Line<'de>, LineKind<'de>),
    },
    /// An array header standing at `parent`.
    Array {
        header: Box<Header<'de>>,
        parent: usize,
        line: Line<'de>,
    },
    /// A keyed tabular header standing at `parent` (§9.5).
    Keyed {
        header: Box<Header<'de>>,
        parent: usize,
        line: Line<'de>,
    },
}

impl<'de> Node<'de> {
    fn array(header: Box<Header<'de>>, parent: usize, line: Line<'de>) -> Self {
        if header.keyed {
            Node::Keyed {
                header,
                parent,
                line,
            }
        } else {
            Node::Array {
                header,
                parent,
                line,
            }
        }
    }
}

/// Deserializes one [`Node`].
struct ValueDeserializer<'a, 'de> {
    de: &'a mut Deserializer<'de>,
    node: Node<'de>,
}

impl<'a, 'de> ValueDeserializer<'a, 'de> {
    fn scalar(token: Token<'de>, de: &Deserializer<'de>) -> ScalarDeserializer<'de> {
        ScalarDeserializer::new(token, de.compat())
    }

    /// Resolves an object node into its field depth, first field and
    /// opening line, or `None` if it is an empty `Block`.
    fn object(self) -> Result<ObjectParts<'a, 'de>> {
        match self.node {
            Node::Block { parent, line } => Ok(self
                .de
                .child_depth(parent)?
                .map(|depth| (self.de, depth, None, line))),
            Node::Fields { depth, first } => {
                let line = first.0;
                Ok(Some((self.de, depth, Some(first), line)))
            }
            _ => unreachable!("object() called on a non-object node"),
        }
    }
}

type ObjectParts<'a, 'de> = Option<(&'a mut Deserializer<'de>, usize, FirstField<'de>, Line<'de>)>;

type FirstField<'de> = Option<(Line<'de>, LineKind<'de>)>;

macro_rules! scalar_hint {
    ($($method:ident)*) => {$(
        fn $method<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
            match self.node {
                Node::Scalar(token) => Self::scalar(token, self.de).$method(visitor),
                _ => self.deserialize_any(visitor),
            }
        }
    )*};
}

impl<'a, 'de> de::Deserializer<'de> for ValueDeserializer<'a, 'de> {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        match self.node {
            Node::Scalar(token) => Self::scalar(token, self.de).deserialize_any(visitor),
            Node::EmptyArray((line, col)) => {
                visitor.visit_seq(Empty).map_err(|e| locate(e, line, col))
            }
            Node::EmptyObject((line, col)) => {
                visitor.visit_map(Empty).map_err(|e| locate(e, line, col))
            }
            Node::Block { .. } | Node::Fields { .. } => match self.object()? {
                None => visitor.visit_map(Empty),
                Some((de, depth, first, line)) => {
                    de.enter(&line)?;
                    let mut map = ObjectMap::new(de, depth, first);
                    let value = visitor
                        .visit_map(&mut map)
                        .map_err(|e| locate(e, line.number, line.lead + 1))?;
                    de.leave();
                    Ok(value)
                }
            },
            Node::Array {
                header,
                parent,
                line,
            } => {
                self.de.enter(&line)?;
                let mut seq = ArraySeq::new(self.de, header, parent, line)?;
                let value = visitor
                    .visit_seq(&mut seq)
                    .map_err(|e| locate(e, line.number, line.lead + 1))?;
                seq.finish()?;
                self.de.leave();
                Ok(value)
            }
            Node::Keyed {
                header,
                parent,
                line,
            } => {
                self.de.enter(&line)?;
                let mut map = KeyedMap::new(self.de, header, parent, line)?;
                let value = visitor
                    .visit_map(&mut map)
                    .map_err(|e| locate(e, line.number, line.lead + 1))?;
                map.finish()?;
                self.de.leave();
                Ok(value)
            }
        }
    }

    scalar_hint! {
        deserialize_bool deserialize_i8 deserialize_i16 deserialize_i32 deserialize_i64
        deserialize_i128 deserialize_u8 deserialize_u16 deserialize_u32 deserialize_u64
        deserialize_u128 deserialize_f32 deserialize_f64 deserialize_char deserialize_str
        deserialize_string deserialize_bytes deserialize_byte_buf
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        match &self.node {
            Node::Scalar(token) if token.text == "null" => visitor.visit_none(),
            _ => visitor.visit_some(self),
        }
    }

    fn deserialize_unit<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        match self.node {
            Node::Scalar(token) => Self::scalar(token, self.de).deserialize_unit(visitor),
            // An empty document or `key:` decodes to `{}`, which is how a
            // unit (struct) is naturally written.
            Node::EmptyObject(_) => visitor.visit_unit(),
            Node::Block { parent, .. } if self.de.child_depth(parent)?.is_none() => {
                visitor.visit_unit()
            }
            _ => self.deserialize_any(visitor),
        }
    }

    fn deserialize_unit_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value> {
        self.deserialize_unit(visitor)
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        name: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value> {
        match self.node {
            Node::Scalar(token) => {
                Self::scalar(token, self.de).deserialize_enum(name, variants, visitor)
            }
            Node::Block { line, .. }
            | Node::Fields {
                first: (line, _), ..
            } => match self.object()? {
                None => Err(Error::type_mismatch(
                    line.number,
                    line.lead + 1,
                    "enum variant",
                    "empty object",
                )),
                Some((de, depth, first, line)) => {
                    de.enter(&line)?;
                    let map = ObjectMap::new(de, depth, first);
                    let value = visitor
                        .visit_enum(MapEnum { map, line })
                        .map_err(|e| locate(e, line.number, line.lead + 1))?;
                    de.leave();
                    Ok(value)
                }
            },
            _ => self.deserialize_any(visitor),
        }
    }

    fn deserialize_ignored_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        match self.node {
            Node::Scalar(_) => visitor.visit_unit(),
            _ => self.deserialize_any(visitor),
        }
    }

    serde::forward_to_deserialize_any! {
        seq tuple tuple_struct map struct identifier
    }
}

// ---------------------------------------------------------------------------
// Empty containers
// ---------------------------------------------------------------------------

struct Empty;

impl<'de> de::SeqAccess<'de> for Empty {
    type Error = Error;
    fn next_element_seed<T: DeserializeSeed<'de>>(&mut self, _: T) -> Result<Option<T::Value>> {
        Ok(None)
    }
    fn size_hint(&self) -> Option<usize> {
        Some(0)
    }
}

impl<'de> de::MapAccess<'de> for Empty {
    type Error = Error;
    fn next_key_seed<K: DeserializeSeed<'de>>(&mut self, _: K) -> Result<Option<K::Value>> {
        Ok(None)
    }
    fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, _: V) -> Result<V::Value> {
        Err(Error::custom("next_value_seed called before next_key_seed"))
    }
    fn size_hint(&self) -> Option<usize> {
        Some(0)
    }
}

// ---------------------------------------------------------------------------
// Objects (§8, §10)
// ---------------------------------------------------------------------------

/// The fields of an object: the lines at `depth` until the depth decreases.
struct ObjectMap<'a, 'de> {
    de: &'a mut Deserializer<'de>,
    depth: usize,
    first: FirstField<'de>,
    pending: Option<Node<'de>>,
    /// Keys seen so far, for the strict duplicate-key check (§14.3).
    seen: Option<HashSet<Cow<'de, str>>>,
}

impl<'a, 'de> ObjectMap<'a, 'de> {
    fn new(de: &'a mut Deserializer<'de>, depth: usize, first: FirstField<'de>) -> Self {
        let seen = de.strict().then(HashSet::new);
        ObjectMap {
            de,
            depth,
            first,
            pending: None,
            seen,
        }
    }

    /// The next field line at this object's depth, classified.
    fn next_field(&mut self) -> Result<Option<(Line<'de>, LineKind<'de>)>> {
        if let Some(first) = self.first.take() {
            return Ok(Some(first));
        }
        loop {
            let Some(line) = self.de.peek() else {
                return Ok(None);
            };
            if line.depth < self.depth {
                return Ok(None);
            }
            if line.depth > self.depth {
                self.de.orphan(self.depth)?;
                continue;
            }
            let line = self.de.next_line()?;
            let kind = classify(line.content, &line, &self.de.options)?;
            return Ok(Some((line, kind)));
        }
    }
}

impl<'a, 'de> de::MapAccess<'de> for ObjectMap<'a, 'de> {
    type Error = Error;

    fn next_key_seed<K: DeserializeSeed<'de>>(&mut self, seed: K) -> Result<Option<K::Value>> {
        let Some((line, mut kind)) = self.next_field()? else {
            return Ok(None);
        };
        if let LineKind::Header(header) = &kind {
            if header.key.is_none() {
                // §6: a keyless header is not valid in field position.
                if self.de.strict() {
                    return Err(Error::syntax_with_context(
                        line.number,
                        line.lead + 1,
                        "array header without a key in object field position",
                        line.content,
                        Some("keyless headers are only valid at the document root or after `- `"),
                    ));
                }
                kind = fallback(line.content, &line)?;
            }
        }
        let (key, node) = match kind {
            LineKind::Header(mut header) => {
                let key = header.key.take().unwrap_or_default();
                (key, Node::array(header, self.depth, line))
            }
            LineKind::KeyValue { key, value } => (key, self.de.value_node(value, self.depth, line)),
            LineKind::Scalar => return Err(missing_colon(&line)),
        };
        if let Some(seen) = &mut self.seen {
            if !seen.insert(key.clone()) {
                return Err(Error::syntax_with_context(
                    line.number,
                    line.lead + 1,
                    &format!("duplicate key `{key}`"),
                    line.content,
                    Some("strict mode rejects repeated sibling keys"),
                ));
            }
        }
        self.pending = Some(node);
        seed.deserialize(KeyDeserializer {
            key,
            line: line.number,
            col: line.lead + 1,
        })
        .map(Some)
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value> {
        let node = self
            .pending
            .take()
            .ok_or_else(|| Error::custom("next_value_seed called before next_key_seed"))?;
        seed.deserialize(ValueDeserializer {
            de: &mut *self.de,
            node,
        })
    }
}

/// An externally tagged enum written as a single-key object
/// (`Variant: value`, or `Variant:` with a nested block).
struct MapEnum<'a, 'de> {
    map: ObjectMap<'a, 'de>,
    line: Line<'de>,
}

impl<'a, 'de> MapEnum<'a, 'de> {
    fn value(&mut self) -> Result<ValueDeserializer<'_, 'de>> {
        let node = self
            .map
            .pending
            .take()
            .ok_or_else(|| Error::custom("enum variant value requested before the variant"))?;
        Ok(ValueDeserializer {
            de: &mut *self.map.de,
            node,
        })
    }

    /// The object must hold exactly one key.
    fn end(mut self) -> Result<()> {
        use de::MapAccess;
        match self.map.next_key_seed(PhantomData::<IgnoredAny>)? {
            None => Ok(()),
            Some(_) => Err(Error::syntax_with_context(
                self.line.number,
                self.line.lead + 1,
                "an enum must be written as an object with exactly one key",
                self.line.content,
                None,
            )),
        }
    }
}

impl<'a, 'de> de::EnumAccess<'de> for MapEnum<'a, 'de> {
    type Error = Error;
    type Variant = Self;

    fn variant_seed<V: DeserializeSeed<'de>>(mut self, seed: V) -> Result<(V::Value, Self)> {
        use de::MapAccess;
        match self.map.next_key_seed(seed)? {
            Some(variant) => Ok((variant, self)),
            None => Err(Error::type_mismatch(
                self.line.number,
                self.line.lead + 1,
                "enum variant",
                "empty object",
            )),
        }
    }
}

impl<'a, 'de> de::VariantAccess<'de> for MapEnum<'a, 'de> {
    type Error = Error;

    fn unit_variant(mut self) -> Result<()> {
        <() as de::Deserialize>::deserialize(self.value()?)?;
        self.end()
    }

    fn newtype_variant_seed<T: DeserializeSeed<'de>>(mut self, seed: T) -> Result<T::Value> {
        let value = seed.deserialize(self.value()?)?;
        self.end()?;
        Ok(value)
    }

    fn tuple_variant<V: Visitor<'de>>(mut self, _len: usize, visitor: V) -> Result<V::Value> {
        let value = de::Deserializer::deserialize_seq(self.value()?, visitor)?;
        self.end()?;
        Ok(value)
    }

    fn struct_variant<V: Visitor<'de>>(
        mut self,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value> {
        let value = de::Deserializer::deserialize_map(self.value()?, visitor)?;
        self.end()?;
        Ok(value)
    }
}

// ---------------------------------------------------------------------------
// Arrays (§9.1-§9.4)
// ---------------------------------------------------------------------------

/// Delimiter-separated cells of an inline array, row or entry row, split
/// lazily and quote-aware on the active delimiter only (§11.2).
#[derive(Clone)]
struct Cells<'de> {
    rest: Option<&'de str>,
    delimiter: u8,
    line: Line<'de>,
}

impl<'de> Cells<'de> {
    fn new(text: &'de str, delimiter: u8, line: Line<'de>) -> Self {
        // An empty cell sequence is zero cells, not one empty cell.
        let rest = (!trim_spaces(text).is_empty()).then_some(text);
        Cells {
            rest,
            delimiter,
            line,
        }
    }

    fn is_empty(&self) -> bool {
        self.rest.is_none()
    }
}

impl<'de> Iterator for Cells<'de> {
    type Item = Token<'de>;

    fn next(&mut self) -> Option<Token<'de>> {
        let s = self.rest?;
        let cell = match find_unquoted(s, self.delimiter) {
            Some(i) => {
                self.rest = Some(&s[i + 1..]);
                &s[..i]
            }
            None => {
                self.rest = None;
                s
            }
        };
        Some(Token::new(trim_spaces(cell), &self.line))
    }
}

enum ArrayKind<'de> {
    Inline(Cells<'de>),
    /// List items (§9.2, §9.4) at the given depth.
    List(Option<usize>),
    /// Tabular rows (§9.3) at the given depth.
    Rows(Option<usize>),
}

struct ArraySeq<'a, 'de> {
    de: &'a mut Deserializer<'de>,
    header: Box<Header<'de>>,
    line: Line<'de>,
    kind: ArrayKind<'de>,
    count: usize,
    /// The enclosing header-span state, saved when this span began.
    saved_span: Option<bool>,
}

impl<'a, 'de> ArraySeq<'a, 'de> {
    fn new(
        de: &'a mut Deserializer<'de>,
        header: Box<Header<'de>>,
        parent: usize,
        line: Line<'de>,
    ) -> Result<Self> {
        let kind = if header.fields.is_some() {
            ArrayKind::Rows(de.child_depth(parent)?)
        } else if !header.inline.is_empty() {
            ArrayKind::Inline(Cells::new(header.inline, header.delimiter, line))
        } else {
            // `key[N]:` with nothing after the colon is list form (§9.1).
            ArrayKind::List(de.child_depth(parent)?)
        };
        Ok(ArraySeq {
            de,
            header,
            line,
            kind,
            count: 0,
            saved_span: None,
        })
    }

    /// Consumes a line of this array's scope; the first one starts the
    /// header span (§12).
    fn take_line(&mut self) -> Result<Line<'de>> {
        let line = self.de.next_line()?;
        if self.saved_span.is_none() {
            self.saved_span = Some(self.de.in_span);
            self.de.in_span = true;
        }
        Ok(line)
    }

    /// Checks the scope after the visitor is done.
    fn finish(mut self) -> Result<()> {
        if let Some(saved) = self.saved_span {
            self.de.in_span = saved;
        }
        let extra = match &mut self.kind {
            ArrayKind::Inline(cells) => cells.next().is_some(),
            ArrayKind::List(Some(depth)) => self
                .de
                .peek()
                .is_some_and(|l| l.depth == *depth && l.list_item().is_some()),
            ArrayKind::Rows(Some(depth)) => self
                .de
                .peek()
                .is_some_and(|l| l.depth == *depth && is_row(l, self.header.delimiter)),
            _ => false,
        };
        if extra {
            return Err(Error::invalid_format(
                self.line.number,
                self.line.lead + 1,
                &format!("array has more than the {} elements expected", self.count),
            ));
        }
        if self.de.strict() && self.count != self.header.len {
            let what = match self.kind {
                ArrayKind::Inline(_) => "inline array length mismatch",
                ArrayKind::List(_) => "list item count mismatch",
                ArrayKind::Rows(_) => "tabular row count mismatch",
            };
            return Err(count_error(&self.line, what, self.header.len, self.count));
        }
        Ok(())
    }
}

impl<'a, 'de> de::SeqAccess<'de> for ArraySeq<'a, 'de> {
    type Error = Error;

    fn next_element_seed<T: DeserializeSeed<'de>>(&mut self, seed: T) -> Result<Option<T::Value>> {
        let depth = match &mut self.kind {
            ArrayKind::Inline(cells) => {
                let Some(token) = cells.next() else {
                    return Ok(None);
                };
                self.count += 1;
                return seed
                    .deserialize(ScalarDeserializer::new(token, self.de.compat()))
                    .map(Some);
            }
            ArrayKind::List(None) | ArrayKind::Rows(None) => return Ok(None),
            ArrayKind::List(Some(d)) | ArrayKind::Rows(Some(d)) => *d,
        };
        let rows = matches!(self.kind, ArrayKind::Rows(_));
        loop {
            let Some(line) = self.de.peek() else {
                return Ok(None);
            };
            if line.depth < depth {
                return Ok(None);
            }
            if line.depth > depth {
                self.de.orphan(depth)?;
                continue;
            }
            if rows {
                if !is_row(line, self.header.delimiter) {
                    // A key-value line ends the rows (§9.3).
                    return Ok(None);
                }
                let line = self.take_line()?;
                self.count += 1;
                let fields = self.header.fields.as_deref().unwrap_or(&[]);
                let mut cells = Cells::new(line.content, self.header.delimiter, line);
                if self.de.strict() {
                    check_width(&cells, self.header.leaf_count, &line)?;
                }
                return seed
                    .deserialize(RowDeserializer {
                        fields,
                        cells: &mut cells,
                        compat: self.de.compat(),
                        line: &line,
                    })
                    .map(Some);
            }
            let Some(rest) = line.list_item() else {
                // A non-item line at item depth ends the list (§9.4).
                return Ok(None);
            };
            let line = self.take_line()?;
            self.count += 1;
            let node = self.de.item_node(rest, depth, line)?;
            return seed
                .deserialize(ValueDeserializer {
                    de: &mut *self.de,
                    node,
                })
                .map(Some);
        }
    }

    fn size_hint(&self) -> Option<usize> {
        // Never trust a declared length for allocation (§15).
        let bound = match &self.kind {
            ArrayKind::Inline(cells) => cells.rest.map_or(0, |s| s.len() + 1),
            ArrayKind::List(_) | ArrayKind::Rows(_) => self.de.lines.len() - self.de.pos,
        };
        Some(self.header.len.saturating_sub(self.count).min(bound))
    }
}

fn check_width(cells: &Cells<'_>, expected: usize, line: &Line<'_>) -> Result<()> {
    let found = cells.clone().count();
    if found == expected {
        Ok(())
    } else {
        Err(count_error(line, "row width mismatch", expected, found))
    }
}

/// Whether a line at row depth is a row of a table with the given active
/// delimiter (§9.3 disambiguation): it has no unquoted colon, or its first
/// unquoted delimiter precedes its first unquoted colon.
fn is_row(line: &Line<'_>, delimiter: u8) -> bool {
    !matches!(
        find_unquoted2(line.content, delimiter, b':'),
        Some((_, b':'))
    )
}

// ---------------------------------------------------------------------------
// Tabular rows (§9.3, §9.5)
// ---------------------------------------------------------------------------

/// One row (or entry row) as an object built from the header's field tree.
struct RowDeserializer<'s, 'de> {
    fields: &'s [Field<'de>],
    cells: &'s mut Cells<'de>,
    compat: bool,
    line: &'s Line<'de>,
}

impl<'s, 'de> de::Deserializer<'de> for RowDeserializer<'s, 'de> {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        let (number, col) = (self.line.number, self.line.lead + 1);
        visitor
            .visit_map(RowMap {
                fields: self.fields.iter(),
                cells: self.cells,
                compat: self.compat,
                line: self.line,
                pending: None,
            })
            .map_err(|e| locate(e, number, col))
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_some(self)
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value> {
        visitor.visit_newtype_struct(self)
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf unit unit_struct seq tuple tuple_struct map struct enum
        identifier ignored_any
    }
}

enum RowValue<'s, 'de> {
    Cell(Token<'de>),
    Group(&'s [Field<'de>]),
}

struct RowMap<'s, 'de> {
    fields: std::slice::Iter<'s, Field<'de>>,
    cells: &'s mut Cells<'de>,
    compat: bool,
    line: &'s Line<'de>,
    pending: Option<RowValue<'s, 'de>>,
}

impl<'s, 'de> de::MapAccess<'de> for RowMap<'s, 'de> {
    type Error = Error;

    fn next_key_seed<K: DeserializeSeed<'de>>(&mut self, seed: K) -> Result<Option<K::Value>> {
        let Some(field) = self.fields.next() else {
            return Ok(None);
        };
        // Non-strict width mismatch: fields without a cell are absent (§14.1).
        let value = match &field.children {
            None => match self.cells.next() {
                Some(token) => RowValue::Cell(token),
                None => return Ok(None),
            },
            Some(_) if self.cells.is_empty() => return Ok(None),
            Some(children) => RowValue::Group(children),
        };
        self.pending = Some(value);
        seed.deserialize(KeyDeserializer {
            key: field.name.clone(),
            line: self.line.number,
            col: self.line.lead + 1,
        })
        .map(Some)
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value> {
        match self.pending.take() {
            Some(RowValue::Cell(token)) => {
                seed.deserialize(ScalarDeserializer::new(token, self.compat))
            }
            Some(RowValue::Group(fields)) => seed.deserialize(RowDeserializer {
                fields,
                cells: &mut *self.cells,
                compat: self.compat,
                line: self.line,
            }),
            None => Err(Error::custom("next_value_seed called before next_key_seed")),
        }
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.fields.len())
    }
}

// ---------------------------------------------------------------------------
// Keyed tabular objects (§9.5)
// ---------------------------------------------------------------------------

struct KeyedMap<'a, 'de> {
    de: &'a mut Deserializer<'de>,
    header: Box<Header<'de>>,
    line: Line<'de>,
    depth: Option<usize>,
    count: usize,
    saved_span: Option<bool>,
    pending: Option<(Cells<'de>, Line<'de>)>,
    seen: Option<HashSet<Cow<'de, str>>>,
}

impl<'a, 'de> KeyedMap<'a, 'de> {
    fn new(
        de: &'a mut Deserializer<'de>,
        header: Box<Header<'de>>,
        parent: usize,
        line: Line<'de>,
    ) -> Result<Self> {
        let depth = de.child_depth(parent)?;
        let seen = de.strict().then(HashSet::new);
        Ok(KeyedMap {
            de,
            header,
            line,
            depth,
            count: 0,
            saved_span: None,
            pending: None,
            seen,
        })
    }

    fn finish(self) -> Result<()> {
        if let Some(saved) = self.saved_span {
            self.de.in_span = saved;
        }
        if self.de.strict() && self.count != self.header.len {
            return Err(count_error(
                &self.line,
                "keyed entry count mismatch",
                self.header.len,
                self.count,
            ));
        }
        Ok(())
    }
}

impl<'a, 'de> de::MapAccess<'de> for KeyedMap<'a, 'de> {
    type Error = Error;

    fn next_key_seed<K: DeserializeSeed<'de>>(&mut self, seed: K) -> Result<Option<K::Value>> {
        let Some(depth) = self.depth else {
            return Ok(None);
        };
        loop {
            let Some(&line) = self.de.peek() else {
                return Ok(None);
            };
            if line.depth < depth {
                return Ok(None);
            }
            if line.depth > depth {
                self.de.orphan(depth)?;
                continue;
            }
            // Every line at entry depth with an unquoted colon is an entry
            // row (§9.5); others are an error (strict) or skipped.
            let Some(colon) = find_unquoted(line.content, b':') else {
                if self.de.strict() {
                    return Err(missing_colon(&line));
                }
                self.de.pos += 1;
                continue;
            };
            let line = self.de.next_line()?;
            if self.saved_span.is_none() {
                self.saved_span = Some(self.de.in_span);
                self.de.in_span = true;
            }
            let key = decode_key(&line.content[..colon], &line)?;
            let cells = Cells::new(
                trim_spaces(&line.content[colon + 1..]),
                self.header.delimiter,
                line,
            );
            if let Some(seen) = &mut self.seen {
                check_width(&cells, self.header.leaf_count, &line)?;
                if !seen.insert(key.clone()) {
                    return Err(Error::syntax_with_context(
                        line.number,
                        line.lead + 1,
                        &format!("duplicate entry key `{key}`"),
                        line.content,
                        Some("strict mode rejects repeated sibling keys"),
                    ));
                }
            }
            self.count += 1;
            self.pending = Some((cells, line));
            return seed
                .deserialize(KeyDeserializer {
                    key,
                    line: line.number,
                    col: line.lead + 1,
                })
                .map(Some);
        }
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value> {
        let (mut cells, line) = self
            .pending
            .take()
            .ok_or_else(|| Error::custom("next_value_seed called before next_key_seed"))?;
        seed.deserialize(RowDeserializer {
            fields: self.header.fields.as_deref().unwrap_or(&[]),
            cells: &mut cells,
            compat: self.de.compat(),
            line: &line,
        })
    }
}

// ---------------------------------------------------------------------------
// The public deserializer: root dispatch
// ---------------------------------------------------------------------------

macro_rules! root_method {
    ($($method:ident($($arg:ident: $ty:ty),*))*) => {$(
        fn $method<V: Visitor<'de>>(self, $($arg: $ty,)* visitor: V) -> Result<V::Value> {
            let node = self.root()?;
            let value = ValueDeserializer { de: &mut *self, node }.$method($($arg,)* visitor)?;
            self.end()?;
            Ok(value)
        }
    )*};
}

impl<'de> de::Deserializer<'de> for &mut Deserializer<'de> {
    type Error = Error;

    root_method! {
        deserialize_any()
        deserialize_bool()
        deserialize_i8()
        deserialize_i16()
        deserialize_i32()
        deserialize_i64()
        deserialize_i128()
        deserialize_u8()
        deserialize_u16()
        deserialize_u32()
        deserialize_u64()
        deserialize_u128()
        deserialize_f32()
        deserialize_f64()
        deserialize_char()
        deserialize_str()
        deserialize_string()
        deserialize_bytes()
        deserialize_byte_buf()
        deserialize_option()
        deserialize_unit()
        deserialize_unit_struct(name: &'static str)
        deserialize_newtype_struct(name: &'static str)
        deserialize_seq()
        deserialize_tuple(len: usize)
        deserialize_tuple_struct(name: &'static str, len: usize)
        deserialize_map()
        deserialize_struct(name: &'static str, fields: &'static [&'static str])
        deserialize_identifier()
        deserialize_ignored_any()
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        name: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value> {
        if self.compat() && self.pending_error.is_none() {
            if let Some(variant) = legacy::root_enum_variant(self)? {
                let value = visitor.visit_enum(legacy::RootEnum {
                    de: &mut *self,
                    variant,
                })?;
                self.end()?;
                return Ok(value);
            }
        }
        let node = self.root()?;
        let value = ValueDeserializer {
            de: &mut *self,
            node,
        }
        .deserialize_enum(name, variants, visitor)?;
        self.end()?;
        Ok(value)
    }
}
