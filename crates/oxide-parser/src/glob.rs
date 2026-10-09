use glob::glob;

/// The paths matching `pattern`, in sorted order; empty when nothing
/// matches or the pattern is malformed.
///
/// On Windows the pattern's `/` separators become `\` for matching, and the
/// results use `/` again, so scripts can write paths the same way everywhere.
pub fn matches(pattern: &str) -> Vec<String> {
    let normalized = if cfg!(windows) {
        pattern.replace('/', "\\")
    } else {
        pattern.to_string()
    };

    let Ok(paths) = glob(&normalized) else {
        return Vec::new();
    };
    paths
        .flatten()
        .map(|path| {
            let path = path.display().to_string();
            if cfg!(windows) {
                path.replace('\\', "/")
            } else {
                path
            }
        })
        .collect()
}
