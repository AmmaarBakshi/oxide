use std::fs;
use oxide_data::{json, csv};

pub fn execute(args: &[String]) -> i32 {
    if args.is_empty() {
        eprintln!("oxide: open: missing filename (e.g., 'open data.json')");
        return 1;
    }

    let filename = &args[0];

    // 1. Read the raw text from the hard drive
    let contents = match fs::read_to_string(filename) {
        Ok(text) => text,
        Err(e) => {
            eprintln!("oxide: open: failed to read '{}': {}", filename, e);
            return 1;
        }
    };

    if filename.ends_with(".json") {
        match json::parse(&contents) {
            Ok(value) => {
                println!("{:#?}", value);
                0
            }
            Err(e) => {
                eprintln!("oxide: open: {}", e);
                1
            }
        }
    // --- CSV ROUTING ---
    } else if filename.ends_with(".csv") {
        match csv::parse(&contents) {
            Ok(table) => {
                println!("{:#?}", table);
                0
            }
            Err(e) => {
                eprintln!("oxide: open: {}", e);
                1
            }
        }
    } else {
        println!("{}", contents);
        0
    }
}

// ========================================================
// --- NEW: A helper for the Object Pipeline ---
// This reads the file but DOES NOT print it. It just returns the Object in memory!
// ========================================================
pub fn get_data(filename: &str) -> Result<oxide_data::value::Value, String> {
    let contents = fs::read_to_string(filename).map_err(|e| e.to_string())?;
    
    if filename.ends_with(".json") {
        oxide_data::json::parse(&contents)
    } else {
        Err("Only JSON is supported for object pipelines right now".to_string())
    }
}