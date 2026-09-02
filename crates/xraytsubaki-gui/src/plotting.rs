//! Builders turning a processed `XASSpectrum` into ruviz `Plot`s for the four
//! Explore quadrants: mu(E), normalized mu(E), k-weighted chi(k), |chi(R)|.
//!
//! ruviz 0.5 retains native rotated y labels and adds the interactive APIs used
//! by the operando plots. Card headers remain plot titles rather than serving as
//! a workaround for broken vertical text.

use ruviz::core::LegendPosition;
use ruviz::data::Observable;
use ruviz::plots::heatmap::{HeatmapConfig, HeatmapOrigin};
use ruviz::prelude::Plot;
use ruviz::render::{Color, LineStyle};
use xraytsubaki::prelude::BackgroundMethod;
use xraytsubaki::prelude::NormalizationMethod;
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
    /// Back-transform chi(q) with the active trace's k-weighted chi(k) behind it.
    pub chi_q: Plot,
    /// Card-header plot titles for each quadrant.
    pub titles: [String; 5],
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
    /// Diagnostics on the mu(E) quadrant (drawn for the active trace only).
    pub show_pre: bool,
    pub show_post: bool,
    pub show_e0: bool,
    pub show_ranges: bool,
    /// Normalized quadrant shows flattened (true) or plain normalized mu(E).
    pub flat: bool,
    /// chi(k) quadrant: FFT k-range lines and the FT window curve.
    pub show_krange: bool,
    pub show_kwin: bool,
    /// |chi(R)| quadrant: also plot Re[chi(R)] of the active trace.
    pub show_re: bool,
    /// AUTOBK background spline over mu(E) (Background stage).
    pub show_bkg: bool,
}

impl Default for ViewOptions {
    fn default() -> Self {
        Self {
            layout: TraceLayout::Overlay,
            offset_frac: 0.6,
            legend: true,
            grid: true,
            show_pre: false,
            show_post: false,
            show_e0: false,
            show_ranges: false,
            flat: true,
            show_krange: false,
            show_kwin: false,
            show_re: false,
            show_bkg: false,
        }
    }
}

/// Legends beyond this many traces are clutter.
const MAX_LEGEND_TRACES: usize = 8;

/// Stable per-trace color: keyed by the trace's position in the sorted
/// selection so draw order (active drawn last) never reshuffles colors.
pub fn trace_color(theme: &Theme, index: usize) -> Color {
    theme.plot_theme().get_color(index)
}

/// The same stable trace color as a gpui color, for the shared legend strip.
pub fn trace_rgba(theme: &Theme, index: usize) -> gpui::Rgba {
    let c = trace_color(theme, index);
    gpui::Rgba {
        r: c.r as f32 / 255.0,
        g: c.g as f32 / 255.0,
        b: c.b as f32 / 255.0,
        a: c.a as f32 / 255.0,
    }
}

/// Middle-truncate `name` to at most `max` characters ("frame_0…003.dat").
pub fn middle_truncate(name: &str, max: usize) -> String {
    let count = name.chars().count();
    if count <= max || max < 2 {
        return name.to_string();
    }
    let head = (max - 1) / 2;
    let tail = max - 1 - head;
    let front: String = name.chars().take(head).collect();
    let back: String = name.chars().skip(count - tail).collect();
    format!("{front}…{back}")
}

/// Axis label for the wavenumber axis.
pub const K_AXIS: &str = "k (Å⁻¹)";
/// Axis label for the radial-distance axis.
pub const R_AXIS: &str = "R (Å)";

/// Label for |chi(R)|, whose unit depends on the k-weight used for the
/// transform: k-weight n gives Å^-(n+1). Hardcoding Å⁻³ was only correct for
/// the default k-weight of 2.
pub fn chir_label(kw: f64) -> String {
    let n = kw.round();
    if (kw - n).abs() > 1e-9 {
        return format!("|χ(R)| (Å^-{:.1})", kw + 1.0);
    }
    match n as i64 + 1 {
        1 => "|χ(R)| (Å⁻¹)".to_string(),
        2 => "|χ(R)| (Å⁻²)".to_string(),
        3 => "|χ(R)| (Å⁻³)".to_string(),
        4 => "|χ(R)| (Å⁻⁴)".to_string(),
        m => format!("|χ(R)| (Å^-{m})"),
    }
}

/// Label for a k-weighted chi(k) quantity.
pub fn chik_label(kw: f64) -> String {
    let n = kw.round();
    if (kw - n).abs() > 1e-9 {
        return format!("k^{kw:.1} χ(k)");
    }
    match n as i64 {
        0 => "χ(k)".to_string(),
        1 => "k·χ(k)".to_string(),
        2 => "k²χ(k)".to_string(),
        3 => "k³χ(k)".to_string(),
        _ => format!("k^{n:.0} χ(k)"),
    }
}

/// Build one quadrant from per-trace (x, y) extractions. Inactive traces
/// draw first (thin), the active trace last (thick, on top); every trace has
/// an explicit color keyed to its original position so the reordering never
/// recolors anything. Waterfall mode offsets successive traces by
/// `offset_frac` x the first trace's range. Returns the plot plus the
/// waterfall shift of the active (or first) trace so diagnostic overlays can
/// be aligned to the curve they annotate.
fn build_multi(
    traces: &[QuadTrace],
    view: &ViewOptions,
    theme: &Theme,
    in_plot_legend: bool,
    xlabel: &str,
    ylabel: &str,
    extract: impl Fn(&XASSpectrum) -> Option<(Vec<f64>, Vec<f64>)>,
) -> (Plot, f64) {
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
    let active_shift = offset * traces.iter().position(|t| t.active).unwrap_or(0) as f64;
    // Active trace drawn last so it sits on top.
    series.sort_by_key(|(_, t, _, _)| t.active);

    let n = series.len();
    let mut plot = Plot::new()
        .theme(theme.plot_theme())
        .grid(view.grid)
        .xlabel(xlabel)
        .ylabel(ylabel);
    // In the grid the shared GPUI legend strip identifies traces; in-plot
    // legends only when a quadrant is maximized.
    let with_legend = in_plot_legend && view.legend && n > 1 && n <= MAX_LEGEND_TRACES;
    for (i, trace, x, y) in &series {
        let width = if trace.active && n > 1 { 2.2 } else { 1.4 };
        let sb = plot
            .line(x, y)
            .line_width(width)
            .color(trace_color(theme, *i));
        let sb = if with_legend {
            sb.label(middle_truncate(&trace.label, 24))
        } else {
            sb
        };
        plot = sb.into();
    }
    if with_legend {
        return (
            plot.legend_position(LegendPosition::OutsideLower)
                .legend_columns(n.min(4)),
            active_shift,
        );
    }
    (plot, active_shift)
}

/// Normalization-check overlays for the active trace on the mu(E) plot:
/// dashed pre/post-edge trendlines, the E0 line, and the fit-window lines
/// (pre-edge range muted, norm range in a second hue; values are stored
/// relative to E0). `shift` is the active trace's waterfall offset so the
/// trendlines sit on the curve they belong to.
fn add_mu_diagnostics(mut plot: Plot, sp: &XASSpectrum, view: &ViewOptions, shift: f64) -> Plot {
    let trend = Color::from_gray(150);
    let e0_color = Color::ORANGE;
    let pre_color = Color::from_rgb(90, 140, 200);
    let norm_color = Color::from_rgb(90, 180, 120);

    if view.show_pre
        && let (Some(energy), Some(pre)) = (sp.energy.as_ref(), sp.get_pre_edge())
    {
        let x = vecs(energy);
        let y: Vec<f64> = pre.iter().map(|v| v + shift).collect();
        plot = plot
            .line(&x, &y)
            .line_width(1.0)
            .line_style(LineStyle::Dashed)
            .color(trend)
            .label("pre-edge")
            .into();
    }
    if view.show_post
        && let (Some(energy), Some(post)) = (sp.energy.as_ref(), sp.get_post_edge())
    {
        let x = vecs(energy);
        let y: Vec<f64> = post.iter().map(|v| v + shift).collect();
        plot = plot
            .line(&x, &y)
            .line_width(1.0)
            .line_style(LineStyle::Dashed)
            .color(trend)
            .label("post-edge")
            .into();
    }
    if view.show_bkg
        && let (Some(energy), Some(BackgroundMethod::AUTOBK(autobk))) =
            (sp.energy.as_ref(), sp.background.as_ref())
        && let Some(bkg) = autobk.get_bkg()
    {
        let x = vecs(energy);
        let n = x.len().min(bkg.len());
        let y: Vec<f64> = bkg.iter().take(n).map(|v| v + shift).collect();
        plot = plot
            .line(&x[..n], &y)
            .line_width(1.5)
            .color(Color::from_rgb(232, 121, 47))
            .label("background μ₀(E)")
            .into();
    }
    let e0 = sp.get_e0();
    if view.show_e0
        && let Some(e0) = e0
    {
        plot = plot.vline_styled(e0, e0_color, 1.2, LineStyle::Dashed);
    }
    if view.show_ranges
        && let (Some(e0), Some(NormalizationMethod::PrePostEdge(ppe))) =
            (e0, sp.normalization.as_ref())
    {
        for (value, color) in [
            (ppe.get_pre_edge_start(), pre_color),
            (ppe.get_pre_edge_end(), pre_color),
            (ppe.get_norm_start(), norm_color),
            (ppe.get_norm_end(), norm_color),
        ] {
            if let Some(rel) = value {
                plot = plot.vline_styled(e0 + rel, color, 1.0, LineStyle::Dashed);
            }
        }
    }
    plot
}

/// All four Explore quadrants for a set of traces. `in_plot_legend` opts into
/// ruviz legends (maximized quadrant); the grid uses the shared GPUI strip.
pub fn build_quadrants_multi(
    traces: &[QuadTrace],
    view: &ViewOptions,
    theme: &Theme,
    in_plot_legend: bool,
) -> QuadrantPlots {
    let kw = traces
        .iter()
        .find(|t| t.active)
        .or_else(|| traces.first())
        .and_then(|t| t.sp.get_kweight().copied())
        .unwrap_or(2.0);

    let (mut mu_e, mu_shift) = build_multi(
        traces,
        view,
        theme,
        in_plot_legend,
        "Energy (eV)",
        "μ(E)",
        |sp| Some((sp.energy.as_ref().map(vecs)?, sp.mu.as_ref().map(vecs)?)),
    );
    let flat = view.flat;
    let norm_label = if flat {
        "flat μ(E)"
    } else {
        "normalized μ(E)"
    };
    let (mut norm, _) = build_multi(
        traces,
        view,
        theme,
        in_plot_legend,
        "Energy (eV)",
        norm_label,
        move |sp| {
            let y = if flat {
                sp.get_flat().or_else(|| sp.get_norm())
            } else {
                sp.get_norm().or_else(|| sp.get_flat())
            };
            Some((sp.energy.as_ref().map(vecs)?, y.map(|v| vecs(&v))?))
        },
    );
    let chik_label = chik_label(kw);
    let (mut chi_k, chik_shift) = build_multi(
        traces,
        view,
        theme,
        in_plot_legend,
        K_AXIS,
        &chik_label,
        |sp| {
            Some((
                sp.get_k().map(|v| vecs(&v))?,
                sp.get_chi_kweighted().map(|v| vecs(&v))?,
            ))
        },
    );
    let (mut chi_r, chir_shift) = build_multi(
        traces,
        view,
        theme,
        in_plot_legend,
        R_AXIS,
        &chir_label(kw),
        |sp| {
            let r = sp.get_r().map(|v| vecs(&v))?;
            let m = sp.get_chir_mag().map(|v| vecs(&v))?;
            let n = r.len().min(m.len());
            Some((r[..n].to_vec(), m[..n].to_vec()))
        },
    );

    if let Some(active) = traces.iter().find(|t| t.active).or_else(|| traces.first()) {
        mu_e = add_mu_diagnostics(mu_e, &active.sp, view, mu_shift);
        if view.show_e0
            && let Some(e0) = active.sp.get_e0()
        {
            norm = norm.vline_styled(e0, Color::ORANGE, 1.2, LineStyle::Dashed);
        }
        // FT window diagnostics on chi(k), scaled to the data amplitude and
        // shifted onto the active trace in waterfall mode.
        if view.show_kwin
            && let (Some(k), Some(kwin), Some(chi)) = (
                active.sp.get_k(),
                active.sp.get_kwin(),
                active.sp.get_chi_kweighted(),
            )
        {
            let peak = chi.iter().fold(0.0f64, |m, v| m.max(v.abs())).max(1e-12);
            let x = vecs(&k);
            let n = x.len().min(kwin.len());
            let y: Vec<f64> = kwin.iter().take(n).map(|w| w * peak + chik_shift).collect();
            let x = x[..n].to_vec();
            chi_k = chi_k
                .line(&x, &y)
                .line_width(1.0)
                .line_style(LineStyle::Dashed)
                .color(Color::from_gray(150))
                .label("window")
                .into();
        }
        if view.show_krange
            && let Some(xftf) = active.sp.xftf.as_ref()
        {
            for v in [xftf.kmin, xftf.kmax].into_iter().flatten() {
                chi_k =
                    chi_k.vline_styled(v, Color::from_rgb(90, 140, 200), 1.0, LineStyle::Dashed);
            }
        }
        // Re part of chi(R) for phase-agreement checks (doc: "|χ(R)| (+Re
        // part toggle)"), aligned to the active trace's waterfall offset.
        if view.show_re
            && let (Some(r), Some(re)) = (active.sp.get_r(), active.sp.get_chir_real())
        {
            let r = vecs(&r);
            let n = r.len().min(re.len());
            let y: Vec<f64> = re.iter().take(n).map(|v| v + chir_shift).collect();
            let x = r[..n].to_vec();
            chi_r = chi_r
                .line(&x, &y)
                .line_width(1.0)
                .line_style(LineStyle::Dashed)
                .color(Color::from_gray(150))
                .label("Re")
                .into();
        }
    }
    let (mut chi_q, chiq_shift) = build_multi(
        traces,
        view,
        theme,
        in_plot_legend,
        "q (Å⁻¹)",
        "χ(q)",
        |sp| {
            let q = sp.get_q().map(|v| vecs(&v))?;
            let c = sp.get_chiq().map(|v| vecs(&v))?;
            let n = q.len().min(c.len());
            Some((q[..n].to_vec(), c[..n].to_vec()))
        },
    );
    if let Some(active) = traces.iter().find(|t| t.active).or_else(|| traces.first())
        && let (Some(k), Some(chi)) = (active.sp.get_k(), active.sp.get_chi_kweighted())
    {
        let x = vecs(&k);
        let n = x.len().min(chi.len());
        let y: Vec<f64> = chi.iter().take(n).map(|v| v + chiq_shift).collect();
        chi_q = chi_q
            .line(&x[..n], &y)
            .line_width(1.0)
            .line_style(LineStyle::Dashed)
            .color(Color::from_gray(150))
            .label(chik_label.clone())
            .into();
    }
    let titles = [
        "μ(E)".to_string(),
        norm_label.to_string(),
        chik_label,
        if view.show_re {
            "|χ(R)| + Re".to_string()
        } else {
            "|χ(R)|".to_string()
        },
        "χ(q)".to_string(),
    ];
    QuadrantPlots {
        mu_e,
        norm,
        chi_k,
        chi_r,
        chi_q,
        titles,
    }
}

fn heatmap_y_extent(scan_len: usize, row_count: usize) -> (f64, f64) {
    let last_frame = scan_len.saturating_sub(1) as f64;
    let row_step = if row_count > 1 {
        last_frame / (row_count - 1) as f64
    } else {
        1.0
    };
    let half_step = row_step / 2.0;
    (-half_step, last_frame + half_step)
}

/// Operando heatmap in physical units: x in k (1/Angstrom) from the resample
/// grid, y = frame index over the FULL scan (rows are the evenly sampled
/// overview). Rows remain in chronological order; the lower heatmap origin maps
/// row 0 to frame 0, while the displayed y axis is reversed so frame 0 appears
/// at the top and tick values remain truthful.
pub fn build_heatmap(
    matrix: &[Vec<f64>],
    grid: &[f64],
    scan_len: usize,
    xlabel: &str,
    theme: &Theme,
) -> Plot {
    let kmin = grid.first().copied().unwrap_or(0.0);
    let kmax = grid.last().copied().unwrap_or(1.0).max(kmin + 1e-9);
    let last_frame = scan_len.saturating_sub(1) as f64;
    let (ymin, ymax) = heatmap_y_extent(scan_len, matrix.len());
    let (view_max, view_min) = if scan_len <= 1 {
        (ymax, ymin)
    } else {
        (last_frame, 0.0)
    };
    Plot::new()
        .theme(theme.plot_theme())
        .xlabel(xlabel)
        .ylabel("frame")
        .heatmap_with(
            matrix,
            HeatmapConfig::new()
                .colorbar(true)
                .origin(HeatmapOrigin::Lower)
                .extent(kmin, kmax, ymin, ymax),
        )
        .ylim(view_max, view_min)
        .into()
}

/// One overview frame as a plain line (energy / R spaces), with the
/// reference standards behind it when given.
pub fn build_frame_row(
    grid: &[f64],
    row: &[f64],
    xlabel: &str,
    ylabel: &str,
    theme: &Theme,
) -> Plot {
    let n = grid.len().min(row.len());
    let plot: Plot = Plot::new()
        .theme(theme.plot_theme())
        .line(&grid[..n], &row[..n])
        .color(trace_color(theme, 0))
        .into();
    plot.xlabel(xlabel).ylabel(ylabel)
}

/// Source-backed chi(k) of one operando frame. Replacing `values` redraws the
/// data without rebuilding the interactive plot session.
pub fn build_frame_chik_source(
    grid: &[f64],
    values: Observable<Vec<f64>>,
    kweight: f64,
    theme: &Theme,
) -> Plot {
    Plot::new()
        .theme(theme.plot_theme())
        .line_source(grid, values)
        .xlabel(K_AXIS)
        .ylabel(chik_label(kweight))
        .into()
}

/// k-space fit overlay: k-weighted data vs model and, optionally, the
/// per-path contributions. The fit range is drawn by the handle decor.
pub fn build_fit_k(
    result: &xraytsubaki::prelude::FeffFitResult,
    theme: &Theme,
    show_paths: bool,
) -> Plot {
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
    let mut plot: Plot = Plot::new()
        .theme(theme.plot_theme())
        .line(&k, &data)
        .color(trace_color(theme, 0))
        .label("data")
        .line(&k, &model)
        .color(FIT_COLOR)
        .line_width(1.6)
        .label("fit")
        .into();
    if show_paths && result.path_contributions.len() > 1 {
        for (i, path) in result.path_contributions.iter().enumerate() {
            let y = weight(&path.chi);
            plot = plot
                .line(&k, &y)
                .color(trace_color(theme, i + 2))
                .line_width(1.0)
                .line_style(LineStyle::Dashed)
                .label(&path.label)
                .into();
        }
    }
    plot.legend_position(ruviz::core::LegendPosition::UpperRight)
        .xlabel(K_AXIS)
        .ylabel(chik_label(kw))
}

/// Fit trace colour (orange in both themes, distinct from the data trace).
const FIT_COLOR: Color = Color {
    r: 232,
    g: 121,
    b: 47,
    a: 255,
};

/// R-space fit overlay: |χ(R)| of data vs model, optionally Re[χ(R)] and
/// the per-path contributions stacked below the data (Artemis style).
pub fn build_fit_r(
    result: &xraytsubaki::prelude::FeffFitResult,
    theme: &Theme,
    show_paths: bool,
    show_re: bool,
) -> Plot {
    let r = vecs(&result.r);
    let data_mag = fit_data_chir_mag(result);
    let model_mag = vecs(&result.model_chir_mag);
    let n = r.len().min(data_mag.len()).min(model_mag.len());
    let r = r[..n].to_vec();
    let data_mag = data_mag[..n].to_vec();
    let model_mag = model_mag[..n].to_vec();
    let peak = data_mag.iter().fold(0.0f64, |m, v| m.max(*v)).max(1e-12);
    let mut plot: Plot = Plot::new()
        .theme(theme.plot_theme())
        .line(&r, &data_mag)
        .color(trace_color(theme, 0))
        .label("|χ(R)| data")
        .line(&r, &model_mag)
        .color(FIT_COLOR)
        .line_width(1.6)
        .label("fit")
        .into();
    if show_re {
        let re_d: Vec<f64> = result.data_chir_re.iter().take(n).copied().collect();
        let re_m: Vec<f64> = result.model_chir_re.iter().take(n).copied().collect();
        let r_d = r[..re_d.len()].to_vec();
        let r_m = r[..re_m.len()].to_vec();
        plot = plot
            .line(&r_d, &re_d)
            .color(trace_color(theme, 0))
            .line_width(1.0)
            .line_style(LineStyle::Dotted)
            .label("Re data")
            .line(&r_m, &re_m)
            .color(FIT_COLOR)
            .line_width(1.0)
            .line_style(LineStyle::Dotted)
            .label("Re fit")
            .into();
    }
    if show_paths && result.path_contributions.len() > 1 {
        // Each path sits on its own baseline below zero so the shells read
        // as a stack rather than a tangle.
        for (i, path) in result.path_contributions.iter().enumerate() {
            let offset = -peak * (0.35 + 0.3 * i as f64);
            let m: Vec<f64> = path.chir_mag.iter().take(n).map(|v| v + offset).collect();
            let x = r[..m.len()].to_vec();
            plot = plot
                .line(&x, &m)
                .color(trace_color(theme, i + 2))
                .line_width(1.0)
                .label(&path.label)
                .into();
        }
    }
    plot.legend_position(ruviz::core::LegendPosition::UpperRight)
        .xlabel(R_AXIS)
        .ylabel(chir_label(result.kweight))
}

/// q-space view: back-transformed data vs model over the fit's R-window.
pub fn build_fit_q(result: &xraytsubaki::prelude::FeffFitResult, theme: &Theme) -> Plot {
    let q = vecs(&result.q);
    let n = q
        .len()
        .min(result.data_chiq.len())
        .min(result.model_chiq.len());
    let q = q[..n].to_vec();
    let data: Vec<f64> = result.data_chiq.iter().take(n).copied().collect();
    let model: Vec<f64> = result.model_chiq.iter().take(n).copied().collect();
    let plot: Plot = Plot::new()
        .theme(theme.plot_theme())
        .line(&q, &data)
        .color(trace_color(theme, 0))
        .label("data")
        .line(&q, &model)
        .color(FIT_COLOR)
        .line_width(1.6)
        .label("fit")
        .into();
    plot.legend_position(ruviz::core::LegendPosition::UpperRight)
        .xlabel("q (Å⁻¹)")
        .ylabel(format!("Re χ(q) · kw {:.0}", result.kweight))
}

fn fit_data_chir_mag(result: &xraytsubaki::prelude::FeffFitResult) -> Vec<f64> {
    result
        .data_chir_re
        .iter()
        .zip(result.data_chir_im.iter())
        .map(|(re, im)| (re * re + im * im).sqrt())
        .collect()
}

/// Residual strip under the k-space fit: k-weighted (data - model).
pub fn build_fit_residual_k(result: &xraytsubaki::prelude::FeffFitResult, theme: &Theme) -> Plot {
    let k = vecs(&result.k);
    let kw = result.kweight;
    let res: Vec<f64> = result
        .data_chi
        .iter()
        .zip(result.model_chi.iter())
        .zip(k.iter())
        .map(|((d, m), kk)| (d - m) * kk.powf(kw))
        .collect();
    let plot: Plot = Plot::new()
        .theme(theme.plot_theme())
        .line(&k, &res)
        .line_width(1.0)
        .color(Color::from_gray(150))
        .into();
    plot.hline_styled(0.0, Color::from_gray(120), 0.8, LineStyle::Dashed)
        .xlabel(K_AXIS)
        .ylabel("residual")
}

/// Residual strip under the R-space fit: |chi(R)| data - model.
pub fn build_fit_residual_r(result: &xraytsubaki::prelude::FeffFitResult, theme: &Theme) -> Plot {
    let r = vecs(&result.r);
    let data_mag = fit_data_chir_mag(result);
    let model_mag = vecs(&result.model_chir_mag);
    let n = r.len().min(data_mag.len()).min(model_mag.len());
    let res: Vec<f64> = (0..n).map(|i| data_mag[i] - model_mag[i]).collect();
    let r = r[..n].to_vec();
    let plot: Plot = Plot::new()
        .theme(theme.plot_theme())
        .line(&r, &res)
        .line_width(1.0)
        .color(Color::from_gray(150))
        .into();
    plot.hline_styled(0.0, Color::from_gray(120), 0.8, LineStyle::Dashed)
        .xlabel(R_AXIS)
        .ylabel("residual")
}

/// LCF overlay: data, fit, residual (offset below) and the weighted
/// components stacked underneath.
pub fn build_lcf_plot(
    result: &xraytsubaki::prelude::LcfResult,
    xlabel: &str,
    ylabel: &str,
    theme: &Theme,
) -> Plot {
    let x = vecs(&result.x);
    let data = vecs(&result.data);
    let fit = vecs(&result.fit);
    let span = data.iter().fold(0.0f64, |m, v| m.max(v.abs())).max(1e-12);
    let mut plot: Plot = Plot::new()
        .theme(theme.plot_theme())
        .line(&x, &data)
        .color(trace_color(theme, 0))
        .label("data")
        .line(&x, &fit)
        .color(FIT_COLOR)
        .line_width(1.6)
        .label("fit")
        .into();
    for (i, (comp, weight)) in result.components.iter().zip(&result.weights).enumerate() {
        let y: Vec<f64> = comp
            .iter()
            .map(|v| v - span * 0.25 * (i + 1) as f64)
            .collect();
        let n = y.len().min(x.len());
        let (xs, ys) = (x[..n].to_vec(), y[..n].to_vec());
        plot = plot
            .line(&xs, &ys)
            .color(trace_color(theme, i + 2))
            .line_width(1.0)
            .label(format!("{} × {:.2}", weight.name, weight.weight))
            .into();
    }
    let n = result.components.len();
    let res: Vec<f64> = result
        .residual
        .iter()
        .map(|v| v - span * 0.25 * (n + 1) as f64)
        .collect();
    let n = res.len().min(x.len());
    let (xs, rs) = (x[..n].to_vec(), res[..n].to_vec());
    plot = plot
        .line(&xs, &rs)
        .color(Color::from_gray(150))
        .line_width(1.0)
        .line_style(LineStyle::Dashed)
        .label("residual")
        .into();
    plot.legend_position(ruviz::core::LegendPosition::UpperRight)
        .xlabel(xlabel)
        .ylabel(ylabel)
}

/// PCA target transform: data vs reconstruction with the residual below.
pub fn build_pca_plot(
    fit: &xraytsubaki::prelude::PcaFit,
    xlabel: &str,
    ylabel: &str,
    theme: &Theme,
) -> Plot {
    let x = vecs(&fit.x);
    let data = vecs(&fit.data);
    let recon = vecs(&fit.fit);
    let span = data.iter().fold(0.0f64, |m, v| m.max(v.abs())).max(1e-12);
    let res: Vec<f64> = fit.residual.iter().map(|v| v - span * 0.3).collect();
    let n = res.len().min(x.len());
    let (xs, rs) = (x[..n].to_vec(), res[..n].to_vec());
    let plot: Plot = Plot::new()
        .theme(theme.plot_theme())
        .line(&x, &data)
        .color(trace_color(theme, 0))
        .label("data")
        .line(&x, &recon)
        .color(FIT_COLOR)
        .line_width(1.6)
        .label(format!("{} components", fit.n_components))
        .line(&xs, &rs)
        .color(Color::from_gray(150))
        .line_width(1.0)
        .line_style(LineStyle::Dashed)
        .label("residual")
        .into();
    plot.legend_position(ruviz::core::LegendPosition::UpperRight)
        .xlabel(xlabel)
        .ylabel(ylabel)
}

/// Parameter-vs-frame trend. The moving cursor is a dynamic annotation owned
/// by the interactive plot session, so scrubbing does not rebuild this data.
pub fn build_trend(values: &[f64], ylabel: &str, theme: &Theme) -> Plot {
    let mut plot = Plot::new()
        .theme(theme.plot_theme())
        .xlabel("frame")
        .ylabel(ylabel);
    // Missing/failed frames remain real gaps: each contiguous finite run is
    // its own series, located at the true full-scan frame index.
    let mut start = 0;
    while start < values.len() {
        while start < values.len() && !values[start].is_finite() {
            start += 1;
        }
        let mut end = start;
        while end < values.len() && values[end].is_finite() {
            end += 1;
        }
        if start < end {
            let xs: Vec<f64> = (start..end).map(|index| index as f64).collect();
            let ys = &values[start..end];
            plot = if end - start == 1 {
                plot.scatter(&xs, &ys)
                    .color(Color::from_rgb(31, 119, 180))
                    .into()
            } else {
                plot.line(&xs, &ys)
                    .color(Color::from_rgb(31, 119, 180))
                    .into()
            };
        }
        start = end.saturating_add(1);
    }
    if values.is_empty() {
        plot
    } else {
        plot.xlim(0.0, values.len().saturating_sub(1).max(1) as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::{chik_label, chir_label, heatmap_y_extent, middle_truncate};

    #[test]
    fn middle_truncate_passes_short_names_through() {
        assert_eq!(middle_truncate("short.dat", 28), "short.dat");
        assert_eq!(middle_truncate("", 8), "");
    }

    #[test]
    fn middle_truncate_keeps_head_and_tail() {
        let out = middle_truncate("frame_000000000000123456.dat", 15);
        assert_eq!(out.chars().count(), 15);
        assert!(out.starts_with("frame_0"));
        assert!(out.ends_with("456.dat"));
        assert!(out.contains('…'));
    }

    #[test]
    fn middle_truncate_respects_char_boundaries() {
        let out = middle_truncate("αβγδεζηθικλμνξοπρστυ", 9);
        assert_eq!(out.chars().count(), 9);
    }

    #[test]
    fn chik_labels_common_weights() {
        assert_eq!(chik_label(0.0), "χ(k)");
        assert_eq!(chik_label(1.0), "k·χ(k)");
        assert_eq!(chik_label(2.0), "k²χ(k)");
        assert_eq!(chik_label(3.0), "k³χ(k)");
        assert_eq!(chik_label(4.0), "k^4 χ(k)");
        assert_eq!(chik_label(2.5), "k^2.5 χ(k)");
    }

    #[test]
    fn chir_label_tracks_kweight() {
        // |chi(R)| for k-weight n carries units of Å^-(n+1); the old hardcoded
        // Å⁻³ was only right for the default k-weight of 2.
        assert_eq!(chir_label(0.0), "|χ(R)| (Å⁻¹)");
        assert_eq!(chir_label(1.0), "|χ(R)| (Å⁻²)");
        assert_eq!(chir_label(2.0), "|χ(R)| (Å⁻³)");
        assert_eq!(chir_label(3.0), "|χ(R)| (Å⁻⁴)");
        assert_eq!(chir_label(4.0), "|χ(R)| (Å^-5)");
    }

    #[test]
    fn heatmap_extent_centers_endpoint_rows_on_scan_frames() {
        let scan_len = 1_000;
        let row_count = 192;
        let (ymin, ymax) = heatmap_y_extent(scan_len, row_count);
        let row_step = (ymax - ymin) / row_count as f64;

        assert!((ymin + row_step / 2.0).abs() < 1e-12);
        assert!((ymax - row_step / 2.0 - 999.0).abs() < 1e-12);
    }

    #[test]
    fn heatmap_extent_handles_short_and_degenerate_inputs() {
        assert_eq!(heatmap_y_extent(3, 3), (-0.5, 2.5));
        assert_eq!(heatmap_y_extent(1, 1), (-0.5, 0.5));
        assert_eq!(heatmap_y_extent(0, 0), (-0.5, 0.5));
    }
}
