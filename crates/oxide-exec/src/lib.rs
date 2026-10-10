/// `eprintln!` for shell errors: inside a script, the message names the
/// file and line it came from. See [`location`].
macro_rules! shell_error {
    ($($arg:tt)*) => {
        eprintln!("{}", $crate::location::tag(&format!($($arg)*)))
    };
}

pub mod executor;
pub mod jobs;
pub mod location;
pub mod pipeline;
pub mod process;
pub mod redirect;
pub mod signals;
pub mod subshell;
