//! Where in a script the shell is reading, so an error can point at the
//! statement that caused it: `oxide: build.ox:3: command not found: cc`.

use std::cell::RefCell;

thread_local! {
    static HERE: RefCell<Option<(String, usize)>> = const { RefCell::new(None) };
}

/// Records that the statement starting on `line` of `file` is running.
pub fn set(file: &str, line: usize) {
    HERE.with(|here| *here.borrow_mut() = Some((file.to_string(), line)));
}

/// Forgets the location once the statement is done.
pub fn clear() {
    HERE.with(|here| *here.borrow_mut() = None);
}

/// Puts the current `file:line` into an `oxide: ...` message, when a
/// script is running. Other messages come back unchanged.
pub fn tag(message: &str) -> String {
    HERE.with(
        |here| match (&*here.borrow(), message.strip_prefix("oxide: ")) {
            (Some((file, line)), Some(rest)) => format!("oxide: {}:{}: {}", file, line, rest),
            _ => message.to_string(),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_only_while_a_location_is_set() {
        assert_eq!(tag("oxide: boom"), "oxide: boom");
        set("build.ox", 3);
        assert_eq!(tag("oxide: boom"), "oxide: build.ox:3: boom");
        assert_eq!(tag("plain text"), "plain text");
        clear();
        assert_eq!(tag("oxide: boom"), "oxide: boom");
    }
}
