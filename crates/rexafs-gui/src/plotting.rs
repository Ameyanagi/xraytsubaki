//! Builders turning a processed `XASSpectrum` into ruviz `Plot`s for the four
//! Explore quadrants: mu(E), normalized mu(E), k-weighted chi(k), |chi(R)|.
//!
//! ruviz 0.5 retains native rotated y labels and adds the interactive APIs used
//! by the operando plots. Card headers remain plot titles rather than serving as
//! a workaround for broken vertical text.

use rexafs::prelude::BackgroundMethod;
use rexafs::prelude::NormalizationMethod;
use rexafs::prelude::XASSpectrum;
use ruviz::core::LegendPosition;
use ruviz::data::{BatchUpdate, Observable};
use ruviz::plots::heatmap::{HeatmapConfig, HeatmapOrigin};
use ruviz::prelude::Plot;
use ruviz::render::{Color, LineStyle};

use crate::theme::Theme;

fn vecs(v: &nalgebra::DVector<f64>) -> Vec<f64> {
    v.iter().copied().collect()
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
    pub show_im: bool,
    /// AUTOBK background spline over mu(E) (Background stage).
    pub show_bkg: bool,
    /// Overlay the scaled derivative dμ/dE on the normalized plot (Athena's
    /// "normalized + scaled derivative").
    pub show_deriv: bool,
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
            show_im: false,
            show_bkg: false,
            show_deriv: false,
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

/// Identity of one series inside a quadrant. Together with its style it
/// forms the quadrant *structure*; the values behind it are reactive.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum SeriesKey {
    /// Data trace `i` (position in the compare set).
    Trace(usize),
    PreEdge,
    PostEdge,
    Bkg,
    Deriv,
    Kwin,
    Re,
    Im,
    /// k-weighted chi(k) drawn under chi(q).
    ChiKOnQ,
}

/// One series: identity + style (structure) and its current values.
pub struct SeriesSpec {
    pub key: SeriesKey,
    pub x: Vec<f64>,
    pub y: Vec<f64>,
    pub width: f32,
    pub color: Color,
    pub dashed: bool,
    pub label: Option<String>,
}

/// The observables behind one series of a live quadrant.
#[derive(Clone)]
pub struct SeriesSource {
    pub x: Observable<Vec<f64>>,
    pub y: Observable<Vec<f64>>,
}

/// Everything a quadrant shows, split into structure (labels, series
/// identities and styles, guide lines) and reactive values.
pub struct QuadrantSpec {
    pub title: String,
    pub xlabel: String,
    pub ylabel: String,
    pub series: Vec<SeriesSpec>,
    /// Static vertical guide lines: (x, color, width, dashed).
    pub vlines: Vec<(f64, Color, f32, bool)>,
    pub legend_columns: Option<usize>,
    pub grid: bool,
}

impl QuadrantSpec {
    /// Hash of the structure only (never the values). Two specs with equal
    /// keys can share one ruviz session: a refresh just replaces the
    /// observables. `salt` folds in host-side structure (figure size, theme).
    pub fn structure_key(&self, salt: u64) -> u64 {
        use std::hash::{DefaultHasher, Hash, Hasher};
        let mut h = DefaultHasher::new();
        salt.hash(&mut h);
        self.title.hash(&mut h);
        self.xlabel.hash(&mut h);
        self.ylabel.hash(&mut h);
        self.legend_columns.hash(&mut h);
        self.grid.hash(&mut h);
        for s in &self.series {
            s.key.hash(&mut h);
            s.width.to_bits().hash(&mut h);
            (s.color.r, s.color.g, s.color.b, s.color.a).hash(&mut h);
            s.dashed.hash(&mut h);
            s.label.hash(&mut h);
        }
        for (x, c, w, d) in &self.vlines {
            x.to_bits().hash(&mut h);
            (c.r, c.g, c.b, c.a).hash(&mut h);
            w.to_bits().hash(&mut h);
            d.hash(&mut h);
        }
        h.finish()
    }

    /// A ruviz plot whose series read from fresh observables (returned in
    /// series order, for later [`QuadrantSpec::apply`] calls).
    pub fn to_plot(&self, theme: &Theme) -> (Plot, Vec<SeriesSource>) {
        let mut plot = Plot::new()
            .theme(theme.plot_theme())
            .grid(self.grid)
            .xlabel(&self.xlabel)
            .ylabel(&self.ylabel);
        let mut sources = Vec::with_capacity(self.series.len());
        for s in &self.series {
            let src = SeriesSource {
                x: Observable::new(s.x.clone()),
                y: Observable::new(s.y.clone()),
            };
            let mut sb = plot
                .line_source(src.x.clone(), src.y.clone())
                .line_width(s.width)
                .color(s.color);
            if s.dashed {
                sb = sb.line_style(LineStyle::Dashed);
            }
            if let Some(label) = &s.label {
                sb = sb.label(label.clone());
            }
            plot = sb.into();
            sources.push(src);
        }
        for &(x, color, width, dashed) in &self.vlines {
            let style = if dashed {
                LineStyle::Dashed
            } else {
                LineStyle::Solid
            };
            plot = plot.vline_styled(x, color, width, style);
        }
        if let Some(columns) = self.legend_columns {
            plot = plot
                .legend_position(LegendPosition::OutsideLower)
                .legend_columns(columns);
        }
        (plot, sources)
    }

    /// Push this spec's values into an existing session's observables. All
    /// notifications are deferred to one batch so a render never sees a new
    /// x with an old y.
    /// Unchanged vectors are left alone, so a refresh that lands on the
    /// same values (a cache hit while dragging back and forth) costs a
    /// comparison and no re-raster.
    pub fn apply(self, sources: &[SeriesSource]) {
        let mut batch = BatchUpdate::new();
        for src in sources {
            batch.add(&src.x);
            batch.add(&src.y);
        }
        for (s, src) in self.series.into_iter().zip(sources) {
            if *src.x.read() != s.x {
                src.x.set(s.x);
            }
            if *src.y.read() != s.y {
                src.y.set(s.y);
            }
        }
        drop(batch);
    }
}

const TREND: Color = Color {
    r: 150,
    g: 150,
    b: 150,
    a: 255,
};
const GUIDE: Color = Color {
    r: 140,
    g: 140,
    b: 140,
    a: 255,
};
const E0_COLOR: Color = Color::ORANGE;
const PRE_COLOR: Color = Color {
    r: 90,
    g: 140,
    b: 200,
    a: 255,
};
const NORM_COLOR: Color = Color {
    r: 90,
    g: 180,
    b: 120,
    a: 255,
};
const BKG_COLOR: Color = Color {
    r: 232,
    g: 121,
    b: 47,
    a: 255,
};

/// One quadrant's data traces from per-trace (x, y) extractions. Inactive
/// traces draw first (thin), the active trace last (thick, on top); every
/// trace keeps the colour of its original position so the reordering never
/// recolors anything. Waterfall mode offsets successive traces by
/// `offset_frac` x the first trace's range. Returns the spec plus the
/// waterfall shift of the active (or first) trace so diagnostic overlays can
/// be aligned to the curve they annotate.
fn build_multi(
    traces: &[QuadTrace],
    view: &ViewOptions,
    theme: &Theme,
    in_plot_legend: bool,
    (title, xlabel, ylabel): (&str, &str, &str),
    extract: impl Fn(&XASSpectrum) -> Option<(Vec<f64>, Vec<f64>)>,
) -> (QuadrantSpec, f64) {
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
    // In the grid the shared GPUI legend strip identifies traces; in-plot
    // legends only when a quadrant is maximized.
    let with_legend = in_plot_legend && view.legend && n > 1 && n <= MAX_LEGEND_TRACES;
    let specs = series
        .into_iter()
        .map(|(i, trace, x, y)| SeriesSpec {
            key: SeriesKey::Trace(i),
            x,
            y,
            width: if trace.active && n > 1 { 2.2 } else { 1.4 },
            color: trace_color(theme, i),
            dashed: false,
            label: with_legend.then(|| middle_truncate(&trace.label, 24)),
        })
        .collect();
    (
        QuadrantSpec {
            title: title.to_string(),
            xlabel: xlabel.to_string(),
            ylabel: ylabel.to_string(),
            series: specs,
            vlines: Vec::new(),
            legend_columns: with_legend.then_some(n.min(4)),
            grid: view.grid,
        },
        active_shift,
    )
}

fn dashed(key: SeriesKey, x: Vec<f64>, y: Vec<f64>, color: Color, label: &str) -> SeriesSpec {
    SeriesSpec {
        key,
        x,
        y,
        width: 1.0,
        color,
        dashed: true,
        label: Some(label.to_string()),
    }
}

/// Normalization-check overlays for the active trace on the mu(E) plot:
/// dashed pre/post-edge trendlines, the background spline, the E0 line, and
/// the fit-window lines (pre-edge range muted, norm range in a second hue;
/// values are stored relative to E0). `shift` is the active trace's
/// waterfall offset so the trendlines sit on the curve they belong to.
fn add_mu_diagnostics(spec: &mut QuadrantSpec, sp: &XASSpectrum, view: &ViewOptions, shift: f64) {
    if view.show_pre
        && let (Some(energy), Some(pre)) = (sp.energy.as_ref(), sp.pre_edge())
    {
        let x = vecs(energy);
        let y: Vec<f64> = pre.iter().map(|v| v + shift).collect();
        spec.series
            .push(dashed(SeriesKey::PreEdge, x, y, TREND, "pre-edge"));
    }
    if view.show_post
        && let (Some(energy), Some(post)) = (sp.energy.as_ref(), sp.post_edge())
    {
        let x = vecs(energy);
        let y: Vec<f64> = post.iter().map(|v| v + shift).collect();
        spec.series
            .push(dashed(SeriesKey::PostEdge, x, y, TREND, "post-edge"));
    }
    if view.show_bkg
        && let (Some(energy), Some(BackgroundMethod::AUTOBK(autobk))) =
            (sp.energy.as_ref(), sp.background.as_ref())
        && let Some(bkg) = autobk.get_bkg()
    {
        let x = vecs(energy);
        let n = x.len().min(bkg.len());
        let y: Vec<f64> = bkg.iter().take(n).map(|v| v + shift).collect();
        spec.series.push(SeriesSpec {
            key: SeriesKey::Bkg,
            x: x[..n].to_vec(),
            y,
            width: 1.5,
            color: BKG_COLOR,
            dashed: false,
            label: Some("background μ₀(E)".to_string()),
        });
    }
    let e0 = sp.e0();
    if view.show_e0
        && let Some(e0) = e0
    {
        spec.vlines.push((e0, E0_COLOR, 1.2, true));
    }
    if view.show_ranges
        && let (Some(e0), Some(NormalizationMethod::PrePostEdge(ppe))) =
            (e0, sp.normalization.as_ref())
    {
        for (value, color) in [
            (ppe.get_pre_edge_start(), PRE_COLOR),
            (ppe.get_pre_edge_end(), PRE_COLOR),
            (ppe.get_norm_start(), NORM_COLOR),
            (ppe.get_norm_end(), NORM_COLOR),
        ] {
            if let Some(rel) = value {
                spec.vlines.push((e0 + rel, color, 1.0, true));
            }
        }
    }
}

/// Specs for all five Explore quadrants (mu(E), normalized mu(E), k-weighted
/// chi(k), |chi(R)|, chi(q)) for a set of traces. `in_plot_legend` opts into
/// ruviz legends (maximized quadrant); the grid uses the shared GPUI strip.
pub fn build_quadrant_specs(
    traces: &[QuadTrace],
    view: &ViewOptions,
    theme: &Theme,
    in_plot_legend: bool,
) -> [QuadrantSpec; 5] {
    let kw = traces
        .iter()
        .find(|t| t.active)
        .or_else(|| traces.first())
        .and_then(|t| t.sp.kweight().copied())
        .unwrap_or(2.0);

    let (mut mu_e, mu_shift) = build_multi(
        traces,
        view,
        theme,
        in_plot_legend,
        ("μ(E)", "Energy (eV)", "μ(E)"),
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
        (norm_label, "Energy (eV)", norm_label),
        move |sp| {
            let y = if flat {
                sp.flat().or_else(|| sp.norm())
            } else {
                sp.norm().or_else(|| sp.flat())
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
        (&chik_label, K_AXIS, &chik_label),
        |sp| {
            Some((
                sp.k()
                    .map(nalgebra::DVector::from_column_slice)
                    .map(|v| vecs(&v))?,
                sp.chi_kweighted().map(|v| vecs(&v))?,
            ))
        },
    );
    let chir_title = match (view.show_re, view.show_im) {
        (true, true) => "|χ(R)| + Re + Im",
        (true, false) => "|χ(R)| + Re",
        (false, true) => "|χ(R)| + Im",
        _ => "|χ(R)|",
    };
    let (mut chi_r, chir_shift) = build_multi(
        traces,
        view,
        theme,
        in_plot_legend,
        (chir_title, R_AXIS, &chir_label(kw)),
        |sp| {
            let r = sp.r().map(|v| vecs(&v))?;
            let m = sp.chir_mag().map(|v| vecs(&v))?;
            let n = r.len().min(m.len());
            Some((r[..n].to_vec(), m[..n].to_vec()))
        },
    );

    if let Some(active) = traces.iter().find(|t| t.active).or_else(|| traces.first()) {
        add_mu_diagnostics(&mut mu_e, &active.sp, view, mu_shift);
        if view.show_e0
            && let Some(e0) = active.sp.e0()
        {
            norm.vlines.push((e0, E0_COLOR, 1.2, true));
        }
        if view.show_deriv
            && let (Some(e), Some(n)) = (
                active.sp.energy.as_ref(),
                active.sp.norm().or_else(|| active.sp.flat()),
            )
        {
            // dμ/dE scaled so its peak reaches half the edge, as Athena does.
            let e = vecs(e);
            let n = vecs(&n);
            let m = e.len().min(n.len());
            let d: Vec<f64> = (0..m)
                .map(|i| {
                    let a = i.saturating_sub(1);
                    let b = (i + 1).min(m - 1);
                    let de = e[b] - e[a];
                    if de > 0.0 { (n[b] - n[a]) / de } else { 0.0 }
                })
                .collect();
            let peak = d.iter().fold(0.0f64, |acc, v| acc.max(v.abs())).max(1e-12);
            let y: Vec<f64> = d.iter().map(|v| v / peak * 0.5).collect();
            norm.series.push(dashed(
                SeriesKey::Deriv,
                e[..m].to_vec(),
                y,
                GUIDE,
                "dμ/dE (scaled)",
            ));
        }
        // FT window diagnostics on chi(k), scaled to the data amplitude and
        // shifted onto the active trace in waterfall mode.
        if view.show_kwin
            && let (Some(k), Some(kwin), Some(chi)) = (
                active.sp.k().map(nalgebra::DVector::from_column_slice),
                active.sp.kwin(),
                active.sp.chi_kweighted(),
            )
        {
            let peak = chi.iter().fold(0.0f64, |m, v| m.max(v.abs())).max(1e-12);
            let x = vecs(&k);
            let n = x.len().min(kwin.len());
            let y: Vec<f64> = kwin.iter().take(n).map(|w| w * peak + chik_shift).collect();
            chi_k
                .series
                .push(dashed(SeriesKey::Kwin, x[..n].to_vec(), y, TREND, "window"));
        }
        if view.show_krange
            && let Some(xftf) = active.sp.xftf.as_ref()
        {
            for v in [xftf.kmin, xftf.kmax].into_iter().flatten() {
                chi_k.vlines.push((v, PRE_COLOR, 1.0, true));
            }
        }
        // Re part of chi(R) for phase-agreement checks (doc: "|χ(R)| (+Re
        // part toggle)"), aligned to the active trace's waterfall offset.
        if view.show_re
            && let (Some(r), Some(re)) = (active.sp.r(), active.sp.chir_real())
        {
            let r = vecs(&r);
            let n = r.len().min(re.len());
            let y: Vec<f64> = re.iter().take(n).map(|v| v + chir_shift).collect();
            chi_r
                .series
                .push(dashed(SeriesKey::Re, r[..n].to_vec(), y, TREND, "Re"));
        }
        if view.show_im
            && let (Some(r), Some(im)) = (active.sp.r(), active.sp.chir_imag())
        {
            let n = r.len().min(im.len());
            let y = im.iter().take(n).map(|v| v + chir_shift).collect();
            chi_r.series.push(dashed(
                SeriesKey::Im,
                r.iter().take(n).copied().collect(),
                y,
                FIT_COLOR,
                "Im",
            ));
        }
    }
    let (mut chi_q, chiq_shift) = build_multi(
        traces,
        view,
        theme,
        in_plot_legend,
        ("χ(q)", "q (Å⁻¹)", "χ(q)"),
        |sp| {
            let q = sp.q().map(|v| vecs(&v))?;
            let c = sp.chiq().map(|v| vecs(&v))?;
            let n = q.len().min(c.len());
            Some((q[..n].to_vec(), c[..n].to_vec()))
        },
    );
    if let Some(active) = traces.iter().find(|t| t.active).or_else(|| traces.first())
        && let (Some(k), Some(chi)) = (
            active.sp.k().map(nalgebra::DVector::from_column_slice),
            active.sp.chi_kweighted(),
        )
    {
        let x = vecs(&k);
        let n = x.len().min(chi.len());
        let y: Vec<f64> = chi.iter().take(n).map(|v| v + chiq_shift).collect();
        chi_q.series.push(dashed(
            SeriesKey::ChiKOnQ,
            x[..n].to_vec(),
            y,
            TREND,
            &chik_label,
        ));
    }
    [mu_e, norm, chi_k, chi_r, chi_q]
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
    result: &rexafs::prelude::FeffFitResult,
    theme: &Theme,
    show_paths: bool,
    highlight: Option<&str>,
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
    if let Some(h) = highlight
        && let Some(path) = result.path_contributions.iter().find(|p| p.label == h)
    {
        let y = weight(&path.chi);
        plot = plot
            .line(&k, &y)
            .color(HOVER_COLOR)
            .line_width(2.2)
            .label(format!("{h} (hover)"))
            .into();
    }
    plot.legend_position(ruviz::core::LegendPosition::UpperRight)
        .xlabel(K_AXIS)
        .ylabel(chik_label(kw))
}

/// Hovered-path preview trace (amber).
const HOVER_COLOR: Color = Color {
    r: 224,
    g: 179,
    b: 90,
    a: 255,
};

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
    result: &rexafs::prelude::FeffFitResult,
    theme: &Theme,
    show_paths: bool,
    show_re: bool,
    show_im: bool,
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
    if show_im {
        for (values, color, label) in [
            (&result.data_chir_im, trace_color(theme, 0), "Im data"),
            (&result.model_chir_im, FIT_COLOR, "Im fit"),
        ] {
            let y: Vec<_> = values.iter().take(n).copied().collect();
            plot = plot
                .line(&r[..y.len()], &y)
                .color(color)
                .line_width(1.2)
                .line_style(LineStyle::Dashed)
                .label(label)
                .into();
        }
    }
    if show_paths && !result.path_contributions.is_empty() {
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
        .ylabel(if show_re || show_im {
            chir_label(result.kweight).replace("|χ(R)|", "χ(R)")
        } else {
            chir_label(result.kweight)
        })
}

/// q-space view: back-transformed data vs model over the fit's R-window.
pub fn build_fit_q(result: &rexafs::prelude::FeffFitResult, theme: &Theme) -> Plot {
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

fn fit_data_chir_mag(result: &rexafs::prelude::FeffFitResult) -> Vec<f64> {
    result
        .data_chir_re
        .iter()
        .zip(result.data_chir_im.iter())
        .map(|(re, im)| (re * re + im * im).sqrt())
        .collect()
}

/// Residual strip under the k-space fit: k-weighted (data - model).
pub fn build_fit_residual_k(result: &rexafs::prelude::FeffFitResult, theme: &Theme) -> Plot {
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
        .xlabel("")
        .ylabel("")
        .major_ticks_y(3)
}

/// Residual strip under the R-space fit: |chi(R)| data - model.
pub fn build_fit_residual_r(result: &rexafs::prelude::FeffFitResult, theme: &Theme) -> Plot {
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
        .xlabel("")
        .ylabel("")
        .major_ticks_y(3)
}

/// LCF overlay: data, fit, residual (offset below) and the weighted
/// components stacked underneath.
pub fn build_lcf_plot(
    result: &rexafs::prelude::LcfResult,
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
    fit: &rexafs::prelude::PcaFit,
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
pub fn build_trend(values: &[f64], frames: &[f64], ylabel: &str, theme: &Theme) -> Plot {
    let mut plot = Plot::new()
        .theme(theme.plot_theme())
        .xlabel("frame")
        .ylabel(ylabel);
    // Missing/failed frames remain real gaps: each contiguous finite run is
    // its own series, located at its true frame position.
    let n = values.len().min(frames.len());
    let mut start = 0;
    while start < n {
        while start < n && !values[start].is_finite() {
            start += 1;
        }
        let mut end = start;
        while end < n && values[end].is_finite() {
            end += 1;
        }
        if start < end {
            let xs: Vec<f64> = frames[start..end].to_vec();
            let ys: Vec<f64> = values[start..end].to_vec();
            plot = if end - start == 1 {
                plot.scatter(&xs, &ys).color(trace_color(theme, 0)).into()
            } else {
                plot.line(&xs, &ys).color(trace_color(theme, 0)).into()
            };
        }
        start = end.saturating_add(1);
    }
    let xmax = frames.iter().copied().fold(0.0f64, f64::max).max(1.0);
    if n == 0 { plot } else { plot.xlim(0.0, xmax) }
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
