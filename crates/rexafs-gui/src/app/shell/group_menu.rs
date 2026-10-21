//! Row commands use a durable menu target, independently of current and marks.
use super::{MONO, journal::UndoOp, tools::Tool};
use crate::app::{DERIVED_BASE, StudioApp};
use crate::group_identity::GroupId;
use crate::params::{DerivedSpectrum, DetectionMode, PipelineParams};
use crate::widgets::text_input::TextInput;
use gpui::{
    ClickEvent, Context, Entity, FocusHandle, Focusable, IntoElement, MouseButton, MouseDownEvent,
    Pixels, Point, Window, div, prelude::*, px,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Item {
    Current,
    Mark,
    Rename,
    Colors,
    Color(u8),
    Duplicate,
    Lock,
    Remap,
    Channels,
    Channel(DetectionMode),
    Standard,
    Inputs,
    Reveal,
    Export,
    Remove,
    RemoveAll,
    ConfirmRemove,
    Cancel,
    Merge,
    Align,
    Compare,
}
#[derive(Clone, Default)]
pub(crate) struct MenuContext {
    spectrum: bool,
    file: bool,
    members: usize,
    operation: bool,
    locked: bool,
    marked: bool,
    marks: usize,
    absorption: bool,
    marked_absorption: usize,
    incompatible_marks: bool,
    merge_reason: Option<String>,
    source_exists: bool,
    compare: bool,
    channels: Vec<(DetectionMode, Option<&'static str>)>,
}
pub(crate) struct MenuItem {
    item: Item,
    enabled: bool,
    reason: Option<String>,
    label: String,
}
pub(crate) fn menu_items(c: &MenuContext) -> Vec<MenuItem> {
    use Item::*;
    let merge = format!("Merge {} marked…", c.marks);
    let remove_all = format!("Remove all {} groups from this file…", c.members);
    [
        (Current, "Make current"),
        (Mark, if c.marked { "Unmark" } else { "Mark" }),
        (Rename, "Rename"),
        (Colors, "Color ▸"),
        (Duplicate, "Duplicate group"),
        (
            Lock,
            if c.locked {
                "Unlock processing"
            } else {
                "Lock processing"
            },
        ),
        (Remap, "Re-map columns…"),
        (Channels, "Add channel ▸"),
        (Standard, "Set as alignment standard"),
        (Inputs, "Show inputs"),
        (Reveal, "Reveal source in Finder"),
        (Export, "Export…"),
        (Remove, "Remove group"),
        (RemoveAll, remove_all.as_str()),
        (Merge, merge.as_str()),
        (Align, "Align marked…"),
        (Compare, "Compare current and marked"),
    ]
    .into_iter()
    .map(|(item, label)| {
        let reason = match item {
            Mark | Colors | Duplicate | Lock | Export if !c.spectrum => Some("Requires a spectrum"),
            Remap | Channels if !c.file => Some("No source columns"),
            Standard if !c.absorption => Some("Requires an absorption spectrum"),
            Inputs if !c.operation => Some("No recorded inputs"),
            Reveal if !c.source_exists => Some("Source is not a local file"),
            Remove if !c.spectrum => Some("Requires a group"),
            RemoveAll if !c.file || c.members < 2 => Some("Requires a file with multiple groups"),
            Merge if c.incompatible_marks => Some("Marked quantities are incompatible"),
            Merge if c.marks < 2 => Some("Mark at least two spectra"),
            Align if c.marked_absorption == 0 => Some("Mark an absorption spectrum"),
            Compare if !c.compare => Some("Requires two compatible spectra"),
            _ => None,
        };
        let reason = if item == Merge {
            c.merge_reason
                .clone()
                .or_else(|| reason.map(str::to_string))
        } else {
            reason.map(str::to_string)
        };
        MenuItem {
            item,
            enabled: reason.is_none(),
            reason,
            label: label.into(),
        }
    })
    .collect()
}

fn missing_channels(
    available: &[DetectionMode],
    present: &[DetectionMode],
) -> Vec<(DetectionMode, Option<&'static str>)> {
    use DetectionMode::*;
    [Transmission, Fluorescence, Reference, MuColumn]
        .into_iter()
        .filter(|mode| !present.contains(mode))
        .map(|mode| {
            (
                mode,
                (!available.contains(&mode)).then_some("Required columns were not detected"),
            )
        })
        .collect()
}

impl MenuContext {
    fn detect_channels(
        &mut self,
        path: &std::path::Path,
        target: &crate::params::ImportConfig,
        siblings: &[crate::params::ImportConfig],
    ) {
        let detection = crate::params::detect_import(path, target).ok().flatten();
        let present: Vec<_> = siblings
            .iter()
            .filter_map(|import| {
                if import.mode == DetectionMode::Auto {
                    crate::params::detect_import(path, import)
                        .ok()
                        .flatten()
                        .map(|d| d.auto_mode)
                } else {
                    Some(import.mode)
                }
            })
            .collect();
        self.channels = missing_channels(
            &detection
                .map(|d| d.available_channels())
                .unwrap_or_default(),
            &present,
        );
        // An existing channel can still be missing from other files in this
        // exact batch. Its editor reports the existing count and never duplicates it.
        self.channels.extend(
            [
                DetectionMode::Transmission,
                DetectionMode::Fluorescence,
                DetectionMode::Reference,
                DetectionMode::MuColumn,
            ]
            .into_iter()
            .filter(|mode| present.contains(mode))
            .map(|mode| (mode, None)),
        );
    }
}

/// Default labels are presentation, so accepting one clears custom metadata.
fn apply_rename(
    labels: &mut std::collections::BTreeMap<GroupId, String>,
    id: GroupId,
    label: String,
    default_entry: &str,
    default_row: &str,
) -> Option<UndoOp> {
    let before = labels.get(&id).cloned();
    let after = (label != default_entry && label != default_row).then_some(label);
    if before == after {
        return None;
    }
    if let Some(label) = &after {
        labels.insert(id.clone(), label.clone());
    } else {
        labels.remove(&id);
    }
    Some(UndoOp::Label { id, before, after })
}

pub(crate) fn validate_label(label: &str) -> Result<String, &'static str> {
    let label = label.trim();
    if label.is_empty() {
        Err("A group label cannot be empty")
    } else {
        Ok(label.into())
    }
}

pub(crate) struct GroupMenu {
    target: GroupId,
    position: Point<Pixels>,
    focus: FocusHandle,
    context: MenuContext,
    submenu: Option<Item>,
    removal: Vec<(GroupId, String)>,
}
pub(crate) struct RenameState {
    target: GroupId,
    input: Entity<TextInput>,
    default_entry: String,
    default_row: String,
}

/// A lazy source copy is independent without reading or retaining raw arrays.
fn duplicate(
    group: Option<&DerivedSpectrum>,
    path: std::path::PathBuf,
    params: PipelineParams,
    label: String,
    id: u64,
) -> DerivedSpectrum {
    let mut copy = group.cloned().unwrap_or_else(|| DerivedSpectrum {
        source: Some(path),
        ..Default::default()
    });
    copy.id = id;
    copy.group_id = Some(GroupId::new_result());
    copy.params = Some(params);
    copy.label = format!("{label} copy");
    copy
}

impl StudioApp {
    pub(crate) fn menu_index(&self, id: &GroupId) -> Option<usize> {
        if self.group_registry.is_excluded(id) {
            return None;
        }
        self.group_registry.index(id).or_else(|| {
            self.standalone_source
                .as_ref()
                .filter(|(_, _, sid)| sid == id)
                .map(|_| crate::app::NO_ENTRY)
        })
    }
    fn file_members(&self, ix: usize) -> Vec<(GroupId, String)> {
        let Some(target) = self
            .tool_target(ix)
            .filter(|t| !t.path.as_os_str().is_empty())
        else {
            return vec![];
        };
        let path = target.path;
        self.catalog
            .find_by_canonical_path(&path)
            .into_iter()
            .chain(
                self.standalone_path()
                    .filter(|p| *p == path)
                    .map(|_| crate::app::NO_ENTRY),
            )
            .chain(
                self.derived
                    .iter()
                    .enumerate()
                    .filter(|(_, d)| d.source.as_ref() == Some(&path))
                    .map(|(i, _)| DERIVED_BASE + i),
            )
            .filter_map(|g| self.group_id(g).map(|id| (id, self.entry_label(g))))
            .collect()
    }

    fn group_absorption(&self, ix: usize) -> bool {
        ix.checked_sub(DERIVED_BASE)
            .and_then(|i| self.derived.get(i))
            .is_none_or(|d| d.quantity.is_absorption() && !d.quantity_unconfirmed)
    }
    pub(crate) fn open_group_menu(
        &mut self,
        ix: usize,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(target) = self.tool_target(ix) else {
            return;
        };
        let Some(id) = target.group_id else { return };
        let derived = ix
            .checked_sub(DERIVED_BASE)
            .and_then(|i| self.derived.get(i));
        let file = !target.path.as_os_str().is_empty();
        let marked_absorption = self
            .selection
            .iter()
            .filter(|&&i| self.group_absorption(i))
            .count();
        let mut union = self.selection.clone();
        union.extend(self.current_group_index());
        let mut context = MenuContext {
            spectrum: true,
            file,
            members: self.file_members(ix).len(),
            operation: derived.is_some_and(|d| d.operation.is_some()),
            locked: self.frozen.contains(&ix),
            marked: self.selection.contains(&ix),
            marks: self.selection.len(),
            absorption: self.group_absorption(ix),
            marked_absorption,
            incompatible_marks: marked_absorption != self.selection.len(),
            merge_reason: self.merge_disabled_reason(),
            source_exists: target.path.is_file(),
            compare: union.len() >= 2 && union.iter().all(|&i| self.group_absorption(i)),
            channels: [
                DetectionMode::Transmission,
                DetectionMode::Fluorescence,
                DetectionMode::Reference,
                DetectionMode::MuColumn,
            ]
            .into_iter()
            .map(|m| (m, Some("Checking source columns…")))
            .collect(),
        };
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        self.group_rename = None;
        self.group_menu = Some(GroupMenu {
            target: id.clone(),
            position,
            focus,
            context: context.clone(),
            submenu: None,
            removal: vec![],
        });
        if file {
            let params = self.effective_params(ix).clone();
            let path = target.path;
            let mut present: Vec<_> = self
                .derived
                .iter()
                .filter(|d| d.source.as_ref() == Some(&path))
                .map(|d| d.params.as_ref().unwrap_or(&self.params).import.clone())
                .collect();
            let primary = self
                .catalog
                .find_by_canonical_path(&path)
                .filter(|&i| self.valid_group_index(i))
                .map(|i| self.effective_params(i).import.clone())
                .or_else(|| {
                    self.standalone_path()
                        .filter(|p| *p == path)
                        .map(|_| self.params.import.clone())
                });
            present.extend(primary);
            cx.spawn(async move |this, cx| {
                let channels = cx
                    .background_executor()
                    .spawn(async move {
                        context.detect_channels(&path, &params.import, &present);
                        context.channels
                    })
                    .await;
                this.update(cx, |app, cx| {
                    if let Some(menu) = &mut app.group_menu
                        && menu.target == id
                    {
                        menu.context.channels = channels;
                        cx.notify();
                    }
                })
                .ok();
            })
            .detach();
        }
        cx.notify();
    }
    pub(crate) fn dismiss_group_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let target = self
            .group_menu
            .take()
            .map(|m| m.target)
            .or_else(|| self.group_rename.take().map(|r| r.target));
        if let Some(id) = target {
            self.focus_group = self.menu_index(&id);
        }
        window.focus(&self.data_focus, cx);
        cx.notify();
    }
    pub(crate) fn start_group_rename(
        &mut self,
        ix: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(target) = self.group_id(ix) else {
            return;
        };
        let rows = self.interaction_rows();
        let row = rows
            .row_index(ix)
            .and_then(|i| rows.row_at(i))
            .unwrap_or(crate::app::group_rows::Row::Result { group: ix });
        let default_entry = self.default_entry_label(ix);
        let default_row = self.default_group_row_label(row);
        let input =
            cx.new(|cx| TextInput::new("Group label", self.group_row_label(row), self.theme, cx));
        window.focus(&input.read(cx).focus_handle(cx), cx);
        self.group_menu = None;
        self.focus_group = Some(ix);
        self.group_rename = Some(RenameState {
            target,
            input,
            default_entry,
            default_row,
        });
        cx.notify();
    }
    fn commit_group_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(rename) = &self.group_rename else {
            return;
        };
        let label = match validate_label(rename.input.read(cx).text()) {
            Ok(label) => label,
            Err(reason) => {
                rename
                    .input
                    .update(cx, |input, cx| input.set_error(true, cx));
                self.status = reason.into();
                cx.notify();
                return;
            }
        };
        let id = rename.target.clone();
        if let Some(undo) = apply_rename(
            &mut self.group_state.labels,
            id,
            label.clone(),
            &rename.default_entry,
            &rename.default_row,
        ) {
            self.record(format!("rename {label}"), Some(undo));
            self.invalidate_explore_plots(cx);
        }
        self.dismiss_group_editor(window, cx);
    }
    pub(crate) fn group_label_editor(
        &self,
        ix: usize,
        label: String,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let input = self
            .group_rename
            .as_ref()
            .filter(|r| self.peek_group_id(ix).as_ref() == Some(&r.target))
            .map(|r| r.input.clone());
        div()
            .id("group-label")
            .flex_1()
            .min_w_0()
            .overflow_hidden()
            .whitespace_nowrap()
            // Keep the start of the name; long names end in an ellipsis.
            .text_ellipsis()
            .font_family(MONO)
            .text_size(px(12.))
            .key_context("GroupRename")
            .on_action(
                cx.listener(|this, _: &crate::app::CommitGroupRename, window, cx| {
                    this.commit_group_rename(window, cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &crate::app::DismissGroupEditor, window, cx| {
                    this.dismiss_group_editor(window, cx)
                }),
            )
            .on_click(cx.listener(move |this, ev: &ClickEvent, window, cx| {
                if this.group_rename.is_some() {
                    cx.stop_propagation();
                } else if ev.click_count() == 2 {
                    cx.stop_propagation();
                    this.start_group_rename(ix, window, cx);
                }
            }))
            .when(input.is_none(), |d| d.child(label))
            .children(input)
            .into_any_element()
    }
    fn dispatch_group_item(&mut self, item: Item, window: &mut Window, cx: &mut Context<Self>) {
        let Some(menu) = &self.group_menu else { return };
        let Some(ix) = self.menu_index(&menu.target) else {
            self.dismiss_group_editor(window, cx);
            return;
        };
        let id = menu.target.clone();
        if item == Item::RemoveAll {
            let removal = self.file_members(ix);
            if removal.len() > 1
                && let Some(menu) = &mut self.group_menu
            {
                menu.removal = removal;
                menu.submenu = Some(Item::RemoveAll);
            }
            cx.notify();
            return;
        }
        let removal = menu.removal.clone();
        if matches!(item, Item::Colors | Item::Channels) {
            if let Some(menu) = &mut self.group_menu {
                menu.submenu = if menu.submenu == Some(item) {
                    None
                } else {
                    Some(item)
                };
            }
            cx.notify();
            return;
        }
        self.dismiss_group_editor(window, cx);
        match item {
            Item::Current => self.select_entry(ix, cx),
            Item::Mark => self.toggle_mark(ix, cx),
            Item::Rename => self.start_group_rename(ix, window, cx),
            Item::Color(color) => {
                let after = crate::spectrum_colors::Assignment {
                    palette: crate::spectrum_colors::Palette::Theme,
                    index: usize::from(color),
                    count: 8,
                    reversed: false,
                };
                let before = self.group_state.plot_colors.insert(id.clone(), after);
                if before != Some(after) {
                    self.record(
                        "change group color",
                        Some(UndoOp::SpectrumColors {
                            changes: vec![(id, before, Some(after))],
                        }),
                    );
                }
                self.invalidate_explore_plots(cx);
            }
            Item::Duplicate => {
                let Some(target) = self.tool_target(ix) else {
                    return;
                };
                let next = self.next_group_id();
                let copy = duplicate(
                    ix.checked_sub(DERIVED_BASE)
                        .and_then(|i| self.derived.get(i)),
                    target.path,
                    self.effective_params(ix).clone(),
                    self.entry_label(ix),
                    next,
                );
                if let Some(id) = &copy.group_id {
                    self.group_state
                        .labels
                        .insert(id.clone(), copy.label.clone());
                    self.group_state
                        .colors
                        .insert(id.clone(), crate::app::group_rows::color_index(id) as u8);
                }
                let index = self.derived.len();
                self.derived.push(copy.clone());
                self.group_registry.append_derived(&self.derived, index);
                self.record(
                    "duplicate group",
                    Some(UndoOp::DerivedAdd {
                        index,
                        spectrum: copy,
                    }),
                );
            }
            Item::Lock => self.toggle_group_lock(ix, cx),
            Item::Remap => {
                self.open_import_editor(ix, window, cx);
            }
            Item::Channel(mode) => self.open_channel_editor(ix, mode, window, cx),
            Item::Standard => {
                if self.tools.pin_alignment_standard(id) {
                    self.choose_tool_standard(self.tool_target(ix), cx);
                }
            }
            Item::Inputs => {
                if let Some(op) = ix
                    .checked_sub(DERIVED_BASE)
                    .and_then(|i| self.derived.get(i))
                    .and_then(|d| d.operation.as_ref())
                {
                    self.status = format!(
                        "Inputs: {}",
                        op.inputs
                            .iter()
                            .map(|i| format!("{} ({})", i.label, i.path.display()))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                    .into();
                }
            }
            Item::Reveal => {
                if let Some(target) = self.tool_target(ix) {
                    #[cfg(target_os = "macos")]
                    if let Err(error) = std::process::Command::new("open")
                        .arg("-R")
                        .arg(target.path)
                        .spawn()
                    {
                        self.status = format!("Reveal failed: {error}").into();
                    }
                    #[cfg(not(target_os = "macos"))]
                    {
                        self.status = format!("Source: {}", target.path.display()).into();
                    }
                }
            }
            Item::Export => self.export_group_csv(ix, cx),
            Item::Remove => {
                if ix != crate::app::NO_ENTRY
                    && let Some(index) = ix.checked_sub(DERIVED_BASE)
                {
                    self.remove_derived(index, cx);
                } else {
                    self.remove_groups(
                        std::collections::BTreeSet::from([id]),
                        format!("remove {}", self.entry_label(ix)),
                        cx,
                    );
                }
            }
            Item::ConfirmRemove => {
                let file = self
                    .tool_target(ix)
                    .map(|t| t.path.display().to_string())
                    .unwrap_or_default();
                self.remove_groups(
                    removal.iter().map(|(id, _)| id.clone()).collect(),
                    format!("remove {} groups from {file}", removal.len()),
                    cx,
                );
            }
            Item::RemoveAll | Item::Cancel => {}
            Item::Merge => self.merge_selection(cx),
            Item::Align => self.open_tool(Tool::Align, cx),
            Item::Compare => {
                self.stage_view.scope = super::PlotScope::Marked;
                self.ensure_compare_loaded(cx);
                self.invalidate_explore_plots(cx);
            }
            Item::Colors | Item::Channels => {}
        }
        cx.notify();
    }
    fn export_group_csv(&mut self, ix: usize, cx: &mut Context<Self>) {
        let Some(target) = self.tool_target(ix) else {
            return;
        };
        let params = self.effective_params(ix).clone();
        let derived = ix
            .checked_sub(DERIVED_BASE)
            .and_then(|i| self.derived.get(i))
            .cloned();
        cx.spawn(async move |this, cx| {
            let data = cx
                .background_executor()
                .spawn(async move {
                    crate::params::load_group_raw_with_diagnostics(
                        &target.path,
                        &params,
                        derived.as_ref(),
                    )
                })
                .await;
            let data = match data {
                Ok(data) => data,
                Err(error) => {
                    this.update(cx, |app, cx| {
                        app.status = format!("Export failed: {error}").into();
                        cx.notify();
                    })
                    .ok();
                    return;
                }
            };
            let picker = this.update(cx, |_, cx| {
                cx.prompt_for_new_path(
                    &crate::settings::home_dir().unwrap_or_else(std::env::temp_dir),
                    Some("spectrum.csv"),
                )
            });
            if let Ok(picker) = picker
                && let Ok(Ok(Some(path))) = picker.await
            {
                let result = cx
                    .background_executor()
                    .spawn(async move {
                        use std::io::Write;
                        let mut file = std::io::BufWriter::new(std::fs::File::create(path)?);
                        writeln!(file, "energy,value")?;
                        for (energy, value) in data.energy.iter().zip(&data.mu) {
                            writeln!(file, "{energy},{value}")?;
                        }
                        file.flush()
                    })
                    .await;
                this.update(cx, |app, cx| {
                    app.status = match result {
                        Ok(()) => "Exported spectrum CSV".into(),
                        Err(e) => format!("Export failed: {e}").into(),
                    };
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }
    pub(crate) fn group_menu_overlay(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let menu = self.group_menu.as_ref()?;
        let t = self.theme;
        let mut items = menu_items(&menu.context);
        if menu.submenu == Some(Item::Channels) {
            items = menu
                .context
                .channels
                .iter()
                .map(|&(mode, reason)| MenuItem {
                    item: Item::Channel(mode),
                    enabled: reason.is_none(),
                    reason: reason.map(str::to_string),
                    label: mode.label().into(),
                })
                .collect();
        }
        if menu.submenu == Some(Item::Colors) {
            items = (0..8)
                .map(|c| MenuItem {
                    item: Item::Color(c),
                    enabled: true,
                    reason: None,
                    label: format!("Color {}", c + 1),
                })
                .collect();
        }
        if menu.submenu == Some(Item::RemoveAll) {
            items = vec![
                MenuItem {
                    item: Item::ConfirmRemove,
                    enabled: true,
                    reason: None,
                    label: format!("Remove {} groups", menu.removal.len()),
                },
                MenuItem {
                    item: Item::Cancel,
                    enabled: true,
                    reason: None,
                    label: "Cancel".into(),
                },
            ];
        }
        let mut list = div()
            .id("group-menu-list")
            .w(px(310.))
            .max_h(px(540.))
            .overflow_y_scroll()
            .p_1()
            .bg(t.surface)
            .border_1()
            .border_color(t.border)
            .rounded_md()
            .shadow_lg()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation());
        if menu.submenu == Some(Item::RemoveAll) {
            list = list.child(
                div()
                    .p_2()
                    .child("Remove these groups? Source file is kept."),
            );
            for (_, label) in &menu.removal {
                list = list.child(div().px_2().py_1().text_size(px(12.)).child(label.clone()));
            }
        }
        if items.is_empty() {
            list = list.child("All detected channels already exist");
        }
        for (i, entry) in items.into_iter().enumerate() {
            let item = entry.item;
            list = list.child(
                div()
                    .id(("group-menu-item", i))
                    .px_2()
                    .py_1()
                    .text_size(px(12.))
                    .text_color(if entry.enabled { t.text } else { t.text_muted })
                    .when(entry.enabled, |d| {
                        d.cursor_pointer()
                            .hover(|d| d.bg(t.raised))
                            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                this.dispatch_group_item(item, window, cx)
                            }))
                    })
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .when_some(
                                if let Item::Color(c) = item {
                                    Some(c)
                                } else {
                                    None
                                },
                                |d, c| {
                                    d.child(
                                        div()
                                            .size(px(12.))
                                            .bg(crate::plotting::trace_rgba(&t, c as usize)),
                                    )
                                },
                            )
                            .child(entry.label),
                    )
                    .children(
                        entry
                            .reason
                            .map(|reason| div().text_size(px(11.)).child(reason)),
                    ),
            );
        }
        Some(
            div()
                .id("group-menu-overlay")
                .absolute()
                .inset_0()
                .occlude()
                .key_context("GroupMenu")
                .track_focus(&menu.focus)
                .on_action(
                    cx.listener(|this, _: &crate::app::DismissGroupEditor, window, cx| {
                        this.dismiss_group_editor(window, cx)
                    }),
                )
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _: &MouseDownEvent, window, cx| {
                        this.dismiss_group_editor(window, cx)
                    }),
                )
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(|this, _: &MouseDownEvent, window, cx| {
                        this.dismiss_group_editor(window, cx)
                    }),
                )
                .child(
                    gpui::anchored()
                        .position(menu.position)
                        .snap_to_window()
                        .child(list),
                )
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn removal_menu_requires_multiple_groups_of_the_target_file() {
        for (file, members, enabled) in [(true, 1, false), (true, 3, true), (false, 3, false)] {
            let items = menu_items(&MenuContext {
                spectrum: true,
                file,
                members,
                ..Default::default()
            });
            let remove = items.iter().find(|i| i.item == Item::RemoveAll).unwrap();
            assert_eq!(remove.enabled, enabled);
            assert_eq!(
                remove.label,
                format!("Remove all {members} groups from this file…")
            );
            assert!(
                items
                    .iter()
                    .find(|i| i.item == Item::Remove)
                    .unwrap()
                    .enabled
            );
        }
    }

    #[test]
    fn missing_channel_rules_resolve_auto_independently_of_the_menu_target() {
        use DetectionMode::*;
        // Right-click Reference: the Auto primary is still Transmission.
        assert_eq!(
            missing_channels(
                &[Transmission, Fluorescence, Reference],
                &[Transmission, Reference]
            ),
            vec![
                (Fluorescence, None),
                (MuColumn, Some("Required columns were not detected"))
            ]
        );
        assert!(
            missing_channels(&[], &[Transmission, Fluorescence, Reference, MuColumn]).is_empty()
        );
        assert!(
            missing_channels(&[], &[])
                .iter()
                .all(|(_, reason)| reason.is_some())
        );
    }

    #[test]
    fn context_resolves_each_auto_siblings_own_mapping() {
        use crate::params::ImportConfig;
        use DetectionMode::*;
        let path =
            std::env::temp_dir().join(format!("rexafs-menu-channels-{}.dat", std::process::id()));
        std::fs::write(
            &path,
            "# energy i0 it ir roi1\n1000 100 50 25 10\n1001 100 50 25 10\n",
        )
        .unwrap();
        let primary = ImportConfig {
            mu_col: Some(4),
            ..Default::default()
        };
        let reference = ImportConfig {
            mode: Reference,
            ..Default::default()
        };
        let mut context = MenuContext {
            file: true,
            ..Default::default()
        };
        context.detect_channels(&path, &reference, &[primary.clone(), reference.clone()]);
        assert!(context.channels.contains(&(Transmission, None)));
        // Existing interpretations remain reviewable for missing channels in
        // other batch members, including Auto's manually chosen μ column.
        assert!(context.channels.contains(&(MuColumn, None)));
        assert!(context.channels.contains(&(Reference, None)));
        assert_eq!(context.channels.len(), 4);
        // A second Auto sibling uses the source's transmission mapping.
        context.detect_channels(
            &path,
            &reference,
            &[primary, reference.clone(), ImportConfig::default()],
        );
        assert!(context.channels.contains(&(Transmission, None)));
        assert!(context.channels.contains(&(MuColumn, None)));
        assert_eq!(context.channels.len(), 4);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn committing_default_labels_is_noop_or_removes_custom_metadata() {
        let id = GroupId::new_result();
        let mut labels = std::collections::BTreeMap::new();
        let full = "Fluorescence · file.dat";
        let row = "Fluorescence";
        for default in [full, row] {
            assert!(apply_rename(&mut labels, id.clone(), default.into(), full, row).is_none());
            assert!(labels.is_empty());
            labels.insert(id.clone(), "custom".into());
            assert!(matches!(
                apply_rename(&mut labels, id.clone(), default.into(), full, row),
                Some(UndoOp::Label { before: Some(before), after: None, .. }) if before == "custom"
            ));
            assert!(labels.is_empty());
        }
        assert!(apply_rename(&mut labels, id.clone(), "custom".into(), full, row).is_some());
        assert!(apply_rename(&mut labels, id.clone(), "custom".into(), full, row).is_none());
        assert_eq!(labels.get(&id).map(String::as_str), Some("custom"));
    }

    #[test]
    fn rename_validation_trims_rejects_empty_and_allows_duplicates() {
        for label in ["", " \t\n", "\u{3000}"] {
            assert!(validate_label(label).is_err());
        }
        assert_eq!(
            validate_label("  Cu foil 日本語  "),
            Ok("Cu foil 日本語".into())
        );
        assert_eq!(validate_label("Cu foil"), validate_label("Cu foil"));
    }
    #[test]
    fn merge_menu_explains_the_known_edge_conflict() {
        let reason = "Different declared edges: Cu K and Ni K.";
        let context = MenuContext {
            marks: 2,
            merge_reason: Some(reason.into()),
            ..Default::default()
        };
        let item = menu_items(&context)
            .into_iter()
            .find(|entry| entry.item == Item::Merge)
            .unwrap();
        assert!(!item.enabled);
        assert_eq!(item.reason.as_deref(), Some(reason));
    }
    #[test]
    fn menu_rules_distinguish_target_capabilities_from_project_marks() {
        let mut context = MenuContext {
            spectrum: true,
            file: true,
            locked: true,
            marks: 2,
            marked_absorption: 2,
            absorption: true,
            ..Default::default()
        };
        let items = menu_items(&context);
        for item in [
            Item::Current,
            Item::Mark,
            Item::Rename,
            Item::Colors,
            Item::Duplicate,
            Item::Lock,
            Item::Remap,
            Item::Channels,
            Item::Standard,
            Item::Export,
            Item::Remove,
            Item::Merge,
            Item::Align,
        ] {
            assert!(
                items.iter().find(|i| i.item == item).unwrap().enabled,
                "{item:?}"
            );
        }
        for item in [Item::Inputs, Item::Reveal, Item::RemoveAll, Item::Compare] {
            let entry = items.iter().find(|i| i.item == item).unwrap();
            assert!(!entry.enabled);
            assert!(entry.reason.is_some());
        }
        assert_eq!(
            items.iter().find(|i| i.item == Item::Lock).unwrap().label,
            "Unlock processing"
        );
        context.incompatible_marks = true;
        context.marked_absorption = 0;
        context.absorption = false;
        context.file = false;
        context.members = 2;
        context.operation = true;
        context.marked = true;
        context.source_exists = true;
        let items = menu_items(&context);
        for item in [
            Item::Merge,
            Item::Align,
            Item::Standard,
            Item::Remap,
            Item::Channels,
        ] {
            assert!(!items.iter().find(|i| i.item == item).unwrap().enabled);
        }
        for item in [Item::Inputs, Item::Remove, Item::Reveal] {
            assert!(items.iter().find(|i| i.item == item).unwrap().enabled);
        }
        assert_eq!(
            items.iter().find(|i| i.item == Item::Mark).unwrap().label,
            "Unmark"
        );
        let empty = menu_items(&MenuContext::default());
        assert!(
            empty
                .iter()
                .filter(|i| i.enabled)
                .all(|i| matches!(i.item, Item::Current | Item::Rename))
        );
    }
    #[test]
    fn duplicate_has_independent_identity_settings_and_preserves_interaction() {
        use crate::group_identity::{GroupRegistry, GroupState};
        use std::collections::{BTreeMap, BTreeSet};
        let mut sources = vec![];
        let registry = GroupRegistry::rebuild(
            [(0, "/cu.dat".into(), DetectionMode::Auto)],
            &mut [],
            &mut sources,
            &BTreeMap::new(),
        );
        let mut state = GroupState::default();
        state.capture(
            &registry,
            &BTreeSet::from([0]),
            &BTreeSet::from([0]),
            &BTreeMap::new(),
            Some(0),
        );
        let original = registry.id(0).unwrap();
        let copy = duplicate(
            None,
            "/cu.dat".into(),
            PipelineParams::default(),
            "Cu foil".into(),
            1,
        );
        assert_ne!(copy.group_id.as_ref(), Some(&original));
        assert_eq!(copy.label, "Cu foil copy");
        assert_eq!(
            copy.source.as_deref(),
            Some(std::path::Path::new("/cu.dat"))
        );
        registry.append_derived(std::slice::from_ref(&copy), 0);
        assert_eq!(registry.indices(&state.marked), BTreeSet::from([0]));
        assert_eq!(registry.indices(&state.frozen), BTreeSet::from([0]));
        assert_eq!(state.current, Some(original));
        let result = DerivedSpectrum {
            energy: vec![1., 2.],
            mu: vec![3., 4.],
            ..Default::default()
        };
        let mut copy = duplicate(
            Some(&result),
            Default::default(),
            PipelineParams::default(),
            "Result".into(),
            2,
        );
        copy.mu[0] = 99.;
        copy.params.as_mut().unwrap().e0 = Some(20.);
        assert_eq!(result.mu[0], 3.);
        assert!(result.params.is_none());
    }
}
