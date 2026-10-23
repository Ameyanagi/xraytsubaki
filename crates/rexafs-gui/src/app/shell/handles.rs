//! Direct manipulation on plots: range parameters are shaded regions with
//! draggable edges, E₀ / Rbkg are draggable lines.
//!
//! In the Process stage every range is drawn on each linked plot whose axis
//! it lives on: the pre-edge (blue) and normalization (yellow) ranges and E₀
//! on both μ(E) plots; the AUTOBK spline k range (yellow edges) on χ(k) and,
//! converted to energy, on μ(E); the forward-FT k window (blue) on χ(k); and
//! the back-FT R window (blue) with the Rbkg cutoff (yellow) on |χ(R)|.
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
use rexafs::prelude::BackgroundMethod;

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

/// Use raw energy coverage, not the shortened χ(k) grid: users must be able
/// to drag k-max outward again after reducing it. Share the core conversion.
pub(crate) fn background_k_domain(sp: &rexafs::prelude::XASSpectrum) -> Option<(f64, f64)> {
    let e0 = background_ek0(sp)?;
    let emax = sp.energy.as_ref()?.iter().copied().max_by(f64::total_cmp)?;
    (emax > e0).then(|| (0.0, energy_to_k(emax, e0)))
}

/// Edge energy E₀ (eV) that AUTOBK used to define k: its resolved `ek0`, or
/// the spectrum E₀ before the first background run has stored one.
pub(crate) fn background_ek0(sp: &rexafs::prelude::XASSpectrum) -> Option<f64> {
    match sp.background.as_ref()? {
        BackgroundMethod::AUTOBK(a) => a.ek0.or_else(|| sp.e0()),
        _ => None,
    }
}

/// Photon energy E (eV) of photoelectron wave number k (Å⁻¹):
/// E = E₀ + ħ²k²/(2mₑ) = E₀ + k²/ETOK, with 1/ETOK ≈ 3.81 eV·Å² taken from
/// the core crate. AUTOBK uses the same relation to place `kmax` in energy.
pub(crate) fn k_to_energy(k: f64, e0: f64) -> f64 {
    use rexafs::xafs::xafsutils::XAFSUtils;
    e0 + k.ktoe()
}

/// Inverse of [`k_to_energy`]: k = √((E − E₀)·ETOK) in Å⁻¹. Energies below
/// E₀ map to k = 0.
pub(crate) fn energy_to_k(e: f64, e0: f64) -> f64 {
    use rexafs::xafs::xafsutils::XAFSUtils;
    (e - e0).etok()
}

/// Drag readout for a background k bound shown on an energy axis: the stored
/// k and the photon energy where that cut falls.
fn background_k_energy_readout(k: f64, e: f64) -> String {
    format!("k {k:.1} Å⁻¹ · {e:.0} eV")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn background_handle_can_expand_past_a_cropped_chi_grid() {
        use rexafs::xafs::xafsutils::constants::KTOE;
        let mut sp = rexafs::prelude::XASSpectrum::default();
        sp.energy = Some(nalgebra::DVector::from_vec(vec![
            9000.,
            9000. + KTOE * 16.0_f64.powi(2),
        ]));
        sp.background = Some(BackgroundMethod::AUTOBK(rexafs::xafs::background::AUTOBK {
            ek0: Some(9000.),
            kmax: Some(10.),
            k: Some(nalgebra::DVector::from_vec(vec![0., 10.])),
            ..Default::default()
        }));
        let (lo, hi) = background_k_domain(&sp).unwrap();
        assert_eq!(lo, 0.0);
        assert!((hi - 16.).abs() < 1e-12, "{hi}");
    }

    #[test]
    fn background_k_bounds_map_to_energy_and_back() {
        use rexafs::xafs::xafsutils::constants::ETOK;
        // Ni K edge: k = 13.5 Å⁻¹ lies 13.5²/ETOK ≈ 694 eV above E₀.
        let e0 = 8333.0;
        let e = k_to_energy(13.5, e0);
        assert!((e - (e0 + 13.5_f64.powi(2) / ETOK)).abs() < 1e-9, "{e}");
        assert!((e - 9027.4).abs() < 0.1, "{e}");
        assert!((energy_to_k(e, e0) - 13.5).abs() < 1e-12);
        assert_eq!(k_to_energy(0.0, e0), e0);
        // Dragging below the edge clamps to k = 0 rather than producing NaN.
        assert_eq!(energy_to_k(e0 - 50.0, e0), 0.0);
        assert_eq!(
            background_k_energy_readout(13.5, 9030.0),
            "k 13.5 Å⁻¹ · 9030 eV"
        );
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HandleKey {
    E0,
    PreStart,
    PreEnd,
    NormStart,
    NormEnd,
    BkgKmin,
    BkgKmax,
    FftKmin,
    FftKmax,
    BftRmin,
    BftRmax,
    Rbkg,
    FitKmin,
    FitKmax,
    FitRmin,
    FitRmax,
    MeasurementStart,
    MeasurementEnd,
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
            HandleKey::BkgKmin => ParamKey::BkgKmin,
            HandleKey::BkgKmax => ParamKey::BkgKmax,
            HandleKey::FftKmin => ParamKey::FftKmin,
            HandleKey::FftKmax => ParamKey::FftKmax,
            HandleKey::BftRmin => ParamKey::BftRmin,
            HandleKey::BftRmax => ParamKey::BftRmax,
            HandleKey::Rbkg => ParamKey::Rbkg,
            HandleKey::MeasurementStart
            | HandleKey::MeasurementEnd
            | HandleKey::FitKmin
            | HandleKey::FitKmax
            | HandleKey::FitRmin
            | HandleKey::FitRmax => {
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
            HandleKey::MeasurementStart | HandleKey::MeasurementEnd => 0.1,
            HandleKey::E0 => 0.1,
            HandleKey::PreStart | HandleKey::PreEnd | HandleKey::NormStart | HandleKey::NormEnd => {
                1.0
            }
            HandleKey::FftKmin | HandleKey::FftKmax | HandleKey::BkgKmin | HandleKey::BkgKmax => {
                0.1
            }
            HandleKey::Rbkg => 0.05,
            HandleKey::FitKmin | HandleKey::FitKmax => 0.1,
            HandleKey::FitRmin | HandleKey::FitRmax | HandleKey::BftRmin | HandleKey::BftRmax => {
                0.05
            }
        }
    }

    /// Text shown next to the handle while it is dragged.
    fn readout(self, x: f64) -> String {
        match self {
            HandleKey::MeasurementStart | HandleKey::MeasurementEnd => format!("{x:.2}"),
            HandleKey::E0 => format!("E₀ {x:.1} eV"),
            HandleKey::PreStart | HandleKey::PreEnd | HandleKey::NormStart | HandleKey::NormEnd => {
                format!("{x:.0} eV")
            }
            HandleKey::FftKmin
            | HandleKey::FftKmax
            | HandleKey::FitKmin
            | HandleKey::FitKmax
            | HandleKey::BkgKmin
            | HandleKey::BkgKmax => {
                format!("k {x:.1} Å⁻¹")
            }
            HandleKey::Rbkg => format!("Rbkg {x:.2} Å"),
            HandleKey::FitRmin | HandleKey::FitRmax | HandleKey::BftRmin | HandleKey::BftRmax => {
                format!("R {x:.2} Å")
            }
        }
    }

    /// Primary (accent) handles are the window each plot is mainly about;
    /// secondary (warning-coloured) handles share a plot with one: the
    /// normalization range beside the pre-edge range, the spline k range
    /// beside the FT window, and Rbkg beside the back-FT R window.
    fn accent(self) -> bool {
        !matches!(
            self,
            HandleKey::NormStart
                | HandleKey::NormEnd
                | HandleKey::BkgKmin
                | HandleKey::BkgKmax
                | HandleKey::Rbkg
        )
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
    /// Display preference only; hiding ranges does not change any parameter.
    pub hidden: bool,
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
    pub(crate) fn plot_ranges_button(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        super::controls::icon_button(
            &self.theme,
            "toggle-plot-ranges",
            crate::icons::Icon::Range,
            if self.handles.hidden {
                "Show plot ranges"
            } else {
                "Hide plot ranges"
            },
            !self.handles.hidden,
        )
        .on_click(cx.listener(|app, _, _, cx| app.toggle_plot_ranges(cx)))
    }
    pub(crate) fn toggle_plot_ranges(&mut self, cx: &mut Context<Self>) {
        self.end_handle_drag(cx);
        self.handles.armed = None;
        self.measurements.wavelet_drag = None;
        self.measurements.wavelet_armed = None;
        self.handles.hidden = !self.handles.hidden;
        if self.stage == Stage::Process && self.normalization.open {
            self.rebuild_normalization_plot(true, cx);
        }
        if self.stage == Stage::Data && self.peaks.open {
            self.rebuild_peak_plot(true, cx);
        }
        // Analysis boundaries are part of the plot specification. Ordinary
        // processing handles only need repainting, not a scientific recompute.
        if self.stage == Stage::Data && self.analysis_energy_interval(cx).is_some() {
            self.invalidate_explore_plots(cx);
        }
        cx.notify();
    }

    /// Handles the stage exposes on a given plot, with their current data x.
    fn handle_specs(&self, plot: usize) -> (Vec<(HandleKey, f64)>, Vec<Span>) {
        if self.handles.hidden {
            return (Vec::new(), Vec::new());
        }
        if plot == super::measurements::drag::PLOT_MEASUREMENT || plot == super::peaks::PLOT_PEAKS {
            let specs = if plot == super::peaks::PLOT_PEAKS {
                self.peak_handle_specs()
            } else {
                self.measurement_handle_specs()
            };
            let spans = if specs.len() == 2 {
                vec![Span {
                    lo: Some(HandleKey::MeasurementStart),
                    hi: Some(HandleKey::MeasurementEnd),
                    fixed_lo: 0.,
                    accent: true,
                }]
            } else {
                vec![]
            };
            return (specs, spans);
        }
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
        if !self.spectrum_quantity.supports_exafs() {
            return (Vec::new(), Vec::new());
        }
        if self.stage != Stage::Process {
            return (Vec::new(), Vec::new());
        }
        // Each plot lists its primary handles first: on a tie in distance
        // the earlier handle arms (E₀ before a spline k-min at E₀).
        let parts = match plot {
            PLOT_MU => vec![
                self.normalization_handles(sp),
                self.spline_k_handles(plot, sp),
            ],
            PLOT_NORM => vec![self.normalization_handles(sp)],
            PLOT_CHIK => vec![self.ft_k_handles(), self.spline_k_handles(plot, sp)],
            PLOT_CHIR => vec![self.back_ft_r_handles(sp), self.rbkg_handles(sp)],
            _ => Vec::new(),
        };
        parts.into_iter().fold(
            (Vec::new(), Vec::new()),
            |(mut specs, mut spans), (more_specs, more_spans)| {
                specs.extend(more_specs);
                spans.extend(more_spans);
                (specs, spans)
            },
        )
    }

    /// Pre-edge and normalization ranges (stored relative to E₀) and E₀.
    fn normalization_handles(
        &self,
        sp: &rexafs::prelude::XASSpectrum,
    ) -> (Vec<(HandleKey, f64)>, Vec<Span>) {
        let p = self.ui_params();
        let Some(e0) = p.e0.or_else(|| sp.e0()) else {
            return (Vec::new(), Vec::new());
        };
        if self.spectrum_quantity.prepared_space().is_some() && !p.refit_prepared {
            // The component already has its declared scale. Only its
            // energy origin is editable; baseline ranges do not apply.
            return (vec![(HandleKey::E0, e0)], Vec::new());
        }
        let ranges = crate::params::normalization_ranges(sp);
        let rel = |param: Option<f64>, from_sp: Option<f64>, default: f64| {
            param.or(from_sp).unwrap_or(default)
        };
        let pre1 = rel(p.pre_edge_start, ranges.map(|r| r[0]), -200.0);
        let pre2 = rel(p.pre_edge_end, ranges.map(|r| r[1]), -30.0);
        let nor1 = rel(p.norm_start, ranges.map(|r| r[2]), 150.0);
        let emax = sp
            .energy
            .as_ref()
            .and_then(|e| e.iter().next_back().copied())
            .unwrap_or(e0 + 2000.0);
        let nor2 = rel(p.norm_end, ranges.map(|r| r[3]), 2000.0).min(emax - e0);
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

    /// AUTOBK spline k range: in k on χ(k), at E₀ + k²/ETOK on μ(E). Edges
    /// only, so the shaded FT window stays readable.
    fn spline_k_handles(
        &self,
        plot: usize,
        sp: &rexafs::prelude::XASSpectrum,
    ) -> (Vec<(HandleKey, f64)>, Vec<Span>) {
        let p = self.ui_params();
        let a = match sp.background.as_ref() {
            Some(BackgroundMethod::AUTOBK(a)) => a,
            _ => return (Vec::new(), Vec::new()),
        };
        let lo = p.bkg_kmin.or(a.kmin).unwrap_or(0.0);
        let available = background_k_domain(sp).map(|(_, hi)| hi).unwrap_or(0.0);
        let hi = p.bkg_kmax.or(a.kmax).unwrap_or(available).min(available);
        // On μ(E) the same k bounds sit at E₀ + k²/ETOK. The stored
        // value stays in k, so the cut follows E₀ when it changes.
        let (lo, hi) = match self.background_energy_e0(plot) {
            Some(e0) => (k_to_energy(lo, e0), k_to_energy(hi, e0)),
            None if plot == PLOT_CHIK => (lo, hi),
            None => return (Vec::new(), Vec::new()),
        };
        (
            vec![(HandleKey::BkgKmin, lo), (HandleKey::BkgKmax, hi)],
            Vec::new(),
        )
    }

    /// Forward-FT k window on χ(k).
    fn ft_k_handles(&self) -> (Vec<(HandleKey, f64)>, Vec<Span>) {
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

    /// Back-FT R window on |χ(R)|.
    fn back_ft_r_handles(
        &self,
        sp: &rexafs::prelude::XASSpectrum,
    ) -> (Vec<(HandleKey, f64)>, Vec<Span>) {
        let p = self.ui_params();
        let lo = p
            .bft_rmin
            .or_else(|| sp.xftr.as_ref().and_then(|f| f.rmin))
            .unwrap_or(0.0);
        let hi = p
            .bft_rmax
            .or_else(|| sp.xftr.as_ref().and_then(|f| f.rmax))
            .unwrap_or(20.0);
        (
            vec![(HandleKey::BftRmin, lo), (HandleKey::BftRmax, hi)],
            vec![Span {
                lo: Some(HandleKey::BftRmin),
                hi: Some(HandleKey::BftRmax),
                fixed_lo: 0.0,
                accent: true,
            }],
        )
    }

    /// Rbkg on |χ(R)|: AUTOBK removes the region R < Rbkg (shaded).
    fn rbkg_handles(
        &self,
        sp: &rexafs::prelude::XASSpectrum,
    ) -> (Vec<(HandleKey, f64)>, Vec<Span>) {
        let rbkg = self
            .ui_params()
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
                accent: false,
            }],
        )
    }

    /// E₀ for converting the background k handles when `plot` is the
    /// Process stage's μ(E) plot; `None` on k-axis plots.
    fn background_energy_e0(&self, plot: usize) -> Option<f64> {
        if self.stage != Stage::Process || plot != PLOT_MU {
            return None;
        }
        background_ek0(self.spectrum.as_deref()?)
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
        if !self.stage.is_processing()
            && self.stage != Stage::Fit
            && plot != super::measurements::drag::PLOT_MEASUREMENT
        {
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
            super::measurements::drag::PLOT_MEASUREMENT => self.measurement_preview_entity(),
            super::peaks::PLOT_PEAKS => self.peaks.plot.clone(),
            super::wavelet::PLOT_WAVELET_K => self.wavelet.plot_k.clone(),
            super::wavelet::PLOT_WAVELET_R => self.wavelet.plot_r.clone(),
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
                specs.iter().find(|(k, _)| *k == key).map(|&(k, x)| {
                    (
                        x,
                        if p == super::measurements::drag::PLOT_MEASUREMENT {
                            self.measurement_handle_readout(x)
                        } else if p == super::peaks::PLOT_PEAKS {
                            self.peak_handle_readout(x)
                        } else if let Some(e0) = self.background_energy_e0(p) {
                            background_k_energy_readout(energy_to_k(x, e0), x)
                        } else {
                            k.readout(x)
                        },
                    )
                })
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
        if self.handles.hidden || entity.read(cx).is_context_menu_open() {
            if self.handles.armed.take().is_some() {
                cx.notify();
            }
            return;
        }
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
            // Background k bounds dragged on μ(E) arrive in eV; the
            // parameter and its clamp to the data are in k.
            let x = match self.background_energy_e0(plot) {
                Some(e0) if matches!(key, HandleKey::BkgKmin | HandleKey::BkgKmax) => {
                    x.map(|e| energy_to_k(e, e0))
                }
                _ => x,
            };
            if let Some(x) = x {
                self.apply_handle_drag(key, x, cx);
            }
            return;
        }
        // The visible grab tabs are above the plot area. Hit-test their
        // window coordinates too; data_at intentionally excludes this strip.
        let (specs, _) = self.handle_specs(plot);
        let visible = self
            .handles
            .areas
            .lock()
            .ok()
            .and_then(|c| c.get(&plot).copied());
        let mut nearest: Option<(HandleKey, f64)> = None;
        if let Some(area) = visible {
            let x = f32::from(position.x);
            let y = f32::from(position.y);
            if y >= area.top - 18.
                && y <= area.bottom
                && x >= area.left - ARM_PX as f32
                && x <= area.right + ARM_PX as f32
            {
                for (key, value) in specs {
                    if value < area.data_x0 || value > area.data_x1 {
                        continue;
                    }
                    let distance = (area.px_of(value) - x).abs() as f64;
                    if distance <= ARM_PX && nearest.is_none_or(|(_, best)| distance < best) {
                        nearest = Some((key, distance));
                    }
                }
            }
        }
        let armed = nearest.map(|(key, _)| (plot, key));
        if armed != self.handles.armed {
            self.handles.armed = armed;
            cx.notify();
        }
    }

    pub(crate) fn capture_handle_press(
        &mut self,
        plot: usize,
        event: &gpui::MouseDownEvent,
        cx: &mut Context<Self>,
    ) {
        if event.button != gpui::MouseButton::Left {
            // A context-menu press may start directly on a range boundary.
            // Remove its capture layer before the plot opens the menu.
            if self.handles.armed.take().is_some() {
                cx.notify();
            }
            return;
        }
        self.plot_pointer_move(plot, event.position, cx);
        if let Some((p, key)) = self.handles.armed
            && p == plot
        {
            self.handles.dragging = Some((p, key));
            cx.stop_propagation();
            cx.notify();
        }
    }

    fn apply_handle_drag(&mut self, key: HandleKey, x: f64, cx: &mut Context<Self>) {
        if matches!(key, HandleKey::MeasurementStart | HandleKey::MeasurementEnd) {
            if self.peaks.open && self.stage == Stage::Data {
                self.drag_peak_boundary(key, x, cx);
            } else {
                self.drag_measurement_boundary(key, x, cx);
            }
            return;
        }
        let e0 = self
            .ui_params()
            .e0
            .or_else(|| self.spectrum.as_ref().and_then(|s| s.e0()))
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
        // Reapply the domain after rounding so the end never crosses the
        // measured range when the final sample is between rounding steps.
        let value = if !key.relative_to_e0() {
            self.handle_domain(key)
                .filter(|(lo, hi)| lo < hi)
                .map_or(value, |(lo, hi)| value.clamp(lo, hi))
        } else {
            value
        };
        let value = match key {
            HandleKey::Rbkg | HandleKey::FftKmin | HandleKey::FitKmin | HandleKey::FitRmin => {
                value.max(0.0)
            }
            _ => value,
        };
        let Some(param) = key.param() else {
            // Fit ranges: model state, not pipeline params.
            let id = self.joint_plotted_dataset_id();
            // Clamp after rounding as Rbkg need not lie on the handle's grid.
            let value = if key == HandleKey::FitRmin {
                value.max(self.fit_background_floor(id))
            } else {
                value
            };
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
            if (key == HandleKey::FitRmin && value >= r.rmax)
                || (key == HandleKey::FitRmax && value <= r.rmin)
                || (key == HandleKey::FitKmin && value >= r.kmax)
                || (key == HandleKey::FitKmax && value <= r.kmin)
            {
                return;
            }
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
        let plot = match key {
            HandleKey::BkgKmin | HandleKey::BkgKmax => Some(PLOT_CHIK),
            HandleKey::BftRmin | HandleKey::BftRmax => Some(PLOT_CHIR),
            _ => None,
        };
        if let Some(plot) = plot {
            let (specs, _) = self.handle_specs(plot);
            let (other, is_min) = match key {
                HandleKey::BkgKmin => (HandleKey::BkgKmax, true),
                HandleKey::BkgKmax => (HandleKey::BkgKmin, false),
                HandleKey::BftRmin => (HandleKey::BftRmax, true),
                _ => (HandleKey::BftRmin, false),
            };
            if specs
                .iter()
                .any(|(k, x)| *k == other && if is_min { value >= *x } else { value <= *x })
            {
                return;
            }
        }
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
            HandleKey::MeasurementStart | HandleKey::MeasurementEnd => None,
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
            HandleKey::BkgKmin | HandleKey::BkgKmax => background_k_domain(sp),
            HandleKey::FftKmin | HandleKey::FftKmax | HandleKey::FitKmin | HandleKey::FitKmax => {
                Some((
                    0.0,
                    sp.k()
                        .map(nalgebra::DVector::from_column_slice)
                        .as_ref()
                        .and_then(last)?,
                ))
            }
            HandleKey::Rbkg
            | HandleKey::FitRmin
            | HandleKey::FitRmax
            | HandleKey::BftRmin
            | HandleKey::BftRmax => Some((0.0, sp.r().as_ref().and_then(last)?)),
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
        if !active
            || self.handles.hidden
            || self
                .plot_entity(plot)
                .is_some_and(|p| p.read(cx).is_context_menu_open())
        {
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
            if !self.structure.settings.range_hint_seen {
                self.structure.settings.range_hint_seen = true;
                if let Err(error) = self.structure.settings.save() {
                    self.record_job_error("range hint setting", error);
                }
            }
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
