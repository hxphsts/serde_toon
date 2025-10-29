//! Customizing TOON output with ToonOptions.
//!
//! Run with: cargo run --example custom_options

use serde::{Deserialize, Serialize};
use serde_toon::{to_string_with_options, Delimiter, ToonOptions};
use std::error::Error;

#[derive(Debug, Serialize, Deserialize)]
struct Config {
    name: String,
    version: String,
    debug: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct DataRow {
    id: u32,
    value: String,
    active: bool,
}

fn main() -> Result<(), Box<dyn Error>> {
    let config = Config {
        name: "MyApp".to_string(),
        version: "1.0.0".to_string(),
        debug: true,
    };

    // Default format (comma delimiter)
    println!("Default (comma):");
    let default = serde_toon::to_string(&config)?;
    println!("{}\n", default);

    // Tab delimiter (useful for spreadsheets)
    println!("Tab delimiter:");
    let tab_options = ToonOptions::new().with_delimiter(Delimiter::Tab);
    let tab_format = to_string_with_options(&config, tab_options)?;
    println!("{}\n", tab_format);

    // Pipe delimiter (useful for shell processing)
    println!("Pipe delimiter:");
    let pipe_options = ToonOptions::new().with_delimiter(Delimiter::Pipe);
    let pipe_format = to_string_with_options(&config, pipe_options)?;
    println!("{}\n", pipe_format);

    // Custom length marker
    println!("Custom length marker (#):");
    let marked_options = ToonOptions::new().with_length_marker('#');
    let data = vec![
        DataRow {
            id: 1,
            value: "test".to_string(),
            active: true,
        },
        DataRow {
            id: 2,
            value: "prod".to_string(),
            active: false,
        },
    ];
    let marked = to_string_with_options(&data, marked_options)?;
    println!("{}", marked);

    Ok(())
}
