//! About, update-channel preferences, and verified desktop downloads.
use super::{
    button,
    controls::{disclosure, icon, icon_button},
};
use crate::icons::Icon;
use crate::{
    app::StudioApp,
    updates::{self, UpdateChannel, UpdateCheck},
};
use gpui::{Context, IntoElement, ParentElement, Styled, div, prelude::*, px};
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Default)]
pub(crate) struct UpdateState {
    pub open: bool,
    checking: bool,
    downloading: bool,
    installing: bool,
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    handoff: Option<updates::install::UpdateHandoff>,
    cancel: Option<Arc<AtomicBool>>,
    progress: Option<String>,
    generation: u64,
    pub result: Option<UpdateCheck>,
    error: Option<String>,
    downloaded: Option<PathBuf>,
    focus: Option<gpui::FocusHandle>,
    return_focus: Option<gpui::FocusHandle>,
    preferences_open: bool,
}

impl UpdateState {
    pub(crate) fn is_busy(&self) -> bool {
        self.downloading || self.installing
    }
    pub(crate) fn is_installing(&self) -> bool {
        self.installing
    }
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
                        if app.updates.return_focus.is_none() {
                            app.updates.return_focus = window.focused(cx);
                        }
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
        if self.updates.installing {
            self.cancel_update(cx);
            return;
        }
        self.updates.open = false;
        let focus = self
            .updates
            .return_focus
            .take()
            .unwrap_or_else(|| self.root_focus.clone());
        cx.defer_in(window, move |_, window, cx| focus.focus(window, cx));
        cx.notify();
    }
    pub(crate) fn check_for_updates(&mut self, cx: &mut Context<Self>) {
        if self.updates.checking || self.updates.downloading || self.updates.installing {
            return;
        }
        self.updates.generation += 1;
        let generation = self.updates.generation;
        let channel = self.structure.settings.update_channel;
        self.updates.checking = true;
        self.updates.error = None;
        self.updates.downloaded = None;
        self.updates.result = None;
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
    pub(crate) fn set_update_channel(&mut self, channel: UpdateChannel, cx: &mut Context<Self>) {
        if self.updates.downloading
            || self.updates.installing
            || channel == self.structure.settings.update_channel
        {
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
    pub(crate) fn toggle_startup_update_check(&mut self, cx: &mut Context<Self>) {
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
        if self.updates.downloading || self.updates.installing {
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
                        app.status = "Download verified. Open Updates for the next step.".into();
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

    fn cancel_update(&mut self, cx: &mut Context<Self>) {
        if let Some(cancel) = &self.updates.cancel {
            cancel.store(true, Ordering::Relaxed);
            self.updates.progress = Some("Cancelling…".into());
            cx.notify();
        }
    }

    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    fn update_and_restart(&mut self, cx: &mut Context<Self>) {
        if self.updates.installing || self.updates.downloading || self.updates.checking {
            return;
        }
        let Some(release) = self
            .updates
            .result
            .as_ref()
            .filter(|result| result.available)
            .and_then(|result| result.release.clone())
        else {
            return;
        };
        if self.running_job_count() > 0
            || self.project_saving
            || self.structure.search_running
            || self.structure.fetch_running
            || self
                .assistant
                .as_ref()
                .is_some_and(|view| !view.read(cx).can_restart_for_update())
        {
            self.updates.error = Some("Wait for the current calculation, import or Assistant turn to finish, then update.".into());
            cx.notify();
            return;
        }
        if let Err(error) = updates::install::installed_app() {
            self.updates.error = Some(error);
            cx.notify();
            return;
        }
        let mut project = self.project_file();
        // Restart reopens exactly the current accepted inputs; it must not
        // discover new files that appeared in a watched folder during download.
        project.source_dir = None;
        let project_generation = self.project_generation;
        let cancel = Arc::new(AtomicBool::new(false));
        self.updates.cancel = Some(cancel.clone());
        self.updates.installing = true;
        self.updates.open = true;
        self.updates.error = None;
        self.updates.progress = Some("Downloading…".into());
        if let Some(assistant) = &self.assistant {
            assistant.update(cx, |view, cx| view.pause_for_update(true, cx));
        }
        let (progress_tx, progress_rx) = std::sync::mpsc::channel();
        let progress_view = cx.weak_entity();
        cx.spawn(async move |_, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(100))
                    .await;
                let latest = progress_rx.try_iter().last();
                let active = progress_view
                    .update(cx, |app, cx| {
                        if let Some(progress) = latest
                            && app
                                .updates
                                .cancel
                                .as_ref()
                                .is_some_and(|cancel| !cancel.load(Ordering::Relaxed))
                        {
                            app.updates.progress = Some(progress);
                            cx.notify();
                        }
                        app.updates.installing
                    })
                    .unwrap_or(false);
                if !active {
                    break;
                }
            }
        })
        .detach();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let check_cancel = || {
                        if cancel.load(Ordering::Relaxed) {
                            Err("Update cancelled. Your app is unchanged.".to_owned())
                        } else {
                            Ok(())
                        }
                    };
                    let mut last_percent = None;
                    let archive =
                        updates::download_with_progress(&release, |downloaded, total| {
                            check_cancel()?;
                            let percent = downloaded.saturating_mul(100) / total.max(1);
                            if last_percent != Some(percent) {
                                let _ = progress_tx.send(format!("Downloading · {percent}%"));
                                last_percent = Some(percent);
                            }
                            Ok(())
                        })?;
                    check_cancel()?;
                    let _ = progress_tx.send("Preparing update…".into());
                    let prepared = updates::install::prepare(&release, &archive, check_cancel)?;
                    check_cancel()?;
                    let _ = progress_tx.send("Saving your analysis…".into());
                    crate::project::save_with_storage(
                        &prepared.recovery_path(),
                        &project,
                        crate::project::DataStorage::Embedded,
                    )
                    .map_err(|e| {
                        format!("Could not save update recovery; rexafs will stay open: {e}")
                    })?;
                    crate::project::load(&prepared.recovery_path()).map_err(|e| {
                        format!("Could not verify update recovery; rexafs will stay open: {e}")
                    })?;
                    check_cancel()?;
                    Ok::<_, String>(prepared)
                })
                .await;
            match result {
                Ok(prepared) => {
                    let proceed = this
                        .update(cx, |app, cx| {
                            if app.project_generation != project_generation {
                                app.finish_update_error("The analysis changed while preparing the update. Try again from the current project.".into(), cx);
                                return false;
                            }
                            let proceed = app
                                .updates
                                .cancel
                                .take()
                                .is_some_and(|cancel| !cancel.load(Ordering::Relaxed));
                            if proceed {
                                app.updates.progress = Some("Restarting…".into());
                                cx.notify();
                            }
                            proceed
                        })
                        .unwrap_or(false);
                    if !proceed {
                        let _ = this.update(cx, |app, cx| {
                            if app.updates.installing {
                                app.finish_update_error(
                                    "Update cancelled. Your app is unchanged.".into(),
                                    cx,
                                );
                            }
                        });
                        return;
                    }
                    let armed = cx.background_spawn(async move { prepared.start() }).await;
                    let _ = this.update(cx, |app, cx| match armed {
                        Ok(handoff) => { app.updates.handoff = Some(handoff); cx.quit(); },
                        Err(error) => app.finish_update_error(error, cx),
                    });
                }
                Err(error) => {
                    let _ = this.update(cx, |app, cx| app.finish_update_error(error, cx));
                }
            }
        })
        .detach();
        cx.notify();
    }

    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    fn finish_update_error(&mut self, error: String, cx: &mut Context<Self>) {
        self.updates.installing = false;
        self.updates.cancel = None;
        self.updates.progress = None;
        if error == "Update cancelled. Your app is unchanged." {
            self.updates.error = None;
            self.status = "Update cancelled.".into();
        } else {
            self.updates.error = Some(error);
        }
        if let Some(assistant) = &self.assistant {
            assistant.update(cx, |view, cx| view.pause_for_update(false, cx));
        }
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
        let install_status = self
            .updates
            .result
            .as_ref()
            .filter(|result| result.available)
            .and_then(|result| result.release.as_ref())
            .map(|release| {
                if channel != updates::installed_channel() {
                    return Err(
                        "Install the other channel separately; this app keeps its current channel."
                            .into(),
                    );
                }
                #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
                {
                    updates::install::check_install(release)
                }
                #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
                {
                    let _ = release;
                    Err::<(), String>(
                        "In-app installation is not supported on this platform.".into(),
                    )
                }
            });
        let automatic = matches!(&install_status, Some(Ok(())));
        let mut panel = crate::accessibility::Control::new(
            div().id("updates-panel"),
            "Updates",
            accesskit::Role::Dialog,
        )
        .modal()
        .w(px(440.))
        .max_w_full()
        .max_h_full()
        .overflow_y_scroll()
        .p_4()
        .flex()
        .flex_col()
        .gap_3()
        .rounded_lg()
        .bg(t.surface)
        .border_1()
        .border_color(t.border)
        .child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .child(div().flex_1().text_size(px(18.)).child("Updates"))
                .when(!self.updates.installing, |d| {
                    d.child(
                        icon_button(&t, "close-updates", Icon::Close, "Close updates", false)
                            .on_click(
                                cx.listener(|app, _, window, cx| app.close_updates(window, cx)),
                            ),
                    )
                }),
        )
        .child(div().text_color(t.text_muted).child(format!(
            "{} · {}",
            updates::installed_label(),
            updates::installed_channel().label()
        )));
        if self.updates.checking {
            panel = panel.child("Checking…");
        } else if let Some(result) = &self.updates.result {
            if let Some(release) = &result.release {
                panel = panel.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(icon(
                            &t,
                            if result.available {
                                Icon::Download
                            } else {
                                Icon::Check
                            },
                        ))
                        .child(if result.available {
                            format!("{} available", release.tag)
                        } else {
                            "Up to date".into()
                        }),
                );
                let url = release.url.clone();
                panel = panel.child(
                    button(&t, "update-notes", "Release notes", false)
                        .child(icon(&t, Icon::External))
                        .on_click(cx.listener(move |_, _, _, cx| cx.open_url(&url))),
                );
                if let Some(asset) = &release.asset {
                    if !self.updates.downloading
                        && !self.updates.installing
                        && (result.available || self.updates.preferences_open)
                    {
                        #[cfg(any(
                            target_os = "macos",
                            target_os = "windows",
                            target_os = "linux"
                        ))]
                        if automatic {
                            panel = panel
                                .child(
                                    button(&t, "install-update", "Update and restart", true)
                                        .on_click(
                                            cx.listener(|app, _, _, cx| app.update_and_restart(cx)),
                                        ),
                                )
                                .child(div().text_color(t.text_muted).child(format!(
                                    "{:.1} MB · Reopens a recovery copy of your analysis.",
                                    updates::install::download_size(release) as f64 / 1_000_000.
                                )));
                        }
                        if self.updates.downloaded.is_none()
                            && (!automatic || self.updates.preferences_open)
                        {
                            panel = panel.child(
                                button(
                                    &t,
                                    "download-update",
                                    format!(
                                        "Download {} · {:.1} MB",
                                        release.tag,
                                        asset.size as f64 / 1_000_000.
                                    ),
                                    !automatic,
                                )
                                .on_click(cx.listener(|app, _, _, cx| app.download_update(cx))),
                            );
                        }
                    }
                } else if result.available {
                    panel = panel.child(
                        div()
                            .text_color(t.warn)
                            .child("No verified download for this platform."),
                    );
                }
            } else {
                panel = panel.child(format!("No {} releases yet", channel.label()));
            }
        }
        if self.updates.installing {
            panel = panel.child(
                self.updates
                    .progress
                    .clone()
                    .unwrap_or_else(|| "Updating…".into()),
            );
            if self.updates.cancel.is_some() {
                panel = panel.child(
                    button(&t, "cancel-update", "Cancel", false)
                        .on_click(cx.listener(|app, _, _, cx| app.cancel_update(cx))),
                );
            }
        }
        if self.updates.downloading {
            panel = panel.child("Downloading and verifying…");
        }
        if !self.updates.installing
            && !self.updates.downloading
            && let Some(Err(reason)) = &install_status
        {
            panel = panel.child(div().text_color(t.text_muted).child(reason.clone()));
        }
        if let Some(path) = &self.updates.downloaded
            && !self.updates.installing
        {
            let path = path.clone();
            panel = panel
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(icon(&t, Icon::Check))
                        .child("Download verified"),
                )
                .child(
                    button(&t, "reveal-update", "Show download", !automatic)
                        .on_click(cx.listener(move |_, _, _, cx| cx.reveal_path(&path))),
                )
                .when(!automatic, |panel| {
                    panel.child(
                        div()
                            .text_color(t.text_muted)
                            .child("Save your project and quit before replacing the app."),
                    )
                });
        }
        if let Some(error) = &self.updates.error {
            panel = panel.child(div().text_color(t.error).child(error.clone()));
        }
        panel = panel.when(!self.updates.installing, |panel| {
            panel.child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        disclosure(
                            &t,
                            "update-preferences",
                            "Preferences",
                            self.updates.preferences_open,
                            false,
                        )
                        .flex_1()
                        .on_click(cx.listener(|app, _, _, cx| {
                            app.updates.preferences_open = !app.updates.preferences_open;
                            cx.notify();
                        })),
                    )
                    .when(!self.updates.checking && !self.updates.downloading, |d| {
                        d.child(
                            icon_button(&t, "check-updates", Icon::Refresh, "Check again", false)
                                .on_click(cx.listener(|app, _, _, cx| app.check_for_updates(cx))),
                        )
                    }),
            )
        });
        if self.updates.preferences_open && !self.updates.installing {
            panel = panel
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(
                            button(
                                &t,
                                "stable-updates",
                                "Stable",
                                channel == UpdateChannel::Stable,
                            )
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.set_update_channel(UpdateChannel::Stable, cx)
                            })),
                        )
                        .child(
                            button(
                                &t,
                                "nightly-updates",
                                "Nightly",
                                channel == UpdateChannel::Nightly,
                            )
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.set_update_channel(UpdateChannel::Nightly, cx)
                            })),
                        ),
                )
                .when(channel == UpdateChannel::Nightly, |d| {
                    d.child(
                        div()
                            .text_color(t.text_muted)
                            .child("Daily development builds"),
                    )
                })
                .child(
                    button(
                        &t,
                        "startup-updates",
                        if auto {
                            "✓ Check on startup"
                        } else {
                            "Check on startup"
                        },
                        false,
                    )
                    .on_click(cx.listener(|app, _, _, cx| app.toggle_startup_update_check(cx))),
                );
        }
        Some(
            div()
                .id("updates-overlay")
                .occlude()
                .tab_group()
                .track_focus(
                    self.updates
                        .focus
                        .as_ref()
                        .expect("open update dialog has focus"),
                )
                .on_key_down(cx.listener(|app, event: &gpui::KeyDownEvent, window, cx| {
                    if event.keystroke.key == "escape" {
                        app.close_updates(window, cx);
                    }
                    super::controls::navigate(event, window, cx);
                    cx.stop_propagation();
                }))
                .absolute()
                .inset_0()
                .p_4()
                .flex()
                .items_center()
                .justify_center()
                .bg(gpui::rgba(0x00000099))
                .child(panel)
                .into_any_element(),
        )
    }
}
