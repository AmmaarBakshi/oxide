use anyhow::Result;
use oxide_core::shell::Shell;
use std::env;

fn main() -> Result<()> {
    let mut shell = Shell::new();
    let args: Vec<String> = env::args().collect();

    if args.len() > 1 {
        shell.run_script(&args[1])?;
    } else {
        shell.run_repl()?;
    }

    Ok(())
}