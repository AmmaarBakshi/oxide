pub mod bash_mode;
pub mod migration;
pub mod posix_mode;

#[derive(Debug, Clone, Default, PartialEq)]
pub enum CompatMode {
    #[default]
    Oxide, // Your native, default mode
    Posix, // Strict, old-school 'sh' mode
    Bash,  // Bash emulation mode
}
