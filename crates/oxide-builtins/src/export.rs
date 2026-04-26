use std::env;

pub fn execute(args: &[String]) -> i32 {
    if args.is_empty() {
        eprintln!("oxide: export: missing argument");
        return 1;
    }

    for arg in args {
        // Split the argument by the '=' sign (e.g., "NAME=Atlas")
        if let Some((key, value)) = arg.split_once('=') {
            env::set_var(key, value);
        } else {
            eprintln!("oxide: export: invalid format '{}'. Use KEY=VALUE", arg);
            return 1;
        }
    }
    
    0 // Success
}