//! TOON deserialization.
//!
//! This module provides the [`Deserializer`] implementation that parses
//! TOON format strings into Rust data structures.
//!
//! ## Overview
//!
//! The deserializer handles all TOON format features:
//!
//! - **Single-pass parsing**: Efficient O(n) parsing with no backtracking
//! - **Format detection**: Automatically recognizes inline, list, and tabular formats
//! - **Error reporting**: Detailed error messages with line/column information
//! - **Indentation tracking**: Proper handling of nested structures
//!
//! ## Usage
//!
//! Most users should use the high-level functions in the crate root:
//!
//! ```rust
//! use serde_toon::from_str;
//! use serde::Deserialize;
//!
//! #[derive(Deserialize, Debug, PartialEq)]
//! struct Data { x: i32, y: i32 }
//!
//! let toon = "x: 1\ny: 2";
//! let data: Data = from_str(toon).unwrap();
//! assert_eq!(data, Data { x: 1, y: 2 });
//! ```
//!
//! ## Format Support
//!
//! The deserializer handles multiple array formats:
//!
//! ```rust
//! use serde_toon::from_str;
//!
//! // Inline format
//! let nums: Vec<i32> = from_str("[3]: 1,2,3").unwrap();
//! assert_eq!(nums, vec![1, 2, 3]);
//! ```

use crate::options::Delimiter;
use crate::{Error, Number, Result, ToonMap, Value};
use serde::de::IntoDeserializer;
use serde::{de, forward_to_deserialize_any};

/// The TOON deserializer.
///
/// Parses TOON format strings into Rust values implementing `Deserialize`.
/// Created via [`Deserializer::from_str`].
pub struct Deserializer<'de> {
    input: &'de str,
    position: usize,
    line: usize,
    column: usize,
    indent_stack: Vec<usize>, // Stack of indentation levels for nested scopes
    current_indent: usize,    // Current line's detected indentation
}

impl<'de> Deserializer<'de> {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(input: &'de str) -> Self {
        Deserializer {
            input,
            position: 0,
            line: 1,
            column: 1,
            indent_stack: vec![0], // Start with base indentation level
            current_indent: 0,
        }
    }

    fn peek_char(&self) -> Option<char> {
        self.input[self.position..].chars().next()
    }

    fn next_char(&mut self) -> Option<char> {
        if let Some(ch) = self.input[self.position..].chars().next() {
            self.position += ch.len_utf8();
            if ch == '\n' {
                self.line += 1;
                self.column = 1;
            } else {
                self.column += 1;
            }
            Some(ch)
        } else {
            None
        }
    }

    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.peek_char() {
            if ch.is_whitespace() && ch != '\n' {
                self.next_char();
            } else {
                break;
            }
        }
    }

    /// Checks if we're at the end of input
    fn at_end(&self) -> bool {
        self.position >= self.input.len()
    }

    /// Detects the indentation level at the current position
    /// Counts leading spaces on current line
    fn detect_indent_level(&self) -> usize {
        let mut count = 0;
        let mut pos = self.position;

        while pos < self.input.len() {
            match self.input.as_bytes()[pos] {
                b' ' => {
                    count += 1;
                    pos += 1;
                }
                _ => break,
            }
        }

        count
    }

    /// Pushes a new indentation scope
    fn push_indent(&mut self, level: usize) {
        self.indent_stack.push(level);
    }

    /// Pops the current indentation scope
    fn pop_indent(&mut self) -> Option<usize> {
        if self.indent_stack.len() > 1 {
            self.indent_stack.pop()
        } else {
            None
        }
    }

    /// Skips whitespace on the same line only (no newlines)
    fn skip_whitespace_same_line(&mut self) {
        while let Some(ch) = self.peek_char() {
            if ch == ' ' || ch == '\t' {
                self.next_char();
            } else {
                break;
            }
        }
    }

    fn parse_string(&mut self) -> Result<String> {
        if self.peek_char() == Some('"') {
            self.next_char(); // consume opening quote
            let mut result = String::new();

            while let Some(ch) = self.next_char() {
                match ch {
                    '"' => return Ok(result),
                    '\\' => {
                        match self.next_char() {
                            Some('\\') => result.push('\\'),
                            Some('"') => result.push('"'),
                            Some('n') => result.push('\n'),
                            Some('r') => result.push('\r'),
                            Some('t') => result.push('\t'),
                            Some('b') => result.push('\u{0008}'), // backspace
                            Some('f') => result.push('\u{000C}'), // form feed
                            Some('0') => result.push('\0'),       // null
                            Some('u') => {
                                // Unicode escape: \uXXXX
                                let mut hex = String::new();
                                for _ in 0..4 {
                                    match self.next_char() {
                                        Some(ch) if ch.is_ascii_hexdigit() => hex.push(ch),
                                        _ => return Err(Error::syntax(
                                            self.line,
                                            self.column,
                                            "Invalid unicode escape sequence (expected 4 hex digits)"
                                        )),
                                    }
                                }

                                let code_point = u32::from_str_radix(&hex, 16).map_err(|_| {
                                    Error::syntax(
                                        self.line,
                                        self.column,
                                        "Invalid hex in unicode escape",
                                    )
                                })?;

                                let ch = char::from_u32(code_point).ok_or_else(|| {
                                    Error::syntax(
                                        self.line,
                                        self.column,
                                        "Invalid unicode code point",
                                    )
                                })?;

                                result.push(ch);
                            }
                            Some(other) => {
                                // Unknown escape - preserve literally (lenient parsing)
                                result.push('\\');
                                result.push(other);
                            }
                            None => {
                                return Err(Error::syntax(
                                    self.line,
                                    self.column,
                                    "Unexpected end of input in string",
                                ))
                            }
                        }
                    }
                    other => result.push(other),
                }
            }
            Err(Error::syntax(self.line, self.column, "Unterminated string"))
        } else {
            // Unquoted string - read until delimiter or newline
            let start = self.position;
            while let Some(ch) = self.peek_char() {
                if ch == ':'
                    || ch == ','
                    || ch == '\n'
                    || ch == '\t'
                    || ch == '|'
                    || ch == ']'
                    || ch == '}'
                {
                    break;
                }
                self.next_char();
            }

            if start == self.position {
                Err(Error::syntax(self.line, self.column, "Expected string"))
            } else {
                Ok(self.input[start..self.position].trim().to_string())
            }
        }
    }

    fn parse_number(&mut self) -> Result<Number> {
        let start = self.position;

        // Handle negative sign
        if self.peek_char() == Some('-') {
            self.next_char();
        }

        // Parse digits
        let mut has_decimal = false;
        while let Some(ch) = self.peek_char() {
            if ch.is_ascii_digit() {
                self.next_char();
            } else if ch == '.' && !has_decimal {
                has_decimal = true;
                self.next_char();
            } else {
                break;
            }
        }

        let number_str = &self.input[start..self.position];

        if has_decimal {
            number_str
                .parse::<f64>()
                .map(Number::Float)
                .map_err(|_| Error::syntax(self.line, self.column, "Invalid float"))
        } else {
            number_str
                .parse::<i64>()
                .map(Number::Integer)
                .map_err(|_| Error::syntax(self.line, self.column, "Invalid integer"))
        }
    }

    fn parse_bool(&mut self) -> Result<bool> {
        let _start = self.position;

        // Try to match "true" or "false"
        if self.input[self.position..].starts_with("true") {
            for _ in 0..4 {
                self.next_char();
            }
            Ok(true)
        } else if self.input[self.position..].starts_with("false") {
            for _ in 0..5 {
                self.next_char();
            }
            Ok(false)
        } else {
            Err(Error::syntax(self.line, self.column, "Expected boolean"))
        }
    }

    fn parse_null(&mut self) -> Result<()> {
        if self.input[self.position..].starts_with("null") {
            for _ in 0..4 {
                self.next_char();
            }
            Ok(())
        } else {
            Err(Error::syntax(self.line, self.column, "Expected null"))
        }
    }

    fn parse_array(&mut self) -> Result<Value> {
        // Parse array format like "[3]: a,b,c" or "[2]{id,name}: 1,Alice 2,Bob" or "[3]:"
        if self.peek_char() != Some('[') {
            return Err(Error::syntax(self.line, self.column, "Expected '['"));
        }
        self.next_char(); // consume '['

        // Parse optional length marker
        let _length_marker = if let Some(ch) = self.peek_char() {
            if ch == '#' {
                self.next_char();
                Some('#')
            } else {
                None
            }
        } else {
            None
        };

        // Parse length
        let start = self.position;
        while let Some(ch) = self.peek_char() {
            if ch.is_ascii_digit() {
                self.next_char();
            } else {
                break;
            }
        }

        let declared_length: usize = self.input[start..self.position]
            .parse()
            .map_err(|_| Error::syntax(self.line, self.column, "Invalid array length"))?;

        // Parse optional delimiter indicator in header
        let delimiter = if self.peek_char() == Some('|') {
            self.next_char(); // consume '|'
            Delimiter::Pipe
        } else {
            // Check for tab indicator (spaces in header)
            let mut space_count = 0;
            let temp_pos = self.position;
            while self.peek_char() == Some(' ') {
                self.next_char();
                space_count += 1;
            }
            if space_count >= 4 {
                Delimiter::Tab
            } else {
                // Reset position if not enough spaces
                self.position = temp_pos;
                Delimiter::Comma
            }
        };

        if self.peek_char() != Some(']') {
            return Err(Error::syntax(self.line, self.column, "Expected ']'"));
        }
        self.next_char(); // consume ']'

        // Check if this is a table format
        if self.peek_char() == Some('{') {
            self.parse_table(declared_length, delimiter)
        } else {
            // Simple array format or list format
            if self.peek_char() != Some(':') {
                return Err(Error::syntax(self.line, self.column, "Expected ':'"));
            }
            self.next_char(); // consume ':'

            if declared_length == 0 {
                return Ok(Value::Array(vec![]));
            }

            self.skip_whitespace();

            // Check if this is inline format (same line) or list format (next line with -)
            if self.peek_char() == Some('\n') {
                // List format
                self.parse_list_array(declared_length)
            } else {
                // Inline format
                self.parse_inline_array(declared_length, delimiter)
            }
        }
    }

    fn parse_inline_array(
        &mut self,
        declared_length: usize,
        delimiter: Delimiter,
    ) -> Result<Value> {
        let mut elements = Vec::new();

        for i in 0..declared_length {
            if i > 0 {
                // Skip delimiter
                match delimiter {
                    Delimiter::Comma => {
                        if self.peek_char() == Some(',') {
                            self.next_char();
                        }
                    }
                    Delimiter::Tab => {
                        if self.peek_char() == Some('\t') {
                            self.next_char();
                        }
                    }
                    Delimiter::Pipe => {
                        if self.peek_char() == Some('|') {
                            self.next_char();
                        }
                    }
                }
                self.skip_whitespace();
            }

            let value = self.parse_primitive_value()?;
            elements.push(value);
        }

        Ok(Value::Array(elements))
    }

    fn parse_list_array(&mut self, declared_length: usize) -> Result<Value> {
        let mut elements = Vec::new();

        for _ in 0..declared_length {
            // Skip to next line
            if self.peek_char() == Some('\n') {
                self.next_char();
            }

            // Update current indentation level for proper nested object parsing
            self.current_indent = self.detect_indent_level();
            self.skip_whitespace();

            // Expect "- " prefix
            if self.peek_char() != Some('-') {
                return Err(Error::syntax(
                    self.line,
                    self.column,
                    "Expected '- ' prefix in list format",
                ));
            }
            self.next_char(); // consume '-'

            if self.peek_char() != Some(' ') {
                return Err(Error::syntax(
                    self.line,
                    self.column,
                    "Expected space after '-'",
                ));
            }
            self.next_char(); // consume ' '

            let value = self.parse_value()?;
            elements.push(value);
        }

        Ok(Value::Array(elements))
    }

    fn parse_table(&mut self, declared_length: usize, delimiter: Delimiter) -> Result<Value> {
        // Parse table headers
        if self.peek_char() != Some('{') {
            return Err(Error::syntax(self.line, self.column, "Expected '{'"));
        }
        self.next_char(); // consume '{'

        let mut headers = Vec::new();

        while !self.at_end() && self.peek_char() != Some('}') {
            let header = self.parse_string()?;
            headers.push(header);

            if self.peek_char() == Some(',') {
                self.next_char();
            } else {
                break;
            }
        }

        if self.peek_char() != Some('}') {
            return Err(Error::syntax(self.line, self.column, "Expected '}'"));
        }
        self.next_char(); // consume '}'

        if self.peek_char() != Some(':') {
            return Err(Error::syntax(self.line, self.column, "Expected ':'"));
        }
        self.next_char(); // consume ':'

        // Parse table rows
        let mut rows = Vec::new();

        for _ in 0..declared_length {
            // Skip to next line
            if self.peek_char() == Some('\n') {
                self.next_char();
            }
            self.skip_whitespace();

            if self.at_end() {
                break;
            }

            // Parse row
            let mut row = Vec::new();

            for (i, _header) in headers.iter().enumerate() {
                if i > 0 {
                    // Skip delimiter
                    match delimiter {
                        Delimiter::Comma => {
                            if self.peek_char() == Some(',') {
                                self.next_char();
                            }
                        }
                        Delimiter::Tab => {
                            if self.peek_char() == Some('\t') {
                                self.next_char();
                            }
                        }
                        Delimiter::Pipe => {
                            if self.peek_char() == Some('|') {
                                self.next_char();
                            }
                        }
                    }
                    self.skip_whitespace();
                }

                let value = self.parse_primitive_value()?;
                row.push(value);
            }

            rows.push(row);
        }

        Ok(Value::Table { headers, rows })
    }

    fn parse_object(&mut self) -> Result<Value> {
        let mut map = ToonMap::new();

        // Detect the base indentation for this object
        let base_indent = self.current_indent;

        // Handle empty object case - peek ahead
        self.skip_whitespace_same_line();
        if self.peek_char() == Some('\n') || self.at_end() {
            // Empty object - no fields on same line, check next line
            if self.peek_char() == Some('\n') {
                let saved_pos = self.position;
                let saved_line = self.line;
                let saved_col = self.column;

                self.next_char(); // consume newline
                let next_indent = self.detect_indent_level();

                // If next line has LESS indent (not same), object is empty
                // Exception: if base_indent is 0, we need to check if there's content
                if base_indent > 0 && next_indent < base_indent {
                    // Restore position
                    self.position = saved_pos;
                    self.line = saved_line;
                    self.column = saved_col;
                    return Ok(Value::Object(map));
                } else if base_indent == 0 && next_indent == 0 {
                    // For top-level objects, check if next line looks like a key
                    // by looking for a ':' character
                    let mut temp_pos = self.position;
                    let mut found_colon = false;
                    while temp_pos < self.input.len() {
                        match self.input.as_bytes()[temp_pos] {
                            b':' => {
                                found_colon = true;
                                break;
                            }
                            b'\n' => break,
                            _ => temp_pos += 1,
                        }
                    }

                    if !found_colon {
                        // No colon found, assume empty object
                        self.position = saved_pos;
                        self.line = saved_line;
                        self.column = saved_col;
                        return Ok(Value::Object(map));
                    }
                }

                // Otherwise continue parsing with content
                self.position = saved_pos;
                self.line = saved_line;
                self.column = saved_col;
            } else if self.at_end() {
                return Ok(Value::Object(map));
            }
        }

        // Push indent scope
        self.push_indent(base_indent);

        loop {
            self.skip_whitespace_same_line();

            // Check for newline or end
            if self.peek_char() == Some('\n') {
                self.next_char(); // consume newline
                self.current_indent = self.detect_indent_level();

                // Check if we've dedented (exited object scope)
                // For nested objects (base_indent > 0), dedent means we've exited
                // For top-level objects (base_indent == 0), we stay at the same level
                if base_indent > 0 && self.current_indent < base_indent {
                    self.pop_indent();
                    break;
                }

                // Skip blank lines
                if self.peek_char() == Some('\n') {
                    continue;
                }
            } else if self.at_end() {
                self.pop_indent();
                break;
            }

            // Skip any leading whitespace on this line
            self.skip_whitespace_same_line();

            if self.at_end() || self.peek_char() == Some('\n') {
                continue;
            }

            // Parse key
            let key = self.parse_string()?;

            self.skip_whitespace_same_line();

            if self.peek_char() != Some(':') {
                return Err(Error::syntax(
                    self.line,
                    self.column,
                    "Expected ':' after key",
                ));
            }
            self.next_char(); // consume ':'

            self.skip_whitespace_same_line();

            // Check if value is on same line or nested
            if self.peek_char() == Some('\n') || self.at_end() {
                // Value is on next line(s) - nested structure
                if self.peek_char() == Some('\n') {
                    self.next_char(); // consume newline
                    self.current_indent = self.detect_indent_level();
                }

                let value = self.parse_value()?;
                map.insert(key, value);
            } else {
                // Inline value
                let value = self.parse_value()?;
                map.insert(key, value);
            }

            // Continue to next field or end
            self.skip_whitespace_same_line();
        }

        Ok(Value::Object(map))
    }

    fn parse_primitive_value(&mut self) -> Result<Value> {
        self.skip_whitespace();

        match self.peek_char() {
            Some('"') => Ok(Value::String(self.parse_string()?)),
            Some('t') | Some('f') => Ok(Value::Bool(self.parse_bool()?)),
            Some('n') => {
                self.parse_null()?;
                Ok(Value::Null)
            }
            Some(ch) if ch.is_ascii_digit() || ch == '-' => Ok(Value::Number(self.parse_number()?)),
            _ => {
                // Try parsing as unquoted string
                let s = self.parse_string()?;
                if s == "true" {
                    Ok(Value::Bool(true))
                } else if s == "false" {
                    Ok(Value::Bool(false))
                } else if s == "null" {
                    Ok(Value::Null)
                } else if let Ok(n) = s.parse::<i64>() {
                    Ok(Value::Number(Number::Integer(n)))
                } else if let Ok(f) = s.parse::<f64>() {
                    Ok(Value::Number(Number::Float(f)))
                } else {
                    Ok(Value::String(s))
                }
            }
        }
    }

    fn parse_value(&mut self) -> Result<Value> {
        self.skip_whitespace();

        match self.peek_char() {
            Some('[') => self.parse_array(),
            Some('"') => Ok(Value::String(self.parse_string()?)),
            Some('t') | Some('f') => Ok(Value::Bool(self.parse_bool()?)),
            Some('n') => {
                self.parse_null()?;
                Ok(Value::Null)
            }
            Some(ch) if ch.is_ascii_digit() || ch == '-' => Ok(Value::Number(self.parse_number()?)),
            _ => {
                // Check if we're at end of input (empty object case)
                if self.at_end() {
                    return Ok(Value::Object(ToonMap::new()));
                }

                // Could be object or unquoted string
                // Look ahead to see if we have key: value pattern
                let _start_pos = self.position;
                let _start_line = self.line;
                let _start_column = self.column;

                // Try to find ':'
                let mut found_colon = false;
                let mut temp_pos = self.position;

                while temp_pos < self.input.len() {
                    match self.input.as_bytes()[temp_pos] {
                        b':' => {
                            found_colon = true;
                            break;
                        }
                        b'\n' => break,
                        _ => temp_pos += 1,
                    }
                }

                if found_colon {
                    // Parse as object
                    self.parse_object()
                } else {
                    // Parse as string
                    let s = self.parse_string()?;
                    if s == "true" {
                        Ok(Value::Bool(true))
                    } else if s == "false" {
                        Ok(Value::Bool(false))
                    } else if s == "null" {
                        Ok(Value::Null)
                    } else if let Ok(n) = s.parse::<i64>() {
                        Ok(Value::Number(Number::Integer(n)))
                    } else if let Ok(f) = s.parse::<f64>() {
                        Ok(Value::Number(Number::Float(f)))
                    } else {
                        Ok(Value::String(s))
                    }
                }
            }
        }
    }
}

impl<'de> de::Deserializer<'de> for &mut Deserializer<'de> {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        let value = self.parse_value()?;
        match value {
            Value::Null => visitor.visit_unit(),
            Value::Bool(b) => visitor.visit_bool(b),
            Value::Number(Number::Integer(i)) => visitor.visit_i64(i),
            Value::Number(Number::Float(f)) => visitor.visit_f64(f),
            Value::Number(Number::Infinity) => visitor.visit_f64(f64::INFINITY),
            Value::Number(Number::NegativeInfinity) => visitor.visit_f64(f64::NEG_INFINITY),
            Value::Number(Number::NaN) => visitor.visit_f64(f64::NAN),
            Value::String(s) => visitor.visit_string(s),
            Value::Array(arr) => visitor.visit_seq(SeqDeserializer::new(arr)),
            Value::Object(obj) => visitor.visit_map(MapDeserializer::new(obj)),
            Value::Table { headers, rows } => {
                // Convert table to array of objects
                let mut objects = Vec::new();
                for row in rows {
                    let mut obj = ToonMap::new();
                    for (i, value) in row.into_iter().enumerate() {
                        if let Some(header) = headers.get(i) {
                            obj.insert(header.clone(), value);
                        }
                    }
                    objects.push(Value::Object(obj));
                }
                visitor.visit_seq(SeqDeserializer::new(objects))
            }
            Value::Date(dt) => visitor.visit_string(dt.to_rfc3339()),
            Value::BigInt(bi) => visitor.visit_string(format!("{}n", bi)),
        }
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_bool(self.parse_bool()?)
    }

    fn deserialize_i8<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        match self.parse_number()? {
            Number::Integer(i) => visitor.visit_i8(i as i8),
            Number::Float(f) => visitor.visit_i8(f as i8),
            Number::Infinity => visitor.visit_i8(i8::MAX),
            Number::NegativeInfinity => visitor.visit_i8(i8::MIN),
            Number::NaN => visitor.visit_i8(0),
        }
    }

    fn deserialize_i16<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        match self.parse_number()? {
            Number::Integer(i) => visitor.visit_i16(i as i16),
            Number::Float(f) => visitor.visit_i16(f as i16),
            Number::Infinity => visitor.visit_i16(i16::MAX),
            Number::NegativeInfinity => visitor.visit_i16(i16::MIN),
            Number::NaN => visitor.visit_i16(0),
        }
    }

    fn deserialize_i32<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        match self.parse_number()? {
            Number::Integer(i) => visitor.visit_i32(i as i32),
            Number::Float(f) => visitor.visit_i32(f as i32),
            Number::Infinity => visitor.visit_i32(i32::MAX),
            Number::NegativeInfinity => visitor.visit_i32(i32::MIN),
            Number::NaN => visitor.visit_i32(0),
        }
    }

    fn deserialize_i64<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        match self.parse_number()? {
            Number::Integer(i) => visitor.visit_i64(i),
            Number::Float(f) => visitor.visit_i64(f as i64),
            Number::Infinity => visitor.visit_i64(i64::MAX),
            Number::NegativeInfinity => visitor.visit_i64(i64::MIN),
            Number::NaN => visitor.visit_i64(0),
        }
    }

    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        match self.parse_number()? {
            Number::Integer(i) => visitor.visit_u8(i as u8),
            Number::Float(f) => visitor.visit_u8(f as u8),
            Number::Infinity => visitor.visit_u8(u8::MAX),
            Number::NegativeInfinity => visitor.visit_u8(u8::MIN),
            Number::NaN => visitor.visit_u8(0),
        }
    }

    fn deserialize_u16<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        match self.parse_number()? {
            Number::Integer(i) => visitor.visit_u16(i as u16),
            Number::Float(f) => visitor.visit_u16(f as u16),
            Number::Infinity => visitor.visit_u16(u16::MAX),
            Number::NegativeInfinity => visitor.visit_u16(u16::MIN),
            Number::NaN => visitor.visit_u16(0),
        }
    }

    fn deserialize_u32<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        match self.parse_number()? {
            Number::Integer(i) => visitor.visit_u32(i as u32),
            Number::Float(f) => visitor.visit_u32(f as u32),
            Number::Infinity => visitor.visit_u32(u32::MAX),
            Number::NegativeInfinity => visitor.visit_u32(u32::MIN),
            Number::NaN => visitor.visit_u32(0),
        }
    }

    fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        match self.parse_number()? {
            Number::Integer(i) => visitor.visit_u64(i as u64),
            Number::Float(f) => visitor.visit_u64(f as u64),
            Number::Infinity => visitor.visit_u64(u64::MAX),
            Number::NegativeInfinity => visitor.visit_u64(u64::MIN),
            Number::NaN => visitor.visit_u64(0),
        }
    }

    fn deserialize_f32<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_f32(self.parse_number()?.as_f64() as f32)
    }

    fn deserialize_f64<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_f64(self.parse_number()?.as_f64())
    }

    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        let s = self.parse_string()?;
        if s.len() == 1 {
            visitor.visit_char(s.chars().next().unwrap())
        } else {
            Err(Error::custom("Expected single character"))
        }
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_string(self.parse_string()?)
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_string(self.parse_string()?)
    }

    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        if self.peek_char() == Some('n') && self.input[self.position..].starts_with("null") {
            self.parse_null()?;
            visitor.visit_none()
        } else {
            visitor.visit_some(self)
        }
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        self.parse_null()?;
        visitor.visit_unit()
    }

    fn deserialize_unit_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        self.deserialize_unit(visitor)
    }

    fn deserialize_newtype_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        let value = self.parse_value()?;
        match value {
            Value::Array(arr) => visitor.visit_seq(SeqDeserializer::new(arr)),
            Value::Table { headers, rows } => {
                let mut objects = Vec::new();
                for row in rows {
                    let mut obj = ToonMap::new();
                    for (i, value) in row.into_iter().enumerate() {
                        if let Some(header) = headers.get(i) {
                            obj.insert(header.clone(), value);
                        }
                    }
                    objects.push(Value::Object(obj));
                }
                visitor.visit_seq(SeqDeserializer::new(objects))
            }
            _ => Err(Error::custom("Expected array")),
        }
    }

    fn deserialize_tuple<V>(self, _len: usize, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        let value = self.parse_value()?;
        match value {
            Value::Object(obj) => visitor.visit_map(MapDeserializer::new(obj)),
            _ => Err(Error::custom("Expected object")),
        }
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        self.deserialize_map(visitor)
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        let value = self.parse_value()?;
        match value {
            Value::String(s) => visitor.visit_enum(s.into_deserializer()),
            Value::Object(obj) => {
                if obj.len() == 1 {
                    let (variant, value) = obj.into_iter().next().unwrap();
                    visitor.visit_enum(EnumDeserializer::new(variant, value))
                } else {
                    Err(Error::custom("Expected enum variant"))
                }
            }
            _ => Err(Error::custom("Expected enum")),
        }
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        self.deserialize_string(visitor)
    }

    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        self.deserialize_any(visitor)
    }
}

struct SeqDeserializer {
    iter: std::vec::IntoIter<Value>,
}

impl SeqDeserializer {
    fn new(vec: Vec<Value>) -> Self {
        SeqDeserializer {
            iter: vec.into_iter(),
        }
    }
}

impl<'de> de::SeqAccess<'de> for SeqDeserializer {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
    where
        T: de::DeserializeSeed<'de>,
    {
        match self.iter.next() {
            Some(value) => seed.deserialize(ValueDeserializer::new(value)).map(Some),
            None => Ok(None),
        }
    }

    fn size_hint(&self) -> Option<usize> {
        match self.iter.size_hint() {
            (lower, Some(upper)) if lower == upper => Some(upper),
            _ => None,
        }
    }
}

struct MapDeserializer {
    iter: indexmap::map::IntoIter<String, Value>,
    value: Option<Value>,
}

impl MapDeserializer {
    fn new(map: ToonMap) -> Self {
        MapDeserializer {
            iter: map.into_iter(),
            value: None,
        }
    }
}

impl<'de> de::MapAccess<'de> for MapDeserializer {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>>
    where
        K: de::DeserializeSeed<'de>,
    {
        match self.iter.next() {
            Some((key, value)) => {
                self.value = Some(value);
                seed.deserialize(ValueDeserializer::new(Value::String(key)))
                    .map(Some)
            }
            None => Ok(None),
        }
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value>
    where
        V: de::DeserializeSeed<'de>,
    {
        match self.value.take() {
            Some(value) => seed.deserialize(ValueDeserializer::new(value)),
            None => Err(Error::custom("next_value_seed called before next_key_seed")),
        }
    }

    fn size_hint(&self) -> Option<usize> {
        match self.iter.size_hint() {
            (lower, Some(upper)) if lower == upper => Some(upper),
            _ => None,
        }
    }
}

struct EnumDeserializer {
    variant: String,
    value: Option<Value>,
}

impl EnumDeserializer {
    fn new(variant: String, value: Value) -> Self {
        EnumDeserializer {
            variant,
            value: Some(value),
        }
    }
}

impl<'de> de::EnumAccess<'de> for EnumDeserializer {
    type Error = Error;
    type Variant = VariantDeserializer;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant)>
    where
        V: de::DeserializeSeed<'de>,
    {
        let variant = seed.deserialize(ValueDeserializer::new(Value::String(self.variant)))?;
        let visitor = VariantDeserializer { value: self.value };
        Ok((variant, visitor))
    }
}

struct VariantDeserializer {
    value: Option<Value>,
}

impl<'de> de::VariantAccess<'de> for VariantDeserializer {
    type Error = Error;

    fn unit_variant(self) -> Result<()> {
        match self.value {
            Some(Value::Null) | None => Ok(()),
            _ => Err(Error::custom("Expected unit variant")),
        }
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value>
    where
        T: de::DeserializeSeed<'de>,
    {
        match self.value {
            Some(value) => seed.deserialize(ValueDeserializer::new(value)),
            None => Err(Error::custom("Expected newtype variant")),
        }
    }

    fn tuple_variant<V>(self, _len: usize, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        match self.value {
            Some(Value::Array(arr)) => visitor.visit_seq(SeqDeserializer::new(arr)),
            _ => Err(Error::custom("Expected tuple variant")),
        }
    }

    fn struct_variant<V>(self, _fields: &'static [&'static str], visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        match self.value {
            Some(Value::Object(obj)) => visitor.visit_map(MapDeserializer::new(obj)),
            _ => Err(Error::custom("Expected struct variant")),
        }
    }
}

struct ValueDeserializer {
    value: Value,
}

impl ValueDeserializer {
    fn new(value: Value) -> Self {
        ValueDeserializer { value }
    }
}

impl<'de> de::Deserializer<'de> for ValueDeserializer {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        match self.value {
            Value::Null => visitor.visit_unit(),
            Value::Bool(b) => visitor.visit_bool(b),
            Value::Number(Number::Integer(i)) => visitor.visit_i64(i),
            Value::Number(Number::Float(f)) => visitor.visit_f64(f),
            Value::Number(Number::Infinity) => visitor.visit_f64(f64::INFINITY),
            Value::Number(Number::NegativeInfinity) => visitor.visit_f64(f64::NEG_INFINITY),
            Value::Number(Number::NaN) => visitor.visit_f64(f64::NAN),
            Value::String(s) => visitor.visit_string(s),
            Value::Array(arr) => visitor.visit_seq(SeqDeserializer::new(arr)),
            Value::Object(obj) => visitor.visit_map(MapDeserializer::new(obj)),
            Value::Table { headers, rows } => {
                let mut objects = Vec::new();
                for row in rows {
                    let mut obj = ToonMap::new();
                    for (i, value) in row.into_iter().enumerate() {
                        if let Some(header) = headers.get(i) {
                            obj.insert(header.clone(), value);
                        }
                    }
                    objects.push(Value::Object(obj));
                }
                visitor.visit_seq(SeqDeserializer::new(objects))
            }
            Value::Date(dt) => visitor.visit_string(dt.to_rfc3339()),
            Value::BigInt(bi) => visitor.visit_string(format!("{}n", bi)),
        }
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }
}
