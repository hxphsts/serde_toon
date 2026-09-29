//! Configuration options for TOON serialization and deserialization.
//!
//! - [`ToonOptions`]: encoder configuration (indentation and delimiter).
//! - [`Delimiter`]: the delimiter for inline arrays and tabular rows.
//! - [`DecodeOptions`]: decoder configuration (strictness and indentation).
//!
//! ## Examples
//!
//! ```rust
//! use serde_toon::{to_string_with_options, Delimiter, ToonOptions};
//! use serde::Serialize;
//!
//! #[derive(Serialize)]
//! struct Data { tags: Vec<&'static str> }
//!
//! let data = Data { tags: vec!["a", "b"] };
//!
//! let options = ToonOptions::new().with_delimiter(Delimiter::Pipe);
//! let toon = to_string_with_options(&data, options)?;
//! assert_eq!(toon, "tags[2|]: a|b");
//! # Ok::<(), serde_toon::Error>(())
//! ```

/// Delimiter for TOON inline arrays and tabular rows (TOON spec §11).
///
/// - **Comma**: the default and most compact.
/// - **Tab**: TSV-like output; often tokenizes best.
/// - **Pipe**: readable, markdown-style tables.
///
/// The encoder declares the chosen delimiter in every array header it emits
/// (`[N]` for comma, `[N\t]` for tab, `[N|]` for pipe), so the decoder never
/// needs to be told which one was used.
///
/// # Examples
///
/// ```rust
/// use serde_toon::Delimiter;
///
/// assert_eq!(Delimiter::Comma.as_str(), ",");
/// assert_eq!(Delimiter::Tab.as_str(), "\t");
/// assert_eq!(Delimiter::Pipe.as_str(), "|");
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub enum Delimiter {
    /// Comma delimiter (`,`). This is the default.
    #[default]
    Comma,
    /// Tab delimiter (`\t`).
    Tab,
    /// Pipe delimiter (`|`).
    Pipe,
}

impl Delimiter {
    /// Returns the delimiter as a string slice.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use serde_toon::Delimiter;
    ///
    /// assert_eq!(Delimiter::Pipe.as_str(), "|");
    /// ```
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Delimiter::Comma => ",",
            Delimiter::Tab => "\t",
            Delimiter::Pipe => "|",
        }
    }

    /// The delimiter as a single ASCII byte.
    pub(crate) const fn as_byte(&self) -> u8 {
        match self {
            Delimiter::Comma => b',',
            Delimiter::Tab => b'\t',
            Delimiter::Pipe => b'|',
        }
    }

    /// The symbol declared inside an array header's brackets (§6): nothing
    /// for comma, the delimiter itself otherwise.
    pub(crate) const fn header_symbol(&self) -> &'static str {
        match self {
            Delimiter::Comma => "",
            Delimiter::Tab => "\t",
            Delimiter::Pipe => "|",
        }
    }
}

/// Configuration options for TOON serialization.
///
/// # Examples
///
/// ```rust
/// use serde_toon::{Delimiter, ToonOptions};
///
/// // Defaults: 2-space indentation, comma delimiter.
/// let options = ToonOptions::new();
///
/// // Custom configuration.
/// let options = ToonOptions::new()
///     .with_delimiter(Delimiter::Pipe)
///     .with_indent(4);
/// assert_eq!(options.indent, 4);
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct ToonOptions {
    /// Number of spaces per indentation level (TOON `indentSize`). Must be
    /// at least 1; serializing with `0` returns an error. Default is 2.
    pub indent: usize,
    /// Delimiter for inline arrays and tabular rows. Default is comma.
    pub delimiter: Delimiter,
    /// Formerly prefixed array lengths with a marker (`[#3]`).
    ///
    /// TOON v2.0 removed length markers and v4.1 forbids encoders from
    /// emitting them, so this field is ignored by the serializer. The decoder
    /// still accepts `[#N]` input produced by serde_toon 0.2 (see
    /// [`DecodeOptions::compatible`]).
    #[deprecated(
        since = "0.3.0",
        note = "TOON v2+ forbids length markers; this field is ignored by the serializer"
    )]
    pub length_marker: Option<char>,
    /// Retained for compatibility. TOON output is always multi-line and
    /// indented, so this flag has no effect on the serialized text.
    pub pretty: bool,
}

impl Default for ToonOptions {
    #[allow(deprecated)]
    fn default() -> Self {
        ToonOptions {
            indent: 2,
            delimiter: Delimiter::default(),
            length_marker: None,
            pretty: false,
        }
    }
}

impl ToonOptions {
    /// Creates default options (comma delimiter, 2-space indent).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use serde_toon::ToonOptions;
    ///
    /// let options = ToonOptions::new();
    /// assert_eq!(options.indent, 2);
    /// assert!(!options.pretty);
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates default options with the `pretty` flag set.
    ///
    /// TOON output is always indented, so this produces the same text as
    /// [`ToonOptions::new`]; it is kept for compatibility.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use serde_toon::ToonOptions;
    ///
    /// let options = ToonOptions::pretty();
    /// assert!(options.pretty);
    /// ```
    #[must_use]
    pub fn pretty() -> Self {
        ToonOptions {
            pretty: true,
            ..Default::default()
        }
    }

    /// Sets the number of spaces per indentation level. Default is 2.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use serde_toon::ToonOptions;
    ///
    /// let options = ToonOptions::new().with_indent(4);
    /// assert_eq!(options.indent, 4);
    /// ```
    #[must_use]
    pub fn with_indent(mut self, indent: usize) -> Self {
        self.indent = indent;
        self
    }

    /// Sets the delimiter for inline arrays and tabular rows.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use serde_toon::{Delimiter, ToonOptions};
    ///
    /// let options = ToonOptions::new().with_delimiter(Delimiter::Pipe);
    /// assert_eq!(options.delimiter, Delimiter::Pipe);
    /// ```
    #[must_use]
    pub fn with_delimiter(mut self, delimiter: Delimiter) -> Self {
        self.delimiter = delimiter;
        self
    }

    /// Formerly set a length marker for arrays (`[#3]`).
    ///
    /// TOON v2.0 removed length markers, so the serializer ignores this
    /// setting and always emits `[3]`.
    #[deprecated(
        since = "0.3.0",
        note = "TOON v2+ forbids length markers; this setting is ignored by the serializer"
    )]
    #[must_use]
    #[allow(deprecated)]
    pub fn with_length_marker(mut self, marker: char) -> Self {
        self.length_marker = Some(marker);
        self
    }
}

/// How strictly the decoder enforces the TOON specification.
///
/// Three modes only: combining strict validation with the legacy
/// serde_toon 0.2 syntax would be contradictory, so it is unrepresentable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum DecodeMode {
    /// Spec strict mode (§14): every count, width, indentation and
    /// duplicate-key violation is an error.
    Strict,
    /// Spec non-strict mode: violations that §14 permits a lenient decoder to
    /// accept are accepted.
    Lenient,
    /// [`DecodeMode::Lenient`], plus the syntax emitted by serde_toon 0.2
    /// (`[#N]` length markers, four-space tab headers, `NaN`/`Infinity`).
    Compatible,
}

/// Configuration options for TOON deserialization.
///
/// The fields are private; construct one of the three modes and adjust it
/// with the `with_*` methods.
///
/// | Constructor                       | Spec validation (§14) | serde_toon 0.2 syntax |
/// |-----------------------------------|-----------------------|-----------------------|
/// | [`DecodeOptions::strict`]         | enforced              | rejected              |
/// | [`DecodeOptions::lenient`]        | relaxed               | rejected              |
/// | [`DecodeOptions::compatible`] (default) | relaxed         | accepted              |
///
/// [`from_str`](crate::from_str) uses the default, `compatible`, so every
/// document that serde_toon 0.2 could read still decodes.
///
/// # Examples
///
/// ```rust
/// use serde_toon::{from_str_with_options, DecodeOptions};
/// use std::collections::BTreeMap;
///
/// // Strict mode rejects a declared length that does not match.
/// let res: Result<BTreeMap<String, Vec<i32>>, _> =
///     from_str_with_options("xs[3]: 1,2", DecodeOptions::strict());
/// assert!(res.is_err());
///
/// let ok: BTreeMap<String, Vec<i32>> =
///     from_str_with_options("xs[2]: 1,2", DecodeOptions::strict())?;
/// assert_eq!(ok["xs"], vec![1, 2]);
/// # Ok::<(), serde_toon::Error>(())
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DecodeOptions {
    mode: DecodeMode,
    indent_size: usize,
}

impl Default for DecodeOptions {
    /// Same as [`DecodeOptions::compatible`].
    fn default() -> Self {
        Self::compatible()
    }
}

impl DecodeOptions {
    /// Spec-conformant strict decoding (TOON v4.1 §14), as recommended by the
    /// specification for validating untrusted or LLM-generated input.
    #[must_use]
    pub const fn strict() -> Self {
        Self::with_mode(DecodeMode::Strict)
    }

    /// Spec-conformant non-strict decoding: declared lengths, indentation
    /// multiples and duplicate keys are not enforced.
    #[must_use]
    pub const fn lenient() -> Self {
        Self::with_mode(DecodeMode::Lenient)
    }

    /// Non-strict decoding that additionally accepts the syntax produced by
    /// serde_toon 0.2. This is the default and what [`from_str`](crate::from_str) uses.
    #[must_use]
    pub const fn compatible() -> Self {
        Self::with_mode(DecodeMode::Compatible)
    }

    const fn with_mode(mode: DecodeMode) -> Self {
        DecodeOptions {
            mode,
            indent_size: 2,
        }
    }

    /// Sets the number of spaces per indentation level (TOON `indentSize`).
    /// Default is 2. Decoding with `0` returns an error.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use serde_toon::DecodeOptions;
    ///
    /// let options = DecodeOptions::strict().with_indent_size(4);
    /// assert_eq!(options.indent_size(), 4);
    /// ```
    #[must_use]
    pub const fn with_indent_size(mut self, indent_size: usize) -> Self {
        self.indent_size = indent_size;
        self
    }

    /// Whether spec strict-mode validation (§14) is enforced.
    #[must_use]
    pub const fn is_strict(&self) -> bool {
        matches!(self.mode, DecodeMode::Strict)
    }

    /// Whether serde_toon 0.2 syntax is accepted.
    #[must_use]
    pub const fn is_compatible(&self) -> bool {
        matches!(self.mode, DecodeMode::Compatible)
    }

    /// The number of spaces per indentation level.
    #[must_use]
    pub const fn indent_size(&self) -> usize {
        self.indent_size
    }

    #[allow(dead_code)] // used by the decoder
    pub(crate) const fn mode(&self) -> DecodeMode {
        self.mode
    }
}
