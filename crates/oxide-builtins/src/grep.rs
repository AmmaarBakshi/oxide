pub fn execute(args: &[String]) -> i32 {
    if args.len() < 2 {
        eprintln!("grep: usage: grep <pattern> <file...>");
        return 1;
    }

    let pattern = &args[0];
    let files = &args[1..];

    for path in files {
        match std::fs::read_to_string(path) {
            Ok(content) => {
                for (i, line) in content.lines().enumerate() {
                    if line.contains(pattern) {
                        // Print filename and line number if multiple files, like standard grep
                        if files.len() > 1 {
                            println!("{}:{}: {}", path, i + 1, line);
                        } else {
                            println!("{}", line);
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("grep: {}: {}", path, e);
                return 1;
            }
        }
    }
    0
}