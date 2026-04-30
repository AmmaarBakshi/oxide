use glob::glob;

pub fn expand_glob(pattern: &str) -> Vec<String> {
    let mut matches = Vec::new();

    let normalized_pattern = if cfg!(windows) {
        pattern.replace('/', "\\")
    } else {
        pattern.to_string()
    };

    // DEBUG 1: What is the engine actually seeing?
    println!("DEBUG: Glob engine received -> '{}'", normalized_pattern);

    if normalized_pattern.contains('*') || normalized_pattern.contains('?') || normalized_pattern.contains('[') {
        if let Ok(paths) = glob(&normalized_pattern) {
            for entry in paths.flatten() {
                matches.push(entry.display().to_string().replace('\\', "/"));
            }
        }
    }

    // DEBUG 2: What did the engine find on the hard drive?
    println!("DEBUG: Glob engine found -> {:?}", matches);

    if matches.is_empty() {
        matches.push(pattern.to_string());
    }

    matches
}