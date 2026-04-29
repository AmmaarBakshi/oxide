use std::env;

/// Expands $VARIABLES and ~ (Home Directory)
pub fn expand_text(input: &str) -> String {
    let mut result = String::new();
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '$' {
            let mut var_name = String::new();
            // Read the variable name until we hit a space or symbol
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