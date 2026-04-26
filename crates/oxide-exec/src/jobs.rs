use std::process::Child;

pub struct Job {
    pub id: usize,
    pub name: String,
    pub child: Child,
}

pub struct JobManager {
    pub jobs: Vec<Job>,
    next_id: usize,
}

impl JobManager {
    pub fn new() -> Self {
        Self { jobs: Vec::new(), next_id: 1 }
    }

    pub fn add(&mut self, name: String, child: Child) {
        let id = self.next_id;
        self.next_id += 1;
        // Print the classic shell background notification: [Job ID] Process ID
        println!("[{}] {}", id, child.id()); 
        self.jobs.push(Job { id, name, child });
    }

    /// Checks if any background jobs have finished and cleans them up
    pub fn check_completed(&mut self) {
        self.jobs.retain_mut(|job| {
            // try_wait() checks the process WITHOUT freezing the shell!
            match job.child.try_wait() {
                Ok(Some(status)) => {
                    println!("\n[+] Job {} completed: {} ({})", job.id, job.name, status);
                    false // Remove it from the active jobs list
                }
                Ok(None) => true, // Still running, keep it
                Err(_) => false,  // Something went wrong, remove it
            }
        });
    }

    pub fn print_jobs(&mut self) {
        self.check_completed(); // Clean up before printing
        if self.jobs.is_empty() {
            println!("No active background jobs.");
            return;
        }
        for job in &self.jobs {
            println!("[{}] Running \t\t {}", job.id, job.name);
        }
    }
}