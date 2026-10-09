//! Here-documents (`<<EOF`) and here-strings (`<<<`).

mod common;

use common::{run_stdin, Oxide};

#[test]
fn a_heredoc_feeds_a_builtin() {
    run_stdin(
        "heredoc-builtin",
        "NAME=oxide\ncat <<EOF\nhello $NAME\n  \"quotes\" 'stay'\nEOF\necho after\n",
    )
    .assert_ok("hello oxide\n  \"quotes\" 'stay'\nafter\n");
}

#[test]
fn a_quoted_delimiter_keeps_the_body_literal() {
    run_stdin("heredoc-literal", "cat <<'EOF'\n$HOME $(echo no)\nEOF\n")
        .assert_ok("$HOME $(echo no)\n");
}

#[test]
fn dash_strips_tabs_so_the_body_can_be_indented() {
    run_stdin(
        "heredoc-dash",
        "if 1 {\n\tcat <<-END\n\t\tindented\n\tEND\n}\n",
    )
    .assert_ok("indented\n");
}

#[test]
fn substitutions_in_the_body_run() {
    run_stdin("heredoc-subst", "cat <<EOF\n[$(echo inner)]\nEOF\n").assert_ok("[inner]\n");
}

#[test]
fn a_heredoc_feeds_a_pipeline() {
    run_stdin("heredoc-pipe", "cat <<EOF | grep b\na\nb\nc\nEOF\n").assert_ok("b\n");
}

#[test]
fn a_heredoc_feeds_an_external_program() {
    let out = Oxide::new("heredoc-external")
        .stdin("\"$OXIDE_BIN\" <<EOF\necho from the heredoc\nEOF\n")
        .run();
    out.assert_ok("from the heredoc\n");
}

#[test]
fn a_here_string_feeds_one_line() {
    run_stdin(
        "herestring",
        "X=world\ncat <<< \"hello $X\"\ngrep b <<< 'a b c'\n",
    )
    .assert_ok("hello world\na b c\n");
}

#[test]
fn a_here_string_feeds_an_external_program() {
    Oxide::new("herestring-external")
        .stdin("\"$OXIDE_BIN\" <<< 'echo via here-string'\n")
        .run()
        .assert_ok("via here-string\n");
}
