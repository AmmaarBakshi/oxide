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

#[test]
fn two_to_one_sends_a_builtin_s_errors_down_the_pipe() {
    let out = run_c("redir-2to1-pipe", "cat missing.txt 2>&1 | grep missing");
    assert!(out.stdout.starts_with("cat: missing.txt"), "{:?}", out);
    assert_eq!(out.stderr, "");
}

#[test]
fn two_to_one_after_a_file_redirect_puts_both_in_the_file() {
    let oxide = Oxide::new("redir-2to1-file")
        .file("present.txt", "here\n")
        .args(&["-c", "cat present.txt missing.txt > all.txt 2>&1"]);
    oxide.run().assert("", "", 1);
    let all = std::fs::read_to_string(oxide.dir().join("all.txt")).unwrap();
    assert!(all.starts_with("here\ncat: missing.txt"), "{}", all);
}

#[test]
fn ampersand_arrow_sends_both_streams_to_a_file() {
    let oxide = Oxide::new("redir-all")
        .file("present.txt", "here\n")
        .args(&[
            "-c",
            "cat present.txt missing.txt &> all.txt; cat present.txt &>> all.txt",
        ]);
    oxide.run().assert("", "", 0);
    let all = std::fs::read_to_string(oxide.dir().join("all.txt")).unwrap();
    assert_eq!(all.lines().count(), 3, "{}", all);
    assert!(all.ends_with("here\n"), "{}", all);
}

#[test]
fn an_external_program_s_streams_join_in_a_pipe() {
    let out = run_c(
        "redir-2to1-external",
        "\"$OXIDE_BIN\" -c 'echo out; cat missing.txt' 2>&1 | cat",
    );
    assert!(out.stdout.starts_with("out\n"), "{:?}", out);
    assert!(out.stdout.contains("missing.txt"), "{:?}", out);
    assert_eq!(out.stderr, "");
}

#[test]
fn arrow_ampersand_two_sends_output_to_stderr() {
    run_c("redir-to-err", "echo oops >&2").assert("", "oops\n", 0);
    run_c("redir-to-err-ext", "\"$OXIDE_BIN\" -c 'echo oops' >&2").assert("", "oops\n", 0);
}
