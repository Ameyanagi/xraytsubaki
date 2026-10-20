//! Semantic theme tokens with Dark (instrument-style, default) and Light
//! (publication-style) presets. See doc/gui-ux-design.md "Visual design".

use gpui::{Rgba, rgb};
use serde::{Deserialize, Serialize};

/// Use a native monospace family so numeric columns align on every desktop.
#[cfg(target_os = "macos")]
pub const MONO: &str = "Menlo";
#[cfg(target_os = "windows")]
pub const MONO: &str = "Consolas";
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub const MONO: &str = "DejaVu Sans Mono";

/// The two presets. Serialized in lowercase ("dark" / "light") so the value in
/// `settings.json` matches the `REXAFS_THEME` environment variable.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    Dark,
    Light,
}

impl ThemeMode {
    /// Parse the `REXAFS_THEME` value; anything other than "light" or "dark" is None.
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "light" => Some(Self::Light),
            "dark" => Some(Self::Dark),
            _ => None,
        }
    }
}

#[derive(Clone, Copy)]
pub struct Theme {
    pub mode: ThemeMode,
    /// Window background (deepest layer).
    pub bg: Rgba,
    /// Panels: data browser, context panel, status bar.
    pub surface: Rgba,
    /// Cards / hovered rows / plot frames.
    pub raised: Rgba,
    pub border: Rgba,
    pub text: Rgba,
    pub text_muted: Rgba,
    pub accent: Rgba,
    // Job/status colors; consumed from M1 (catalog progress, batch errors) on.
    #[allow(dead_code)]
    pub success: Rgba,
    #[allow(dead_code)]
    pub warn: Rgba,
    #[allow(dead_code)]
    pub error: Rgba,
}

impl Theme {
    pub fn dark() -> Self {
        Self {
            mode: ThemeMode::Dark,
            bg: rgb(0x16181d),
            surface: rgb(0x1d2026),
            raised: rgb(0x262a31),
            border: rgb(0x33373f),
            text: rgb(0xd8dbe2),
            text_muted: rgb(0x8a909c),
            accent: rgb(0x5ba9f7),
            success: rgb(0x67c587),
            warn: rgb(0xe0b35a),
            error: rgb(0xe06c75),
        }
    }

    pub fn light() -> Self {
        Self {
            mode: ThemeMode::Light,
            bg: rgb(0xf2f3f5),
            surface: rgb(0xfafafa),
            raised: rgb(0xffffff),
            border: rgb(0xd9dce1),
            text: rgb(0x24292f),
            // 5.8:1 on `surface`, so 11 px captions pass WCAG AA (4.5:1).
            text_muted: rgb(0x5c636d),
            accent: rgb(0x1f6feb),
            success: rgb(0x1a7f37),
            warn: rgb(0x9a6700),
            error: rgb(0xcf222e),
        }
    }

    /// The preset for a mode.
    pub fn for_mode(mode: ThemeMode) -> Self {
        match mode {
            ThemeMode::Dark => Self::dark(),
            ThemeMode::Light => Self::light(),
        }
    }

    pub fn toggled(&self) -> Self {
        match self.mode {
            ThemeMode::Dark => Self::light(),
            ThemeMode::Light => Self::dark(),
        }
    }

    /// Solid accent tint for the current row in a list (for example the
    /// current group). Text on it should use `text`, not `text_muted`, to keep
    /// WCAG AA contrast. Marked groups use a checkbox, never this tint.
    pub fn current_row(&self) -> Rgba {
        Rgba {
            a: 0.28,
            ..self.accent
        }
    }

    /// Matching ruviz plot theme so plot canvases restyle with the chrome.
    pub fn plot_theme(&self) -> ruviz::render::Theme {
        match self.mode {
            ThemeMode::Dark => ruviz::render::Theme::dark(),
            ThemeMode::Light => ruviz::render::Theme::light(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_mode_round_trips_through_settings_json() {
        assert_eq!(
            serde_json::to_string(&ThemeMode::Light).unwrap(),
            "\"light\""
        );
        assert_eq!(
            serde_json::from_str::<ThemeMode>("\"dark\"").unwrap(),
            ThemeMode::Dark
        );
        assert_eq!(ThemeMode::parse("Light"), Some(ThemeMode::Light));
        assert_eq!(ThemeMode::parse("auto"), None);
        assert_eq!(Theme::for_mode(ThemeMode::Light).mode, ThemeMode::Light);
    }
}
