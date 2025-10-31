//! Customizing TOON output with ToonOptions.
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

    // Tab delimiter (useful for TSV export)
    println!("Tab delimiter:");
    let tab_options = ToonOptions::new().with_delimiter(Delimiter::Tab);
    let tab_format = to_string_with_options(&data, tab_options)?;
    println!("{}\n", tab_format);

    // Pipe delimiter (useful for markdown tables)
    println!("Pipe delimiter:");
    let pipe_options = ToonOptions::new().with_delimiter(Delimiter::Pipe);
    let pipe_format = to_string_with_options(&data, pipe_options)?;
    println!("{}\n", pipe_format);

    // Custom length marker
    println!("Custom length marker (#):");
    let marked_options = ToonOptions::new().with_length_marker('#');
    let marked = to_string_with_options(&data, marked_options)?;
    println!("{}\n", marked);

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
