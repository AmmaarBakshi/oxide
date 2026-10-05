use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    // `-n` leaves off the trailing newline, for printing a prompt or building
    // a line up in pieces.
    let newline = args.first().map(String::as_str) != Some("-n");
    let words = if newline { args } else { &args[1..] };

    // Redirection (`> file`, `>> file`) is applied by the caller when it
    // builds `io`, so echo only ever has to write one line.
    let written = if newline {
        writeln!(io.stdout, "{}", words.join(" "))
    } else {
        write!(io.stdout, "{}", words.join(" "))
    };
    if written.is_err() {
        return 1;
    }
    0
}

#[cfg(test)]
mod tests {
    use crate::test_support::run;

    #[test]
    fn joins_arguments_onto_one_line() {
        assert_eq!(run("echo", &["hello", "world"], "").stdout, "hello world\n");
        assert_eq!(run("echo", &[], "").stdout, "\n");
    }

    #[test]
    fn dash_n_drops_the_newline() {
        assert_eq!(run("echo", &["-n", "no", "newline"], "").stdout, "no newline");
        assert_eq!(run("echo", &["-n"], "").stdout, "");
    }
}
