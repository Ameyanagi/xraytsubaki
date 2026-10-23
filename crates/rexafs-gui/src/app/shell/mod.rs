//! Stage-based shell (doc/gui-ux-design-v2.md): the pipeline is the
//! navigation. A stage strip across the top selects *the* plot and *the*
//! parameters; groups live on the left, the inspector on the right.
//!
//! These are child modules of `app` so they can render straight off the
//! private `StudioApp` state without a pub(crate) field explosion; `app.rs`
//! keeps state and jobs, the shell keeps presentation.

pub(crate) mod assistant;
pub(crate) mod assistant_actions;
mod assistant_analysis;
mod assistant_receipts;
pub(crate) mod assistant_shell;
mod assistant_state;
mod bond_geometry;
pub mod center;
mod chrome;
pub(crate) mod controls;
mod depth_controls;
pub mod fit;
pub(crate) mod fit_preview;
pub mod fit_workspace;
pub(crate) mod fluorescence;
pub(crate) mod group_menu;
pub mod groups_panel;
pub mod handles;
pub(crate) mod help;
pub(crate) mod import_editor;
mod import_receipt;
mod import_review;
mod import_summary;
pub mod inspector;
mod joint_browser;
pub(crate) mod joint_fit;
pub mod journal;
pub(crate) mod live;
mod marked_removal;
pub(crate) mod measurement_import;
pub(crate) mod measurements;
mod molecular_geometry;
pub mod molecule_view;
pub(crate) mod normalization;
pub mod palette;
pub(crate) mod parameter_actions;
mod path_diagnostics;
pub mod path_picker;
pub(crate) mod path_routing;
pub(crate) mod peaks;
pub(crate) mod plot_export;
mod processing_reference;
pub(crate) mod publish;
pub(crate) mod rmc;
pub mod series;
mod spectrum_colors;
pub mod stage_strip;
mod structure_depth;
pub mod structure_view;
pub mod thumbnails;
pub mod tools;
pub(crate) mod updates_view;
pub(crate) mod wavelet;

use gpui::{
    ClickEvent, Context, IntoElement, ParentElement, SharedString, Styled, div, prelude::*, px,
};

use super::{StudioApp, Workspace};
use crate::theme::Theme;

/// Pipeline stages, in pipeline order. The number is the ⌘-shortcut.
///
/// Normalization, background removal and the Fourier transform share one
/// Process stage: they are sections of one inspector and their plots update
/// together. Releases up to 0.2.14 had separate Normalize, Background and
/// Transform stages; [`Stage::parse`] still accepts those names.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stage {
    Data,
    Process,
    Fit,
    Series,
    Publish,
}

impl Stage {
    pub const ALL: [Stage; 5] = [
        Stage::Data,
        Stage::Process,
        Stage::Fit,
        Stage::Series,
        Stage::Publish,
    ];

    pub fn number(self) -> usize {
        Self::ALL.iter().position(|s| *s == self).unwrap_or(0) + 1
    }

    /// Shortcut label shown in the command palette (⌘1…⌘5, Ctrl on
    /// Windows and Linux). `studio_keybindings` binds the same numbers.
    pub fn shortcut(self) -> &'static str {
        match self {
            Stage::Data => "⌘1",
            Stage::Process => "⌘2",
            Stage::Fit => "⌘3",
            Stage::Series => "⌘4",
            Stage::Publish => "⌘5",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Stage::Data => "Data",
            Stage::Process => "Process",
            Stage::Fit => "Fit",
            Stage::Series => "Series",
            Stage::Publish => "Publish",
        }
    }

    /// Resolve a stage name (case-insensitive) from scripted launches
    /// (`REXAFS_STAGE`), assistant navigation and saved assistant receipts.
    /// The names of the former stages, `normalize`, `background` and
    /// `transform`, map to the Process stage and its matching section.
    pub fn parse(name: &str) -> Option<(Stage, Option<ProcessSection>)> {
        Some(match name.trim().to_ascii_lowercase().as_str() {
            "data" => (Stage::Data, None),
            "process" => (Stage::Process, None),
            "normalize" => (Stage::Process, Some(ProcessSection::Normalize)),
            "background" => (Stage::Process, Some(ProcessSection::Background)),
            "transform" => (Stage::Process, Some(ProcessSection::Transform)),
            "fit" => (Stage::Fit, None),
            "series" => (Stage::Series, None),
            "publish" => (Stage::Publish, None),
            _ => return None,
        })
    }

    /// Legacy workspace the stage maps onto: Data and Process share the
    /// explore plot machinery, Fit and Series keep their centers.
    pub fn workspace(self) -> Workspace {
        match self {
            Stage::Fit => Workspace::Fit,
            Stage::Series => Workspace::Operando,
            _ => Workspace::Explore,
        }
    }

    pub fn is_processing(self) -> bool {
        matches!(self, Stage::Data | Stage::Process)
    }

    /// Limit fitted overlays to the Process stage without clearing the user's
    /// toggle preferences, so returning to Process restores them.
    pub(crate) fn plot_options(
        self,
        mut view: crate::plotting::ViewOptions,
    ) -> crate::plotting::ViewOptions {
        view.show_pre &= self == Self::Process;
        view.show_post &= self == Self::Process;
        view.show_bkg &= self == Self::Process;
        view
    }
}

/// The three collapsible inspector sections of the Process stage, in
/// pipeline order: pre-/post-edge normalization, AUTOBK background removal
/// (μ(E) → χ(k)), and the forward/back Fourier transforms (χ(k) → χ(R) → χ(q)).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ProcessSection {
    Normalize,
    Background,
    Transform,
}

impl ProcessSection {
    pub const ALL: [ProcessSection; 3] = [
        ProcessSection::Normalize,
        ProcessSection::Background,
        ProcessSection::Transform,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Normalize => "Normalize",
            Self::Background => "Background",
            Self::Transform => "Transform",
        }
    }

    /// `ui.sections` key recording that the user collapsed this section.
    /// Sections start expanded, so the key marks the exception.
    pub(crate) fn collapsed_key(self) -> &'static str {
        match self {
            Self::Normalize => "process-normalize-collapsed",
            Self::Background => "process-background-collapsed",
            Self::Transform => "process-transform-collapsed",
        }
    }

    /// Position among the inspector's scroll children (for scrolling to it).
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }
}

/// Which groups the stage plots show (Athena: current vs marked).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PlotScope {
    Current,
    Marked,
}

/// μ(E)-family quantity for the Data plot. In Process it chooses normalized
/// or flattened μ(E) for the second linked plot.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EQuantity {
    Mu,
    Norm,
    Flat,
}

/// Fit stage main view.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FitView {
    Both,
    K,
    R,
    Q,
}

/// Per-stage view state that is presentation only (never persisted).
#[derive(Clone, Copy, Debug)]
pub struct StageView {
    pub scope: PlotScope,
    pub e_quantity: EQuantity,
    pub show_bkg: bool,
    pub show_re: bool,
    pub fit_view: FitView,
    pub fit_step: fit_workspace::FitStep,
    pub fit_model_tab: usize,
    pub fit_result_tab: usize,
    pub fit_show_paths: bool,
    pub fit_show_re: bool,
    pub fit_show_im: bool,
    pub fit_show_batch: bool,
    pub series_space: crate::app::SeriesSpace,
}

impl Default for StageView {
    fn default() -> Self {
        Self {
            scope: PlotScope::Current,
            e_quantity: EQuantity::Norm,
            show_bkg: true,
            show_re: false,
            fit_view: FitView::Both,
            fit_step: fit_workspace::FitStep::Structure,
            fit_model_tab: 0,
            fit_result_tab: 0,
            fit_show_paths: true,
            fit_show_re: false,
            fit_show_im: false,
            fit_show_batch: false,
            series_space: crate::app::SeriesSpace::Flat,
        }
    }
}

/// Status dot semantics shared by the stage strip and thumbnails.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StageStatus {
    Ok,
    Auto,
    Attention,
    Idle,
}

impl StageStatus {
    pub fn color(self, t: &Theme) -> gpui::Rgba {
        match self {
            StageStatus::Ok => t.success,
            StageStatus::Auto => t.accent,
            StageStatus::Attention => t.warn,
            StageStatus::Idle => t.text_muted,
        }
    }
}

/// Monospace face for numbers (tabular) in the chrome.
pub use crate::theme::MONO;

/// Uppercase, letter-spaced section label used by every panel.
pub fn section_label(t: &Theme, text: impl Into<SharedString>) -> impl IntoElement {
    div()
        .text_size(px(11.))
        .text_color(t.text_muted)
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .child(text.into().to_uppercase())
}

/// Bordered pill toggle. Filled with the soft accent when on.
pub fn chip(
    t: &Theme,
    id: impl Into<gpui::ElementId>,
    label: impl Into<SharedString>,
    on: bool,
) -> crate::accessibility::Control {
    let label = label.into();
    crate::accessibility::Control::new(div().id(id), label.clone(), accesskit::Role::Button)
        .selected(on)
        .tab_index(0)
        .key_context("Control")
        .h(px(22.))
        .px_2()
        .flex()
        .items_center()
        .rounded_full()
        .text_size(px(11.5))
        .cursor_pointer()
        .border_1()
        .whitespace_nowrap()
        .when(on, |d| {
            d.bg(gpui::Rgba {
                a: 0.16,
                ..t.accent
            })
            .border_color(t.accent)
            .text_color(t.text)
        })
        .when(!on, |d| d.border_color(t.border).text_color(t.text_muted))
        .hover(|d| d.bg(t.raised))
        .focus(|d| d.border_color(t.accent))
        .child(label)
}

/// One button of a segmented control.
pub fn segment(
    t: &Theme,
    id: impl Into<gpui::ElementId>,
    label: impl Into<SharedString>,
    on: bool,
    first: bool,
) -> crate::accessibility::Control {
    let label = label.into();
    crate::accessibility::Control::new(div().id(id), label.clone(), accesskit::Role::Tab)
        .selected(on)
        .tab_index(0)
        .key_context("Control")
        .h(px(24.))
        .px_2()
        .flex()
        .items_center()
        .text_size(px(11.5))
        .cursor_pointer()
        .whitespace_nowrap()
        .when(!first, |d| d.border_l_1().border_color(t.border))
        .when(on, |d| d.bg(t.accent).text_color(t.bg))
        .when(!on, |d| {
            d.text_color(t.text_muted).hover(|d| d.bg(t.raised))
        })
        .focus(|d| d.bg(t.raised).text_color(t.accent))
        .child(label)
}

/// Container for a row of [`segment`]s.
pub fn segmented(t: &Theme) -> gpui::Div {
    div()
        .flex()
        .items_center()
        .rounded_md()
        .border_1()
        .border_color(t.border)
        .bg(t.raised)
        .overflow_hidden()
}

/// Small bordered button.
pub fn button(
    t: &Theme,
    id: impl Into<gpui::ElementId>,
    label: impl Into<SharedString>,
    primary: bool,
) -> crate::accessibility::Control {
    let label = label.into();
    crate::accessibility::Control::new(div().id(id), label.clone(), accesskit::Role::Button)
        .tab_index(0)
        .key_context("Control")
        .h(px(24.))
        .px_2()
        .flex()
        .items_center()
        .gap_1()
        .rounded_md()
        .text_size(px(11.5))
        .font_weight(gpui::FontWeight::MEDIUM)
        .cursor_pointer()
        .whitespace_nowrap()
        .border_1()
        .when(primary, |d| {
            d.bg(t.accent).border_color(t.accent).text_color(t.bg)
        })
        .when(!primary, |d| {
            d.bg(t.raised)
                .border_color(t.border)
                .text_color(t.text)
                .hover(|d| d.border_color(t.accent))
        })
        .focus(|d| d.border_color(t.accent))
        .child(label)
}

impl StudioApp {
    /// Stage shortcuts must leave focus on a surviving element. A focused plot
    /// option can disappear in the next stage, stranding later key bindings.
    pub(crate) fn navigate_stage(
        &mut self,
        stage: Stage,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        self.root_focus.focus(window, cx);
        self.set_stage(stage, cx);
    }

    pub(crate) fn set_stage(&mut self, stage: Stage, cx: &mut Context<Self>) {
        let previous = self.stage;
        self.stage = stage;
        self.set_workspace(stage.workspace(), cx);
        if previous != stage {
            self.view.show_kwin = stage == Stage::Process;
            // An enlarged plot belongs to the Process grid; other stages show
            // their own plots with the shared legend strip.
            self.maximized = None;
            self.stage_view_changed(cx);
        }
    }

    /// Open the Process stage with `section` expanded and scrolled into view.
    /// The former Normalize / Background / Transform commands land here.
    pub(crate) fn focus_process_section(
        &mut self,
        section: ProcessSection,
        cx: &mut Context<Self>,
    ) {
        self.set_stage(Stage::Process, cx);
        self.ui.sections.remove(section.collapsed_key());
        self.set_side_panel_visible(assistant_shell::SidePanel::Inspector, true);
        self.inspector_scroll.scroll_to_top_of_item(section.index());
        cx.notify();
    }

    /// Keyboard/menu variant of [`Self::focus_process_section`]; see
    /// [`Self::navigate_stage`] for the focus handling.
    pub(crate) fn navigate_process_section(
        &mut self,
        section: ProcessSection,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        self.root_focus.focus(window, cx);
        self.focus_process_section(section, cx);
    }

    /// Stage view options feed the explore plot builders; changing them
    /// rebuilds the plots and re-syncs the drag handles.
    pub(crate) fn stage_view_changed(&mut self, cx: &mut Context<Self>) {
        self.invalidate_explore_plots(cx);
        self.sync_handles(cx);
        cx.notify();
    }

    /// The whole window.
    pub(crate) fn shell_root(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let center = match self.stage {
            Stage::Fit => self.fit_workspace(cx).into_any_element(),
            Stage::Series => self.series_stage_center(cx).into_any_element(),
            Stage::Publish => self.publish_panel(cx),
            _ => self.stage_center(cx).into_any_element(),
        };
        let groups = self
            .data_panel_open
            .then(|| self.groups_panel(cx).into_any_element());
        let inspector = (self.context_panel_open
            && (self.stage != Stage::Series
                || (self.measurements.overview && !self.live.open && self.series_ready()))
            && !matches!(self.stage, Stage::Fit | Stage::Publish))
        .then(|| self.inspector(cx).into_any_element());
        let assistant = self.assistant_panel(cx);
        let live_monitor = self.live_monitor_panel(cx);
        div()
            .id("studio-shell")
            .on_mouse_move(cx.listener(|this, event: &gpui::MouseMoveEvent, _, cx| {
                this.resize_assistant(event, cx)
            }))
            .on_mouse_up(
                gpui::MouseButton::Left,
                cx.listener(|this, _, _, cx| this.finish_assistant_resize(cx)),
            )
            .on_mouse_up_out(
                gpui::MouseButton::Left,
                cx.listener(|this, _, _, cx| this.finish_assistant_resize(cx)),
            )
            .size_full()
            .min_h_0()
            .min_w_0()
            .relative()
            .flex()
            .flex_col()
            .bg(t.bg)
            .text_color(t.text)
            .text_size(px(12.5))
            .child(self.top_bar(cx))
            .child(self.stage_strip(cx))
            .children(self.import_receipt(cx))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .flex()
                    .children(groups)
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .children(self.stale_plots_banner(cx))
                            .children(self.live_average_update_banner(cx))
                            .child(center),
                    )
                    .children(inspector)
                    .children(assistant)
                    .children(live_monitor),
            )
            .children(
                self.problems_open
                    .then(|| self.problems_panel(cx).into_any_element()),
            )
            .children(
                self.journal
                    .open
                    .then(|| self.journal_panel(cx).into_any_element()),
            )
            .child(self.status_bar(cx))
            .children(self.series_appearance_overlay(cx))
            .children(self.wavelet_appearance_overlay(cx))
            .children(self.live_menu_overlay(false, cx))
            .children(self.peak_menu_overlay(cx))
            .children(self.group_menu_overlay(cx))
            .children(self.palette_overlay(cx))
            .children(self.parameter_menu_overlay(cx))
            .children(self.parameter_context_overlay(cx))
            .children(self.updates_overlay(cx))
            .children(self.help_overlay(cx))
            .children(self.chrome_menu_overlay(cx))
            .when(self.assistant_resizing.is_some(), |d| {
                d.child(
                    div()
                        .id("assistant-resize-capture")
                        .absolute()
                        .inset_0()
                        .cursor(gpui::CursorStyle::ResizeLeftRight)
                        .occlude()
                        .on_mouse_move(cx.listener(|this, event: &gpui::MouseMoveEvent, _, cx| {
                            this.resize_assistant(event, cx)
                        }))
                        .on_mouse_up(
                            gpui::MouseButton::Left,
                            cx.listener(|this, _, _, cx| this.finish_assistant_resize(cx)),
                        ),
                )
            })
            .children(self.path_route_overlay(cx))
            .children(self.import_editor.clone())
    }

    /// Project identity and a small set of familiar actions.
    fn top_bar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        use self::controls::{Menu, icon_button};
        use crate::icons::Icon;
        let t = self.theme;
        let mut project = self
            .project_path
            .as_ref()
            .and_then(|p| p.file_stem())
            .or_else(|| self.source_dir.as_ref().and_then(|p| p.file_name()))
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.spectrum_label.to_string());
        if self.assistant_history_revision != self.assistant_history_saved_revision {
            project.push_str(" •");
        }
        let action = |id: &'static str,
                      glyph: Icon,
                      label: &'static str,
                      active: bool,
                      f: fn(&mut Self, &mut Context<Self>)| {
            icon_button(&t, id, glyph, label, active)
                .on_click(cx.listener(move |app, _, _, cx| f(app, cx)))
        };
        div()
            .h(px(38.))
            .min_w_0()
            .w_full()
            .flex_none()
            .flex()
            .items_center()
            .gap_1()
            .px_2()
            .bg(t.surface)
            .border_b_1()
            .border_color(t.border)
            .child(
                icon_button(
                    &t,
                    "project-menu",
                    Icon::Folder,
                    "Project",
                    self.ui.menu == Some(Menu::Project),
                )
                .on_click(cx.listener(|app, event, window, cx| {
                    app.open_chrome_menu(Menu::Project, event, window, cx)
                })),
            )
            .child(
                div()
                    .min_w_0()
                    .flex_1()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .child(if project.is_empty() {
                        "rexafs".into()
                    } else {
                        project
                    }),
            )
            .children(self.live_status_indicator(cx))
            .when(self.viewport_w >= 800., |d| {
                d.child(
                    action("undo", Icon::Undo, "Undo · ⌘Z", false, |a, c| a.undo(c))
                        .when(self.journal.undo.is_empty(), |d| {
                            d.disabled(true).opacity(0.4).tab_stop(false)
                        }),
                )
                .child(
                    action("redo", Icon::Redo, "Redo · ⇧⌘Z", false, |a, c| {
                        a.redo(c)
                    })
                    .when(self.journal.redo.is_empty(), |d| {
                        d.disabled(true).opacity(0.4).tab_stop(false)
                    }),
                )
            })
            .child(
                action(
                    "open-folder",
                    Icon::Import,
                    "Import files or folders · ⇧⌘O",
                    false,
                    |a, c| a.open_folder(c),
                )
                .w_auto()
                .px_2()
                .gap_1()
                .child("Import…"),
            )
            .child(action(
                "save-project",
                Icon::Save,
                "Save project · ⌘S",
                self.project_saving,
                |a, c| a.save_project(c),
            ))
            .child(action(
                "switch-theme",
                if t.mode == crate::theme::ThemeMode::Dark {
                    Icon::Sun
                } else {
                    Icon::Moon
                },
                if t.mode == crate::theme::ThemeMode::Dark {
                    "Switch to light theme"
                } else {
                    "Switch to dark theme"
                },
                false,
                |a, c| a.toggle_theme(c),
            ))
            .child(div().w(px(1.)).h(px(18.)).mx_1().bg(t.border))
            .child(
                icon_button(
                    &t,
                    "cmdk",
                    Icon::Search,
                    "Find actions, tools, or groups · ⌘K",
                    false,
                )
                .on_click(cx.listener(|app, _, window, cx| app.open_palette(window, cx))),
            )
            .child(action(
                "toggle-groups",
                Icon::PanelLeft,
                "Groups · ⌘B",
                self.data_panel_open,
                |a, c| {
                    a.toggle_side_panel(assistant_shell::SidePanel::Groups);
                    c.notify();
                },
            ))
            .when(!matches!(self.stage, Stage::Fit | Stage::Publish), |d| {
                d.child(action(
                    "toggle-inspector",
                    Icon::PanelRight,
                    "Parameters · ⌘J",
                    self.context_panel_open,
                    |a, c| {
                        a.toggle_side_panel(assistant_shell::SidePanel::Inspector);
                        c.notify();
                    },
                ))
            })
            .child(action(
                "assistant-window",
                Icon::Chat,
                "Assistant",
                self.assistant_host == assistant_shell::AssistantHost::Docked,
                |a, c| a.open_assistant(c),
            ))
            .when(
                self.updates.result.as_ref().is_some_and(|r| r.available),
                |d| {
                    // A badge dot, not the "panel on" style, so the notice is
                    // not confused with an active toggle.
                    d.child(
                        action(
                            "updates",
                            Icon::Download,
                            "Update available · open Updates",
                            false,
                            |a, c| a.open_updates(c),
                        )
                        .relative()
                        .child(
                            div()
                                .absolute()
                                .top(px(5.))
                                .right(px(5.))
                                .size(px(7.))
                                .rounded_full()
                                .bg(t.accent)
                                .border_1()
                                .border_color(t.surface),
                        ),
                    )
                },
            )
            .child(action("help", Icon::Help, "Help", false, |a, c| {
                a.open_help(c)
            }))
    }
}

#[cfg(test)]
mod plot_overlay_tests {
    #[test]
    fn fitted_overlays_follow_stage_and_restore_on_return() {
        use super::{Stage, Theme};
        use crate::params::{PipelineParams, process_file};
        use crate::plotting::{
            QuadTrace, SeriesKey, ViewOptions, build_quadrant_specs, quantity_quadrant_specs,
        };
        use std::{path::Path, sync::Arc};
        let file =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/projects/data/cu_150k.xmu");
        let polynomial = process_file(&file, &PipelineParams::default()).unwrap();
        let mut mback = polynomial.clone();
        mback
            .set_normalization_method(rexafs::MBack::for_edge("Cu", "K"))
            .unwrap();
        mback.calc_background().unwrap();
        let preferences = ViewOptions {
            show_pre: true,
            show_post: true,
            show_bkg: true,
            ..Default::default()
        };
        for (spectrum, is_mback) in [(polynomial, false), (mback, true)] {
            let traces = [QuadTrace {
                color_index: 0,
                color: None,
                label: "Experimental Cu".into(),
                sp: Arc::new(spectrum),
                active: true,
            }];
            // Every stage, then Process again: the preferences survive.
            for stage in Stage::ALL.into_iter().chain([Stage::Process]) {
                let visible = stage.plot_options(preferences);
                let specs = quantity_quadrant_specs(
                    &traces,
                    &visible,
                    &Theme::dark(),
                    true,
                    crate::params::Quantity::RawMu,
                );
                let keys = specs[0].series.iter().map(|s| s.key).collect::<Vec<_>>();
                let process = stage == Stage::Process;
                // The linked μ(E) plot carries the spline and the selected
                // normalization fit together.
                assert_eq!(keys.contains(&SeriesKey::Bkg), process);
                assert_eq!(keys.contains(&SeriesKey::MbackFit), is_mback && process);
                for key in [SeriesKey::PreEdge, SeriesKey::PostEdge] {
                    assert_eq!(keys.contains(&key), !is_mback && process);
                }
                if !process {
                    assert_eq!(keys, [SeriesKey::Trace(0)]);
                } else {
                    // Turning Spline and Pre/post off leaves no hidden fit.
                    let hidden = stage.plot_options(ViewOptions {
                        show_bkg: false,
                        show_pre: false,
                        show_post: false,
                        ..preferences
                    });
                    assert_eq!(
                        build_quadrant_specs(&traces, &hidden, &Theme::dark(), true)[0]
                            .series
                            .len(),
                        1
                    );
                }
            }
        }
        assert!(preferences.show_pre && preferences.show_post && preferences.show_bkg);
    }
}

#[cfg(test)]
mod stage_tests {
    use super::{ProcessSection, Stage};

    #[test]
    fn five_stages_are_numbered_in_pipeline_order() {
        assert_eq!(
            Stage::ALL.map(Stage::name),
            ["Data", "Process", "Fit", "Series", "Publish"]
        );
        for (i, stage) in Stage::ALL.into_iter().enumerate() {
            assert_eq!(stage.number(), i + 1);
            assert_eq!(stage.shortcut(), format!("⌘{}", i + 1));
        }
    }

    #[test]
    fn stage_names_round_trip_and_former_stages_map_to_process() {
        for stage in Stage::ALL {
            assert_eq!(Stage::parse(stage.name()), Some((stage, None)));
            assert_eq!(
                Stage::parse(&stage.name().to_lowercase()),
                Some((stage, None))
            );
        }
        // Saved assistant receipts and REXAFS_STAGE values from 0.2.14 and
        // earlier name the three former processing stages.
        for (name, section) in [
            ("normalize", ProcessSection::Normalize),
            ("Background", ProcessSection::Background),
            ("TRANSFORM", ProcessSection::Transform),
        ] {
            assert_eq!(Stage::parse(name), Some((Stage::Process, Some(section))));
            assert_eq!(Stage::parse(section.name()).unwrap().1, Some(section));
        }
        assert_eq!(Stage::parse("wavelet"), None);
        assert_eq!(Stage::parse(""), None);
    }

    #[test]
    fn process_sections_have_distinct_order_and_keys() {
        let keys = ProcessSection::ALL.map(ProcessSection::collapsed_key);
        assert!(keys[0] != keys[1] && keys[1] != keys[2] && keys[0] != keys[2]);
        for (i, section) in ProcessSection::ALL.into_iter().enumerate() {
            assert_eq!(section.index(), i);
        }
    }
}
