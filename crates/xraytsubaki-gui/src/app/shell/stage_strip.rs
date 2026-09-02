//! The stage strip: six pipeline stages with a status dot and a one-line
//! summary each, so the whole state of the current group reads at a glance.

use gpui::{
    ClickEvent, Context, IntoElement, ParentElement, SharedString, Styled, div, prelude::*, px,
};

use super::{MONO, Stage, StageStatus};
use crate::app::StudioApp;

impl StudioApp {
    pub(crate) fn stage_strip(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        // Summaries shrink before the strip overflows; the dot legend is the
        // first thing to go at narrow widths (the prototype does the same).
        let vw = self.viewport_w;
        let summary_w = if vw < 1300. {
            96.
        } else if vw < 1500. {
            128.
        } else {
            190.
        };
        let show_legend = vw >= 1500.;
        let mut strip = div()
            .h(px(40.))
            .w_full()
            .min_w_0()
            .flex_none()
            .flex()
            .items_stretch()
            .px_2()
            .bg(t.surface)
            .border_b_1()
            .border_color(t.border)
            .overflow_hidden();
        for stage in Stage::ALL {
            let active = self.stage == stage;
            let (status, summary) = self.stage_summary(stage);
            let summary: SharedString = summary.into();
            strip = strip.child(
                div()
                    .id(SharedString::from(format!("stage-{}", stage.number())))
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .min_w_0()
                    .cursor_pointer()
                    .border_b_2()
                    .border_color(if active {
                        t.accent
                    } else {
                        gpui::Rgba { a: 0.0, ..t.border }
                    })
                    .when(!active, |d| d.hover(|d| d.bg(t.raised)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                        this.set_stage(stage, cx);
                    }))
                    .child(
                        div()
                            .font_family(MONO)
                            .text_size(px(10.5))
                            .text_color(t.text_muted)
                            .child(stage.number().to_string()),
                    )
                    .child(
                        div()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(if active { t.accent } else { t.text })
                            .child(stage.name()),
                    )
                    .child(
                        div()
                            .w(px(7.))
                            .h(px(7.))
                            .rounded_full()
                            .bg(status.color(&t)),
                    )
                    .child(
                        div()
                            .font_family(MONO)
                            .text_size(px(11.))
                            .text_color(t.text_muted)
                            .whitespace_nowrap()
                            .overflow_hidden()
                            .text_ellipsis()
                            .max_w(px(summary_w))
                            .child(summary),
                    ),
            );
        }
        strip.child(div().flex_1()).when(show_legend, |strip| {
            strip.child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_size(px(11.))
                    .text_color(t.text_muted)
                    .whitespace_nowrap()
                    .child(legend_dot(t.success))
                    .child("ok")
                    .child(legend_dot(t.accent))
                    .child("auto")
                    .child(legend_dot(t.warn))
                    .child("attention"),
            )
        })
    }

    /// Status and one-line summary per stage for the current group.
    pub(crate) fn stage_summary(&self, stage: Stage) -> (StageStatus, String) {
        let p = self.ui_params();
        let sp = self.spectrum.as_deref();
        match stage {
            Stage::Data => {
                let groups = if self.catalog.is_empty() {
                    usize::from(self.spectrum.is_some()) + self.derived.len()
                } else {
                    self.catalog.len() + self.derived.len()
                };
                let series = self.catalog.scans.len();
                let status = if self.spectrum.is_some() {
                    StageStatus::Ok
                } else {
                    StageStatus::Idle
                };
                (
                    status,
                    if series > 0 {
                        format!("{groups} files · {series} scans")
                    } else {
                        format!("{groups} files")
                    },
                )
            }
            Stage::Normalize => {
                let Some(sp) = sp else {
                    return (StageStatus::Idle, "—".into());
                };
                let e0 = sp.get_e0().map(|v| format!("{v:.1}")).unwrap_or("?".into());
                let step = self
                    .edge_step()
                    .map(|v| format!("{v:.3}"))
                    .unwrap_or("?".into());
                let status = if p.e0.is_none() {
                    StageStatus::Auto
                } else {
                    StageStatus::Ok
                };
                (status, format!("E₀ {e0} · step {step}"))
            }
            Stage::Background => {
                let rbkg = p.rbkg.unwrap_or(1.0);
                let kw = p.bkg_kweight.unwrap_or(1);
                let status = if sp.is_some_and(|s| s.get_chi().is_some()) {
                    if p.rbkg.is_none() {
                        StageStatus::Auto
                    } else {
                        StageStatus::Ok
                    }
                } else {
                    StageStatus::Idle
                };
                (status, format!("Rbkg {rbkg:.1} · kw {kw}"))
            }
            Stage::Transform => {
                let (kmin, kmax, win, kw) = self.fft_summary();
                let status = if sp.is_some_and(|s| s.get_chir_mag().is_some()) {
                    StageStatus::Ok
                } else {
                    StageStatus::Idle
                };
                (
                    status,
                    format!("k {kmin:.1}–{kmax:.1} · {win} · kw {kw:.0}"),
                )
            }
            Stage::Fit => {
                let paths = self.fit_paths.len();
                match &self.fit_result {
                    Some(r) => (
                        StageStatus::Ok,
                        format!("{paths} paths · R {:.4}", r.r_factor),
                    ),
                    None if paths > 0 => (StageStatus::Attention, format!("{paths} paths · unfit")),
                    None => (StageStatus::Idle, "no paths".into()),
                }
            }
            Stage::Series => match self.operando_scan_len() {
                Some(n) => (StageStatus::Ok, format!("{n} frames")),
                None if !self.catalog.scans.is_empty() => (
                    StageStatus::Idle,
                    format!("{} scans", self.catalog.scans.len()),
                ),
                None => (StageStatus::Idle, "no scan".into()),
            },
        }
    }

    /// Effective FT summary values (params, else the core defaults).
    pub(crate) fn fft_summary(&self) -> (f64, f64, &'static str, f64) {
        let p = self.ui_params();
        let defaults = xraytsubaki::prelude::XrayFFTF::default();
        let kmin = p.fft_kmin.or(defaults.kmin).unwrap_or(2.0);
        let kmax = p
            .fft_kmax
            .or(defaults.kmax)
            .or_else(|| {
                self.spectrum
                    .as_ref()
                    .and_then(|s| s.get_k())
                    .and_then(|k| k.iter().next_back().copied())
            })
            .unwrap_or(15.0);
        let win = crate::params::window_label(p.fft_window.or(defaults.window));
        let kw = p.fft_kweight.or(defaults.kweight).unwrap_or(2.0);
        (kmin, kmax, win, kw)
    }

    /// Edge step of the current spectrum.
    pub(crate) fn edge_step(&self) -> Option<f64> {
        use xraytsubaki::prelude::NormalizationMethod;
        match self.spectrum.as_ref()?.normalization.as_ref()? {
            NormalizationMethod::PrePostEdge(ppe) => ppe.edge_step,
            _ => None,
        }
    }
}

fn legend_dot(color: gpui::Rgba) -> impl IntoElement {
    div().w(px(7.)).h(px(7.)).rounded_full().bg(color)
}
