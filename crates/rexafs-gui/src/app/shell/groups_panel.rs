//! Groups panel (Athena's group list, modernized): every catalog file,
//! scan, and derived spectrum is a *group*. The current group (highlighted)
//! fills the inspector; marked groups (checkbox) are what overlays and bulk
//! actions act on. The catalog/scan/filter machinery is unchanged.

use gpui::{
    ClickEvent, Context, IntoElement, ParentElement, SharedString, Styled, div, prelude::*, px,
    uniform_list,
};

use super::{MONO, PlotScope, button};
use crate::app::{
    DERIVED_BASE, DataTab, NavDown, NavExtendDown, NavExtendUp, NavUp, ScanListRow, StudioApp,
    catalog_row_index, scan_list_row,
};
use crate::plotting::trace_rgba;

/// Colour swatch: the trace colour when the group is plotted, a hollow
/// square otherwise, so the list doubles as the legend.
fn swatch(t: &crate::theme::Theme, color: Option<gpui::Rgba>) -> impl IntoElement {
    let mut d = div().w(px(10.)).h(px(10.)).rounded_xs().flex_none();
    d = match color {
        Some(c) => d.bg(c),
        None => d.border_1().border_color(t.border),
    };
    d
}

/// Mark checkbox.
fn checkbox(t: &crate::theme::Theme, on: bool) -> impl IntoElement {
    div()
        .w(px(14.))
        .h(px(14.))
        .flex_none()
        .rounded_sm()
        .border_1()
        .flex()
        .items_center()
        .justify_center()
        .when(on, |d| d.bg(t.accent).border_color(t.accent))
        .when(!on, |d| d.bg(t.raised).border_color(t.border))
        .child(
            div()
                .text_size(px(10.))
                .text_color(t.bg)
                .child(if on { "✓" } else { "" }),
        )
}

/// "frozen" marker: bulk operations skip this group (Athena's Alt+F).
fn frozen_badge(t: &crate::theme::Theme) -> impl IntoElement {
    div()
        .flex_none()
        .px_1()
        .rounded_sm()
        .border_1()
        .border_color(t.border)
        .font_family(MONO)
        .text_size(px(9.5))
        .text_color(t.text_muted)
        .child("frozen")
}

impl StudioApp {
    pub(crate) fn invert_group_marks(&mut self, cx: &mut Context<Self>) {
        let groups =
            (0..self.catalog.len()).chain((0..self.derived.len()).map(|i| DERIVED_BASE + i));
        self.selection = groups.filter(|i| !self.selection.contains(i)).collect();
        self.ensure_compare_loaded(cx);
        self.sync_param_fields(cx);
        cx.notify();
    }

    /// Freeze / thaw the current group.
    pub(crate) fn toggle_frozen(&mut self, cx: &mut Context<Self>) {
        let Some(ix) = self.selected else {
            return;
        };
        if ix == crate::app::NO_ENTRY {
            return;
        }
        let label = self.entry_label(ix);
        if !self.frozen.remove(&ix) {
            self.frozen.insert(ix);
            self.record(format!("freeze {label}"), None);
        } else {
            self.record(format!("thaw {label}"), None);
        }
        cx.notify();
    }

    /// Colour index per plotted group (position in the compare set), so the
    /// list swatch matches the overlay trace.
    fn plotted_color_index(&self) -> Vec<(usize, usize)> {
        let (indices, _) = self.compare_indices();
        indices
            .into_iter()
            .enumerate()
            .map(|(color, ix)| (ix, color))
            .collect()
    }

    /// Plain click = make current (marks untouched); ⌘-click = toggle mark;
    /// ⇧-click = mark the range from the current group.
    pub(crate) fn click_group(
        &mut self,
        ix: usize,
        modifiers: gpui::Modifiers,
        cx: &mut Context<Self>,
    ) {
        if modifiers.shift || modifiers.platform {
            self.click_entry(ix, modifiers, cx);
            return;
        }
        self.select_entry(ix, cx);
        self.sync_param_fields(cx);
        self.sync_handles(cx);
        cx.notify();
    }

    pub(crate) fn toggle_mark(&mut self, ix: usize, cx: &mut Context<Self>) {
        let modifiers = gpui::Modifiers {
            platform: true,
            ..Default::default()
        };
        self.click_entry(ix, modifiers, cx);
    }

    pub(crate) fn groups_panel(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let footer: SharedString = if self.catalog.is_empty() && !self.catalog.scanning {
            "Use + Import to add files or folders".into()
        } else if let Some(filtered) = &self.filtered {
            format!("{} of {} files", filtered.len(), self.catalog.len()).into()
        } else {
            format!(
                "{} files{}",
                self.catalog.len(),
                if self.catalog.scanning {
                    " · scanning…"
                } else {
                    ""
                }
            )
            .into()
        };
        let marked = self.selection.len();
        let scope_on = self.stage_view.scope == PlotScope::Marked;
        div()
            .id("groups-panel")
            .key_context("DataPanel")
            .track_focus(&self.data_focus)
            .on_action(
                cx.listener(|this: &mut Self, _: &crate::app::MarkAllGroups, _, cx| {
                    this.mark_all(true, cx)
                }),
            )
            .on_action(
                cx.listener(|this: &mut Self, _: &crate::app::InvertGroupMarks, _, cx| {
                    this.invert_group_marks(cx)
                }),
            )
            .on_action(cx.listener(|this: &mut Self, _: &NavUp, _window, cx| {
                this.nav_move(-1, false, cx);
            }))
            .on_action(cx.listener(|this: &mut Self, _: &NavDown, _window, cx| {
                this.nav_move(1, false, cx);
            }))
            .on_action(
                cx.listener(|this: &mut Self, _: &NavExtendUp, _window, cx| {
                    this.nav_move(-1, true, cx);
                }),
            )
            .on_action(
                cx.listener(|this: &mut Self, _: &NavExtendDown, _window, cx| {
                    this.nav_move(1, true, cx);
                }),
            )
            .on_action(cx.listener(
                |this: &mut Self, _: &crate::app::ClearCompare, _window, cx| {
                    this.clear_selection(cx);
                },
            ))
            .w(px(248.))
            .h_full()
            .min_h_0()
            .min_w_0()
            .flex_none()
            .flex()
            .flex_col()
            .bg(t.surface)
            .border_r_1()
            .border_color(t.border)
            .child(
                div()
                    .px_3()
                    .pt_2()
                    .pb_1()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(super::section_label(&t, "Groups"))
                    .child(
                        div()
                            .font_family(MONO)
                            .text_size(px(11.))
                            .text_color(t.text_muted)
                            .child(format!("{}", self.catalog.len() + self.derived.len())),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .id("import-files")
                            .px_1p5()
                            .rounded_sm()
                            .text_size(px(11.5))
                            .text_color(t.accent)
                            .cursor_pointer()
                            .hover(|d| d.bg(t.raised))
                            .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                                this.open_folder(cx);
                            }))
                            .child("+ Import"),
                    ),
            )
            .child(div().px_2().pb_1().children(self.filter_input.clone()))
            .child(
                div()
                    .px_2()
                    .pb_1()
                    .flex()
                    .flex_wrap()
                    .gap_1()
                    .child(
                        button(&t, "mark-all-groups", "Select all", false)
                            .on_click(cx.listener(|this, _, _, cx| this.mark_all(true, cx))),
                    )
                    .child(
                        button(&t, "unmark-all-groups", "Deselect all", false)
                            .on_click(cx.listener(|this, _, _, cx| this.clear_selection(cx))),
                    )
                    .child(
                        button(&t, "invert-all-groups", "Invert", false)
                            .on_click(cx.listener(|this, _, _, cx| this.invert_group_marks(cx))),
                    ),
            )
            .child(
                div()
                    .px_3()
                    .pb_1()
                    .text_size(px(10.))
                    .text_color(t.text_muted)
                    .child("All groups, including filtered-out groups"),
            )
            .child(
                div()
                    .flex()
                    .border_b_1()
                    .border_color(t.border)
                    .child(self.data_tab_button("tab-files", "Files", DataTab::Files, cx))
                    .child(self.data_tab_button("tab-scans", "Scans", DataTab::Scans, cx)),
            )
            .child(if self.catalog.is_empty() {
                let is_active =
                    self.selected.is_none() || self.selected == Some(crate::app::NO_ENTRY);
                let color = trace_rgba(&t, 0);
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .py_1()
                    .child(
                        self.group_row(
                            "single-file".into(),
                            crate::app::NO_ENTRY,
                            self.spectrum_label.clone(),
                            self.spectrum
                                .as_ref()
                                .and_then(|s| s.energy.as_ref())
                                .map(|e| format!("{} pts", e.len()))
                                .unwrap_or_default(),
                            is_active,
                            true,
                            Some(color),
                            false,
                            cx,
                        ),
                    )
                    .child(self.derived_list(cx))
                    .into_any_element()
            } else {
                let list = match self.data_tab {
                    DataTab::Files => self.file_list(cx).into_any_element(),
                    DataTab::Scans => self.scan_list(cx).into_any_element(),
                };
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .child(self.derived_list(cx))
                    .child(list)
                    .into_any_element()
            })
            .child(
                div()
                    .px_2()
                    .py_2()
                    .flex()
                    .flex_wrap()
                    .gap_1p5()
                    .border_t_1()
                    .border_color(t.border)
                    .child(
                        button(&t, "merge-marked", "Merge marked", false).on_click(cx.listener(
                            |this, _: &ClickEvent, _window, cx| {
                                this.merge_selection(cx);
                            },
                        )),
                    )
                    .child(
                        button(&t, "align-marked", "Align…", false).on_click(cx.listener(
                            |this, _: &ClickEvent, _window, cx| {
                                this.open_tool(super::tools::Tool::Align, cx);
                            },
                        )),
                    )
                    .child(
                        button(&t, "compare-scope", "Compare", scope_on).on_click(cx.listener(
                            |this, _: &ClickEvent, _window, cx| {
                                this.stage_view.scope = match this.stage_view.scope {
                                    PlotScope::Current => PlotScope::Marked,
                                    PlotScope::Marked => PlotScope::Current,
                                };
                                this.stage_view_changed(cx);
                            },
                        )),
                    ),
            )
            .child(if self.catalog.is_empty() {
                div().into_any_element()
            } else {
                let cmd = |id: &'static str,
                           label: &'static str,
                           enabled: bool,
                           action: fn(&mut Self, &mut Context<Self>)| {
                    div()
                        .id(id)
                        .px_1()
                        .rounded_sm()
                        .text_size(px(11.))
                        .text_color(if enabled { t.accent } else { t.text_muted })
                        .when(enabled, |d| d.cursor_pointer().hover(|d| d.bg(t.raised)))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                            if enabled {
                                action(this, cx);
                            }
                        }))
                        .child(label)
                };
                div()
                    .px_2()
                    .pb_1()
                    .flex()
                    .items_center()
                    .gap_1()
                    .text_size(px(11.))
                    .text_color(t.text_muted)
                    .child("mark:")
                    .flex_wrap()
                    .child(cmd(
                        "sel-scan",
                        "scan",
                        self.selected.is_some(),
                        |this, cx| this.select_active_scan(cx),
                    ))
                    .child(cmd(
                        "sel-tenth",
                        "every 10th",
                        self.selection.len() > 1,
                        |this, cx| this.thin_selection(cx),
                    ))
                    .child(cmd(
                        "sel-filter",
                        "filter",
                        self.filtered.is_some(),
                        |this, cx| this.select_filter_results(cx),
                    ))
                    .child(cmd(
                        "sel-freeze",
                        if self.selected.is_some_and(|ix| self.frozen.contains(&ix)) {
                            "thaw"
                        } else {
                            "freeze"
                        },
                        self.selected.is_some_and(|ix| ix != crate::app::NO_ENTRY),
                        |this, cx| this.toggle_frozen(cx),
                    ))
                    .child(div().flex_1())
                    .child(cmd(
                        "sel-clear",
                        "clear",
                        !self.selection.is_empty(),
                        |this, cx| this.clear_selection(cx),
                    ))
                    .into_any_element()
            })
            .child(
                div()
                    .px_3()
                    .py_1()
                    .text_size(px(11.))
                    .font_family(MONO)
                    .text_color(if self.catalog.scanning {
                        t.warn
                    } else {
                        t.text_muted
                    })
                    .border_t_1()
                    .border_color(t.border)
                    .child(format!("{footer} · {marked} marked")),
            )
    }

    /// Derived (merged / tool output) groups, indented under their sources.
    fn derived_list(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let entity = cx.entity();
        uniform_list(
            "additional-groups",
            self.derived.len(),
            move |range, _, app| entity.update(app, |this, cx| this.derived_rows(range, cx)),
        )
        .track_scroll(&self.derived_scroll)
        .w_full()
        .min_w_0()
        .h(px(self.derived.len().min(6) as f32 * 27.))
        .flex_none()
    }

    fn derived_rows(
        &self,
        range: std::ops::Range<usize>,
        cx: &mut Context<Self>,
    ) -> Vec<gpui::AnyElement> {
        let t = self.theme;
        let colors = self.plotted_color_index();
        range
            .filter_map(|i| self.derived.get(i).map(|d| (i, d)))
            .map(|(i, d)| {
                let ix = DERIVED_BASE + i;
                let is_active = self.selected == Some(ix);
                let marked = self.selection.contains(&ix);
                let color = colors
                    .iter()
                    .find(|(k, _)| *k == ix)
                    .map(|(_, c)| trace_rgba(&t, *c));
                let label: SharedString = format!("↳ {}", self.entry_label(ix)).into();
                let meta = if d.source.is_some() {
                    String::new()
                } else {
                    format!("{} pts", d.energy.len())
                };
                self.group_row(
                    ("derived", i).into(),
                    ix,
                    label,
                    meta,
                    is_active,
                    marked,
                    color,
                    true,
                    cx,
                )
                .into_any_element()
            })
            .collect()
    }

    /// One group row: [mark] [swatch] name … meta [✕ for derived].
    #[allow(clippy::too_many_arguments)]
    fn group_row(
        &self,
        id: gpui::ElementId,
        ix: usize,
        label: SharedString,
        meta: String,
        is_active: bool,
        marked: bool,
        color: Option<gpui::Rgba>,
        removable: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let t = self.theme;
        let real = ix != crate::app::NO_ENTRY;
        div()
            .id(id)
            .h(px(27.))
            .w_full()
            .min_w_0()
            .px_1p5()
            .flex()
            .items_center()
            .gap_2()
            .rounded_md()
            .cursor_pointer()
            .when(is_active, |d| {
                d.bg(gpui::Rgba {
                    a: 0.16,
                    ..t.accent
                })
            })
            .when(!is_active, |d| d.hover(|d| d.bg(t.raised)))
            .on_click(cx.listener(move |this, ev: &ClickEvent, window, cx| {
                window.focus(&this.data_focus, cx);
                if real {
                    this.click_group(ix, ev.modifiers(), cx);
                }
            }))
            .child(
                div()
                    .id("mark")
                    .flex_none()
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        cx.stop_propagation();
                        window.focus(&this.data_focus, cx);
                        if real {
                            this.toggle_mark(ix, cx);
                        }
                    }))
                    .child(checkbox(&t, marked)),
            )
            .child(swatch(&t, color))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .when(is_active, |d| d.font_weight(gpui::FontWeight::MEDIUM))
                    .child(label),
            )
            .children(self.frozen.contains(&ix).then(|| frozen_badge(&t)))
            .child(
                div()
                    .flex_none()
                    .font_family(MONO)
                    .text_size(px(10.5))
                    .text_color(t.text_muted)
                    .child(meta),
            )
            .when(removable, |row| {
                row.child(
                    div()
                        .id("remove")
                        .px_1()
                        .text_size(px(11.))
                        .text_color(t.text_muted)
                        .cursor_pointer()
                        .hover(|d| d.text_color(t.error))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                            cx.stop_propagation();
                            this.remove_derived(ix - DERIVED_BASE, cx);
                        }))
                        .child("✕"),
                )
            })
    }

    pub(crate) fn file_list(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let entity = cx.entity();
        let active = self.selected;
        let selection = self.selection.clone();
        let filtered = self.filtered.clone();
        let frozen = self.frozen.clone();
        let colors = self.plotted_color_index();
        let count = filtered
            .as_ref()
            .map(|f| f.len())
            .unwrap_or(self.catalog.len());
        uniform_list("catalog-files", count, move |range, _window, app| {
            let mut rows = Vec::with_capacity(range.len());
            for row in range {
                let (ix, name) = {
                    let state = entity.read(app);
                    let Some(ix) = catalog_row_index(
                        filtered.as_deref().map(Vec::as_slice),
                        row,
                        state.catalog.len(),
                    ) else {
                        continue;
                    };
                    let name: SharedString = state.catalog.name(ix).to_string().into();
                    (ix, name)
                };
                let is_active = active == Some(ix);
                let marked = selection.contains(&ix);
                let is_frozen = frozen.contains(&ix);
                let color = colors
                    .iter()
                    .find(|(k, _)| *k == ix)
                    .map(|(_, c)| trace_rgba(&t, *c));
                let row_entity = entity.clone();
                let mark_entity = entity.clone();
                rows.push(
                    div()
                        .id(ix)
                        .h(px(27.))
                        .mx_1p5()
                        .px_1p5()
                        .flex()
                        .items_center()
                        .gap_2()
                        .rounded_md()
                        .cursor_pointer()
                        .when(is_active, |d| {
                            d.bg(gpui::Rgba {
                                a: 0.16,
                                ..t.accent
                            })
                        })
                        .when(!is_active, |d| d.hover(|d| d.bg(t.raised)))
                        .on_click(move |ev: &ClickEvent, window, app| {
                            let modifiers = ev.modifiers();
                            let focus = row_entity.read(app).data_focus.clone();
                            window.focus(&focus, app);
                            row_entity.update(app, |this, cx| this.click_group(ix, modifiers, cx));
                        })
                        .child(
                            div()
                                .id("mark")
                                .flex_none()
                                .on_click(move |_: &ClickEvent, _window, app| {
                                    app.stop_propagation();
                                    mark_entity.update(app, |this, cx| this.toggle_mark(ix, cx));
                                })
                                .child(checkbox(&t, marked)),
                        )
                        .child(swatch(&t, color))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .when(is_active, |d| d.font_weight(gpui::FontWeight::MEDIUM))
                                .child(name),
                        )
                        .children(is_frozen.then(|| frozen_badge(&t))),
                );
            }
            rows
        })
        .track_scroll(&self.file_scroll)
        .flex_1()
        .min_h_0()
    }

    pub(crate) fn scan_list(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let entity = cx.entity();
        let active = self.active_scan;
        let selected = self.selected;
        let expanded_scan = self.expanded_scan;
        let expanded = expanded_scan.and_then(|scan_ix| {
            self.catalog
                .scans
                .get(scan_ix)
                .map(|scan| (scan_ix, scan.len))
        });
        let count = self.catalog.scans.len() + expanded.map(|(_, len)| len).unwrap_or(0);
        uniform_list("catalog-scans", count, move |range, _window, app| {
            let mut rows = Vec::with_capacity(range.len());
            for row in range {
                let Some(item) = scan_list_row(row, entity.read(app).catalog.scans.len(), expanded)
                else {
                    continue;
                };
                match item {
                    ScanListRow::Header(scan_ix) => {
                        let (label, meta): (SharedString, SharedString) = {
                            let scan = &entity.read(app).catalog.scans[scan_ix];
                            (
                                scan.label.clone().into(),
                                format!("{} frames", scan.len).into(),
                            )
                        };
                        let is_active = active == Some(scan_ix);
                        let is_expanded = expanded_scan == Some(scan_ix);
                        let row_entity = entity.clone();
                        let button_entity = entity.clone();
                        rows.push(
                            div()
                                .id(("scan-header", scan_ix))
                                .h(px(27.))
                                .mx_1p5()
                                .px_1p5()
                                .gap_1p5()
                                .flex()
                                .items_center()
                                .rounded_md()
                                .overflow_hidden()
                                .when(is_active, |d| {
                                    d.bg(gpui::Rgba {
                                        a: 0.16,
                                        ..t.accent
                                    })
                                })
                                .when(!is_active, |d| d.hover(|d| d.bg(t.raised)))
                                .cursor_pointer()
                                .on_click(move |ev: &ClickEvent, _window, app| {
                                    let modifiers = ev.modifiers();
                                    let double = ev.click_count() >= 2;
                                    row_entity.update(app, |this, cx| {
                                        if modifiers.shift || modifiers.platform {
                                            this.select_scan_range(scan_ix, cx);
                                        } else if double {
                                            this.open_scan(scan_ix, cx);
                                        } else {
                                            this.active_scan = Some(scan_ix);
                                            this.expanded_scan = (this.expanded_scan
                                                != Some(scan_ix))
                                            .then_some(scan_ix);
                                            cx.notify();
                                        }
                                    });
                                })
                                .child(div().text_color(t.text_muted).child(if is_expanded {
                                    "▾"
                                } else {
                                    "▸"
                                }))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .overflow_hidden()
                                        .whitespace_nowrap()
                                        .text_ellipsis()
                                        .child(label),
                                )
                                .child(
                                    div()
                                        .flex_none()
                                        .font_family(MONO)
                                        .text_size(px(10.5))
                                        .text_color(t.accent)
                                        .child(meta),
                                )
                                .child(
                                    div()
                                        .id(("scan-series", scan_ix))
                                        .flex_none()
                                        .px_1()
                                        .rounded_sm()
                                        .text_size(px(10.5))
                                        .text_color(t.accent)
                                        .border_1()
                                        .border_color(t.border)
                                        .hover(|d| d.bg(t.surface))
                                        .cursor_pointer()
                                        .on_click(move |_: &ClickEvent, _window, app| {
                                            app.stop_propagation();
                                            button_entity.update(app, |this, cx| {
                                                this.open_scan(scan_ix, cx)
                                            });
                                        })
                                        .child("series"),
                                ),
                        );
                    }
                    ScanListRow::Member { scan, offset } => {
                        let catalog_ix = {
                            let scan = &entity.read(app).catalog.scans[scan];
                            scan.start + offset
                        };
                        let label: SharedString =
                            entity.read(app).catalog.name(catalog_ix).to_string().into();
                        let is_active = selected == Some(catalog_ix);
                        let member_entity = entity.clone();
                        rows.push(
                            div()
                                .id(("scan-member", catalog_ix))
                                .h(px(27.))
                                .ml_5()
                                .mr_1p5()
                                .px_1p5()
                                .flex()
                                .items_center()
                                .rounded_md()
                                .overflow_hidden()
                                .when(is_active, |d| {
                                    d.bg(gpui::Rgba {
                                        a: 0.16,
                                        ..t.accent
                                    })
                                })
                                .when(!is_active, |d| d.hover(|d| d.bg(t.raised)))
                                .cursor_pointer()
                                .on_click(move |ev: &ClickEvent, window, app| {
                                    let modifiers = ev.modifiers();
                                    let focus = member_entity.read(app).data_focus.clone();
                                    window.focus(&focus, app);
                                    member_entity.update(app, |this, cx| {
                                        this.active_scan = Some(scan);
                                        this.click_group(catalog_ix, modifiers, cx);
                                    });
                                })
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .overflow_hidden()
                                        .whitespace_nowrap()
                                        .text_ellipsis()
                                        .child(label),
                                ),
                        );
                    }
                }
            }
            rows
        })
        .track_scroll(&self.scan_scroll)
        .flex_1()
        .min_h_0()
    }

    pub(crate) fn data_tab_button(
        &self,
        id: &'static str,
        label: &'static str,
        tab: DataTab,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let t = self.theme;
        let active = self.data_tab == tab;
        div()
            .id(id)
            .flex_1()
            .py_1()
            .flex()
            .justify_center()
            .text_size(px(11.5))
            .cursor_pointer()
            .when(active, |d| {
                d.text_color(t.accent).border_b_2().border_color(t.accent)
            })
            .when(!active, |d| d.text_color(t.text_muted))
            .hover(|d| d.bg(t.raised))
            .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                this.data_tab = tab;
                cx.notify();
            }))
            .child(label)
    }
}
