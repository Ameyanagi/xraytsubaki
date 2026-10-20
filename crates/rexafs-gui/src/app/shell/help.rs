//! Secondary actions and an offline license reader.
use super::button;
use crate::{app::StudioApp, licenses};
use gpui::{Context, FocusHandle, IntoElement, MouseButton, SharedString, div, prelude::*, px};
mod storage;

#[derive(Default)]
pub(crate) struct HelpState {
    menu: bool,
    reader: Option<LicenseReader>,
    focus: Option<FocusHandle>,
    return_focus: Option<FocusHandle>,
    storage: Option<storage::StorageView>,
    storage_generation: u64,
}

struct LicenseReader {
    documents: Vec<licenses::Document>,
    selected: usize,
    text: SharedString,
    scroll: gpui::ScrollHandle,
}

impl HelpState {
    pub(crate) fn is_open(&self) -> bool {
        self.menu || self.reader.is_some() || self.storage.is_some()
    }
}

impl StudioApp {
    fn focus_help(&mut self, cx: &mut Context<Self>) {
        let focus = self
            .help
            .focus
            .get_or_insert_with(|| cx.focus_handle())
            .clone();
        let view = cx.weak_entity();
        let handle = self.main_window;
        cx.defer(move |cx| {
            let _ = handle.update(cx, |_, window, cx| {
                let _ = view.update(cx, |app, cx| {
                    if app.help.is_open() {
                        if app.help.return_focus.is_none() {
                            app.help.return_focus = window.focused(cx);
                        }
                        focus.focus(window, cx);
                    }
                });
            });
        });
        cx.notify();
    }

    pub(crate) fn open_help(&mut self, cx: &mut Context<Self>) {
        self.help.menu = true;
        self.focus_help(cx);
    }

    fn close_help(&mut self, cx: &mut Context<Self>) {
        self.help.menu = false;
        self.help.reader = None;
        self.help.storage = None;
        let focus = self
            .help
            .return_focus
            .take()
            .unwrap_or_else(|| self.root_focus.clone());
        let view = cx.weak_entity();
        let handle = self.main_window;
        cx.defer(move |cx| {
            let _ = handle.update(cx, |_, window, cx| {
                let _ = view.update(cx, |app, cx| {
                    if !app.help.is_open() && !app.updates.open && app.palette.is_none() {
                        focus.focus(window, cx);
                    }
                });
            });
        });
        cx.notify();
    }

    pub(crate) fn open_example(&mut self, cx: &mut Context<Self>) {
        self.close_help(cx);
        if let Some(path) = crate::app::example_data_file() {
            self.route_paths(vec![path], false, cx);
        } else {
            self.status = "Bundled example unavailable".into();
            cx.notify();
        }
    }

    pub(crate) fn open_copper_series(&mut self, cx: &mut Context<Self>) {
        if self.updates.is_installing() {
            return;
        }
        self.close_help(cx);
        self.pending_routed_import.clear();
        self.project_load_generation += 1;
        let generation = self.project_load_generation;
        self.status = "Opening synthetic copper reduction…".into();
        cx.spawn(async move |this, cx| {
            let result = cx.background_executor().spawn(async {
                crate::project::copper_example().map(|mut project| {
                    let next = project.assign_group_ids();
                    let registry = crate::group_identity::GroupRegistry::from_sources(std::mem::take(&mut project.source_groups));
                    (project, next, registry)
                })
            }).await;
            this.update(cx, |app, cx| {
                if app.project_load_generation != generation { return; }
                match result {
                    Ok((project, next, registry)) => {
                        app.apply_project(project, next, registry, cx);
                        // The bundled example can be reopened at any time.
                        app.mark_project_clean();
                        app.status = "Synthetic raw-μ copper series · 50 frames + 3 references · labels show raw mixing weights".into();
                    }
                    Err(error) => app.status = format!("Example could not open: {error}").into(),
                }
                cx.notify();
            }).ok();
        }).detach();
        cx.notify();
    }

    pub(crate) fn open_licenses(&mut self, cx: &mut Context<Self>) {
        let documents = licenses::documents();
        let text = documents[0].read().unwrap_or_else(|error| error).into();
        self.help.menu = false;
        self.help.reader = Some(LicenseReader {
            documents,
            selected: 0,
            text,
            scroll: Default::default(),
        });
        self.focus_help(cx);
    }

    pub(crate) fn help_overlay(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        if !self.help.is_open() {
            return None;
        }
        let t = self.theme;
        let overlay = crate::accessibility::Control::new(
            div().id("help-overlay"),
            if self.help.storage.is_some() {
                "Storage"
            } else {
                "Help"
            },
            accesskit::Role::Dialog,
        )
        .modal()
        .tab_group()
        .absolute()
        .inset_0()
        .occlude()
        .track_focus(self.help.focus.as_ref().expect("help has focus"))
        .on_key_down(cx.listener(|app, event: &gpui::KeyDownEvent, window, cx| {
            if event.keystroke.key == "escape" {
                app.close_help(cx);
            }
            super::controls::navigate(event, window, cx);
            cx.stop_propagation();
        }))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|app, _, _, cx| app.close_help(cx)),
        );
        if self.help.storage.is_some() {
            return Some(
                overlay
                    .flex()
                    .p_4()
                    .items_center()
                    .justify_center()
                    .bg(gpui::rgba(0x00000099))
                    .child(self.storage_panel(cx))
                    .into_any_element(),
            );
        }
        let Some(reader) = &self.help.reader else {
            let item =
                |id: &'static str,
                 label: &'static str,
                 action: fn(&mut StudioApp, &mut Context<StudioApp>)| {
                    crate::accessibility::Control::new(
                        div().id(id),
                        label,
                        accesskit::Role::MenuItem,
                    )
                    .px_3()
                    .py_2()
                    .cursor_pointer()
                    .rounded_sm()
                    .hover(|d| d.bg(t.raised))
                    .child(label)
                    .on_click(cx.listener(move |app, _, _, cx| action(app, cx)))
                };
            return Some(
                overlay
                    .child(
                        div()
                            .absolute()
                            .top(px(38.))
                            .right(px(12.))
                            .w(px(275.))
                            .p_1()
                            .bg(t.surface)
                            .border_1()
                            .border_color(t.border)
                            .rounded_md()
                            .shadow_lg()
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .child(item("help-example", "Open Cu example", Self::open_example))
                            .child(item(
                                "help-copper-series",
                                "Synthetic copper reduction…",
                                Self::open_copper_series,
                            ))
                            .child(item("help-storage", "Storage…", Self::open_storage))
                            .child(item("help-licenses", "Licenses", Self::open_licenses))
                            .child(item("help-updates", "Updates", |app, cx| {
                                app.close_help(cx);
                                app.open_updates(cx);
                            })),
                    )
                    .into_any_element(),
            );
        };
        let mut list = div()
            .id("license-list")
            .w(px(250.))
            .flex_none()
            .overflow_y_scroll()
            .p_2()
            .border_r_1()
            .border_color(t.border);
        for (index, document) in reader.documents.iter().enumerate() {
            list = list.child(
                div()
                    .id(("license-document", index))
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .cursor_pointer()
                    .when(index == reader.selected, |d| {
                        d.bg(t.raised).text_color(t.accent)
                    })
                    .hover(|d| d.bg(t.raised))
                    .child(document.title.clone())
                    .on_click(cx.listener(move |app, _, _, cx| {
                        if let Some(reader) = &mut app.help.reader {
                            reader.selected = index;
                            reader.text = reader.documents[index]
                                .read()
                                .unwrap_or_else(|error| error)
                                .into();
                            reader.scroll.set_offset(gpui::point(px(0.), px(0.)));
                            cx.notify();
                        }
                    })),
            );
        }
        let panel = div()
            .w(px(920.))
            .max_w_full()
            .h(px(620.))
            .max_h_full()
            .flex()
            .flex_col()
            .bg(t.surface)
            .border_1()
            .border_color(t.border)
            .rounded_lg()
            .shadow_lg()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .p_3()
                    .border_b_1()
                    .border_color(t.border)
                    .child(div().flex_1().text_size(px(16.)).child("Licenses"))
                    .child(
                        button(&t, "copy-license", "Copy", false).on_click(cx.listener(
                            |app, _, _, cx| {
                                if let Some(reader) = &app.help.reader {
                                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                        reader.text.to_string(),
                                    ));
                                }
                            },
                        )),
                    )
                    .child(
                        button(&t, "close-licenses", "Close", false)
                            .on_click(cx.listener(|app, _, _, cx| app.close_help(cx))),
                    ),
            )
            .child(
                div().flex_1().min_h_0().flex().child(list).child(
                    div()
                        .id("license-text")
                        .flex_1()
                        .min_w_0()
                        .overflow_y_scroll()
                        .track_scroll(&reader.scroll)
                        .p_4()
                        .child(reader.text.clone()),
                ),
            );
        Some(
            overlay
                .flex()
                .p_4()
                .items_center()
                .justify_center()
                .bg(gpui::rgba(0x00000099))
                .child(panel)
                .into_any_element(),
        )
    }
}
