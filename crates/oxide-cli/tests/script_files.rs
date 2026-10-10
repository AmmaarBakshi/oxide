//! Whole scripts run from files, the way they are meant to be used: a
//! shebang line, comments, and functions, loops and `case` working together.
//! Each `tests/ox/NAME.ox` must print exactly `tests/ox/NAME.out`.

mod common;

use common::Oxide;

#[track_caller]
fn check(name: &str, script: &str, expected: &str) {
    // A Windows checkout may have turned the newlines into CRLF.
    let script = script.replace("\r\n", "\n");
    let expected = expected.replace("\r\n", "\n");
    let file = format!("{}.ox", name);
    Oxide::new(&format!("ox-{}", name))
        .file(&file, &script)
        .arg(&file)
        .run()
        .assert_ok(&expected);
}

macro_rules! script_test {
    ($name:ident) => {
        #[test]
        fn $name() {
            check(
                stringify!($name),
                include_str!(concat!("ox/", stringify!($name), ".ox")),
                include_str!(concat!("ox/", stringify!($name), ".out")),
            );
        }
    };
}

script_test!(functions);
script_test!(loops);
script_test!(case);
