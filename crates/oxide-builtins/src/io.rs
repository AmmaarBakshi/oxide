//! The stream trio a builtin reads from and writes to.

use std::fs::File;
use std::io::{BufRead, BufReader, ErrorKind, Write};

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

    /// Feeds each input operand to `f` in turn, the way `cat`, `head` and
    /// `wc` treat their arguments: every operand is a file to open, `-` means
    /// stdin, and no operands at all means stdin alone.
    ///
    /// `f` receives the operand's name, a reader for it, and stdout. A file
    /// that can't be opened or read is reported under `cmd` and skipped, so
    /// one bad operand doesn't hide the output of the rest. Returns 0 if every
    /// input was processed in full, 1 otherwise.
    pub fn each_input<F>(&mut self, cmd: &str, operands: &[&str], mut f: F) -> i32
    where
        F: FnMut(&str, &mut dyn BufRead, &mut dyn Write) -> std::io::Result<()>,
    {
        let operands = if operands.is_empty() { &["-"] } else { operands };

        let mut status = 0;
        for &name in operands {
            let result = if name == "-" {
                f(name, &mut *self.stdin, &mut *self.stdout)
            } else {
                File::open(name).and_then(|file| f(name, &mut BufReader::new(file), &mut *self.stdout))
            };
            match result {
                Ok(()) => {}
                // The consumer hung up; nothing else we write will be read.
                Err(e) if e.kind() == ErrorKind::BrokenPipe => return 1,
                Err(e) => {
                    let _ = writeln!(self.stderr, "{}: {}: {}", cmd, name, e);
                    status = 1;
                }
            }
        }
        status
    }
}
