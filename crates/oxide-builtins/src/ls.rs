use std::env;
use std::fs;
use std::path::Path;

pub fn execute(args: &[String]) -> i32 {
    // 1. Separate flags (starting with '-') from paths
    let flags: Vec<&String> = args.iter().filter(|a| a.starts_with('-')).collect();
    let paths: Vec<&String> = args.iter().filter(|a| !a.starts_with('-')).collect();

    let show_all = flags.iter().any(|f| f.contains('a'));

    // 2. Determine target directory
    let target_dir = if paths.is_empty() {
        env::current_dir().unwrap_or_default()
    } else {
        Path::new(paths[0]).to_path_buf()
    };

    match fs::read_dir(&target_dir) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                
                // 3. Apply flag logic: skip hidden files unless -a is present
                if !show_all && name.starts_with('.') {
                    continue;
                }

                println!("{}", name);
            }
            0
        }
        Err(e) => {
            eprintln!("oxide: ls: cannot access '{}': {}", target_dir.display(), e);
            1
        }
    }
}