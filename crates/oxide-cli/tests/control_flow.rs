//! `if`/`elif`/`else` and `while`, with commands as their conditions.

mod common;

use common::{run_c, run_stdin, Oxide};

#[test]
fn if_takes_the_branch_whose_condition_succeeds() {
    run_c("if-true", "if true { echo yes } else { echo no }").assert_ok("yes\n");
    run_c("if-false", "if false { echo yes } else { echo no }").assert_ok("no\n");
}

#[test]
fn elif_is_tried_in_order() {
    let script = "for n in 1 2 3 {
        if [ $n = 1 ] {
            echo one
        } elif [ $n = 2 ] {
            echo two
        } else {
            echo many
        }
    }";
    run_c("if-elif", script).assert_ok("one\ntwo\nmany\n");
}

#[test]
fn a_condition_can_be_any_command() {
    let out = Oxide::new("if-file")
        .file("notes.txt", "remember the milk\n")
        .args(&[
            "-c",
            "if grep milk notes.txt > /dev/null { echo found } else { echo missing }",
        ])
        .run();
    out.assert_ok("found\n");
}

#[test]
fn the_condition_s_own_output_is_shown() {
    run_c("if-output", "if echo checking { echo then }").assert_ok("checking\nthen\n");
}

#[test]
fn conditions_chain_with_and_and_or() {
    run_c(
        "if-chain",
        "if true && false { echo a } else { echo b }; if false || true { echo c }",
    )
    .assert_ok("b\nc\n");
}

#[test]
fn a_failing_command_in_the_condition_is_just_false() {
    let out = run_c(
        "if-missing",
        "if no-such-command-oxide { echo yes } else { echo no }",
    );
    assert_eq!(out.stdout, "no\n", "{:?}", out);
    assert_eq!(out.code, 0);
}

#[test]
fn the_status_of_an_if_is_its_branch_s() {
    run_c(
        "if-status",
        "if true { false }; echo $?; if false { true }; echo $?",
    )
    .assert_ok("1\n0\n");
}

#[test]
fn while_runs_until_its_condition_fails() {
    run_c(
        "while-count",
        "x=; while [ \"$x\" != aaa ] { x=a$x; echo $x }",
    )
    .assert_ok("a\naa\naaa\n");
}

#[test]
fn while_read_walks_the_lines_of_stdin() {
    Oxide::new("while-read")
        .args(&["-c", "while read line { echo [$line] }"])
        .stdin("first\nsecond\n")
        .run()
        .assert_ok("[first]\n[second]\n");
}

#[test]
fn the_status_of_a_while_is_its_body_s() {
    run_c("while-status-none", "while false { true }; echo $?").assert_ok("0\n");
    run_c(
        "while-status-body",
        "x=; while [ -z \"$x\" ] { x=1; false }; echo $?",
    )
    .assert_ok("1\n");
}

#[test]
fn blocks_nest_and_span_lines() {
    run_stdin(
        "nested",
        "for a in x y {\n  if [ $a = y ] {\n    echo found-$a\n  } else {\n    echo skip-$a\n  }\n}\n",
    )
    .assert_ok("skip-x\nfound-y\n");
}

#[test]
fn break_leaves_the_loop() {
    run_c(
        "loop-break",
        "for x in a b c d { if [ $x = c ] { break }; echo $x }; echo after",
    )
    .assert_ok("a\nb\nafter\n");
}

#[test]
fn continue_skips_to_the_next_pass() {
    run_c(
        "loop-continue",
        "for x in 1 2 3 4 { if [ $x = 2 ] || [ $x = 3 ] { continue }; echo $x }",
    )
    .assert_ok("1\n4\n");
}

#[test]
fn break_works_in_while_true() {
    run_c(
        "loop-while-break",
        "x=; while true { x=a$x; if [ $x = aaa ] { break } }; echo $x",
    )
    .assert_ok("aaa\n");
}

#[test]
fn a_count_reaches_outer_loops() {
    run_c(
        "loop-nested",
        "for a in 1 2 { for b in x y z { if [ $b = y ] { continue 2 }; if [ $a = 2 ] { break 2 }; echo $a$b } }; echo done",
    )
    .assert_ok("1x\ndone\n");
}

#[test]
fn break_outside_a_loop_is_an_error() {
    run_c("loop-break-top", "break; echo still here").assert(
        "still here\n",
        "oxide: break: only meaningful in a loop\n",
        0,
    );
}
