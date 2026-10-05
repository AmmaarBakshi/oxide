use std::io;

use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    // No operands (or `-`) means "copy stdin through", which is what makes
    // `cat < in.txt | grep foo` and a bare `cat` in a pipeline work.
    //
    // `io::copy` forwards each chunk as soon as it's read, so a long-running
    // upstream stage shows up as it arrives, large files never sit whole in
    // memory, and bytes that aren't valid UTF-8 pass through untouched.
    let files: Vec<&str> = args.iter().map(String::as_str).collect();
    io.each_input("cat", &files, |_, input, out| io::copy(input, out).map(drop))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::test_support::{run, scratch_dir};

    #[test]
    fn copies_stdin_through() {
        assert_eq!(run("cat", &[], "a\nb").stdout, "a\nb");
        assert_eq!(run("cat", &["-"], "piped\n").stdout, "piped\n");
    }

    #[test]
    fn a_missing_file_does_not_stop_the_rest() {
        let dir = scratch_dir("cat_missing");
        let (a, missing) = (dir.join("a.txt"), dir.join("missing.txt"));
        fs::write(&a, "from a\n").unwrap();

        let out = run("cat", &[missing.to_str().unwrap(), a.to_str().unwrap()], "");
        assert_eq!(out.status, 1);
        assert_eq!(out.stdout, "from a\n");
        assert!(out.stderr.starts_with("cat: "));
        fs::remove_dir_all(dir).unwrap();
    }
}
