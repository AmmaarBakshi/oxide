use oxide_core::shell::Shell;
use std::env;

fn main() -> anyhow::Result<()> {
    let mut shell = Shell::new();
    let mut history = oxide_builtins::history::load();
    // Grab command line arguments passed to our shell program
    let args: Vec<String> = env::args().collect();

    // If they typed `oxide my_script.sh`, run the script!
    // (args[0] is the oxide executable itself, args[1] is the script name)
    if args.len() > 1 {
        let script_path = &args[1];
        shell.run_script(script_path)?;
    } else {
        // Otherwise, boot into the interactive terminal
        shell.run_repl()?;
    }
    loop {
        let input = prompt(); // however you get input
        if input.trim().is_empty() { continue; }
        
        // 1. Save to disk
        oxide_builtins::history::append(&input);
        // 2. Add to session memory
        history.push(input.clone());
        
        // 3. Execute...
        executor.execute_line(&input, ..., &history);
    }

    Ok(())
}