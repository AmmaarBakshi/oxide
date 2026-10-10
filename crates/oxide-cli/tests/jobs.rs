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
        "[1] Exit 3\t\"$OXIDE_BIN\" -c 'exit 3'\n\
         [2] Running\t\"$OXIDE_BIN\" -c 'sleep 3'\n\
         --\n\
         [2] Running\t\"$OXIDE_BIN\" -c 'sleep 3'\n",
    );
}
