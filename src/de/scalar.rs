//! Primitive tokens (TOON spec §4, §7.1, §7.4) and their deserializers.

use super::line::{find_unquoted, trim_spaces, Line};
use crate::lexical::is_number_token;
use crate::{Error, Result};
use serde::de::{self, IntoDeserializer, Visitor};
use std::borrow::Cow;

/// One primitive token: a value after a key-value colon, an inline array
/// element, a row cell or a list-item value. `text` is already trimmed of
/// U+0020 and may be quoted.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Token<'a> {
    pub text: &'a str,
    pub line: usize,
    pub col: usize,
}

impl<'a> Token<'a> {
    pub fn new(text: &'a str, line: &Line<'a>) -> Self {
        Token {
            text,
            line: line.number,
            col: line.col_of(text),
        }
    }

    fn is_quoted(&self) -> bool {
        self.text.as_bytes().first() == Some(&b'"')
    }

    /// Classifies the token per §4.
    pub fn primitive(&self) -> Result<Primitive<'a>> {
        if self.is_quoted() {
            return unquote(self.text, self.line, self.col).map(Primitive::Str);
        }
        Ok(match self.text {
            "null" => Primitive::Null,
            "true" => Primitive::Bool(true),
            "false" => Primitive::Bool(false),
            t if is_number_token(t) => Primitive::Number(t),
            t => Primitive::Str(Cow::Borrowed(t)),
        })
    }

    fn mismatch(&self, expected: &str) -> Error {
        Error::type_mismatch(self.line, self.col, expected, &describe(self.text))
    }
}

/// A classified primitive token (§4).
#[derive(Debug)]
pub(crate) enum Primitive<'a> {
    Null,
    Bool(bool),
    /// Matches the §4 number grammar; kept as text so every numeric target
    /// can convert it losslessly.
    Number(&'a str),
    Str(Cow<'a, str>),
}

/// Short human description of a token for error messages.
fn describe(text: &str) -> String {
    const MAX: usize = 40;
    match text.char_indices().nth(MAX) {
        Some((i, _)) => format!("`{}...`", &text[..i]),
        None => format!("`{text}`"),
    }
}

/// Decodes a quoted token (§7.1). `text` starts with `"`; the closing quote
/// must be its last character (§7.4 quoted-token boundary). Borrows from the
/// input when the string has no escapes.
pub(crate) fn unquote(text: &str, line: usize, col: usize) -> Result<Cow<'_, str>> {
    let body = &text[1..];
    let bytes = body.as_bytes();
    let Some(first) = bytes.iter().position(|&b| b == b'"' || b == b'\\') else {
        return Err(unterminated(text, line, col));
    };
    if bytes[first] == b'"' {
        check_boundary(text, first + 2, line, col)?;
        return Ok(Cow::Borrowed(&body[..first]));
    }

    let mut out = String::with_capacity(body.len());
    let mut i = first;
    out.push_str(&body[..i]);
    loop {
        // `i` is at a quote or backslash.
        if bytes[i] == b'"' {
            check_boundary(text, i + 2, line, col)?;
            return Ok(Cow::Owned(out));
        }
        let esc_col = col + 1 + i;
        let Some(&e) = bytes.get(i + 1) else {
            return Err(unterminated(text, line, col));
        };
        i += 2;
        match e {
            b'\\' => out.push('\\'),
            b'"' => out.push('"'),
            b'n' => out.push('\n'),
            b'r' => out.push('\r'),
            b't' => out.push('\t'),
            b'u' => {
                let hex = body
                    .get(i..i + 4)
                    .filter(|h| h.bytes().all(|b| b.is_ascii_hexdigit()));
                let Some(hex) = hex else {
                    return Err(Error::syntax_with_context(
                        line,
                        esc_col,
                        "invalid unicode escape: \\u must be followed by exactly four hex digits",
                        text,
                        None,
                    ));
                };
                let code = u32::from_str_radix(hex, 16).unwrap_or(0xD800);
                let Some(ch) = char::from_u32(code) else {
                    return Err(Error::syntax_with_context(
                        line,
                        esc_col,
                        &format!("invalid unicode escape \\u{hex}: surrogate code points are not allowed"),
                        text,
                        Some("write characters outside the Basic Multilingual Plane as literal UTF-8"),
                    ));
                };
                out.push(ch);
                i += 4;
            }
            _ => {
                let shown = body[i - 1..].chars().next().unwrap_or('?');
                return Err(Error::syntax_with_context(
                    line,
                    esc_col,
                    &format!("invalid escape sequence \\{shown}"),
                    text,
                    Some("valid escapes are \\\\ \\\" \\n \\r \\t and \\uXXXX"),
                ));
            }
        }
        let Some(next) = bytes[i..].iter().position(|&b| b == b'"' || b == b'\\') else {
            return Err(unterminated(text, line, col));
        };
        out.push_str(&body[i..i + next]);
        i += next;
    }
}

fn unterminated(text: &str, line: usize, col: usize) -> Error {
    Error::syntax_with_context(
        line,
        col,
        "unterminated string",
        text,
        Some("add the closing `\"`"),
    )
}

/// §7.4: the closing quote must be the token's last character.
fn check_boundary(text: &str, end: usize, line: usize, col: usize) -> Result<()> {
    if end == text.len() {
        Ok(())
    } else {
        Err(Error::syntax_with_context(
            line,
            col + end,
            "unexpected characters after the closing quote",
            text,
            Some("quote the whole value, or remove the text after the closing `\"`"),
        ))
    }
}

/// Decodes a key token (§7.4): unquoted text is taken literally, quoted text
/// is unescaped.
pub(crate) fn decode_key<'a>(text: &'a str, line: &Line<'a>) -> Result<Cow<'a, str>> {
    let text = trim_spaces(text);
    if text.as_bytes().first() == Some(&b'"') {
        unquote(text, line.number, line.col_of(text))
    } else {
        Ok(Cow::Borrowed(text))
    }
}

/// Splits a key-value line (or an entry row) at its first unquoted colon.
pub(crate) fn split_key(content: &str) -> Option<(&str, &str)> {
    find_unquoted(content, b':').map(|i| (&content[..i], trim_spaces(&content[i + 1..])))
}

// ---------------------------------------------------------------------------
// Numbers
// ---------------------------------------------------------------------------

/// Fast path for the most common number token: `-?[0-9]{1,18}` without
/// forbidden leading zeros (§4), which always fits an `i64`. Returns `None`
/// for anything else, including valid numbers that need the general path.
fn small_integer(text: &str) -> Option<i64> {
    let b = text.as_bytes();
    let (negative, digits) = match b.first()? {
        b'-' => (true, &b[1..]),
        _ => (false, b),
    };
    if digits.is_empty() || digits.len() > 18 || (digits[0] == b'0' && digits.len() > 1) {
        return None;
    }
    let mut value: i64 = 0;
    for &d in digits {
        if !d.is_ascii_digit() {
            return None;
        }
        value = value * 10 + i64::from(d - b'0');
    }
    Some(if negative { -value } else { value })
}

/// Whether a §4 number token has no fraction or exponent.
fn is_integer_text(text: &str) -> bool {
    !text.bytes().any(|b| matches!(b, b'.' | b'e' | b'E'))
}

/// Parses a number token as `f64`, normalizing `-0` to `0` (§4).
fn parse_f64(text: &str) -> f64 {
    let f: f64 = text.parse().unwrap_or(f64::NAN);
    if f == 0.0 {
        0.0
    } else {
        f
    }
}

/// serde_toon 0.2 wrote non-finite floats as these tokens.
fn legacy_float(text: &str) -> Option<f64> {
    match text {
        "NaN" | "nan" => Some(f64::NAN),
        "Infinity" | "inf" | "+inf" => Some(f64::INFINITY),
        "-Infinity" | "-inf" => Some(f64::NEG_INFINITY),
        _ => None,
    }
}

/// Visits a number token in untyped (`deserialize_any`) position: `i64`,
/// then `u64`, then the nearest `f64`.
///
/// Integers beyond the `u64` range deliberately become `f64` here rather than
/// `i128`/`u128`: many self-describing targets (notably `serde_json::Value`)
/// reject 128-bit visits outright, so a lossless-but-failing visit would be
/// worse than an approximate one. Typed `i128`/`u128` targets still decode
/// such tokens exactly through their own hints.
pub(crate) fn visit_number<'de, V: Visitor<'de>>(text: &str, visitor: V) -> Result<V::Value> {
    if is_integer_text(text) {
        if let Ok(i) = text.parse::<i64>() {
            return visitor.visit_i64(i);
        }
        if let Ok(u) = text.parse::<u64>() {
            return visitor.visit_u64(u);
        }
    }
    visitor.visit_f64(parse_f64(text))
}

/// Integer conversion shared by the typed integer hints.
trait FromToken: Sized {
    const NAME: &'static str;
    fn from_i128(v: i128) -> Option<Self>;
    fn from_u128(v: u128) -> Option<Self>;
    fn from_f64(v: f64) -> Option<Self>;
}

macro_rules! from_token {
    ($($t:ty),*) => {$(
        impl FromToken for $t {
            const NAME: &'static str = stringify!($t);
            fn from_i128(v: i128) -> Option<Self> {
                <$t>::try_from(v).ok()
            }
            fn from_u128(v: u128) -> Option<Self> {
                <$t>::try_from(v).ok()
            }
            fn from_f64(v: f64) -> Option<Self> {
                // Integral values only, and only where f64 is exact.
                if v.fract() != 0.0 || v.abs() > 9_007_199_254_740_992.0 {
                    return None;
                }
                <$t>::try_from(v as i128).ok()
            }
        }
    )*};
}
from_token!(i8, i16, i32, i64, i128, u8, u16, u32, u64, u128);

impl<'a> Token<'a> {
    /// Checked conversion of a number token to an integer type; never
    /// truncates.
    fn integer<T: FromToken>(&self) -> Result<T> {
        if let Some(i) = small_integer(self.text) {
            if let Some(v) = T::from_i128(i128::from(i)) {
                return Ok(v);
            }
        }
        let Primitive::Number(text) = self.primitive()? else {
            return Err(self.mismatch(&format!("integer ({})", T::NAME)));
        };
        let value = if is_integer_text(text) {
            match text.parse::<i128>() {
                Ok(i) => T::from_i128(i),
                Err(_) => text.parse::<u128>().ok().and_then(T::from_u128),
            }
        } else {
            T::from_f64(parse_f64(text))
        };
        value.ok_or_else(|| {
            Error::type_mismatch(
                self.line,
                self.col,
                &format!("integer in range of {}", T::NAME),
                &describe(text),
            )
        })
    }

    fn float(&self, compat: bool) -> Result<f64> {
        match self.primitive()? {
            Primitive::Number(text) => Ok(parse_f64(text)),
            Primitive::Str(Cow::Borrowed(t)) if compat => {
                legacy_float(t).ok_or_else(|| self.mismatch("number"))
            }
            _ => Err(self.mismatch("number")),
        }
    }
}

// ---------------------------------------------------------------------------
// Scalar deserializer
// ---------------------------------------------------------------------------

/// Deserializes one primitive token.
///
/// `deserialize_any` types the token strictly per §4; the typed hints are
/// accommodating in the way serde_json and serde_yaml are: a string target
/// accepts the text of an unquoted number or boolean (`2024-01-01`, `123`),
/// and a float target accepts an integer token.
pub(crate) struct ScalarDeserializer<'a> {
    pub token: Token<'a>,
    /// serde_toon 0.2 compatibility (`NaN`, `inf` tokens).
    pub compat: bool,
}

impl<'a> ScalarDeserializer<'a> {
    pub fn new(token: Token<'a>, compat: bool) -> Self {
        ScalarDeserializer { token, compat }
    }

    fn locate(&self, err: Error) -> Error {
        locate(err, self.token.line, self.token.col)
    }
}

/// Attaches a position to a position-less error raised by a visitor
/// (`invalid type`, `unknown variant`, custom `Deserialize` impls).
pub(crate) fn locate(err: Error, line: usize, col: usize) -> Error {
    match err {
        Error::Custom(msg) => Error::InvalidFormat { line, col, msg },
        other => other,
    }
}

macro_rules! deserialize_integer {
    ($($method:ident => $visit:ident: $t:ty,)*) => {$(
        fn $method<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
            let value: $t = self.token.integer()?;
            visitor.$visit(value).map_err(|e| self.locate(e))
        }
    )*};
}

impl<'de> de::Deserializer<'de> for ScalarDeserializer<'de> {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        if let Some(i) = small_integer(self.token.text) {
            return visitor.visit_i64(i).map_err(|e| self.locate(e));
        }
        let result = match self.token.primitive()? {
            Primitive::Null => visitor.visit_unit(),
            Primitive::Bool(b) => visitor.visit_bool(b),
            Primitive::Number(text) => visit_number(text, visitor),
            Primitive::Str(Cow::Borrowed(s)) => visitor.visit_borrowed_str(s),
            Primitive::Str(Cow::Owned(s)) => visitor.visit_string(s),
        };
        result.map_err(|e| self.locate(e))
    }

    fn deserialize_bool<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        match self.token.primitive()? {
            Primitive::Bool(b) => visitor.visit_bool(b).map_err(|e| self.locate(e)),
            _ => Err(self.token.mismatch("boolean")),
        }
    }

    deserialize_integer! {
        deserialize_i8 => visit_i8: i8,
        deserialize_i16 => visit_i16: i16,
        deserialize_i32 => visit_i32: i32,
        deserialize_i64 => visit_i64: i64,
        deserialize_i128 => visit_i128: i128,
        deserialize_u8 => visit_u8: u8,
        deserialize_u16 => visit_u16: u16,
        deserialize_u32 => visit_u32: u32,
        deserialize_u64 => visit_u64: u64,
        deserialize_u128 => visit_u128: u128,
    }

    fn deserialize_f32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        let f = self.token.float(self.compat)?;
        visitor.visit_f32(f as f32).map_err(|e| self.locate(e))
    }

    fn deserialize_f64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        let f = self.token.float(self.compat)?;
        visitor.visit_f64(f).map_err(|e| self.locate(e))
    }

    fn deserialize_char<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        let text = match self.token.primitive()? {
            Primitive::Str(s) => s,
            Primitive::Number(t) => Cow::Borrowed(t),
            _ => return Err(self.token.mismatch("character")),
        };
        let mut chars = text.chars();
        match (chars.next(), chars.next()) {
            (Some(c), None) => visitor.visit_char(c).map_err(|e| self.locate(e)),
            _ => Err(self.token.mismatch("a single character")),
        }
    }

    fn deserialize_str<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        let result = match self.token.primitive()? {
            Primitive::Str(Cow::Borrowed(s)) => visitor.visit_borrowed_str(s),
            Primitive::Str(Cow::Owned(s)) => visitor.visit_string(s),
            // Accommodating: an unquoted number or boolean read as text.
            Primitive::Number(_) | Primitive::Bool(_) => {
                visitor.visit_borrowed_str(self.token.text)
            }
            Primitive::Null => visitor.visit_unit(),
        };
        result.map_err(|e| self.locate(e))
    }

    fn deserialize_string<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_str(visitor)
    }

    fn deserialize_bytes<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        let result = match self.token.primitive()? {
            Primitive::Str(Cow::Borrowed(s)) => visitor.visit_borrowed_bytes(s.as_bytes()),
            Primitive::Str(Cow::Owned(s)) => visitor.visit_byte_buf(s.into_bytes()),
            _ => return self.deserialize_any(visitor),
        };
        result.map_err(|e| self.locate(e))
    }

    fn deserialize_byte_buf<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_bytes(visitor)
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        if self.token.text == "null" {
            visitor.visit_none().map_err(|e| self.locate(e))
        } else {
            visitor.visit_some(self)
        }
    }

    fn deserialize_unit<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        match self.token.primitive()? {
            Primitive::Null => visitor.visit_unit().map_err(|e| self.locate(e)),
            _ => Err(self.token.mismatch("null")),
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
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value> {
        // A primitive names a unit variant.
        let (line, col) = (self.token.line, self.token.col);
        let result = match self.token.primitive()? {
            Primitive::Str(Cow::Borrowed(s)) | Primitive::Number(s) => {
                visitor.visit_enum(s.into_deserializer())
            }
            Primitive::Str(Cow::Owned(s)) => visitor.visit_enum(s.into_deserializer()),
            Primitive::Bool(_) => visitor.visit_enum(self.token.text.into_deserializer()),
            Primitive::Null => return Err(self.token.mismatch("enum variant name")),
        };
        result.map_err(|e: Error| locate(e, line, col))
    }

    fn deserialize_ignored_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_unit()
    }

    serde::forward_to_deserialize_any! {
        seq tuple tuple_struct map struct identifier
    }
}

// ---------------------------------------------------------------------------
// Key deserializer
// ---------------------------------------------------------------------------

/// Deserializes an object key, entry key or field name. Keys are always
/// strings (§4); numeric and boolean map-key types are parsed from the text,
/// as serde_json does.
pub(crate) struct KeyDeserializer<'a> {
    pub key: Cow<'a, str>,
    pub line: usize,
    pub col: usize,
}

impl<'a> KeyDeserializer<'a> {
    fn parse<T: std::str::FromStr>(&self, expected: &str) -> Result<T> {
        self.key
            .parse()
            .map_err(|_| Error::type_mismatch(self.line, self.col, expected, &describe(&self.key)))
    }
}

macro_rules! deserialize_key_parsed {
    ($($method:ident => $visit:ident: $t:ty,)*) => {$(
        fn $method<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
            let value: $t = self.parse(stringify!($t))?;
            visitor.$visit(value).map_err(|e| locate(e, self.line, self.col))
        }
    )*};
}

impl<'de> de::Deserializer<'de> for KeyDeserializer<'de> {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        let (line, col) = (self.line, self.col);
        match self.key {
            Cow::Borrowed(s) => visitor.visit_borrowed_str(s),
            Cow::Owned(s) => visitor.visit_string(s),
        }
        .map_err(|e| locate(e, line, col))
    }

    deserialize_key_parsed! {
        deserialize_bool => visit_bool: bool,
        deserialize_i8 => visit_i8: i8,
        deserialize_i16 => visit_i16: i16,
        deserialize_i32 => visit_i32: i32,
        deserialize_i64 => visit_i64: i64,
        deserialize_i128 => visit_i128: i128,
        deserialize_u8 => visit_u8: u8,
        deserialize_u16 => visit_u16: u16,
        deserialize_u32 => visit_u32: u32,
        deserialize_u64 => visit_u64: u64,
        deserialize_u128 => visit_u128: u128,
        deserialize_f32 => visit_f32: f32,
        deserialize_f64 => visit_f64: f64,
        deserialize_char => visit_char: char,
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

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value> {
        let (line, col) = (self.line, self.col);
        match self.key {
            Cow::Borrowed(s) => visitor.visit_enum(s.into_deserializer()),
            Cow::Owned(s) => visitor.visit_enum(s.into_deserializer()),
        }
        .map_err(|e: Error| locate(e, line, col))
    }

    serde::forward_to_deserialize_any! {
        str string bytes byte_buf unit unit_struct seq tuple tuple_struct map
        struct identifier ignored_any
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unq(s: &str) -> Result<Cow<'_, str>> {
        unquote(s, 1, 1)
    }

    #[test]
    fn unquote_borrows_without_escapes() {
        assert!(matches!(unq(r#""abc""#).unwrap(), Cow::Borrowed("abc")));
        assert_eq!(unq(r#""a\"b\\c\n\t\r""#).unwrap(), "a\"b\\c\n\t\r");
        assert_eq!(unq(r#""é\u0004""#).unwrap(), "é\u{4}");
        assert_eq!(unq("\"a\tb\"").unwrap(), "a\tb");
    }

    #[test]
    fn unquote_errors() {
        for bad in [
            r#""abc"#,
            r#""a\x""#,
            r#""\u00b""#,
            r#""\uD800""#,
            concat!("\"\\", "uD83D\\", "uDE80\""),
            r#""abc" def"#,
            r#""a\"#,
        ] {
            assert!(unq(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn small_integers() {
        assert_eq!(small_integer("0"), Some(0));
        assert_eq!(small_integer("-0"), Some(0));
        assert_eq!(small_integer("-123"), Some(-123));
        assert_eq!(
            small_integer("999999999999999999"),
            Some(999_999_999_999_999_999)
        );
        for t in [
            "",
            "-",
            "05",
            "-01",
            "1.5",
            "1e3",
            "+1",
            "1_0",
            "9999999999999999999",
        ] {
            assert_eq!(small_integer(t), None, "{t}");
        }
    }

    #[test]
    fn split_key_at_first_unquoted_colon() {
        assert_eq!(split_key("a: b:c"), Some(("a", "b:c")));
        assert_eq!(split_key(r#""a:b": 1"#), Some((r#""a:b""#, "1")));
        assert_eq!(split_key("abc"), None);
    }
}
