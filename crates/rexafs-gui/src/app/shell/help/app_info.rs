//! The About panel (version, build and licence) and the Preferences panel,
//! which gathers the machine-local settings already saved in
//! `~/.rexafs/settings.json` (or the file named by `REXAFS_SETTINGS`).
//!
//! Both panels reuse the Help overlay's focus and Escape handling. Preferences
//! introduce no new settings: every control calls the same code path as the
//! older entry points (View → Switch theme, Updates → Preferences), so a
//! change applies immediately and is written to the settings file at once.
use super::*;
use crate::{
    theme::ThemeMode,
    updates::{self, UpdateChannel},
};

/// Which application panel the Help overlay shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum InfoPanel {
    About,
    Preferences,
}

impl InfoPanel {
    pub(super) fn title(self) -> &'static str {
        match self {
            Self::About => "About rexafs",
            Self::Preferences => "Preferences",
        }
    }
}

/// Source repository named in the workspace manifest.
const REPOSITORY: &str = "https://github.com/Ameyanagi/rexafs";

/// Human-readable build lines for the About panel.
///
/// `commit` is the Git commit recorded by CI (`GITHUB_SHA`); local builds
/// have none and are labelled "local build". `built_at` is the UTC build time
/// recorded by nightly builds (`REXAFS_BUILD_UTC`), omitted when absent.
fn build_lines(version: &str, commit: Option<&str>, built_at: Option<&str>) -> Vec<String> {
    let mut lines = vec![format!("Version {version}")];
    lines.push(match commit.map(str::trim).filter(|c| !c.is_empty()) {
        Some(commit) => format!("Commit {}", &commit[..commit.len().min(12)]),
        None => "Commit unavailable · local build".into(),
    });
    if let Some(built_at) = built_at.map(str::trim).filter(|b| !b.is_empty()) {
        lines.push(format!("Built {built_at}"));
    }
    lines
}

impl StudioApp {
    fn open_info(&mut self, panel: InfoPanel, cx: &mut Context<Self>) {
        self.help.menu = false;
        self.help.reader = None;
        self.help.storage = None;
        self.help.info = Some(panel);
        self.focus_help(cx);
    }

    /// App menu → About rexafs: name, version, build and licence.
    pub(crate) fn open_about(&mut self, cx: &mut Context<Self>) {
        self.open_info(InfoPanel::About, cx);
    }

    /// App menu → Settings… (Cmd+, on macOS, Ctrl+, elsewhere).
    pub(crate) fn open_preferences(&mut self, cx: &mut Context<Self>) {
        self.open_info(InfoPanel::Preferences, cx);
    }

    pub(super) fn info_panel(&self, panel: InfoPanel, cx: &mut Context<Self>) -> gpui::AnyElement {
        let t = self.theme;
        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .child(div().text_size(px(20.)).child(match panel {
                InfoPanel::About => updates::application_name(),
                InfoPanel::Preferences => panel.title(),
            }))
            .child(
                button(&t, "close-app-info", "Close", false)
                    .on_click(cx.listener(|app, _, _, cx| app.close_help(cx))),
            );
        let body = match panel {
            InfoPanel::About => self.about_body(cx),
            InfoPanel::Preferences => self.preferences_body(cx),
        };
        div()
            .id("app-info-panel")
            .w(px(480.))
            .max_w_full()
            .max_h_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .bg(t.surface)
            .border_1()
            .border_color(t.border)
            .rounded_lg()
            .shadow_lg()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(header)
            .child(body)
            .into_any_element()
    }

    fn about_body(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let t = self.theme;
        let mut body = div()
            .flex()
            .flex_col()
            .gap_2()
            .child("X-ray absorption fine-structure (XAFS) analysis.");
        for line in build_lines(
            &updates::installed_label(),
            option_env!("GITHUB_SHA"),
            option_env!("REXAFS_BUILD_UTC"),
        ) {
            body = body.child(div().text_color(t.text_muted).child(line));
        }
        body.child(format!(
            "{} channel · licensed under MIT OR Apache-2.0",
            updates::installed_channel().label()
        ))
        .child(
            div()
                .text_color(t.text_muted)
                .child("Third-party notices for bundled dependencies are listed under Licenses."),
        )
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_2()
                .child(
                    button(&t, "about-licenses", "Licenses", false)
                        .on_click(cx.listener(|app, _, _, cx| app.open_licenses(cx))),
                )
                .child(
                    button(&t, "about-repository", "Source repository", false)
                        .on_click(cx.listener(|_, _, _, cx| cx.open_url(REPOSITORY))),
                )
                .child(
                    button(&t, "about-updates", "Updates…", false).on_click(cx.listener(
                        |app, _, _, cx| {
                            app.close_help(cx);
                            app.open_updates(cx);
                        },
                    )),
                ),
        )
        .into_any_element()
    }

    fn preferences_body(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let t = self.theme;
        let settings = &self.structure.settings;
        let section = |title: &'static str| div().pt_1().text_size(px(15.)).child(title);
        let note = |text: String| div().text_color(t.text_muted).child(text);
        let theme_mode = self.theme.mode;
        let theme_choice = |id: &'static str, label: &'static str, mode: ThemeMode| {
            button(&t, id, label, theme_mode == mode).on_click(cx.listener(move |app, _, _, cx| {
                if app.theme.mode != mode {
                    app.toggle_theme(cx);
                }
            }))
        };
        let channel = settings.update_channel;
        let updates_busy = self.updates.is_busy();
        let channel_choice = |id: &'static str, value: UpdateChannel| {
            button(&t, id, value.label(), channel == value).on_click(cx.listener(
                move |app, _, _, cx| {
                    app.set_update_channel(value, cx);
                    cx.notify();
                },
            ))
        };
        let startup = settings.check_updates_on_startup.unwrap_or(true);
        let path_or = |path: Option<&std::path::Path>| {
            path.map_or_else(|| "Not set".to_string(), |p| p.display().to_string())
        };
        let mut body = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(note(
                "Changes apply immediately and are saved for this computer.".into(),
            ))
            .child(section("Appearance"))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(theme_choice("prefs-theme-dark", "Dark", ThemeMode::Dark))
                    .child(theme_choice("prefs-theme-light", "Light", ThemeMode::Light)),
            )
            .when(crate::settings::env_var_os("THEME").is_some(), |d| {
                d.child(note(
                    "REXAFS_THEME overrides the saved theme at the next launch.".into(),
                ))
            })
            .child(section("Updates"))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(channel_choice("prefs-stable", UpdateChannel::Stable))
                    .child(channel_choice("prefs-nightly", UpdateChannel::Nightly)),
            )
            .when(updates_busy, |d| {
                d.child(note(
                    "The channel cannot change while an update downloads or installs.".into(),
                ))
            })
            .child(
                button(
                    &t,
                    "prefs-startup-updates",
                    if startup {
                        "✓ Check for updates on startup"
                    } else {
                        "Check for updates on startup"
                    },
                    false,
                )
                .on_click(cx.listener(|app, _, _, cx| app.toggle_startup_update_check(cx))),
            )
            .child(note(
                "A startup check only looks for a release; nothing installs without your click."
                    .into(),
            ))
            .child(section("Structure sources"))
            .child(note(format!(
                "CIF library: {}",
                path_or(settings.cif_library.as_deref())
            )))
            .child(note(format!(
                "AMCSD database: {}",
                path_or(settings.amcsd_db.as_deref())
            )))
            .child(note(format!(
                "Materials Project API key: {}",
                if settings.mp_api_key.trim().is_empty() {
                    "Not set"
                } else {
                    "Saved"
                }
            )))
            .child(note(
                "Change these in the structure search where they are used.".into(),
            ))
            .child(section("Assistant"))
            .child(note(format!(
                "Model: {} · Web search: {} · {} conversations kept per project",
                settings.assistant_model.as_deref().unwrap_or("Automatic"),
                if settings.assistant_web_search.unwrap_or(true) {
                    "On"
                } else {
                    "Off"
                },
                settings.assistant_history_limit
            )))
            .child(note("Change these in the Assistant panel.".into()));
        if let Some(path) = crate::settings::location() {
            body = body
                .child(section("Settings file"))
                .child(note(path.display().to_string()));
            if path.exists() {
                body = body.child(
                    button(&t, "prefs-reveal-settings", "Show settings file", false)
                        .on_click(cx.listener(move |_, _, _, cx| cx.reveal_path(&path))),
                );
            }
        }
        body.into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::build_lines;

    #[test]
    fn build_lines_shorten_ci_commit_and_label_local_builds() {
        assert_eq!(
            build_lines(
                "0.2.14",
                Some("0123456789abcdef0123"),
                Some("2026-10-23T00:00Z")
            ),
            [
                "Version 0.2.14",
                "Commit 0123456789ab",
                "Built 2026-10-23T00:00Z"
            ]
        );
        assert_eq!(
            build_lines("0.2.14", None, Some(" ")),
            ["Version 0.2.14", "Commit unavailable · local build"]
        );
        assert_eq!(
            build_lines("0.2.14", Some(""), None)[1],
            "Commit unavailable · local build"
        );
    }
}
