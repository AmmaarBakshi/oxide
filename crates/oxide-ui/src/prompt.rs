//! Prompt rendering.
//!
//! Returns the *plain-text* prompt from a template. Coloring is applied
//! separately by the highlighter (`highlight::colorize_prompt`) so rustyline
//! can still measure the prompt's visible width and place the cursor correctly.

/// Render a prompt template for the given working directory.
///
/// Supported placeholders:
/// - `{cwd}`  — the current working directory;
/// - `{user}` — the current username (`$USERNAME` / `$USER`);
/// - `{host}` — the machine hostname (`$COMPUTERNAME` / `$HOSTNAME`).
pub fn render(template: &str, cwd: &str) -> String {
    template
        .replace("{cwd}", cwd)
        .replace("{user}", &user())
        .replace("{host}", &host())
}

fn user() -> String {
    std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_default()
}

fn host() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitutes_cwd() {
        assert_eq!(render("oxide {cwd} > ", "C:\\tmp"), "oxide C:\\tmp > ");
    }

    #[test]
    fn leaves_unknown_placeholders_untouched() {
        assert_eq!(render("{cwd} {bogus}", "x"), "x {bogus}");
    }

    #[test]
    fn template_without_placeholders_is_verbatim() {
        assert_eq!(render("$ ", "ignored"), "$ ");
    }
}
