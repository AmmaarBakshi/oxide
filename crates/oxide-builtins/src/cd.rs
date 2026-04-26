use std::env;
use std::path::Path;

pub fn execute(args: &[String]) -> i32 {
    // If they just type `cd` with no arguments, we'll throw a friendly error for now.
    // (Later we can wire this up to read the $HOME environment variable!)
    if args.is_empty() {
        eprintln!("oxide: cd: missing argument");
        return 1;
    }

    let target = &args[0];
    let path = Path::new(target);

    // Attempt to change the directory of the running Oxide process
    if let Err(e) = env::set_current_dir(&path) {
        eprintln!("oxide: cd: {}: {}", target, e);
        return 1; // Return error code 1
    }

    0 // Success!
}