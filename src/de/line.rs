//! Line pre-pass (TOON spec §5.1, §12) and quote-aware scanning helpers.
//!
//! The pre-pass runs once over the input and produces a flat list of typed
//! [`Line`]s: the byte-order mark, line terminators (LF and CRLF), trailing
//! spaces, blank lines and comment lines are dealt with here, so nothing
//! downstream ever sees them. Every later stage works on `Line::content`.

use crate::{DecodeOptions, Error, Result};

/// One non-blank, non-comment line of the document.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Line<'a> {
    /// 1-based line number in the original input.
    pub number: usize,
    /// Indentation level (§12): leading spaces divided by `indentSize`.
    pub depth: usize,
    /// Leading whitespace in bytes; `content` starts at column `lead + 1`.
    pub lead: usize,
    /// The line after indentation, with the CR terminator and trailing
    /// spaces removed. Never empty.
    pub content: &'a str,
    /// Whether one or more blank lines precede this line (after comment
    /// removal). Used for the strict header-span check of §12.
    pub after_blank: bool,
}

impl<'a> Line<'a> {
    /// 1-based column of `part`, which must be a subslice of `content`.
    pub fn col_of(&self, part: &str) -> usize {
        let offset = (part.as_ptr() as usize).saturating_sub(self.content.as_ptr() as usize);
        self.lead + offset.min(self.content.len()) + 1
    }

    /// 1-based line and column of the start of the content.
    pub fn pos(&self) -> (usize, usize) {
        (self.number, self.lead + 1)
    }

    /// Whether this is a list-item line (§5.2): the bare marker `-` or a
    /// line starting with `- `. Returns the text after the marker.
    pub fn list_item(&self) -> Option<&'a str> {
        let c = self.content;
        if c == "-" {
            Some("")
        } else {
            c.strip_prefix("- ").map(trim_spaces)
        }
    }
}

/// Splits `input` into typed lines.
///
/// Strict mode rejects indentation that is not a multiple of the indent size
/// and tabs used as indentation (§12). Non-strict modes floor the depth and
/// count a leading tab as one full indentation level.
pub(crate) fn split_lines<'a>(input: &'a str, options: &DecodeOptions) -> Result<Vec<Line<'a>>> {
    let strict = options.is_strict();
    let indent_size = options.indent_size();
    if indent_size == 0 {
        return Err(Error::invalid_format(
            0,
            0,
            "DecodeOptions::indent_size must be at least 1",
        ));
    }
    // §12: a single U+FEFF at the very start is a byte-order mark.
    let input = input.strip_prefix('\u{feff}').unwrap_or(input);

    let mut lines = Vec::with_capacity(input.len() / 16 + 1);
    let mut after_blank = false;
    // Non-strict: indentation of the first content line is the root level.
    let mut base: Option<usize> = None;

    for (index, raw) in input.split('\n').enumerate() {
        let number = index + 1;
        let raw = raw.strip_suffix('\r').unwrap_or(raw);
        let raw = raw.trim_end_matches(' ');
        let bytes = raw.as_bytes();

        let mut lead = 0;
        let mut spaces = 0;
        let mut tabs = 0;
        while lead < bytes.len() {
            match bytes[lead] {
                b' ' => spaces += 1,
                b'\t' => tabs += 1,
                _ => break,
            }
            lead += 1;
        }
        let content = &raw[lead..];
        if content.is_empty() {
            // Blank line (§12): never creates or closes structure.
            after_blank = true;
            continue;
        }
        if tabs == 0 && content.as_bytes()[0] == b'#' {
            // Comment line (§5.1): removed without interpretation.
            continue;
        }
        if tabs > 0 && strict {
            return Err(Error::syntax_with_context(
                number,
                1,
                "tab character used for indentation",
                raw,
                Some("TOON indents with spaces only; tabs are allowed inside quoted strings and as a declared delimiter"),
            ));
        }
        let indent = spaces + tabs * indent_size;
        if strict && indent % indent_size != 0 {
            return Err(Error::indentation_error(
                number,
                indent + 1,
                indent / indent_size * indent_size,
                indent,
                raw,
            ));
        }
        let indent = if strict {
            indent
        } else {
            indent.saturating_sub(*base.get_or_insert(indent))
        };
        lines.push(Line {
            number,
            depth: indent / indent_size,
            lead,
            content,
            after_blank,
        });
        after_blank = false;
    }
    Ok(lines)
}

/// Trims U+0020 only (§12 token trimming), never other whitespace.
pub(crate) fn trim_spaces(s: &str) -> &str {
    let b = s.as_bytes();
    let mut start = 0;
    let mut end = b.len();
    while start < end && b[start] == b' ' {
        start += 1;
    }
    while end > start && b[end - 1] == b' ' {
        end -= 1;
    }
    // Both bounds are next to ASCII spaces, hence char boundaries.
    &s[start..end]
}

/// Index of the first unquoted occurrence of `a` or `b` in `s`, with the byte
/// found. Double quotes toggle the quoted state; inside quotes a backslash
/// escapes the next byte (Appendix B.3).
pub(crate) fn find_unquoted2(s: &str, a: u8, b: u8) -> Option<(usize, u8)> {
    let bytes = s.as_bytes();
    let mut i = 0;
    let mut quoted = false;
    while i < bytes.len() {
        let c = bytes[i];
        if quoted {
            if c == b'\\' {
                i += 1;
            } else if c == b'"' {
                quoted = false;
            }
        } else if c == b'"' {
            quoted = true;
        } else if c == a || c == b {
            return Some((i, c));
        }
        i += 1;
    }
    None
}

/// Index of the first unquoted occurrence of `needle` in `s`.
pub(crate) fn find_unquoted(s: &str, needle: u8) -> Option<usize> {
    find_unquoted2(s, needle, needle).map(|(i, _)| i)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contents(input: &str, options: DecodeOptions) -> Vec<(usize, usize, String, bool)> {
        split_lines(input, &options)
            .unwrap()
            .into_iter()
            .map(|l| (l.number, l.depth, l.content.to_string(), l.after_blank))
            .collect()
    }

    #[test]
    fn strips_bom_crlf_comments_and_trailing_spaces() {
        let got = contents(
            "\u{feff}a: 1  \r\n  # c\n\n  b: 2\r\n",
            DecodeOptions::strict(),
        );
        assert_eq!(
            got,
            vec![
                (1, 0, "a: 1".to_string(), false),
                (4, 1, "b: 2".to_string(), true)
            ]
        );
    }

    #[test]
    fn tab_before_hash_is_not_a_comment() {
        assert!(split_lines("\t#x", &DecodeOptions::strict()).is_err());
        let got = contents("a:\n\t#x", DecodeOptions::lenient());
        assert_eq!(got[1], (2, 1, "#x".to_string(), false));
    }

    #[test]
    fn strict_indentation() {
        assert!(split_lines("a:\n   b: 1", &DecodeOptions::strict()).is_err());
        let got = contents("a:\n   b: 1", DecodeOptions::lenient());
        assert_eq!(got[1].1, 1);
    }

    #[test]
    fn zero_indent_size_is_an_error() {
        assert!(split_lines("a: 1", &DecodeOptions::strict().with_indent_size(0)).is_err());
    }

    #[test]
    fn unquoted_search() {
        assert_eq!(find_unquoted(r#""a:b": c"#, b':'), Some(5));
        assert_eq!(find_unquoted(r#""a\":b": c"#, b':'), Some(7));
        assert_eq!(find_unquoted2("x,y:z", b':', b','), Some((1, b',')));
        assert_eq!(find_unquoted("abc", b':'), None);
    }
}
