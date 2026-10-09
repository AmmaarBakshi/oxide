//! Drives the built `oxide` binary the way a person or another program
//! would: commands go in on stdin (or `-c`), and a test compares what comes
//! back on stdout and stderr, plus the exit code.
//!
//! Every run gets its own scratch directory as its working directory and
//! home, so tests can create files freely, run in parallel, and never touch
//! the real history or config files.

// Each test binary uses its own subset of these helpers.
#![allow(dead_code)]

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Path to the `oxide` binary under test.
pub const OXIDE: &str = env!("CARGO_BIN_EXE_oxide-cli");

/// What one run of the binary produced.
#[derive(Debug)]
pub struct Output {
    pub stdout: String,
    pub stderr: String,
    pub code: i32,
}

impl Output {
    /// Asserts on all three results at once, so a failure shows the whole
    /// picture rather than just the first field that differed.
    #[track_caller]
    pub fn assert(&self, stdout: &str, stderr: &str, code: i32) {
        assert_eq!(
            (self.stdout.as_str(), self.stderr.as_str(), self.code),
            (stdout, stderr, code),
            "\n(stdout, stderr, code) differ"
        );
    }

    /// Asserts a clean run: this stdout, nothing on stderr, exit status 0.
    #[track_caller]
    pub fn assert_ok(&self, stdout: &str) {
        self.assert(stdout, "", 0);
    }
}

/// One invocation of the binary, built up before it is run.
pub struct Oxide {
    dir: PathBuf,
    args: Vec<String>,
    env: Vec<(String, String)>,
    stdin: String,
}

impl Oxide {
    /// A run in a fresh scratch directory named after `test`.
    pub fn new(test: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("oxide-it-{}-{}", std::process::id(), test));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create scratch dir");
        Self {
            dir,
            args: Vec::new(),
            env: Vec::new(),
            stdin: String::new(),
        }
    }

    pub fn arg(mut self, arg: &str) -> Self {
        self.args.push(arg.to_string());
        self
    }

    pub fn args(mut self, args: &[&str]) -> Self {
        self.args.extend(args.iter().map(|a| a.to_string()));
        self
    }

    pub fn env(mut self, key: &str, value: &str) -> Self {
        self.env.push((key.to_string(), value.to_string()));
        self
    }

    pub fn stdin(mut self, input: &str) -> Self {
        self.stdin = input.to_string();
        self
    }

    /// Creates `name` in the scratch directory before the run.
    pub fn file(self, name: &str, contents: &str) -> Self {
        let path = self.dir.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent dir");
        }
        fs::write(path, contents).expect("write fixture file");
        self
    }

    /// The scratch directory the shell runs in.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn run(&self) -> Output {
        let mut cmd = Command::new(OXIDE);
        cmd.args(&self.args)
            .current_dir(&self.dir)
            .env("HOME", &self.dir)
            .env("USERPROFILE", &self.dir)
            .env("OXIDE_CONFIG", self.dir.join("no-such-config.toml"))
            .env("OXIDE_BIN", OXIDE)
            .env_remove("OXIDE_PERF")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (key, value) in &self.env {
            cmd.env(key, value);
        }

        let mut child = cmd.spawn().expect("spawn oxide");
        let mut stdin = child.stdin.take().expect("child stdin");
        let input = self.stdin.clone();
        // Feed stdin from a thread so a large input can't deadlock against
        // a child that is busy filling its stdout pipe.
        let feeder = std::thread::spawn(move || {
            let _ = stdin.write_all(input.as_bytes());
        });
        let out = child.wait_with_output().expect("wait for oxide");
        let _ = feeder.join();

        Output {
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
            code: out.status.code().unwrap_or(-1),
        }
    }
}

/// Runs `script` piped in on stdin, in a scratch directory for `test`.
pub fn run_stdin(test: &str, script: &str) -> Output {
    Oxide::new(test).stdin(script).run()
}

/// Runs `oxide -c command` in a scratch directory for `test`.
pub fn run_c(test: &str, command: &str) -> Output {
    Oxide::new(test).args(&["-c", command]).run()
}
