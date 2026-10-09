//! Stderr redirection through the executor, for builtins and external
//! programs. `rustc` stands in for an external program that writes to
//! stderr, since it is on PATH wherever these tests can build.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use oxide_compat::CompatMode;
use oxide_exec::executor::Executor;
use oxide_exec::jobs::JobManager;
use oxide_perf::cache::CommandCache;

/// Runs one line through a fresh executor and returns its exit status.
fn run(line: &str) -> i32 {
    let mut executor = Executor::new();
    let mut status = 0;
    executor.execute_line(
        line,
        &mut CompatMode::Oxide,
        &mut HashMap::new(),
        &mut status,
        &mut JobManager::new(),
        &[],
        &mut CommandCache::new(),
    );
    status
}

/// A fresh scratch directory for one test. Paths are passed to the shell
/// absolute, since tests share one working directory.
fn scratch(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("oxide-exec-{}-{}", std::process::id(), test));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// `path` single-quoted for the command line, so spaces and backslashes in
/// a temp path are taken literally.
fn q(path: &std::path::Path) -> String {
    format!("'{}'", path.display())
}

fn read(path: &std::path::Path) -> String {
    fs::read_to_string(path).unwrap_or_default()
}

#[test]
fn builtin_stderr_to_a_file() {
    let dir = scratch("builtin-err");
    let err = dir.join("err.txt");
    let status = run(&format!("cat {} 2> {}", q(&dir.join("missing")), q(&err)));
    assert_eq!(status, 1);
    assert!(read(&err).starts_with("cat: "), "{:?}", read(&err));
}

#[test]
fn builtin_stderr_appends() {
    let dir = scratch("builtin-append");
    let err = dir.join("err.txt");
    let missing = q(&dir.join("missing"));
    run(&format!(
        "cat {m} 2> {e}; cat {m} 2>> {e}",
        m = missing,
        e = q(&err)
    ));
    assert_eq!(read(&err).lines().count(), 2, "{:?}", read(&err));
}

#[test]
fn stderr_of_a_middle_stage_is_redirected_too() {
    let dir = scratch("middle-stage");
    let err = dir.join("err.txt");
    let out = dir.join("out.txt");
    run(&format!(
        "cat {} 2> {} | echo next > {}",
        q(&dir.join("missing")),
        q(&err),
        q(&out)
    ));
    assert!(read(&err).starts_with("cat: "), "{:?}", read(&err));
    assert_eq!(read(&out), "next\n");
}

#[test]
fn external_stderr_to_a_file() {
    let dir = scratch("external-err");
    let err = dir.join("err.txt");
    let status = run(&format!("rustc --oxide-no-such-flag 2> {}", q(&err)));
    assert_ne!(status, 0);
    assert!(!read(&err).is_empty());
}

#[test]
fn external_stderr_joins_stdout_in_a_file() {
    let dir = scratch("external-join");
    let all = dir.join("all.txt");
    run(&format!("rustc --oxide-no-such-flag > {} 2>&1", q(&all)));
    assert!(!read(&all).is_empty());

    let both = dir.join("both.txt");
    run(&format!("rustc --oxide-no-such-flag &> {}", q(&both)));
    assert_eq!(read(&both), read(&all));
}

#[test]
fn external_stderr_joins_stdout_down_a_pipe() {
    let dir = scratch("external-pipe");
    let out = dir.join("out.txt");
    run(&format!(
        "rustc --oxide-no-such-flag 2>&1 | cat > {}",
        q(&out)
    ));
    assert!(
        read(&out).contains("oxide-no-such-flag"),
        "{:?}",
        read(&out)
    );
}

#[test]
fn builtin_output_can_go_to_stderr() {
    let dir = scratch("builtin-to-err");
    let err = dir.join("err.txt");
    run(&format!("echo oops >&2 2> {}", q(&err)));
    assert_eq!(read(&err), "oops\n");
}

#[test]
fn dev_null_is_a_real_sink() {
    let dir = scratch("dev-null");
    let status = run(&format!(
        "cat {} 2>/dev/null > /dev/null",
        q(&dir.join("missing"))
    ));
    assert_eq!(status, 1);
    // Nothing named after /dev/null appeared anywhere relative to us.
    assert!(!PathBuf::from("dev").exists());
}
