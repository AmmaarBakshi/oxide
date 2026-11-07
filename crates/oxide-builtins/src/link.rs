use std::fs;

use crate::args::parse_flags;
use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    let (_, operands) = match parse_flags(args, "") {
        Ok(parsed) => parsed,
        Err(msg) => {
            ewln!(io, "link: {}", msg);
            ewln!(io, "Usage: link FILE1 FILE2");
            return 1;
        }
    };
    // Exactly two files and no options: link is the bare system call, the
    // counterpart to unlink, for when ln's directory forms aren't wanted.
    let (target, name) = match operands[..] {
        [target, name] => (target, name),
        [] => {
            ewln!(io, "link: missing operand");
            return 1;
        }
        [target] => {
            ewln!(io, "link: missing operand after '{}'", target);
            return 1;
        }
        [_, _, extra, ..] => {
            ewln!(io, "link: extra operand '{}'", extra);
            return 1;
        }
    };
    match fs::hard_link(target, name) {
        Ok(()) => 0,
        Err(e) => {
            ewln!(
                io,
                "link: cannot create link '{}' to '{}': {}",
                name,
                target,
                e
            );
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::test_support::{run, scratch_dir};

    #[test]
    fn makes_a_hard_link() {
        let dir = scratch_dir("link_file");
        let (target, name) = (dir.join("a.txt"), dir.join("b.txt"));
        fs::write(&target, "hello").unwrap();

        let out = run(
            "link",
            &[target.to_str().unwrap(), name.to_str().unwrap()],
            "",
        );
        assert_eq!(out.status, 0, "{}", out.stderr);
        assert_eq!(fs::read_to_string(&name).unwrap(), "hello");

        // A hard link is the same file, so a write through one name shows
        // through the other.
        fs::write(&name, "changed").unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), "changed");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn will_not_replace_an_existing_file() {
        let dir = scratch_dir("link_exists");
        let (target, name) = (dir.join("a.txt"), dir.join("b.txt"));
        fs::write(&target, "a").unwrap();
        fs::write(&name, "b").unwrap();

        let out = run(
            "link",
            &[target.to_str().unwrap(), name.to_str().unwrap()],
            "",
        );
        assert_eq!(out.status, 1);
        assert!(
            out.stderr.starts_with("link: cannot create link"),
            "{}",
            out.stderr
        );
        assert_eq!(fs::read_to_string(&name).unwrap(), "b");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn takes_exactly_two_files() {
        let out = run("link", &[], "");
        assert_eq!(out.status, 1);
        assert!(out.stderr.contains("missing operand"));

        let out = run("link", &["a"], "");
        assert_eq!(out.status, 1);
        assert!(out.stderr.contains("missing operand after 'a'"));

        let out = run("link", &["a", "b", "c"], "");
        assert_eq!(out.status, 1);
        assert!(out.stderr.contains("extra operand 'c'"));

        assert_eq!(run("link", &["-s", "a", "b"], "").status, 1);
    }
}
