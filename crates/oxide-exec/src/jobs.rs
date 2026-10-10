//! Background jobs: what `cmd &` started, kept until it is reaped.

use std::fmt;
use std::process::Child;

/// Where a job is in its life.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JobState {
    Running,
    /// Suspended by Ctrl+Z or a stop signal, until `fg` or `bg` resumes it.
    Stopped,
    /// Every process has exited. The status is the last stage's.
    Done(i32),
}

impl fmt::Display for JobState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            JobState::Running => f.pad("Running"),
            JobState::Stopped => f.pad("Stopped"),
            JobState::Done(0) => f.pad("Done"),
            JobState::Done(code) => f.pad(&format!("Exit {}", code)),
        }
    }
}

/// One process of a job; a pipeline has one per stage.
pub struct Process {
    pub child: Child,
    /// The exit status, once the process has been reaped.
    pub status: Option<i32>,
    /// Whether it was last seen stopped.
    pub stopped: bool,
}

impl Process {
    fn new(child: Child) -> Self {
        Self {
            child,
            status: None,
            stopped: false,
        }
    }

    /// Takes note of an exit that has already happened, without blocking.
    fn poll(&mut self) {
        if self.status.is_none() {
            if let Ok(Some(status)) = self.child.try_wait() {
                self.status = Some(crate::process::status_code(status));
            }
        }
    }
}

pub struct Job {
    pub id: usize,
    /// The command line as it was typed, which `jobs` shows.
    pub name: String,
    pub processes: Vec<Process>,
}

impl Job {
    pub fn state(&self) -> JobState {
        if self.processes.iter().all(|p| p.status.is_some()) {
            let last = self.processes.last().and_then(|p| p.status);
            JobState::Done(last.unwrap_or(0))
        } else if self
            .processes
            .iter()
            .all(|p| p.status.is_some() || p.stopped)
        {
            JobState::Stopped
        } else {
            JobState::Running
        }
    }
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
        let processes = children.into_iter().map(Process::new).collect();
        self.jobs.push(Job {
            id,
            name,
            processes,
        });
        id
    }

    /// Brings every job's state up to date, without blocking.
    pub fn refresh(&mut self) {
        for job in &mut self.jobs {
            job.processes.iter_mut().for_each(Process::poll);
        }
    }

    /// Takes the jobs that have finished out of the table.
    fn remove_done(&mut self) -> Vec<Job> {
        let (done, rest) = std::mem::take(&mut self.jobs)
            .into_iter()
            .partition(|job| matches!(job.state(), JobState::Done(_)));
        self.jobs = rest;
        done
    }

    /// Called before each command. In an interactive session, finished
    /// jobs are reported and forgotten; elsewhere they wait for `jobs`.
    pub fn check_completed(&mut self) {
        self.refresh();
        if self.interactive {
            for job in self.remove_done() {
                println!("[{}] {}\t{}", job.id, job.state(), job.name);
            }
        }
    }

    /// `jobs`: lists every job with its state. A finished job is listed
    /// one last time, then forgotten.
    pub fn print_jobs(&mut self) {
        self.refresh();
        for job in &self.jobs {
            println!("[{}] {}\t{}", job.id, job.state(), job.name);
        }
        self.remove_done();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn states_read_like_other_shells() {
        assert_eq!(JobState::Running.to_string(), "Running");
        assert_eq!(JobState::Stopped.to_string(), "Stopped");
        assert_eq!(JobState::Done(0).to_string(), "Done");
        assert_eq!(JobState::Done(2).to_string(), "Exit 2");
        assert_eq!(format!("{:<8}|", JobState::Done(0)), "Done    |");
    }
}
