//! Exit statuses: of commands, of compound statements, and of the shell.

mod common;

use common::{run_c, run_stdin, Oxide};

#[test]
fn a_script_exits_with_its_last_status() {
    let out = Oxide::new("status-script")
        .file("fail.ox", "echo start\ncat missing.txt\n")
        .arg("fail.ox")
        .run();
    assert_eq!((out.stdout.as_str(), out.code), ("start\n", 1), "{:?}", out);
}

#[test]
fn stdin_exits_with_its_last_status() {
    let out = run_stdin("status-stdin", "cat missing.txt\n");
    assert_eq!(out.code, 1, "{:?}", out);
    assert_eq!(
        run_stdin("status-stdin-ok", "cat missing.txt\necho fine\n").code,
        0
    );
}

#[test]
fn an_if_with_no_branch_taken_succeeds() {
    let out = run_c("status-if", "cat missing.txt; if 0 { echo no }");
    assert_eq!((out.stdout.as_str(), out.code), ("", 0), "{:?}", out);
}

#[test]
fn a_loop_that_never_runs_succeeds() {
    let out = run_c("status-while", "cat missing.txt; while 0 { echo no }");
    assert_eq!(out.code, 0, "{:?}", out);
}

#[test]
fn dollar_question_reads_the_last_status() {
    let out = run_c("status-dollar-q", "cat missing.txt; echo $?; echo \"[$?]\"");
    assert_eq!(out.stdout, "1\n[0]\n", "{:?}", out);
}

#[test]
fn command_not_found_is_127() {
    let out = run_c("status-127", "definitely-not-a-command-oxide");
    assert_eq!(out.code, 127, "{:?}", out);
}

#[cfg(unix)]
#[test]
fn a_process_killed_by_a_signal_reports_128_plus_the_signal() {
    let out = run_c("status-signal", "sh -c 'kill -9 $$'");
    assert_eq!(out.code, 137, "{:?}", out);
}
