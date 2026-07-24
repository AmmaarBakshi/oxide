//! The top-level shell configuration and its loader.

use serde::Deserialize;

use crate::rc_loader;
use crate::theme::{RawTheme, Theme};

/// How the prompt is rendered.
#[derive(Debug, Clone)]
pub struct PromptConfig {
    /// Template string. Supported placeholders: `{cwd}`, `{user}`, `{host}`.
    pub template: String,
    /// Leading substring of the rendered prompt to paint in the accent color
    /// (e.g. `"oxide"`). Empty disables prompt accenting.
    pub accent: String,
}

impl Default for PromptConfig {
    fn default() -> Self {
        Self {
            template: "oxide {cwd} > ".to_string(),
            accent: "oxide".to_string(),
        }
    }
}

/// The resolved shell configuration.
#[derive(Debug, Clone, Default)]
pub struct Config {
    pub prompt: PromptConfig,
    pub theme: Theme,
}

impl Config {
    /// Load configuration from `$OXIDE_CONFIG` (or `~/.oxiderc.toml`), falling
    /// back to defaults when the file is absent. A malformed file is reported
    /// on stderr and defaults are used, so a typo never bricks the shell.
    pub fn load() -> Self {
        match rc_loader::read() {
            None => Config::default(),
            Some(contents) => match toml::from_str::<RawConfig>(&contents) {
                Ok(raw) => Config::from_raw(raw),
                Err(err) => {
                    eprintln!(
                        "oxide: ignoring invalid config at {}: {err}",
                        rc_loader::config_path().display()
                    );
                    Config::default()
                }
            },
        }
    }

    fn from_raw(raw: RawConfig) -> Self {
        let defaults = PromptConfig::default();
        let prompt = raw.prompt.unwrap_or_default();
        Config {
            prompt: PromptConfig {
                template: prompt.template.unwrap_or(defaults.template),
                accent: prompt.accent.unwrap_or(defaults.accent),
            },
            theme: Theme::from_raw(raw.theme.unwrap_or_default()),
        }
    }
}

/// Mirror of the on-disk TOML structure, with everything optional so a partial
/// file only overrides what it names.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawConfig {
    prompt: Option<RawPrompt>,
    theme: Option<RawTheme>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawPrompt {
    template: Option<String>,
    accent: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_toml_yields_defaults() {
        let cfg = Config::from_raw(toml::from_str("").unwrap());
        assert_eq!(cfg.prompt.template, "oxide {cwd} > ");
        assert_eq!(cfg.prompt.accent, "oxide");
        assert_eq!(cfg.theme.command, crate::theme::to_ansi("#B7410E"));
    }

    #[test]
    fn partial_prompt_override_keeps_other_defaults() {
        let raw: RawConfig = toml::from_str("[prompt]\ntemplate = \"{user}$ \"").unwrap();
        let cfg = Config::from_raw(raw);
        assert_eq!(cfg.prompt.template, "{user}$ ");
        assert_eq!(cfg.prompt.accent, "oxide"); // untouched
    }

    #[test]
    fn theme_name_and_override_resolve() {
        let raw: RawConfig =
            toml::from_str("[theme]\nname = \"minimal\"\nstring = \"green\"").unwrap();
        let cfg = Config::from_raw(raw);
        assert_eq!(cfg.theme.command, ""); // minimal
        assert_eq!(cfg.theme.string, "\x1b[32m"); // override
    }
}
