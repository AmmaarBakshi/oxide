use glob::glob;

pub fn expand_glob(pattern: &str) -> Vec<String> {
    let mut matches = Vec::new();

    let normalized_pattern = if cfg!(windows) {
        pattern.replace('/', "\\")
    } else {
        pattern.to_string()
    };

    if normalized_pattern.contains('*') || normalized_pattern.contains('?') || normalized_pattern.contains('[') {
        if let Ok(paths) = glob(&normalized_pattern) {
            for entry in paths.flatten() {
                matches.push(entry.display().to_string().replace('\\', "/"));
            }
        }
    }

    if matches.is_empty() {
        matches.push(pattern.to_string()); // Return the original string if no * or ? matched
    }
    matches
}