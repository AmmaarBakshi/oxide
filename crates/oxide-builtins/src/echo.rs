use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    // `-n` leaves off the trailing newline, for printing a prompt or building
    // a line up in pieces. As in bash, every leading `-n`, `-nn`, ... is an
    // option, and the first word that isn't one starts the text.
    let flags = args.iter().take_while(|arg| is_dash_n(arg)).count();
    let newline = flags == 0;
    let words = &args[flags..];

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

/// `-n`, `-nn`, `-nnn`... but not a lone `-`, which is printed as text.
fn is_dash_n(arg: &str) -> bool {
    arg.strip_prefix('-')
        .is_some_and(|f| !f.is_empty() && f.bytes().all(|b| b == b'n'))
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
        assert_eq!(
            run("echo", &["-n", "no", "newline"], "").stdout,
            "no newline"
        );
        assert_eq!(run("echo", &["-n"], "").stdout, "");
    }

    #[test]
    fn repeated_dash_n_is_still_one_option() {
        assert_eq!(run("echo", &["-n", "-n", "hi"], "").stdout, "hi");
        assert_eq!(run("echo", &["-nn", "hi"], "").stdout, "hi");
        // Only leading words count, and anything else is printed as text.
        assert_eq!(run("echo", &["-n", "a", "-n"], "").stdout, "a -n");
        assert_eq!(
            run("echo", &["-nx", "-"], "").stdout,
            "-nx -
"
        );
    }
}
