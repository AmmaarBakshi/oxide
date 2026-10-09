//! The command line itself: flags, `-c`, and commands piped on stdin.

mod common;

use common::{run_c, run_stdin, Oxide};

#[test]
fn version_flag_prints_the_crate_version() {
    let out = Oxide::new("version").arg("--version").run();
    out.assert_ok(&format!("oxide {}\n", env!("CARGO_PKG_VERSION")));
}

#[test]
fn help_flag_lists_the_options() {
    let out = Oxide::new("help").arg("-h").run();
    assert_eq!(out.code, 0);
    assert!(out.stdout.starts_with("Usage: oxide"), "{}", out.stdout);
    assert!(out.stdout.contains("-c COMMAND"), "{}", out.stdout);
}

#[test]
fn unknown_option_fails_with_status_2() {
    let out = Oxide::new("unknown-option").arg("--frobnicate").run();
    out.assert(
        "",
        "oxide: unknown option '--frobnicate'\nTry 'oxide --help' for more information.\n",
        2,
    );
}

#[test]
fn dash_c_runs_the_command() {
    run_c("dash-c", "echo hello; echo world").assert_ok("hello\nworld\n");
}

#[test]
fn dash_c_exits_with_the_command_status() {
    let out = run_c("dash-c-status", "cat missing.txt");
    assert_eq!(out.code, 1);
    assert!(out.stderr.contains("missing.txt"), "{}", out.stderr);
}

#[test]
fn stdin_lines_run_in_order() {
    run_stdin("stdin", "echo one\n# a comment\n\necho two\n").assert_ok("one\ntwo\n");
}

#[test]
fn commands_run_in_the_scratch_directory() {
    let oxide = Oxide::new("scratch")
        .file("greeting.txt", "hi there\n")
        .stdin("cat greeting.txt\necho saved > out.txt\n");
    oxide.run().assert_ok("hi there\n");
    let saved = std::fs::read_to_string(oxide.dir().join("out.txt")).unwrap();
    assert_eq!(saved, "saved\n");
}
