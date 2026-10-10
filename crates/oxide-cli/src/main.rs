use anyhow::Result;
use oxide_core::shell::Shell;
use std::env;
use std::io::IsTerminal;
use std::process::ExitCode;

const USAGE: &str = "\
Usage: oxide [OPTIONS] [SCRIPT [ARGS...]]
       oxide -c COMMAND [NAME [ARGS...]]

Runs SCRIPT, or starts an interactive session when no script is given.
When standard input is not a terminal, commands are read from it instead.
ARGS become the positional parameters $1, $2, ...; $0 is SCRIPT, or NAME.

Options:
  -c COMMAND       Run COMMAND and exit with its status
  -h, --help       Print this help and exit
  -V, --version    Print the version and exit
";

/// What the command line asks the shell to do.
#[derive(Debug, PartialEq)]
enum Action {
    Help,
    Version,
    /// `-c COMMAND [NAME [ARGS...]]`.
    Command {
        command: String,
        name: Option<String>,
        args: Vec<String>,
    },
    Script {
        path: String,
        args: Vec<String>,
    },
    Repl,
}

fn parse_args(args: &[String]) -> Result<Action, String> {
    match args.first().map(String::as_str) {
        None => Ok(Action::Repl),
        Some("-h" | "--help") => Ok(Action::Help),
        Some("-V" | "--version") => Ok(Action::Version),
        Some("-c") => match args.get(1) {
            Some(command) => Ok(Action::Command {
                command: command.clone(),
                name: args.get(2).cloned(),
                args: args.iter().skip(3).cloned().collect(),
            }),
            None => Err("option '-c' requires an argument".to_string()),
        },
        Some(flag) if flag.starts_with('-') && flag != "-" => {
            Err(format!("unknown option '{}'", flag))
        }
        Some(script) => Ok(Action::Script {
            path: script.to_string(),
            args: args[1..].to_vec(),
        }),
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

    let mut shell = Shell::new();
    match action {
        Action::Help => print!("{}", USAGE),
        Action::Version => println!("oxide {}", env!("CARGO_PKG_VERSION")),
        Action::Command {
            command,
            name,
            args,
        } => {
            shell.set_arguments(name, args);
            let status = shell.run_command(&command);
            return Ok(ExitCode::from(status as u8));
        }
        Action::Script { path, args } => {
            shell.set_arguments(Some(path.clone()), args);
            shell.run_script(&path)?
        }
        Action::Repl if std::io::stdin().is_terminal() => shell.run_repl()?,
        Action::Repl => shell.run_stdin()?,
    }

    // Without an explicit `exit`, the shell exits with the status of the
    // last command it ran.
    let status = shell
        .state
        .exit_status
        .unwrap_or(shell.state.last_exit_code);
    Ok(ExitCode::from(status as u8))
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
    fn dash_c_takes_the_next_argument_as_a_command() {
        assert_eq!(
            parse(&["-c", "echo hi"]),
            Ok(Action::Command {
                command: "echo hi".into(),
                name: None,
                args: vec![]
            })
        );
        assert_eq!(
            parse(&["-c", "echo $1", "me", "a", "b"]),
            Ok(Action::Command {
                command: "echo $1".into(),
                name: Some("me".into()),
                args: vec!["a".into(), "b".into()]
            })
        );
        assert_eq!(
            parse(&["-c"]),
            Err("option '-c' requires an argument".into())
        );
    }

    #[test]
    fn a_plain_argument_is_a_script_and_the_rest_its_arguments() {
        assert_eq!(
            parse(&["build.ox", "--release", "x"]),
            Ok(Action::Script {
                path: "build.ox".into(),
                args: vec!["--release".into(), "x".into()]
            })
        );
    }

    #[test]
    fn unknown_flags_are_rejected() {
        assert_eq!(parse(&["--bogus"]), Err("unknown option '--bogus'".into()));
    }
}
