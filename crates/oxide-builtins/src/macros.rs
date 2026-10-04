//! Small helpers every builtin uses to talk to its [`Io`](crate::Io) streams.
//!
//! Builtins must not `println!` — that bypasses redirection and pipelines.
//! These macros write to the streams the caller supplied and bail out with a
//! non-zero status when the sink has gone away (a closed pipe, a full disk),
//! which is what stops `ls | head` style chains from spinning forever.

/// Writes a formatted line to stdout, returning `1` from the enclosing
/// function if the stream is gone.
macro_rules! wln {
    ($io:expr, $($arg:tt)*) => {
        if ::std::writeln!($io.stdout, $($arg)*).is_err() {
            return 1;
        }
    };
}

/// Writes a formatted line to stderr. Diagnostics are best-effort: if stderr
/// is gone there is nowhere left to report that fact, so the error is dropped.
macro_rules! ewln {
    ($io:expr, $($arg:tt)*) => {{
        let _ = ::std::writeln!($io.stderr, $($arg)*);
    }};
}
