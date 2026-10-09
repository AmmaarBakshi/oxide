use std::ffi::OsStr;
use std::time::Instant;

/// Times a stretch of work and reports it when profiling is switched on.
///
/// Reports are printed only when `OXIDE_PERF=1`, and they go to stderr so a
/// timing line can never end up in a pipe or a redirected file.
pub struct Profiler {
    start_time: Instant,
}

impl Profiler {
    pub fn start() -> Self {
        Self {
            start_time: Instant::now(),
        }
    }

    pub fn stop(&self, operation: &str) {
        if enabled() {
            eprintln!("[PERF] {} took {:?}", operation, self.start_time.elapsed());
        }
    }
}

/// Whether timing reports are switched on with `OXIDE_PERF=1`.
pub fn enabled() -> bool {
    is_on(std::env::var_os("OXIDE_PERF").as_deref())
}

fn is_on(value: Option<&OsStr>) -> bool {
    value == Some(OsStr::new("1"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_exactly_one_turns_profiling_on() {
        assert!(is_on(Some(OsStr::new("1"))));
        assert!(!is_on(None));
        assert!(!is_on(Some(OsStr::new("0"))));
        assert!(!is_on(Some(OsStr::new(""))));
    }
}
