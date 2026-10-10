//! Background jobs: `&`, `$!`, and the builtins that manage jobs.
//!
//! The jobs are copies of the shell itself (`$OXIDE_BIN -c ...`), so the
//! tests need no outside programs on any platform.

mod common;

use common::run_c;

#[test]
fn an_ampersand_runs_the_command_without_waiting() {
    // The job writes its file only after a second; by then the shell has
    // long since looked for it.
    run_c(
        "bg-no-wait",
        "\"$OXIDE_BIN\" -c 'sleep 1; echo late > late.txt' & cat late.txt 2>/dev/null || echo not yet",
    )
    .assert_ok("not yet\n");
}

#[test]
fn dollar_bang_is_the_last_background_pid() {
    run_c(
        "bg-pid",
        "echo [$!]; \"$OXIDE_BIN\" -c 'exit 0' & [ -n \"$!\" ] && echo set",
    )
    .assert_ok("[]\nset\n");
}

#[test]
fn the_ampersand_ends_the_statement() {
    run_c("bg-separator", "echo first & echo second").assert_ok("first\nsecond\n");
}

#[test]
fn jobs_lists_each_job_with_its_state() {
    run_c(
        "jobs-state",
        "\"$OXIDE_BIN\" -c 'exit 3' & \"$OXIDE_BIN\" -c 'sleep 3' >/dev/null 2>&1 & sleep 1; jobs; echo --; jobs",
    )
    .assert_ok(
        "[1]-  Exit 3                  \"$OXIDE_BIN\" -c 'exit 3'\n\
         [2]+  Running                 \"$OXIDE_BIN\" -c 'sleep 3' &\n\
         --\n\
         [2]+  Running                 \"$OXIDE_BIN\" -c 'sleep 3' &\n",
    );
}

#[test]
fn jobs_dash_p_and_dash_l_show_the_process_id() {
    let out = run_c(
        "jobs-pids",
        "\"$OXIDE_BIN\" -c 'sleep 2' >/dev/null 2>&1 & jobs -p; echo $!; jobs -l",
    );
    let lines: Vec<&str> = out.stdout.lines().collect();
    assert_eq!(lines.len(), 3, "{:?}", out);
    assert_eq!(lines[0], lines[1]);
    assert!(
        lines[2].starts_with(&format!("[1]+ {} Running ", lines[0])),
        "{:?}",
        out
    );
}

#[test]
fn jobs_rejects_unknown_options() {
    run_c("jobs-bad", "jobs -x").assert(
        "",
        "oxide: jobs: -x: invalid option\njobs: usage: jobs [-l | -p]\n",
        2,
    );
}

#[test]
fn wait_blocks_until_every_job_is_done() {
    run_c(
        "wait-all",
        "\"$OXIDE_BIN\" -c 'sleep 1; echo one > one.txt' & \"$OXIDE_BIN\" -c 'echo two > two.txt' & wait; echo $?; cat one.txt two.txt; jobs",
    )
    .assert_ok("0\none\ntwo\n");
}

#[test]
fn wait_for_one_job_gives_its_status() {
    run_c(
        "wait-one",
        "\"$OXIDE_BIN\" -c 'exit 4' & pid=$!; \"$OXIDE_BIN\" -c 'exit 5' & wait %2; echo $?; wait $pid; echo $?",
    )
    .assert_ok("5\n4\n");
}

#[test]
fn wait_waits_for_a_whole_background_pipeline() {
    run_c(
        "wait-pipeline",
        "\"$OXIDE_BIN\" -c 'echo piped' | \"$OXIDE_BIN\" -c 'read x; echo got $x > out.txt' & wait %1; cat out.txt",
    )
    .assert_ok("got piped\n");
}

#[test]
fn wait_reports_unknown_jobs() {
    run_c("wait-unknown", "wait %3; echo $?; wait 1").assert(
        "127\n",
        "oxide: wait: %3: no such job\noxide: wait: pid 1 is not a child of this shell\n",
        127,
    );
}
