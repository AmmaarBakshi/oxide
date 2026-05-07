use std::env;
use std::path::PathBuf;

pub fn execute(args: &[String]) -> i32 {
    // 1. Determine the target directory
    let target = if args.is_empty() {
        // Fallback to ~ (Home) if no args are provided
        dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
    } else if args[0] == "-" {
        // Fallback to previous directory if `cd -` is used
        PathBuf::from(env::var("OLDPWD").unwrap_or_else(|_| ".".to_string()))
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
                env::set_var("PWD", new_pwd);
            }
            0 // Success
        }
        Err(e) => {
            eprintln!("oxide: cd: {}: {}", target.display(), e);
            1 // Error
        }
    }
}