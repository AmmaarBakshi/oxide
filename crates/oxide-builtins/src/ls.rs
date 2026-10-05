use std::env;
use std::fs;
use std::path::Path;

use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
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
            let mut names: Vec<String> = entries
                .flatten()
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                // 3. Apply flag logic: skip hidden files unless -a is present
                .filter(|name| show_all || !name.starts_with('.'))
                .collect();

            // 4. read_dir order is up to the filesystem, so sort for stable
            // output. Case-insensitive, so `Cargo.toml` sits beside `crates`.
            names.sort_by_cached_key(|name| name.to_lowercase());

            for name in names {
                wln!(io, "{}", name);
            }
            0
        }
        Err(e) => {
            ewln!(io, "oxide: ls: cannot access '{}': {}", target_dir.display(), e);
            1
        }
    }
}
