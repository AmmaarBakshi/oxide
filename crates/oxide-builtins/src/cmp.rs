use std::fs;
use std::io;

use crate::args::parse_flags;
use crate::{Ctx, Io};

/// Exit statuses, as POSIX fixes them: the inputs are the same, they
/// differ, or one of them couldn't be compared at all.
const SAME: i32 = 0;
const DIFFERENT: i32 = 1;
const TROUBLE: i32 = 2;

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    let (flags, files) = match parse_flags(args, "s") {
        Ok(parsed) => parsed,
        Err(msg) => {
            ewln!(io, "cmp: {}", msg);
            ewln!(io, "Usage: cmp [-s] FILE1 [FILE2]");
            return TROUBLE;
        }
    };
    let names = match files[..] {
        [first] => [first, "-"],
        [first, second] => [first, second],
        _ => {
            ewln!(io, "cmp: expected one or two files, got {}", files.len());
            ewln!(io, "Usage: cmp [-s] FILE1 [FILE2]");
            return TROUBLE;
        }
    };
    let silent = flags.contains('s');
    // Stdin can only be read once; compared with itself it is the same.
    if names == ["-", "-"] {
        return SAME;
    }

    let mut data = Vec::with_capacity(2);
    for name in names {
        match read(io, name) {
            Ok(bytes) => data.push(bytes),
            Err(e) => {
                ewln!(io, "cmp: {}: {}", name, e);
                return TROUBLE;
            }
        }
    }
    let (a, b) = (&data[0], &data[1]);

    let common = a.len().min(b.len());
    if let Some(i) = (0..common).find(|&i| a[i] != b[i]) {
        if !silent {
            let line = 1 + newlines(&a[..i]);
            wln!(
                io,
                "{} {} differ: byte {}, line {}",
                names[0],
                names[1],
                i + 1,
                line
            );
        }
        return DIFFERENT;
    }
    if a.len() == b.len() {
        return SAME;
    }

    // One input is a prefix of the other: say where the shorter one ran out.
    let (name, short) = if a.len() < b.len() {
        (names[0], a)
    } else {
        (names[1], b)
    };
    if !silent {
        if short.is_empty() {
            ewln!(io, "cmp: EOF on {} which is empty", name);
        } else if short.ends_with(b"\n") {
            let lines = newlines(short);
            ewln!(
                io,
                "cmp: EOF on {} after byte {}, line {}",
                name,
                short.len(),
                lines
            );
        } else {
            let line = 1 + newlines(short);
            ewln!(
                io,
                "cmp: EOF on {} after byte {}, in line {}",
                name,
                short.len(),
                line
            );
        }
    }
    DIFFERENT
}

/// The whole of one operand: stdin for `-`, a file otherwise.
fn read(io: &mut Io<'_>, name: &str) -> io::Result<Vec<u8>> {
    if name == "-" {
        let mut bytes = Vec::new();
        io.stdin.read_to_end(&mut bytes)?;
        Ok(bytes)
    } else {
        fs::read(name)
    }
}

fn newlines(bytes: &[u8]) -> usize {
    bytes.iter().filter(|&&b| b == b'\n').count()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::test_support::{run, scratch_dir};

    #[test]
    fn identical_inputs_are_silent_and_succeed() {
        let dir = scratch_dir("cmp_same");
        let file = dir.join("a.txt");
        fs::write(&file, "same\n").unwrap();
        let file = file.to_str().unwrap();

        let out = run("cmp", &[file, file], "");
        assert_eq!((out.status, out.stdout.as_str()), (0, ""));
        // The second file defaults to stdin.
        assert_eq!(run("cmp", &[file], "same\n").status, 0);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn reports_the_first_differing_byte_and_line() {
        let dir = scratch_dir("cmp_differ");
        let file = dir.join("a.txt");
        fs::write(&file, "abc\ndef\n").unwrap();
        let file = file.to_str().unwrap();

        let out = run("cmp", &[file, "-"], "abc\ndxf\n");
        assert_eq!(out.status, 1);
        assert_eq!(out.stdout, format!("{} - differ: byte 6, line 2\n", file));

        let out = run("cmp", &["-s", file, "-"], "abc\ndxf\n");
        assert_eq!((out.status, out.stdout.as_str()), (1, ""));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_prefix_reports_where_the_shorter_input_ends() {
        let dir = scratch_dir("cmp_eof");
        let file = dir.join("a.txt");
        fs::write(&file, "abc\nd").unwrap();
        let file = file.to_str().unwrap();

        let out = run("cmp", &[file, "-"], "abc\n");
        assert_eq!(out.status, 1);
        assert_eq!(out.stderr, "cmp: EOF on - after byte 4, line 1\n");

        let out = run("cmp", &["-", file], "ab");
        assert_eq!(out.stderr, "cmp: EOF on - after byte 2, in line 1\n");

        let out = run("cmp", &["-", file], "");
        assert_eq!(out.stderr, "cmp: EOF on - which is empty\n");

        let out = run("cmp", &["-s", "-", file], "");
        assert_eq!((out.status, out.stderr.as_str()), (1, ""));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn trouble_exits_two() {
        let out = run("cmp", &["no-such-file.txt", "-"], "");
        assert_eq!(out.status, 2);
        assert!(out.stderr.starts_with("cmp: no-such-file.txt:"));

        let out = run("cmp", &[], "");
        assert_eq!(out.status, 2);
        assert!(out.stderr.contains("expected one or two files"));
    }
}
