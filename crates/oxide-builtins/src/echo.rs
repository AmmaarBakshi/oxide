pub fn execute(args: &[String]) -> i32 {
    // Join all the arguments with a space and print them
    let output = args.join(" ");
    println!("{}", output);
    
    // Return 0 to indicate success (exit code 0)
    0
}