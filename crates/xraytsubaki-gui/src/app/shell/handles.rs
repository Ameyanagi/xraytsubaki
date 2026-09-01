//! Direct manipulation on plots: range parameters are shaded regions with
//! draggable edges, E₀ / Rbkg are draggable lines.
//!
//! The geometry is baked into the base plot as static annotations (ruviz
//! `Plot::annotate`), and a drag rebuilds the plot with `set_plot_keep_view`
//! from the cached spectrum — cheap, and it goes through the same renderer
//! as the data so it can never disagree with the axes. (ruviz-gpui 0.12's
//! session-annotation overlay composites with a stride mismatch on this
//! window size, which shears the overlay diagonally; see the M1 report.)
//! Pointer handling lives in GPUI on the plot card: while a handle is armed
//! a transparent overlay sits over the plot so the press never reaches
//! ruviz's pan.

use gpui::{Context, IntoElement, ParentElement, Pixels, Point, Styled, div, prelude::*, px};
use ruviz::core::Annotation;
use ruviz::render::{Color as PlotColor, LineStyle};
use xraytsubaki::prelude::{BackgroundMethod, NormalizationMethod};

use super::Stage;
use super::center::{PLOT_CHIK, PLOT_CHIR, PLOT_MU, PLOT_NORM};
use crate::app::{ParamKey, StudioApp};

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
}

impl HandleKey {
    fn param(self) -> ParamKey {
        match self {
            HandleKey::E0 => ParamKey::E0,
            HandleKey::PreStart => ParamKey::PreEdgeStart,
            HandleKey::PreEnd => ParamKey::PreEdgeEnd,
            HandleKey::NormStart => ParamKey::NormStart,
            HandleKey::NormEnd => ParamKey::NormEnd,
            HandleKey::FftKmin => ParamKey::FftKmin,
            HandleKey::FftKmax => ParamKey::FftKmax,
            HandleKey::Rbkg => ParamKey::Rbkg,
        }
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
}

impl StudioApp {
    /// Handles the stage exposes on a given plot, with their current data x.
    fn handle_specs(&self, plot: usize) -> (Vec<(HandleKey, f64)>, Vec<Span>) {
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

    fn handle_color(&self, accent: bool) -> PlotColor {
        let c = if accent {
            self.theme.accent
        } else {
            self.theme.warn
        };
        PlotColor::from_rgb(
            (c.r * 255.0) as u8,
            (c.g * 255.0) as u8,
            (c.b * 255.0) as u8,
        )
    }

    /// Static annotations (spans + handle lines) to bake into plot `plot`.
    pub(crate) fn handle_decor(&self, plot: usize) -> Vec<Annotation> {
        if !self.stage.is_processing() {
            return Vec::new();
        }
        let (specs, spans) = self.handle_specs(plot);
        let mut out = Vec::new();
        let at = |k: HandleKey| specs.iter().find(|(kk, _)| *kk == k).map(|(_, x)| *x);
        for span in spans {
            let x0 = span.lo.and_then(at).unwrap_or(span.fixed_lo);
            let x1 = span.hi.and_then(at).unwrap_or(x0);
            out.push(Annotation::HSpan {
                x_min: x0.min(x1),
                x_max: x0.max(x1),
                style: ruviz::core::ShapeStyle {
                    fill_color: Some(self.handle_color(span.accent)),
                    fill_alpha: 0.13,
                    edge_color: None,
                    edge_width: 0.0,
                    edge_style: LineStyle::Solid,
                },
            });
        }
        for (key, x) in specs {
            let hot = self.handles.armed == Some((plot, key))
                || self.handles.dragging == Some((plot, key));
            out.push(Annotation::VLine {
                x,
                style: if key == HandleKey::E0 {
                    LineStyle::Solid
                } else {
                    LineStyle::Dashed
                },
                color: self.handle_color(key.accent()),
                width: if hot { 2.6 } else { 1.3 },
            });
        }
        out
    }

    /// Stage / group changed: drop any armed handle (the geometry itself is
    /// part of the plots and follows the rebuild).
    pub(crate) fn sync_handles(&mut self, _cx: &mut Context<Self>) {
        if self.handles.dragging.is_none() {
            self.handles.armed = None;
        }
    }

    /// Pointer moved over plot `plot` (window coordinates). Arms the nearest
    /// handle, or drags the active one.
    pub(crate) fn plot_pointer_move(
        &mut self,
        plot: usize,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let Some((_, entity)) = self.quadrants.get(plot) else {
            return;
        };
        let entity = entity.clone();
        if let Some((drag_plot, key)) = self.handles.dragging {
            if drag_plot != plot {
                return;
            }
            let data = entity.read(cx).data_at(position).ok().flatten();
            if let Some(pt) = data {
                self.apply_handle_drag(key, pt.x, cx);
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
        let mut nearest: Option<(HandleKey, f64)> = None;
        if let (Some(here), Some(there)) = (here, there) {
            let tol = (there.x - here.x).abs().max(1e-9);
            for (key, x) in specs {
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
            // Hot/cold line width lives in the baked plot.
            self.rebuild_explore_plots(cx);
            cx.notify();
        }
    }

    fn apply_handle_drag(&mut self, key: HandleKey, x: f64, cx: &mut Context<Self>) {
        let e0 = self
            .ui_params()
            .e0
            .or_else(|| self.spectrum.as_ref().and_then(|s| s.get_e0()))
            .unwrap_or(0.0);
        let step = key.rounding();
        let value = if key.relative_to_e0() {
            ((x - e0) / step).round() * step
        } else {
            (x / step).round() * step
        };
        let value = match key {
            HandleKey::Rbkg | HandleKey::FftKmin => value.max(0.0),
            _ => value,
        };
        let current = crate::app::param_field_value(key.param(), self.ui_params());
        if current == Some(value) {
            return;
        }
        // Params first (debounced recompute), then an immediate visual
        // update from the cached spectrum so the region follows the pointer.
        self.apply_param(key.param(), Some(value), cx);
        self.sync_param_fields(cx);
        self.rebuild_explore_plots(cx);
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
        if self.handles.dragging.take().is_some() {
            self.handles.armed = None;
            self.rebuild_explore_plots(cx);
            cx.notify();
        }
    }
}
