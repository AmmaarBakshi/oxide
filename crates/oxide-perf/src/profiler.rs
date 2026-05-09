use std::time::Instant;

pub struct Profiler {
    start_time: Instant,
}

impl Profiler {
    pub fn start() -> Self {
        Self {
            start_time: Instant::now(),
        }
    }

    pub fn stop(&self, operation: &str) {
        let duration = self.start_time.elapsed();
        // You can conditionally print this based on an environment variable later
        println!("[PERF] {} took {:?}", operation, duration);
    }
}