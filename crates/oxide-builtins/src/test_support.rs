//! Helpers for exercising builtins against in-memory streams in unit tests.

use std::collections::HashMap;

use crate::{registry, Ctx, Io};

/// What a builtin produced: exit status, stdout, stderr.
pub struct Output {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

/// Runs the registered builtin `name` with `args`, feeding it `stdin`.
pub fn run(name: &str, args: &[&str], stdin: &str) -> Output {
    let builtin = registry().get(name).expect("builtin is registered");
    let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();

    let mut input = stdin.as_bytes();
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    let mut aliases = HashMap::new();
    let mut ctx = Ctx {
        aliases: &mut aliases,
        history: &[],
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
