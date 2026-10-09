//! Redirecting stdout and stderr, for builtins and external programs alike.

mod common;

use common::{run_c, Oxide};

#[test]
fn a_builtin_s_stderr_goes_to_the_file() {
    let oxide = Oxide::new("redir-builtin-err").args(&["-c", "cat missing.txt 2> err.txt"]);
    let out = oxide.run();
    out.assert("", "", 1);
    let err = std::fs::read_to_string(oxide.dir().join("err.txt")).unwrap();
    assert!(err.starts_with("cat: missing.txt"), "{}", err);
}

#[test]
fn an_external_program_s_stderr_goes_to_the_file() {
    let oxide = Oxide::new("redir-external-err")
        .args(&["-c", "\"$OXIDE_BIN\" -c 'cat missing.txt' 2> err.txt"]);
    oxide.run().assert("", "", 1);
    let err = std::fs::read_to_string(oxide.dir().join("err.txt")).unwrap();
    assert!(err.contains("missing.txt"), "{}", err);
}

#[test]
fn two_arrows_append_to_the_file() {
    let oxide = Oxide::new("redir-err-append")
        .args(&["-c", "cat one.txt 2> err.txt; cat two.txt 2>> err.txt"]);
    oxide.run();
    let err = std::fs::read_to_string(oxide.dir().join("err.txt")).unwrap();
    assert_eq!(err.lines().count(), 2, "{}", err);
}

#[test]
fn dev_null_discards_stderr_everywhere() {
    run_c("redir-dev-null", "cat missing.txt 2>/dev/null; echo done").assert_ok("done\n");
}

#[test]
fn command_not_found_follows_the_stderr_redirect() {
    run_c(
        "redir-not-found",
        "no-such-program-oxide 2>/dev/null; echo $?",
    )
    .assert_ok("127\n");
}
