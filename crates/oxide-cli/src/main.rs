use anyhow::Result;
use oxide_core::shell::Shell;
use std::env;
use std::process::ExitCode;

const USAGE: &str = "\
Usage: oxide [OPTIONS] [SCRIPT]

Runs SCRIPT, or starts an interactive session when no script is given.

Options:
  -h, --help       Print this help and exit
  -V, --version    Print the version and exit
";

/// What the command line asks the shell to do.
#[derive(Debug, PartialEq)]
enum Action {
    Help,
    Version,
    Script(String),
    Repl,
}

fn parse_args(args: &[String]) -> Result<Action, String> {
    match args.first().map(String::as_str) {
        None => Ok(Action::Repl),
        Some("-h" | "--help") => Ok(Action::Help),
        Some("-V" | "--version") => Ok(Action::Version),
        Some(flag) if flag.starts_with('-') && flag != "-" => {
            Err(format!("unknown option '{}'", flag))
        }
        Some(script) => Ok(Action::Script(script.to_string())),
    }
}

fn main() -> Result<ExitCode> {
    let args: Vec<String> = env::args().skip(1).collect();

    let action = match parse_args(&args) {
        Ok(action) => action,
        Err(e) => {
            eprintln!("oxide: {}", e);
            eprintln!("Try 'oxide --help' for more information.");
            return Ok(ExitCode::from(2));
        }
    };

    match action {
        Action::Help => print!("{}", USAGE),
        Action::Version => println!("oxide {}", env!("CARGO_PKG_VERSION")),
        Action::Script(path) => Shell::new().run_script(&path)?,
        Action::Repl => Shell::new().run_repl()?,
    }

    Ok(ExitCode::SUCCESS)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Action, String> {
        let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
        parse_args(&args)
    }

    #[test]
    fn no_arguments_starts_the_repl() {
        assert_eq!(parse(&[]), Ok(Action::Repl));
    }

    #[test]
    fn help_and_version_flags() {
        assert_eq!(parse(&["-h"]), Ok(Action::Help));
        assert_eq!(parse(&["--help"]), Ok(Action::Help));
        assert_eq!(parse(&["-V"]), Ok(Action::Version));
        assert_eq!(parse(&["--version"]), Ok(Action::Version));
    }

    #[test]
    fn a_plain_argument_is_a_script() {
        assert_eq!(parse(&["build.ox"]), Ok(Action::Script("build.ox".into())));
    }

    #[test]
    fn unknown_flags_are_rejected() {
        assert_eq!(parse(&["--bogus"]), Err("unknown option '--bogus'".into()));
    }
}
