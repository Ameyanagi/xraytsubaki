//! Inspect the FEFF arrays while keeping the path selection table visible.
use super::chip;
use crate::app::StudioApp;
use gpui::{Context, Entity, IntoElement, ParentElement, Styled, div, prelude::*, px};
use rexafs::xafs::fitting::{FeffFlavor, FeffPathModel, FitVariables, feffpath, path2chi};
use ruviz_gpui::{RuvizPlot, plot_builder};

#[derive(Default)]
pub(crate) struct PathDiagnostics {
    pub open: bool,
    metric: Metric,
    key: Option<(std::path::PathBuf, Option<std::time::SystemTime>, Metric)>,
    plot: Option<Entity<RuvizPlot>>,
    error: Option<String>,
}

#[derive(Clone, Copy, Default, PartialEq)]
enum Metric {
    #[default]
    Amplitude,
    Scattering,
    Phase,
    Lambda,
    Chi,
}
impl Metric {
    fn label(self) -> &'static str {
        match self {
            Self::Amplitude => "F_eff",
            Self::Scattering => "|f_eff|",
            Self::Phase => "Phase",
            Self::Lambda => "λ(k)",
            Self::Chi => "k²χ(k)",
        }
    }
    fn axis(self) -> &'static str {
        match self {
            Self::Amplitude => "F_eff(k) (Å)",
            Self::Scattering => "|f_eff(k)| (Å)",
            Self::Phase => "Total phase (rad)",
            Self::Lambda => "λ(k) (Å)",
            Self::Chi => "k²χ(k) (Å⁻²)",
        }
    }
    fn description(self) -> &'static str {
        match self {
            Self::Amplitude => {
                "Effective amplitude · |f_eff| × reduction factor, before degeneracy and fit parameters."
            }
            Self::Scattering => {
                "Scattering amplitude magnitude from the FEFF path file, before the reduction factor."
            }
            Self::Phase => "Total phase shift · central-atom phase + scattering phase (radians).",
            Self::Lambda => "Photoelectron mean free path from the FEFF calculation (Å).",
            Self::Chi => {
                "Reference path · FEFF degeneracy, S₀² = 1; ΔE₀ = ΔR = σ² = 0. Not the fitted contribution."
            }
        }
    }
}

fn curve(path: &FeffPathModel, metric: Metric) -> Result<(Vec<f64>, Vec<f64>), String> {
    let f = &path.feff;
    // FEFF's high-k table is too coarse to sample oscillatory chi directly.
    let k = if metric == Metric::Chi && f.k.len() >= 3 {
        let start = f.k[0];
        let end = f.k[f.k.len() - 1];
        let n = ((end - start) / 0.025).ceil() as usize;
        nalgebra::DVector::from_iterator(
            n + 1,
            (0..=n).map(|i| start + (end - start) * i as f64 / n.max(1) as f64),
        )
    } else {
        f.k.clone()
    };
    let y = match metric {
        Metric::Amplitude => f.amp.clone(),
        Metric::Scattering => f.mag_feff.clone(),
        Metric::Phase => f.pha.clone(),
        Metric::Lambda => f.lam.clone(),
        Metric::Chi => path2chi(path, &FitVariables::new(), &k)
            .map_err(|e| e.to_string())?
            .component_mul(&k.map(|k| k * k)),
    };
    let pairs: Vec<_> = k
        .iter()
        .zip(y.iter())
        .filter(|(k, y)| k.is_finite() && y.is_finite())
        .map(|(k, y)| (*k, *y))
        .collect();
    if pairs.len() < 3 {
        return Err("This FEFF path has insufficient finite data for the selected curve.".into());
    }
    Ok(pairs.into_iter().unzip())
}

impl StudioApp {
    pub(super) fn path_view_tabs(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let open = self.structure.diagnostics.open;
        div()
            .flex()
            .flex_wrap()
            .gap_1()
            .child(
                chip(&t, "path-geometry", "Geometry", !open).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.structure.diagnostics.open = false;
                        cx.notify();
                    },
                )),
            )
            .child(
                chip(&t, "path-feff-curves", "FEFF curves", open).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.structure.diagnostics.open = true;
                        cx.notify();
                    },
                )),
            )
    }

    fn refresh_path_curve(&mut self, cx: &mut Context<Self>) {
        let selected = self.structure.selected.and_then(|i| self.fit_paths.get(i));
        let Some(path) = selected.map(|p| p.spec.file.clone()) else {
            self.structure.diagnostics.plot = None;
            self.structure.diagnostics.key = None;
            self.structure.diagnostics.error = Some("Click a path row to inspect its FEFF curves. The checkbox controls whether it is used in the fit.".into());
            return;
        };
        let metric = self.structure.diagnostics.metric;
        let key = (
            path.clone(),
            std::fs::metadata(&path)
                .ok()
                .and_then(|m| m.modified().ok()),
            metric,
        );
        if self.structure.diagnostics.key.as_ref() == Some(&key) {
            return;
        }
        self.structure.diagnostics.key = Some(key.clone());
        self.structure.diagnostics.plot = None;
        self.structure.diagnostics.error = None;
        let job = cx.background_executor().spawn(async move {
            let model = feffpath(&path.to_string_lossy(), FeffFlavor::Feff85L)
                .map_err(|e| e.to_string())?;
            curve(&model, metric)
        });
        cx.spawn(async move |this, cx| {
            let result = job.await;
            this.update(cx, |app, cx| {
                if app.structure.diagnostics.key.as_ref() != Some(&key) {
                    return;
                }
                match result {
                    Ok((k, y)) => {
                        let plot: ruviz::core::Plot = ruviz::core::Plot::new()
                            .theme(app.theme.plot_theme())
                            .line(&k, &y)
                            .xlabel("k (Å⁻¹)")
                            .ylabel(metric.axis())
                            .into();
                        app.structure.diagnostics.plot =
                            Some(plot_builder(plot.size_px(760, 610)).interactive().build(cx));
                    }
                    Err(e) => app.structure.diagnostics.error = Some(e),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn path_diagnostics_center(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        self.refresh_path_curve(cx);
        let t = self.theme;
        let state = &self.structure.diagnostics;
        let mut metrics = div().flex().flex_wrap().gap_1();
        for (i, metric) in [
            Metric::Amplitude,
            Metric::Scattering,
            Metric::Phase,
            Metric::Lambda,
            Metric::Chi,
        ]
        .into_iter()
        .enumerate()
        {
            metrics = metrics.child(
                chip(
                    &t,
                    ("feff-curve", i),
                    metric.label(),
                    state.metric == metric,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.structure.diagnostics.metric = metric;
                    cx.notify();
                })),
            );
        }
        let path = self.structure.selected.and_then(|i| self.fit_paths.get(i));
        let title = path
            .map(|p| {
                format!(
                    "{} · {}",
                    p.spec.label,
                    p.spec
                        .file
                        .parent()
                        .and_then(|p| p.file_name())
                        .unwrap_or_default()
                        .to_string_lossy()
                )
            })
            .unwrap_or_else(|| "Select a path".into());
        let mut plot = div()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .rounded_lg()
            .bg(t.raised)
            .border_1()
            .border_color(t.border)
            .overflow_hidden();
        if let Some(p) = &state.plot {
            plot = plot.child(p.clone());
        } else {
            plot = plot.flex().items_center().justify_center().px_3().child(
                state
                    .error
                    .clone()
                    .unwrap_or_else(|| "Reading FEFF path…".into()),
            );
        }
        let left = div()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .gap_2()
            .child(self.path_view_tabs(cx))
            .child(div().text_size(px(13.)).child(title))
            .when_some(path.and_then(|p| p.meta), |d, m| {
                d.child(
                    div()
                        .text_size(px(11.))
                        .text_color(t.text_muted)
                        .child(format!(
                            "R_eff {:.4} Å · {} legs · degeneracy {}",
                            m.reff, m.nleg, m.degen
                        )),
                )
            })
            .child(metrics)
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(t.text_muted)
                    .child(state.metric.description()),
            )
            .child(plot);
        div()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .flex()
            .gap_2()
            .px_3()
            .pt_2()
            .pb_3()
            .child(left)
            .child(
                div()
                    .w(px(430.))
                    .flex_none()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .rounded_lg()
                    .border_1()
                    .border_color(t.border)
                    .bg(t.surface)
                    .overflow_hidden()
                    .child(self.structure_paths_table(true, cx)),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn feff_diagnostics_use_physical_arrays_and_reference_chi() {
        let file = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../rexafs/tests/testfiles/feffcu01.dat"
        );
        // Resolve the maintained core test fixture, independent of temporary workspaces.
        let file = if std::path::Path::new(file).exists() {
            std::path::PathBuf::from(file)
        } else {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../rexafs/tests/testfiles/xraylarch_d867/feffit/feffcu01.dat")
        };
        let model = feffpath(&file.to_string_lossy(), FeffFlavor::Feff85L).unwrap();
        for metric in [
            Metric::Amplitude,
            Metric::Scattering,
            Metric::Phase,
            Metric::Lambda,
            Metric::Chi,
        ] {
            let (x, y) = curve(&model, metric).unwrap();
            assert!(x.len() >= model.feff.k.len());
            assert!(y.iter().all(|v| v.is_finite()));
        }
        let (_, amp) = curve(&model, Metric::Amplitude).unwrap();
        assert!((amp[10] - model.feff.mag_feff[10] * model.feff.red_fact[10]).abs() < 1e-12);
    }
}
