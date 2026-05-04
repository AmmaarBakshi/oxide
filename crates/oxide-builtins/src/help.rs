pub fn execute(args: &[String]) -> i32 {
    if args.is_empty() {
        println!("Available built-in commands:");
        
        // One string, split by space, printed line by line
        "alias cat cd clear echo env export find grep help history jail kill ls mode open pwd ps rm sleep source top touch unset"
            .split_whitespace()
            .for_each(|cmd| println!("{}", cmd));
            
        return 0;
    }

    println!("oxide: help: no detailed help available for '{}'.", args.join(" "));
    0
}