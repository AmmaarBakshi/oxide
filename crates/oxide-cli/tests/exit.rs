//! The `exit` builtin.

mod common;

use common::{run_c, run_stdin};

#[test]
fn exit_with_a_status() {
    run_c("exit-status", "exit 3").assert("", "", 3);
}

#[test]
fn exit_stops_the_rest_of_the_line() {
    run_c("exit-line", "echo a; exit 5; echo b").assert("a\n", "", 5);
}

#[test]
fn exit_stops_a_script_read_from_stdin() {
    run_stdin("exit-stdin", "echo a\nexit 4\necho b\n").assert("a\n", "", 4);
}

#[test]
fn exit_inside_a_block_stops_everything() {
    run_c(
        "exit-block",
        "if 1 { echo in; exit 6; echo not reached }; echo after",
    )
    .assert("in\n", "", 6);
}

#[test]
fn bare_exit_uses_the_last_status() {
    let out = run_c("exit-bare", "cat missing.txt; exit");
    assert_eq!(out.code, 1, "{:?}", out);
}

#[test]
fn exit_status_wraps_at_256() {
    run_c("exit-wrap", "exit 257").assert("", "", 1);
    run_c("exit-negative", "exit -1").assert("", "", 255);
}

#[test]
fn a_non_numeric_status_is_an_error() {
    run_c("exit-nan", "exit soon").assert("", "oxide: exit: soon: numeric argument required\n", 2);
}

#[test]
fn too_many_arguments_does_not_exit() {
    run_c("exit-many", "exit 1 2; echo still here").assert(
        "still here\n",
        "oxide: exit: too many arguments\n",
        0,
    );
}
