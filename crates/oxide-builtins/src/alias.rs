use std::collections::HashMap;

// Notice we pass the aliases map in as a mutable reference!
pub fn execute(args: &[String], aliases: &mut HashMap<String, String>) -> i32 {
    if args.is_empty() {
        for (key, val) in aliases {
            println!("alias {}='{}'", key, val);
        }
        return 0;
    }

    for arg in args {
        if let Some((key, value)) = arg.split_once('=') {
            let clean_value = value.trim_matches(|c| c == '"' || c == '\'');
            aliases.insert(key.to_string(), clean_value.to_string());
        } else {
            eprintln!("oxide: alias: invalid format. Use name=value");
            return 1;
        }
    }
    0
}