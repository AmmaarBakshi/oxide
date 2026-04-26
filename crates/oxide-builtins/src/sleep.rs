use std::thread;
use std::time::Duration;

pub fn execute(args: &[String]) -> i32 {
    if args.is_empty() {
        eprintln!("oxide: sleep: missing operand (e.g., 'sleep 5')");
        return 1;
    }

    if let Ok(secs) = args[0].parse::<u64>() {
        thread::sleep(Duration::from_secs(secs));
        0
    } else {
        eprintln!("oxide: sleep: invalid time interval '{}'", args[0]);
        1
    }
}