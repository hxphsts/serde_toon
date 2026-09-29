//! Lexical rules shared by the encoder and the decoder.
//!
//! Every function here is a direct transcription of a normative rule in the
//! TOON specification (v4.1). Keeping them in one place guarantees that what
//! the encoder considers "safe to leave unquoted" is exactly what the decoder
//! reads back as the same value.

use crate::options::Delimiter;

/// §7.3: a key may be emitted unquoted only if it matches
/// `^[A-Za-z_][A-Za-z0-9_.]*$`.
pub(crate) fn is_unquoted_key(s: &str) -> bool {
    let mut bytes = s.bytes();
    match bytes.next() {
        Some(b) if b.is_ascii_alphabetic() || b == b'_' => {}
        _ => return false,
    }
    bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'.')
}

/// Matches `/^[+-]?[0-9]+(?:\.[0-9]+)?(?:e[+-]?[0-9]+)?$/i` when
/// `allow_plus` is true, or the same pattern with only an optional `-` sign
/// when it is false. Leading zeros are *not* rejected here.
fn matches_decimal_grammar(s: &str, allow_plus: bool) -> bool {
    let b = s.as_bytes();
    let mut i = 0;
    if i < b.len() && (b[i] == b'-' || (allow_plus && b[i] == b'+')) {
        i += 1;
    }
    let int_start = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    if i == int_start {
        return false;
    }
    if i < b.len() && b[i] == b'.' {
        i += 1;
        let frac_start = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i == frac_start {
            return false;
        }
    }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        i += 1;
        if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
            i += 1;
        }
        let exp_start = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i == exp_start {
            return false;
        }
    }
    i == b.len()
}

/// §7.2: a string value that matches the numeric-like pattern
/// `/^[+-]?[0-9]+(?:\.[0-9]+)?(?:e[+-]?[0-9]+)?$/i` must be quoted by the
/// encoder (this deliberately includes leading-zero forms like `05`).
pub(crate) fn is_numeric_like(s: &str) -> bool {
    matches_decimal_grammar(s, true)
}

/// §4: an unquoted token decodes as a number if and only if it matches
/// `/^-?[0-9]+(?:\.[0-9]+)?(?:e[+-]?[0-9]+)?$/i` and its integer part has no
/// forbidden leading zeros (`05`, `-0001` are strings; `0.5`, `0e1` are
/// numbers).
pub(crate) fn is_number_token(s: &str) -> bool {
    if !matches_decimal_grammar(s, false) {
        return false;
    }
    let digits = s.strip_prefix('-').unwrap_or(s).as_bytes();
    // The grammar guarantees at least one leading digit.
    !(digits[0] == b'0' && digits.get(1).is_some_and(u8::is_ascii_digit))
}

/// §7.2: whether a string *value* must be quoted when emitted in a context
/// whose relevant delimiter is `delimiter` (§11.1).
pub(crate) fn value_needs_quotes(s: &str, delimiter: &Delimiter) -> bool {
    let b = s.as_bytes();
    let Some((&first, &last)) = b.first().zip(b.last()) else {
        return true; // empty
    };
    if matches!(first, b' ' | b'\t' | b'-' | b'#') || matches!(last, b' ' | b'\t') {
        return true;
    }
    if matches!(s, "true" | "false" | "null") || is_numeric_like(s) {
        return true;
    }
    let delim = delimiter.as_byte();
    b.iter().any(|&c| {
        c == delim || c < 0x20 || matches!(c, b':' | b'"' | b'\\' | b'[' | b']' | b'{' | b'}')
    })
}

/// §7.1: append `s` to `out` as a quoted string, escaping `\\`, `"`, `\n`,
/// `\r`, `\t` and every other U+0000–U+001F control as lowercase `\uXXXX`.
pub(crate) fn write_quoted(out: &mut String, s: &str) {
    out.reserve(s.len() + 2);
    out.push('"');
    let bytes = s.as_bytes();
    let mut start = 0;
    for (i, &c) in bytes.iter().enumerate() {
        let escape: &str = match c {
            b'"' => "\\\"",
            b'\\' => "\\\\",
            b'\n' => "\\n",
            b'\r' => "\\r",
            b'\t' => "\\t",
            0x00..=0x1f => "",
            _ => continue,
        };
        // `i` is at an ASCII byte, so both slice bounds are char boundaries.
        out.push_str(&s[start..i]);
        if escape.is_empty() {
            const HEX: &[u8; 16] = b"0123456789abcdef";
            out.push_str("\\u00");
            out.push(HEX[usize::from(c >> 4)] as char);
            out.push(HEX[usize::from(c & 0xf)] as char);
        } else {
            out.push_str(escape);
        }
        start = i + 1;
    }
    out.push_str(&s[start..]);
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unquoted_keys() {
        for k in ["a", "_x", "user.name", "A1_b"] {
            assert!(is_unquoted_key(k), "{k}");
        }
        for k in ["", "1a", "foo-bar", "a b", "a:b", ".a", "é"] {
            assert!(!is_unquoted_key(k), "{k}");
        }
    }

    #[test]
    fn number_tokens() {
        for t in [
            "0", "-0", "1", "-12", "0.5", "0e1", "-0.5", "1E+5", "1.5e-3",
        ] {
            assert!(is_number_token(t), "{t}");
        }
        for t in [
            "", "-", ".5", "1.", "+5", "05", "-0001", "0001", "Infinity", "NaN", "0x10", "1_000",
            "1e", "1e+", "--1",
        ] {
            assert!(!is_number_token(t), "{t}");
        }
    }

    #[test]
    fn numeric_like() {
        for t in ["05", "+1", "-1", "1e5", "1.5"] {
            assert!(is_numeric_like(t), "{t}");
        }
        for t in ["1.", ".5", "1a", "Infinity"] {
            assert!(!is_numeric_like(t), "{t}");
        }
    }

    #[test]
    fn value_quoting() {
        let c = Delimiter::Comma;
        for s in [
            "", " a", "a ", "true", "null", "42", "-x", "-", "#tag", "a:b", "a,b", "[x", "q\"",
            "\u{1}",
        ] {
            assert!(value_needs_quotes(s, &c), "{s:?}");
        }
        for s in ["hello world", "a|b", "café", "x-y", "a.b"] {
            assert!(!value_needs_quotes(s, &c), "{s:?}");
        }
        assert!(value_needs_quotes("a|b", &Delimiter::Pipe));
        assert!(value_needs_quotes("a\tb", &Delimiter::Tab));
        assert!(!value_needs_quotes("a,b", &Delimiter::Pipe));
    }

    #[test]
    fn quoting_escapes() {
        let mut out = String::new();
        write_quoted(&mut out, "a\"b\\c\nd\u{1}é\t");
        assert_eq!(out, r#""a\"b\\c\nd\u0001é\t""#);
    }
}
