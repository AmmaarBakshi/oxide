//! Brace expansion, and how it fits with the other expansions.

mod common;

use common::{run_c, Oxide};

#[test]
fn braces_multiply_out() {
    run_c("brace-basic", "echo {a,b,c} x{1,2}y").assert_ok("a b c x1y x2y\n");
}

#[test]
fn quoted_braces_are_literal() {
    run_c("brace-quoted", "echo '{a,b}' \"{c,d}\"").assert_ok("{a,b} {c,d}\n");
}

#[test]
fn braces_work_inside_blocks() {
    run_c("brace-block", "for f in {one,two} { echo $f }").assert_ok("one\ntwo\n");
}

#[test]
fn braces_name_several_files() {
    let oxide = Oxide::new("brace-files").args(&["-c", "touch note{1,2}.txt; ls note*"]);
    oxide.run().assert_ok("note1.txt\nnote2.txt\n");
}
