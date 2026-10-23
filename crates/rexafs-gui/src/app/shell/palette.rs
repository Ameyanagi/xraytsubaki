//! Command palette (⌘K): actions, tools and group names, filtered as you
//! type. ↑/↓ move, Enter runs, Esc closes.

use gpui::{
    ClickEvent, Context, Entity, Focusable, IntoElement, ParentElement, SharedString, Styled,
    Window, div, prelude::*, px,
};

use super::{MONO, ProcessSection, Stage};
use super::{parameter_actions::ParamScope, tools::Tool};
use crate::app::{DERIVED_BASE, NO_ENTRY, PaletteClose, StudioApp};
use crate::widgets::text_input::{InputEvent, TextInput};

fn catalog_items(
    len: usize,
    registry: &crate::group_identity::GroupRegistry,
    cap: usize,
) -> impl Iterator<Item = usize> + '_ {
    (0..len)
        .filter(|&ix| !registry.index_excluded(ix))
        .take(cap)
}

#[derive(Clone, Debug, PartialEq)]
pub enum PaletteCmd {
    Stage(Stage),
    /// Process stage with one section expanded and in view (the former
    /// Normalize / Background / Transform stages).
    ProcessSection(ProcessSection),
    MarkAll,
    MarkNone,
    ApplySectionToMarked(ProcessSection),
    ApplyAllProcessingToMarked,
    CopyMappingToMarked,
    ResetParams,
    Tool(Tool),
    Fit,
    ExportBatchCsv,
    SaveProject,
    OpenProject,
    OpenFolder,
    RecentProject(std::path::PathBuf),
    RecentFolder(std::path::PathBuf),
    Theme,
    Updates,
    Help,
    About,
    Preferences,
    Licenses,
    Storage,
    Example,
    CopperSeries,
    Undo,
    Redo,
    Journal,
    Problems,
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

/// Match quality, lower is better: the whole label, a label prefix, a word
/// prefix, a substring, then the fuzzy match.
fn match_rank(query: &str, label: &str) -> Option<u8> {
    let query = query.to_lowercase();
    let label = label.to_lowercase();
    if label == query {
        Some(0)
    } else if label.starts_with(&query) {
        Some(1)
    } else if label
        .split(|c: char| !c.is_alphanumeric())
        .any(|word| !word.is_empty() && word.starts_with(&query))
    {
        Some(2)
    } else if label.contains(&query) {
        Some(3)
    } else if fuzzy(&query, &label) {
        Some(4)
    } else {
        None
    }
}

/// State-changing commands sort after navigation, analysis and tool items of
/// the same match rank.
fn category_rank(category: &str) -> u8 {
    u8::from(matches!(category, "params" | "export"))
}

/// Keep the items matching `query`, best match first; ties keep catalog order.
fn rank_items(query: &str, items: Vec<PaletteItem>) -> Vec<PaletteItem> {
    if query.is_empty() {
        return items;
    }
    let mut ranked: Vec<_> = items
        .into_iter()
        .enumerate()
        .filter_map(|(order, item)| {
            match_rank(query, &item.label)
                .map(|rank| (rank, category_rank(item.category), order, item))
        })
        .collect();
    ranked.sort_by_key(|(rank, category, order, _)| (*rank, *category, *order));
    ranked.into_iter().map(|(_, _, _, item)| item).collect()
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

/// "Go to" entries: the five stages with their ⌘-shortcuts, then the three
/// Process sections that used to be stages of their own.
fn stage_items() -> Vec<PaletteItem> {
    Stage::ALL
        .into_iter()
        .map(|stage| PaletteItem {
            label: format!("Go to {}", stage.name()),
            category: "stage",
            keys: stage.shortcut(),
            cmd: PaletteCmd::Stage(stage),
        })
        .chain(ProcessSection::ALL.into_iter().map(|section| PaletteItem {
            label: format!("Go to Process ▸ {}", section.name()),
            category: "stage",
            keys: "",
            cmd: PaletteCmd::ProcessSection(section),
        }))
        .collect()
}

fn reset_params_item(stage: Stage) -> Option<PaletteItem> {
    stage.is_processing().then(|| PaletteItem {
        label: "Reset parameters of the current group".into(),
        category: "params",
        keys: "",
        cmd: PaletteCmd::ResetParams,
    })
}

fn recent_items(settings: &crate::settings::UserSettings) -> Vec<PaletteItem> {
    settings
        .recent_projects
        .iter()
        .map(|path| PaletteItem {
            label: format!("Open recent project ▸ {}", path.display()),
            category: "file",
            keys: "",
            cmd: PaletteCmd::RecentProject(path.clone()),
        })
        .chain(
            settings
                .recent_import_folders
                .iter()
                .map(|path| PaletteItem {
                    label: format!("Import from recent folder ▸ {}", path.display()),
                    category: "file",
                    keys: "",
                    cmd: PaletteCmd::RecentFolder(path.clone()),
                }),
        )
        .collect()
}

fn validate_recent_project(
    projects: &mut Vec<std::path::PathBuf>,
    path: &std::path::Path,
    is_file: impl FnOnce(&std::path::Path) -> bool,
) -> Result<(), String> {
    if is_file(path) {
        Ok(())
    } else {
        crate::settings::remove_recent(projects, path);
        Err(format!(
            "Recent project no longer exists: {}",
            path.display()
        ))
    }
}

fn validate_recent_folder(
    folders: &mut Vec<std::path::PathBuf>,
    path: &std::path::Path,
    is_dir: impl FnOnce(&std::path::Path) -> bool,
) -> Result<(), String> {
    if is_dir(path) {
        Ok(())
    } else {
        folders.retain(|folder| folder != path);
        Err(format!(
            "Recent folder no longer exists: {}",
            path.display()
        ))
    }
}

impl StudioApp {
    fn palette_items(&self) -> Vec<PaletteItem> {
        let mut items = stage_items();
        if self.stage == Stage::Process {
            items.extend(ProcessSection::ALL.into_iter().map(|section| PaletteItem {
                label: format!("Apply {} settings to marked groups", section.name()),
                category: "params",
                keys: "",
                cmd: PaletteCmd::ApplySectionToMarked(section),
            }));
        }
        items.extend(reset_params_item(self.stage));
        let simple = [
            ("Mark shown", "groups", "", PaletteCmd::MarkAll),
            ("Clear marks", "groups", "", PaletteCmd::MarkNone),
            (
                "Apply all processing settings to marked groups",
                "params",
                "",
                PaletteCmd::ApplyAllProcessingToMarked,
            ),
            (
                "Copy column mapping to marked groups (same detection mode only)",
                "params",
                "",
                PaletteCmd::CopyMappingToMarked,
            ),
            ("Fit the current group", "fit", "", PaletteCmd::Fit),
            (
                "Export batch-fit results as CSV",
                "export",
                "",
                PaletteCmd::ExportBatchCsv,
            ),
            ("Save project…", "file", "", PaletteCmd::SaveProject),
            ("Open project…", "file", "⌘O", PaletteCmd::OpenProject),
            ("Import…", "file", "⇧⌘O", PaletteCmd::OpenFolder),
            ("Switch theme", "view", "", PaletteCmd::Theme),
            (
                "Check for updates · Stable / Nightly",
                "app",
                "",
                PaletteCmd::Updates,
            ),
            ("Undo", "edit", "⌘Z", PaletteCmd::Undo),
            ("Redo", "edit", "⇧⌘Z", PaletteCmd::Redo),
            ("Help", "app", "", PaletteCmd::Help),
            ("About rexafs", "app", "", PaletteCmd::About),
            ("Preferences…", "app", "⌘,", PaletteCmd::Preferences),
            ("Licenses", "app", "", PaletteCmd::Licenses),
            (
                "Storage · clean old installers and app copies",
                "app",
                "",
                PaletteCmd::Storage,
            ),
            ("Open Cu example", "file", "", PaletteCmd::Example),
            (
                "Open synthetic copper reduction · 50 frames + 3 references",
                "file",
                "",
                PaletteCmd::CopperSeries,
            ),
        ];
        for (label, category, keys, cmd) in simple {
            items.push(PaletteItem {
                label: label.into(),
                category,
                keys,
                cmd,
            });
        }
        items.extend(recent_items(&self.structure.settings));
        items.push(PaletteItem {
            label: "Show journal".into(),
            category: "view",
            keys: "",
            cmd: PaletteCmd::Journal,
        });
        items.push(PaletteItem {
            label: "Show problems (errors and warnings)".into(),
            category: "view",
            keys: "",
            cmd: PaletteCmd::Problems,
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
        for ix in catalog_items(self.catalog.len(), &self.group_registry, cap) {
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
        rank_items(query.trim(), self.palette_items())
            .into_iter()
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
        let focus = self.root_focus.clone();
        let handle = self.main_window;
        let view = cx.weak_entity();
        cx.defer(move |cx| {
            let _ = handle.update(cx, |_, window, cx| {
                let _ = view.update(cx, |app, cx| {
                    if app.palette.is_none() && !app.updates.open && !app.help.is_open() {
                        focus.focus(window, cx);
                    }
                });
            });
        });
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
        self.close_palette(cx);
        self.run_palette_cmd(cmd, cx);
    }

    fn run_palette_cmd(&mut self, cmd: PaletteCmd, cx: &mut Context<Self>) {
        match cmd {
            PaletteCmd::Stage(stage) => self.set_stage(stage, cx),
            PaletteCmd::ProcessSection(section) => self.focus_process_section(section, cx),
            PaletteCmd::MarkAll => self.mark_all(true, cx),
            PaletteCmd::MarkNone => self.mark_all(false, cx),
            PaletteCmd::ApplySectionToMarked(section) => {
                self.apply_scope_to_marked(ParamScope::Process(section), cx)
            }
            PaletteCmd::ApplyAllProcessingToMarked => {
                self.apply_scope_to_marked(ParamScope::All, cx)
            }
            PaletteCmd::CopyMappingToMarked => self.apply_scope_to_marked(ParamScope::Mapping, cx),
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
            PaletteCmd::RecentProject(path) => {
                match validate_recent_project(
                    &mut self.structure.settings.recent_projects,
                    &path,
                    std::path::Path::is_file,
                ) {
                    Ok(()) => self.load_project_path(path, cx),
                    Err(message) => {
                        self.status = message.into();
                        self.persist_recent_locations();
                    }
                }
            }
            PaletteCmd::RecentFolder(path) => {
                match validate_recent_folder(
                    &mut self.structure.settings.recent_import_folders,
                    &path,
                    std::path::Path::is_dir,
                ) {
                    Ok(()) => self.route_paths(vec![path], true, cx),
                    Err(message) => {
                        self.status = message.into();
                        self.persist_recent_locations();
                    }
                }
            }
            PaletteCmd::Theme => self.toggle_theme(cx),
            PaletteCmd::Updates => self.open_updates(cx),
            PaletteCmd::Help => self.open_help(cx),
            PaletteCmd::About => self.open_about(cx),
            PaletteCmd::Preferences => self.open_preferences(cx),
            PaletteCmd::Licenses => self.open_licenses(cx),
            PaletteCmd::Storage => self.open_storage(cx),
            PaletteCmd::Example => self.open_example(cx),
            PaletteCmd::CopperSeries => self.open_copper_series(cx),
            PaletteCmd::Undo => self.undo(cx),
            PaletteCmd::Redo => self.redo(cx),
            PaletteCmd::Journal => {
                self.journal.open = true;
                cx.notify();
            }
            PaletteCmd::Problems => {
                self.problems_batch = None;
                self.problems_open = true;
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
                        this.close_palette(cx);
                        this.run_palette_cmd(cmd.clone(), cx);
                    }))
                    .child(
                        div()
                            .w(px(64.))
                            .flex_none()
                            .text_size(px(11.))
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
                            .text_size(px(11.))
                            .text_color(t.text_muted)
                            .child(item.keys),
                    ),
            );
        }
        Some(
            div()
                .id("palette-overlay")
                .occlude()
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
                                .text_size(px(11.))
                                .text_color(t.text_muted)
                                .child("↑↓ move")
                                .child("↩ run")
                                .child("esc close"),
                        ),
                ),
        )
    }

    /// Mark displayed rows, or clear marks across the project.
    pub(crate) fn mark_all(&mut self, on: bool, cx: &mut Context<Self>) {
        if on {
            self.interaction_rows().mark_shown(&mut self.selection);
        } else {
            self.clear_selection(cx);
            return;
        }
        self.ensure_compare_loaded(cx);
        self.sync_param_fields(cx);
        cx.notify();
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn recent_project_validation_keeps_files_and_prunes_missing_or_directory_entries() {
        let valid = std::path::PathBuf::from("/project.rxs");
        let stale = std::path::PathBuf::from("/moved.rxs");
        let directory = std::path::PathBuf::from("/directory.rxs");
        let mut projects = vec![
            valid.clone(),
            stale.clone(),
            directory.clone(),
            stale.clone(),
        ];
        let original = projects.clone();
        let is_file = |p: &std::path::Path| p == valid;
        assert_eq!(
            validate_recent_project(&mut projects, &valid, is_file),
            Ok(())
        );
        assert_eq!(projects, original);
        for path in [&stale, &directory] {
            assert_eq!(
                validate_recent_project(&mut projects, path, is_file),
                Err(format!(
                    "Recent project no longer exists: {}",
                    path.display()
                ))
            );
        }
        assert_eq!(projects, vec![valid]);
    }

    #[test]
    fn recent_palette_entries_carry_exact_paths_and_separate_actions() {
        let settings = crate::settings::UserSettings {
            recent_projects: vec!["/one/same.rxs".into(), "/two/same.rxs".into()],
            recent_import_folders: vec!["/data/run".into()],
            ..Default::default()
        };
        let items = recent_items(&settings);
        assert_eq!(items.len(), 3);
        assert_ne!(items[0].label, items[1].label);
        assert_eq!(
            items[0].cmd,
            PaletteCmd::RecentProject(settings.recent_projects[0].clone())
        );
        assert_eq!(
            items[2].cmd,
            PaletteCmd::RecentFolder(settings.recent_import_folders[0].clone())
        );
        assert_eq!(items[2].label, "Import from recent folder ▸ /data/run");
        assert!(recent_items(&Default::default()).is_empty());
    }

    #[test]
    fn recent_folder_validation_keeps_directories_and_prunes_stale_entries() {
        let root = std::env::temp_dir().join(format!(
            "rexafs-recent-folder-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let missing = root.join("missing");
        let file = root.join("file");
        std::fs::write(&file, "data").unwrap();
        let mut folders = vec![missing.clone(), root.clone(), file.clone(), missing.clone()];
        let original = folders.clone();
        assert_eq!(
            validate_recent_folder(&mut folders, &root, std::path::Path::is_dir),
            Ok(())
        );
        assert_eq!(folders, original);
        for stale in [&missing, &file] {
            assert_eq!(
                validate_recent_folder(&mut folders, stale, std::path::Path::is_dir),
                Err(format!(
                    "Recent folder no longer exists: {}",
                    stale.display()
                ))
            );
            assert!(!folders.contains(stale));
        }
        assert_eq!(folders, vec![root.clone()]);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn palette_catalog_items_skip_removed_groups_before_the_cap() {
        let registry = crate::group_identity::GroupRegistry::default();
        let id = registry.register_source(
            Some(0),
            "/data/a.dat".into(),
            Default::default(),
            &Default::default(),
        );
        registry.set_excluded(&std::collections::BTreeSet::from([id]));
        assert_eq!(
            catalog_items(500, &registry, 2).collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert_eq!(registry.sources().len(), 1);
    }

    use super::*;

    #[test]
    fn label_matches_outrank_fuzzy_hits_and_state_changing_commands() {
        let item = |label: &str, category| PaletteItem {
            label: label.into(),
            category,
            keys: "",
            cmd: PaletteCmd::Help,
        };
        let ranked = rank_items(
            "linear",
            vec![
                item("Apply all processing settings to marked groups", "params"),
                item("Linear combination fit…", "analysis"),
                item("Go to Fit", "stage"),
            ],
        );
        assert_eq!(
            ranked.iter().map(|i| i.label.as_str()).collect::<Vec<_>>(),
            [
                "Linear combination fit…",
                "Apply all processing settings to marked groups"
            ]
        );
        let ranked = rank_items(
            "fit",
            vec![
                item("Export batch-fit results as CSV", "export"),
                item("Fit the current group", "fit"),
                item("Go to Fit", "stage"),
            ],
        );
        assert_eq!(
            ranked.iter().map(|i| i.category).collect::<Vec<_>>(),
            ["fit", "stage", "export"]
        );
        assert!(rank_items("zzz", vec![item("Help", "app")]).is_empty());
        assert_eq!(rank_items("", vec![item("Help", "app")]).len(), 1);
    }

    #[test]
    fn reset_palette_entry_only_exists_for_processing_stages() {
        for stage in [Stage::Data, Stage::Process] {
            assert_eq!(
                reset_params_item(stage).unwrap().cmd,
                PaletteCmd::ResetParams
            );
        }
        for stage in [Stage::Fit, Stage::Series, Stage::Publish] {
            assert!(reset_params_item(stage).is_none());
        }
    }

    #[test]
    fn stage_entries_show_five_shortcuts_and_keep_former_stage_commands() {
        let items = stage_items();
        let shortcuts = items
            .iter()
            .filter(|i| !i.keys.is_empty())
            .map(|i| (i.label.as_str(), i.keys))
            .collect::<Vec<_>>();
        assert_eq!(
            shortcuts,
            [
                ("Go to Data", "⌘1"),
                ("Go to Process", "⌘2"),
                ("Go to Fit", "⌘3"),
                ("Go to Series", "⌘4"),
                ("Go to Publish", "⌘5"),
            ]
        );
        // Typing a former stage name still finds a command that opens its
        // section of Process.
        for section in ProcessSection::ALL {
            let ranked = rank_items(&section.name().to_lowercase(), stage_items());
            assert_eq!(ranked[0].cmd, PaletteCmd::ProcessSection(section));
        }
    }
}
