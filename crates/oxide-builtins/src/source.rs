use std::fs;

pub fn execute(args: &[String]) -> i32 {
    if args.is_empty() {
        eprintln!("oxide: source: missing filename");
        return 1;
    }

    let filename = &args[0];
    match fs::read_to_string(filename) {
        Ok(contents) => {
            println!("{}", contents);
            0
        }
        Err(e) => {
            eprintln!("oxide: source: failed to read '{}': {}", filename, e);
            1
        }
    }
}
