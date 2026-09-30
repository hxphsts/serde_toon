//! Line classification (TOON spec §5.2) and array/keyed header parsing (§6).
//!
//! A line is classified once, when the decoder consumes it, into a
//! [`LineKind`]; header lines carry a fully parsed [`Header`] whose field
//! list is a tree of [`Field`]s. Nothing downstream re-scans the header text.

use super::line::{find_unquoted, find_unquoted2, trim_spaces, Line};
use super::scalar::{split_key, unquote};
use crate::{DecodeOptions, Error, Result};
use std::borrow::Cow;

/// Maximum nesting of field groups inside one header (`{a{b{c}}}`).
const MAX_FIELD_DEPTH: usize = 64;

/// The classification of a structural line (§5.2), excluding list-item
/// markers and tabular rows, which their enclosing scopes recognize.
#[derive(Debug)]
pub(crate) enum LineKind<'a> {
    /// An array header or keyed header (§6).
    Header(Box<Header<'a>>),
    /// `key: value`; `value` is trimmed and may be empty.
    KeyValue { key: Cow<'a, str>, value: &'a str },
    /// A single primitive token (valid only as the root primitive).
    Scalar,
}

/// A parsed array header (`key[N<delim?>]{fields}:`) or keyed header
/// (`key[N:<delim?>]{fields}:`).
#[derive(Debug)]
pub(crate) struct Header<'a> {
    /// `None` for a keyless header (`[N]:`).
    pub key: Option<Cow<'a, str>>,
    /// Declared length, or entry count for a keyed header.
    pub len: usize,
    /// Whether this is a keyed tabular header (§9.5).
    pub keyed: bool,
    /// The active delimiter declared by the bracket segment (§11).
    pub delimiter: u8,
    /// The field list, if any; nested groups form a tree.
    pub fields: Option<Vec<Field<'a>>>,
    /// Number of leaf fields (cells per row).
    pub leaf_count: usize,
    /// Content after the colon, trimmed (inline array values).
    pub inline: &'a str,
}

/// One field entry of a header's field list (§6).
#[derive(Debug)]
pub(crate) struct Field<'a> {
    pub name: Cow<'a, str>,
    /// A nested field group (`customer{name,country}`).
    pub children: Option<Vec<Field<'a>>>,
}

fn count_leaves(fields: &[Field<'_>]) -> usize {
    fields
        .iter()
        .map(|f| f.children.as_deref().map_or(1, count_leaves))
        .sum()
}

/// Why a bracketed line is not a valid header.
enum NotHeader {
    /// Not a header at all; the caller may fall through to key-value
    /// parsing in non-strict mode (§6, §14.2).
    Malformed(&'static str),
    /// A hard error in every mode (e.g. a valid header missing its colon).
    Fatal(Error),
}

/// Classifies `content` (the text of `line` or a suffix of it, e.g. after a
/// list-item marker).
pub(crate) fn classify<'a>(
    content: &'a str,
    line: &Line<'a>,
    options: &DecodeOptions,
) -> Result<LineKind<'a>> {
    let (key_end, key) = if content.as_bytes()[0] == b'"' {
        // Quoted key: find its closing quote.
        let end = quoted_end(content).ok_or_else(|| {
            Error::syntax_with_context(
                line.number,
                line.col_of(content),
                "unterminated string",
                line.content,
                None,
            )
        })?;
        let after = &content[end..];
        if !after.starts_with('[') {
            return match split_key(content) {
                // Validates the quoted key and its boundary (§7.4).
                Some((key, value)) => Ok(LineKind::KeyValue {
                    key: unquote(trim_spaces(key), line.number, line.col_of(content))?,
                    value,
                }),
                None => Ok(LineKind::Scalar),
            };
        }
        (end, Some(&content[..end]))
    } else {
        match find_unquoted2(content, b':', b'[') {
            None => return Ok(LineKind::Scalar),
            Some((i, b':')) => {
                return Ok(LineKind::KeyValue {
                    key: Cow::Borrowed(trim_spaces(&content[..i])),
                    value: trim_spaces(&content[i + 1..]),
                })
            }
            Some((i, _)) => (i, (i > 0).then(|| &content[..i])),
        }
    };

    match parse_header(key, &content[key_end..], line, options) {
        Ok(header) => Ok(LineKind::Header(Box::new(header))),
        Err(NotHeader::Fatal(e)) => Err(e),
        Err(NotHeader::Malformed(_)) if find_unquoted(content, b':').is_none() => {
            // No colon anywhere: not a header, not a key-value line.
            Ok(LineKind::Scalar)
        }
        Err(NotHeader::Malformed(msg)) => {
            if options.is_strict() {
                return Err(Error::syntax_with_context(
                    line.number,
                    line.col_of(&content[key_end..]),
                    &format!("invalid array header: {msg}"),
                    line.content,
                    Some("headers look like `key[N]: ...`, `key[N]{a,b}:` or `key[N:]{a,b}:`"),
                ));
            }
            // Non-strict fall-through: a key-value line with a literal key.
            Ok(fallback(content, line)?)
        }
    }
}

/// The non-strict fall-through for a line that is not a valid header in its
/// position: a key-value line whose key is the literal text before the
/// first unquoted colon (§6, §14.2).
pub(crate) fn fallback<'a>(content: &'a str, line: &Line<'a>) -> Result<LineKind<'a>> {
    Ok(match split_key(content) {
        Some((key, value)) => LineKind::KeyValue {
            key: literal_key(key, line)?,
            value,
        },
        None => LineKind::Scalar,
    })
}

/// A key token taken literally unless it is a complete quoted token.
fn literal_key<'a>(key: &'a str, line: &Line<'a>) -> Result<Cow<'a, str>> {
    let key = trim_spaces(key);
    if key.starts_with('"') && quoted_end(key) == Some(key.len()) {
        unquote(key, line.number, line.col_of(key))
    } else {
        Ok(Cow::Borrowed(key))
    }
}

/// Byte index just past the closing quote of the quoted token starting at
/// `s[0]`.
fn quoted_end(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut i = 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b'"' => return Some(i + 1),
            _ => i += 1,
        }
    }
    None
}

/// Parses a keyless header in value position (serde_toon 0.2 wrote
/// `key: [N]: ...`). Returns `None` if `value` is not a valid header.
pub(crate) fn parse_legacy_value_header<'a>(
    value: &'a str,
    line: &Line<'a>,
    options: &DecodeOptions,
) -> Option<Header<'a>> {
    parse_header(None, value, line, options).ok()
}

/// Parses `rest` (starting at `[`) as the bracket segment, optional field
/// list, colon and inline remainder of a header whose key text is `key`.
fn parse_header<'a>(
    key: Option<&'a str>,
    rest: &'a str,
    line: &Line<'a>,
    options: &DecodeOptions,
) -> std::result::Result<Header<'a>, NotHeader> {
    let compat = options.is_compatible();
    let b = rest.as_bytes();
    debug_assert_eq!(b.first(), Some(&b'['));
    let mut i = 1;

    // serde_toon 0.2 length marker `[#N]` (removed in TOON v2.0).
    if b.get(i) == Some(&b'#') {
        if !compat {
            return Err(NotHeader::Fatal(Error::syntax_with_context(
                line.number,
                line.col_of(rest),
                "length markers (`[#N]`) are not valid TOON v2+ syntax",
                line.content,
                Some("write `[N]`, or decode with DecodeOptions::compatible() to accept serde_toon 0.2 output"),
            )));
        }
        i += 1;
    }

    // length = "0" / ( %x31-39 *DIGIT )
    let start = i;
    while b.get(i).is_some_and(u8::is_ascii_digit) {
        i += 1;
    }
    let digits = &rest[start..i];
    if digits.is_empty() {
        return Err(NotHeader::Malformed("missing or non-integer length"));
    }
    if digits.len() > 1 && digits.starts_with('0') {
        return Err(NotHeader::Malformed("length has leading zeros"));
    }
    let len: usize = digits
        .parse()
        .map_err(|_| NotHeader::Malformed("length is too large"))?;

    let keyed = b.get(i) == Some(&b':');
    if keyed {
        i += 1;
    }

    // Delimiter symbol: HTAB or "|"; serde_toon 0.2 wrote tab as spaces.
    let mut legacy_tab = false;
    let delimiter = match b.get(i) {
        Some(b'\t') => {
            i += 1;
            b'\t'
        }
        Some(b'|') => {
            i += 1;
            b'|'
        }
        Some(b' ') if compat && !keyed => {
            while b.get(i) == Some(&b' ') {
                i += 1;
            }
            legacy_tab = true;
            b'\t'
        }
        _ => b',',
    };
    if b.get(i) != Some(&b']') {
        return Err(NotHeader::Malformed("malformed bracket segment"));
    }
    i += 1;

    // §6: no whitespace between the key and its bracket segment.
    if let Some(k) = key {
        if k.ends_with([' ', '\t']) {
            return Err(NotHeader::Malformed(
                "whitespace between the key and its bracket segment",
            ));
        }
    }

    let fields = if b.get(i) == Some(&b'{') {
        let mut parser = FieldParser {
            s: rest,
            pos: i,
            delimiter,
            legacy_tab,
            strict: options.is_strict(),
            line,
        };
        let fields = parser.list(0)?;
        i = parser.pos;
        Some(fields)
    } else {
        None
    };

    if b.get(i) != Some(&b':') {
        if find_unquoted(&rest[i..], b':').is_none() {
            return Err(NotHeader::Fatal(Error::syntax_with_context(
                line.number,
                line.col_of(&rest[i..]),
                "missing colon after array header",
                line.content,
                Some("every header ends with `:`"),
            )));
        }
        return Err(NotHeader::Malformed(
            "unexpected content between the bracket segment and the colon",
        ));
    }
    let inline = trim_spaces(&rest[i + 1..]);

    if keyed && fields.is_none() {
        return Err(NotHeader::Malformed("keyed header without a field list"));
    }
    if fields.is_some() && !inline.is_empty() {
        return Err(NotHeader::Malformed(
            "a header with a field list carries no inline values",
        ));
    }

    let key = match key {
        None => None,
        Some(k) if k.starts_with('"') => {
            Some(unquote(k, line.number, line.col_of(k)).map_err(NotHeader::Fatal)?)
        }
        Some(k) => Some(Cow::Borrowed(k)),
    };
    let leaf_count = fields.as_deref().map_or(0, count_leaves);
    Ok(Header {
        key,
        len,
        keyed,
        delimiter,
        fields,
        leaf_count,
        inline,
    })
}

/// Recursive-descent parser for a field list (`fields-seg`, §6).
struct FieldParser<'a, 'l> {
    s: &'a str,
    pos: usize,
    delimiter: u8,
    /// serde_toon 0.2 tab headers separate field names with spaces.
    legacy_tab: bool,
    strict: bool,
    line: &'l Line<'a>,
}

impl<'a> FieldParser<'a, '_> {
    fn peek(&self) -> Option<u8> {
        self.s.as_bytes().get(self.pos).copied()
    }

    fn skip_spaces(&mut self) {
        while self.peek() == Some(b' ') {
            self.pos += 1;
        }
    }

    fn is_separator(&self, c: u8) -> bool {
        c == self.delimiter || (self.legacy_tab && c == b' ')
    }

    /// Parses `{ field-entry *( delim field-entry ) }` at `self.pos`.
    fn list(&mut self, depth: usize) -> std::result::Result<Vec<Field<'a>>, NotHeader> {
        if depth >= MAX_FIELD_DEPTH {
            return Err(NotHeader::Fatal(Error::invalid_format(
                self.line.number,
                self.line.col_of(&self.s[self.pos..]),
                "field groups are nested too deeply",
            )));
        }
        debug_assert_eq!(self.peek(), Some(b'{'));
        self.pos += 1;
        let mut fields: Vec<Field<'a>> = Vec::new();
        loop {
            if self.legacy_tab {
                while self.peek() == Some(b' ') {
                    self.pos += 1;
                }
            }
            let name = self.name()?;
            let children = if self.peek() == Some(b'{') {
                Some(self.list(depth + 1)?)
            } else {
                None
            };
            if self.strict && fields.iter().any(|f| f.name == name) {
                return Err(NotHeader::Fatal(Error::syntax_with_context(
                    self.line.number,
                    self.line.col_of(&self.s[self.pos..]),
                    &format!("duplicate field name `{name}` in header"),
                    self.line.content,
                    None,
                )));
            }
            fields.push(Field { name, children });
            self.skip_spaces();
            match self.peek() {
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(fields);
                }
                Some(c) if self.is_separator(c) => self.pos += 1,
                Some(b',' | b'|' | b'\t') => {
                    return Err(NotHeader::Malformed(
                        "field list delimiter does not match the bracket segment",
                    ))
                }
                _ => return Err(NotHeader::Malformed("unmatched brace in field list")),
            }
        }
    }

    /// One field name, quoted or unquoted.
    fn name(&mut self) -> std::result::Result<Cow<'a, str>, NotHeader> {
        self.skip_spaces();
        let rest = &self.s[self.pos..];
        if rest.starts_with('"') {
            let Some(end) = quoted_end(rest) else {
                return Err(NotHeader::Malformed("unterminated field name"));
            };
            let name = unquote(&rest[..end], self.line.number, self.line.col_of(rest))
                .map_err(NotHeader::Fatal)?;
            self.pos += end;
            self.skip_spaces();
            return Ok(name);
        }
        let len = rest
            .bytes()
            .position(|c| {
                matches!(c, b',' | b'|' | b'\t' | b'{' | b'}' | b':' | b'"')
                    || (self.legacy_tab && c == b' ')
            })
            .unwrap_or(rest.len());
        let name = trim_spaces(&rest[..len]);
        if name.is_empty() {
            return Err(NotHeader::Malformed("empty field name or field list"));
        }
        self.pos += len;
        Ok(Cow::Borrowed(name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(content: &str) -> Line<'_> {
        Line {
            number: 1,
            depth: 0,
            lead: 0,
            content,
            after_blank: false,
        }
    }

    fn header<'a>(l: &'a Line<'a>, options: DecodeOptions) -> Result<Header<'a>> {
        match classify(l.content, l, &options)? {
            LineKind::Header(h) => Ok(*h),
            other => panic!("not a header: {other:?}"),
        }
    }

    #[test]
    fn parses_nested_fields() {
        let l = line("orders[2|]{id|customer{name|\"full name\"}|total}:");
        let h = header(&l, DecodeOptions::strict()).unwrap();
        assert_eq!(h.key.as_deref(), Some("orders"));
        assert_eq!(
            (h.len, h.keyed, h.delimiter, h.leaf_count),
            (2, false, b'|', 4)
        );
        let fields = h.fields.unwrap();
        assert_eq!(fields[1].name, "customer");
        assert_eq!(fields[1].children.as_ref().unwrap()[1].name, "full name");
    }

    #[test]
    fn keyed_and_inline() {
        let l = line("m[2:\t]{a\tb}:");
        let h = header(&l, DecodeOptions::strict()).unwrap();
        assert!(h.keyed);
        assert_eq!(h.delimiter, b'\t');
        let l = line("\"a:b\"[2]: 1,2");
        let h = header(&l, DecodeOptions::strict()).unwrap();
        assert_eq!(h.key.as_deref(), Some("a:b"));
        assert_eq!(h.inline, "1,2");
    }

    #[test]
    fn strict_rejects_malformed() {
        for bad in [
            "items[03]: a",
            "items[-1]: a",
            "key[]: 1",
            "foo [2]: a",
            "foo[2]extra: a",
            "items[1]{}:",
            "items[1]{a,a}:",
            "items[2\t]{a,b}:",
            "m[2:]:",
            "m[2|:]{v}:",
            "items[2]{a}: 1",
            "items[2]{a}",
            "[#2]: 1,2",
        ] {
            let l = line(bad);
            assert!(
                classify(bad, &l, &DecodeOptions::strict()).is_err(),
                "{bad}"
            );
        }
    }

    #[test]
    fn lenient_falls_through_to_key_value() {
        let l = line("foo[1][bar]: 10");
        match classify(l.content, &l, &DecodeOptions::lenient()).unwrap() {
            LineKind::KeyValue { key, value } => assert_eq!((&*key, value), ("foo[1][bar]", "10")),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn colon_before_bracket_is_key_value() {
        let l = line("a:b[2]: x");
        match classify(l.content, &l, &DecodeOptions::strict()).unwrap() {
            LineKind::KeyValue { key, value } => assert_eq!((&*key, value), ("a", "b[2]: x")),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn legacy_headers_only_in_compat_mode() {
        let l = line("[#3    ]: 1\t2\t3");
        let h = header(&l, DecodeOptions::compatible()).unwrap();
        assert_eq!((h.len, h.delimiter), (3, b'\t'));
        assert!(classify(l.content, &l, &DecodeOptions::lenient()).is_err());
    }
}
