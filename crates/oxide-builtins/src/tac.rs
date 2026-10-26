use std::io::{self, BufRead, Write};

use crate::args::parse_flags;
use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    let (_, files) = match parse_flags(args, "") {
        Ok(parsed) => parsed,
        Err(msg) => {
            ewln!(io, "tac: {}", msg);
            ewln!(io, "Usage: tac [file...]");
            return 1;
        }
    };
    // Each input is reversed on its own, not the concatenation of them all.
    io.each_input("tac", &files, |_, input, out| reverse(input, out))
}

/// Writes the lines of `input` last first. The whole input has to be held,
/// since its last line is the first one out.
///
/// Lines are copied with their own newline, so a final line without one is
/// glued to the line after it, exactly as coreutils does.
fn reverse(input: &mut dyn BufRead, out: &mut dyn Write) -> io::Result<()> {
    let mut data = Vec::new();
    input.read_to_end(&mut data)?;
    for line in data.split_inclusive(|&b| b == b'\n').rev() {
        out.write_all(line)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::test_support::run;

    #[test]
    fn prints_lines_last_first() {
        assert_eq!(run("tac", &[], "1\n2\n3\n").stdout, "3\n2\n1\n");
    }

    #[test]
    fn an_unterminated_last_line_is_not_given_a_newline() {
        assert_eq!(run("tac", &[], "a\nb").stdout, "ba\n");
    }

    #[test]
    fn empty_input_prints_nothing() {
        let out = run("tac", &[], "");
        assert_eq!((out.status, out.stdout.as_str()), (0, ""));
    }
}
