use std::fs;

use oxide_data::{csv, json};

use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    if args.is_empty() {
        ewln!(io, "oxide: open: missing filename (e.g., 'open data.json')");
        return 1;
    }

    let filename = &args[0];

    // 1. Read the raw text from the hard drive
    let contents = match fs::read_to_string(filename) {
        Ok(text) => text,
        Err(e) => {
            ewln!(io, "oxide: open: failed to read '{}': {}", filename, e);
            return 1;
        }
    };

    if filename.ends_with(".json") {
        match json::parse(&contents) {
            Ok(value) => {
                wln!(io, "{:#?}", value);
                0
            }
            Err(e) => {
                ewln!(io, "oxide: open: {}", e);
                1
            }
        }
    // --- CSV ROUTING ---
    } else if filename.ends_with(".csv") {
        match csv::parse(&contents) {
            Ok(table) => {
                wln!(io, "{:#?}", table);
                0
            }
            Err(e) => {
                ewln!(io, "oxide: open: {}", e);
                1
            }
        }
    } else {
        if io.stdout.write_all(contents.as_bytes()).is_err() {
            return 1;
        }
        0
    }
}

/// Reads a file and returns it as a structured value without printing it —
/// the entry point for the object pipeline.
pub fn get_data(filename: &str) -> Result<oxide_data::value::Value, String> {
    let contents = fs::read_to_string(filename).map_err(|e| e.to_string())?;

    if filename.ends_with(".json") {
        oxide_data::json::parse(&contents)
    } else {
        Err("Only JSON is supported for object pipelines right now".to_string())
    }
}
