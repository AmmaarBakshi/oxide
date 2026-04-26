use std::env;

pub fn execute(_args: &[String]) -> i32 {
    match env::current_dir() {
        Ok(dir) => {
            println!("{}", dir.display());
            0 // Success
        }
        Err(e) => {
            eprintln!("oxide: pwd: {}", e);
            1 // Error
        }
    }
}