//! Stage-based shell (doc/gui-ux-design-v2.md): the pipeline is the
//! navigation. A stage strip across the top selects *the* plot and *the*
//! parameters; groups live on the left, the inspector on the right.
//!
//! These are child modules of `app` so they can render straight off the
//! private `StudioApp` state without a pub(crate) field explosion; `app.rs`
//! keeps state and jobs, the shell keeps presentation.

pub mod center;
pub mod fit;
pub mod groups_panel;
pub mod handles;
pub mod inspector;
pub mod journal;
pub mod palette;
pub mod series;
pub mod stage_strip;
pub mod thumbnails;
pub mod tools;

use gpui::{
    ClickEvent, Context, IntoElement, ParentElement, SharedString, Styled, div, prelude::*, px,
};

use super::{StudioApp, Workspace};
use crate::theme::Theme;

/// Pipeline stages, in pipeline order. The number is the ⌘-shortcut.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stage {
    Data,
    Normalize,
    Background,
    Transform,
    Fit,
    Series,
}

impl Stage {
    pub const ALL: [Stage; 6] = [
        Stage::Data,
        Stage::Normalize,
        Stage::Background,
        Stage::Transform,
        Stage::Fit,
        Stage::Series,
    ];

    pub fn number(self) -> usize {
        Self::ALL.iter().position(|s| *s == self).unwrap_or(0) + 1
    }

    pub fn name(self) -> &'static str {
        match self {
            Stage::Data => "Data",
            Stage::Normalize => "Normalize",
            Stage::Background => "Background",
            Stage::Transform => "Transform",
            Stage::Fit => "Fit",
            Stage::Series => "Series",
        }
    }

    /// Legacy workspace the stage maps onto: the four processing stages share
    /// the explore plot machinery, Fit and Series keep their centers.
    pub fn workspace(self) -> Workspace {
        match self {
            Stage::Fit => Workspace::Fit,
            Stage::Series => Workspace::Operando,
            _ => Workspace::Explore,
        }
    }

    pub fn is_processing(self) -> bool {
        matches!(
            self,
            Stage::Data | Stage::Normalize | Stage::Background | Stage::Transform
        )
    }
}

/// Which groups the stage plots show (Athena: current vs marked).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PlotScope {
    Current,
    Marked,
}

/// μ(E)-family quantity for the Data / Normalize plot.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EQuantity {
    Mu,
    Norm,
    Flat,
}

/// Background stage main view.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BkgView {
    /// μ(E) with the AUTOBK spline over |χ(R)| with the R < Rbkg region.
    Energy,
    /// k-weighted χ(k) over |χ(R)|.
    K,
}

/// Transform stage main view.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TfView {
    K,
    R,
    Q,
    Both,
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
    pub bkg_view: BkgView,
    pub tf_view: TfView,
    pub show_bkg: bool,
    pub show_re: bool,
    pub fit_view: FitView,
    pub fit_show_paths: bool,
    pub fit_show_re: bool,
    pub fit_show_feff: bool,
    pub fit_show_batch: bool,
    pub series_space: crate::app::SeriesSpace,
}

impl Default for StageView {
    fn default() -> Self {
        Self {
            scope: PlotScope::Current,
            e_quantity: EQuantity::Norm,
            bkg_view: BkgView::Energy,
            tf_view: TfView::Both,
            show_bkg: true,
            show_re: false,
            fit_view: FitView::Both,
            fit_show_paths: true,
            fit_show_re: false,
            fit_show_feff: false,
            fit_show_batch: false,
            series_space: crate::app::SeriesSpace::Energy,
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
pub const MONO: &str = "Menlo";

/// Uppercase, letter-spaced section label used by every panel.
pub fn section_label(t: &Theme, text: impl Into<SharedString>) -> impl IntoElement {
    div()
        .text_size(px(10.5))
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
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
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
        .child(label.into())
}

/// One button of a segmented control.
pub fn segment(
    t: &Theme,
    id: impl Into<gpui::ElementId>,
    label: impl Into<SharedString>,
    on: bool,
    first: bool,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
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
        .child(label.into())
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
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
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
        .child(label.into())
}

impl StudioApp {
    pub(crate) fn set_stage(&mut self, stage: Stage, cx: &mut Context<Self>) {
        let previous = self.stage;
        self.stage = stage;
        self.set_workspace(stage.workspace(), cx);
        if previous != stage {
            self.stage_view_changed(cx);
        }
    }

    /// Stage view options feed the explore plot builders; changing them
    /// rebuilds the plots and re-syncs the drag handles.
    pub(crate) fn stage_view_changed(&mut self, cx: &mut Context<Self>) {
        self.view.show_kwin = self.stage == Stage::Transform;
        self.invalidate_explore_plots(cx);
        self.sync_handles(cx);
        cx.notify();
    }

    /// The whole window.
    pub(crate) fn shell_root(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let center = match self.stage {
            Stage::Fit => self.fit_stage_center(cx).into_any_element(),
            Stage::Series => self.series_stage_center(cx).into_any_element(),
            _ => self.stage_center(cx).into_any_element(),
        };
        let groups = self
            .data_panel_open
            .then(|| self.groups_panel(cx).into_any_element());
        let inspector = self
            .context_panel_open
            .then(|| self.inspector(cx).into_any_element());
        div()
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
                            .child(center),
                    )
                    .children(inspector),
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
            .children(self.palette_overlay(cx))
    }

    /// Brand · project · actions (open folder / project, theme).
    fn top_bar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let project: SharedString = self
            .source_dir
            .as_ref()
            .and_then(|d| d.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_else(|| self.spectrum_label.to_string())
            .into();
        let action = |id: &'static str,
                      label: &'static str,
                      f: fn(&mut Self, &mut Context<Self>)|
         -> gpui::Stateful<gpui::Div> {
            div()
                .id(id)
                .h(px(24.))
                .px_2()
                .flex()
                .items_center()
                .rounded_md()
                .text_size(px(11.5))
                .text_color(t.text_muted)
                .cursor_pointer()
                .hover(|d| d.bg(t.raised).text_color(t.text))
                .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| f(this, cx)))
                .child(label)
        };
        div()
            .h(px(38.))
            .w_full()
            .min_w_0()
            .flex_none()
            .flex()
            .items_center()
            .gap_3()
            .px_3()
            .bg(t.surface)
            .border_b_1()
            .border_color(t.border)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(div().w(px(16.)).h(px(16.)).rounded_sm().bg(t.accent))
                    .child("XrayTsubaki"),
            )
            .child(div().text_color(t.text_muted).child("›"))
            .child(
                div()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .child(project),
            )
            .child(div().flex_1())
            .child(
                div()
                    .id("cmdk")
                    .h(px(24.))
                    .px_2()
                    .min_w(px(220.))
                    .flex()
                    .items_center()
                    .gap_2()
                    .rounded_md()
                    .border_1()
                    .border_color(t.border)
                    .bg(t.bg)
                    .text_size(px(11.5))
                    .text_color(t.text_muted)
                    .cursor_pointer()
                    .hover(|d| d.border_color(t.accent))
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        this.open_palette(window, cx);
                    }))
                    .child("Search actions, tools, groups…")
                    .child(div().flex_1())
                    .child(div().font_family(MONO).text_size(px(10.5)).child("⌘K")),
            )
            .child(action("undo", "↶", |this, cx| this.undo(cx)))
            .child(action("redo", "↷", |this, cx| this.redo(cx)))
            .child(action("open-folder", "Open folder…", |this, cx| {
                this.open_folder(cx)
            }))
            .child(action("open-project", "Open project…", |this, cx| {
                this.open_project(cx)
            }))
            .child(action("save-project", "Save project", |this, cx| {
                this.save_project(cx)
            }))
            .child(action("theme-toggle", "Theme", |this, cx| {
                this.toggle_theme(cx)
            }))
    }
}
