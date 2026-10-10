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

#[test]
fn arguments_become_positional_parameters() {
    run_c(
        "fn-args",
        "greet() { echo hello $1, you have $# args: $* }; greet Ann x y; greet",
    )
    .assert_ok("hello Ann, you have 3 args: Ann x y\nhello , you have 0 args:\n");
}

#[test]
fn quoted_dollar_at_passes_arguments_on_whole() {
    run_c(
        "fn-at",
        "show() { for a in \"$@\" { echo [$a] } }; wrap() { show \"$@\" }; wrap 'a b' c",
    )
    .assert_ok("[a b]\n[c]\n");
}

#[test]
fn a_call_gets_its_own_parameters_and_gives_them_back() {
    run_c(
        "fn-nested-args",
        "inner() { echo inner $1 }; outer() { inner x; echo outer $1 }; outer y",
    )
    .assert_ok("inner x\nouter y\n");
}

#[test]
fn dollar_zero_is_the_shell() {
    run_c("fn-zero", "echo $0").assert_ok("oxide\n");
}

#[test]
fn local_variables_vanish_when_the_function_returns() {
    run_c(
        "fn-local",
        "x=global; f() { local x=inner y; y=set; echo $x $y }; f; echo $x [$y]",
    )
    .assert_ok("inner set\nglobal []\n");
}

#[test]
fn locals_are_visible_to_functions_called_from_inside() {
    run_c(
        "fn-local-dynamic",
        "show() { echo $v }; f() { local v=\"two words\"; show }; v=outer; f; show",
    )
    .assert_ok("two words\nouter\n");
}

#[test]
fn local_outside_a_function_is_an_error() {
    run_c("fn-local-top", "local x=1").assert(
        "",
        "oxide: local: can only be used in a function\n",
        1,
    );
}

#[test]
fn return_leaves_the_function_with_a_status() {
    run_c(
        "fn-return",
        "check() { if [ $1 = ok ] { return 0 }; echo bad: $1; return 3; echo never }; check ok; echo $?; check no; echo $?",
    )
    .assert_ok("0\nbad: no\n3\n");
}

#[test]
fn return_from_inside_a_loop_leaves_the_whole_function() {
    run_c(
        "fn-return-loop",
        "first() { for x in a b c { echo $x; return } }; first; echo after",
    )
    .assert_ok("a\nafter\n");
}

#[test]
fn return_outside_a_function_is_an_error() {
    run_c("fn-return-top", "return 1; echo still here").assert(
        "still here\n",
        "oxide: return: can only return from a function\n",
        0,
    );
}
