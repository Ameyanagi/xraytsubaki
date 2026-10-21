//! Groups panel (Athena's group list, modernized): every catalog file,
//! scan, and derived spectrum is a *group*. The current group (highlighted)
//! fills the inspector; marked groups (checkbox) are what overlays and bulk
//! actions act on. Source stacks and Results share one virtualized list.

use gpui::{
    ClickEvent, Context, IntoElement, ParentElement, Styled, div, prelude::*, px, uniform_list,
};

use super::{MONO, PlotScope, button, controls::icon};
use crate::app::group_rows::{self, Row};
use crate::app::{DERIVED_BASE, NavDown, NavExtendDown, NavExtendUp, NavUp, StudioApp};
use crate::icons::Icon;
use crate::plotting::trace_rgba;

/// Merge reasons read "<group label>: <message>". When the label already ends
/// with the message's opening phrase (the label carries "· quantity unconfirmed"
/// and the message opens with "Quantity unconfirmed:"), show the phrase once.
fn dedupe_reason(reason: &str) -> String {
    if let Some((label, message)) = reason.split_once(": ")
        && let Some((phrase, rest)) = message.split_once(':')
        && label.to_lowercase().ends_with(&phrase.to_lowercase())
    {
        return format!("{label}:{rest}");
    }
    reason.to_string()
}

/// Stable group colour, also used by its plot trace and legend.
fn swatch(color: gpui::Rgba) -> impl IntoElement {
    div()
        .w(px(10.))
        .h(px(10.))
        .rounded_xs()
        .flex_none()
        .bg(color)
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
        .children(on.then(|| icon(t, Icon::Check).size(px(11.)).text_color(t.bg)))
}

#[derive(Clone)]
struct RowTooltip {
    text: String,
    theme: crate::theme::Theme,
}
impl gpui::Render for RowTooltip {
    fn render(&mut self, _: &mut gpui::Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .max_w(px(500.))
            .p_2()
            .bg(self.theme.raised)
            .text_color(self.theme.text)
            .text_size(px(12.))
            .child(self.text.clone())
    }
}
struct SidebarResize;
impl gpui::Render for SidebarResize {
    fn render(&mut self, _: &mut gpui::Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

pub(crate) fn processing_locked(
    current: Option<usize>,
    frozen: &std::collections::BTreeSet<usize>,
) -> bool {
    current.is_some_and(|ix| frozen.contains(&ix))
}

pub(crate) fn restore_standalone_lock(
    frozen: &mut std::collections::BTreeSet<usize>,
    state: &crate::group_identity::GroupState,
    id: &crate::group_identity::GroupId,
) {
    if state.frozen.contains(id) {
        frozen.insert(crate::app::NO_ENTRY);
    } else {
        frozen.remove(&crate::app::NO_ENTRY);
    }
}

pub(crate) fn migrate_standalone_lock(
    frozen: &mut std::collections::BTreeSet<usize>,
    index: usize,
) {
    if frozen.remove(&crate::app::NO_ENTRY) {
        frozen.insert(index);
    }
}

impl StudioApp {
    pub(crate) fn group_row_mode(&self, ix: usize) -> crate::params::DetectionMode {
        let mode = self.effective_params(ix).import.mode;
        if mode == crate::params::DetectionMode::Auto && self.current_group_index() == Some(ix) {
            self.import_preview
                .as_ref()
                .map_or(mode, |p| p.resolved.mode)
        } else {
            mode
        }
    }

    pub(crate) fn default_group_row_label(&self, row: Row) -> String {
        let ix = row.group().unwrap_or(crate::app::NO_ENTRY);
        if matches!(row, Row::Child { .. }) {
            self.group_row_mode(ix).label().to_string()
        } else {
            ix.checked_sub(DERIVED_BASE)
                .and_then(|i| self.derived.get(i))
                .map(|d| d.display_label())
                .unwrap_or_else(|| self.default_entry_label(ix))
        }
    }

    pub(crate) fn group_row_label(&self, row: Row) -> String {
        let ix = row.group().unwrap_or(crate::app::NO_ENTRY);
        self.group_state
            .display_label(self.peek_group_id(ix).as_ref(), || {
                self.default_group_row_label(row)
            })
    }

    pub(crate) fn invert_group_marks(&mut self, cx: &mut Context<Self>) {
        self.interaction_rows().invert_shown(&mut self.selection);
        self.ensure_compare_loaded(cx);
        self.sync_param_fields(cx);
        cx.notify();
    }

    /// Lock / unlock processing for the current group.
    pub(crate) fn toggle_frozen(&mut self, cx: &mut Context<Self>) {
        let Some(ix) = self.current_group_index() else {
            return;
        };
        self.toggle_group_lock(ix, cx);
    }

    pub(crate) fn toggle_group_lock(&mut self, ix: usize, cx: &mut Context<Self>) {
        let Some(id) = self.group_id(ix) else { return };
        let label = self.entry_label(ix);
        if !self.frozen.remove(&ix) {
            self.frozen.insert(ix);
            self.group_state.frozen.insert(id);
            self.record(format!("lock processing {label}"), None);
        } else {
            self.group_state.frozen.remove(&id);
            self.record(format!("unlock processing {label}"), None);
        }
        cx.notify();
    }

    pub(crate) fn group_plot_color(&self, ix: usize) -> Option<ruviz::render::Color> {
        self.peek_group_id(ix)
            .as_ref()
            .and_then(|id| self.group_state.plot_colors.get(id))
            .map(|c| c.color(&self.theme))
    }

    pub(crate) fn group_color_index(&self, ix: usize) -> usize {
        self.peek_group_id(ix)
            .as_ref()
            .map(|id| {
                self.group_state
                    .colors
                    .get(id)
                    .map(|c| usize::from(*c % 8))
                    .unwrap_or_else(|| group_rows::color_index(id))
            })
            .unwrap_or(0)
    }

    fn stack_id(&self, ix: usize) -> Option<crate::group_identity::GroupId> {
        let path = if ix < self.catalog.len() {
            Some(self.catalog.path(ix))
        } else {
            ix.checked_sub(DERIVED_BASE)
                .and_then(|i| self.derived.get(i))
                .and_then(|d| d.source.clone())
        };
        if let Some(path) = path {
            if let Some(primary) = self.catalog.find_by_canonical_path(&path) {
                return Some(self.group_registry.register_source(
                    Some(primary),
                    path,
                    self.effective_params(primary).import.mode,
                    &self.project_source_origins,
                ));
            }
            if let Some((_, _, id)) = self
                .standalone_source
                .as_ref()
                .filter(|(p, _, _)| p == &path)
            {
                return Some(id.clone());
            }
            // Orphan stacks also retain their disclosure when their first channel changes.
            return Some(crate::group_identity::GroupId::source(
                &path,
                crate::params::DetectionMode::Auto,
            ));
        }
        self.group_id(ix)
    }

    pub(crate) fn interaction_rows(&self) -> group_rows::Rows {
        self.group_rows()
    }

    pub(crate) fn group_rows(&self) -> group_rows::Rows {
        self.rows_for_filter(self.filter_reveal.is_some())
    }

    fn rows_for_filter(&self, reveal: bool) -> group_rows::Rows {
        let mut marks = std::collections::BTreeSet::new();
        if reveal {
            marks.clone_from(&self.selection);
            marks.extend(self.reveal_current);
            marks.extend(
                self.intake
                    .reveal
                    .iter()
                    .filter_map(|id| self.group_registry.index(id)),
            );
        }
        let mut excluded = self.group_registry.excluded_indices();
        excluded.extend(self.live_hidden_sources());
        group_rows::build_rows_active(
            &self.catalog,
            &self.derived,
            |g| {
                self.stack_id(g)
                    .and_then(|id| self.expanded_sources.get(&id).copied())
            },
            self.filtered.clone(),
            &self.filter_text,
            self.standalone_path(),
            &marks,
            &excluded,
        )
    }

    pub(crate) fn toggle_filter_reveal(&mut self, cx: &mut Context<Self>) {
        if let Some(expanded) = self.filter_reveal.take() {
            self.expanded_sources = expanded;
            self.reveal_current = None;
            self.intake.reveal.clear();
        } else {
            self.filter_reveal = Some(self.expanded_sources.clone());
        }
        cx.notify();
    }

    pub(crate) fn clear_hidden_marks(&mut self, cx: &mut Context<Self>) {
        let rows = self.interaction_rows();
        self.selection.retain(|&g| !rows.hidden_by_filter(g));
        self.ensure_compare_loaded(cx);
        self.sync_param_fields(cx);
        cx.notify();
    }

    fn focus_stack(&mut self, expand: bool, cx: &mut Context<Self>) {
        let rows = self.group_rows();
        let Some(row) = self
            .focus_group
            .and_then(|g| rows.row_index(g))
            .and_then(|i| rows.row_at(i))
        else {
            return;
        };
        match row {
            Row::Primary {
                group,
                extra_channels,
                ..
            } if extra_channels > 0 => {
                if let Some(id) = self.stack_id(group) {
                    self.expanded_sources.insert(id, expand);
                }
            }
            Row::Child { parent, .. } if !expand => self.focus_group = Some(parent),
            _ => {}
        }
        cx.notify();
    }

    fn leave_filter(&mut self, window: &mut gpui::Window, cx: &mut Context<Self>) {
        window.focus(&self.data_focus, cx);
        let rows = self.interaction_rows();
        if let Some(group) = self
            .current_group_index()
            .filter(|&g| rows.row_index(g).is_some())
            .or_else(|| rows.shown().next())
        {
            self.click_group(group, Default::default(), cx);
        }
    }

    pub(crate) fn reveal_group_row(&mut self, ix: usize) {
        self.expand_group_stack(ix);
        self.scroll_group_row(ix);
    }

    pub(crate) fn expand_group_stack(&mut self, ix: usize) {
        if let Some(path) = ix
            .checked_sub(DERIVED_BASE)
            .and_then(|i| self.derived.get(i))
            .and_then(|d| d.source.as_ref())
        {
            let parent = self
                .standalone_path()
                .filter(|p| *p == path)
                .map(|_| crate::app::NO_ENTRY)
                .or_else(|| self.catalog.find_by_canonical_path(path))
                .or_else(|| {
                    self.derived
                        .iter()
                        .position(|d| d.source.as_ref() == Some(path))
                        .map(|i| DERIVED_BASE + i)
                });
            if let Some(parent) = parent.filter(|&p| p != ix)
                && let Some(id) = self.stack_id(parent)
            {
                self.expanded_sources.insert(id, true);
            }
        }
    }

    pub(crate) fn scroll_group_row(&mut self, ix: usize) {
        if let Some(row) = self.interaction_rows().scroll_row(ix, None) {
            self.file_scroll
                .scroll_to_item(row, gpui::ScrollStrategy::Nearest);
        }
    }

    fn finish_groups_resize(&mut self, cx: &mut Context<Self>) {
        if self.groups_resize.take().is_some() {
            if let Err(error) = self.structure.settings.save() {
                self.record_job_error("sidebar width", error);
            }
            cx.notify();
        }
    }

    pub(crate) fn click_group(
        &mut self,
        ix: usize,
        modifiers: gpui::Modifiers,
        cx: &mut Context<Self>,
    ) {
        self.click_entry(ix, modifiers, cx);
    }

    pub(crate) fn toggle_mark(&mut self, ix: usize, cx: &mut Context<Self>) {
        let modifiers = gpui::Modifiers {
            platform: true,
            ..Default::default()
        };
        self.click_entry(ix, modifiers, cx);
    }

    pub(crate) fn groups_panel(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let merge_reason = self.merge_disabled_reason();
        let t = self.theme;
        let (marked, hidden, collapsed) = self.interaction_rows().mark_counts(&self.selection);
        let mut footer = format!("{marked} marked");
        if hidden > 0 {
            footer.push_str(&format!(" · {hidden} filtered"));
        }
        if collapsed > 0 {
            footer.push_str(&format!(" · {collapsed} collapsed"));
        }
        let scope_on = self.stage_view.scope == PlotScope::Marked;
        div()
            .id("groups-panel")
            .key_context("DataPanel")
            .track_focus(&self.data_focus)
            .on_action(
                cx.listener(|this, _: &crate::app::RenameGroup, window, cx| {
                    if let Some(ix) = this
                        .focus_group
                        .filter(|&ix| this.interaction_rows().row_index(ix).is_some())
                    {
                        this.start_group_rename(ix, window, cx);
                    }
                }),
            )
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
            .on_action(
                cx.listener(|this, _: &crate::app::ToggleFocusedMark, _, cx| {
                    if let Some(ix) = this
                        .focus_group
                        .filter(|&g| this.interaction_rows().row_index(g).is_some())
                    {
                        this.toggle_mark(ix, cx);
                    }
                }),
            )
            .on_action(
                cx.listener(|this, _: &crate::app::CollapseFocusedStack, _, cx| {
                    this.focus_stack(false, cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &crate::app::ExpandFocusedStack, _, cx| {
                    this.focus_stack(true, cx)
                }),
            )
            .relative()
            .w(px(self.structure.settings.groups_panel_width()))
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
                            .child(format!(
                                "{}",
                                self.catalog.len()
                                    - self
                                        .group_registry
                                        .excluded_indices()
                                        .iter()
                                        .filter(|&&g| g < DERIVED_BASE)
                                        .count()
                                    + self.derived.len()
                                    - self.live_hidden_sources().len()
                                    + usize::from(self.standalone_path().is_some())
                            )),
                    )
                    .child(div().flex_1())
                    .child(
                        super::controls::icon_button(
                            &t,
                            "import-files",
                            crate::icons::Icon::Plus,
                            "Import files or folders",
                            false,
                        )
                        .on_click(cx.listener(|app, _, _, cx| app.open_folder(cx))),
                    )
                    .child(
                        super::controls::icon_button(
                            &t,
                            "group-actions",
                            crate::icons::Icon::More,
                            "Group actions",
                            false,
                        )
                        .on_click(cx.listener(|app, event, window, cx| {
                            app.open_chrome_menu(super::controls::Menu::Groups, event, window, cx)
                        })),
                    ),
            )
            .child(
                div()
                    .key_context("GroupFilter")
                    .px_2()
                    .pb_1()
                    .on_action(
                        cx.listener(|this, _: &crate::app::LeaveFilter, window, cx| {
                            this.leave_filter(window, cx)
                        }),
                    )
                    .on_action(
                        cx.listener(|this, _: &crate::app::EscapeFilter, window, cx| {
                            if this.filter_text.is_empty() {
                                window.focus(&this.data_focus, cx);
                            } else {
                                this.filter_text.clear();
                                if let Some(input) = &this.filter_input {
                                    input.update(cx, |input, cx| input.set_text("", cx));
                                }
                                this.apply_filter(cx);
                            }
                        }),
                    )
                    .children(self.filter_input.clone()),
            )
            .children(self.live_sources_toggle(cx))
            .children(self.pending_imports(cx))
            .child(self.file_list(cx))
            .child(
                div()
                    .px_3()
                    .pt_2()
                    .text_size(px(11.))
                    .text_color(t.text_muted)
                    .child(footer),
            )
            .when(
                hidden + collapsed > 0
                    || self.filter_reveal.is_some()
                    || self
                        .current_group_index()
                        .is_some_and(|g| self.interaction_rows().row_index(g).is_none()),
                |d| {
                    d.child(
                        div()
                            .px_2()
                            .py_1()
                            .flex()
                            .flex_wrap()
                            .gap_1()
                            .child(
                                button(
                                    &t,
                                    "show-marked",
                                    if self.filter_reveal.is_some() {
                                        "Back to filter"
                                    } else {
                                        "Show marked"
                                    },
                                    false,
                                )
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.toggle_filter_reveal(cx)),
                                ),
                            )
                            .child(
                                button(&t, "clear-hidden", "Clear hidden marks", false).on_click(
                                    cx.listener(|this, _, _, cx| this.clear_hidden_marks(cx)),
                                ),
                            )
                            .when(
                                self.current_group_index().is_some_and(|g| {
                                    self.interaction_rows().row_index(g).is_none()
                                }),
                                |d| {
                                    d.child(
                                        button(&t, "reveal-current", "Reveal current", false)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.filter_reveal.get_or_insert_with(|| {
                                                    this.expanded_sources.clone()
                                                });
                                                this.reveal_current = this.current_group_index();
                                                if let Some(ix) = this.reveal_current {
                                                    this.reveal_group_row(ix);
                                                }
                                                cx.notify();
                                            })),
                                    )
                                },
                            ),
                    )
                },
            )
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
                        button(&t, "merge-marked", format!("Merge {marked}…"), false)
                            .when_some(merge_reason.clone(), |d, reason| {
                                let tip = RowTooltip {
                                    text: reason,
                                    theme: t,
                                };
                                d.opacity(0.45)
                                    .cursor_default()
                                    .tooltip(move |_, cx| cx.new(|_| tip.clone()).into())
                            })
                            .when(merge_reason.is_none(), |d| {
                                d.on_click(cx.listener(|this, event: &ClickEvent, window, cx| {
                                    this.open_merge_review(event, window, cx);
                                }))
                            }),
                    )
                    .child(
                        button(&t, "align-marked", "Align current…", false).on_click(cx.listener(
                            |this, _: &ClickEvent, _window, cx| {
                                this.open_tool(super::tools::Tool::Align, cx);
                            },
                        )),
                    )
                    .child(
                        button(&t, "compare-scope", "Compare", scope_on).on_click(cx.listener(
                            |this, _: &ClickEvent, _window, cx| {
                                this.stage_view.scope = PlotScope::Marked;
                                this.stage_view_changed(cx);
                            },
                        )),
                    )
                    .when(marked > 0, |d| {
                        d.child(
                            super::controls::icon_button(
                                &t,
                                "remove-marked",
                                crate::icons::Icon::Trash,
                                format!("Remove {marked} marked groups…"),
                                false,
                            )
                            .on_click(
                                cx.listener(|app, _, window, cx| {
                                    app.open_remove_marked(window, cx)
                                }),
                            ),
                        )
                    }),
            )
            .children(merge_reason.filter(|_| marked >= 2).map(|reason| {
                div()
                    .flex_none()
                    .px_2()
                    .pb_1()
                    .text_size(px(11.))
                    .text_color(t.warn)
                    .child(dedupe_reason(&reason))
            }))
            .child(
                div()
                    .id("groups-width-handle")
                    .absolute()
                    .right_0()
                    .top_0()
                    .bottom_0()
                    .w(px(5.))
                    .cursor(gpui::CursorStyle::ResizeLeftRight)
                    .hover(|d| d.bg(t.accent))
                    .on_mouse_down(
                        gpui::MouseButton::Left,
                        cx.listener(|this, ev: &gpui::MouseDownEvent, _, cx| {
                            this.groups_resize =
                                Some((ev.position.x, this.structure.settings.groups_panel_width()));
                            cx.stop_propagation();
                        }),
                    )
                    .on_drag(SidebarResize, |_, _, _, cx| cx.new(|_| SidebarResize))
                    .on_drag_move::<SidebarResize>(cx.listener(
                        |this, ev: &gpui::DragMoveEvent<SidebarResize>, _, cx| {
                            if let Some((start, width)) = this.groups_resize {
                                this.structure.settings.groups_panel_width =
                                    Some(crate::settings::clamp_groups_panel_width(
                                        width + f32::from(ev.event.position.x - start),
                                    ));
                                cx.notify();
                            }
                        },
                    ))
                    .on_mouse_up(
                        gpui::MouseButton::Left,
                        cx.listener(|this, _, _, cx| this.finish_groups_resize(cx)),
                    )
                    .on_mouse_up_out(
                        gpui::MouseButton::Left,
                        cx.listener(|this, _, _, cx| this.finish_groups_resize(cx)),
                    ),
            )
    }

    /// One virtualized scroll surface for every source, channel and result.
    pub(crate) fn file_list(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let entity = cx.entity();
        let rows = self.group_rows();
        uniform_list("catalog-groups", rows.row_count(), move |range, _, app| {
            entity.update(app, |this, cx| {
                range
                    .filter_map(|i| rows.row_at(i))
                    .map(|row| {
                        if let Row::Header(_) = row {
                            div()
                                .h(px(group_rows::ROW_HEIGHT))
                                .px_3()
                                .pb_1()
                                .flex()
                                .items_end()
                                .text_color(this.theme.text_muted)
                                .child("Results")
                                .into_any_element()
                        } else {
                            this.group_row(row, cx).into_any_element()
                        }
                    })
                    .collect()
            })
        })
        .track_scroll(&self.file_scroll)
        .flex_1()
        .min_h_0()
    }

    fn group_row(&self, row: Row, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let ix = row.group().unwrap_or(crate::app::NO_ENTRY);
        let real = ix != crate::app::NO_ENTRY;
        let active = self.current_group_index() == Some(ix);
        let child = matches!(row, Row::Child { .. });
        let (expanded, extra) = match row {
            Row::Primary {
                expanded,
                extra_channels,
                ..
            } => (expanded, extra_channels),
            _ => (false, 0),
        };
        let derived = ix
            .checked_sub(DERIVED_BASE)
            .and_then(|i| self.derived.get(i));
        let path = derived
            .and_then(|d| d.source.clone())
            .or_else(|| (ix < self.catalog.len()).then(|| self.catalog.path(ix)))
            .or_else(|| {
                (!real)
                    .then(|| self.standalone_path().map(ToOwned::to_owned))
                    .flatten()
            });
        let mode = self.group_row_mode(ix);
        let tag = group_rows::kind(
            mode,
            if matches!(row, Row::Result { .. }) {
                derived
            } else {
                None
            },
        );
        let full_label = self.entry_label(ix);
        let label = self.group_row_label(row);
        let path_label = path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        let id = self.peek_group_id(ix);
        let problems = id
            .as_ref()
            .map(|id| {
                self.group_diagnostics
                    .get(id, self.effective_fingerprint(ix))
            })
            .unwrap_or_default();
        let missing = derived
            .and_then(|d| group_rows::input_missing(d, |id| self.group_registry.is_excluded(id)));
        let changed = derived.and_then(|d| self.inputs_changed(d));
        let saved_warnings = if problems.is_empty() {
            self.saved_parser_record(ix)
                .map(|record| record.diagnostics.warnings())
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        let has_problems = !problems.is_empty()
            || !saved_warnings.is_empty()
            || missing.is_some()
            || changed.is_some();
        let error = problems
            .iter()
            .any(|p| p.severity == crate::app::ProblemSeverity::Error);
        let locked = self.frozen.contains(&ix);
        let mut detail = format!("{full_label}\n{path_label}\n{}", mode.label());
        for problem in problems {
            detail.push_str(&format!("\n{}", problem.message));
        }
        for warning in &saved_warnings {
            detail.push_str(&format!("\nPrevious parser check: {warning}"));
        }
        if let Some(missing) = &missing {
            detail.push_str(&format!("\n{missing}"));
        }
        if let Some(changed) = &changed {
            detail.push_str(&format!("\n{changed}"));
        }
        if locked {
            detail.push_str("\nProcessing locked");
        }
        if let Some(op) = derived.and_then(|d| d.operation.as_ref()) {
            detail.push_str(&format!("\n{} inputs", op.inputs.len()));
        }
        let tip = RowTooltip {
            text: detail,
            theme: t,
        };
        let more_tip = RowTooltip {
            text: "More actions".into(),
            theme: t,
        };
        // Source rows name their channel on the second line; only results keep
        // the compact operation tag ("Merge", "Diff", ...) beside the name.
        let tag = if matches!(row, Row::Result { .. }) {
            tag
        } else {
            ""
        };
        let source = match (&path, derived) {
            (None, Some(_)) => "derived".to_string(),
            (path, _) => path
                .as_deref()
                .and_then(|p| p.file_stem())
                .map(|stem| stem.to_string_lossy().into_owned())
                .unwrap_or_default(),
        };
        let quantity = match derived {
            Some(d) if d.source.is_none() || !d.quantity.is_absorption() => d.quantity.label(),
            _ => group_rows::measured_quantity(mode),
        };
        // E₀ is shown only when already known: a cached processed spectrum,
        // the inspector's current spectrum, or a fixed E₀ setting.
        let e0 = self
            .cache
            .peek(&(ix, self.effective_fingerprint(ix)))
            .and_then(|sp| sp.e0())
            .or_else(|| {
                active
                    .then(|| self.spectrum.as_deref().and_then(|sp| sp.e0()))
                    .flatten()
            })
            .or(self.effective_params(ix).e0);
        let subtitle = group_rows::row_detail(&source, quantity, e0);
        // Muted text lacks AA contrast on the current-row tint.
        let muted = if active { t.text } else { t.text_muted };
        let marked_children = if extra > 0 && !expanded {
            self.derived
                .iter()
                .enumerate()
                .filter(|(i, d)| {
                    DERIVED_BASE + i != ix
                        && d.source.is_some()
                        && d.source == path
                        && self.selection.contains(&(DERIVED_BASE + i))
                })
                .count()
        } else {
            0
        };
        let suffix = if marked_children > 0 {
            format!("+{extra} · {marked_children} marked")
        } else if extra > 0 && !expanded {
            format!("+{extra}")
        } else {
            String::new()
        };
        crate::accessibility::Control::new(
            div().id(("group", ix)),
            format!(
                "{}{}{}",
                full_label,
                if locked { " · locked" } else { "" },
                if has_problems {
                    " · source needs attention"
                } else {
                    ""
                }
            ),
            accesskit::Role::Button,
        )
        .selected(active)
        .when(active, |d| d.track_focus(&self.data_focus))
        .h(px(group_rows::ROW_HEIGHT))
        .w_full()
        .min_w_0()
        .px_1p5()
        .when(child, |d| d.pl(px(18.)))
        .flex()
        .items_center()
        .gap_1()
        .rounded_md()
        .border_1()
        .border_color(if self.focus_group == Some(ix) {
            t.accent
        } else {
            gpui::Rgba { a: 0., ..t.border }
        })
        .cursor_pointer()
        // Only the current group is highlighted; marks show as checkboxes.
        .when(active, |d| d.bg(t.current_row()))
        .when(!active, |d| d.hover(|d| d.bg(t.raised)))
        .tooltip(move |_, cx| cx.new(|_| tip.clone()).into())
        .on_mouse_down(
            gpui::MouseButton::Right,
            cx.listener(move |this, ev: &gpui::MouseDownEvent, window, cx| {
                cx.stop_propagation();
                this.open_group_menu(ix, ev.position, window, cx);
            }),
        )
        .on_click(cx.listener(move |this, ev: &ClickEvent, window, cx| {
            window.focus(&this.data_focus, cx);
            this.click_group(ix, ev.modifiers(), cx);
        }))
        .child(
            crate::accessibility::Control::new(
                div().id("disclosure"),
                "Additional channels",
                accesskit::Role::DisclosureTriangle,
            )
            .expanded(expanded)
            .disabled(extra == 0)
            .w(px(12.))
            .flex_none()
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                cx.stop_propagation();
                if extra > 0
                    && let Some(id) = this.stack_id(ix)
                {
                    this.expanded_sources.insert(id, !expanded);
                    cx.notify();
                }
            }))
            .children((extra > 0).then(|| {
                icon(
                    &t,
                    if expanded {
                        Icon::ChevronDown
                    } else {
                        Icon::ChevronRight
                    },
                )
                .size(px(12.))
            })),
        )
        .child(
            crate::accessibility::Control::new(
                div().id("mark"),
                format!("Mark {}", self.entry_label(ix)),
                accesskit::Role::CheckBox,
            )
            .selected(self.selection.contains(&ix))
            .flex_none()
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                cx.stop_propagation();
                window.focus(&this.data_focus, cx);
                this.toggle_mark(ix, cx);
            }))
            .child(checkbox(&t, self.selection.contains(&ix))),
        )
        .child(swatch(
            self.group_plot_color(ix)
                .map(crate::spectrum_colors::rgba)
                .unwrap_or_else(|| trace_rgba(&t, self.group_color_index(ix))),
        ))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(self.group_label_editor(ix, label, cx))
                .child(
                    div()
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .text_size(px(11.))
                        .text_color(muted)
                        .child(subtitle),
                ),
        )
        .child(
            div()
                .flex_none()
                .text_size(px(11.))
                .text_color(muted)
                .child(suffix),
        )
        .child(
            div()
                .flex_none()
                .text_size(px(11.))
                .text_color(muted)
                .child(tag),
        )
        .when(has_problems, |d| {
            d.child(icon(&t, Icon::Warning).size(px(13.)).text_color(if error {
                t.error
            } else {
                t.warn
            }))
        })
        .when(locked, |d| d.child(icon(&t, Icon::Lock).size(px(12.))))
        .child(
            crate::accessibility::Control::new(
                div().id("group-more"),
                "More actions",
                accesskit::Role::Button,
            )
            .size(px(16.))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded_sm()
            .cursor_pointer()
            .hover(|d| d.bg(t.raised))
            .tooltip(move |_, cx| cx.new(|_| more_tip.clone()).into())
            .child(icon(&t, Icon::More).size(px(14.)))
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(move |this, ev: &gpui::MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    this.open_group_menu(ix, ev.position, window, cx);
                }),
            )
            .on_click(cx.listener(
                move |this, event: &gpui::ClickEvent, window, cx| {
                    cx.stop_propagation();
                    // The pointer path opens on mouse down; keyboard and
                    // assistive activation arrive here.
                    if let gpui::ClickEvent::Keyboard(key) = event {
                        this.open_group_menu(ix, key.bounds.origin, window, cx);
                    }
                },
            )),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::group_identity::{GroupId, GroupRegistry};
    use crate::params::DetectionMode;
    use std::{collections::BTreeMap, path::PathBuf};

    #[test]
    fn merge_reason_shows_a_repeated_phrase_once() {
        assert_eq!(
            dedupe_reason(
                "Synthetic average · quantity unconfirmed: Quantity unconfirmed: confirm the quantity before normalization/AUTOBK; plotting/export available."
            ),
            "Synthetic average · quantity unconfirmed: confirm the quantity before normalization/AUTOBK; plotting/export available."
        );
        assert_eq!(
            dedupe_reason(
                "scan_1: χ(k): normalization/AUTOBK disabled; plotting/export available."
            ),
            "scan_1: χ(k): normalization/AUTOBK disabled; plotting/export available."
        );
        assert_eq!(
            dedupe_reason("A merge is already running."),
            "A merge is already running."
        );
    }

    #[test]
    fn standalone_lock_survives_capture_restore_migration_and_unlock() {
        use crate::app::NO_ENTRY;
        use crate::group_identity::GroupState;
        use std::collections::BTreeSet;
        let registry = GroupRegistry::default();
        let id = registry.register_source(
            None,
            "/standalone.dat".into(),
            DetectionMode::Auto,
            &BTreeMap::new(),
        );
        let mut state = GroupState::default();
        let mut frozen = BTreeSet::from([NO_ENTRY]);
        assert!(processing_locked(Some(NO_ENTRY), &frozen));
        assert!(!processing_locked(None, &frozen));
        state.capture(&registry, &BTreeSet::new(), &frozen, &BTreeMap::new(), None);
        state.capture_standalone_lock(&registry, &id, frozen.contains(&NO_ENTRY));
        let mut restored: GroupState =
            serde_json::from_str(&serde_json::to_string(&state).unwrap()).unwrap();
        assert!(restored.frozen.contains(&id));
        frozen.clear();
        restore_standalone_lock(&mut frozen, &restored, &id);
        assert!(processing_locked(Some(NO_ENTRY), &frozen));
        restore_standalone_lock(&mut frozen, &restored, &GroupId::new_result());
        assert!(!processing_locked(Some(NO_ENTRY), &frozen));
        restore_standalone_lock(&mut frozen, &restored, &id);
        // An unlock must remove the otherwise retained unresolved identity.
        restored.capture_standalone_lock(&registry, &id, false);
        assert!(restored.frozen.is_empty());
        registry.register_source(
            Some(3),
            "/standalone.dat".into(),
            DetectionMode::Auto,
            &BTreeMap::new(),
        );
        migrate_standalone_lock(&mut frozen, 3);
        assert_eq!(frozen, BTreeSet::from([3]));
        assert!(processing_locked(Some(3), &frozen));
        assert_eq!(registry.indices(&state.frozen), frozen);
        state.capture(&registry, &BTreeSet::new(), &frozen, &BTreeMap::new(), None);
        state.capture_standalone_lock(&registry, &id, false);
        assert!(
            state.frozen.contains(&id),
            "catalog lock survives retiring standalone adapter"
        );
    }

    #[test]
    fn standalone_and_child_labels_use_durable_display_metadata() {
        let id = GroupId::source(std::path::Path::new("/standalone.dat"), DetectionMode::Auto);
        let mut state = crate::group_identity::GroupState::default();
        for default in ["standalone.dat", "Fluorescence"] {
            assert_eq!(state.display_label(Some(&id), || default.into()), default);
        }
        state.labels.insert(id.clone(), "Cu foil".into());
        for default in ["standalone.dat", "Fluorescence", "Fluorescence · file.dat"] {
            assert_eq!(state.display_label(Some(&id), || default.into()), "Cu foil");
        }
        assert_eq!(
            state.display_label(Some(&GroupId::new_result()), || "other".into()),
            "other"
        );
    }

    #[test]
    fn standalone_swatch_matches_the_plotted_durable_identity() {
        let registry = GroupRegistry::default();
        let origins = BTreeMap::new();
        let path = (0..100)
            .map(|i| PathBuf::from(format!("/standalone/{i}.dat")))
            .find(|path| group_rows::color_index(&GroupId::source(path, DetectionMode::Auto)) != 0)
            .expect("fixture must exercise a nonzero swatch");
        let plot_id = registry.register_source(None, path.clone(), DetectionMode::Auto, &origins);
        assert!(
            registry.id(crate::app::NO_ENTRY).is_none(),
            "standalone has no catalog adapter index"
        );
        let row_id = registry.peek_source(&path, DetectionMode::Auto, &origins);
        assert_eq!(
            group_rows::color_index(&row_id),
            group_rows::color_index(&plot_id)
        );
        assert_ne!(group_rows::color_index(&row_id), 0);
        // Editing the mode must keep the original durable colour.
        assert_eq!(
            registry.peek_source(&path, DetectionMode::Reference, &origins),
            plot_id
        );
    }
}
