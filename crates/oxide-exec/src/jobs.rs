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

    /// Blocks until the process exits, unless it already has.
    fn wait(&mut self) {
        if self.status.is_none() {
            self.status = Some(match self.child.wait() {
                Ok(status) => crate::process::status_code(status),
                Err(_) => 127,
            });
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
    /// The id of the job's first process, which leads its process group.
    pub fn pid(&self) -> Option<u32> {
        self.processes.first().map(|p| p.child.id())
    }

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

    /// Finds the job `spec` names: `%N` for job N, `%%` or `%+` for the
    /// current job, `%-` for the previous one, or a bare process id.
    pub fn find(&self, spec: &str) -> Option<usize> {
        let count = self.jobs.len();
        match spec {
            "%" | "%%" | "%+" => count.checked_sub(1),
            "%-" => count.checked_sub(2),
            _ => match spec.strip_prefix('%') {
                Some(id) => {
                    let id: usize = id.parse().ok()?;
                    self.jobs.iter().position(|job| job.id == id)
                }
                None => {
                    let pid: u32 = spec.parse().ok()?;
                    self.jobs
                        .iter()
                        .position(|job| job.processes.iter().any(|p| p.child.id() == pid))
                }
            },
        }
    }

    /// Blocks until every process of the job at `index` has exited, then
    /// takes the job out of the table and returns its status.
    pub fn wait_for(&mut self, index: usize) -> i32 {
        let mut job = self.jobs.remove(index);
        job.processes.iter_mut().for_each(Process::wait);
        match job.state() {
            JobState::Done(code) => code,
            _ => 0,
        }
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

    /// Called before each prompt: reports the jobs that have finished
    /// since the last one, as `[1]+  Done                    make`, and
    /// forgets them. Only an interactive session does this; a script's
    /// finished jobs wait for `jobs` to list them.
    pub fn notify(&mut self) {
        if self.interactive {
            for line in self.take_finished() {
                println!("{}", line);
            }
        }
    }

    /// The report lines for jobs that have finished, which are taken out
    /// of the table.
    fn take_finished(&mut self) -> Vec<String> {
        self.refresh();
        let lines = (0..self.jobs.len())
            .filter(|&index| matches!(self.jobs[index].state(), JobState::Done(_)))
            .map(|index| self.line(index, false))
            .collect();
        self.remove_done();
        lines
    }

    /// `jobs [-l | -p]`: lists every job with its state, `-l` adding its
    /// process id and `-p` showing only that. A finished job is listed one
    /// last time, then forgotten.
    pub fn print_jobs(&mut self, args: &[String]) -> i32 {
        let (mut long, mut pids) = (false, false);
        for arg in args {
            match arg.as_str() {
                "-l" => long = true,
                "-p" => pids = true,
                _ => {
                    eprintln!("oxide: jobs: {}: invalid option", arg);
                    eprintln!("jobs: usage: jobs [-l | -p]");
                    return 2;
                }
            }
        }
        self.refresh();
        for (index, job) in self.jobs.iter().enumerate() {
            if pids {
                if let Some(pid) = job.pid() {
                    println!("{}", pid);
                }
            } else {
                println!("{}", self.line(index, long));
            }
        }
        self.remove_done();
        0
    }

    /// `+` for the current job, the newest, which `%+` and `%%` name; `-`
    /// for the one before it, `%-`.
    fn marker(&self, index: usize) -> char {
        match self.jobs.len() - index {
            1 => '+',
            2 => '-',
            _ => ' ',
        }
    }

    /// A job as `jobs` and the notices show it, in the layout other shells
    /// use: `[1]+  Running                 sleep 10 &`. With `long`, the
    /// process id comes after the marker.
    fn line(&self, index: usize, long: bool) -> String {
        let job = &self.jobs[index];
        let state = job.state();
        let pid = match job.pid() {
            Some(pid) if long => format!(" {}", pid),
            _ => " ".to_string(),
        };
        let ampersand = if state == JobState::Running { " &" } else { "" };
        format!(
            "[{}]{}{} {:<24}{}{}",
            job.id,
            self.marker(index),
            pid,
            state,
            job.name,
            ampersand
        )
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

    #[test]
    fn job_specs_name_jobs_by_number_or_position() {
        let mut jobs = JobManager::new();
        for id in [1, 2, 4] {
            jobs.jobs.push(Job {
                id,
                name: String::new(),
                processes: Vec::new(),
            });
        }
        assert_eq!(jobs.find("%4"), Some(2));
        assert_eq!(jobs.find("%1"), Some(0));
        assert_eq!(jobs.find("%3"), None);
        assert_eq!(jobs.find("%%"), Some(2));
        assert_eq!(jobs.find("%+"), Some(2));
        assert_eq!(jobs.find("%-"), Some(1));
        assert_eq!(jobs.find("%x"), None);
        assert_eq!(jobs.find("123"), None);
    }

    /// A child that exits with `code` straight away.
    fn exiting(code: i32) -> Child {
        let script = format!("exit {}", code);
        #[cfg(windows)]
        let mut command = std::process::Command::new("cmd");
        #[cfg(windows)]
        command.args(["/C", &script]);
        #[cfg(not(windows))]
        let mut command = std::process::Command::new("sh");
        #[cfg(not(windows))]
        command.args(["-c", &script]);
        command.spawn().expect("spawn a child")
    }

    #[test]
    fn finished_jobs_are_reported_once() {
        let mut jobs = JobManager::new();
        jobs.add("quick".to_string(), vec![exiting(3)]);
        jobs.add("ok".to_string(), vec![exiting(0)]);

        // Let both exit before looking, so the markers don't depend on
        // which finished first.
        for _ in 0..500 {
            jobs.refresh();
            if jobs
                .jobs
                .iter()
                .all(|job| matches!(job.state(), JobState::Done(_)))
            {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(
            jobs.take_finished(),
            [
                format!("[1]-  {:<24}quick", "Exit 3"),
                format!("[2]+  {:<24}ok", "Done"),
            ]
        );
        assert!(jobs.take_finished().is_empty());
        assert!(jobs.jobs.is_empty());
    }

    #[test]
    fn the_newest_job_is_current_and_the_one_before_previous() {
        let mut jobs = JobManager::new();
        for name in ["a", "b", "c"] {
            jobs.jobs.push(Job {
                id: jobs.jobs.len() + 1,
                name: name.to_string(),
                processes: Vec::new(),
            });
        }
        let markers: Vec<char> = (0..3).map(|i| jobs.marker(i)).collect();
        assert_eq!(markers, [' ', '-', '+']);
        // With no processes left, a job counts as done.
        assert_eq!(jobs.line(2, false), format!("[3]+  {:<24}c", "Done"));
    }
}
