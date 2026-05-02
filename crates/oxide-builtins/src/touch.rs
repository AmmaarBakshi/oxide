use std::fs::OpenOptions;

pub fn execute(args: &[String]) -> i32 {
    if args.is_empty() {
        eprintln!("touch: missing file operand");
        return 1;
    }
    for path in args {
        if let Err(e) = OpenOptions::new().create(true).write(true).open(path) {
            eprintln!("touch: cannot touch '{}': {}", path, e);
            return 1;
        }
    }
    0
}