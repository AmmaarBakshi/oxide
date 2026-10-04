use std::fs;
use std::io::Write;
use std::path::Path;

use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    // We need at least 2 arguments: the starting path and the search term
    if args.len() < 2 {
        ewln!(io, "oxide: find: missing arguments.");
        ewln!(io, "Usage: find <path> <search_term> (e.g., 'find . .json')");
        return 1;
    }

    let start_path = Path::new(&args[0]);
    let target = &args[1];

    if !start_path.exists() {
        ewln!(io, "oxide: find: path '{}' does not exist", start_path.display());
        return 1;
    }

    match search_directory(start_path, target, io.stdout) {
        Ok(()) => 0,
        // The sink went away mid-walk (closed pipe); stop quietly.
        Err(_) => 1,
    }
}

/// Recursively digs through folders looking for files that match the target.
///
/// Directories we can't read (access denied, races) are skipped silently, but
/// a failed *write* aborts the whole walk — there is no point searching on
/// once the consumer has hung up.
fn search_directory(dir: &Path, target: &str, out: &mut dyn Write) -> std::io::Result<()> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Ok(());
    };

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_dir() {
            search_directory(&path, target, out)?;
        } else if let Some(file_name) = path.file_name() {
            if file_name.to_string_lossy().contains(target) {
                writeln!(out, "{}", path.display())?;
            }
        }
    }
    Ok(())
}
