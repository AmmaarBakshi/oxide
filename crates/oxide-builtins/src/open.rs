use std::fs;
use oxide_data::json;

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

    // 2. Feed the text into your new Data Engine!
    // We check if it ends in .json to know which parser to use
    if filename.ends_with(".json") {
        match json::parse(&contents) {
            Ok(value) => {
                // {:#?} tells Rust to "pretty print" the Object tree!
                println!("{:#?}", value);
                0
            }
            Err(e) => {
                eprintln!("oxide: open: {}", e);
                1
            }
        }
    } else {
        // If it's not a JSON file, just print the raw text for now
        println!("{}", contents);
        0
    }
}