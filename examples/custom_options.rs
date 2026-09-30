//! Customizing TOON output with ToonOptions.
//!
//! Every array header declares the delimiter in use (`[N]` comma, `[N<TAB>]`
//! tab, `[N|]` pipe), so decoders need no configuration to read it back.
//!
//! Run with: cargo run --example custom_options

use serde::{Deserialize, Serialize};
use serde_toon::{to_string_with_options, Delimiter, ToonOptions};
use std::error::Error;

#[derive(Debug, Serialize, Deserialize)]
struct DataRow {
    id: u32,
    value: String,
    active: bool,
}

fn main() -> Result<(), Box<dyn Error>> {
    let data = vec![
        DataRow {
            id: 1,
            value: "test".into(),
            active: true,
        },
        DataRow {
            id: 2,
            value: "prod".into(),
            active: false,
        },
    ];

    // Default (comma delimiter)
    println!("Default (comma delimiter):");
    let default = serde_toon::to_string(&data)?;
    println!("{}\n", default);

    // Tab delimiter (TSV-like rows; the header is `[2<TAB>]`)
    println!("Tab delimiter:");
    let tab_options = ToonOptions::new().with_delimiter(Delimiter::Tab);
    let tab_format = to_string_with_options(&data, tab_options)?;
    println!("{}\n", tab_format);

    // Pipe delimiter (the header is `[2|]`)
    println!("Pipe delimiter:");
    let pipe_options = ToonOptions::new().with_delimiter(Delimiter::Pipe);
    let pipe_format = to_string_with_options(&data, pipe_options)?;
    println!("{}\n", pipe_format);

    // Decoding reads the delimiter from the header.
    let back: Vec<DataRow> = serde_toon::from_str(&pipe_format)?;
    assert_eq!(back.len(), data.len());

    // Custom indentation (rows are indented one level under the header)
    println!("Four-space indentation:");
    let indent_options = ToonOptions::new().with_indent(4);
    let indented = to_string_with_options(&data, indent_options)?;
    println!("{}\n", indented);

    // Primitive arrays show delimiters clearly
    println!("Primitive arrays with different delimiters:");
    let numbers = vec![1, 2, 3, 4, 5];

    println!("  Comma: {}", serde_toon::to_string(&numbers)?);

    let tab_opts = ToonOptions::new().with_delimiter(Delimiter::Tab);
    println!("  Tab:   {}", to_string_with_options(&numbers, tab_opts)?);

    let pipe_opts = ToonOptions::new().with_delimiter(Delimiter::Pipe);
    println!("  Pipe:  {}", to_string_with_options(&numbers, pipe_opts)?);

    Ok(())
}
