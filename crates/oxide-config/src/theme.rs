//! Color themes for the interactive line editor.
//!
//! A [`Theme`] is a set of *resolved* ANSI escape sequences, one per syntax
//! role. Users pick a built-in theme by name and/or override individual roles
//! in `~/.oxiderc.toml`; color specs are written as names (`red`), truecolor
//! hex (`#B7410E`), or `none`/`default` for "no color".

use serde::Deserialize;

/// Resets all SGR attributes. Appended after every colored span.
pub const RESET: &str = "\x1b[0m";

/// A fully-resolved set of ANSI color codes, one per highlighting role. An
/// empty string means "leave this role uncolored" (the terminal default).
#[derive(Debug, Clone)]
pub struct Theme {
    /// A recognized command in command position.
    pub command: String,
    /// A command in command position that isn't a known builtin/executable.
    pub unknown_command: String,
    /// A `-flag` / `--flag`.
    pub flag: String,
    /// A quoted string.
    pub string: String,
    /// A pipe/redirect/separator operator.
    pub operator: String,
    /// Fish-style autosuggestion ghost text.
    pub hint: String,
    /// The prompt accent (and, by default, the same hue as `command`).
    pub accent: String,
}

impl Default for Theme {
    fn default() -> Self {
        named("default").expect("built-in default theme must exist")
    }
}

/// The raw `[theme]` table as written in `oxiderc.toml`: an optional base
/// `name` plus optional per-role overrides.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct RawTheme {
    pub name: Option<String>,
    pub command: Option<String>,
    pub unknown_command: Option<String>,
    pub flag: Option<String>,
    pub string: Option<String>,
    pub operator: Option<String>,
    pub hint: Option<String>,
    pub accent: Option<String>,
}

impl Theme {
    /// Resolve a raw `[theme]` table into concrete ANSI codes: start from the
    /// named base theme (default: `"default"`), then apply any per-role
    /// overrides. An unknown theme name falls back to `default`.
    pub fn from_raw(raw: RawTheme) -> Self {
        let base_name = raw.name.as_deref().unwrap_or("default");
        let mut theme = named(base_name).unwrap_or_default();

        apply(&mut theme.command, raw.command);
        apply(&mut theme.unknown_command, raw.unknown_command);
        apply(&mut theme.flag, raw.flag);
        apply(&mut theme.string, raw.string);
        apply(&mut theme.operator, raw.operator);
        apply(&mut theme.hint, raw.hint);
        apply(&mut theme.accent, raw.accent);
        theme
    }
}

fn apply(slot: &mut String, spec: Option<String>) {
    if let Some(spec) = spec {
        *slot = to_ansi(&spec);
    }
}

/// Look up a built-in theme by name. Color arguments are specs (see
/// [`to_ansi`]), resolved here into ANSI codes.
pub fn named(name: &str) -> Option<Theme> {
    let build = |command, unknown, flag, string, operator, hint, accent| Theme {
        command: to_ansi(command),
        unknown_command: to_ansi(unknown),
        flag: to_ansi(flag),
        string: to_ansi(string),
        operator: to_ansi(operator),
        hint: to_ansi(hint),
        accent: to_ansi(accent),
    };

    Some(match name {
        // Oxide's signature look: rusty-orange accent on a full palette.
        "default" => build("#B7410E", "red", "yellow", "green", "cyan", "dim", "#B7410E"),
        // Brighter hues that read well on dark backgrounds.
        "dark" => build(
            "#E8A87C", "#FF6B6B", "#F4D35E", "#A3D9A5", "#7FD4E0", "dim", "#E8A87C",
        ),
        // Only the prompt accent and dimmed hints; no line coloring.
        "minimal" => build("none", "none", "none", "none", "none", "dim", "#B7410E"),
        _ => return None,
    })
}

/// The list of built-in theme names, for docs/error messages.
pub const BUILTIN_THEMES: &[&str] = &["default", "dark", "minimal"];

/// Convert a color spec into an ANSI foreground escape sequence.
///
/// Accepts:
/// - `""`, `"none"`, `"default"` → empty string (no color);
/// - named colors: `black`, `red`, `green`, `yellow`, `blue`, `magenta`,
///   `cyan`, `white`, and `dim`/`gray`/`grey` (bright black);
/// - truecolor hex: `#RRGGBB`.
///
/// Anything unrecognized resolves to no color.
pub fn to_ansi(spec: &str) -> String {
    let spec = spec.trim();
    if let Some(hex) = spec.strip_prefix('#') {
        if let Some((r, g, b)) = parse_hex(hex) {
            return format!("\x1b[38;2;{r};{g};{b}m");
        }
        return String::new();
    }

    let code = match spec.to_ascii_lowercase().as_str() {
        "" | "none" | "default" => return String::new(),
        "black" => 30,
        "red" => 31,
        "green" => 32,
        "yellow" => 33,
        "blue" => 34,
        "magenta" => 35,
        "cyan" => 36,
        "white" => 37,
        "dim" | "gray" | "grey" => 90,
        _ => return String::new(),
    };
    format!("\x1b[{code}m")
}

fn parse_hex(hex: &str) -> Option<(u8, u8, u8)> {
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some((r, g, b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_colors_resolve() {
        assert_eq!(to_ansi("red"), "\x1b[31m");
        assert_eq!(to_ansi("dim"), "\x1b[90m");
        assert_eq!(to_ansi("YELLOW"), "\x1b[33m"); // case-insensitive
    }

    #[test]
    fn hex_colors_resolve_to_truecolor() {
        assert_eq!(to_ansi("#B7410E"), "\x1b[38;2;183;65;14m");
    }

    #[test]
    fn none_and_garbage_are_empty() {
        assert_eq!(to_ansi("none"), "");
        assert_eq!(to_ansi("default"), "");
        assert_eq!(to_ansi(""), "");
        assert_eq!(to_ansi("#nothex"), "");
        assert_eq!(to_ansi("chartreuse"), "");
    }

    #[test]
    fn all_builtin_themes_exist() {
        for name in BUILTIN_THEMES {
            assert!(named(name).is_some(), "missing theme {name}");
        }
        assert!(named("does-not-exist").is_none());
    }

    #[test]
    fn minimal_leaves_line_roles_uncolored() {
        let t = named("minimal").unwrap();
        assert_eq!(t.command, "");
        assert_eq!(t.string, "");
        assert!(!t.accent.is_empty()); // but keeps the prompt accent
    }

    #[test]
    fn from_raw_applies_overrides_on_top_of_named_base() {
        let raw = RawTheme {
            name: Some("minimal".into()),
            string: Some("green".into()),
            ..Default::default()
        };
        let t = Theme::from_raw(raw);
        assert_eq!(t.command, ""); // inherited from minimal
        assert_eq!(t.string, "\x1b[32m"); // overridden
    }

    #[test]
    fn from_raw_unknown_name_falls_back_to_default() {
        let raw = RawTheme {
            name: Some("bogus".into()),
            ..Default::default()
        };
        let t = Theme::from_raw(raw);
        assert_eq!(t.command, to_ansi("#B7410E"));
    }
}
