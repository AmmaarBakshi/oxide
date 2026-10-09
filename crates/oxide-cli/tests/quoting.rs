//! Quoting, escaping, and how words are split and globbed.

mod common;

use common::{run_c, Oxide};

#[test]
fn quotes_keep_spacing() {
    run_c("quote-spacing", "echo 'a  b' \"c  d\"").assert_ok("a  b c  d\n");
}

#[test]
fn single_quotes_do_not_expand() {
    run_c("quote-single", "echo '$HOME'").assert_ok("$HOME\n");
}

#[test]
fn a_quoted_value_stays_one_word() {
    run_c(
        "quote-assign",
        "export GREETING=\"hello   world\"; echo \"$GREETING\"",
    )
    .assert_ok("hello   world\n");
}

#[test]
fn unquoted_expansions_are_split_into_words() {
    let out = Oxide::new("quote-split")
        .env("OXIDE_LIST", "a   b c")
        .args(&["-c", "for x in $OXIDE_LIST { echo [$x] }"])
        .run();
    out.assert_ok("[a]\n[b]\n[c]\n");
}

#[test]
fn quoted_and_unquoted_parts_join() {
    run_c("quote-join", "echo pre\"mid dle\"'post'").assert_ok("premid dlepost\n");
}

#[test]
fn backslash_escapes_a_space() {
    let oxide =
        Oxide::new("quote-backslash").stdin("echo hi > two\\ words.txt\ncat 'two words.txt'\n");
    oxide.run().assert_ok("hi\n");
}

#[test]
fn quoted_redirect_targets_keep_their_spaces() {
    run_c(
        "quote-redirect",
        "echo saved > \"out file.txt\"; cat 'out file.txt'",
    )
    .assert_ok("saved\n");
}

#[test]
fn globs_match_unless_quoted() {
    let out = Oxide::new("quote-glob")
        .file("a.rs", "")
        .file("b.rs", "")
        .args(&["-c", "echo *.rs; echo \"*.rs\" '*.rs'"])
        .run();
    out.assert_ok("a.rs b.rs\n*.rs *.rs\n");
}

#[test]
fn empty_quotes_are_an_argument_but_unset_variables_are_not() {
    run_c("quote-empty", "echo [\"\"] $OXIDE_UNSET_VAR end").assert_ok("[] end\n");
}
