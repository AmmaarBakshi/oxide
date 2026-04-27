use std::fs;
use std::path::Path;

pub fn execute(args: &[String]) -> i32 {
    // We need at least 2 arguments: the starting path and the search term
    if args.len() < 2 {
        eprintln!("oxide: find: missing arguments.");
        eprintln!("Usage: find <path> <search_term> (e.g., 'find . .json')");
        return 1;
    }

    let start_path = Path::new(&args[0]);
    let target = &args[1];

    if !start_path.exists() {
        eprintln!("oxide: find: path '{}' does not exist", start_path.display());
        return 1;
    }

    // Kick off the recursive search!
    search_directory(start_path, target);
    
    0
}

/// Recursively digs through folders looking for files that match the target
fn search_directory(dir: &Path, target: &str) {
    // Try to read the directory. If we get an error (like Access Denied), just quietly skip it.
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            
            // If it's a directory, dive into it! (This is the recursion)
            if path.is_dir() {
                search_directory(&path, target);
            } 
            // If it's a file, check if its name contains our search target
            else if let Some(file_name) = path.file_name() {
                if file_name.to_string_lossy().contains(target) {
                    // We found a match! Print the full path.
                    println!("{}", path.display());
                }
            }
        }
    }
}