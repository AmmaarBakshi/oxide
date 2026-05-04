use std::env;

pub fn execute(args: &[String]) -> i32 {
    if args.is_empty() {
        eprintln!("oxide: unset: missing variable name");
        return 1;
    }

    for var in args {
        env::remove_var(var);
    }
    0
}
