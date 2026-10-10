//! Parameter expansion: `${NAME...}` with an operator.

mod common;

use common::run_c;

#[test]
fn default_values() {
    run_c(
        "param-default",
        "name=; echo ${name:-anonymous} ${missing-unset} [${name-unused}]",
    )
    .assert_ok("anonymous unset []\n");
}

#[test]
fn assigning_default_values() {
    run_c("param-assign", "echo ${count:=0}; echo $count").assert_ok("0\n0\n");
}

#[test]
fn defaults_for_function_arguments() {
    run_c(
        "param-args",
        "greet() { echo hello ${1:-world} }; greet; greet you",
    )
    .assert_ok("hello world\nhello you\n");
}

#[test]
fn a_bad_substitution_fails_the_command() {
    run_c("param-bad", "echo ${x!y}; echo $?").assert(
        "1\n",
        "oxide: ${x!y}: bad substitution\n",
        0,
    );
}
