//! Configuration options for TOON serialization.
//!
//! This module provides types to customize TOON output format:
//!
//! - [`ToonOptions`]: Main configuration struct
//! - [`Delimiter`]: Choice of delimiter for arrays and tables (comma, tab, or pipe)
//!
//! ## Examples
//!
//! ```rust
//! use serde_toon::{ToonOptions, Delimiter, to_string_with_options};
//! use serde::Serialize;
//!
//! #[derive(Serialize)]
//! struct Data { x: i32, y: i32 }
//!
//! let data = Data { x: 1, y: 2 };
//!
//! // Use pipe delimiter
//! let options = ToonOptions::new().with_delimiter(Delimiter::Pipe);
//! let toon = to_string_with_options(&data, options).unwrap();
//!
//! // Use length marker '#' for arrays
//! let options = ToonOptions::new().with_length_marker('#');
//! let toon = to_string_with_options(&vec![1, 2, 3], options).unwrap();
//! // Output: "[#3]: 1,2,3"
//! ```

/// Delimiter choice for TOON arrays and tables.
///
/// TOON supports multiple delimiters to optimize for different contexts:
///
/// - **Comma**: Default, most compact
/// - **Tab**: Best for TSV-like output
/// - **Pipe**: Readable for markdown-style tables
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
#[derive(Clone, Debug, PartialEq, Default)]
pub enum Delimiter {
    #[default]
    Comma,
    Tab,
    Pipe,
}

impl Delimiter {
    /// Returns the string representation of this delimiter.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Delimiter::Comma => ",",
            Delimiter::Tab => "\t",
            Delimiter::Pipe => "|",
        }
    }
}

/// Configuration options for TOON serialization.
///
/// Controls formatting aspects like indentation, delimiters, and special markers.
///
/// # Examples
///
/// ```rust
/// use serde_toon::{ToonOptions, Delimiter};
///
/// // Default compact options
/// let options = ToonOptions::new();
///
/// // Pretty-printed with 2-space indentation
/// let options = ToonOptions::pretty();
///
/// // Custom configuration
/// let options = ToonOptions::new()
///     .with_delimiter(Delimiter::Pipe)
///     .with_length_marker('#')
///     .with_indent(4);
/// ```
#[derive(Clone, Debug)]
pub struct ToonOptions {
    pub indent: usize,
    pub delimiter: Delimiter,
    pub length_marker: Option<char>,
    pub pretty: bool,
}

impl Default for ToonOptions {
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
    /// Creates default options (compact format, comma delimiter, 2-space indent).
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

    /// Creates options for pretty-printed output with newlines and indentation.
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

    /// Sets the indentation size (number of spaces per level).
    ///
    /// Default is 2. Only affects pretty-printed output.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use serde_toon::ToonOptions;
    ///
    /// let options = ToonOptions::pretty().with_indent(4);
    /// assert_eq!(options.indent, 4);
    /// ```
    #[must_use]
    pub fn with_indent(mut self, indent: usize) -> Self {
        self.indent = indent;
        self
    }

    /// Sets the delimiter for arrays and tables.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use serde_toon::{ToonOptions, Delimiter};
    ///
    /// let options = ToonOptions::new().with_delimiter(Delimiter::Pipe);
    /// ```
    #[must_use]
    pub fn with_delimiter(mut self, delimiter: Delimiter) -> Self {
        self.delimiter = delimiter;
        self
    }

    /// Sets an optional length marker character for arrays.
    ///
    /// When set, array lengths are prefixed with this character (e.g., `[#3]` instead of `[3]`).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use serde_toon::ToonOptions;
    ///
    /// let options = ToonOptions::new().with_length_marker('#');
    /// ```
    #[must_use]
    pub fn with_length_marker(mut self, marker: char) -> Self {
        self.length_marker = Some(marker);
        self
    }
}
