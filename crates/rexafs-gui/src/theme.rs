//! Semantic theme tokens with Dark (instrument-style, default) and Light
//! (publication-style) presets. See doc/gui-ux-design.md "Visual design".

use gpui::{Rgba, rgb};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ThemeMode {
    Dark,
    Light,
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
            text_muted: rgb(0x6e7681),
            accent: rgb(0x1f6feb),
            success: rgb(0x1a7f37),
            warn: rgb(0x9a6700),
            error: rgb(0xcf222e),
        }
    }

    pub fn toggled(&self) -> Self {
        match self.mode {
            ThemeMode::Dark => Self::light(),
            ThemeMode::Light => Self::dark(),
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
