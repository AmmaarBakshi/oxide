pub fn init() {
    // Set a global Ctrl-C handler.
    // This catches the SIGINT signal and prevents the shell process from dying.
    // Any foreground child process will still be killed by the OS.
    let _ = ctrlc::set_handler(move || {
        // We intentionally leave this empty!
        // Just catching the signal is enough to keep our shell alive.
        // Rustyline handles the visual "^C" at the empty prompt automatically.
    });
}