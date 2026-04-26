use oxide_core::shell::Shell;

fn main() -> anyhow::Result<()> {
    // 1. Setup (Later: parse CLI args like `oxide --version`)
    
    // 2. Initialize the core engine
    let mut shell = Shell::new();
    
    // 3. Start the loop
    shell.run_repl()?;

    // 4. Exit cleanly with the last command's exit code
    std::process::exit(shell.state.last_exit_code);
}