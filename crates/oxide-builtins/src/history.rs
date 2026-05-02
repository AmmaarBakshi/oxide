// This assumes you pass a reference to your history list from the CLI crate
pub fn execute(history: &[String]) -> i32 {
    for (i, cmd) in history.iter().enumerate() {
        println!("{:>5}  {}", i + 1, cmd);
    }
    0
}