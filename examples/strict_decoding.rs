//! Validating TOON input with strict decoding.
//!
//! `from_str` is lenient: it accepts documents whose declared array lengths
//! are wrong or that repeat a key. `DecodeOptions::strict()` enforces the
//! checks of TOON spec §14 and reports the line of each violation, which is
//! what you want for untrusted or LLM-generated input.
//!
//! Run with: cargo run --example strict_decoding

use serde::Deserialize;
use serde_toon::{from_str, from_str_with_options, DecodeOptions};
use std::error::Error;

#[derive(Debug, Deserialize, PartialEq)]
struct Task {
    id: u32,
    title: String,
    done: bool,
}

#[derive(Debug, Deserialize, PartialEq)]
struct Plan {
    owner: String,
    tasks: Vec<Task>,
}

fn main() -> Result<(), Box<dyn Error>> {
    // A well-formed document passes strict validation.
    let valid = "\
owner: Ada
tasks[2]{id,title,done}:
  1,Write docs,true
  2,Tag release,false";
    let plan: Plan = from_str_with_options(valid, DecodeOptions::strict())?;
    println!(
        "Valid document: {} tasks for {}\n",
        plan.tasks.len(),
        plan.owner
    );

    // The header promises three rows, but only two follow.
    let short = "\
owner: Ada
tasks[3]{id,title,done}:
  1,Write docs,true
  2,Tag release,false";
    let lenient: Plan = from_str(short)?;
    println!(
        "Length mismatch, default mode: accepted {} tasks",
        lenient.tasks.len()
    );
    match from_str_with_options::<Plan>(short, DecodeOptions::strict()) {
        Ok(_) => unreachable!("strict mode checks the declared row count"),
        Err(err) => println!("Length mismatch, strict mode: {err}\n"),
    }

    // `owner` appears twice.
    let duplicate = "\
owner: Ada
tasks[0]{id,title,done}:
owner: Bob";
    match from_str_with_options::<Plan>(duplicate, DecodeOptions::strict()) {
        Ok(_) => unreachable!("strict mode rejects duplicate keys"),
        Err(err) => println!("Duplicate key, strict mode: {err}"),
    }

    Ok(())
}
