use std::fs::{File, OpenOptions};
use std::io::{PipeWriter, Write};
use std::process::{Command, Stdio};

use oxide_parser::ast;

/// Opens a `>` / `>>` / `2>` / `2>>` target. When `append` is true the file
/// is opened in append mode; otherwise it is truncated/created.
///
/// Shared by process spawning and by builtins, so both honor a redirect the
/// same way. `/dev/null` works on every platform.
pub fn open_output(path: &str, append: bool) -> std::io::Result<File> {
    let path = null_device(path);
    if append {
        OpenOptions::new().create(true).append(true).open(path)
    } else {
        File::create(path)
    }
}

/// Maps `/dev/null`, the idiom scripts use to discard output, to `NUL` on
/// Windows, where no such path exists.
fn null_device(path: &str) -> &str {
    if cfg!(windows) && path == "/dev/null" {
        "NUL"
    } else {
        path
    }
}

/// Where one of a child's output streams goes.
enum Dest {
    /// Wherever the shell's own stream of the same name goes.
    Inherit,
    /// The shell's stdout, for a child stderr joined to it (`2>&1`).
    ShellStdout,
    /// The shell's stderr, for a child stdout joined to it (`>&2`).
    ShellStderr,
    File(File),
    /// The pipe to the next stage of a pipeline.
    Pipe(PipeWriter),
}

impl Dest {
    fn open(path: &str, append: bool) -> Self {
        match open_output(path, append) {
            Ok(file) => Dest::File(file),
            Err(e) => {
                eprintln!("oxide: redirection error: {}: {}", path, e);
                Dest::Inherit
            }
        }
    }

    /// A second handle to the same place, for the other stream of a pair
    /// joined with `2>&1` or `>&2`.
    fn duplicate(&self, inherited: Dest) -> Dest {
        let copy = match self {
            Dest::Inherit => return inherited,
            Dest::ShellStdout => return Dest::ShellStdout,
            Dest::ShellStderr => return Dest::ShellStderr,
            Dest::File(file) => file.try_clone().map(Dest::File),
            Dest::Pipe(pipe) => pipe.try_clone().map(Dest::Pipe),
        };
        copy.unwrap_or_else(|e| {
            eprintln!("oxide: redirection error: {}", e);
            Dest::Inherit
        })
    }

    fn into_stdio(self) -> Option<Stdio> {
        match self {
            Dest::Inherit => None,
            Dest::ShellStdout => Some(Stdio::from(std::io::stdout())),
            Dest::ShellStderr => Some(Stdio::from(std::io::stderr())),
            Dest::File(file) => Some(Stdio::from(file)),
            Dest::Pipe(pipe) => Some(Stdio::from(pipe)),
        }
    }
}

/// Points a child's stdout and stderr where `cmd`'s redirects say: files
/// (`>`, `>>`, `2>`, `2>>`, `&>`), or each other (`2>&1`, `>&2`). `pipe`, when
/// given, is where stdout goes unless a redirect says otherwise.
pub fn apply_outputs(process: &mut Command, cmd: &ast::Command, pipe: Option<PipeWriter>) {
    let mut out = match (&cmd.outfile, pipe) {
        (Some(path), _) => Dest::open(path, cmd.append),
        (None, Some(pipe)) => Dest::Pipe(pipe),
        (None, None) => Dest::Inherit,
    };
    let mut err = match &cmd.errfile {
        Some(path) => Dest::open(path, cmd.err_append),
        None => Dest::Inherit,
    };
    if cmd.err_to_out {
        err = out.duplicate(Dest::ShellStdout);
    }
    if cmd.out_to_err {
        out = err.duplicate(Dest::ShellStderr);
    }

    if let Some(stdio) = out.into_stdio() {
        process.stdout(stdio);
    }
    if let Some(stdio) = err.into_stdio() {
        process.stderr(stdio);
    }
}

/// Hooks up an input redirect (`< file`), feeding the file as the process stdin.
pub fn apply_input(process: &mut Command, infile: &Option<String>) {
    if let Some(path) = infile {
        match File::open(null_device(path)) {
            Ok(file) => {
                process.stdin(Stdio::from(file));
            }
            Err(e) => {
                eprintln!("oxide: redirection error: {}: {}", path, e);
            }
        }
    }
}

/// Reports an error about a command where that command's stderr would go,
/// so `missing-program 2>/dev/null` stays quiet the way it does elsewhere.
pub fn report(errfile: &Option<String>, append: bool, message: &str) {
    if let Some(path) = errfile {
        if let Ok(mut file) = open_output(path, append) {
            let _ = writeln!(file, "{}", message);
            return;
        }
    }
    eprintln!("{}", message);
}
