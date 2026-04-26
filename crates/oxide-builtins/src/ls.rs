use std::env;
use std::fs;
use std::path::Path;

pub fn execute(args: &[String]) -> i32 {
    // If they typed `ls folder_name`, read that folder. 
    // If they just typed `ls`, read the current directory.
    let target_dir = if args.is_empty() {
        env::current_dir().unwrap_or_default()
    } else {
        Path::new(&args[0]).to_path_buf()
    };

    match fs::read_dir(&target_dir) {
        Ok(entries) => {
            for entry in entries.flatten() {
                // Print the name of each file/folder
                println!("{}", entry.file_name().to_string_lossy());
            }
            0 // Success
        }
        Err(e) => {
            eprintln!("oxide: ls: cannot access '{}': {}", target_dir.display(), e);
            1 // Error
        }
    }
}