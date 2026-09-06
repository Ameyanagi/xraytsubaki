//! Processing tools (Athena's Data menu) as non-destructive inline forms:
//! every tool reads the current group and produces a *derived* group, so
//! the source is never mutated and nothing needs undo.

use gpui::{
    ClickEvent, Context, Entity, IntoElement, ParentElement, SharedString, Styled, div, prelude::*,
    px,
};
use rexafs::prelude::XASSpectrum;
use rexafs::xafs::tools::{EdgeFeature, RebinConfig};
use rexafs::xafs::xafsutils::ConvolveForm;

use rexafs::prelude::{AnalysisSpace, LcfConfig, PcaConfig};

use super::button;
use crate::app::{DERIVED_BASE, StudioApp};
use crate::params::DerivedSpectrum;
use crate::widgets::numeric_field::{FieldEvent, FieldKind, NumericField};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tool {
    Align,
    Calibrate,
    Deglitch,
    Truncate,
    Rebin,
    Smooth,
    Difference,
    Lcf,
    Pca,
}

impl Tool {
    /// Processing tools (each Apply creates a derived group).
    pub const PROCESSING: [Tool; 7] = [
        Tool::Align,
        Tool::Calibrate,
        Tool::Deglitch,
        Tool::Truncate,
        Tool::Rebin,
        Tool::Smooth,
        Tool::Difference,
    ];

    /// Analysis tools (results, not new groups).
    pub const ANALYSIS: [Tool; 2] = [Tool::Lcf, Tool::Pca];

    pub fn is_analysis(self) -> bool {
        matches!(self, Tool::Lcf | Tool::Pca)
    }

    pub fn name(self) -> &'static str {
        match self {
            Tool::Align => "Align to reference",
            Tool::Calibrate => "Calibrate energy",
            Tool::Deglitch => "Deglitch",
            Tool::Truncate => "Truncate",
            Tool::Rebin => "Rebin",
            Tool::Smooth => "Smooth",
            Tool::Difference => "Difference spectrum",
            Tool::Lcf => "Linear combination fit",
            Tool::Pca => "Principal components",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            Tool::Align => "shift onto the first other marked group",
            Tool::Calibrate => "put the derivative maximum at a target eV",
            Tool::Deglitch => "remove the points inside an energy range",
            Tool::Truncate => "keep the points between two energies",
            Tool::Rebin => "Athena grid: 10 eV · 0.5 eV · 0.05 Å⁻¹",
            Tool::Smooth => "Gaussian convolution of μ(E)",
            Tool::Difference => "current − first other marked group",
            Tool::Lcf => "current as a mix of the marked standards",
            Tool::Pca => "components of the marked groups",
        }
    }

    fn fields(self) -> &'static [ToolField] {
        match self {
            Tool::Align => &[ToolField::WinLo, ToolField::WinHi],
            Tool::Calibrate => &[ToolField::Target],
            Tool::Deglitch => &[ToolField::ELo, ToolField::EHi],
            Tool::Truncate => &[ToolField::Before, ToolField::After],
            Tool::Rebin => &[ToolField::PreStep, ToolField::XanesStep, ToolField::KStep],
            Tool::Smooth => &[ToolField::Sigma],
            Tool::Difference => &[],
            Tool::Lcf => &[ToolField::RangeLo, ToolField::RangeHi],
            Tool::Pca => &[
                ToolField::RangeLo,
                ToolField::RangeHi,
                ToolField::Components,
            ],
        }
    }

    fn apply_label(self) -> &'static str {
        match self {
            Tool::Lcf => "Fit",
            Tool::Pca => "Train + target transform",
            _ => "Apply → new group",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ToolField {
    WinLo,
    WinHi,
    Target,
    ELo,
    EHi,
    Before,
    After,
    PreStep,
    XanesStep,
    KStep,
    Sigma,
    RangeLo,
    RangeHi,
    Components,
}

impl ToolField {
    const ALL: [ToolField; 14] = [
        ToolField::WinLo,
        ToolField::WinHi,
        ToolField::Target,
        ToolField::ELo,
        ToolField::EHi,
        ToolField::Before,
        ToolField::After,
        ToolField::PreStep,
        ToolField::XanesStep,
        ToolField::KStep,
        ToolField::Sigma,
        ToolField::RangeLo,
        ToolField::RangeHi,
        ToolField::Components,
    ];

    fn spec(self) -> (&'static str, &'static str, Option<f64>) {
        match self {
            ToolField::WinLo => ("window start (eV rel. E₀)", "-50", Some(-50.0)),
            ToolField::WinHi => ("window end (eV rel. E₀)", "100", Some(100.0)),
            ToolField::Target => ("target E₀ (eV)", "e.g. 22117", None),
            ToolField::ELo => ("from (eV)", "energy", None),
            ToolField::EHi => ("to (eV)", "energy", None),
            ToolField::Before => ("keep from (eV)", "auto (start)", None),
            ToolField::After => ("keep to (eV)", "auto (end)", None),
            ToolField::PreStep => ("pre-edge step (eV)", "10", Some(10.0)),
            ToolField::XanesStep => ("XANES step (eV)", "0.5", Some(0.5)),
            ToolField::KStep => ("EXAFS step (Å⁻¹)", "0.05", Some(0.05)),
            ToolField::Sigma => ("sigma (eV)", "1.0", Some(1.0)),
            ToolField::RangeLo => ("range start (rel. E₀)", "auto (−20)", None),
            ToolField::RangeHi => ("range end (rel. E₀)", "auto (+30)", None),
            ToolField::Components => ("components", "2", Some(2.0)),
        }
    }
}

/// Results of the analysis tools (LCF / PCA) for the current group.
#[derive(Default)]
pub struct AnalysisState {
    pub lcf: Option<rexafs::prelude::LcfResult>,
    /// Ranked combinations ("fit all combinations"), best first.
    pub ranked: Vec<rexafs::prelude::LcfResult>,
    pub pca: Option<rexafs::prelude::PcaModel>,
    pub pca_fit: Option<rexafs::prelude::PcaFit>,
    /// Which tool the center plot shows.
    pub shown: Option<Tool>,
    pub plot: Option<Entity<ruviz_gpui::RuvizPlot>>,
}

#[derive(Default)]
pub struct ToolState {
    pub open: Option<Tool>,
    pub fields: Vec<(ToolField, Entity<NumericField>)>,
    pub message: SharedString,
    /// LCF / PCA options (shared by the Data-stage tools and the Series
    /// LCF trend).
    pub lcf_space: LcfSpaceChoice,
    pub lcf_sum_to_one: bool,
    pub lcf_e0_shift: bool,
    pub lcf_all_combinations: bool,
    pub lcf_range: Option<(f64, f64)>,
    pub pca_components: usize,
}

/// Space an LCF / PCA runs in (the χ variant uses the plot k-weight).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum LcfSpaceChoice {
    #[default]
    Norm,
    Flat,
    Deriv,
    Chi,
}

impl LcfSpaceChoice {
    pub const ALL: [LcfSpaceChoice; 4] = [
        LcfSpaceChoice::Norm,
        LcfSpaceChoice::Flat,
        LcfSpaceChoice::Deriv,
        LcfSpaceChoice::Chi,
    ];

    pub fn label(self) -> &'static str {
        match self {
            LcfSpaceChoice::Norm => "norm",
            LcfSpaceChoice::Flat => "flat",
            LcfSpaceChoice::Deriv => "dμ/dE",
            LcfSpaceChoice::Chi => "χ(k)",
        }
    }

    pub fn space(self, kweight: f64) -> AnalysisSpace {
        match self {
            LcfSpaceChoice::Norm => AnalysisSpace::Norm,
            LcfSpaceChoice::Flat => AnalysisSpace::Flat,
            LcfSpaceChoice::Deriv => AnalysisSpace::Deriv,
            LcfSpaceChoice::Chi => AnalysisSpace::Chi { kweight },
        }
    }
}

impl ToolState {
    pub fn lcf_space_label(&self) -> &'static str {
        self.lcf_space.label()
    }

    pub fn lcf_config(&self) -> LcfConfig {
        LcfConfig {
            space: self.lcf_space.space(2.0),
            range: self.lcf_range,
            sum_to_one: self.lcf_sum_to_one,
            fit_e0_shift: self.lcf_e0_shift,
            ..LcfConfig::default()
        }
    }

    pub fn pca_config(&self) -> PcaConfig {
        PcaConfig {
            space: self.lcf_space.space(2.0),
            range: self.lcf_range,
            center: false,
        }
    }

    fn field_value(&self, field: ToolField, cx: &gpui::App) -> Option<f64> {
        self.fields
            .iter()
            .find(|(f, _)| *f == field)
            .and_then(|(_, e)| e.read(cx).value())
    }

    /// Pull the range fields into `lcf_range` (None = the space default).
    fn sync_range(&mut self, cx: &gpui::App) {
        let lo = self.field_value(ToolField::RangeLo, cx);
        let hi = self.field_value(ToolField::RangeHi, cx);
        self.lcf_range = match (lo, hi) {
            (Some(lo), Some(hi)) if hi > lo => Some((lo, hi)),
            _ => None,
        };
        if let Some(n) = self.field_value(ToolField::Components, cx) {
            self.pca_components = (n.round() as usize).max(1);
        }
    }
}

impl ToolState {
    pub fn new() -> Self {
        Self {
            lcf_sum_to_one: true,
            pca_components: 2,
            ..Default::default()
        }
    }
}

impl StudioApp {
    pub(crate) fn open_tool(&mut self, tool: Tool, cx: &mut Context<Self>) {
        if self.tools.fields.is_empty() {
            let theme = self.theme;
            self.tools.fields = ToolField::ALL
                .iter()
                .map(|&f| {
                    let (label, placeholder, default) = f.spec();
                    let field = cx.new(|cx| {
                        NumericField::new(label, placeholder, default, FieldKind::Float, theme, cx)
                    });
                    cx.subscribe(&field, |this: &mut Self, _f, event, cx| {
                        if let FieldEvent::Invalid(message) = event {
                            this.status = message.clone();
                            cx.notify();
                        }
                    })
                    .detach();
                    (f, field)
                })
                .collect();
        }
        self.tools.open = Some(tool);
        self.tools.message = SharedString::default();
        if tool.is_analysis() {
            self.analysis.shown = Some(tool);
            // The analysis section sits at the bottom of the Data inspector;
            // bring the form into view.
            self.inspector_scroll.scroll_to_bottom();
        }
        self.set_stage(super::Stage::Data, cx);
        cx.notify();
    }

    /// Standards / training set: every marked group other than the current
    /// one whose processed spectrum is cached.
    fn marked_spectra(&self) -> Vec<(String, std::sync::Arc<XASSpectrum>)> {
        let current = self.selected;
        self.lcf_standards()
            .into_iter()
            .filter(|_| true)
            .filter(|(label, _)| {
                Some(label.as_str()) != current.map(|ix| self.entry_label(ix)).as_deref()
            })
            .collect()
    }

    /// Run the LCF / PCA tool on the current group (synchronous: both are
    /// milliseconds) and show the result in the center.
    fn run_analysis_tool(&mut self, tool: Tool, cx: &mut Context<Self>) {
        let Some(unknown) = self.spectrum.clone() else {
            self.tools.message = "no current group".into();
            cx.notify();
            return;
        };
        self.tools.sync_range(cx);
        let standards = self.marked_spectra();
        let names: Vec<String> = standards.iter().map(|(n, _)| n.clone()).collect();
        let spectra: Vec<std::sync::Arc<XASSpectrum>> =
            standards.into_iter().map(|(_, sp)| sp).collect();
        let kw = self.fft_summary().3;
        let mut cfg = self.tools.lcf_config();
        cfg.space = self.tools.lcf_space.space(kw);
        let outcome: Result<String, String> = match tool {
            Tool::Lcf => {
                if spectra.len() < 2 {
                    Err("mark at least two standards (other than the current group); their spectra load when marked".into())
                } else if self.tools.lcf_all_combinations {
                    rexafs::prelude::lcf_combinatorial(&unknown, &spectra, &cfg, 4)
                        .map_err(|e| e.to_string())
                        .map(|mut ranked| {
                            for r in &mut ranked {
                                relabel(r, &names);
                            }
                            let best = ranked.first().cloned();
                            let n = ranked.len();
                            self.analysis.ranked = ranked;
                            self.analysis.lcf = best;
                            format!("{n} combinations ranked by R-factor")
                        })
                } else {
                    rexafs::prelude::lcf(&unknown, &spectra, &cfg)
                        .map_err(|e| e.to_string())
                        .map(|mut r| {
                            relabel(&mut r, &names);
                            let msg = format!("LCF R-factor {:.2e}", r.r_factor);
                            self.analysis.ranked.clear();
                            self.analysis.lcf = Some(r);
                            msg
                        })
                }
            }
            Tool::Pca => {
                if spectra.len() < 2 {
                    Err("mark at least two groups to train on".into())
                } else {
                    let mut pcfg = self.tools.pca_config();
                    pcfg.space = cfg.space;
                    rexafs::prelude::pca_train(&spectra, &pcfg)
                        .map_err(|e| e.to_string())
                        .and_then(|model| {
                            let n = self.tools.pca_components.min(model.n_components().max(1));
                            let fit = model
                                .target_transform(&unknown, n)
                                .map_err(|e| e.to_string())?;
                            let msg = format!(
                                "{} components explain {:.2} % · target R {:.2e}",
                                n,
                                model.cumulative_variance.get(n - 1).copied().unwrap_or(0.0)
                                    * 100.0,
                                fit.r_factor
                            );
                            self.analysis.pca = Some(model);
                            self.analysis.pca_fit = Some(fit);
                            Ok(msg)
                        })
                }
            }
            _ => Err("not an analysis tool".into()),
        };
        match outcome {
            Ok(msg) => {
                self.record(
                    format!("{} on {}: {msg}", tool.name(), self.current_group_label()),
                    None,
                );
                self.tools.message = msg.into();
                self.analysis.shown = Some(tool);
                self.rebuild_analysis_plot(cx);
                self.invalidate_explore_plots(cx);
            }
            Err(msg) => {
                self.tools.message = msg.into();
            }
        }
        cx.notify();
    }

    /// (Re)build the analysis plot entity from the current result.
    pub(crate) fn rebuild_analysis_plot(&mut self, cx: &mut Context<Self>) {
        let kw = self.fft_summary().3;
        let space = self.tools.lcf_space;
        let (xlabel, ylabel) = match space {
            LcfSpaceChoice::Chi => (
                crate::plotting::K_AXIS.to_string(),
                crate::plotting::chik_label(kw),
            ),
            LcfSpaceChoice::Deriv => ("Energy (eV)".to_string(), "dμ/dE".to_string()),
            LcfSpaceChoice::Norm => ("Energy (eV)".to_string(), "normalized μ(E)".to_string()),
            LcfSpaceChoice::Flat => ("Energy (eV)".to_string(), "flattened μ(E)".to_string()),
        };
        let plot = match self.analysis.shown {
            Some(Tool::Lcf) => self
                .analysis
                .lcf
                .as_ref()
                .map(|r| crate::plotting::build_lcf_plot(r, &xlabel, &ylabel, &self.theme)),
            Some(Tool::Pca) => self
                .analysis
                .pca_fit
                .as_ref()
                .map(|f| crate::plotting::build_pca_plot(f, &xlabel, &ylabel, &self.theme)),
            _ => None,
        };
        let Some(plot) = plot else {
            self.analysis.plot = None;
            return;
        };
        let plot = plot.size_px(820, 300);
        match &self.analysis.plot {
            Some(entity) => entity.update(cx, |rp, cx| rp.set_plot_keep_view(plot, cx)),
            None => {
                self.analysis.plot = Some(ruviz_gpui::plot_builder(plot).interactive().build(cx));
            }
        }
    }

    fn tool_value(&self, field: ToolField, cx: &Context<Self>) -> Option<f64> {
        self.tools
            .fields
            .iter()
            .find(|(f, _)| *f == field)
            .and_then(|(_, e)| e.read(cx).value())
    }

    /// The first marked group other than the current one, if its processed
    /// spectrum is cached (reference for align / difference).
    fn reference_spectrum(&self) -> Option<(String, std::sync::Arc<XASSpectrum>)> {
        let current = self.selected;
        for &ix in &self.selection {
            if Some(ix) == current || ix == crate::app::NO_ENTRY {
                continue;
            }
            let fp = self.effective_fingerprint(ix);
            if let Some(sp) = self.cache.peek(&(ix, fp)) {
                return Some((self.entry_label(ix), sp.clone()));
            }
        }
        None
    }

    pub(crate) fn apply_tool(&mut self, cx: &mut Context<Self>) {
        let Some(tool) = self.tools.open else {
            return;
        };
        if tool.is_analysis() {
            self.run_analysis_tool(tool, cx);
            return;
        }
        let Some(source) = self.spectrum.clone() else {
            self.tools.message = "no current group".into();
            cx.notify();
            return;
        };
        let name = self.current_group_label().to_string();
        let mut sp: XASSpectrum = (*source).clone();
        let result: Result<String, String> = (|| {
            let label = match tool {
                Tool::Align => {
                    let (ref_name, reference) = self
                        .reference_spectrum()
                        .ok_or("mark a reference group (its spectrum must be loaded)")?;
                    let lo = self.tool_value(ToolField::WinLo, cx).unwrap_or(-50.0);
                    let hi = self.tool_value(ToolField::WinHi, cx).unwrap_or(100.0);
                    let shift = sp
                        .align_to(&reference, (lo, hi))
                        .map_err(|e| e.to_string())?;
                    format!("align: {name} → {ref_name} ({shift:+.2} eV)")
                }
                Tool::Calibrate => {
                    let target = self
                        .tool_value(ToolField::Target, cx)
                        .ok_or("enter the target E₀")?;
                    let shift = sp
                        .calibrate(EdgeFeature::DerivativeMax, target)
                        .map_err(|e| e.to_string())?;
                    format!("calibrate: {name} → {target:.1} eV ({shift:+.2})")
                }
                Tool::Deglitch => {
                    let lo = self.tool_value(ToolField::ELo, cx).ok_or("enter a range")?;
                    let hi = self.tool_value(ToolField::EHi, cx).ok_or("enter a range")?;
                    let n = sp.deglitch_range(lo, hi).map_err(|e| e.to_string())?;
                    format!("deglitch: {name} (−{n} pts)")
                }
                Tool::Truncate => {
                    let before = self.tool_value(ToolField::Before, cx);
                    let after = self.tool_value(ToolField::After, cx);
                    sp.truncate(before, after).map_err(|e| e.to_string())?;
                    format!("truncate: {name}")
                }
                Tool::Rebin => {
                    let cfg = RebinConfig {
                        pre_step: self.tool_value(ToolField::PreStep, cx).unwrap_or(10.0),
                        xanes_step: self.tool_value(ToolField::XanesStep, cx).unwrap_or(0.5),
                        exafs_kstep: self.tool_value(ToolField::KStep, cx).unwrap_or(0.05),
                        ..RebinConfig::default()
                    };
                    sp.rebin(&cfg).map_err(|e| e.to_string())?;
                    format!("rebin: {name}")
                }
                Tool::Smooth => {
                    let sigma = self.tool_value(ToolField::Sigma, cx).unwrap_or(1.0);
                    sp.smooth_mu(ConvolveForm::Gaussian, Some(sigma), None)
                        .map_err(|e| e.to_string())?;
                    format!("smooth: {name} (σ {sigma:.2} eV)")
                }
                Tool::Lcf | Tool::Pca => unreachable!("analysis tools run above"),
                Tool::Difference => {
                    let (ref_name, reference) = self
                        .reference_spectrum()
                        .ok_or("mark a second group (its spectrum must be loaded)")?;
                    sp = rexafs::xafs::tools::difference(
                        &sp,
                        &reference,
                        rexafs::xafs::tools::DiffSpace::Norm,
                    )
                    .map_err(|e| e.to_string())?;
                    format!("diff: {name} − {ref_name}")
                }
            };
            Ok(label)
        })();
        match result {
            Ok(label) => {
                let (Some(energy), Some(mu)) = (sp.energy.as_ref(), sp.mu.as_ref()) else {
                    self.tools.message = "tool produced no data".into();
                    cx.notify();
                    return;
                };
                let derived = DerivedSpectrum {
                    label: label.clone(),
                    energy: energy.iter().copied().collect(),
                    mu: mu.iter().copied().collect(),
                    id: self.next_group_id(),
                    params: Some(self.ui_params().clone()),
                    ..Default::default()
                };
                self.record(
                    format!("tool: {label}"),
                    Some(super::journal::UndoOp::DerivedAdd {
                        index: self.derived.len(),
                        spectrum: derived.clone(),
                    }),
                );
                self.derived.push(derived);
                let ix = DERIVED_BASE + self.derived.len() - 1;
                self.tools.message = format!("created {label}").into();
                self.tools.open = None;
                self.select_entry(ix, cx);
                self.sync_param_fields(cx);
                cx.notify();
            }
            Err(message) => {
                self.tools.message = message.into();
                cx.notify();
            }
        }
    }

    /// Processing tool list plus the open tool's inline form.
    pub(crate) fn tools_section(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        self.tool_list(&Tool::PROCESSING, cx)
    }

    /// Analysis tool list (LCF / PCA) plus form and results.
    pub(crate) fn analysis_tools_section(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        self.tool_list(&Tool::ANALYSIS, cx)
    }

    fn tool_list(&self, tools: &[Tool], cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let mut list = div().flex().flex_col().gap_0p5();
        for &tool in tools {
            let open = self.tools.open == Some(tool);
            list = list.child(
                div()
                    .id(SharedString::from(format!("tool-{}", tool.name())))
                    .min_h(px(30.))
                    .px_2()
                    .py_0p5()
                    .flex()
                    .flex_col()
                    .justify_center()
                    .rounded_md()
                    .cursor_pointer()
                    .when(open, |d| {
                        d.bg(gpui::Rgba {
                            a: 0.16,
                            ..t.accent
                        })
                    })
                    .when(!open, |d| d.hover(|d| d.bg(t.raised)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _w, cx| {
                        if this.tools.open == Some(tool) {
                            this.tools.open = None;
                            cx.notify();
                        } else {
                            this.open_tool(tool, cx);
                        }
                    }))
                    .child(div().text_color(t.text).child(tool.name()))
                    .child(
                        div()
                            .text_size(px(10.5))
                            .text_color(t.text_muted)
                            .whitespace_nowrap()
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(tool.hint()),
                    ),
            );
            if open {
                let mut form = div()
                    .mx_1()
                    .mb_1()
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(t.border)
                    .bg(t.bg)
                    .flex()
                    .flex_col();
                if tool.is_analysis() {
                    form = form.child(self.analysis_options(tool, cx));
                }
                for field in tool.fields() {
                    if let Some((_, entity)) = self.tools.fields.iter().find(|(f, _)| f == field) {
                        form = form.child(entity.clone());
                    }
                }
                form =
                    form.child(
                        div()
                            .px_3()
                            .py_1()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(button(&t, "tool-apply", tool.apply_label(), true).on_click(
                                cx.listener(|this, _: &ClickEvent, _w, cx| this.apply_tool(cx)),
                            ))
                            .child(button(&t, "tool-cancel", "Close", false).on_click(
                                cx.listener(|this, _: &ClickEvent, _w, cx| {
                                    this.tools.open = None;
                                    cx.notify();
                                }),
                            )),
                    );
                if !self.tools.message.is_empty() {
                    form = form.child(
                        div()
                            .px_3()
                            .pb_1()
                            .text_size(px(11.))
                            .text_color(t.text_muted)
                            .child(self.tools.message.clone()),
                    );
                }
                list = list.child(form);
                if tool.is_analysis() {
                    list = list.children(self.analysis_results(tool, cx));
                }
            }
        }
        list
    }

    /// Space segment + option chips shared by LCF and PCA.
    fn analysis_options(&self, tool: Tool, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let mut seg = super::segmented(&t);
        for (i, choice) in LcfSpaceChoice::ALL.into_iter().enumerate() {
            seg = seg.child(
                super::segment(
                    &t,
                    SharedString::from(format!("lcf-space-{i}")),
                    choice.label(),
                    self.tools.lcf_space == choice,
                    i == 0,
                )
                .on_click(cx.listener(move |this, _: &ClickEvent, _w, cx| {
                    this.tools.lcf_space = choice;
                    cx.notify();
                })),
            );
        }
        let mut row = div()
            .px_3()
            .py_1()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_1p5()
            .child(seg);
        if tool == Tool::Lcf {
            row = row
                .child(
                    super::chip(&t, "lcf-sum", "Σ = 1", self.tools.lcf_sum_to_one).on_click(
                        cx.listener(|this, _: &ClickEvent, _w, cx| {
                            this.tools.lcf_sum_to_one = !this.tools.lcf_sum_to_one;
                            cx.notify();
                        }),
                    ),
                )
                .child(
                    super::chip(&t, "lcf-e0", "E₀ shifts", self.tools.lcf_e0_shift).on_click(
                        cx.listener(|this, _: &ClickEvent, _w, cx| {
                            this.tools.lcf_e0_shift = !this.tools.lcf_e0_shift;
                            cx.notify();
                        }),
                    ),
                )
                .child(
                    super::chip(
                        &t,
                        "lcf-all",
                        "all combinations",
                        self.tools.lcf_all_combinations,
                    )
                    .on_click(cx.listener(|this, _: &ClickEvent, _w, cx| {
                        this.tools.lcf_all_combinations = !this.tools.lcf_all_combinations;
                        cx.notify();
                    })),
                );
        }
        row
    }

    /// Result tables under the analysis form.
    fn analysis_results(&self, tool: Tool, _cx: &mut Context<Self>) -> Vec<gpui::AnyElement> {
        let t = self.theme;
        let mut out: Vec<gpui::AnyElement> = Vec::new();
        let row = |k: String, v: String, warn: bool| {
            div()
                .px_3()
                .flex()
                .items_center()
                .justify_between()
                .text_size(px(11.5))
                .child(div().text_color(t.text_muted).child(k))
                .child(
                    div()
                        .font_family(super::MONO)
                        .text_color(if warn { t.warn } else { t.text })
                        .child(v),
                )
                .into_any_element()
        };
        match tool {
            Tool::Lcf => {
                if let Some(r) = &self.analysis.lcf {
                    for c in &r.weights {
                        let sigma = c.stderr.map(|e| format!(" ± {e:.3}")).unwrap_or_default();
                        let shift = if c.e0_shift.abs() > 1e-9 {
                            format!(" · ΔE {:+.2}", c.e0_shift)
                        } else {
                            String::new()
                        };
                        out.push(row(
                            c.name.clone(),
                            format!("{:.3}{sigma}{shift}", c.weight),
                            false,
                        ));
                    }
                    out.push(row(
                        "Σ weights".into(),
                        format!("{:.3}", r.sum_of_weights),
                        false,
                    ));
                    out.push(row("R-factor".into(), format!("{:.3e}", r.r_factor), false));
                    out.push(row(
                        "reduced χ²".into(),
                        format!("{:.3e}", r.reduced_chi_square),
                        false,
                    ));
                }
                for (i, r) in self.analysis.ranked.iter().enumerate().skip(1).take(4) {
                    let combo = r
                        .weights
                        .iter()
                        .map(|c| format!("{} {:.2}", c.name, c.weight))
                        .collect::<Vec<_>>()
                        .join(" · ");
                    out.push(row(
                        format!("#{} {combo}", i + 1),
                        format!("R {:.2e}", r.r_factor),
                        false,
                    ));
                }
            }
            Tool::Pca => {
                if let Some(m) = &self.analysis.pca {
                    let ind_min = m.suggested_components_ind();
                    for i in 0..m.n_components().min(6) {
                        let var = m.variance_explained.get(i).copied().unwrap_or(0.0) * 100.0;
                        let cum = m.cumulative_variance.get(i).copied().unwrap_or(0.0) * 100.0;
                        let ind = m
                            .ind
                            .get(i)
                            .map(|v| format!(" · IND {v:.2e}"))
                            .unwrap_or_default();
                        out.push(row(
                            format!(
                                "PC{} {}",
                                i + 1,
                                if i + 1 == ind_min { "← IND" } else { "" }
                            ),
                            format!("{var:.2} % · Σ {cum:.2} %{ind}"),
                            false,
                        ));
                    }
                }
                if let Some(f) = &self.analysis.pca_fit {
                    out.push(row(
                        format!("target transform ({} comp.)", f.n_components),
                        format!("R {:.2e}", f.r_factor),
                        f.r_factor > 1e-2,
                    ));
                }
            }
            _ => {}
        }
        out
    }
}

/// Replace the index-based component names with the group labels.
fn relabel(result: &mut rexafs::prelude::LcfResult, names: &[String]) {
    for c in &mut result.weights {
        if let Some(n) = names.get(c.index) {
            c.name = n.clone();
        }
    }
}
