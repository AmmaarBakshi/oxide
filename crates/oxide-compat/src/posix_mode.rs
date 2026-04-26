pub fn translate(input: &str) -> String {
    // If a POSIX script tries to run ". ~/.profile", translate it to "source ~/.profile"
    if input.starts_with(". ") {
        return input.replacen(". ", "source ", 1); 
    }
    
    input.to_string()
}