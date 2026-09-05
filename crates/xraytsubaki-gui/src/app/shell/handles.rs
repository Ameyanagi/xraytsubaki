//! Direct manipulation on plots: range parameters are shaded regions with
//! draggable edges, E₀ / Rbkg are draggable lines.
//!
//! The geometry is painted by GPUI ([`StudioApp::handle_layer`], a canvas
//! layered over the plot card) from the plot's current data→pixel mapping
//! (`RuvizPlot::screen_at`), so a drag or hover frame only re-paints a
//! handful of quads — the ruviz frame is untouched until the debounced
//! recompute rebuilds the data plot. Pointer handling lives in GPUI on the
//! plot card: while a handle is armed a transparent overlay sits over the
//! plot so the press never reaches ruviz's pan.

use gpui::{
    Bounds, Context, Corners, Hsla, IntoElement, ParentElement, Pixels, Point, Rgba, Styled,
    canvas, div, fill, point, prelude::*, px, quad, size,
};
use xraytsubaki::prelude::{BackgroundMethod, NormalizationMethod};

use gpui::Entity;
use ruviz::core::plot::ViewportPoint;
use ruviz_gpui::RuvizPlot;

use super::Stage;
use super::center::{PLOT_CHIK, PLOT_CHIR, PLOT_MU, PLOT_NORM};
use super::fit_preview::{PREVIEW_K, PREVIEW_Q, PREVIEW_R};
use crate::app::{ParamKey, StudioApp};

/// Virtual plot indices for the Fit stage plots (not quadrants).
pub const PLOT_FIT_K: usize = 100;
pub const PLOT_FIT_R: usize = 101;
/// Residual strips and the q-space card (measured for sizing only).
pub const PLOT_FIT_K_RES: usize = 102;
pub const PLOT_FIT_R_RES: usize = 103;
pub const PLOT_FIT_Q: usize = 104;

/// Pixel distance within which a handle arms.
const ARM_PX: f64 = 7.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HandleKey {
    E0,
    PreStart,
    PreEnd,
    NormStart,
    NormEnd,
    FftKmin,
    FftKmax,
    Rbkg,
    FitKmin,
    FitKmax,
    FitRmin,
    FitRmax,
}

impl HandleKey {
    /// Pipeline parameter the handle edits (`None` for fit ranges).
    fn param(self) -> Option<ParamKey> {
        Some(match self {
            HandleKey::E0 => ParamKey::E0,
            HandleKey::PreStart => ParamKey::PreEdgeStart,
            HandleKey::PreEnd => ParamKey::PreEdgeEnd,
            HandleKey::NormStart => ParamKey::NormStart,
            HandleKey::NormEnd => ParamKey::NormEnd,
            HandleKey::FftKmin => ParamKey::FftKmin,
            HandleKey::FftKmax => ParamKey::FftKmax,
            HandleKey::Rbkg => ParamKey::Rbkg,
            HandleKey::FitKmin | HandleKey::FitKmax | HandleKey::FitRmin | HandleKey::FitRmax => {
                return None;
            }
        })
    }

    /// Values are stored relative to E₀ for the normalization ranges.
    fn relative_to_e0(self) -> bool {
        matches!(
            self,
            HandleKey::PreStart | HandleKey::PreEnd | HandleKey::NormStart | HandleKey::NormEnd
        )
    }

    fn rounding(self) -> f64 {
        match self {
            HandleKey::E0 => 0.1,
            HandleKey::PreStart | HandleKey::PreEnd | HandleKey::NormStart | HandleKey::NormEnd => {
                1.0
            }
            HandleKey::FftKmin | HandleKey::FftKmax => 0.1,
            HandleKey::Rbkg => 0.05,
            HandleKey::FitKmin | HandleKey::FitKmax => 0.1,
            HandleKey::FitRmin | HandleKey::FitRmax => 0.05,
        }
    }

    /// Text shown next to the handle while it is dragged.
    fn readout(self, x: f64) -> String {
        match self {
            HandleKey::E0 => format!("E₀ {x:.1} eV"),
            HandleKey::PreStart | HandleKey::PreEnd | HandleKey::NormStart | HandleKey::NormEnd => {
                format!("{x:.0} eV")
            }
            HandleKey::FftKmin | HandleKey::FftKmax | HandleKey::FitKmin | HandleKey::FitKmax => {
                format!("k {x:.1} Å⁻¹")
            }
            HandleKey::Rbkg => format!("Rbkg {x:.2} Å"),
            HandleKey::FitRmin | HandleKey::FitRmax => format!("R {x:.2} Å"),
        }
    }

    fn accent(self) -> bool {
        !matches!(self, HandleKey::NormStart | HandleKey::NormEnd)
    }
}

/// A shaded region between two handles (or from a fixed edge).
#[derive(Clone, Copy, Debug)]
struct Span {
    lo: Option<HandleKey>,
    hi: Option<HandleKey>,
    fixed_lo: f64,
    accent: bool,
}

#[derive(Default)]
pub struct HandleState {
    /// (plot index, handle) under the pointer.
    pub armed: Option<(usize, HandleKey)>,
    pub dragging: Option<(usize, HandleKey)>,
    /// Last resolved plot area per plot. While a live-follow refresh swaps
    /// the ruviz session, the viewport snapshot is briefly unavailable; the
    /// cached area keeps the handles painted and the drag mapped (the view
    /// is preserved across refreshes, so the mapping is unchanged).
    areas: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<usize, PlotArea>>>,
}

/// What the handle layer paints for one plot, resolved at render time.
struct HandleDecor {
    /// (data x0, data x1, colour)
    spans: Vec<(f64, f64, Rgba)>,
    /// (data x, colour, hot, dashed)
    lines: Vec<(f64, Rgba, bool, bool)>,
}

impl StudioApp {
    /// Handles the stage exposes on a given plot, with their current data x.
    fn handle_specs(&self, plot: usize) -> (Vec<(HandleKey, f64)>, Vec<Span>) {
        if self.stage == Stage::Fit {
            let r = self
                .joint_plotted_dataset_id()
                .and_then(|id| self.joint.config.datasets.iter().find(|d| d.id == id))
                .and_then(|d| d.ranges.as_ref())
                .unwrap_or(&self.fit_ranges);
            return match plot {
                PLOT_FIT_K | PREVIEW_K | PREVIEW_Q => (
                    vec![(HandleKey::FitKmin, r.kmin), (HandleKey::FitKmax, r.kmax)],
                    vec![Span {
                        lo: Some(HandleKey::FitKmin),
                        hi: Some(HandleKey::FitKmax),
                        fixed_lo: 0.0,
                        accent: true,
                    }],
                ),
                PLOT_FIT_R | PREVIEW_R => (
                    vec![(HandleKey::FitRmin, r.rmin), (HandleKey::FitRmax, r.rmax)],
                    vec![Span {
                        lo: Some(HandleKey::FitRmin),
                        hi: Some(HandleKey::FitRmax),
                        fixed_lo: 0.0,
                        accent: true,
                    }],
                ),
                _ => (Vec::new(), Vec::new()),
            };
        }
        let Some(sp) = self.spectrum.as_deref() else {
            return (Vec::new(), Vec::new());
        };
        let p = self.ui_params();
        match (self.stage, plot) {
            (Stage::Normalize, PLOT_MU | PLOT_NORM) => {
                let Some(e0) = p.e0.or_else(|| sp.get_e0()) else {
                    return (Vec::new(), Vec::new());
                };
                let ppe = match sp.normalization.as_ref() {
                    Some(NormalizationMethod::PrePostEdge(ppe)) => Some(ppe),
                    _ => None,
                };
                let rel = |param: Option<f64>, from_sp: Option<f64>, default: f64| {
                    param.or(from_sp).unwrap_or(default)
                };
                let pre1 = rel(
                    p.pre_edge_start,
                    ppe.and_then(|x| x.get_pre_edge_start()),
                    -200.0,
                );
                let pre2 = rel(
                    p.pre_edge_end,
                    ppe.and_then(|x| x.get_pre_edge_end()),
                    -30.0,
                );
                let nor1 = rel(p.norm_start, ppe.and_then(|x| x.get_norm_start()), 150.0);
                let emax = sp
                    .energy
                    .as_ref()
                    .and_then(|e| e.iter().next_back().copied())
                    .unwrap_or(e0 + 2000.0);
                let nor2 =
                    rel(p.norm_end, ppe.and_then(|x| x.get_norm_end()), 2000.0).min(emax - e0);
                (
                    vec![
                        (HandleKey::PreStart, e0 + pre1),
                        (HandleKey::PreEnd, e0 + pre2),
                        (HandleKey::NormStart, e0 + nor1),
                        (HandleKey::NormEnd, e0 + nor2),
                        (HandleKey::E0, e0),
                    ],
                    vec![
                        Span {
                            lo: Some(HandleKey::PreStart),
                            hi: Some(HandleKey::PreEnd),
                            fixed_lo: 0.0,
                            accent: true,
                        },
                        Span {
                            lo: Some(HandleKey::NormStart),
                            hi: Some(HandleKey::NormEnd),
                            fixed_lo: 0.0,
                            accent: false,
                        },
                    ],
                )
            }
            (Stage::Transform, PLOT_CHIK) => {
                let (kmin, kmax, _, _) = self.fft_summary();
                (
                    vec![(HandleKey::FftKmin, kmin), (HandleKey::FftKmax, kmax)],
                    vec![Span {
                        lo: Some(HandleKey::FftKmin),
                        hi: Some(HandleKey::FftKmax),
                        fixed_lo: 0.0,
                        accent: true,
                    }],
                )
            }
            (Stage::Background, PLOT_CHIR) => {
                let rbkg = p
                    .rbkg
                    .or(match sp.background.as_ref() {
                        Some(BackgroundMethod::AUTOBK(a)) => a.rbkg,
                        _ => None,
                    })
                    .unwrap_or(1.0);
                (
                    vec![(HandleKey::Rbkg, rbkg)],
                    vec![Span {
                        lo: None,
                        hi: Some(HandleKey::Rbkg),
                        fixed_lo: 0.0,
                        accent: true,
                    }],
                )
            }
            _ => (Vec::new(), Vec::new()),
        }
    }

    fn handle_color(&self, accent: bool) -> Rgba {
        if accent {
            self.theme.accent
        } else {
            self.theme.warn
        }
    }

    /// Spans + handle lines for plot `plot`, in data coordinates.
    fn handle_decor(&self, plot: usize) -> Option<HandleDecor> {
        if !self.stage.is_processing() && self.stage != Stage::Fit {
            return None;
        }
        let (specs, spans) = self.handle_specs(plot);
        if specs.is_empty() {
            return None;
        }
        let at = |k: HandleKey| specs.iter().find(|(kk, _)| *kk == k).map(|(_, x)| *x);
        let spans = spans
            .iter()
            .map(|span| {
                let x0 = span.lo.and_then(at).unwrap_or(span.fixed_lo);
                let x1 = span.hi.and_then(at).unwrap_or(x0);
                (x0.min(x1), x0.max(x1), self.handle_color(span.accent))
            })
            .collect();
        let lines = specs
            .iter()
            .map(|&(key, x)| {
                let hot = self.handles.armed == Some((plot, key))
                    || self.handles.dragging == Some((plot, key));
                (
                    x,
                    self.handle_color(key.accent()),
                    hot,
                    key != HandleKey::E0,
                )
            })
            .collect();
        Some(HandleDecor { spans, lines })
    }

    /// Stage / group changed: drop any armed handle.
    pub(crate) fn sync_handles(&mut self, _cx: &mut Context<Self>) {
        if self.handles.dragging.is_none() {
            self.handles.armed = None;
        }
    }

    /// The interactive plot behind a handle plot index.
    pub(crate) fn plot_entity(&self, plot: usize) -> Option<Entity<RuvizPlot>> {
        match plot {
            PREVIEW_K => self.fit_preview.k.clone(),
            PREVIEW_R => self.fit_preview.r.clone(),
            PREVIEW_Q => self.fit_preview.q.clone(),
            PLOT_FIT_K => self.fit_plots.as_ref().map(|p| p.k.clone()),
            PLOT_FIT_R => self.fit_plots.as_ref().map(|p| p.r.clone()),
            _ => self.quadrants.get(plot).map(|(_, e)| e.clone()),
        }
    }

    /// GPUI-painted spans and handle lines for plot `plot`, positioned from
    /// the plot's current data→pixel mapping at paint time (so they follow
    /// pan/zoom and resizes without touching the ruviz frame).
    pub(crate) fn handle_layer(
        &self,
        plot: usize,
        _cx: &Context<Self>,
    ) -> Option<impl IntoElement + use<>> {
        let decor = self.handle_decor(plot)?;
        let entity = self.plot_entity(plot)?;
        let areas = self.handles.areas.clone();
        // Value readout for the handle being dragged on this plot.
        let label: Option<(f64, String)> = match self.handles.dragging {
            Some((p, key)) if p == plot => {
                let (specs, _) = self.handle_specs(plot);
                specs
                    .iter()
                    .find(|(k, _)| *k == key)
                    .map(|&(k, x)| (x, k.readout(x)))
            }
            _ => None,
        };
        let label_ink: Hsla = self.theme.text.into();
        let label_bg: Hsla = {
            let mut c: Hsla = self.theme.raised.into();
            c.a = 0.94;
            c
        };
        let label_border: Hsla = self.theme.border.into();
        Some(
            canvas(
                move |bounds, _window, cx| {
                    crate::debug_stats::painted();
                    let rp = entity.read(cx);
                    let area = match plot_area_window_bounds(rp) {
                        Some(area) => {
                            if let Ok(mut cache) = areas.lock() {
                                cache.insert(plot, area);
                            }
                            area
                        }
                        None => *areas.lock().ok()?.get(&plot)?,
                    };
                    let to_x = |x: f64| Some(area.px_of(x));
                    let spans = decor
                        .spans
                        .iter()
                        .filter_map(|&(x0, x1, color)| {
                            // Clamp to the plot area; a span entirely off-screen
                            // maps to None on both ends and is skipped.
                            let (vx0, vx1) = (area.data_x0, area.data_x1);
                            if x1 < vx0 || x0 > vx1 {
                                return None;
                            }
                            let px0 = to_x(x0.max(vx0))?;
                            let px1 = to_x(x1.min(vx1))?;
                            Some((px0, px1, color))
                        })
                        .collect::<Vec<_>>();
                    // Lines and tabs only inside the visible plot area; the
                    // canvas is not clipped by GPUI, so an off-screen handle
                    // would otherwise paint over neighbouring panels.
                    let inside = |px: f32| px >= area.left - 0.5 && px <= area.right + 0.5;
                    let lines = decor
                        .lines
                        .iter()
                        .filter_map(|&(x, color, hot, dashed)| {
                            let px = to_x(x)?;
                            inside(px).then_some((px, color, hot, dashed))
                        })
                        .collect::<Vec<_>>();
                    let label = label.as_ref().and_then(|(x, text)| {
                        let px = to_x(*x)?;
                        inside(px).then(|| (px, text.clone()))
                    });
                    Some((area, spans, lines, label, f32::from(bounds.origin.y)))
                },
                move |_bounds, geom, window, cx| {
                    let Some((area, spans, lines, label, card_top)) = geom else {
                        return;
                    };
                    let top = px(area.top);
                    let bottom = px(area.bottom);
                    let height = bottom - top;
                    for (x0, x1, color) in spans {
                        let mut c: Hsla = color.into();
                        c.a = 0.13;
                        let b = Bounds::new(point(px(x0), top), size(px(x1 - x0), height));
                        window.paint_quad(fill(b, c));
                    }
                    for (x, color, hot, dashed) in lines {
                        let c: Hsla = color.into();
                        let w = if hot { 2.6 } else { 1.3 };
                        let lx = px(x - w / 2.0);
                        if dashed && !hot {
                            let (on, off) = (6.0_f32, 4.0_f32);
                            let mut y = area.top;
                            while y < area.bottom {
                                let seg = on.min(area.bottom - y);
                                let b = Bounds::new(point(lx, px(y)), size(px(w), px(seg)));
                                window.paint_quad(fill(b, c));
                                y += on + off;
                            }
                        } else {
                            let b = Bounds::new(point(lx, top), size(px(w), height));
                            window.paint_quad(fill(b, c));
                        }
                        // Grab tab just above the top axis line (never over
                        // the tick labels inside the plot), wider when hot.
                        let (tw, th) = if hot { (11.0, 14.0) } else { (7.0, 11.0) };
                        let tab_y = (area.top - th - 1.0).max(card_top + 1.0);
                        let tab =
                            Bounds::new(point(px(x - tw / 2.0), px(tab_y)), size(px(tw), px(th)));
                        window.paint_quad(quad(
                            tab,
                            Corners::all(px(3.0)),
                            c,
                            gpui::Edges::all(px(1.0)),
                            Hsla {
                                a: 0.9,
                                ..gpui::black()
                            },
                            gpui::BorderStyle::Solid,
                        ));
                    }
                    // Live value next to the handle while dragging.
                    if let Some((x, text)) = label {
                        let font_size = px(11.0);
                        let mut font = window.text_style().font();
                        font.family = "IBM Plex Mono".into();
                        let run = gpui::TextRun {
                            len: text.len(),
                            font,
                            color: label_ink,
                            background_color: None,
                            underline: None,
                            strikethrough: None,
                        };
                        let text: gpui::SharedString = text.into();
                        let line = window
                            .text_system()
                            .shape_line(text, font_size, &[run], None);
                        let w = f32::from(line.width) + 10.0;
                        let h = 18.0;
                        let mut lx = x + 8.0;
                        if lx + w > area.right {
                            lx = x - 8.0 - w;
                        }
                        let ly = area.top + 8.0;
                        let bg = Bounds::new(point(px(lx), px(ly)), size(px(w), px(h)));
                        window.paint_quad(quad(
                            bg,
                            Corners::all(px(4.0)),
                            label_bg,
                            gpui::Edges::all(px(1.0)),
                            label_border,
                            gpui::BorderStyle::Solid,
                        ));
                        let _ = line.paint(
                            point(px(lx + 5.0), px(ly + 2.0)),
                            px(14.0),
                            gpui::TextAlign::Left,
                            None,
                            window,
                            cx,
                        );
                    }
                },
            )
            .absolute()
            .inset_0(),
        )
    }

    pub(crate) fn plot_pointer_move(
        &mut self,
        plot: usize,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        crate::debug_stats::pointer_event();
        let Some(entity) = self.plot_entity(plot) else {
            return;
        };
        if let Some((drag_plot, key)) = self.handles.dragging {
            if drag_plot != plot {
                return;
            }
            let data = entity.read(cx).data_at(position).ok().flatten();
            let x = match data {
                Some(pt) => Some(pt.x),
                None => self
                    .handles
                    .areas
                    .lock()
                    .ok()
                    .and_then(|c| c.get(&plot).map(|a| a.data_of(f32::from(position.x)))),
            };
            if let Some(x) = x {
                self.apply_handle_drag(key, x, cx);
            }
            return;
        }
        // Arming: the pixel threshold is converted to data units through
        // two `data_at` probes (`screen_at` needs a y inside the view).
        let debug = std::env::var_os("XTS_DEBUG_HANDLES").is_some();
        let plot_entity = entity.read(cx);
        let here = plot_entity.data_at(position).ok().flatten();
        let there = plot_entity
            .data_at(gpui::point(position.x + px(ARM_PX as f32), position.y))
            .ok()
            .flatten();
        let (specs, _) = self.handle_specs(plot);
        let visible = self
            .handles
            .areas
            .lock()
            .ok()
            .and_then(|c| c.get(&plot).copied());
        let mut nearest: Option<(HandleKey, f64)> = None;
        if let (Some(here), Some(there)) = (here, there) {
            let tol = (there.x - here.x).abs().max(1e-9);
            for (key, x) in specs {
                // Off-screen handles cannot be armed.
                if let Some(area) = visible
                    && (x < area.data_x0 || x > area.data_x1)
                {
                    continue;
                }
                let d = (x - here.x).abs();
                if d <= tol && nearest.is_none_or(|(_, best)| d < best) {
                    nearest = Some((key, d));
                }
            }
            if debug {
                eprintln!(
                    "[handles] plot {plot} pointer x={:.2} tol={tol:.3} nearest={nearest:?}",
                    here.x
                );
            }
        } else if debug {
            eprintln!("[handles] plot {plot} pointer outside plot area ({position:?})");
        }
        let armed = nearest.map(|(key, _)| (plot, key));
        if armed != self.handles.armed {
            self.handles.armed = armed;
            cx.notify();
        }
    }

    fn apply_handle_drag(&mut self, key: HandleKey, x: f64, cx: &mut Context<Self>) {
        let e0 = self
            .ui_params()
            .e0
            .or_else(|| self.spectrum.as_ref().and_then(|s| s.get_e0()))
            .unwrap_or(0.0);
        // Never drag a handle past the data: a range outside the spectrum
        // (or E₀ off the edge) only produces a degenerate normalization.
        let x = match self.handle_domain(key) {
            Some((lo, hi)) if lo < hi => x.clamp(lo, hi),
            _ => x,
        };
        let step = key.rounding();
        let value = if key.relative_to_e0() {
            ((x - e0) / step).round() * step
        } else {
            (x / step).round() * step
        };
        // Snap away the binary noise of `round() * step` (22130.1000000002)
        // so fields and the journal show the intended decimal.
        let value = (value * 1e6).round() / 1e6;
        let value = match key {
            HandleKey::Rbkg | HandleKey::FftKmin | HandleKey::FitKmin | HandleKey::FitRmin => {
                value.max(0.0)
            }
            _ => value,
        };
        let Some(param) = key.param() else {
            // Fit ranges: model state, not pipeline params.
            let id = self.joint_plotted_dataset_id();
            let r = if let Some(d) = self
                .joint
                .config
                .datasets
                .iter_mut()
                .find(|d| Some(d.id) == id)
            {
                d.ranges.get_or_insert_with(|| self.fit_ranges.clone())
            } else {
                &mut self.fit_ranges
            };
            let slot = match key {
                HandleKey::FitKmin => &mut r.kmin,
                HandleKey::FitKmax => &mut r.kmax,
                HandleKey::FitRmin => &mut r.rmin,
                HandleKey::FitRmax => &mut r.rmax,
                _ => return,
            };
            if *slot == value {
                return;
            }
            *slot = value;
            self.sync_range_fields(cx);
            self.fit_model_changed(cx);
            cx.notify();
            return;
        };
        let current = crate::app::param_field_value(param, self.ui_params());
        if current == Some(value) {
            return;
        }
        // Params first (debounced recompute); the handle layer re-paints
        // from the new value on this frame, the data plot follows when the
        // recompute lands.
        self.apply_param(param, Some(value), cx);
        self.sync_param_fields(cx);
        cx.notify();
    }

    /// Absolute x-range a handle may take: the spectrum's energy range for
    /// energy handles, `0..k_max` / `0..R_max` for the k- and R-space ones.
    fn handle_domain(&self, key: HandleKey) -> Option<(f64, f64)> {
        if self.stage_view.fit_step == super::fit_workspace::FitStep::Model
            && let Some(data) = &self.fit_preview.data
        {
            match key {
                HandleKey::FitKmin | HandleKey::FitKmax => {
                    return data.input.0.iter().next_back().map(|v| (0., *v));
                }
                HandleKey::FitRmin | HandleKey::FitRmax => {
                    return data.arrays.r_space.r.iter().next_back().map(|v| (0., *v));
                }
                _ => (),
            }
        }
        if self.joint_plotted_dataset_id().is_some()
            && let Some(d) = self
                .fit_result
                .as_ref()
                .and_then(|r| r.datasets.get(self.joint.result_index))
        {
            match key {
                HandleKey::FitKmin | HandleKey::FitKmax => {
                    return d.k.iter().next_back().map(|v| (0., *v));
                }
                HandleKey::FitRmin | HandleKey::FitRmax => {
                    return d.r.iter().next_back().map(|v| (0., *v));
                }
                _ => (),
            }
        }
        let sp = self.spectrum.as_ref()?;
        let last = |v: &nalgebra::DVector<f64>| v.iter().next_back().copied();
        match key {
            HandleKey::E0
            | HandleKey::PreStart
            | HandleKey::PreEnd
            | HandleKey::NormStart
            | HandleKey::NormEnd => {
                let e = sp.energy.as_ref()?;
                let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
                for v in e.iter() {
                    lo = lo.min(*v);
                    hi = hi.max(*v);
                }
                // E₀ is a fine-tuning handle: keep it within ±50 eV of the
                // derivative maximum (max-derivative, half-step and
                // white-line conventions all live there). Further out the
                // pre-/post-edge fits straddle the edge and the edge step
                // collapses, which only produces a degenerate normalization.
                if key == HandleKey::E0 {
                    let mu = sp.mu.as_ref()?;
                    let n = e.len().min(mu.len());
                    let mut best = (0usize, f64::NEG_INFINITY);
                    for i in 1..n.saturating_sub(1) {
                        let de = e[i + 1] - e[i - 1];
                        if de <= 0.0 {
                            continue;
                        }
                        let d = (mu[i + 1] - mu[i - 1]) / de;
                        if d > best.1 {
                            best = (i, d);
                        }
                    }
                    let auto = if best.1.is_finite() {
                        e[best.0]
                    } else {
                        (lo + hi) / 2.0
                    };
                    let pad = ((hi - lo) * 0.02).max(1.0);
                    Some(((auto - 50.0).max(lo + pad), (auto + 50.0).min(hi - pad)))
                } else {
                    Some((lo, hi))
                }
            }
            HandleKey::FftKmin | HandleKey::FftKmax | HandleKey::FitKmin | HandleKey::FitKmax => {
                Some((0.0, sp.get_k().as_ref().and_then(last)?))
            }
            HandleKey::Rbkg | HandleKey::FitRmin | HandleKey::FitRmax => {
                Some((0.0, sp.get_r().as_ref().and_then(last)?))
            }
        }
    }

    /// Transparent capture layer shown while a handle is armed or dragging.
    pub(crate) fn handle_overlay(
        &mut self,
        plot: usize,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement + use<>> {
        let active = matches!(self.handles.armed, Some((p, _)) if p == plot)
            || matches!(self.handles.dragging, Some((p, _)) if p == plot);
        if !active {
            return None;
        }
        Some(
            div()
                .id(("handle-overlay", plot))
                .absolute()
                .inset_0()
                .cursor(gpui::CursorStyle::ResizeLeftRight)
                .on_mouse_down(
                    gpui::MouseButton::Left,
                    cx.listener(move |this, _: &gpui::MouseDownEvent, _w, cx| {
                        // The press must not reach ruviz's pan underneath.
                        cx.stop_propagation();
                        if let Some((p, key)) = this.handles.armed
                            && p == plot
                        {
                            this.handles.dragging = Some((p, key));
                            cx.notify();
                        }
                    }),
                )
                .on_mouse_up(
                    gpui::MouseButton::Left,
                    cx.listener(move |this, _: &gpui::MouseUpEvent, _w, cx| {
                        cx.stop_propagation();
                        this.end_handle_drag(cx);
                    }),
                )
                .on_mouse_up_out(
                    gpui::MouseButton::Left,
                    cx.listener(move |this, _: &gpui::MouseUpEvent, _w, cx| {
                        this.end_handle_drag(cx);
                    }),
                )
                .child(div()),
        )
    }

    fn end_handle_drag(&mut self, cx: &mut Context<Self>) {
        if let Some((_, key)) = self.handles.dragging.take() {
            self.handles.armed = None;
            // Trailing run: the final pointer position may have landed
            // between throttle ticks (a cache hit when it did not).
            if key.param().is_some() {
                self.schedule_recompute(cx);
            }
            cx.notify();
        }
    }
}

/// The plot's core area in window pixels plus its visible data x-range.
#[derive(Clone, Copy)]
struct PlotArea {
    top: f32,
    bottom: f32,
    left: f32,
    right: f32,
    data_x0: f64,
    data_x1: f64,
}

impl PlotArea {
    /// Data x → window px (the axes are linear).
    fn px_of(&self, x: f64) -> f32 {
        let span = (self.data_x1 - self.data_x0).abs().max(1e-12);
        self.left + ((x - self.data_x0) / span) as f32 * (self.right - self.left)
    }

    /// Window px → data x.
    fn data_of(&self, px: f32) -> f64 {
        let w = (self.right - self.left).max(1e-6);
        self.data_x0 + f64::from((px - self.left) / w) * (self.data_x1 - self.data_x0)
    }
}

/// Derive the plot area from the displayed viewport: `screen_at` on the
/// visible-bounds corners (nudged inwards so they count as inside).
fn plot_area_window_bounds(rp: &RuvizPlot) -> Option<PlotArea> {
    let snap = rp.interactive_session().viewport_snapshot().ok()?;
    let vb = snap.visible_bounds;
    let dx = (vb.max.x - vb.min.x).abs().max(1e-12) * 1e-6;
    let dy = (vb.max.y - vb.min.y).abs().max(1e-12) * 1e-6;
    let (x0, x1) = (vb.min.x.min(vb.max.x) + dx, vb.max.x.max(vb.min.x) - dx);
    let (y0, y1) = (vb.min.y.min(vb.max.y) + dy, vb.max.y.max(vb.min.y) - dy);
    let a = rp.screen_at(ViewportPoint { x: x0, y: y0 }).ok()??;
    let b = rp.screen_at(ViewportPoint { x: x1, y: y1 }).ok()??;
    let (ay, by) = (f32::from(a.y), f32::from(b.y));
    let (ax, bx) = (f32::from(a.x), f32::from(b.x));
    Some(PlotArea {
        top: ay.min(by),
        bottom: ay.max(by),
        left: ax.min(bx),
        right: ax.max(bx),
        data_x0: x0,
        data_x1: x1,
    })
}
