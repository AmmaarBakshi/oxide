//! Variables: special parameters, assignments and the environment.

mod common;

use common::{run_c, run_stdin};

#[test]
fn dollar_dollar_is_the_shell_pid() {
    let out = run_c("var-pid", "echo $$; echo \"$$\" '$$'");
    assert_eq!(out.stdout, format!("{0}\n{0} $$\n", out.pid), "{:?}", out);
}

#[test]
fn assignments_set_shell_variables() {
    run_c("var-assign", "GREETING=hello; echo $GREETING world").assert_ok("hello world\n");
}

#[test]
fn later_assignments_see_earlier_ones() {
    run_c("var-chain", "A=1 B=$A$A; echo $B").assert_ok("11\n");
}

#[test]
fn variables_last_from_line_to_line() {
    run_stdin("var-lines", "NAME=oxide\necho $NAME\n").assert_ok("oxide\n");
}

#[test]
fn shell_variables_are_not_exported_until_asked() {
    let out = run_c(
        "var-export",
        "OXIDE_T_VAR=1; env | grep OXIDE_T_VAR; export OXIDE_T_VAR; env | grep OXIDE_T_VAR",
    );
    out.assert_ok("OXIDE_T_VAR=1\n");
}

#[test]
fn unset_removes_shell_variables() {
    run_c("var-unset", "X=1; unset X; echo [$X]").assert_ok("[]\n");
}

#[test]
fn a_prefix_assignment_reaches_only_that_command() {
    let out = run_c(
        "var-prefix",
        "OXIDE_P=1 \"$OXIDE_BIN\" -c 'echo child sees $OXIDE_P'; echo shell sees [$OXIDE_P]",
    );
    out.assert_ok("child sees 1\nshell sees []\n");
}

#[test]
fn a_prefix_assignment_reaches_builtins_too() {
    run_c("var-prefix-builtin", "OXIDE_P=2 env | grep OXIDE_P=").assert_ok("OXIDE_P=2\n");
}

#[test]
fn a_prefix_assignment_leaves_an_exported_variable_as_it_was() {
    let out = run_c(
        "var-prefix-restore",
        "export OXIDE_P=old; OXIDE_P=new env | grep OXIDE_P=; echo $OXIDE_P",
    );
    out.assert_ok("OXIDE_P=new\nold\n");
}
