//! Background jobs: what `cmd &` started, kept until it is reaped.

use std::process::Child;

pub struct Job {
    pub id: usize,
    /// The command line, as `jobs` shows it.
    pub name: String,
    /// One process per pipeline stage.
    pub children: Vec<Child>,
}

pub struct JobManager {
    pub jobs: Vec<Job>,
    next_id: usize,
    /// `$!`: the process id of the last job started, or of its last stage.
    pub last_pid: Option<u32>,
    /// Set for an interactive session. Only then are jobs announced as
    /// they start and finish, as other shells do.
    pub interactive: bool,
}

impl Default for JobManager {
    fn default() -> Self {
        Self::new()
    }
}

impl JobManager {
    pub fn new() -> Self {
        Self {
            jobs: Vec::new(),
            next_id: 1,
            last_pid: None,
            interactive: false,
        }
    }

    /// Records a job started in the background and returns its number.
    pub fn add(&mut self, name: String, children: Vec<Child>) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        self.last_pid = children.last().map(Child::id);
        if self.interactive {
            // The classic notice: `[job number] process id`.
            if let Some(pid) = self.last_pid {
                eprintln!("[{}] {}", id, pid);
            }
        }
        self.jobs.push(Job { id, name, children });
        id
    }

    /// Checks if any background jobs have finished and cleans them up
    pub fn check_completed(&mut self) {
        let interactive = self.interactive;
        self.jobs.retain_mut(|job| {
            // try_wait() checks each process WITHOUT freezing the shell!
            let finished = job
                .children
                .iter_mut()
                .all(|child| !matches!(child.try_wait(), Ok(None)));
            if finished && interactive {
                println!("\n[+] Job {} completed: {}", job.id, job.name);
            }
            !finished
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
