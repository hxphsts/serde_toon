//! Error types for TOON serialization and deserialization.
//!
//! This module provides comprehensive error reporting with contextual information
//! to help diagnose and fix TOON format issues.
//!
//! ## Error Categories
//!
//! - **Syntax Errors**: Invalid TOON syntax with line/column information
//! - **Type Mismatches**: Attempted to deserialize to incompatible type
//! - **Indentation Errors**: Incorrect nesting/indentation (TOON uses 2-space indents)
//! - **I/O Errors**: File reading/writing failures
//!
//! ## Error Context
//!
//! All parsing errors include:
//! - Line and column numbers
//! - Context showing the problematic code
//! - Helpful suggestions for common mistakes
//!
//! ## Examples
//!
//! ```rust
//! use serde_toon::{from_str, Error};
//!
//! let result: Result<serde_toon::Value, Error> = from_str("invalid: [malformed");
//! assert!(result.is_err());
//!
//! if let Err(err) = result {
//!     eprintln!("Parse error: {}", err);
//!     // Error messages include line numbers and suggestions
//! }
//! ```

use std::fmt;
use thiserror::Error;

/// Represents all possible errors that can occur during TOON serialization/deserialization.
///
/// Each error variant includes contextual information to aid debugging.
#[derive(Debug, Clone, Error)]
pub enum Error {
    /// IO error during reading or writing
    #[error("IO error: {0}")]
    Io(String),

    /// Syntax error with detailed context
    #[error("Syntax error at line {line}, column {col}:\n{context}\n{msg}{suggestion}")]
    Syntax {
        line: usize,
        col: usize,
        msg: String,
        context: String,
        suggestion: String,
    },

    /// Type mismatch during deserialization
    #[error("Type mismatch at line {line}, column {col}: expected {expected}, found {found}")]
    TypeMismatch {
        line: usize,
        col: usize,
        expected: String,
        found: String,
    },

    /// Indentation error in nested structures
    #[error("Indentation error at line {line}, column {col}:\n{context}\nExpected {expected} spaces, found {found} spaces\nHelp: TOON uses 2-space indentation for nested objects")]
    IndentationError {
        line: usize,
        col: usize,
        expected: usize,
        found: usize,
        context: String,
    },

    /// Unsupported type for serialization
    #[error("Unsupported type: {0}")]
    UnsupportedType(String),

    /// Invalid TOON format
    #[error("Invalid TOON format at line {line}, column {col}: {msg}")]
    InvalidFormat {
        line: usize,
        col: usize,
        msg: String,
    },

    /// Unexpected end of input
    #[error(
        "Unexpected end of input at line {line}, column {col}\n{context}\nExpected: {expected}"
    )]
    UnexpectedEof {
        line: usize,
        col: usize,
        expected: String,
        context: String,
    },

    /// Custom error
    #[error("Error: {0}")]
    Custom(String),

    /// Generic message
    #[error("{0}")]
    Message(String),
}

impl Error {
    /// Creates a syntax error with line and column information.
    ///
    /// Use [`Error::syntax_with_context`] for more detailed error messages.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use serde_toon::Error;
    ///
    /// let err = Error::syntax(10, 5, "unexpected token");
    /// assert!(err.to_string().contains("line 10"));
    /// ```
    pub fn syntax(line: usize, col: usize, msg: &str) -> Self {
        Error::Syntax {
            line,
            col,
            msg: msg.to_string(),
            context: String::new(),
            suggestion: String::new(),
        }
    }

    /// Creates a syntax error with full context and helpful suggestion.
    ///
    /// This provides richer error messages than [`Error::syntax`] by including
    /// the problematic code context and an optional suggestion.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use serde_toon::Error;
    ///
    /// let err = Error::syntax_with_context(
    ///     10,
    ///     5,
    ///     "missing colon",
    ///     "name Alice",
    ///     Some("Did you mean 'name: Alice'?"),
    /// );
    /// assert!(err.to_string().contains("Help:"));
    /// ```
    pub fn syntax_with_context(
        line: usize,
        col: usize,
        msg: &str,
        context: &str,
        suggestion: Option<&str>,
    ) -> Self {
        Error::Syntax {
            line,
            col,
            msg: msg.to_string(),
            context: context.to_string(),
            suggestion: suggestion
                .map(|s| format!("\nHelp: {}", s))
                .unwrap_or_default(),
        }
    }

    /// Creates a type mismatch error when deserialization fails due to incompatible types.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use serde_toon::Error;
    ///
    /// let err = Error::type_mismatch(5, 10, "integer", "string");
    /// assert!(err.to_string().contains("expected integer"));
    /// ```
    pub fn type_mismatch(line: usize, col: usize, expected: &str, found: &str) -> Self {
        Error::TypeMismatch {
            line,
            col,
            expected: expected.to_string(),
            found: found.to_string(),
        }
    }

    /// Creates an indentation error (TOON uses 2-space indentation for nested objects).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use serde_toon::Error;
    ///
    /// let err = Error::indentation_error(8, 1, 2, 4, "  name: Alice");
    /// assert!(err.to_string().contains("Expected 2 spaces"));
    /// ```
    pub fn indentation_error(
        line: usize,
        col: usize,
        expected: usize,
        found: usize,
        context: &str,
    ) -> Self {
        Error::IndentationError {
            line,
            col,
            expected,
            found,
            context: context.to_string(),
        }
    }

    /// Creates an invalid format error for malformed TOON syntax.
    pub fn invalid_format(line: usize, col: usize, msg: &str) -> Self {
        Error::InvalidFormat {
            line,
            col,
            msg: msg.to_string(),
        }
    }

    /// Creates an unexpected end-of-file error.
    pub fn unexpected_eof(line: usize, col: usize, expected: &str, context: &str) -> Self {
        Error::UnexpectedEof {
            line,
            col,
            expected: expected.to_string(),
            context: context.to_string(),
        }
    }

    /// Creates an unsupported type error for types that cannot be serialized to TOON.
    pub fn unsupported_type(msg: &str) -> Self {
        Error::UnsupportedType(msg.to_string())
    }

    /// Creates a custom error with a display message.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use serde_toon::Error;
    ///
    /// let err = Error::custom("something went wrong");
    /// assert!(err.to_string().contains("something went wrong"));
    /// ```
    pub fn custom<T: fmt::Display>(msg: T) -> Self {
        Error::Custom(msg.to_string())
    }

    /// Creates an I/O error for file reading/writing failures.
    pub fn io(msg: &str) -> Self {
        Error::Io(msg.to_string())
    }
}

impl serde::ser::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Error::Custom(msg.to_string())
    }
}

impl serde::de::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Error::Custom(msg.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
