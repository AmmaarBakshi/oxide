//! Prompt rendering.
//!
//! Returns the *plain-text* prompt. Coloring is applied separately by the
//! highlighter (`highlight::colorize_prompt`) so rustyline can still measure
//! the prompt's visible width and place the cursor correctly.

/// Build the interactive prompt for the given working directory, e.g.
/// `oxide C:\Users\me > `.
pub fn render(cwd: &str) -> String {
    format!("oxide {cwd} > ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_cwd() {
        assert_eq!(render("C:\\tmp"), "oxide C:\\tmp > ");
    }
}
