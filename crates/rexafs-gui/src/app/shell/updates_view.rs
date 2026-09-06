//! About, update-channel preferences, and verified desktop downloads.
use super::button;
use crate::{
    app::StudioApp,
    updates::{self, UpdateChannel, UpdateCheck},
};
use gpui::{Context, IntoElement, ParentElement, Styled, div, prelude::*, px};
use std::path::PathBuf;

#[derive(Default)]
pub(crate) struct UpdateState {
    pub open: bool,
    checking: bool,
    downloading: bool,
    generation: u64,
    pub result: Option<UpdateCheck>,
    error: Option<String>,
    downloaded: Option<PathBuf>,
    focus: Option<gpui::FocusHandle>,
}

impl StudioApp {
    pub(crate) fn open_updates(&mut self, cx: &mut Context<Self>) {
        self.updates.open = true;
        let focus = self
            .updates
            .focus
            .get_or_insert_with(|| cx.focus_handle())
            .clone();
        let view = cx.weak_entity();
        let handle = self.main_window;
        cx.defer(move |cx| {
            let _ = handle.update(cx, |_, window, cx| {
                let _ = view.update(cx, |app, cx| {
                    if app.updates.open {
                        focus.focus(window, cx);
                    }
                });
            });
        });
        if self.updates.result.is_none() && !self.updates.checking {
            self.check_for_updates(cx);
        }
        cx.notify();
    }
    fn close_updates(&mut self, window: &mut gpui::Window, cx: &mut Context<Self>) {
        self.updates.open = false;
        let focus = self.root_focus.clone();
        cx.defer_in(window, move |_, window, cx| focus.focus(window, cx));
        cx.notify();
    }
    pub(crate) fn check_for_updates(&mut self, cx: &mut Context<Self>) {
        if self.updates.checking || self.updates.downloading {
            return;
        }
        self.updates.generation += 1;
        let generation = self.updates.generation;
        let channel = self.structure.settings.update_channel;
        self.updates.checking = true;
        self.updates.error = None;
        self.updates.downloaded = None;
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { updates::check(channel) })
                .await;
            this.update(cx, |app, cx| {
                if generation != app.updates.generation {
                    return;
                }
                app.updates.checking = false;
                match result {
                    Ok(result) => {
                        if result.available {
                            app.status = format!(
                                "{} update available — open Updates to review it.",
                                channel.label()
                            )
                            .into();
                        } else if app.updates.open {
                            app.status = if result.release.is_some() {
                                format!("{} release check complete — up to date.", channel.label())
                            } else {
                                format!("No {} releases are published yet.", channel.label())
                            }
                            .into();
                        }
                        app.updates.result = Some(result);
                    }
                    Err(e) => app.updates.error = Some(e),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }
    fn set_update_channel(&mut self, channel: UpdateChannel, cx: &mut Context<Self>) {
        if self.updates.downloading || channel == self.structure.settings.update_channel {
            return;
        }
        let mut settings = self.structure.settings.clone();
        settings.update_channel = channel;
        if let Err(e) = settings.save() {
            self.updates.error = Some(e);
            cx.notify();
            return;
        }
        self.structure.settings = settings;
        self.updates.generation += 1;
        self.updates.checking = false;
        self.updates.result = None;
        self.updates.downloaded = None;
        self.check_for_updates(cx);
    }
    fn toggle_startup_update_check(&mut self, cx: &mut Context<Self>) {
        let mut settings = self.structure.settings.clone();
        settings.check_updates_on_startup =
            Some(!settings.check_updates_on_startup.unwrap_or(true));
        match settings.save() {
            Ok(()) => self.structure.settings = settings,
            Err(e) => self.updates.error = Some(e),
        }
        cx.notify();
    }
    fn download_update(&mut self, cx: &mut Context<Self>) {
        if self.updates.downloading {
            return;
        }
        let Some(release) = self.updates.result.as_ref().and_then(|r| r.release.clone()) else {
            return;
        };
        if release.asset.is_none() {
            return;
        }
        self.updates.downloading = true;
        self.updates.error = None;
        self.updates.downloaded = None;
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { updates::download(&release) })
                .await;
            this.update(cx, |app, cx| {
                app.updates.downloading = false;
                match result {
                    Ok(path) => {
                        app.updates.downloaded = Some(path);
                        app.status =
                            "Update downloaded and SHA-256 verified. Open Updates to reveal it."
                                .into();
                    }
                    Err(e) => app.updates.error = Some(e),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }
    pub(crate) fn updates_overlay(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        if !self.updates.open {
            return None;
        }
        let t = self.theme;
        let channel = self.structure.settings.update_channel;
        let auto = self
            .structure
            .settings
            .check_updates_on_startup
            .unwrap_or(true);
        let mut panel=div().w(px(480.)).p_4().flex().flex_col().gap_3().rounded_lg().bg(t.surface).border_1().border_color(t.border)
            .child(div().flex().items_center().gap_3()
                .child(div().flex_1().text_size(px(18.)).child("rexafs updates"))
                .child(button(&t,"close-updates","Close",false).on_click(cx.listener(|this,_,window,cx|this.close_updates(window,cx)))))
            .child(div().text_color(t.text_muted).child(format!("Installed: {} · {}",updates::installed_label(),updates::installed_channel().label())))
            .child(div().text_color(t.text_muted).child("Choose your update channel"))
            .child(div().flex().gap_2()
                .child(button(&t,"stable-updates","Stable",channel==UpdateChannel::Stable).on_click(cx.listener(|this,_,_,cx|this.set_update_channel(UpdateChannel::Stable,cx))))
                .child(button(&t,"nightly-updates","Nightly",channel==UpdateChannel::Nightly).on_click(cx.listener(|this,_,_,cx|this.set_update_channel(UpdateChannel::Nightly,cx)))))
            .child(div().text_color(t.text_muted).child(match channel {
                UpdateChannel::Stable=>"Reviewed releases for routine analysis.",
                UpdateChannel::Nightly=>"Daily builds from main with the newest changes. On macOS, rexafs Nightly can be installed alongside Stable.",
            }))
            .child(button(&t,"startup-updates",if auto {"✓ Check for updates on startup"}else{"Check for updates on startup"},false)
                .on_click(cx.listener(|this,_,_,cx|this.toggle_startup_update_check(cx))));
        if self.updates.checking {
            panel = panel.child("Checking GitHub releases…");
        } else if let Some(result) = &self.updates.result {
            if let Some(release) = &result.release {
                panel = panel.child(if result.available {
                    format!("Available: {}", release.tag)
                } else {
                    format!(
                        "Up to date · latest {} release: {}",
                        channel.label(),
                        release.tag
                    )
                });
                let url = release.url.clone();
                panel = panel.child(
                    button(&t, "update-notes", "Release notes", false)
                        .on_click(cx.listener(move |_, _, _, cx| cx.open_url(&url))),
                );
                if let Some(asset) = &release.asset {
                    if !self.updates.downloading && self.updates.downloaded.is_none() {
                        panel = panel.child(
                            button(
                                &t,
                                "download-update",
                                format!(
                                    "Download {} · {:.1} MB",
                                    release.tag,
                                    asset.size as f64 / 1_000_000.
                                ),
                                true,
                            )
                            .on_click(cx.listener(|this, _, _, cx| this.download_update(cx))),
                        );
                    }
                } else {
                    panel=panel.child(div().text_color(t.text_muted).child("No checksum-verified desktop download is available for this platform. See the release notes for supported builds."));
                }
            } else {
                panel = panel.child(format!(
                    "No {} releases are published yet.",
                    channel.label()
                ));
            }
        }
        if self.updates.downloading {
            panel = panel.child("Downloading and verifying SHA-256…");
        }
        if let Some(path) = &self.updates.downloaded {
            let path = path.clone();
            panel=panel.child("Download complete · SHA-256 verified")
                .child(button(&t,"reveal-update","Show download in Finder",true).on_click(cx.listener(move |_,_,_,cx|cx.reveal_path(&path))))
                .child(div().text_color(t.text_muted).child("Open the ZIP. Save your project and quit the app before moving the downloaded application into Applications."));
        }
        if let Some(error) = &self.updates.error {
            panel = panel.child(div().text_color(t.error).child(error.clone()));
        }
        if !self.updates.checking && !self.updates.downloading {
            panel = panel.child(
                button(&t, "check-updates", "Check again", false)
                    .on_click(cx.listener(|this, _, _, cx| this.check_for_updates(cx))),
            );
        }
        Some(
            div()
                .id("updates-overlay")
                .occlude()
                .track_focus(
                    self.updates
                        .focus
                        .as_ref()
                        .expect("open update dialog has focus"),
                )
                .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
                    if event.keystroke.key == "escape" {
                        this.close_updates(window, cx);
                    }
                    cx.stop_propagation();
                }))
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(gpui::rgba(0x00000099))
                .child(panel)
                .into_any_element(),
        )
    }
}
