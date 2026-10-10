//! Arithmetic: `$(( ))`.

mod common;

use common::run_c;

#[test]
fn arithmetic_expands_to_its_value() {
    run_c(
        "arith-basic",
        "echo $((1 + 2 * 3)) $(( (1 + 2) * 3 )) $((7 % 3))",
    )
    .assert_ok("7 9 1\n");
}

#[test]
fn variables_count_with_or_without_a_dollar() {
    run_c(
        "arith-vars",
        "n=4; echo $((n * 2)) $(($n + 1)) $((missing + 1))",
    )
    .assert_ok("8 5 1\n");
}

#[test]
fn a_counter_drives_a_while_loop() {
    run_c(
        "arith-loop",
        "i=0; while [ $i -lt 3 ] { echo step $i; i=$((i + 1)) }",
    )
    .assert_ok("step 0\nstep 1\nstep 2\n");
}

#[test]
fn assignment_inside_arithmetic_sets_the_variable() {
    run_c("arith-assign", "echo $((y = 6)) $y; echo $((y += 2)) $y").assert_ok("6 6\n8 8\n");
}

#[test]
fn an_error_fails_the_command() {
    run_c("arith-error", "echo $((1 / 0)); echo status $?").assert(
        "status 1\n",
        "oxide: 1 / 0: division by zero\n",
        0,
    );
}

#[test]
fn let_assigns_and_reports_through_its_status() {
    run_c(
        "arith-let",
        "let n=6*7; echo $n; let n++ n*=2; echo $n; let 0 || echo zero is false",
    )
    .assert_ok("42\n86\nzero is false\n");
}
