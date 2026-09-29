//! Number formatting for the encoder (TOON spec §2, §3).
//!
//! - Integers are written exactly, whatever their width.
//! - Finite floats with `n == 0` or `1e-6 <= |n| < 1e21` are written in
//!   canonical decimal form: no exponent, no trailing fractional zeros, no
//!   fractional part when it is zero, and `-0` written as `0`.
//! - Other finite floats are written in the shortest round-trip exponent form
//!   with a lowercase `e` and an explicit exponent sign (`1e+21`, `1e-7`).
//! - `NaN` and the infinities are written as `null`.
//!
//! Rust's `Display` for floats already produces the shortest decimal digits
//! that round-trip and never uses an exponent, and `LowerExp` produces the
//! shortest round-trip mantissa; both are exact for `f32` as well, so `0.1f32`
//! is written `0.1`, not `0.10000000149011612`.

use std::fmt::{self, Write as _};

/// Appends the decimal digits of `v` to `out` without going through
/// `core::fmt`.
pub(crate) fn push_u64(out: &mut String, mut v: u64) {
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    loop {
        i -= 1;
        // `v % 10` is below 10, so the cast is lossless.
        buf[i] = b'0' + (v % 10) as u8;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    // Every byte written is an ASCII digit.
    if let Ok(digits) = std::str::from_utf8(&buf[i..]) {
        out.push_str(digits);
    }
}

/// Appends `v` in decimal to `out`.
pub(crate) fn push_i64(out: &mut String, v: i64) {
    if v < 0 {
        out.push('-');
    }
    push_u64(out, v.unsigned_abs());
}

/// A float type the encoder can format: `f32` or `f64`.
pub(crate) trait Float: Copy + fmt::Display + fmt::LowerExp {
    fn to_f64(self) -> f64;
}

impl Float for f64 {
    fn to_f64(self) -> f64 {
        self
    }
}

impl Float for f32 {
    fn to_f64(self) -> f64 {
        f64::from(self)
    }
}

/// Appends `v` to `out` per §2/§3 (see the module documentation).
pub(crate) fn push_float<F: Float>(out: &mut String, v: F) {
    let n = v.to_f64();
    if !n.is_finite() {
        out.push_str("null");
        return;
    }
    if n == 0.0 {
        // Also normalizes -0.
        out.push('0');
        return;
    }
    // Writing into a `String` cannot fail, so the `fmt::Result`s are ignored.
    if (1e-5..1e20).contains(&n.abs()) {
        // Comfortably inside the canonical range whatever the rounding.
        let _ = write!(out, "{v}");
        return;
    }
    // Near or outside the range boundaries, decide on the decimal value that
    // will actually be written (the shortest round-trip digits), not on the
    // binary value: `1e-6f32` is slightly below 1e-6 in binary but is
    // written, and decodes, as 1e-6.
    let start = out.len();
    let _ = write!(out, "{v:e}");
    let Some(e) = out[start..].find('e').map(|e| start + e) else {
        return;
    };
    let exponent: i32 = out[e + 1..].parse().unwrap_or(0);
    if (-6..=20).contains(&exponent) {
        out.truncate(start);
        let _ = write!(out, "{v}");
    } else if exponent >= 0 {
        // `LowerExp` writes `1e21`; the deterministic form carries an
        // explicit exponent sign.
        out.insert(e + 1, '+');
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f64s(v: f64) -> String {
        let mut s = String::new();
        push_float(&mut s, v);
        s
    }

    fn f32s(v: f32) -> String {
        let mut s = String::new();
        push_float(&mut s, v);
        s
    }

    #[test]
    fn integers() {
        let mut s = String::new();
        for v in [0, 7, -7, 10, 1_000_000, i64::MAX, i64::MIN] {
            s.clear();
            push_i64(&mut s, v);
            assert_eq!(s, v.to_string());
        }
        s.clear();
        push_u64(&mut s, u64::MAX);
        assert_eq!(s, "18446744073709551615");
    }

    #[test]
    fn canonical_decimal_range() {
        assert_eq!(f64s(0.1), "0.1");
        assert_eq!(f64s(1.0), "1");
        assert_eq!(f64s(-1.5), "-1.5");
        assert_eq!(f64s(1.5000), "1.5");
        assert_eq!(f64s(123456789.125), "123456789.125");
        assert_eq!(f64s(0.3333333333333333), "0.3333333333333333");
        assert_eq!(f64s(1e6), "1000000");
        assert_eq!(f64s(1e-6), "0.000001");
        assert_eq!(f64s(1e20), "100000000000000000000");
        assert_eq!(f64s(9007199254740992.0), "9007199254740992");
    }

    #[test]
    fn zero_and_negative_zero() {
        assert_eq!(f64s(0.0), "0");
        assert_eq!(f64s(-0.0), "0");
        assert_eq!(f32s(-0.0), "0");
    }

    #[test]
    fn exponent_range() {
        assert_eq!(f64s(1e21), "1e+21");
        assert_eq!(f64s(-1e21), "-1e+21");
        assert_eq!(f64s(1.5e300), "1.5e+300");
        assert_eq!(f64s(1e-7), "1e-7");
        assert_eq!(f64s(-2.5e-9), "-2.5e-9");
        assert_eq!(f64s(f64::MAX), "1.7976931348623157e+308");
        assert_eq!(f64s(f64::MIN), "-1.7976931348623157e+308");
        assert_eq!(f64s(f64::MIN_POSITIVE), "2.2250738585072014e-308");
        assert_eq!(f64s(5e-324), "5e-324");
    }

    #[test]
    fn exponent_forms_round_trip() {
        for v in [
            1e21,
            1.5e300,
            1e-7,
            -2.5e-9,
            f64::MAX,
            f64::MIN_POSITIVE,
            5e-324,
        ] {
            assert_eq!(f64s(v).parse::<f64>().unwrap(), v);
        }
    }

    #[test]
    fn non_finite_is_null() {
        assert_eq!(f64s(f64::NAN), "null");
        assert_eq!(f64s(f64::INFINITY), "null");
        assert_eq!(f64s(f64::NEG_INFINITY), "null");
        assert_eq!(f32s(f32::NAN), "null");
        assert_eq!(f32s(f32::INFINITY), "null");
    }

    #[test]
    fn f32_uses_shortest_f32_digits() {
        assert_eq!(f32s(0.1), "0.1");
        assert_eq!(f32s(3.5), "3.5");
        assert_eq!(f32s(-2.5), "-2.5");
        assert_eq!(f32s(16777216.0), "16777216");
        assert_eq!(f32s(1e-6), "0.000001");
        assert_eq!(f32s(f32::MAX), "3.4028235e+38");
        assert_eq!(f32s(f32::MIN_POSITIVE), "1.1754944e-38");
        assert_eq!(f32s(1e-7), "1e-7");
    }
}
