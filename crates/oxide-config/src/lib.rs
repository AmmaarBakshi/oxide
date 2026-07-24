//! Oxide's configuration: prompt and color theme, loaded from
//! `~/.oxiderc.toml` (or `$OXIDE_CONFIG`).
//!
//! The entry point is [`Config::load`]. Consumers (notably `oxide-ui`) read the
//! resolved [`Config`] to color the prompt and input line.

pub mod config;
pub mod rc_loader;
pub mod settings;
pub mod theme;

pub use config::{Config, PromptConfig};
pub use theme::{Theme, BUILTIN_THEMES, RESET};
