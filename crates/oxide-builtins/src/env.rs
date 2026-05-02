pub fn execute() -> i32 {
    for (key, value) in std::env::vars() {
        println!("{}={}", key, value);
    }
    0
}