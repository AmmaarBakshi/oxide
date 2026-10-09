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

use std::cell::RefCell;
use std::io::{BufRead, BufReader, Cursor, PipeReader, Write};
use std::process::{Child, Stdio};
use std::rc::Rc;
use std::thread::JoinHandle;

use crate::redirect;
use oxide_parser::ast;

/// What the previous pipeline stage left for the next one to read.
pub enum Upstream {
    /// First stage — there is no previous output.
    None,
    /// An OS process's output, connected as a real pipe.
    Pipe(PipeReader),
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
            Upstream::Pipe(pipe) => Ok(Box::new(BufReader::new(pipe))),
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
    /// The read end of this stage's output, for the next stage. `None` for
    /// the last stage.
    pub output: Option<PipeReader>,
}

/// Spawns one external stage of a pipeline, wiring `upstream` to its stdin.
/// Redirects come from `cmd`; `program` is its resolved path.
pub fn spawn_stage(
    program: &str,
    args: &[String],
    upstream: Upstream,
    is_last: bool,
    cmd: &ast::Command,
) -> Result<Spawned, String> {
    let mut process = std::process::Command::new(program);
    process.args(args);

    let mut pending_bytes = None;
    match upstream {
        Upstream::Pipe(pipe) => {
            process.stdin(Stdio::from(pipe));
        }
        Upstream::Buffer(bytes) => {
            process.stdin(Stdio::piped());
            pending_bytes = Some(bytes);
        }
        Upstream::None => {
            // First stage: honor an explicit "< file" input redirect if present.
            redirect::apply_input(&mut process, &cmd.infile);
        }
    }

    // Every stage but the last writes into a pipe for the next one. An OS
    // pipe rather than `Stdio::piped()` lets `2>&1` join stderr onto it.
    let (output, pipe) = if is_last {
        (None, None)
    } else {
        let (reader, writer) = std::io::pipe().map_err(|e| format!("oxide: pipeline: {}", e))?;
        (Some(reader), Some(writer))
    };
    redirect::apply_outputs(&mut process, cmd, pipe);

    let mut child = process
        .spawn()
        .map_err(|_| format!("oxide: command not found: {}", program))?;
    // `process` still holds our copy of the pipe's write end; close it so
    // the next stage sees end-of-file once this one exits.
    drop(process);

    let feeder = pending_bytes.and_then(|bytes| {
        child.stdin.take().map(|mut sink| {
            std::thread::spawn(move || {
                // A closed pipe here just means the child exited early
                // (`... | head -1`), which is not an error worth reporting.
                let _ = sink.write_all(&bytes);
            })
        })
    });

    Ok(Spawned {
        child,
        feeder,
        output,
    })
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

/// One buffer behind any number of writers. It gives a builtin a stdout and
/// a stderr that land in the same place (`2>&1`), in the order written.
#[derive(Clone, Default)]
pub struct SharedBuf(Rc<RefCell<Vec<u8>>>);

impl SharedBuf {
    /// Takes everything written so far.
    pub fn take(&self) -> Vec<u8> {
        std::mem::take(&mut *self.0.borrow_mut())
    }
}

impl Write for SharedBuf {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
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
