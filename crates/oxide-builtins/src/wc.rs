use std::io::{self, BufRead, Write};

use crate::args::parse_flags;
use crate::{Ctx, Io};

/// Line, word, character and byte counts, in the order coreutils prints them.
type Counts = [u64; 4];

/// The flag that selects each entry of [`Counts`].
const COLUMN_FLAGS: [char; 4] = ['l', 'w', 'm', 'c'];

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    let (flags, files) = match parse_flags(args, "lwmc") {
        Ok(parsed) => parsed,
        Err(msg) => {
            ewln!(io, "wc: {}", msg);
            ewln!(io, "Usage: wc [-lwmc] [file...]");
            return 1;
        }
    };

    // No flags means the classic trio: lines, words, bytes.
    let selected = if flags.is_empty() {
        [true, true, false, true]
    } else {
        COLUMN_FLAGS.map(|flag| flags.contains(flag))
    };

    // A lone number for a lone input is printed bare, so `wc -l < f` yields
    // just the count; anything more is padded into aligned columns.
    let single = selected.iter().filter(|&&on| on).count() == 1 && files.len() <= 1;
    let width = if single { 1 } else { 7 };

    let mut total: Counts = [0; 4];
    let status = io.each_input("wc", &files, |name, input, out| {
        let counts = count(input)?;
        for (sum, n) in total.iter_mut().zip(counts) {
            *sum += n;
        }
        // Reading stdin implicitly leaves the name off; an explicit `-` keeps it.
        let label = if files.is_empty() { None } else { Some(name) };
        write_row(out, &counts, &selected, width, label)
    });

    if files.len() > 1 && write_row(io.stdout, &total, &selected, width, Some("total")).is_err() {
        return 1;
    }
    status
}

/// Counts everything in one streaming pass. A character is a UTF-8 scalar
/// value, so every byte that isn't a continuation byte starts a new one.
fn count(input: &mut dyn BufRead) -> io::Result<Counts> {
    let (mut lines, mut words, mut chars, mut bytes) = (0, 0, 0, 0);
    let mut in_word = false;

    loop {
        let buf = input.fill_buf()?;
        if buf.is_empty() {
            break;
        }
        for &b in buf {
            lines += u64::from(b == b'\n');
            chars += u64::from(b & 0xC0 != 0x80);
            let space = b.is_ascii_whitespace();
            words += u64::from(!space && !in_word);
            in_word = !space;
        }
        let len = buf.len();
        bytes += len as u64;
        input.consume(len);
    }
    Ok([lines, words, chars, bytes])
}

fn write_row(
    out: &mut dyn Write,
    counts: &Counts,
    selected: &[bool; 4],
    width: usize,
    label: Option<&str>,
) -> io::Result<()> {
    let row: Vec<String> = counts
        .iter()
        .zip(selected)
        .filter(|(_, &on)| on)
        .map(|(n, _)| format!("{:>width$}", n))
        .collect();
    match label {
        Some(name) => writeln!(out, "{} {}", row.join(" "), name),
        None => writeln!(out, "{}", row.join(" ")),
    }
}

#[cfg(test)]
mod tests {
    use crate::test_support::run;

    #[test]
    fn counts_lines_words_and_bytes_by_default() {
        let out = run("wc", &[], "hello world\nfoo\n");
        assert_eq!(out.stdout, "      2       3      16\n");
    }

    #[test]
    fn single_column_for_single_input_is_bare() {
        assert_eq!(run("wc", &["-l"], "a\nb\nc\n").stdout, "3\n");
        assert_eq!(run("wc", &["-w"], "  spaced   out\twords \n").stdout, "3\n");
    }

    #[test]
    fn chars_differ_from_bytes_for_utf8() {
        assert_eq!(run("wc", &["-mc"], "héllo\n").stdout, "      6       7\n");
    }

    #[test]
    fn multiple_inputs_get_names_and_a_total() {
        // The second `-` finds stdin already drained.
        let out = run("wc", &["-l", "-", "-"], "a\nb\n");
        assert_eq!(out.stdout, "      2 -\n      0 -\n      2 total\n");
    }

    #[test]
    fn rejects_unknown_flags() {
        let out = run("wc", &["-z"], "");
        assert_eq!(out.status, 1);
        assert!(out.stderr.starts_with("wc: invalid option -- 'z'"));
    }
}
