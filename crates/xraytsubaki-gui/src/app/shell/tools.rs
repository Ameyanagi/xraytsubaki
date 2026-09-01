//! Processing tools (Athena's Data menu) as non-destructive inline forms:
//! every tool reads the current group and produces a *derived* group, so
//! the source is never mutated and nothing needs undo.

use gpui::{
    ClickEvent, Context, Entity, IntoElement, ParentElement, SharedString, Styled, div, prelude::*,
    px,
};
use xraytsubaki::prelude::XASSpectrum;
use xraytsubaki::xafs::tools::{EdgeFeature, RebinConfig};
use xraytsubaki::xafs::xafsutils::ConvolveForm;

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
}

impl Tool {
    pub const ALL: [Tool; 7] = [
        Tool::Align,
        Tool::Calibrate,
        Tool::Deglitch,
        Tool::Truncate,
        Tool::Rebin,
        Tool::Smooth,
        Tool::Difference,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Tool::Align => "Align to reference",
            Tool::Calibrate => "Calibrate energy",
            Tool::Deglitch => "Deglitch",
            Tool::Truncate => "Truncate",
            Tool::Rebin => "Rebin",
            Tool::Smooth => "Smooth",
            Tool::Difference => "Difference spectrum",
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
}

impl ToolField {
    const ALL: [ToolField; 11] = [
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
        }
    }
}

#[derive(Default)]
pub struct ToolState {
    pub open: Option<Tool>,
    pub fields: Vec<(ToolField, Entity<NumericField>)>,
    pub message: SharedString,
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
        self.set_stage(super::Stage::Data, cx);
        cx.notify();
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
                Tool::Difference => {
                    let (ref_name, reference) = self
                        .reference_spectrum()
                        .ok_or("mark a second group (its spectrum must be loaded)")?;
                    sp = xraytsubaki::xafs::tools::difference(
                        &sp,
                        &reference,
                        xraytsubaki::xafs::tools::DiffSpace::Norm,
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
                };
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

    /// Tool list plus the open tool's inline form.
    pub(crate) fn tools_section(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let mut list = div().flex().flex_col().gap_0p5();
        for tool in Tool::ALL {
            let open = self.tools.open == Some(tool);
            list = list.child(
                div()
                    .id(SharedString::from(format!("tool-{}", tool.name())))
                    .h(px(28.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
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
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(px(10.5))
                            .text_color(t.text_muted)
                            .whitespace_nowrap()
                            .overflow_hidden()
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
                            .child(
                                button(&t, "tool-apply", "Apply → new group", true).on_click(
                                    cx.listener(|this, _: &ClickEvent, _w, cx| this.apply_tool(cx)),
                                ),
                            )
                            .child(button(&t, "tool-cancel", "Cancel", false).on_click(
                                cx.listener(|this, _: &ClickEvent, _w, cx| {
                                    this.tools.open = None;
                                    cx.notify();
                                }),
                            )),
                    );
                list = list.child(form);
            }
        }
        if !self.tools.message.is_empty() {
            list = list.child(
                div()
                    .px_2()
                    .py_1()
                    .text_size(px(11.))
                    .text_color(t.text_muted)
                    .child(self.tools.message.clone()),
            );
        }
        list
    }
}
