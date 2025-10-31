//! TOON Format Specification
//!
//! This module documents the TOON (Token-Oriented Object Notation) format specification
//! as implemented by this library.
//!
//! # Overview
//!
//! TOON is a minimalist data serialization format designed for efficient token usage in
//! Large Language Model (LLM) contexts. It achieves 30-60% token reduction compared to
//! JSON while maintaining human readability and structural clarity.
//!
//! ## Design Philosophy
//!
//! - **Token Efficiency**: Eliminate syntactic overhead (braces, brackets, redundant quotes)
//! - **Readability**: Maintain clarity through meaningful indentation and structure
//! - **Structural Typing**: Leverage homogeneous data patterns for tabular compression
//! - **Compatibility**: Support LLM-specific values (Infinity, NaN) and standard types
//!
//! # Core Syntax
//!
//! ## Objects
//!
//! Objects use newline-delimited key-value pairs with colon separation:
//!
//! ```text
//! name: Alice
//! age: 30
//! active: true
//! ```
//!
//! **Rules**:
//! - Keys must match identifier pattern `/^[a-zA-Z_][a-zA-Z0-9_.]*$/` OR be quoted:
//!   - Start with letter (`a-z`, `A-Z`) or underscore `_`
//!   - Contain only letters, digits, underscores, or dots
//!   - Examples: `userName`, `user_name`, `user.email` (all valid unquoted)
//!   - Keys starting with digits or containing hyphens must be quoted: `"2ndPlace"`, `"user-id"`
//! - Values follow the `:` separator (space after `:` is optional but recommended)
//! - Nested objects are indented (default 2 spaces per level)
//! - Field order: This implementation sorts fields alphabetically for deterministic output
//!   (not required by spec, but recommended for consistency)
//!
//! ## Primitives
//!
//! | Type | Syntax | Example |
//! |------|--------|---------|
//! | Null | `null` | `value: null` |
//! | Boolean | `true` or `false` | `active: true` |
//! | Integer | Decimal digits, optional `-` | `count: 42` |
//! | Float | Decimal with `.` | `price: 19.99` |
//! | Special Numbers | `Infinity`, `-Infinity`, `NaN` | `limit: Infinity` (converted to `null` by default) |
//! | String | Unquoted or `"quoted"` | `name: Alice` |
//! | Date | ISO 8601 format | `created: 2024-01-15T10:30:00Z` |
//! | BigInt | Large integers | `large: 999999999999999999` |
//!
//! ## Strings
//!
//! Strings are **unquoted by default** to minimize tokens. Quoting is required when:
//!
//! - String is empty or contains only whitespace: `""`, `"  "`
//! - Contains the **active delimiter** for the current context:
//!   - Comma delimiter (default): strings with `,` must be quoted
//!   - Tab delimiter: strings with `\t` must be quoted
//!   - Pipe delimiter: strings with `|` must be quoted
//!   - Note: Only the active delimiter triggers quoting; others remain safe
//! - Contains colon `:` (conflicts with key-value separator)
//! - Contains quotes, backslashes, or control characters: `"`, `\`, `\n`, `\r`, `\t`
//! - Starts or ends with whitespace (trimming ambiguity)
//! - Matches reserved words: `true`, `false`, `null`, `Infinity`, `-Infinity`, `NaN`
//! - Parses as a number (would be ambiguous): `"42"`, `"-3.14"`, `"1e-6"`
//! - Starts with `"- "` (looks like list item marker)
//! - Looks like structural tokens: `"[5]"`, `"{key}"`, `"[3]: x,y"`
//!
//! **Examples**:
//! ```text
//! name: Alice          # Unquoted (safe)
//! note: hello world    # Unquoted (inner spaces OK)
//! emoji: 👋 hello      # Unquoted (Unicode safe)
//! data: "hello,world"  # Quoted (contains comma delimiter)
//! flag: "true"         # Quoted (reserved word)
//! id: "42"             # Quoted (parses as number)
//! ```
//!
//! **Escape sequences** (in quoted strings):
//! ```text
//! \"  - quote
//! \\  - backslash
//! \n  - newline
//! \r  - carriage return
//! \t  - tab
//! \b  - backspace
//! \f  - form feed
//! \0  - null character
//! \uXXXX - Unicode codepoint (4 hex digits)
//! ```
//!
//! # Type Conversions
//!
//! TOON handles JavaScript/TypeScript type conversions for LLM-safe output:
//!
//! | Input Type | TOON Output | Notes |
//! |------------|-------------|-------|
//! | Finite numbers | Decimal notation | No scientific notation: `1000000` not `1e6`, `-0` becomes `0` |
//! | `NaN`, `±Infinity` | `null` | Non-finite numbers converted to null by default (preservable with option) |
//! | `BigInt` | Number or quoted string | If within safe integer range: number. Otherwise: `"9007199254740993"` |
//! | `Date` | Quoted ISO 8601 string | `"2024-01-15T10:30:00.000Z"` |
//! | `undefined` | Omitted or `null` | Omitted from objects, becomes `null` in arrays |
//! | `function` | `null` | Not serializable |
//! | `symbol` | `null` | Not serializable |
//!
//! **Example**:
//! ```javascript
//! {
//!   count: 1e6,              // → count: 1000000
//!   limit: Infinity,         // → limit: null
//!   large: 9007199254740993n // → large: "9007199254740993"
//! }
//! ```
//!
//! # Array Formats
//!
//! TOON uses three array formats based on content structure.
//!
//! ## Inline Arrays
//!
//! For **primitive values** (numbers, booleans, strings, null):
//!
//! ```text
//! [3]: 1,2,3
//! [2]: Alice,Bob
//! [4]: true,false,null,42
//! ```
//!
//! **Syntax**: `[N]: element1,element2,...`
//! - `N` = array length
//! - Elements comma-separated (no spaces by default)
//! - Space after `:` is conventional
//!
//! ## List Arrays
//!
//! For **complex or heterogeneous elements**:
//!
//! ```text
//! [2]:
//!   - name: Alice
//!     role: admin
//!   - name: Bob
//!     role: user
//! ```
//!
//! **Syntax**: `[N]:` followed by indented items with `- ` prefix
//! - Each item on new line, indented 2 spaces
//! - `- ` marks start of item
//! - First field can appear on the same line as the hyphen: `- name: Alice`
//! - Subsequent fields are indented to align under the first field (hyphen + 2 spaces)
//! - Nested structures (arrays/objects) are indented 2 additional spaces from their parent
//!
//! **Example with nested array**:
//! ```text
//! [2]:
//!   - name: Alice
//!     tags: [2]: admin,user
//!   - name: Bob
//!     tags: [1]: user
//! ```
//!
//! ## Tabular Arrays
//!
//! For **homogeneous objects with primitive fields** (TOON's signature feature):
//!
//! ```text
//! [3]{id,name,price}:
//!   1,Widget,9.99
//!   2,Gadget,14.99
//!   3,Tool,19.99
//! ```
//!
//! **Syntax**: `[N]{field1,field2,...}:` followed by rows
//! - Headers in `{}` define field order (alphabetically sorted)
//! - One row per line, indented
//! - Values comma-separated, matching header order
//! - All objects must have identical structure
//! - All field values must be primitives (no nested objects/arrays)
//!
//! **Token savings**: Tabular format eliminates repetitive keys, achieving maximum compression
//! for structured datasets.
//!
//! # Delimiters
//!
//! TOON supports three delimiters for arrays and tables:
//!
//! | Delimiter | Character | Header Encoding | Use Case |
//! |-----------|-----------|-----------------|----------|
//! | Comma (default) | `,` | (none) | Most compact |
//! | Tab | `\t` | `[N    ]` (4 spaces) | TSV-like output |
//! | Pipe | `\|` | `[N\|]` | Markdown tables |
//!
//! **Encoding**: Non-comma delimiters are indicated in array/table headers:
//!
//! Tab-delimited array (4 spaces in header):
//! ```text
//! [3    ]: 1    2    3
//! ```
//!
//! Pipe-delimited array:
//! ```text
//! [3|]: 1|2|3
//! ```
//!
//! Pipe-delimited table:
//! ```text
//! [3]{a|b|c}:
//!   1|2|3
//! ```
//!
//! # Length Markers
//!
//! Optional character prefix for array lengths (e.g., `#` for clarity):
//!
//! With marker:
//! ```text
//! [#3]: 1,2,3
//! ```
//!
//! Without marker:
//! ```text
//! [3]: 1,2,3
//! ```
//!
//! # Indentation
//!
//! - **Default**: 2 spaces per nesting level
//! - **Purpose**: Visual structure for nested objects/arrays
//! - **Parsing**: Indent level determines scope boundaries
//!
//! # Edge Cases
//!
//! ## Empty Collections
//!
//! Empty array:
//! ```text
//! empty_array: [0]:
//! ```
//!
//! Empty object (key with no fields below):
//! ```text
//! empty_object:
//! ```
//!
//! Root-level empty array:
//! ```text
//! [0]:
//! ```
//!
//! ## Nested Structures
//!
//! ```text
//! user:
//!   name: Alice
//!   tags: [2]: admin,user
//!   metadata:
//!     created: 2024-01-01
//!     verified: true
//! ```
//!
//! ## Rust-Specific Serialization
//!
//! This implementation handles Rust enum variants as follows:
//!
//! - **Unit variants**: Serialized as strings: `status: Active`
//! - **Newtype variants**: `Error: "Some message"`
//! - **Struct and tuple variants**: Serialized as objects or arrays
//!
//! Note: These are implementation details of the Rust library, not part of the core TOON specification.
//!
//! # Format Comparison
//!
//! **JSON** (171 chars):
//! ```json
//! [
//!   {"id":1,"name":"Alice","email":"alice@ex.com","active":true},
//!   {"id":2,"name":"Bob","email":"bob@ex.com","active":true}
//! ]
//! ```
//!
//! **TOON** (86 chars, 50% reduction):
//! ```text
//! [2]{active,email,id,name}:
//!   true,alice@ex.com,1,Alice
//!   true,bob@ex.com,2,Bob
//! ```
//!
//! # Limitations
//!
//! - **Map keys**: Must be strings (no numeric or object keys)
//! - **Tabular arrays**: Require identical object structure with primitive values only
//! - **Field order**: Sorted alphabetically (may differ from original struct order)
//! - **Comments**: Not supported in the format
//!
//! # Conformance
//!
//! This implementation follows the TOON specification from:
//! <https://github.com/johannschopplich/toon>
//!
//! For additional examples and use cases, see the crate's `examples/` directory.

// This module contains only documentation; no implementation code
