//! Builders turning a processed `XASSpectrum` into ruviz `Plot`s for the four
//! Explore quadrants: mu(E), normalized mu(E), k-weighted chi(k), |chi(R)|.

use ruviz::plots::heatmap::HeatmapConfig;
use ruviz::prelude::Plot;
use xraytsubaki::prelude::XASSpectrum;

use crate::theme::Theme;

fn vecs(v: &nalgebra::DVector<f64>) -> Vec<f64> {
    v.iter().copied().collect()
}

pub struct QuadrantPlots {
    pub mu_e: Plot,
    pub norm: Plot,
    pub chi_k: Plot,
    pub chi_r: Plot,
}

/// One spectrum in a comparison overlay.
pub struct QuadTrace {
    pub label: String,
    pub sp: std::sync::Arc<XASSpectrum>,
    pub active: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TraceLayout {
    Overlay,
    Waterfall,
}

/// Explore-view display options (see doc/gui-ux-design.md).
#[derive(Clone, Copy)]
pub struct ViewOptions {
    pub layout: TraceLayout,
    /// Waterfall offset as a fraction of the first trace's peak-to-peak.
    pub offset_frac: f64,
    pub legend: bool,
    pub grid: bool,
}

impl Default for ViewOptions {
    fn default() -> Self {
        Self {
            layout: TraceLayout::Overlay,
            offset_frac: 0.6,
            legend: true,
            grid: true,
        }
    }
}

/// Legends beyond this many traces are clutter.
const MAX_LEGEND_TRACES: usize = 8;

/// Build one quadrant from per-trace (x, y) extractions. Inactive traces
/// draw first (thin), the active trace last (thick, on top). Waterfall mode
/// offsets successive traces by `offset_frac` x the first trace's range.
fn build_multi(
    traces: &[QuadTrace],
    view: &ViewOptions,
    theme: &Theme,
    xlabel: &str,
    ylabel: &str,
    extract: impl Fn(&XASSpectrum) -> Option<(Vec<f64>, Vec<f64>)>,
) -> Plot {
    let mut series: Vec<(usize, &QuadTrace, Vec<f64>, Vec<f64>)> = traces
        .iter()
        .enumerate()
        .filter_map(|(i, t)| extract(&t.sp).map(|(x, y)| (i, t, x, y)))
        .collect();

    let offset = if view.layout == TraceLayout::Waterfall {
        let span = series
            .first()
            .map(|(_, _, _, y)| {
                let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
                for v in y {
                    lo = lo.min(*v);
                    hi = hi.max(*v);
                }
                (hi - lo).abs()
            })
            .filter(|s| s.is_finite() && *s > 0.0)
            .unwrap_or(1.0);
        span * view.offset_frac
    } else {
        0.0
    };
    if offset != 0.0 {
        for (i, _, _, y) in series.iter_mut() {
            let shift = *i as f64 * offset;
            for v in y.iter_mut() {
                *v += shift;
            }
        }
    }
    // Active trace drawn last so it sits on top.
    series.sort_by_key(|(_, t, _, _)| t.active);

    let n = series.len();
    let mut builder = Plot::new()
        .theme(theme.plot_theme())
        .grid(view.grid)
        .xlabel(xlabel)
        .ylabel(ylabel);
    let with_legend = view.legend && n > 1 && n <= MAX_LEGEND_TRACES;
    let mut plot: Option<Plot> = None;
    for (_, trace, x, y) in &series {
        let width = if trace.active && n > 1 { 2.2 } else { 1.4 };
        let sb = match plot.take() {
            None => builder.line(x, y),
            Some(p) => p.line(x, y),
        }
        .line_width(width);
        let sb = if with_legend {
            sb.label(&trace.label)
        } else {
            sb
        };
        plot = Some(sb.into());
        builder = Plot::new(); // unused after first; keeps the borrow checker simple
    }
    let plot = plot.unwrap_or_else(|| builder.into());
    if with_legend {
        let p: Plot = plot;
        // Plot -> builder chain for legend placement
        return p.legend(ruviz::core::Position::TopRight).into();
    }
    plot
}

/// All four Explore quadrants for a set of traces.
pub fn build_quadrants_multi(
    traces: &[QuadTrace],
    view: &ViewOptions,
    theme: &Theme,
) -> QuadrantPlots {
    let kw = traces
        .iter()
        .find(|t| t.active)
        .or_else(|| traces.first())
        .and_then(|t| t.sp.get_kweight().copied())
        .unwrap_or(2.0);

    let mu_e = build_multi(traces, view, theme, "Energy (eV)", "mu(E)", |sp| {
        Some((sp.energy.as_ref().map(vecs)?, sp.mu.as_ref().map(vecs)?))
    });
    let norm = build_multi(
        traces,
        view,
        theme,
        "Energy (eV)",
        "normalized mu(E)",
        |sp| {
            Some((
                sp.energy.as_ref().map(vecs)?,
                sp.get_flat().or_else(|| sp.get_norm()).map(|v| vecs(&v))?,
            ))
        },
    );
    let chi_k = build_multi(
        traces,
        view,
        theme,
        "k (1/Angstrom)",
        &format!("k^{kw:.0} chi(k)"),
        |sp| {
            Some((
                sp.get_k().map(|v| vecs(&v))?,
                sp.get_chi_kweighted().map(|v| vecs(&v))?,
            ))
        },
    );
    let chi_r = build_multi(traces, view, theme, "R (Angstrom)", "|chi(R)|", |sp| {
        let r = sp.get_r().map(|v| vecs(&v))?;
        let m = sp.get_chir_mag().map(|v| vecs(&v))?;
        let n = r.len().min(m.len());
        Some((r[..n].to_vec(), m[..n].to_vec()))
    });

    QuadrantPlots {
        mu_e,
        norm,
        chi_k,
        chi_r,
    }
}

/// Operando heatmap: rows = frames (time), cols = k-grid bins.
pub fn build_heatmap(matrix: &Vec<Vec<f64>>, kmax: f64, theme: &Theme) -> Plot {
    let _ = kmax;
    Plot::new()
        .theme(theme.plot_theme())
        .xlabel("k bin")
        .ylabel("frame")
        .heatmap(matrix, Some(HeatmapConfig::new().colorbar(true)))
        .into()
}

/// chi(k) of a single operando frame from the resampled grid row.
pub fn build_frame_chik(grid: &[f64], row: &[f64], kw: f64, theme: &Theme) -> Plot {
    let grid: Vec<f64> = grid.to_vec();
    let row: Vec<f64> = row.to_vec();
    Plot::new()
        .theme(theme.plot_theme())
        .line(&grid, &row)
        .xlabel("k (1/Angstrom)")
        .ylabel(format!("k^{kw:.0} chi(k)"))
        .into()
}

/// k-space fit overlay: k-weighted data vs model.
pub fn build_fit_k(result: &xraytsubaki::prelude::FeffFitResult, theme: &Theme) -> Plot {
    let k = vecs(&result.k);
    let kw = result.kweight;
    let weight = |chi: &nalgebra::DVector<f64>| -> Vec<f64> {
        chi.iter()
            .zip(k.iter())
            .map(|(c, kk)| c * kk.powf(kw))
            .collect()
    };
    let data = weight(&result.data_chi);
    let model = weight(&result.model_chi);
    Plot::new()
        .theme(theme.plot_theme())
        .line(&k, &data)
        .label("data")
        .line(&k, &model)
        .label("fit")
        .legend(ruviz::core::position::Position::TopRight)
        .xlabel("k (1/Angstrom)")
        .ylabel(format!("k^{kw:.0} chi(k)"))
        .into()
}

/// R-space fit overlay: |chi(R)| of data vs model.
pub fn build_fit_r(result: &xraytsubaki::prelude::FeffFitResult, theme: &Theme) -> Plot {
    let r = vecs(&result.r);
    let data_mag: Vec<f64> = result
        .data_chir_re
        .iter()
        .zip(result.data_chir_im.iter())
        .map(|(re, im)| (re * re + im * im).sqrt())
        .collect();
    let model_mag = vecs(&result.model_chir_mag);
    let n = r.len().min(data_mag.len()).min(model_mag.len());
    let r = r[..n].to_vec();
    let data_mag = data_mag[..n].to_vec();
    let model_mag = model_mag[..n].to_vec();
    Plot::new()
        .theme(theme.plot_theme())
        .line(&r, &data_mag)
        .label("data")
        .line(&r, &model_mag)
        .label("fit")
        .legend(ruviz::core::position::Position::TopRight)
        .xlabel("R (Angstrom)")
        .ylabel("|chi(R)|")
        .into()
}

/// Parameter-vs-frame trend with a cursor marker.
pub fn build_trend(values: &[f64], cursor: usize, ylabel: &str, theme: &Theme) -> Plot {
    let xs: Vec<f64> = (0..values.len()).map(|i| i as f64).collect();
    let ys: Vec<f64> = values.to_vec();
    let base = Plot::new()
        .theme(theme.plot_theme())
        .line(&xs, &ys)
        .xlabel("frame")
        .ylabel(ylabel);
    match values.get(cursor) {
        Some(&cy) if cy.is_finite() => {
            let cx = vec![cursor as f64];
            let cyv = vec![cy];
            base.scatter(&cx, &cyv).into()
        }
        _ => base.into(),
    }
}
