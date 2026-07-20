use std::env;

/// Expands $VARIABLES and ~ (Home Directory)
pub fn expand_text(input: &str) -> String {
    let mut result = String::new();
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '$' {
            let mut var_name = String::new();

            if chars.peek() == Some(&'{') {
                // Braced form: ${VAR}. Consume the '{', read until the closing '}'.
                chars.next(); // consume '{'
                let mut closed = false;
                while let Some(&next_c) = chars.peek() {
                    if next_c == '}' {
                        chars.next(); // consume '}'
                        closed = true;
                        break;
                    }
                    var_name.push(chars.next().unwrap());
                }
                if closed && !var_name.is_empty() {
                    result.push_str(&env::var(&var_name).unwrap_or_default());
                } else if closed {
                    // Empty braces `${}` — emit literally, including the close.
                    result.push_str("${}");
                } else {
                    // Unterminated `${...` (no closing brace) — emit literally.
                    result.push_str("${");
                    result.push_str(&var_name);
                }
            } else {
                // Bare form: $VAR — read the name until a non-identifier char.
                while let Some(&next_c) = chars.peek() {
                    if next_c.is_alphanumeric() || next_c == '_' {
                        var_name.push(chars.next().unwrap());
                    } else {
                        break;
                    }
                }

                if !var_name.is_empty() {
                    // Replace with env var, or delete it (empty string) if it doesn't exist
                    result.push_str(&env::var(&var_name).unwrap_or_default());
                } else {
                    result.push('$');
                }
            }
        } else if c == '~' {
            // Windows uses USERPROFILE, Linux/Mac uses HOME
            let home = env::var("HOME")
                .or_else(|_| env::var("USERPROFILE"))
                .unwrap_or_else(|_| "~".to_string());
            result.push_str(&home);
        } else {
            result.push(c);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_variable_expands() {
        env::set_var("OXIDE_TEST_BARE", "world");
        assert_eq!(expand_text("hello $OXIDE_TEST_BARE"), "hello world");
    }

    #[test]
    fn braced_variable_expands_mid_word() {
        env::set_var("OXIDE_TEST_BRACE", "mid");
        assert_eq!(expand_text("pre-${OXIDE_TEST_BRACE}-post"), "pre-mid-post");
    }

    #[test]
    fn undefined_variable_becomes_empty() {
        env::remove_var("OXIDE_TEST_MISSING");
        assert_eq!(expand_text("[$OXIDE_TEST_MISSING]"), "[]");
        assert_eq!(expand_text("[${OXIDE_TEST_MISSING}]"), "[]");
    }

    #[test]
    fn lone_and_unterminated_dollar_are_literal() {
        assert_eq!(expand_text("cost is $"), "cost is $");
        assert_eq!(expand_text("a${b"), "a${b");
        assert_eq!(expand_text("${}"), "${}");
    }
}