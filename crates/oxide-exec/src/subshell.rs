use oxide_compat::CompatMode;
use std::collections::HashMap;
use std::env;

pub fn execute(
    inner_commands: &str,
    mode: &mut CompatMode,
    aliases: &mut HashMap<String, String>,
    job_manager: &mut crate::jobs::JobManager,
    history: &[String],
    vars: &HashMap<String, String>,
) -> i32 {
    // 1. Snapshot the current directory before we do anything
    let original_dir = env::current_dir().expect("oxide: failed to get current directory");

    // 2. Clone the aliases so any new aliases created inside the subshell are destroyed afterward
    let mut subshell_aliases = aliases.clone();
    let mut subshell_exit_code = 0;

    // 3. Boot up a completely fresh execution engine for the sandbox
    let mut sandbox_executor = crate::executor::Executor::new();
    // It starts with a copy of the shell's variables; changes stay inside.
    sandbox_executor.vars = vars.clone();
    // Subshells are short-lived, so give them a throwaway path cache.
    let mut subshell_cache = oxide_perf::cache::CommandCache::new();

    // 4. Run the inner commands!
    sandbox_executor.execute_line(
        inner_commands,
        mode,
        &mut subshell_aliases,
        &mut subshell_exit_code,
        job_manager,
        history,
        &mut subshell_cache,
    );

    // 5. Restore the original directory! (This is the magic part)
    let _ = env::set_current_dir(original_dir);

    subshell_exit_code
}
