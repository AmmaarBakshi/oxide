//! User-defined functions.

mod common;

use common::{run_c, run_stdin, Oxide};

#[test]
fn a_function_runs_its_body_each_time_it_is_called() {
    run_c("fn-call", "greet() { echo hello }; greet; greet").assert_ok("hello\nhello\n");
}

#[test]
fn a_function_can_span_lines_in_a_script() {
    run_stdin(
        "fn-script",
        "function banner {\n    echo ===\n    echo title\n}\nbanner\n",
    )
    .assert_ok("===\ntitle\n");
}

#[test]
fn a_function_s_status_is_its_last_command_s() {
    run_c("fn-status", "fails() { true; false }; fails; echo $?").assert_ok("1\n");
}

#[test]
fn a_function_feeds_a_pipeline_and_a_redirect() {
    let oxide = Oxide::new("fn-pipe").args(&[
        "-c",
        "two() { echo a; echo b }; two | grep b; two > out.txt; cat out.txt",
    ]);
    oxide.run().assert_ok("b\na\nb\n");
}

#[test]
fn a_function_shadows_a_builtin() {
    run_c("fn-shadow", "pwd() { echo not the builtin }; pwd").assert_ok("not the builtin\n");
}

#[test]
fn type_knows_about_functions() {
    run_c("fn-type", "f() { true }; type f").assert_ok("f is a function\n");
}

#[test]
fn runaway_recursion_fails_cleanly() {
    let out = run_c(
        "fn-recursion",
        "forever() { forever }; forever; echo survived",
    );
    assert!(out.stdout.ends_with("survived\n"), "{:?}", out);
    assert!(
        out.stderr.contains("maximum function nesting level"),
        "{:?}",
        out
    );
}
