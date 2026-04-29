use std::process::ChildStdout;
use crate::process;

pub struct OsPipeline {
    /// Holds the output stream from the previous command in the chain
    previous_stdout: Option<ChildStdout>,
}

impl OsPipeline {
    pub fn new() -> Self {
        Self { previous_stdout: None }
    }

    /// Executes a single OS command in the pipeline chain and links its streams
    pub fn execute_node(
        &mut self,
        program: &str,
        args: &[String],
        is_last: bool,
        outfile: &Option<String>,
    ) -> Result<Option<i32>, String> {
        
        // Use our process wrapper to spawn the command with the correct pipes
        match process::spawn_piped(program, args, self.previous_stdout.take(), is_last, outfile) {
            Ok(mut child) => {
                if !is_last {
                    // If it's NOT the last command, capture its output for the next one
                    self.previous_stdout = child.stdout.take();
                    Ok(None) // Return None to signal "Keep going!"
                } else {
                    // If it IS the last command, wait for it to finish and return the exit code
                    let status = child.wait().map_err(|_| "oxide: pipeline failed to wait".to_string())?;
                    Ok(Some(status.code().unwrap_or(1))) 
                }
            }
            Err(e) => Err(e),
        }
    }
}