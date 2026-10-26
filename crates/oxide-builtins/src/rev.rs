use std::io::{self, BufRead, Write};

use crate::args::parse_flags;
use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    let (_, files) = match parse_flags(args, "") {
        Ok(parsed) => parsed,
        Err(msg) => {
            ewln!(io, "rev: {}", msg);
            ewln!(io, "Usage: rev [file...]");
            return 1;
        }
    };
    io.each_input("rev", &files, |_, input, out| reverse_lines(input, out))
}

/// Writes each line of `input` with its characters in reverse order,
/// keeping the line ending (if any) at the end.
fn reverse_lines(input: &mut dyn BufRead, out: &mut dyn Write) -> io::Result<()> {
    let mut line = Vec::new();
    loop {
        line.clear();
        if input.read_until(b'\n', &mut line)? == 0 {
            return Ok(());
        }
        let ending = if line.ends_with(b"\r\n") {
            2
        } else if line.ends_with(b"\n") {
            1
        } else {
            0
        };
        let (text, ending) = line.split_at(line.len() - ending);
        let reversed: String = String::from_utf8_lossy(text).chars().rev().collect();
        out.write_all(reversed.as_bytes())?;
        out.write_all(ending)?;
    }
}

#[cfg(test)]
mod tests {
    use crate::test_support::run;

    #[test]
    fn reverses_each_line() {
        let out = run("rev", &[], "abc\nhello world\n");
        assert_eq!(out.stdout, "cba\ndlrow olleh\n");
    }

    #[test]
    fn keeps_line_endings_and_multibyte_characters() {
        let out = run("rev", &[], "héllo\r\nno newline");
        assert_eq!(out.stdout, "olléh\r\nenilwen on");
    }
}
