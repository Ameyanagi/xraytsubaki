//! Live data transforms for choosing fit ranges, independent of fitted results.
use super::{FitView, chip, fit_workspace::FitStep};
use crate::{
    app::StudioApp,
    fitting::{FitRanges, FitSpaceSpec},
    params::process_file,
    plotting::{chik_label, chir_label, trace_color},
};
use gpui::{Context, Entity, ParentElement, Styled, div, prelude::*, px};
use nalgebra::DVector;
use ruviz::{prelude::Plot, render::LineStyle};
use ruviz_gpui::{RuvizPlot, plot_builder};
use std::{
    collections::{BTreeMap, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
    path::PathBuf,
    sync::Arc,
};
use xraytsubaki::xafs::fitting::transform::{
    KweightTransform, apply_kweight_transform, validate_transform,
};

pub(crate) const PREVIEW_K: usize = 200;
pub(crate) const PREVIEW_R: usize = 201;
pub(crate) const PREVIEW_Q: usize = 204;
type Input = Arc<(DVector<f64>, DVector<f64>)>;
pub(crate) struct PreviewData {
    pub input: Input,
    pub ranges: FitRanges,
    pub arrays: KweightTransform,
    pub q: Vec<f64>,
}
fn transform(input: Input, ranges: FitRanges) -> Result<PreviewData, String> {
    if !ranges.valid() {
        return Err("Set a valid k and R range.".into());
    }
    let config = ranges.transform();
    validate_transform(&config).map_err(|e| e.to_string())?;
    // Use the same primary weight and FFT/back-transform as the fitter.
    let kw = ranges.effective_kweights()[0];
    let arrays =
        apply_kweight_transform(&input.0, &input.1, &config, kw).map_err(|e| e.to_string())?;
    let step = config.kstep.unwrap_or_else(|| input.0[1] - input.0[0]);
    let q = (0..arrays.chiq.len())
        .map(|i| i as f64 * step)
        .take_while(|q| *q <= ranges.kmax + 2.)
        .collect();
    Ok(PreviewData {
        input,
        ranges,
        arrays,
        q,
    })
}

pub(crate) struct FitPreviewState {
    key: Option<u64>,
    input_key: Option<u64>,
    cache: BTreeMap<u64, Input>,
    pub data: Option<Arc<PreviewData>>,
    pub k: Option<Entity<RuvizPlot>>,
    pub r: Option<Entity<RuvizPlot>>,
    pub q: Option<Entity<RuvizPlot>>,
    pub view: FitView,
    label: String,
    error: Option<String>,
    loading: bool,
}
impl Default for FitPreviewState {
    fn default() -> Self {
        Self {
            key: None,
            input_key: None,
            cache: Default::default(),
            data: None,
            k: None,
            r: None,
            q: None,
            view: FitView::Both,
            label: String::new(),
            error: None,
            loading: false,
        }
    }
}
impl StudioApp {
    pub(super) fn model_preview_dataset_id(&self) -> Option<usize> {
        if !self.joint.config.enabled || self.stage_view.fit_step != FitStep::Model {
            return None;
        }
        self.joint
            .selected
            .as_ref()
            .map(|(id, _)| *id)
            .filter(|id| self.joint.config.datasets.iter().any(|d| d.id == *id))
            .or_else(|| self.joint.config.datasets.first().map(|d| d.id))
    }
    fn preview_source(&self) -> (PathBuf, String, FitRanges) {
        if let Some(id) = self.model_preview_dataset_id()
            && let Some(d) = self.joint.config.datasets.iter().find(|d| d.id == id)
        {
            return (
                d.file.clone(),
                d.label.clone(),
                d.ranges
                    .as_ref()
                    .unwrap_or(&self.fit_ranges)
                    .resolved(self.joint_params(&d.file).fft_kweight),
            );
        }
        (
            self.current_path.clone(),
            self.current_group_label().to_string(),
            self.fit_ranges
                .resolved(self.joint_params(&self.current_path).fft_kweight),
        )
    }
    pub(super) fn select_fit_space_view(&mut self, space: FitSpaceSpec, cx: &mut Context<Self>) {
        let view = match space {
            FitSpaceSpec::K => FitView::K,
            FitSpaceSpec::R => FitView::R,
            FitSpaceSpec::Q => FitView::Q,
        };
        self.fit_preview.view = view;
        self.stage_view.fit_view = view;
        self.rebuild_fit_preview_plots(cx, true);
        self.rebuild_fit_plots(cx);
        cx.notify();
    }
    fn ensure_fit_preview(&mut self, cx: &mut Context<Self>) {
        let (file, label, ranges) = self.preview_source();
        let params = self.joint_params(&file);
        let mut hash = DefaultHasher::new();
        file.hash(&mut hash);
        params.fingerprint().hash(&mut hash);
        if let Ok(m) = std::fs::metadata(&file) {
            m.len().hash(&mut hash);
            m.modified().ok().hash(&mut hash);
        }
        let input_key = hash.finish();
        serde_json::to_string(&ranges)
            .unwrap_or_default()
            .hash(&mut hash);
        let key = hash.finish();
        if self.fit_preview.key == Some(key) {
            return;
        }
        let changed_file = self.fit_preview.input_key != Some(input_key);
        self.fit_preview.key = Some(key);
        self.fit_preview.input_key = Some(input_key);
        self.fit_preview.label = label;
        self.fit_preview.error = None;
        self.fit_preview.loading = true;
        if changed_file {
            self.fit_preview.data = None;
            self.fit_preview.k = None;
            self.fit_preview.r = None;
            self.fit_preview.q = None;
            self.handles.armed = None;
            self.handles.dragging = None;
        }
        let cached = self.fit_preview.cache.get(&input_key).cloned();
        let timer = cx
            .background_executor()
            .timer(std::time::Duration::from_millis(70));
        cx.spawn(async move |this, cx| {
            timer.await;
            if !this
                .update(cx, |app, _| app.fit_preview.key == Some(key))
                .unwrap_or(false)
            {
                return;
            }
            let job = cx.background_executor().spawn(async move {
                let input = match cached {
                    Some(input) => input,
                    None => {
                        let sp = process_file(&file, &params).map_err(|e| e.to_string())?;
                        Arc::new(
                            sp.get_k()
                                .zip(sp.get_chi())
                                .ok_or("No processed χ(k). Check the background settings.")?,
                        )
                    }
                };
                transform(input, ranges)
            });
            let result = job.await;
            this.update(cx, |app, cx| {
                if app.fit_preview.key != Some(key) {
                    return;
                }
                app.fit_preview.loading = false;
                match result {
                    Ok(data) => {
                        if app.fit_preview.cache.len() >= 8 {
                            app.fit_preview.cache.clear();
                        }
                        app.fit_preview.cache.insert(input_key, data.input.clone());
                        let reset = app
                            .fit_preview
                            .data
                            .as_ref()
                            .is_none_or(|old| old.arrays.kweight != data.arrays.kweight);
                        app.fit_preview.data = Some(Arc::new(data));
                        app.rebuild_fit_preview_plots(cx, reset);
                    }
                    Err(error) => {
                        app.fit_preview.error = Some(error);
                        app.fit_preview.data = None;
                        app.fit_preview.k = None;
                        app.fit_preview.r = None;
                        app.fit_preview.q = None;
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
    pub(crate) fn rebuild_fit_preview_plots(&mut self, cx: &mut Context<Self>, reset: bool) {
        let Some(data) = self.fit_preview.data.clone() else {
            return;
        };
        let t = self.theme;
        let a = &data.arrays;
        let kw = a.kweight;
        let k = data.input.0.as_slice();
        let mut kp: Plot = Plot::new()
            .theme(t.plot_theme())
            .line(k, a.chik.as_slice())
            .color(trace_color(&t, 0))
            .into();
        kp = kp.xlabel("k (Å⁻¹)").ylabel(chik_label(kw));
        let mut rp: Plot = Plot::new()
            .theme(t.plot_theme())
            .line(a.r_space.r.as_slice(), a.r_space.chir_mag.as_slice())
            .color(trace_color(&t, 0))
            .label("|χ(R)|")
            .into();
        for (on, y, label, style) in [
            (
                self.stage_view.fit_show_re,
                &a.r_space.chir_re,
                "Re",
                LineStyle::Dotted,
            ),
            (
                self.stage_view.fit_show_im,
                &a.r_space.chir_im,
                "Im",
                LineStyle::Dashed,
            ),
        ] {
            if on {
                rp = rp
                    .line(a.r_space.r.as_slice(), y.as_slice())
                    .color(trace_color(&t, 0))
                    .line_style(style)
                    .label(label)
                    .into();
            }
        }
        rp = rp
            .xlabel("R (Å)")
            .ylabel(chir_label(kw))
            .xlim(0., (data.ranges.rmax + 2.).clamp(6., 10.));
        let qp: Plot = Plot::new()
            .theme(t.plot_theme())
            .line(&data.q, &a.chiq.as_slice()[..data.q.len()])
            .color(trace_color(&t, 0))
            .into();
        let qp = qp
            .xlabel("q (Å⁻¹)")
            .ylabel(format!("Re χ(q) · k-weight {kw:.0}"));
        for (id, plot, slot) in [
            (PREVIEW_K, kp, &mut self.fit_preview.k),
            (PREVIEW_R, rp, &mut self.fit_preview.r),
            (PREVIEW_Q, qp, &mut self.fit_preview.q),
        ] {
            let (w, h) = self.card_px.get(&id).copied().unwrap_or((420, 300));
            let plot = plot.size_px(w, h);
            match slot {
                Some(entity) => entity.update(cx, |p, cx| {
                    if reset {
                        p.set_plot(plot, cx)
                    } else {
                        p.set_plot_keep_view(plot, cx)
                    }
                }),
                None => *slot = Some(plot_builder(plot).interactive().build(cx)),
            }
        }
    }
    pub(super) fn fit_preview_panel(&mut self, cx: &mut Context<Self>) -> gpui::Div {
        self.ensure_fit_preview(cx);
        let t = self.theme;
        let view = self.fit_preview.view;
        let mut toolbar = div()
            .flex_none()
            .flex()
            .flex_wrap()
            .gap_1()
            .items_center()
            .text_size(px(11.))
            .child("View");
        for (v, label) in [
            (FitView::Both, "k + R"),
            (FitView::K, "k"),
            (FitView::R, "R"),
            (FitView::Q, "q"),
        ] {
            toolbar = toolbar.child(
                chip(&t, format!("preview-view-{label}"), label, v == view).on_click(cx.listener(
                    move |this, _, _, cx| {
                        this.fit_preview.view = v;
                        this.rebuild_fit_preview_plots(cx, true);
                        cx.notify();
                    },
                )),
            );
        }
        if matches!(view, FitView::Both | FitView::R) {
            for (im, label, on) in [
                (false, "Re", self.stage_view.fit_show_re),
                (true, "Im", self.stage_view.fit_show_im),
            ] {
                toolbar = toolbar.child(
                    chip(&t, format!("preview-component-{label}"), label, on).on_click(
                        cx.listener(move |this, _, _, cx| {
                            if im {
                                this.stage_view.fit_show_im = !this.stage_view.fit_show_im;
                            } else {
                                this.stage_view.fit_show_re = !this.stage_view.fit_show_re;
                            }
                            this.rebuild_fit_preview_plots(cx, true);
                            cx.notify();
                        }),
                    ),
                );
            }
        }
        let mut panel = div()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .gap_2()
            .px_2()
            .pb_2()
            .child(
                div()
                    .flex_none()
                    .text_size(px(12.))
                    .child(format!("Data · {}", self.fit_preview.label)),
            )
            .child(toolbar);
        if let Some(e) = &self.fit_preview.error {
            return panel.child(div().text_color(t.warn).text_size(px(11.)).child(e.clone()));
        }
        if self.fit_preview.data.is_none() {
            return panel.child(div().text_color(t.text_muted).child("Loading spectrum…"));
        }
        let kw = self.fit_preview.data.as_ref().unwrap().arrays.kweight;
        let entries = match view {
            FitView::Both => vec![
                (PREVIEW_K, chik_label(kw), self.fit_preview.k.clone()),
                (PREVIEW_R, chir_label(kw), self.fit_preview.r.clone()),
            ],
            FitView::K => vec![(PREVIEW_K, chik_label(kw), self.fit_preview.k.clone())],
            FitView::R => vec![(PREVIEW_R, chir_label(kw), self.fit_preview.r.clone())],
            FitView::Q => vec![(PREVIEW_Q, "Re χ(q)".into(), self.fit_preview.q.clone())],
        };
        for (id, title, plot) in entries {
            if let Some(plot) = plot {
                panel = panel.child(self.fit_column(id, title.into(), plot, None, cx));
            }
        }
        panel
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn live_preview_uses_fit_windows_and_individual_weights() {
        let k = DVector::from_iterator(301, (0..301).map(|i| i as f64 * 0.05));
        let chi = k.map(|k| (-0.02 * k * k).exp() * (5. * k).sin());
        let input = Arc::new((k, chi));
        let ranges = FitRanges {
            kmin: 2.,
            kmax: 12.,
            rmin: 1.,
            rmax: 3.,
            ..Default::default()
        }
        .resolved(Some(1.));
        let a = transform(input.clone(), ranges.clone()).unwrap();
        let b = transform(input.clone(), ranges.resolved(Some(3.))).unwrap();
        assert!((b.arrays.chik[100] / a.arrays.chik[100] - 25.).abs() < 1e-10);
        let narrowed = transform(
            input.clone(),
            FitRanges {
                kmax: 8.,
                ..ranges.clone()
            },
        )
        .unwrap();
        assert!((&a.arrays.r_space.chir_mag - &narrowed.arrays.r_space.chir_mag).norm() > 0.1);
        let back = transform(
            input.clone(),
            FitRanges {
                rmin: 3.,
                rmax: 5.,
                fitspace: FitSpaceSpec::Q,
                ..ranges.clone()
            },
        )
        .unwrap();
        assert!((&a.arrays.chiq - &back.arrays.chiq).norm() > 0.1);
        assert_eq!(a.q[1], ranges.transform().kstep.unwrap());
        assert!(
            transform(
                input,
                FitRanges {
                    kmin: 13.,
                    ..ranges
                }
            )
            .is_err()
        );
    }
}
