//! Center for the Data and Process stages: a plot bar, the stage's main
//! plot(s) with drag handles, the shared legend strip, and (in Data) the
//! four-thumbnail ripple strip.
//!
//! Process shows four linked plots in a 2 × 2 grid — μ(E), normalized μ(E),
//! kʷχ(k) and |χ(R)| — built from the same processing result, so one edit
//! updates all of them. Any plot can be enlarged and restored.

use gpui::{
    ClickEvent, Context, IntoElement, ParentElement, SharedString, Styled, div, prelude::*, px,
};

use super::{
    EQuantity, MONO, PlotScope, ProcessSection, Stage, StageStatus, chip, segment, segmented,
};
use crate::app::{ParamKey, StudioApp};
use crate::theme::Theme;

/// Quadrant slots in `StudioApp::quadrants` (built by `build_quadrant_specs`).
pub const PLOT_MU: usize = 0;
pub const PLOT_NORM: usize = 1;
pub const PLOT_CHIK: usize = 2;
pub const PLOT_CHIR: usize = 3;
pub const PLOT_CHIQ: usize = 4;

fn current_label(
    selected: Option<usize>,
    entry_label: impl FnOnce(usize) -> String,
) -> SharedString {
    entry_label(selected.unwrap_or(crate::app::NO_ENTRY)).into()
}

/// A 1 px vertical rule between clusters of controls in a plot bar.
fn bar_divider(t: &Theme) -> gpui::Div {
    div().flex_none().w(px(1.)).h(px(16.)).bg(t.border)
}

impl StudioApp {
    pub(crate) fn stage_center(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        if self.fluorescence.open && !self.fluorescence_matches_current() {
            self.fluorescence.close();
        }
        if self.stage == Stage::Data && self.fluorescence.open {
            return div()
                .flex_1()
                .min_w_0()
                .flex()
                .child(self.fluorescence_center(cx));
        }
        if self.stage == Stage::Data && self.peaks.open {
            return div().flex_1().min_w_0().flex().child(self.peak_center(cx));
        }
        if self.normalization.open && !self.normalization_matches_current() {
            self.normalization.open = false;
        }
        // A group without χ(k) (for example a difference spectrum) has no
        // wavelet view; drop it so the plot bar keeps its controls.
        if self.wavelet.open && !process_views(self.spectrum_quantity).wavelet {
            self.wavelet.hide();
        }
        match (self.stage == Stage::Process)
            .then(|| active_process_overlay(self.wavelet.open, self.normalization.open))
        {
            Some(ProcessOverlay::Normalization) => {
                return div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .child(self.normalization_center(cx));
            }
            Some(ProcessOverlay::Wavelet) => {
                if !self.wavelet_matches_current() {
                    self.open_wavelet(cx);
                }
                return div()
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .child(self.plot_bar(cx))
                    .child(self.wavelet_center(cx));
            }
            _ => {}
        }
        let t = self.theme;
        let ready = self.quadrants.len() > PLOT_CHIQ;
        if !ready && self.current_path.as_os_str().is_empty() && !self.catalog.scanning {
            return div().flex_1().flex().child(self.empty_drop_target(cx));
        }
        let mut column = div()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .flex()
            .flex_col()
            .child(self.plot_bar(cx));
        if self.stage == Stage::Process && self.xanes_only() {
            column = column.child(self.xanes_only_notice(cx));
        }
        if let Some(weight) = self.mixed_overlay_weight {
            column = column.child(div().px_3().py_1().text_size(px(11.5)).text_color(t.warn)
                .child(format!("Mixed FT weights: χ(k) uses k^{weight} for all curves. R/q curves retain each group's weight (shown in the legend).")));
        }
        if !ready {
            return column.child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(t.text_muted)
                    .child(self.status.clone()),
            );
        }
        let tool_preview = (self.stage == Stage::Data && self.tool_preview_current(cx))
            .then(|| self.tools.preview_plot.clone())
            .flatten();
        let has_tool_preview = tool_preview.is_some();
        let plots: Vec<(usize, SharedString)> = if tool_preview.is_some()
            || (self.stage == Stage::Data && self.analysis.plot.is_some())
        {
            Vec::new()
        } else {
            self.stage_plots()
        };
        let mut area = div()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_2()
            .px_3()
            .pt_2();
        if let Some(plot) = tool_preview {
            area = area.child(
                div()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(div().text_size(px(11.5)).text_color(t.text_muted).child(
                        if self.tools.open == Some(super::tools::Tool::Align) {
                            if self.tools.preview_running {
                                "Updating alignment…"
                            } else {
                                "XANES · derivative peaks scaled to 1"
                            }
                        } else {
                            "Preview · original / result / standard"
                        },
                    ))
                    .child(div().flex_1().min_h_0().min_w_0().child(plot)),
            );
        }
        // First-run caption: it disappears for good after the first handle drag.
        let range_hint = !self.handles.hidden
            && !self.structure.settings.range_hint_seen
            && self.stage == Stage::Process;
        let hint = || {
            div()
                .flex_none()
                .px_3()
                .text_size(px(11.))
                .text_color(t.text_muted)
                .child("Drag the tabs at the top of a plot to change its range")
        };
        if self.stage == Stage::Process && plots.len() > 1 {
            // Linked plots, two per row: μ(E) | normalized μ(E) over
            // kʷχ(k) | |χ(R)|.
            let mut cards = plots
                .into_iter()
                .map(|(index, title)| self.plot_card(index, title, cx).into_any_element())
                .collect::<Vec<_>>()
                .into_iter();
            let mut grid = div().flex_1().min_h_0().min_w_0().flex().flex_col().gap_2();
            while let Some(first) = cards.next() {
                grid = grid.child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .min_w_0()
                        .flex()
                        .gap_2()
                        .child(first)
                        .children(cards.next()),
                );
            }
            area = area.child(grid).when(range_hint, |d| d.child(hint()));
        } else {
            for (position, (index, title)) in plots.into_iter().enumerate() {
                area = area.child(self.plot_card(index, title, cx));
                if position == 0 && range_hint {
                    area = area.child(hint());
                }
            }
        }
        if self.stage == Stage::Data
            && let Some(plot) = self.analysis.plot.clone()
            && let Some(tool) = self.analysis.shown
        {
            let title: SharedString = match tool {
                super::tools::Tool::Lcf => "Linear combination fit".into(),
                super::tools::Tool::Pca => self
                    .analysis
                    .pca
                    .as_ref()
                    .map(|m| {
                        format!(
                            "PCA · {} · {}",
                            crate::text::plural(m.n_spectra(), "spectrum"),
                            if m.centered {
                                "mean subtracted"
                            } else {
                                "mean retained"
                            }
                        )
                    })
                    .unwrap_or_default()
                    .into(),
                _ => "MCR-ALS · estimated components".into(),
            };
            let views = self.analysis_view_bar(tool, cx);
            area = area.child(
                div()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .rounded_lg()
                    .bg(t.raised)
                    .border_1()
                    .border_color(t.border)
                    .child(
                        div()
                            .flex_none()
                            .px_3()
                            .pt_2()
                            .text_size(px(11.5))
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(div().flex_1().child(title))
                            .child(self.plot_export_button(
                                "analysis-export",
                                super::plot_export::Target::Analysis,
                                cx,
                            )),
                    )
                    .child(views)
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .min_w_0()
                            .p_1()
                            .relative()
                            .child(plot)
                            .child(self.measure_card(300, cx)),
                    ),
            );
        }
        if self.stage == Stage::Data
            && !self.handles.hidden
            && self.analysis.plot.is_none()
            && let Some((lo, hi)) = self.analysis_energy_interval(cx)
        {
            column=column.child(div().px_3().text_size(px(11.)).text_color(t.text_muted)
                .child(format!("Analysis interval: {lo:.1}–{hi:.1} eV · dashed boundaries on the spectrum plot. Viewing presets change only the zoom.")));
        }
        column = column.child(area);
        if !has_tool_preview
            && self.analysis.plot.is_none()
            && self.view.legend
            && !self.legend_entries.is_empty()
        {
            column = column.child(self.legend_strip());
        }
        // The Process grid already shows the four overview quantities.
        column.when(self.ui.overview && self.stage != Stage::Process, |d| {
            d.child(self.thumbnail_strip(cx))
        })
    }

    /// Whether the current group carries a fluorescence self-absorption
    /// correction. Such corrections are qualified for XANES only, so Process
    /// keeps normalization but withholds background removal and transforms.
    pub(crate) fn xanes_only(&self) -> bool {
        self.current_group_index()
            .is_some_and(|i| !self.correction_sources(i).is_empty())
    }

    fn xanes_only_notice(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        div()
            .flex_none()
            .px_3()
            .py_1()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .text_size(px(11.5))
            .text_color(t.warn)
            .child(
                "This fluorescence correction is qualified for XANES. Use the original \
                 spectrum for EXAFS; χ(k) and |χ(R)| are not shown.",
            )
            .child(
                super::button(&t, "fluo-history-return", "Correction history", false).on_click(
                    cx.listener(|a, _, _, cx| {
                        a.set_stage(Stage::Data, cx);
                        a.open_fluorescence(cx);
                    }),
                ),
            )
    }

    fn analysis_view_bar(
        &self,
        tool: super::tools::Tool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        use super::tools::Tool;
        use crate::plotting::analysis::{McrView, PcaScale, PcaView};
        let t = self.theme;
        let mut row = div().px_3().py_1().flex().flex_wrap().gap_1();
        if matches!(tool, Tool::Mcr | Tool::Lcf) {
            row = row.child(
                chip(&t, "analysis-to-groups", "Add to groups", false).on_click(
                    cx.listener(move |this, _, _, cx| this.add_analysis_groups(tool, cx)),
                ),
            );
        }
        if tool == Tool::Pca {
            for (i, view) in PcaView::ALL.into_iter().enumerate() {
                row = row.child(
                    chip(
                        &t,
                        SharedString::from(format!("pca-view-{i}")),
                        view.label(),
                        self.analysis.pca_view == view,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.analysis.pca_view = view;
                        this.rebuild_analysis_plot(cx);
                        cx.notify();
                    })),
                );
            }
            if matches!(
                self.analysis.pca_view,
                PcaView::Scree | PcaView::Cumulative | PcaView::Indicator | PcaView::Residual
            ) {
                row = row.child(
                    chip(
                        &t,
                        "pca-show-all",
                        "All components",
                        self.analysis.pca_all_components,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.analysis.pca_all_components = !this.analysis.pca_all_components;
                        this.rebuild_analysis_plot(cx);
                        cx.notify();
                    })),
                );
            }
            if matches!(
                self.analysis.pca_view,
                PcaView::Scree | PcaView::Indicator | PcaView::Residual
            ) {
                row = row.child(div().px_2().child("Y axis"));
                for scale in [PcaScale::Linear, PcaScale::Log] {
                    row = row.child(
                        chip(
                            &t,
                            SharedString::from(format!("pca-scale-{}", scale.label())),
                            scale.label(),
                            self.analysis.pca_scale == scale,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.analysis.pca_scale = scale;
                            this.rebuild_analysis_plot(cx);
                            cx.notify();
                        })),
                    );
                }
            }
        } else if tool == Tool::Mcr {
            for (i, view) in McrView::ALL.into_iter().enumerate() {
                row = row.child(
                    chip(
                        &t,
                        SharedString::from(format!("mcr-view-{i}")),
                        view.label(),
                        self.analysis.mcr_view == view,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.analysis.mcr_view = view;
                        this.rebuild_analysis_plot(cx);
                        cx.notify();
                    })),
                );
            }
        }
        if tool == Tool::Mcr {
            row = row.child(
                chip(
                    &t,
                    "mcr-reference-compare",
                    "Compare marked standards",
                    false,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    if let Err(error) = this.compare_mcr_references(cx) {
                        this.tools.message = error.into();
                        cx.notify();
                    }
                })),
            );
        }
        row = row.child(
            chip(&t, "analysis-back", "Back to spectra", false).on_click(cx.listener(
                |this, _, _, cx| {
                    this.analysis.shown = None;
                    this.analysis.plot = None;
                    cx.notify();
                },
            )),
        );
        let caption = match tool {
            Tool::Pca => match self.analysis.pca_view {
                PcaView::Scree => {
                    if self.analysis.pca_all_components {
                        "All components"
                    } else {
                        "First 12 components (or all if fewer)"
                    }
                }
                PcaView::Residual => {
                    "Zero components reconstruct only the mean (or zero without centering). Lower error does not prove chemical identity."
                }
                PcaView::Cumulative => {
                    "Choose a retained fraction; this threshold alone does not identify chemical species."
                }
                PcaView::Indicator => {
                    "IND is a rank heuristic. Minima in numerical roundoff do not indicate chemical components."
                }
                PcaView::Loadings => {
                    "Retained orthonormal directions; these are not pure chemical spectra."
                }
                PcaView::Similarity => {
                    "Cosine similarity of retained scores · −1 to +1 · zero vectors are undefined."
                }
                PcaView::Target => {
                    "Current target at calculation time · data, reconstruction and offset residual."
                }
                _ => {
                    "One point per training spectrum. Scores describe relative spectral variation, not chemical fractions."
                }
            },
            Tool::Mcr => match self.analysis.mcr_view {
                McrView::Spectra => {
                    "Component identities and order are not known without reference information."
                }
                McrView::Fractions => {
                    "All input samples in their saved order; no time axis is assumed."
                }
                McrView::Convergence => {
                    "Residual after each complete iteration · plotted factors use the best accepted iteration."
                }
                McrView::Residuals => {
                    "Root mean square reconstruction error for every input spectrum."
                }
                McrView::References => {
                    "Solid: recovered component. Dashed: matched reference. Matching does not change the blind fit."
                }
            },
            _ => "Data and fitted sum; weighted contributions and residual are vertically offset.",
        };
        let caption = if tool == Tool::Pca
            && matches!(
                self.analysis.pca_view,
                PcaView::Scree | PcaView::Indicator | PcaView::Residual
            )
            && self.analysis.pca_scale == PcaScale::Log
        {
            format!("{caption} · Log display floor 10⁻³²")
        } else {
            caption.to_string()
        };
        let saved_range = match tool {
            Tool::Lcf => self.analysis.lcf.as_ref().map(|r| (&r.x, r.space)),
            Tool::Pca => self.analysis.pca.as_ref().map(|m| (&m.x, m.space)),
            Tool::Mcr => self
                .analysis
                .mcr
                .as_ref()
                .map(|m| (&m.result.x, m.result.config.space)),
            _ => None,
        }
        .map(|(x, space)| {
            format!(
                "Saved analysis: {:.1}–{:.1} {} · {}",
                x[0],
                x[x.len() - 1],
                if matches!(space, rexafs::prelude::AnalysisSpace::Chi { .. }) {
                    "Å⁻¹"
                } else {
                    "eV"
                },
                crate::plotting::analysis::analysis_axis_labels(space).1
            )
        })
        .unwrap_or_default();
        div()
            .flex()
            .flex_col()
            .child(row)
            .child(
                div()
                    .px_3()
                    .text_size(px(11.))
                    .text_color(t.text_muted)
                    .child(saved_range),
            )
            .child(
                div()
                    .px_3()
                    .pb_1()
                    .text_size(px(11.))
                    .text_color(t.text_muted)
                    .child(caption),
            )
    }

    /// Which quadrant plots the current stage shows, top to bottom.
    pub(crate) fn stage_plots(&self) -> Vec<(usize, SharedString)> {
        stage_plot_selection(
            self.stage,
            self.stage_view,
            ProcessLayout {
                enlarged: self.maximized,
                xanes_only: self.xanes_only(),
            },
            self.fft_summary().3,
            self.mixed_overlay_weight.is_some(),
            self.processing_plot_quantity(),
        )
    }

    /// Refitting a prepared input uses the ordinary input/norm/flat plot choices;
    /// its original quantity remains on the group and provenance record.
    pub(crate) fn processing_plot_quantity(&self) -> crate::params::Quantity {
        if self.spectrum_quantity.prepared_space().is_some() && self.ui_params().refit_prepared {
            crate::params::Quantity::RawMu
        } else {
            self.spectrum_quantity
        }
    }

    /// Process main view: the four linked plots, the χ(q) back-transform, or
    /// the wavelet transform of χ(k).
    fn process_view_selector(&self, views: ProcessViews, cx: &mut Context<Self>) -> gpui::Div {
        let t = self.theme;
        let q = self.maximized == Some(PLOT_CHIQ);
        segmented(&t)
            .flex_none()
            .child(
                segment(
                    &t,
                    ("process-view", 0_usize),
                    "Linked plots",
                    !self.wavelet.open && !q,
                    true,
                )
                .on_click(cx.listener(|app, _, _, cx| {
                    show_process_overlay(
                        &mut app.wavelet,
                        &mut app.normalization,
                        ProcessOverlay::Plots,
                    );
                    app.maximized = None;
                    app.stage_view_changed(cx);
                })),
            )
            .when(views.q, |d| {
                d.child(
                    segment(
                        &t,
                        ("process-view", 1_usize),
                        "χ(q)",
                        !self.wavelet.open && q,
                        false,
                    )
                    .on_click(cx.listener(|app, _, _, cx| {
                        show_process_overlay(
                            &mut app.wavelet,
                            &mut app.normalization,
                            ProcessOverlay::Plots,
                        );
                        app.ui.sections.insert("back-transform");
                        app.maximized = Some(PLOT_CHIQ);
                        app.stage_view_changed(cx);
                    })),
                )
            })
            .child(
                segment(
                    &t,
                    ("process-view", 2_usize),
                    "Wavelet",
                    self.wavelet.open,
                    false,
                )
                .on_click(cx.listener(|app, _, _, cx| app.open_wavelet(cx))),
            )
    }

    /// Enlarge one linked plot, or restore the grid when it is already
    /// enlarged. ESC also restores the grid.
    fn toggle_enlarged(&mut self, index: usize, cx: &mut Context<Self>) {
        self.maximized = if self.maximized.is_some() {
            None
        } else {
            Some(index)
        };
        self.stage_view_changed(cx);
    }

    /// Compare toggle, its overlay options and the colour menu: one cluster on
    /// every stage's plot bar.
    fn compare_cluster(&self, cx: &mut Context<Self>) -> gpui::Div {
        use super::controls::{Menu, icon_button};
        use crate::icons::Icon;
        let t = self.theme;
        let v = self.stage_view;
        let mut cluster = div().flex().flex_wrap().items_center().gap_1().child(
            icon_button(
                &t,
                "plot-compare",
                Icon::Layers,
                format!(
                    "Compare current + {} marked · {}",
                    self.selection.len(),
                    crate::text::plural(self.compare_count(), "spectrum")
                ),
                v.scope == PlotScope::Marked,
            )
            .w_auto()
            .px_2()
            .gap_1()
            .child(format!("Compare {}", self.compare_count()))
            .on_click(cx.listener(|app, _, _, cx| {
                app.stage_view.scope = if app.stage_view.scope == PlotScope::Current {
                    PlotScope::Marked
                } else {
                    PlotScope::Current
                };
                app.stage_view_changed(cx);
            })),
        );
        if v.scope == PlotScope::Marked {
            cluster = cluster
                .child(
                    chip(&t, "plot-preview", "Preview 12", self.view.sample_overlay).on_click(
                        cx.listener(|app, _, _, cx| {
                            app.view.sample_overlay = true;
                            app.ensure_compare_loaded(cx);
                            app.stage_view_changed(cx);
                        }),
                    ),
                )
                .child(
                    chip(
                        &t,
                        "plot-all",
                        format!("Plot all {}", self.compare_count()),
                        !self.view.sample_overlay,
                    )
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.view.sample_overlay = false;
                        app.ensure_compare_loaded(cx);
                        app.stage_view_changed(cx);
                    })),
                )
                .child(
                    chip(&t, "plot-gradient", "Gradient", self.view.gradient).on_click(
                        cx.listener(|app, _, _, cx| {
                            app.view.gradient = !app.view.gradient;
                            app.stage_view_changed(cx);
                        }),
                    ),
                );
        }
        cluster.child(
            chip(
                &t,
                "spectrum-colors",
                "Colors",
                self.ui.menu == Some(Menu::Colors),
            )
            .on_click(cx.listener(|app, event, window, cx| {
                app.open_chrome_menu(Menu::Colors, event, window, cx);
            })),
        )
    }

    /// The plot bar in three clusters separated by rules: the view segments,
    /// the overlay toggles (and range presets), and the comparison controls.
    fn plot_bar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        use super::controls::{Menu, icon_button};
        use crate::icons::Icon;
        let t = self.theme;
        let v = self.stage_view;
        let mut bar = div()
            .min_h(px(36.))
            .min_w_0()
            .w_full()
            .flex_none()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_1()
            .px_2()
            .py_1()
            .bg(t.surface)
            .border_b_1()
            .border_color(t.border);
        if self.stage == Stage::Data
            && self.tools.open == Some(super::tools::Tool::Align)
            && self.tool_preview_current(cx)
        {
            return bar
                .child(div().text_color(t.text).child("Alignment · dμ/dE"))
                .child(div().text_color(t.text_muted).child("XANES"));
        }
        let process = self.stage == Stage::Process;
        // The view selector precedes the quantity guard so that a χ(k)-only
        // group keeps its way into (and out of) the wavelet view.
        let views = process_views(self.spectrum_quantity);
        if process && views.wavelet {
            bar = bar.child(self.process_view_selector(views, cx));
            if self.wavelet.open {
                return bar
                    .child(bar_divider(&t))
                    .child(self.wavelet_view_selector(cx))
                    .child(div().flex_1())
                    .child(self.wavelet_toolbar_actions(cx));
            }
            bar = bar.child(bar_divider(&t));
        }
        if self.stage.is_processing()
            && (!self.spectrum_quantity.supports_exafs()
                || (self.stage == Stage::Data
                    && self.processing_plot_quantity().prepared_space().is_some()))
        {
            return bar
                .child(self.spectrum_quantity.label())
                .child(bar_divider(&t))
                .child(self.compare_cluster(cx));
        }
        // Data: which μ(E) quantity the single plot shows. Process: whether
        // the second linked plot is normalized or flattened μ(E), unless the
        // stored quantity fixes it (a prepared μnorm or flattened group).
        let fixed = fixed_norm_view(self.processing_plot_quantity());
        let energy_views: &[(EQuantity, &str)] = if process && fixed.is_some() {
            &[]
        } else if process {
            &[(EQuantity::Norm, "norm"), (EQuantity::Flat, "flat")]
        } else {
            &[
                (EQuantity::Mu, "μ(E)"),
                (EQuantity::Norm, "norm"),
                (EQuantity::Flat, "flat"),
            ]
        };
        let mut choices = segmented(&t).flex_none();
        for (index, &(quantity, label)) in energy_views.iter().enumerate() {
            let on = if process && quantity == EQuantity::Norm {
                v.e_quantity != EQuantity::Flat
            } else {
                v.e_quantity == quantity
            };
            choices = choices.child(
                segment(&t, ("energy-view", index), label, on, index == 0).on_click(cx.listener(
                    move |app, _, _, cx| {
                        app.stage_view.e_quantity = quantity;
                        app.stage_view_changed(cx);
                    },
                )),
            );
        }
        if !energy_views.is_empty() {
            bar = bar.child(choices).child(bar_divider(&t));
        }
        bar = bar
            .child(
                chip(&t, "common-e0", "E₀", self.view.show_e0).on_click(cx.listener(
                    |a, _, _, c| {
                        a.view.show_e0 = !a.view.show_e0;
                        a.stage_view_changed(c);
                    },
                )),
            )
            .child(
                chip(&t, "common-derivative", "dμ/dE", self.view.show_deriv).on_click(cx.listener(
                    |a, _, _, c| {
                        a.view.show_deriv = !a.view.show_deriv;
                        a.stage_view_changed(c);
                    },
                )),
            );
        if process {
            // Fitted curves on μ(E): the normalization fit and the AUTOBK
            // spline. Both are in input μ units, like the μ(E) plot.
            bar = bar
                .child(
                    chip(
                        &t,
                        "common-pre-post",
                        if self.ui_params().mback.is_some() {
                            "MBACK fit"
                        } else {
                            "Pre/post"
                        },
                        self.view.show_pre && self.view.show_post,
                    )
                    .w(px(80.))
                    .justify_center()
                    .on_click(cx.listener(|a, _, _, c| {
                        let on = !(a.view.show_pre && a.view.show_post);
                        a.view.show_pre = on;
                        a.view.show_post = on;
                        a.stage_view_changed(c);
                    })),
                )
                .child(
                    chip(&t, "common-spline", "Spline", v.show_bkg).on_click(cx.listener(
                        |a, _, _, c| {
                            a.stage_view.show_bkg = !a.stage_view.show_bkg;
                            a.stage_view_changed(c);
                        },
                    )),
                )
                .child(bar_divider(&t))
                .child(self.kweight_buttons(cx))
                .child(
                    chip(&t, "common-real", "Re", v.show_re).on_click(cx.listener(|a, _, _, c| {
                        a.stage_view.show_re = !a.stage_view.show_re;
                        a.stage_view_changed(c);
                    })),
                )
                .child(
                    chip(&t, "common-window", "Window", self.view.show_kwin).on_click(cx.listener(
                        |a, _, _, c| {
                            a.view.show_kwin = !a.view.show_kwin;
                            a.invalidate_explore_plots(c);
                            c.notify();
                        },
                    )),
                );
        }
        // Energy-axis zoom presets for the μ(E) plots.
        bar = bar.child(bar_divider(&t));
        for (i, label, range) in [
            (0, "XANES", Some((-20., 80.))),
            (1, "−200…+800", Some((-200., 800.))),
            (2, "Full spectrum", None),
        ] {
            bar = bar.child(
                chip(
                    &t,
                    SharedString::from(format!("view-energy-{i}")),
                    label,
                    self.view.energy_view_range == range,
                )
                .on_click(cx.listener(move |app, _, _, cx| {
                    app.view.energy_view_range = range;
                    app.analysis.shown = None;
                    app.analysis.plot = None;
                    app.quadrants.clear();
                    app.quad_bindings.clear();
                    app.stage_view_changed(cx);
                })),
            );
        }
        bar.child(bar_divider(&t))
            .child(self.compare_cluster(cx))
            .child(div().flex_1())
            .child(self.plot_ranges_button(cx))
            .when(!process, |d| {
                d.child(
                    icon_button(
                        &t,
                        "plot-overview",
                        Icon::Grid,
                        "Overview plots",
                        self.ui.overview,
                    )
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.ui.overview = !app.ui.overview;
                        cx.notify();
                    })),
                )
            })
            .child(
                icon_button(
                    &t,
                    "plot-options",
                    Icon::Sliders,
                    "Plot options",
                    self.ui.menu == Some(Menu::Plot),
                )
                .on_click(cx.listener(|app, event, window, cx| {
                    app.open_chrome_menu(Menu::Plot, event, window, cx)
                })),
            )
    }

    pub(crate) fn plot_options(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let panel = div().flex().flex_col().gap_1();
        panel
            .child(self.plot_option(
                "view-offset",
                "Stack curves",
                self.view.layout == crate::plotting::TraceLayout::Waterfall,
                |a, c| {
                    a.view.layout = if a.view.layout == crate::plotting::TraceLayout::Waterfall {
                        crate::plotting::TraceLayout::Overlay
                    } else {
                        crate::plotting::TraceLayout::Waterfall
                    };
                    a.invalidate_explore_plots(c);
                    c.notify();
                },
                cx,
            ))
            .child(self.plot_option(
                "view-legend",
                "Legend",
                self.view.legend,
                |a, c| {
                    a.view.legend = !a.view.legend;
                    a.invalidate_explore_plots(c);
                    c.notify();
                },
                cx,
            ))
            .child(self.plot_option(
                "view-grid",
                "Grid",
                self.view.grid,
                |a, c| {
                    a.view.grid = !a.view.grid;
                    a.invalidate_explore_plots(c);
                    c.notify();
                },
                cx,
            ))
            .child(self.plot_option(
                "view-overview",
                "Overview plots",
                self.ui.overview,
                |a, c| {
                    a.ui.overview = !a.ui.overview;
                    c.notify();
                },
                cx,
            ))
    }

    /// k-weight 0/1/2/3 (plotting + forward FT; AUTOBK's k-weight is its own).
    fn kweight_buttons(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let current = self.fft_summary().3.round() as i32;
        let mut row = div().flex().items_center().gap_1().child(
            div()
                .text_size(px(11.))
                .text_color(t.text_muted)
                .child("k-weight"),
        );
        for kw in 0..=3 {
            let on = kw == current;
            row = row.child(
                crate::accessibility::Control::new(
                    div().id(SharedString::from(format!("kw-{kw}"))),
                    format!("k-weight {kw}"),
                    accesskit::Role::Tab,
                )
                .selected(on)
                .w(px(24.))
                .h(px(22.))
                .flex()
                .items_center()
                .justify_center()
                .rounded_md()
                .border_1()
                .font_family(MONO)
                .text_size(px(11.5))
                .cursor_pointer()
                .when(on, |d| {
                    d.bg(gpui::Rgba {
                        a: 0.16,
                        ..t.accent
                    })
                    .border_color(t.accent)
                    .text_color(t.text)
                })
                .when(!on, |d| {
                    d.border_color(t.border)
                        .text_color(t.text_muted)
                        .hover(|d| d.bg(t.raised))
                })
                .on_click(cx.listener(move |this, _: &ClickEvent, _w, cx| {
                    this.apply_param(ParamKey::FftKweight, Some(kw as f64), cx);
                    this.sync_param_fields(cx);
                }))
                .child(kw.to_string()),
            );
        }
        row
    }

    /// One main plot with its title, a reset-view button, the handle overlay,
    /// and a hover hint.
    fn plot_card(
        &mut self,
        index: usize,
        title: SharedString,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        use super::controls::icon_button;
        use crate::icons::Icon;
        let t = self.theme;
        let plot = self.quadrants[index].1.clone();
        let label = self.current_group_label();
        let overlay = self.handle_overlay(index, cx);
        let enlargeable = self.stage == Stage::Process;
        let enlarged = self.maximized.is_some();
        div()
            .id(SharedString::from(format!("plot-card-{index}")))
            .flex_1()
            .min_h_0()
            .min_w_0()
            .relative()
            .flex()
            .flex_col()
            .rounded_lg()
            .bg(t.raised)
            .border_1()
            .border_color(t.border)
            .on_mouse_move(cx.listener(move |this, ev: &gpui::MouseMoveEvent, _w, cx| {
                this.plot_pointer_move(index, ev.position, cx);
            }))
            .capture_any_mouse_down(cx.listener(move |this, ev: &gpui::MouseDownEvent, _, cx| {
                this.capture_handle_press(index, ev, cx);
            }))
            .child(
                div()
                    .flex_none()
                    .px_3()
                    .pt_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_size(px(11.5))
                    .child(
                        // Title area (not the buttons): double-click enlarges
                        // or restores a linked plot.
                        div()
                            .id(SharedString::from(format!("plot-title-{index}")))
                            .min_w_0()
                            .flex_1()
                            .flex()
                            .items_center()
                            .gap_2()
                            .when(enlargeable, |d| {
                                d.on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                                    if event.click_count() == 2 {
                                        this.toggle_enlarged(index, cx);
                                    }
                                }))
                            })
                            .child(div().font_weight(gpui::FontWeight::MEDIUM).child(title))
                            .child(
                                div()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .text_color(t.text_muted)
                                    .child(label),
                            ),
                    )
                    .child(
                        icon_button(
                            &t,
                            SharedString::from(format!("plot-reset-{index}")),
                            Icon::Undo,
                            "Reset view (undo zoom and pan)",
                            false,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some((_, plot)) = this.quadrants.get(index) {
                                plot.update(cx, |plot, cx| plot.reset_view(cx));
                            }
                        })),
                    )
                    .child(self.plot_export_button(
                        format!("plot-export-{index}"),
                        super::plot_export::Target::Quadrant(index),
                        cx,
                    ))
                    .when(enlargeable, |d| {
                        d.child(
                            icon_button(
                                &t,
                                SharedString::from(format!("plot-enlarge-{index}")),
                                if enlarged {
                                    Icon::Minimize
                                } else {
                                    Icon::Maximize
                                },
                                if enlarged {
                                    "Show the linked plots · Esc or double-click the title"
                                } else {
                                    "Enlarge this plot · or double-click the title"
                                },
                                enlarged,
                            )
                            .on_click(
                                cx.listener(move |this, _, _, cx| this.toggle_enlarged(index, cx)),
                            ),
                        )
                    }),
            )
            .when(
                self.stage_view.scope == PlotScope::Marked && self.stage != Stage::Fit,
                |d| {
                    d.children(
                        self.plot_coverage
                            .get(index)
                            .and_then(|coverage| coverage.disclosure())
                            .map(|text| {
                                div()
                                    .px_3()
                                    .pt_1()
                                    .text_size(px(11.5))
                                    .text_color(t.warn)
                                    .child(text)
                            }),
                    )
                },
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .p_1()
                    .relative()
                    .child(plot)
                    .child(self.measure_card(index, cx))
                    .children(self.handle_layer(index, cx)),
            )
            .children(overlay)
    }

    pub(crate) fn current_group_label(&self) -> SharedString {
        current_label(self.selected, |ix| self.entry_label(ix))
    }

    /// Clicking a thumbnail opens exactly its current-data quantity.
    fn thumbnail_strip(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let kw = self.fft_summary().3;
        // Each thumbnail opens Process with its plot enlarged.
        let captions: [(ProcessSection, usize, String); 4] = [
            (ProcessSection::Normalize, PLOT_NORM, "norm μ(E)".into()),
            (
                ProcessSection::Background,
                PLOT_CHIK,
                crate::plotting::chik_label(kw),
            ),
            (
                ProcessSection::Transform,
                PLOT_CHIK,
                format!("{} · window", crate::plotting::chik_label(kw)),
            ),
            (ProcessSection::Transform, PLOT_CHIR, "|χ(R)|".into()),
        ];
        let mut strip = div()
            .h(px(118.))
            .flex_none()
            .flex()
            .gap_2()
            .px_3()
            .pt_2()
            .pb_3();
        for (i, (section, plot, caption)) in captions.into_iter().enumerate() {
            let (status, _) = self.process_section_summary(section);
            let dot = if status == StageStatus::Idle {
                t.text_muted
            } else {
                status.color(&t)
            };
            let data = self.thumbs.clone();
            strip = strip.child(
                div()
                    .id(SharedString::from(format!("thumb-{i}")))
                    .flex_1()
                    .min_w_0()
                    .relative()
                    .rounded_md()
                    .bg(t.raised)
                    .border_1()
                    .border_color(t.border)
                    .cursor_pointer()
                    .hover(|d| d.border_color(t.accent))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _w, cx| {
                        this.set_stage(Stage::Process, cx);
                        this.stage_view.scope = PlotScope::Current;
                        this.stage_view.e_quantity = EQuantity::Norm;
                        this.stage_view.show_re = false;
                        this.view.show_deriv = false;
                        this.view.show_kwin = i == 2;
                        this.maximized = Some(plot);
                        this.stage_view_changed(cx);
                    }))
                    .child(
                        div()
                            .absolute()
                            .top_1()
                            .left_2()
                            .flex()
                            .items_center()
                            .gap_1p5()
                            .text_size(px(11.))
                            .text_color(t.text_muted)
                            .child(div().w(px(6.)).h(px(6.)).rounded_full().bg(dot))
                            .child(caption),
                    )
                    .children(data.map(|d| {
                        div()
                            .size_full()
                            .pt_4()
                            .child(super::thumbnails::sparkline(d, i, t))
                    })),
            );
        }
        strip
    }
}

/// Auxiliary views that replace the Process linked plots. At most one is
/// shown; activating one closes the other (see [`show_process_overlay`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ProcessOverlay {
    Plots,
    Wavelet,
    /// Normalization comparison or MBACK atomic match.
    Normalization,
}

/// The overlay to draw. The normalization views are opened from the
/// Normalize section without going through the view selector, so they win
/// over a wavelet view left open underneath; closing them returns to it.
pub(crate) fn active_process_overlay(
    wavelet_open: bool,
    normalization_open: bool,
) -> ProcessOverlay {
    if normalization_open {
        ProcessOverlay::Normalization
    } else if wavelet_open {
        ProcessOverlay::Wavelet
    } else {
        ProcessOverlay::Plots
    }
}

/// Keep only `overlay` open (its own flag is set by its opener).
pub(crate) fn show_process_overlay(
    wavelet: &mut super::wavelet::WaveletState,
    normalization: &mut super::normalization::NormalizationState,
    overlay: ProcessOverlay,
) {
    if overlay != ProcessOverlay::Wavelet {
        wavelet.hide();
    }
    if overlay != ProcessOverlay::Normalization {
        normalization.open = false;
    }
}

/// Which Process views a stored quantity supports.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct ProcessViews {
    /// χ(q) back-transform of a processed absorption spectrum.
    pub q: bool,
    /// Wavelet transform: absorption spectra and confirmed χ(k) input.
    pub wavelet: bool,
}

pub(crate) fn process_views(quantity: crate::params::Quantity) -> ProcessViews {
    let exafs = quantity.supports_exafs();
    ProcessViews {
        q: exafs,
        wavelet: exafs || quantity == crate::params::Quantity::ChiK,
    }
}

/// The normalized-plot representation fixed by a prepared quantity:
/// `quantity_quadrant_specs` always draws stored μnorm as normalized and
/// stored flattened μ(E) as flattened, whatever the norm/flat choice.
pub(crate) fn fixed_norm_view(quantity: crate::params::Quantity) -> Option<EQuantity> {
    match quantity {
        crate::params::Quantity::NormalizedMu => Some(EQuantity::Norm),
        crate::params::Quantity::FlattenedMu => Some(EQuantity::Flat),
        _ => None,
    }
}

/// Process grid state that changes which linked plots are shown.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ProcessLayout {
    /// Plot slot shown alone (`PLOT_CHIQ` for the χ(q) view); `None` shows
    /// the grid.
    pub enlarged: Option<usize>,
    /// XANES-only correction: normalization plots only.
    pub xanes_only: bool,
}

/// Shared main-view selection for freshly created and reopened groups.
pub(crate) fn stage_plot_selection(
    stage: Stage,
    v: super::StageView,
    layout: ProcessLayout,
    kw: f64,
    mixed_weights: bool,
    quantity: crate::params::Quantity,
) -> Vec<(usize, SharedString)> {
    if stage.is_processing()
        && (!quantity.supports_exafs()
            || (stage == Stage::Data && quantity.prepared_space().is_some()))
    {
        return vec![if quantity == crate::params::Quantity::ChiK {
            (PLOT_CHIK, crate::plotting::chik_label(kw).into())
        } else if matches!(
            quantity,
            crate::params::Quantity::NormalizedMu
                | crate::params::Quantity::FlattenedMu
                | crate::params::Quantity::FlattenedDifference
        ) {
            (PLOT_NORM, quantity.label().into())
        } else {
            (PLOT_MU, quantity.label().into())
        }];
    }
    let chik: SharedString = crate::plotting::chik_label(kw).into();
    let chir: SharedString = if mixed_weights {
        "|χ(R)| · mixed k weights".into()
    } else {
        crate::plotting::chir_label(kw).into()
    };
    match stage {
        Stage::Data => vec![match v.e_quantity {
            EQuantity::Mu => (PLOT_MU, "μ(E)".into()),
            EQuantity::Norm => (PLOT_NORM, "normalized μ(E)".into()),
            EQuantity::Flat => (PLOT_NORM, "flattened μ(E)".into()),
        }],
        Stage::Process => {
            let mu: SharedString = if quantity.prepared_space().is_some() {
                quantity.label().into()
            } else {
                "μ(E)".into()
            };
            let flat = fixed_norm_view(quantity).unwrap_or(v.e_quantity) == EQuantity::Flat;
            let norm: SharedString = if flat {
                "flattened μ(E)".into()
            } else {
                "normalized μ(E)".into()
            };
            let mut plots = vec![(PLOT_MU, mu), (PLOT_NORM, norm)];
            if !layout.xanes_only {
                plots.extend([(PLOT_CHIK, chik), (PLOT_CHIR, chir)]);
            }
            match layout.enlarged {
                Some(PLOT_CHIQ) if !layout.xanes_only => {
                    vec![(PLOT_CHIQ, "χ(q) back-transform".into())]
                }
                Some(index) if plots.iter().any(|(i, _)| *i == index) => {
                    plots.retain(|(i, _)| *i == index);
                    plots
                }
                _ => plots,
            }
        }
        Stage::Fit | Stage::Series | Stage::Publish => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn current_header_resolves_standalone_and_catalog_display_labels() {
        use crate::app::NO_ENTRY;
        use crate::group_identity::{GroupId, GroupState};
        let standalone = GroupId::new_result();
        let catalog = GroupId::new_result();
        let mut state = GroupState::default();
        state.labels.insert(standalone.clone(), "Cu foil".into());
        state.labels.insert(catalog.clone(), "Cu standard".into());
        for (selected, expected) in [
            (None, "Cu foil"),
            (Some(NO_ENTRY), "Cu foil"),
            (Some(0), "Cu standard"),
        ] {
            let label = super::current_label(selected, |ix| {
                let id = if ix == NO_ENTRY {
                    &standalone
                } else {
                    &catalog
                };
                state.display_label(Some(id), || "load-time label".into())
            });
            assert_eq!(label.as_ref(), expected);
        }
    }

    use super::*;
    use crate::app::shell::StageView;

    #[test]
    fn difference_default_create_reopen_plot_view_uses_quantity_axes() {
        use crate::project::{DataStorage, ProjectFile, load, save_with_storage};
        use crate::{
            params::{DerivedSpectrum, Quantity},
            plotting::{QuadTrace, ViewOptions, quantity_quadrant_specs},
            theme::Theme,
        };
        let temp =
            std::env::temp_dir().join(format!("rexafs-difference-view-{}", std::process::id()));
        std::fs::create_dir_all(&temp).unwrap();
        let mut project = ProjectFile::default();
        let difference = DerivedSpectrum {
            id: 99,
            label: "renamed output".into(),
            quantity: Quantity::NormalizedDifference,
            energy: vec![100., 101., 102.],
            mu: vec![-0.2, 0., 0.3],
            ..Default::default()
        };
        project.derived = vec![difference.clone()];
        project.active_derived = Some(99);
        let assert_default_view = |group: &DerivedSpectrum| {
            let spectrum = std::sync::Arc::new(group.for_display(&project.params).unwrap());
            assert!(spectrum.norm().is_none() && spectrum.flat().is_none());
            let specs = quantity_quadrant_specs(
                &[QuadTrace {
                    color_index: 0,
                    color: None,
                    label: group.display_label(),
                    sp: spectrum,
                    active: true,
                }],
                &ViewOptions::default(),
                &Theme::dark(),
                false,
                group.quantity,
            );
            for stage in [Stage::Data, Stage::Process] {
                let view = StageView::default();
                let layout = ProcessLayout::default();
                let selected = stage_plot_selection(stage, view, layout, 2., false, group.quantity);
                assert_eq!(selected.len(), 1);
                assert_eq!(selected[0].1.as_ref(), "Δμnorm");
                let plot = &specs[selected[0].0];
                assert_eq!(plot.xlabel, "Energy (eV)");
                assert_eq!(plot.ylabel, "Δμnorm (dimensionless)");
                assert_eq!(plot.series[0].x, group.energy);
                assert_eq!(plot.series[0].y, group.mu);
            }
        };
        assert_default_view(&difference);
        for storage in [DataStorage::Paths, DataStorage::Embedded] {
            let path = temp.join(format!("difference-view-{storage:?}.rxs"));
            save_with_storage(&path, &project, storage).unwrap();
            let reopened = load(&path).unwrap();
            assert_eq!(reopened.active_derived, Some(99));
            assert_default_view(&reopened.derived[0]);
        }
        std::fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    fn process_shows_four_linked_plots_and_enlarges_one() {
        use crate::params::Quantity;
        let view = StageView::default();
        let select = |layout| {
            stage_plot_selection(Stage::Process, view, layout, 2., false, Quantity::RawMu)
                .into_iter()
                .map(|(i, _)| i)
                .collect::<Vec<_>>()
        };
        let grid = ProcessLayout::default();
        assert_eq!(select(grid), [PLOT_MU, PLOT_NORM, PLOT_CHIK, PLOT_CHIR]);
        for plot in [PLOT_MU, PLOT_NORM, PLOT_CHIK, PLOT_CHIR, PLOT_CHIQ] {
            let enlarged = ProcessLayout {
                enlarged: Some(plot),
                ..grid
            };
            assert_eq!(select(enlarged), [plot]);
        }
        // A XANES-only correction keeps normalization and withholds χ.
        let xanes = ProcessLayout {
            xanes_only: true,
            ..grid
        };
        assert_eq!(select(xanes), [PLOT_MU, PLOT_NORM]);
        for plot in [PLOT_CHIK, PLOT_CHIQ] {
            let hidden = ProcessLayout {
                enlarged: Some(plot),
                xanes_only: true,
            };
            assert_eq!(select(hidden), [PLOT_MU, PLOT_NORM]);
        }
        // The flat/norm choice titles the second plot; Data keeps one plot.
        let flat = StageView {
            e_quantity: EQuantity::Flat,
            ..view
        };
        let titles = stage_plot_selection(Stage::Process, flat, grid, 2., false, Quantity::RawMu);
        assert_eq!(titles[1].1.as_ref(), "flattened μ(E)");
        let data = stage_plot_selection(Stage::Data, flat, grid, 2., false, Quantity::RawMu);
        assert_eq!(data.len(), 1);
    }

    #[test]
    fn process_views_follow_the_stored_quantity() {
        use crate::params::Quantity;
        for quantity in [
            Quantity::RawMu,
            Quantity::NormalizedMu,
            Quantity::FlattenedMu,
        ] {
            assert_eq!(
                process_views(quantity),
                ProcessViews {
                    q: true,
                    wavelet: true
                }
            );
        }
        // Confirmed χ(k) keeps its wavelet entry; differences have neither.
        assert_eq!(
            process_views(Quantity::ChiK),
            ProcessViews {
                q: false,
                wavelet: true
            }
        );
        for quantity in [
            Quantity::NormalizedDifference,
            Quantity::FlattenedDifference,
        ] {
            assert!(!process_views(quantity).wavelet && !process_views(quantity).q);
        }
    }

    #[test]
    fn opening_one_process_overlay_closes_the_other() {
        use super::super::{normalization::NormalizationState, wavelet::WaveletState};
        let open = || {
            let mut wavelet = WaveletState::default();
            let mut normalization = NormalizationState::default();
            wavelet.open = true;
            normalization.open = true;
            (wavelet, normalization)
        };
        let (wavelet, normalization) = open();
        // The comparison opened from the Normalize section is shown on top.
        assert_eq!(
            active_process_overlay(wavelet.open, normalization.open),
            ProcessOverlay::Normalization
        );
        let (mut wavelet, mut normalization) = open();
        show_process_overlay(&mut wavelet, &mut normalization, ProcessOverlay::Wavelet);
        assert!(wavelet.open && !normalization.open);
        assert_eq!(
            active_process_overlay(wavelet.open, normalization.open),
            ProcessOverlay::Wavelet
        );
        // Linked plots, χ(q) and assistant navigation close both.
        let (mut wavelet, mut normalization) = open();
        show_process_overlay(&mut wavelet, &mut normalization, ProcessOverlay::Plots);
        assert!(!wavelet.open && !normalization.open);
        assert_eq!(
            active_process_overlay(wavelet.open, normalization.open),
            ProcessOverlay::Plots
        );
    }

    #[test]
    fn normalized_plot_title_matches_a_fixed_prepared_quantity() {
        use crate::params::Quantity;
        let grid = ProcessLayout::default();
        for (quantity, choice, title) in [
            (Quantity::NormalizedMu, EQuantity::Flat, "normalized μ(E)"),
            (Quantity::FlattenedMu, EQuantity::Norm, "flattened μ(E)"),
            (Quantity::RawMu, EQuantity::Flat, "flattened μ(E)"),
            (Quantity::RawMu, EQuantity::Norm, "normalized μ(E)"),
        ] {
            let view = StageView {
                e_quantity: choice,
                ..StageView::default()
            };
            let plots = stage_plot_selection(Stage::Process, view, grid, 2., false, quantity);
            assert_eq!(plots[1], (PLOT_NORM, title.into()), "{quantity:?}");
        }
        assert_eq!(fixed_norm_view(Quantity::RawMu), None);
    }
}
