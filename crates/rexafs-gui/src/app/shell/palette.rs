//! Command palette (⌘K): actions, tools and group names, filtered as you
//! type. ↑/↓ move, Enter runs, Esc closes.

use gpui::{
    ClickEvent, Context, Entity, Focusable, IntoElement, ParentElement, SharedString, Styled,
    Window, div, prelude::*, px,
};

use super::tools::Tool;
use super::{MONO, Stage};
use crate::app::{DERIVED_BASE, NO_ENTRY, PaletteClose, StudioApp};
use crate::widgets::text_input::{InputEvent, TextInput};

#[derive(Clone, Debug, PartialEq)]
pub enum PaletteCmd {
    Stage(Stage),
    MarkAll,
    MarkNone,
    ApplyToMarked,
    ResetParams,
    Tool(Tool),
    Fit,
    ExportBatchCsv,
    SaveProject,
    OpenProject,
    OpenFolder,
    Theme,
    Updates,
    Undo,
    Redo,
    Journal,
    Group(usize),
}

pub struct PaletteItem {
    pub label: String,
    pub category: &'static str,
    pub keys: &'static str,
    pub cmd: PaletteCmd,
}

pub struct PaletteState {
    pub input: Entity<TextInput>,
    pub query: String,
    pub selected: usize,
    _subscription: gpui::Subscription,
}

/// Every query character must appear in order (a light fuzzy match).
fn fuzzy(query: &str, text: &str) -> bool {
    let mut chars = query
        .chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| c.to_ascii_lowercase());
    let mut next = chars.next();
    for c in text.chars().map(|c| c.to_ascii_lowercase()) {
        if next == Some(c) {
            next = chars.next();
        }
        if next.is_none() {
            return true;
        }
    }
    next.is_none()
}

impl StudioApp {
    fn palette_items(&self) -> Vec<PaletteItem> {
        let mut items = Vec::new();
        for stage in Stage::ALL {
            items.push(PaletteItem {
                label: format!("Go to {}", stage.name()),
                category: "stage",
                keys: match stage {
                    Stage::Data => "⌘1",
                    Stage::Normalize => "⌘2",
                    Stage::Background => "⌘3",
                    Stage::Transform => "⌘4",
                    Stage::Fit => "⌘5",
                    Stage::Series => "⌘6",
                    Stage::Publish => "",
                },
                cmd: PaletteCmd::Stage(stage),
            });
        }
        let simple: [(&str, &'static str, &'static str, PaletteCmd); 13] = [
            ("Mark all groups", "groups", "", PaletteCmd::MarkAll),
            ("Unmark all groups", "groups", "", PaletteCmd::MarkNone),
            (
                "Apply parameters to marked groups",
                "params",
                "",
                PaletteCmd::ApplyToMarked,
            ),
            (
                "Reset parameters of the current group",
                "params",
                "",
                PaletteCmd::ResetParams,
            ),
            ("Fit the current group", "fit", "", PaletteCmd::Fit),
            (
                "Export batch-fit results as CSV",
                "export",
                "",
                PaletteCmd::ExportBatchCsv,
            ),
            ("Save project…", "file", "", PaletteCmd::SaveProject),
            ("Open project…", "file", "", PaletteCmd::OpenProject),
            ("Import…", "file", "", PaletteCmd::OpenFolder),
            ("Toggle theme", "view", "", PaletteCmd::Theme),
            (
                "Check for updates · Stable / Nightly",
                "app",
                "",
                PaletteCmd::Updates,
            ),
            ("Undo", "edit", "⌘Z", PaletteCmd::Undo),
            ("Redo", "edit", "⇧⌘Z", PaletteCmd::Redo),
        ];
        for (label, category, keys, cmd) in simple {
            items.push(PaletteItem {
                label: label.into(),
                category,
                keys,
                cmd,
            });
        }
        items.push(PaletteItem {
            label: "Show journal".into(),
            category: "view",
            keys: "",
            cmd: PaletteCmd::Journal,
        });
        for tool in Tool::PROCESSING.iter().chain(Tool::ANALYSIS.iter()) {
            items.push(PaletteItem {
                label: format!("{}…", tool.name()),
                category: if tool.is_analysis() {
                    "analysis"
                } else {
                    "tool"
                },
                keys: "",
                cmd: PaletteCmd::Tool(*tool),
            });
        }
        // Groups: the visible catalog (capped) plus derived groups.
        let cap = 400;
        for ix in 0..self.catalog.len().min(cap) {
            items.push(PaletteItem {
                label: self.catalog.name(ix).to_string(),
                category: "group",
                keys: "",
                cmd: PaletteCmd::Group(ix),
            });
        }
        for (i, d) in self.derived.iter().enumerate() {
            items.push(PaletteItem {
                label: format!("↳ {}", d.label),
                category: "group",
                keys: "",
                cmd: PaletteCmd::Group(DERIVED_BASE + i),
            });
        }
        items
    }

    fn palette_matches(&self, query: &str) -> Vec<PaletteItem> {
        let query = query.trim();
        self.palette_items()
            .into_iter()
            .filter(|item| query.is_empty() || fuzzy(query, &item.label))
            .take(14)
            .collect()
    }

    pub(crate) fn open_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let theme = self.theme;
        let input = cx.new(|cx| TextInput::new("Type an action, tool or group…", "", theme, cx));
        let subscription = cx.subscribe(&input, |this: &mut Self, _input, event, cx| {
            match event {
                InputEvent::Edited(text) => {
                    if let Some(p) = &mut this.palette {
                        p.query = text.to_string();
                        p.selected = 0;
                    }
                }
                InputEvent::Committed(_) => {
                    this.run_palette_selection(cx);
                }
                InputEvent::Step(dir) => {
                    let n = this
                        .palette
                        .as_ref()
                        .map(|p| this.palette_matches(&p.query).len())
                        .unwrap_or(0);
                    if let Some(p) = &mut this.palette
                        && n > 0
                    {
                        p.selected =
                            (p.selected as i64 + *dir as i64).rem_euclid(n as i64) as usize;
                    }
                }
            }
            cx.notify();
        });
        let focus = input.read(cx).focus_handle(cx);
        self.palette = Some(PaletteState {
            input,
            query: String::new(),
            selected: 0,
            _subscription: subscription,
        });
        cx.notify();
        cx.defer_in(window, move |_this, window, cx| {
            focus.focus(window, cx);
        });
    }

    pub(crate) fn close_palette(&mut self, cx: &mut Context<Self>) {
        self.palette = None;
        cx.notify();
    }

    fn run_palette_selection(&mut self, cx: &mut Context<Self>) {
        let Some(p) = &self.palette else {
            return;
        };
        let items = self.palette_matches(&p.query);
        let Some(item) = items.get(p.selected) else {
            return;
        };
        let cmd = item.cmd.clone();
        self.palette = None;
        self.run_palette_cmd(cmd, cx);
    }

    fn run_palette_cmd(&mut self, cmd: PaletteCmd, cx: &mut Context<Self>) {
        match cmd {
            PaletteCmd::Stage(stage) => self.set_stage(stage, cx),
            PaletteCmd::MarkAll => self.mark_all(true, cx),
            PaletteCmd::MarkNone => self.mark_all(false, cx),
            PaletteCmd::ApplyToMarked => self.apply_params_to_marked(cx),
            PaletteCmd::ResetParams => self.reset_params(cx),
            PaletteCmd::Tool(tool) => self.open_tool(tool, cx),
            PaletteCmd::Fit => {
                self.set_stage(Stage::Fit, cx);
                self.run_fit_now(cx);
            }
            PaletteCmd::ExportBatchCsv => self.export_batch_csv(cx),
            PaletteCmd::SaveProject => self.save_project(cx),
            PaletteCmd::OpenProject => self.open_project(cx),
            PaletteCmd::OpenFolder => self.open_folder(cx),
            PaletteCmd::Theme => self.toggle_theme(cx),
            PaletteCmd::Updates => self.open_updates(cx),
            PaletteCmd::Undo => self.undo(cx),
            PaletteCmd::Redo => self.redo(cx),
            PaletteCmd::Journal => {
                self.journal.open = true;
                cx.notify();
            }
            PaletteCmd::Group(ix) => {
                if ix != NO_ENTRY {
                    self.select_entry(ix, cx);
                    self.sync_param_fields(cx);
                    self.sync_handles(cx);
                }
            }
        }
        cx.notify();
    }

    /// The overlay, when open.
    pub(crate) fn palette_overlay(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement + use<>> {
        let t = self.theme;
        let p = self.palette.as_ref()?;
        let items = self.palette_matches(&p.query);
        let selected = p.selected.min(items.len().saturating_sub(1));
        let mut list = div().flex().flex_col().p_1();
        if items.is_empty() {
            list = list.child(
                div()
                    .px_3()
                    .py_2()
                    .text_size(px(12.))
                    .text_color(t.text_muted)
                    .child("no matches"),
            );
        }
        for (i, item) in items.into_iter().enumerate() {
            let cmd = item.cmd.clone();
            let on = i == selected;
            list = list.child(
                div()
                    .id(("palette-item", i))
                    .h(px(30.))
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_2()
                    .rounded_md()
                    .cursor_pointer()
                    .when(on, |d| {
                        d.bg(gpui::Rgba {
                            a: 0.16,
                            ..t.accent
                        })
                    })
                    .hover(|d| d.bg(t.raised))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _w, cx| {
                        this.palette = None;
                        this.run_palette_cmd(cmd.clone(), cx);
                    }))
                    .child(
                        div()
                            .w(px(64.))
                            .flex_none()
                            .text_size(px(10.5))
                            .text_color(t.text_muted)
                            .child(item.category),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .text_size(px(12.5))
                            .child(SharedString::from(item.label)),
                    )
                    .child(
                        div()
                            .font_family(MONO)
                            .text_size(px(10.5))
                            .text_color(t.text_muted)
                            .child(item.keys),
                    ),
            );
        }
        Some(
            div()
                .id("palette-overlay")
                .key_context("Palette")
                .on_action(cx.listener(|this: &mut Self, _: &PaletteClose, _w, cx| {
                    this.close_palette(cx);
                }))
                .absolute()
                .inset_0()
                .flex()
                .flex_col()
                .items_center()
                .pt(px(90.))
                .bg(gpui::Rgba {
                    r: 0.04,
                    g: 0.05,
                    b: 0.08,
                    a: 0.45,
                })
                .on_mouse_down(
                    gpui::MouseButton::Left,
                    cx.listener(|this, _: &gpui::MouseDownEvent, _w, cx| {
                        this.close_palette(cx);
                    }),
                )
                .child(
                    div()
                        .id("palette-box")
                        .w(px(560.))
                        .flex()
                        .flex_col()
                        .rounded_lg()
                        .bg(t.raised)
                        .border_1()
                        .border_color(t.border)
                        .shadow_lg()
                        .on_mouse_down(
                            gpui::MouseButton::Left,
                            cx.listener(|_this, _: &gpui::MouseDownEvent, _w, cx| {
                                cx.stop_propagation();
                            }),
                        )
                        .child(
                            div()
                                .px_2()
                                .pt_2()
                                .pb_1()
                                .border_b_1()
                                .border_color(t.border)
                                .child(p.input.clone()),
                        )
                        .child(list)
                        .child(
                            div()
                                .px_3()
                                .py_1()
                                .flex()
                                .gap_3()
                                .text_size(px(10.5))
                                .text_color(t.text_muted)
                                .child("↑↓ move")
                                .child("↩ run")
                                .child("esc close"),
                        ),
                ),
        )
    }

    /// Mark (or unmark) every catalog group.
    pub(crate) fn mark_all(&mut self, on: bool, cx: &mut Context<Self>) {
        if on {
            self.selection.extend(0..self.catalog.len());
            self.selection
                .extend((0..self.derived.len()).map(|i| DERIVED_BASE + i));
        } else {
            self.selection.clear();
        }
        self.ensure_compare_loaded(cx);
        self.sync_param_fields(cx);
        cx.notify();
    }
}
