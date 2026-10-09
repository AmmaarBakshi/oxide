//! Variables: special parameters, assignments and the environment.

mod common;

use common::run_c;

#[test]
fn dollar_dollar_is_the_shell_pid() {
    let out = run_c("var-pid", "echo $$; echo \"$$\" '$$'");
    assert_eq!(out.stdout, format!("{0}\n{0} $$\n", out.pid), "{:?}", out);
}
