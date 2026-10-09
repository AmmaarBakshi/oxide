//! Scripts: run from a file or piped in on stdin.

mod common;

use common::{run_stdin, Oxide};

#[test]
fn a_block_can_span_lines() {
    run_stdin(
        "script-block",
        "if 1 {\n    echo yes\n} else {\n    echo no\n}\necho after\n",
    )
    .assert_ok("yes\nafter\n");
}

#[test]
fn a_pipe_at_the_end_of_a_line_continues_it() {
    run_stdin("script-pipe", "echo abc |\n    grep b\n").assert_ok("abc\n");
}

#[test]
fn a_quoted_string_can_span_lines() {
    run_stdin("script-quote", "echo 'first\nsecond'\n").assert_ok("first\nsecond\n");
}

#[test]
fn a_backslash_joins_lines() {
    run_stdin("script-backslash", "echo one \\\n    two\n").assert_ok("one two\n");
}

#[test]
fn comments_are_skipped() {
    run_stdin(
        "script-comments",
        "# a whole-line comment\necho shown # a trailing comment\n",
    )
    .assert_ok("shown\n");
}

#[test]
fn exit_inside_a_multi_line_block_stops_the_script() {
    run_stdin(
        "script-exit",
        "if 1 {\n    echo before\n    exit 4\n}\necho after\n",
    )
    .assert("before\n", "", 4);
}

#[test]
fn a_script_file_runs_the_same_way_and_leaves_history_alone() {
    let oxide = Oxide::new("script-file")
        .file("blocks.ox", "for x in a b {\n    echo $x\n}\n")
        .arg("blocks.ox");
    oxide.run().assert_ok("a\nb\n");
    assert!(!oxide.dir().join(".oxide_history").exists());
}
