use std::fs;

pub fn execute(args: &[String]) -> i32 {
    if args.is_empty() {
        return 0; // Standard cat waits for stdin, but we'll keep it simple for now
    }
    for path in args {
        match fs::read_to_string(path) {
            Ok(content) => print!("{}", content),
            Err(e) => {
                eprintln!("cat: {}: {}", path, e);
                return 1;
            }
        }
    }
    0
}