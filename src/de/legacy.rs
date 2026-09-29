//! serde_toon 0.2 compatibility that needs more than a lexical tweak.
//!
//! Only reachable in [`DecodeOptions::compatible`](crate::DecodeOptions::compatible)
//! mode. The lexical compat forms (`[#N]`, `[N    ]`, `key: [N]: …`, `NaN`)
//! live next to the spec rules they relax, each behind an `is_compatible()`
//! check.
//!
//! serde_toon 0.2 wrote a root struct variant as `Variant:` directly followed
//! by the variant's first field on the same line, and its remaining fields
//! unindented:
//!
//! ```text
//! Rect:w: 2
//! h: 3.5
//! ```
//!
//! As TOON that is an object with two keys, which cannot be an enum. When a
//! root enum is requested and the document has that shape, the variant name
//! is split off the first line and the rest of the document is decoded as
//! the variant's content.

use super::header::{classify, LineKind};
use super::scalar::KeyDeserializer;
use super::Deserializer;
use crate::{Error, Result};
use serde::de::{self, DeserializeSeed, Visitor};
use std::borrow::Cow;

/// If the document is a 0.2 root enum with its content starting on the
/// variant's line, splits off and returns the variant name; the first line
/// then holds only the content.
pub(super) fn root_enum_variant<'de>(de: &mut Deserializer<'de>) -> Result<Option<Cow<'de, str>>> {
    let Some(&first) = de.peek() else {
        return Ok(None);
    };
    let more_root_lines = de.lines[de.pos + 1..].iter().any(|l| l.depth == 0);
    if first.depth != 0 || !more_root_lines {
        return Ok(None);
    }
    let LineKind::KeyValue { key, value } = classify(first.content, &first, &de.options)? else {
        return Ok(None);
    };
    if value.is_empty() || value.starts_with('"') {
        return Ok(None);
    }
    let line = &mut de.lines[de.pos];
    line.lead = first.col_of(value) - 1;
    line.content = value;
    Ok(Some(key))
}

/// The enum whose variant was split off by [`root_enum_variant`]; its
/// content is the rest of the document.
pub(super) struct RootEnum<'a, 'de> {
    pub de: &'a mut Deserializer<'de>,
    pub variant: Cow<'de, str>,
}

impl<'a, 'de> de::EnumAccess<'de> for RootEnum<'a, 'de> {
    type Error = Error;
    type Variant = Self;

    fn variant_seed<V: DeserializeSeed<'de>>(self, seed: V) -> Result<(V::Value, Self)> {
        let key = KeyDeserializer {
            key: self.variant.clone(),
            line: 1,
            col: 1,
        };
        Ok((seed.deserialize(key)?, self))
    }
}

impl<'a, 'de> de::VariantAccess<'de> for RootEnum<'a, 'de> {
    type Error = Error;

    fn unit_variant(self) -> Result<()> {
        Err(Error::syntax(
            1,
            1,
            &format!("unit variant `{}` must not have content", self.variant),
        ))
    }

    fn newtype_variant_seed<T: DeserializeSeed<'de>>(self, seed: T) -> Result<T::Value> {
        seed.deserialize(self.de)
    }

    fn tuple_variant<V: Visitor<'de>>(self, _len: usize, visitor: V) -> Result<V::Value> {
        de::Deserializer::deserialize_seq(self.de, visitor)
    }

    fn struct_variant<V: Visitor<'de>>(
        self,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value> {
        de::Deserializer::deserialize_map(self.de, visitor)
    }
}
