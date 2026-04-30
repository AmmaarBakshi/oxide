use std::io::{self, Write};

/// Detects if the user triggered a heredoc (e.g., `cat << EOF`)
/// Returns the cleaned command (e.g., `cat`) and the delimiter (e.g., `EOF`).
pub fn detect(input: &str) -> Option<(String, String)> {
    if let Some(pos) = input.find("<<") {
        let cmd_part = input[..pos].trim().to_string();
        let rest = input[pos + 2..].trim();
        
        if let Some(delimiter) = rest.split_whitespace().next() {
            return Some((cmd_part, delimiter.to_string()));
        }
    }
    None
}

/// Takes over the terminal and reads lines until the delimiter is typed.
pub fn read_payload(delimiter: &str) -> String {
    let mut payload = String::new();
    let stdin = io::stdin();
    
    // We enter an infinite loop to capture multiline text
    loop {
        // The standard POSIX prompt for multiline shell inputs
        print!("> ");
        let _ = io::stdout().flush();
        
        let mut line = String::new();
        if stdin.read_line(&mut line).is_err() {
            break;
        }
        
        // If the user types EXACTLY the delimiter, we break the loop!
        if line.trim_end() == delimiter {
            break;
        }
        
        payload.push_str(&line);
    }
    
    payload
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_heredoc_simple() {
        let (cmd, delim) = detect("cat << EOF").unwrap();
        assert_eq!(cmd, "cat");
        assert_eq!(delim, "EOF");
    }

    #[test]
    fn test_detect_heredoc_complex() {
        // Even if there are spaces, it should find the first word after <<
        let (cmd, delim) = detect("grep 'test'   <<    END_OF_TEXT").unwrap();
        assert_eq!(cmd, "grep 'test'");
        assert_eq!(delim, "END_OF_TEXT");
    }

    #[test]
    fn test_no_heredoc() {
        assert!(detect("echo 'no heredoc here'").is_none());
    }
}