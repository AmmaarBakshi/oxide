//! Shell options turned on with `set`.

mod common;

use common::run_c;

#[test]
fn set_e_stops_at_the_first_failure() {
    run_c(
        "set-e",
        "set -e; echo a; cat missing.txt 2>/dev/null; echo b",
    )
    .assert("a\n", "", 1);
}

#[test]
fn set_e_ignores_failures_that_are_tested() {
    run_c(
        "set-e-tested",
        "set -e; false || echo recovered; if false { echo no }; while false { echo no }; false && echo no; echo end",
    )
    .assert_ok("recovered\nend\n");
}

#[test]
fn set_e_counts_the_last_command_of_a_chain() {
    run_c("set-e-chain", "set -e; true && false; echo never").assert("", "", 1);
}

#[test]
fn set_e_reaches_into_functions() {
    run_c(
        "set-e-function",
        "set -e; f() { echo in; false; echo after }; f; echo never",
    )
    .assert("in\n", "", 1);
}

#[test]
fn set_plus_e_turns_it_off_again() {
    run_c("set-e-off", "set -e; set +e; false; echo still here").assert_ok("still here\n");
}

#[test]
fn bad_options_are_rejected() {
    run_c("set-bad", "set -q").assert("", "oxide: set: -q: invalid option\n", 2);
}

#[test]
fn set_x_traces_each_command_expanded() {
    run_c(
        "set-x",
        "set -x; name=\"a b\"; echo hi $name | cat; set +x; echo quiet",
    )
    .assert(
        "hi a b\nquiet\n",
        "+ name='a b'\n+ echo hi a b\n+ cat\n+ set +x\n",
        0,
    );
}
