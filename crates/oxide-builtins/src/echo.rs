use std::fs::File;
use std::io::Write;

pub fn execute(args: &[String], outfile: &Option<String>) -> i32 {
    let text = args.join(" ");

    // If the user typed `> file.txt`, write to the file!
    if let Some(file_name) = outfile {
        match File::create(file_name) {
            Ok(mut file) => {
                let _ = writeln!(file, "{}", text);
                0
            }
            Err(e) => {
                eprintln!("oxide: echo: failed to write to '{}': {}", file_name, e);
                1
            }
        }
    } else {
        // Otherwise, print to the console normally
        println!("{}", text);
        0
    }
}