use std::fs::{File, OpenOptions};
use std::io::Write;

use crate::args::parse_flags;
use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    let (flags, names) = match parse_flags(args, "a") {
        Ok(parsed) => parsed,
        Err(msg) => {
            ewln!(io, "tee: {}", msg);
            ewln!(io, "Usage: tee [-a] [file...]");
            return 1;
        }
    };
    let append = flags.contains('a');

    let mut status = 0;
    let mut files: Vec<(&str, File)> = Vec::new();
    for &name in &names {
        let opened = OpenOptions::new()
            .write(true)
            .create(true)
            .append(append)
            .truncate(!append)
            .open(name);
        match opened {
            Ok(file) => files.push((name, file)),
            Err(e) => {
                ewln!(io, "tee: {}: {}", name, e);
                status = 1;
            }
        }
    }

    // Copy chunk by chunk so each piece reaches stdout as soon as it
    // arrives. A file that fails mid-way is dropped and the rest carry on.
    loop {
        let chunk = match io.stdin.fill_buf() {
            Ok([]) => break,
            Ok(chunk) => chunk,
            Err(e) => {
                let _ = writeln!(io.stderr, "tee: standard input: {}", e);
                return 1;
            }
        };
        if io.stdout.write_all(chunk).is_err() {
            return 1;
        }
        files.retain_mut(|(name, file)| match file.write_all(chunk) {
            Ok(()) => true,
            Err(e) => {
                let _ = writeln!(io.stderr, "tee: {}: {}", name, e);
                status = 1;
                false
            }
        });
        let len = chunk.len();
        io.stdin.consume(len);
    }
    status
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::test_support::{run, scratch_dir};

    #[test]
    fn copies_stdin_to_stdout_and_every_file() {
        let dir = scratch_dir("tee_copies");
        let (a, b) = (dir.join("a.txt"), dir.join("b.txt"));
        fs::write(&a, "old\n").unwrap();

        let out = run(
            "tee",
            &[a.to_str().unwrap(), b.to_str().unwrap()],
            "one\ntwo\n",
        );
        assert_eq!(out.status, 0, "{}", out.stderr);
        assert_eq!(out.stdout, "one\ntwo\n");
        assert_eq!(fs::read_to_string(&a).unwrap(), "one\ntwo\n");
        assert_eq!(fs::read_to_string(&b).unwrap(), "one\ntwo\n");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn dash_a_appends() {
        let dir = scratch_dir("tee_append");
        let log = dir.join("log.txt");
        fs::write(&log, "first\n").unwrap();

        run("tee", &["-a", log.to_str().unwrap()], "second\n");
        assert_eq!(fs::read_to_string(&log).unwrap(), "first\nsecond\n");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn an_unopenable_file_still_lets_output_through() {
        let dir = scratch_dir("tee_bad_file");
        let bad = dir.join("missing").join("x.txt");

        let out = run("tee", &[bad.to_str().unwrap()], "data\n");
        assert_eq!(out.status, 1);
        assert_eq!(out.stdout, "data\n");
        assert!(out.stderr.starts_with("tee: "), "{}", out.stderr);
        fs::remove_dir_all(dir).unwrap();
    }
}
