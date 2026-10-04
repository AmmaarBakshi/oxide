//! Pipeline plumbing.
//!
//! A pipeline mixes two kinds of stage: OS processes, which are connected by
//! real kernel pipes, and shell builtins, which run in-process and can only be
//! handed an ordinary reader and writer. [`Upstream`] is the seam between
//! them — it describes what the previous stage left behind, and each stage
//! knows how to consume all three shapes.
//!
//! Process-to-process links stay true streaming pipes; a builtin's output is
//! buffered because a builtin has nowhere to block. That means a builtin stage
//! in the middle of a chain materializes its output before the next stage
//! starts, which is fine for the sizes builtins produce.

use std::io::{BufRead, BufReader, Cursor, Write};
use std::process::{Child, ChildStdout, Stdio};
use std::thread::JoinHandle;

use crate::redirect;

/// What the previous pipeline stage left for the next one to read.
pub enum Upstream {
    /// First stage — there is no previous output.
    None,
    /// An OS process's stdout, connected as a real pipe.
    Child(ChildStdout),
    /// A builtin's captured output.
    Buffer(Vec<u8>),
}

impl Upstream {
    /// Takes ownership of the upstream, leaving `None` behind.
    pub fn take(&mut self) -> Upstream {
        std::mem::replace(self, Upstream::None)
    }

    /// Turns the upstream into something a builtin can read from.
    ///
    /// With no upstream this falls back to the requested input redirect, and
    /// failing that to the terminal's stdin, matching how a lone builtin runs.
    pub fn into_reader(self, infile: &Option<String>) -> Result<Box<dyn BufRead>, String> {
        match self {
            Upstream::Child(stdout) => Ok(Box::new(BufReader::new(stdout))),
            Upstream::Buffer(bytes) => Ok(Box::new(Cursor::new(bytes))),
            Upstream::None => match infile {
                Some(path) => {
                    let file = std::fs::File::open(path)
                        .map_err(|e| format!("oxide: redirection error: {}: {}", path, e))?;
                    Ok(Box::new(BufReader::new(file)))
                }
                None => Ok(Box::new(std::io::stdin().lock())),
            },
        }
    }
}

/// A process stage that was spawned, plus the thread (if any) feeding it.
pub struct Spawned {
    pub child: Child,
    /// Set when the stage's stdin came from a builtin's buffer: a helper
    /// thread writes those bytes in. Writing from a thread keeps a buffer
    /// larger than the OS pipe capacity from deadlocking us against a child
    /// that hasn't started reading yet.
    pub feeder: Option<JoinHandle<()>>,
}

/// Spawns one external stage of a pipeline, wiring `upstream` to its stdin.
pub fn spawn_stage(
    program: &str,
    args: &[String],
    upstream: Upstream,
    is_last: bool,
    outfile: &Option<String>,
    append: bool,
    infile: &Option<String>,
) -> Result<Spawned, String> {
    let mut process = std::process::Command::new(program);
    process.args(args);

    let mut pending_bytes = None;
    match upstream {
        Upstream::Child(stdout) => {
            process.stdin(Stdio::from(stdout));
        }
        Upstream::Buffer(bytes) => {
            process.stdin(Stdio::piped());
            pending_bytes = Some(bytes);
        }
        Upstream::None => {
            // First stage: honor an explicit "< file" input redirect if present.
            redirect::apply_input(&mut process, infile);
        }
    }

    if is_last {
        redirect::apply_output(&mut process, outfile, append);
    } else {
        process.stdout(Stdio::piped());
    }

    let mut child = process
        .spawn()
        .map_err(|_| format!("oxide: command not found: {}", program))?;

    let feeder = pending_bytes.and_then(|bytes| {
        child.stdin.take().map(|mut sink| {
            std::thread::spawn(move || {
                // A closed pipe here just means the child exited early
                // (`... | head -1`), which is not an error worth reporting.
                let _ = sink.write_all(&bytes);
            })
        })
    });

    Ok(Spawned { child, feeder })
}

/// Opens a builtin's output sink: the final redirect target, the terminal, or
/// a buffer that becomes the next stage's input.
pub fn builtin_sink(
    is_last: bool,
    outfile: &Option<String>,
    append: bool,
) -> Result<BuiltinSink, String> {
    if !is_last {
        return Ok(BuiltinSink::Buffer(Vec::new()));
    }
    match outfile {
        Some(path) => redirect::open_output(path, append)
            .map(BuiltinSink::File)
            .map_err(|e| format!("oxide: redirection error: {}: {}", path, e)),
        None => Ok(BuiltinSink::Stdout(std::io::stdout())),
    }
}

/// Where a builtin's stdout goes for one invocation.
pub enum BuiltinSink {
    Stdout(std::io::Stdout),
    File(std::fs::File),
    Buffer(Vec<u8>),
}

impl BuiltinSink {
    /// Borrows the sink as a writer to hand to the builtin.
    pub fn as_writer(&mut self) -> &mut dyn Write {
        match self {
            BuiltinSink::Stdout(s) => s,
            BuiltinSink::File(f) => f,
            BuiltinSink::Buffer(b) => b,
        }
    }

    /// Flushes and converts the sink into the upstream for the next stage.
    pub fn finish(mut self) -> Upstream {
        let _ = self.as_writer().flush();
        match self {
            BuiltinSink::Buffer(bytes) => Upstream::Buffer(bytes),
            _ => Upstream::None,
        }
    }
}
