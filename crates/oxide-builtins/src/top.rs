use sysinfo::System;

pub fn execute(_args: &[String]) -> i32 {
    let mut sys = System::new_all();
    
    // We have to wait a tiny fraction of a second to get accurate CPU readings
    std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
    sys.refresh_all(); 

    // Convert the map of processes into a vector so we can sort them
    let mut procs: Vec<_> = sys.processes().iter().collect();

    // Sort by highest CPU usage first
    procs.sort_by(|a, b| b.1.cpu_usage().partial_cmp(&a.1.cpu_usage()).unwrap_or(std::cmp::Ordering::Equal));

    println!("=== TOP PROCESSES (CPU USAGE) ===");
    println!("{:<10} {:<30} {:<10} {}", "PID", "NAME", "CPU%", "MEMORY(MB)");
    println!("{:-<10} {:-<30} {:-<10} {:-<10}", "", "", "", "");

    // Print only the top 15 processes
    for (pid, process) in procs.into_iter().take(15) {
        let mem_mb = process.memory() / 1024 / 1024;
        println!("{:<10} {:<30} {:<10.1} {}", 
            pid.to_string(), 
            process.name().to_string_lossy(), // <-- ADD .to_string_lossy() HERE
            process.cpu_usage(), 
            mem_mb
        );
    }
    0
}