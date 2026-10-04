//! The stream trio a builtin reads from and writes to.

use std::io::{BufRead, Write};

/// The standard streams handed to a builtin for one invocation.
///
/// A builtin never touches the process's real stdin/stdout. It reads and
/// writes here, which lets the caller point it at a terminal, a file
/// (`> out.txt`), or the adjacent stage of a pipeline (`ls | grep rs`)
/// without the builtin knowing the difference.
pub struct Io<'a> {
    pub stdin: &'a mut dyn BufRead,
    pub stdout: &'a mut dyn Write,
    pub stderr: &'a mut dyn Write,
}

impl<'a> Io<'a> {
    pub fn new(
        stdin: &'a mut dyn BufRead,
        stdout: &'a mut dyn Write,
        stderr: &'a mut dyn Write,
    ) -> Self {
        Self { stdin, stdout, stderr }
    }
}
