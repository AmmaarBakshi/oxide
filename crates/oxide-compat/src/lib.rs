pub mod bash_mode;
pub mod posix_mode;
pub mod migration;

#[derive(Debug, Clone, PartialEq)]
pub enum CompatMode {
    Oxide, // Your native, default mode
    Posix, // Strict, old-school 'sh' mode
    Bash,  // Bash emulation mode
}

impl Default for CompatMode {
    fn default() -> Self {
        CompatMode::Oxide
    }
}