use std::fs;

use crate::args::parse_flags;
use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    let (_, files) = match parse_flags(args, "") {
        Ok(parsed) => parsed,
        Err(msg) => {
            ewln!(io, "unlink: {}", msg);
            ewln!(io, "Usage: unlink FILE");
            return 1;
        }
    };
    // Exactly one file and no options: unlink is the bare system call, for
    // when rm's flags and prompts are more than is wanted.
    let file = match files[..] {
        [file] => file,
        [] => {
            ewln!(io, "unlink: missing operand");
            return 1;
        }
        [_, extra, ..] => {
            ewln!(io, "unlink: extra operand '{}'", extra);
            return 1;
        }
    };
    match fs::remove_file(file) {
        Ok(()) => 0,
        Err(e) => {
            ewln!(io, "unlink: cannot unlink '{}': {}", file, e);
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::test_support::{run, scratch_dir};

    #[test]
    fn removes_a_file() {
        let dir = scratch_dir("unlink_file");
        let file = dir.join("a.txt");
        fs::write(&file, "").unwrap();

        let out = run("unlink", &[file.to_str().unwrap()], "");
        assert_eq!(out.status, 0, "{}", out.stderr);
        assert!(!file.exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn will_not_remove_a_directory() {
        let dir = scratch_dir("unlink_dir");
        let out = run("unlink", &[dir.to_str().unwrap()], "");
        assert_eq!(out.status, 1);
        assert!(
            out.stderr.starts_with("unlink: cannot unlink"),
            "{}",
            out.stderr
        );
        assert!(dir.exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn takes_exactly_one_file() {
        let out = run("unlink", &[], "");
        assert_eq!(out.status, 1);
        assert!(out.stderr.contains("missing operand"));

        let out = run("unlink", &["a", "b"], "");
        assert_eq!(out.status, 1);
        assert!(out.stderr.contains("extra operand 'b'"));
    }
}
