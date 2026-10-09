//! Builtins that need the whole shell around them to test: stdin from the
//! terminal side, variables, and the rest of the line.

mod common;

use common::{run_c, Oxide};

#[test]
fn read_takes_a_line_from_stdin() {
    Oxide::new("read-stdin")
        .args(&["-c", "read first rest; echo [$first] [$rest]"])
        .stdin("one two three\nignored\n")
        .run()
        .assert_ok("[one] [two three]\n");
}

#[test]
fn read_at_the_end_of_a_pipeline_sets_variables_here() {
    run_c("read-pipe", "echo x y | read p q; echo $p-$q").assert_ok("x-y\n");
}

#[test]
fn read_fails_at_end_of_input() {
    Oxide::new("read-eof")
        .args(&["-c", "read line; echo $?"])
        .run()
        .assert_ok("1\n");
}

#[test]
fn printf_formats_a_table() {
    run_c(
        "printf-table",
        "for n in 1 10 { printf '%-4s|%3d\\n' item$n $n }",
    )
    .assert_ok("item1|  1\nitem10| 10\n");
}
