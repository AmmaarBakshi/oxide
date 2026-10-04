use sysinfo::System;

use crate::{Ctx, Io};

pub fn run(_args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    // Load up the system monitor
    let mut sys = System::new_all();
    sys.refresh_all(); // Grab the latest CPU/Memory data

    wln!(io, "{:<10} {:<30} {:<10} {}", "PID", "NAME", "CPU%", "MEMORY(MB)");
    wln!(io, "{:-<10} {:-<30} {:-<10} {:-<10}", "", "", "", "");

    for (pid, process) in sys.processes() {
        // Convert memory from bytes to Megabytes
        let mem_mb = process.memory() / 1024 / 1024;

        wln!(
            io,
            "{:<10} {:<30} {:<10.1} {}",
            pid.to_string(),
            process.name().to_string_lossy(),
            process.cpu_usage(),
            mem_mb
        );
    }
    0
}
