//! Command substitution: `$(...)`.

mod common;

use common::run_c;

#[test]
fn output_replaces_the_substitution() {
    run_c("subst-basic", "echo [$(echo hi)]").assert_ok("[hi]\n");
}

#[test]
fn trailing_newlines_are_dropped() {
    run_c("subst-newlines", "X=$(echo a; echo b); echo \"[$X]\"").assert_ok("[a\nb]\n");
}

#[test]
fn unquoted_output_is_split_into_words() {
    run_c("subst-split", "for w in $(echo a  b c) { echo [$w] }").assert_ok("[a]\n[b]\n[c]\n");
}

#[test]
fn a_pipeline_can_be_substituted() {
    run_c("subst-pipeline", "echo $(echo abc | grep b)").assert_ok("abc\n");
}

#[test]
fn an_external_program_can_be_substituted() {
    run_c(
        "subst-external",
        "echo [$(\"$OXIDE_BIN\" -c 'echo from child')]",
    )
    .assert_ok("[from child]\n");
}

#[test]
fn an_assignment_has_the_substitution_s_status() {
    run_c(
        "subst-status",
        "X=$(cat missing.txt 2>/dev/null); echo $?; Y=$(echo ok); echo $?",
    )
    .assert_ok("1\n0\n");
}

#[test]
fn a_substitution_cannot_change_the_shell_around_it() {
    run_c(
        "subst-isolated",
        "X=outer; Y=$(X=inner; echo $X); echo $X $Y; Z=$(exit 3); echo still here $?",
    )
    .assert_ok("outer inner\nstill here 3\n");
}
