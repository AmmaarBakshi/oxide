use std::env;
use std::path::PathBuf;

use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    let back = args.first().map(String::as_str) == Some("-");

    // 1. Determine the target directory
    let target = if args.is_empty() {
        // Fallback to ~ (Home) if no args are provided
        dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
    } else if back {
        // `cd -` returns to the previous directory, if there is one
        match env::var("OLDPWD") {
            Ok(old) => PathBuf::from(old),
            Err(_) => {
                ewln!(io, "oxide: cd: OLDPWD not set");
                return 1;
            }
        }
    } else {
        PathBuf::from(&args[0])
    };

    let current_pwd = env::current_dir().unwrap_or_default();

    // 2. Attempt to change the directory
    match env::set_current_dir(&target) {
        Ok(_) => {
            // 3. Update the environment variables upon success
            env::set_var("OLDPWD", current_pwd);
            if let Ok(new_pwd) = env::current_dir() {
                env::set_var("PWD", &new_pwd);
                // Like bash, `cd -` says where it landed, since it isn't
                // visible in the command itself.
                if back {
                    wln!(io, "{}", new_pwd.display());
                }
            }
            0
        }
        Err(e) => {
            ewln!(io, "oxide: cd: {}: {}", target.display(), e);
            1
        }
    }
}
