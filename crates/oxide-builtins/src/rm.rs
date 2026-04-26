use std::fs;
use std::path::Path;

pub fn execute(args: &[String]) -> i32 {
    if args.is_empty() {
        eprintln!("oxide: rm: missing operand (e.g., 'rm file.txt')");
        return 1;
    }

    let mut status = 0;
    for arg in args {
        let path = Path::new(arg);
        if path.is_dir() {
            // Delete folder and everything inside it
            if let Err(e) = fs::remove_dir_all(path) {
                eprintln!("oxide: rm: cannot remove '{}': {}", arg, e);
                status = 1;
            }
        } else {
            // Delete single file
            if let Err(e) = fs::remove_file(path) {
                eprintln!("oxide: rm: cannot remove '{}': {}", arg, e);
                status = 1;
            }
        }
    }
    status
}