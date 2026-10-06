//! Helpers for exercising builtins against in-memory streams in unit tests.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::{registry, Ctx, Io};

/// What a builtin produced: exit status, stdout, stderr.
pub struct Output {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

/// Runs the registered builtin `name` with `args`, feeding it `stdin`.
pub fn run(name: &str, args: &[&str], stdin: &str) -> Output {
    run_in(name, args, stdin, &[], &[])
}

/// Like [`run`] with no stdin, but with `aliases` defined in the shell.
pub fn run_with_aliases(name: &str, args: &[&str], aliases: &[(&str, &str)]) -> Output {
    run_in(name, args, "", aliases, &[])
}

/// Like [`run`] with no stdin, but with `history` as the session so far.
pub fn run_with_history(name: &str, args: &[&str], history: &[&str]) -> Output {
    run_in(name, args, "", &[], history)
}

fn run_in(
    name: &str,
    args: &[&str],
    stdin: &str,
    aliases: &[(&str, &str)],
    history: &[&str],
) -> Output {
    let builtin = registry().get(name).expect("builtin is registered");
    let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();

    let mut input = stdin.as_bytes();
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    let mut aliases: HashMap<String, String> = aliases
        .iter()
        .map(|(name, value)| (name.to_string(), value.to_string()))
        .collect();
    let history: Vec<String> = history.iter().map(|cmd| cmd.to_string()).collect();
    let mut ctx = Ctx {
        aliases: &mut aliases,
        history: &history,
    };

    let status = {
        let mut io = Io::new(&mut input, &mut stdout, &mut stderr);
        builtin.run(&args, &mut io, &mut ctx)
    };

    Output {
        status,
        stdout: String::from_utf8(stdout).expect("stdout is UTF-8"),
        stderr: String::from_utf8(stderr).expect("stderr is UTF-8"),
    }
}

/// A fresh, empty directory for `test` under the system temp dir. Unique per
/// test and per test-binary run, so tests can run in parallel.
pub fn scratch_dir(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("oxide-builtins-{}-{}", std::process::id(), test));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}
