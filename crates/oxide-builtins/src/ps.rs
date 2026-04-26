use sysinfo::System;

pub fn execute(_args: &[String]) -> i32 {
    // Load up the system monitor
    let mut sys = System::new_all();
    sys.refresh_all(); // Grab the latest CPU/Memory data

    println!("{:<10} {:<30} {:<10} {}", "PID", "NAME", "CPU%", "MEMORY(MB)");
    println!("{:-<10} {:-<30} {:-<10} {:-<10}", "", "", "", "");

    for (pid, process) in sys.processes() {
        // Convert memory from bytes to Megabytes
        let mem_mb = process.memory() / 1024 / 1024;
        
        // Print formatted columns
        println!("{:<10} {:<30} {:<10.1} {}", 
            pid.to_string(), 
            process.name().to_string_lossy(), // <-- ADD .to_string_lossy() HERE
            process.cpu_usage(), 
            mem_mb
        );
    }
    0
}