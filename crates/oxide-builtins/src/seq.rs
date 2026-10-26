use crate::{Ctx, Io};

struct Options<'a> {
    separator: &'a str,
    equal_width: bool,
}

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    let (opts, numbers) = match parse_args(args) {
        Ok(parsed) => parsed,
        Err(msg) => {
            ewln!(io, "seq: {}", msg);
            ewln!(io, "Usage: seq [-w] [-s SEP] [FIRST [INCREMENT]] LAST");
            return 1;
        }
    };
    let (first, step, last) = match numbers[..] {
        [last] => (1, 1, last),
        [first, last] => (first, 1, last),
        [first, step, last] => (first, step, last),
        _ => unreachable!("parse_args checks the operand count"),
    };
    if step == 0 {
        ewln!(io, "seq: invalid zero increment");
        return 1;
    }

    // `-w` pads to the wider of the two ends, sign included.
    let width = if opts.equal_width {
        first.to_string().len().max(last.to_string().len())
    } else {
        0
    };

    let mut n = first;
    let mut sep = "";
    while (step > 0 && n <= last) || (step < 0 && n >= last) {
        if write!(io.stdout, "{}{:0width$}", sep, n).is_err() {
            return 1;
        }
        sep = opts.separator;
        match n.checked_add(step) {
            Some(next) => n = next,
            None => break,
        }
    }
    // Something was printed, so finish the last line.
    if !sep.is_empty() {
        wln!(io, "");
    }
    0
}

/// Splits out `-w` and `-s SEP`. Anything that parses as a number is an
/// operand even when it starts with `-`, so `seq -5 5` counts up from -5.
fn parse_args(args: &[String]) -> Result<(Options<'_>, Vec<i64>), String> {
    let mut opts = Options {
        separator: "\n",
        equal_width: false,
    };
    let mut numbers = Vec::new();

    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if let Ok(n) = arg.parse() {
            numbers.push(n);
        } else if arg == "-w" {
            opts.equal_width = true;
        } else if arg == "-s" {
            opts.separator = iter.next().ok_or("option requires an argument -- 's'")?;
        } else if let Some(sep) = arg.strip_prefix("-s") {
            opts.separator = sep;
        } else if arg.starts_with('-') && arg.len() > 1 {
            return Err(format!("invalid option '{}'", arg));
        } else {
            return Err(format!("invalid integer argument: '{}'", arg));
        }
    }
    match numbers.len() {
        0 => Err("missing operand".to_string()),
        1..=3 => Ok((opts, numbers)),
        _ => Err(format!("extra operand '{}'", numbers[3])),
    }
}

#[cfg(test)]
mod tests {
    use crate::test_support::run;

    #[test]
    fn counts_from_one_to_last() {
        assert_eq!(run("seq", &["3"], "").stdout, "1\n2\n3\n");
    }

    #[test]
    fn takes_a_start_and_a_step() {
        assert_eq!(run("seq", &["2", "4"], "").stdout, "2\n3\n4\n");
        assert_eq!(run("seq", &["10", "-3", "1"], "").stdout, "10\n7\n4\n1\n");
        assert_eq!(run("seq", &["-2", "0"], "").stdout, "-2\n-1\n0\n");
    }

    #[test]
    fn an_empty_range_prints_nothing() {
        let out = run("seq", &["5", "1"], "");
        assert_eq!((out.status, out.stdout.as_str()), (0, ""));
    }

    #[test]
    fn separator_and_equal_width() {
        assert_eq!(run("seq", &["-s", ",", "3"], "").stdout, "1,2,3\n");
        assert_eq!(run("seq", &["-w", "8", "10"], "").stdout, "08\n09\n10\n");
    }

    #[test]
    fn rejects_zero_steps_and_non_numbers() {
        let out = run("seq", &["1", "0", "5"], "");
        assert_eq!(out.status, 1);
        assert!(out.stderr.contains("zero increment"));
        let out = run("seq", &["ten"], "");
        assert_eq!(out.status, 1);
        assert!(out.stderr.contains("invalid integer argument: 'ten'"));
    }
}
