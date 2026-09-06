//! Distinct stage tabs with concise summaries and per-spectrum settings on hover.

use gpui::{
    ClickEvent, Context, IntoElement, ParentElement, SharedString, Styled, div, prelude::*, px,
};

use super::{MONO, Stage, StageStatus};
use crate::app::StudioApp;

impl StudioApp {
    pub(crate) fn stage_strip(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let tint = |strength: f32| gpui::Rgba {
            r: t.raised.r * (1. - strength) + t.accent.r * strength,
            g: t.raised.g * (1. - strength) + t.accent.g * strength,
            b: t.raised.b * (1. - strength) + t.accent.b * strength,
            a: 1.,
        };
        let mut strip = div()
            .h(px(58.))
            .w_full()
            .min_w_0()
            .flex_none()
            .flex()
            .items_stretch()
            .gap_1()
            .px_2()
            .py_1()
            .bg(t.bg)
            .border_b_1()
            .border_color(t.border);
        for stage in Stage::ALL {
            let active = self.stage == stage;
            let (status, summary) = self.stage_summary(stage);
            let tip = self.stage_tooltip(stage);
            strip = strip.child(
                div()
                    .id(SharedString::from(format!("stage-{}", stage.number())))
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .justify_center()
                    .gap_1()
                    .px_3()
                    .rounded_md()
                    .cursor_pointer()
                    .border_1()
                    .border_color(if active { t.accent } else { t.border })
                    .bg(if active { tint(0.15) } else { t.surface })
                    .hover(move |d| d.bg(tint(0.09)).border_color(t.accent))
                    .tooltip(move |_, cx| cx.new(|_| tip.clone()).into())
                    .on_click(
                        cx.listener(move |this, _: &ClickEvent, _, cx| this.set_stage(stage, cx)),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .flex_none()
                                    .text_size(px(12.))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(if active { t.accent } else { t.text })
                                    .child(stage.name()),
                            )
                            .child(div().flex_1())
                            .child(
                                div()
                                    .flex_none()
                                    .w(px(6.))
                                    .h(px(6.))
                                    .rounded_full()
                                    .bg(status.color(&t)),
                            ),
                    )
                    .when(self.viewport_w >= 950., |d| {
                        d.child(
                            div()
                                .font_family(MONO)
                                .text_size(px(10.))
                                .text_color(t.text_muted)
                                .whitespace_nowrap()
                                .overflow_hidden()
                                .text_ellipsis()
                                .child(summary),
                        )
                    }),
            );
        }
        strip
    }

    fn stage_tooltip(&self, stage: Stage) -> StageTooltip {
        use rexafs::prelude::{BackgroundMethod, NormalizationMethod};
        let p = self.ui_params();
        let updating = self.load_running || self.recompute_dirty;
        let sp = (!updating && self.stale_plots.is_none())
            .then_some(self.spectrum.as_deref())
            .flatten();
        let norm = sp
            .and_then(|s| s.normalization.as_ref())
            .and_then(|n| match n {
                NormalizationMethod::PrePostEdge(n) => Some(n),
                _ => None,
            });
        let bkg = sp
            .and_then(|s| s.background.as_ref())
            .and_then(|b| match b {
                BackgroundMethod::AUTOBK(b) => Some(b),
                _ => None,
            });
        let ft = sp.and_then(|s| s.xftf.as_ref());
        let mut rows = Vec::<(String, String)>::new();
        let mut add = |label: &str, value: String| rows.push((label.into(), value));
        let mut spectrum = self.current_group_label().to_string();
        match stage {
            Stage::Data => {
                let points = sp.and_then(|s| s.energy.as_ref()).map(|e| e.len());
                add(
                    "Points",
                    points.map(|n| n.to_string()).unwrap_or_else(|| "—".into()),
                );
                let energy = sp.and_then(|s| s.energy.as_ref());
                add(
                    "Energy",
                    range_value(
                        energy.and_then(|e| e.iter().next().copied()),
                        energy.and_then(|e| e.iter().next_back().copied()),
                        "eV",
                    ),
                );
                add("Import", format!("{:?}", p.import.mode));
                add("Marked", self.selection.len().to_string());
            }
            Stage::Normalize => {
                add("E₀", number_value(p.e0, sp.and_then(|s| s.e0()), "eV", 2));
                add(
                    "Edge step",
                    sp.and_then(|_| self.edge_step())
                        .map(|n| format!("{n:.5}"))
                        .unwrap_or_else(|| "—".into()),
                );
                add(
                    "Pre-edge · E − E₀",
                    range_value(
                        p.pre_edge_start
                            .or_else(|| norm.and_then(|n| n.pre_edge_start)),
                        p.pre_edge_end.or_else(|| norm.and_then(|n| n.pre_edge_end)),
                        "eV",
                    ),
                );
                add(
                    "Normalization · E − E₀",
                    range_value(
                        p.norm_start.or_else(|| norm.and_then(|n| n.norm_start)),
                        p.norm_end.or_else(|| norm.and_then(|n| n.norm_end)),
                        "eV",
                    ),
                );
                add(
                    "Polynomial order",
                    p.norm_polyorder
                        .or_else(|| norm.and_then(|n| n.norm_polyorder))
                        .map(|n| n.to_string())
                        .unwrap_or_else(|| "Auto".into()),
                );
            }
            Stage::Background => {
                add(
                    "Rbkg",
                    number_value(p.rbkg, bkg.and_then(|b| b.rbkg), "Å", 2),
                );
                let kmax = bkg.and_then(|b| b.kmax).or_else(|| {
                    sp.and_then(|s| s.k())
                        .and_then(|k| k.iter().next_back().copied())
                });
                add(
                    "k range",
                    range_value(
                        p.bkg_kmin.or_else(|| bkg.and_then(|b| b.kmin)),
                        p.bkg_kmax.or(kmax),
                        "Å⁻¹",
                    ),
                );
                add(
                    "k-weight",
                    number_value(
                        p.bkg_kweight.map(|n| n as f64),
                        bkg.and_then(|b| b.kweight).map(|n| n as f64),
                        "",
                        0,
                    ),
                );
                add(
                    "Window",
                    crate::params::window_label(p.bkg_window.or_else(|| bkg.map(|b| b.window)))
                        .into(),
                );
            }
            Stage::Transform => {
                add(
                    "k range",
                    range_value(
                        p.fft_kmin.or_else(|| ft.and_then(|f| f.kmin)),
                        p.fft_kmax.or_else(|| ft.and_then(|f| f.kmax)),
                        "Å⁻¹",
                    ),
                );
                add(
                    "k-weight",
                    number_value(p.fft_kweight, ft.and_then(|f| f.kweight), "", 0),
                );
                add(
                    "Window",
                    crate::params::window_label(p.fft_window.or_else(|| ft.and_then(|f| f.window)))
                        .into(),
                );
                add(
                    "dk",
                    number_value(p.fft_dk, ft.and_then(|f| f.dk), "Å⁻¹", 2),
                );
                let back = sp.and_then(|s| s.xftr.as_ref());
                add(
                    "Back FT · R range",
                    range_value(
                        p.bft_rmin.or_else(|| back.and_then(|f| f.rmin)),
                        p.bft_rmax.or_else(|| back.and_then(|f| f.rmax)),
                        "Å",
                    ),
                );
            }
            Stage::Fit => {
                let dataset = self
                    .joint
                    .config
                    .enabled
                    .then(|| {
                        self.joint
                            .config
                            .datasets
                            .iter()
                            .find(|d| {
                                self.stage == Stage::Fit
                                    && Some(d.id) == self.joint_plotted_dataset_id()
                            })
                            .or_else(|| {
                                self.joint
                                    .config
                                    .datasets
                                    .iter()
                                    .find(|d| d.file == self.current_path)
                            })
                            .or_else(|| self.joint.config.datasets.first())
                    })
                    .flatten();
                let (r, paths, weight) = if let Some(d) = dataset {
                    spectrum = d.label.clone();
                    (
                        d.ranges.as_ref().unwrap_or(&self.fit_ranges),
                        d.paths.len(),
                        self.joint_params(&d.file).fft_kweight,
                    )
                } else {
                    (
                        &self.fit_ranges,
                        self.fit_paths.iter().filter(|p| p.spec.enabled).count(),
                        p.fft_kweight,
                    )
                };
                let r = r.resolved(weight);
                add("Fit in", format!("{:?}", r.fitspace));
                add("k range", range_value(Some(r.kmin), Some(r.kmax), "Å⁻¹"));
                add("R range", range_value(Some(r.rmin), Some(r.rmax), "Å"));
                add(
                    "k-weight",
                    r.effective_kweights()
                        .iter()
                        .map(|n| n.to_string())
                        .collect::<Vec<_>>()
                        .join(", "),
                );
                add("Selected paths", paths.to_string());
                if let Some(result) = &self.fit_result {
                    add("Latest fit · R-factor", format!("{:.5}", result.r_factor));
                }
            }
            Stage::Series => {
                add("Scans", self.catalog.scans.len().to_string());
                add(
                    "Frames",
                    self.operando_scan_len()
                        .map(|n| n.to_string())
                        .unwrap_or_else(|| "—".into()),
                );
                add("Marked spectra", self.selection.len().to_string());
            }
            Stage::Publish => {
                add("Figures", "PNG · 1600 × 1000".into());
                add("Analysis", "Markdown · JSON · project".into());
                add("References", "Markdown · BibTeX".into());
                add("Recorded fits", self.fit_history.len().to_string());
            }
        }
        StageTooltip {
            title: stage.name().into(),
            spectrum,
            rows,
            theme: self.theme,
            updating,
        }
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
                let e0 = sp.e0().map(|v| format!("{v:.1}")).unwrap_or("?".into());
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
                let status = if sp.is_some_and(|s| s.chi().is_some()) {
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
                let status = if sp.is_some_and(|s| s.chir_mag().is_some()) {
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
            Stage::Publish => (StageStatus::Idle, "Figures · Markdown".into()),
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
        let defaults = rexafs::prelude::XrayFFTF::default();
        let kmin = p.fft_kmin.or(defaults.kmin).unwrap_or(2.0);
        let kmax = p
            .fft_kmax
            .or(defaults.kmax)
            .or_else(|| {
                self.spectrum
                    .as_ref()
                    .and_then(|s| s.k())
                    .and_then(|k| k.iter().next_back().copied())
            })
            .unwrap_or(15.0);
        let win = crate::params::window_label(p.fft_window.or(defaults.window));
        let kw = p.fft_kweight.or(defaults.kweight).unwrap_or(2.0);
        (kmin, kmax, win, kw)
    }

    /// Edge step of the current spectrum.
    pub(crate) fn edge_step(&self) -> Option<f64> {
        use rexafs::prelude::NormalizationMethod;
        match self.spectrum.as_ref()?.normalization.as_ref()? {
            NormalizationMethod::PrePostEdge(ppe) => ppe.edge_step,
            _ => None,
        }
    }
}

fn number_value(
    requested: Option<f64>,
    resolved: Option<f64>,
    unit: &str,
    decimals: usize,
) -> String {
    let Some(value) = requested.or(resolved) else {
        return "Auto".into();
    };
    let auto = if requested.is_none() { " · Auto" } else { "" };
    format!("{value:.decimals$} {unit}{auto}").trim().to_owned()
}
fn range_value(lo: Option<f64>, hi: Option<f64>, unit: &str) -> String {
    let value = |n: Option<f64>| {
        n.map(|n| format!("{n:.2}"))
            .unwrap_or_else(|| "Auto".into())
    };
    format!("{} – {} {unit}", value(lo), value(hi))
}
#[derive(Clone)]
struct StageTooltip {
    title: String,
    spectrum: String,
    rows: Vec<(String, String)>,
    theme: crate::theme::Theme,
    updating: bool,
}
impl gpui::Render for StageTooltip {
    fn render(&mut self, _: &mut gpui::Window, _: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        div()
            .w(px(350.))
            .p_3()
            .rounded_lg()
            .bg(t.raised)
            .border_1()
            .border_color(t.border)
            .shadow_lg()
            .text_color(t.text)
            .text_size(px(12.))
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(self.title.clone()),
                    )
                    .when(self.updating, |d| {
                        d.child(
                            div()
                                .text_color(t.warn)
                                .text_size(px(10.))
                                .child("Updating…"),
                        )
                    }),
            )
            .child(
                div()
                    .text_size(px(10.5))
                    .text_color(t.text_muted)
                    .overflow_hidden()
                    .text_ellipsis()
                    .whitespace_nowrap()
                    .child(self.spectrum.clone()),
            )
            .children(self.rows.iter().map(|(label, value)| {
                div()
                    .flex()
                    .gap_3()
                    .child(div().flex_1().text_color(t.text_muted).child(label.clone()))
                    .child(
                        div()
                            .font_family(MONO)
                            .whitespace_nowrap()
                            .child(value.clone()),
                    )
            }))
    }
}
