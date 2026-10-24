//! Layout helpers for publication figures: legend placement, x-tick density,
//! Typst-safe user labels and the stacked fit + residual figure.
//!
//! These are rexafs presentation choices layered on ruviz 0.15. They change
//! where text and panels are drawn, never the plotted arrays.
use super::{FigureData, FigureOptions, FigureSeries, physical_svg_size};
use ruviz::core::{Image, LegendPosition, MarginConfig};
use ruviz::prelude::{Color, LineStyle, Plot};
use serde::{Deserialize, Serialize};

/// Where a publication figure draws its legend. `FigureOptions::legend`
/// (Off) still hides it entirely.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum LegendPlacement {
    /// Recommended default: the inside corner whose estimated legend box covers
    /// the least of the plotted curves, or above the plot when every corner covers
    /// data (see [`auto_legend`]).
    #[default]
    Auto,
    /// Inside the plot, upper-left corner.
    UpperLeft,
    /// Inside the plot, upper-right corner.
    UpperRight,
    /// Inside the plot, lower-left corner.
    LowerLeft,
    /// Inside the plot, lower-right corner.
    LowerRight,
    /// Outside the plot area, to the right; the plot area narrows to make room.
    /// The fit + residual figure keeps its panels aligned by using Auto instead.
    Outside,
}

impl LegendPlacement {
    /// Every placement in display order.
    pub const ALL: [Self; 6] = [
        Self::Auto,
        Self::UpperLeft,
        Self::UpperRight,
        Self::LowerLeft,
        Self::LowerRight,
        Self::Outside,
    ];

    /// Short chip label in sentence case.
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::UpperLeft => "Upper left",
            Self::UpperRight => "Upper right",
            Self::LowerLeft => "Lower left",
            Self::LowerRight => "Lower right",
            Self::Outside => "Outside",
        }
    }

    pub(super) fn position(self) -> LegendPosition {
        match self {
            Self::Auto => LegendPosition::Best,
            Self::UpperLeft => LegendPosition::UpperLeft,
            Self::UpperRight => LegendPosition::UpperRight,
            Self::LowerLeft => LegendPosition::LowerLeft,
            Self::LowerRight => LegendPosition::LowerRight,
            Self::Outside => LegendPosition::OutsideRight,
        }
    }
}

/// Canvas and text sizes used to estimate where an automatic legend fits.
pub(super) struct LegendFrame {
    /// Canvas width and height in inches.
    pub size_in: (f64, f64),
    /// Legend and tick-label font sizes in points.
    pub legend_pt: f64,
    pub tick_pt: f64,
    /// Whether a title takes space above the plot area.
    pub title: bool,
    /// Whether Auto may fall back to Outside when every corner covers data.
    pub outside_ok: bool,
}

/// Inside corners in preference order when overlaps tie (matplotlib's order).
const CORNERS: [LegendPlacement; 4] = [
    LegendPlacement::UpperRight,
    LegendPlacement::UpperLeft,
    LegendPlacement::LowerRight,
    LegendPlacement::LowerLeft,
];
/// Largest share of the curve samples a legend corner may cover before Auto
/// gives up on the inside corners (rexafs choice: 0.5 %).
const OVERLAP_LIMIT: f64 = 0.005;

/// Resolve the Auto legend placement for the given visible curves.
///
/// rexafs estimates the legend box from the legend font size, the number of
/// entries and the longest label (about 0.5 em per character plus 6 em for
/// the line sample and padding; 1.7 em per entry), converts it to a fraction
/// of an estimated plot area, and places it in each inside corner. Every curve
/// is resampled along its line in axis-fraction coordinates (steps of 0.005,
/// using the visible x and y limits), and the corner whose box holds the
/// smallest share of those samples wins. If every corner covers more than
/// 0.5 % of the curve, the result is Outside when allowed (drawn above the
/// plot by `FigureData::panel`, not to the right); otherwise it
/// takes the least-covered corner. ruviz's own "best" placement also scores
/// edge and centre positions and can choose the middle of a rising edge,
/// which is why rexafs resolves Auto itself.
pub(super) fn auto_legend(
    series: &[&FigureSeries],
    xlim: Option<(f64, f64)>,
    ylim: Option<(f64, f64)>,
    frame: &LegendFrame,
) -> LegendPlacement {
    let extent = |values: &dyn Fn(&FigureSeries) -> &[f64]| {
        series
            .iter()
            .flat_map(|s| values(s).iter().copied())
            .filter(|v| v.is_finite())
            .fold(None, |range: Option<(f64, f64)>, v| {
                Some(range.map_or((v, v), |(lo, hi)| (lo.min(v), hi.max(v))))
            })
    };
    let (Some((x0, x1)), Some((y0, y1))) = (
        xlim.or_else(|| extent(&|s| &s.x)),
        ylim.or_else(|| extent(&|s| &s.y)),
    ) else {
        return LegendPlacement::UpperRight;
    };
    if x1 <= x0 || y1 <= y0 {
        return LegendPlacement::UpperRight;
    }
    let em = frame.legend_pt;
    let chars = series
        .iter()
        .map(|s| s.label.chars().count())
        .max()
        .unwrap_or(0) as f64;
    let (width_in, height_in) = frame.size_in;
    let plot_w = (width_in * 72. - 8. * frame.tick_pt).max(width_in * 36.);
    let title = if frame.title { 2.2 * em } else { 0. };
    let plot_h = (height_in * 72. - 5. * frame.tick_pt - title).max(height_in * 36.);
    // Box plus the gap ruviz leaves between the legend and the axes.
    let box_w = ((chars * 0.5 + 6. + 0.5) * em / plot_w).min(1.);
    let box_h = ((series.len() as f64 * 1.7 + 0.6 + 0.5) * em / plot_h).min(1.);

    let mut samples = Vec::new();
    for s in series {
        let points: Vec<(f64, f64)> =
            s.x.iter()
                .zip(&s.y)
                .filter(|(x, y)| x.is_finite() && y.is_finite())
                .map(|(x, y)| ((x - x0) / (x1 - x0), (y - y0) / (y1 - y0)))
                .collect();
        samples.extend(points.first().copied());
        for pair in points.windows(2) {
            let ((ax, ay), (bx, by)) = (pair[0], pair[1]);
            let steps = ((bx - ax).hypot(by - ay) / 0.005).ceil().clamp(1., 1000.) as usize;
            samples.extend((1..=steps).map(|i| {
                let t = i as f64 / steps as f64;
                (ax + (bx - ax) * t, ay + (by - ay) * t)
            }));
        }
    }
    samples.retain(|(x, y)| (0. ..=1.).contains(x) && (0. ..=1.).contains(y));
    if samples.is_empty() {
        return LegendPlacement::UpperRight;
    }
    let overlap = |corner: LegendPlacement| {
        let left = matches!(
            corner,
            LegendPlacement::UpperLeft | LegendPlacement::LowerLeft
        );
        let upper = matches!(
            corner,
            LegendPlacement::UpperLeft | LegendPlacement::UpperRight
        );
        let inside = samples
            .iter()
            .filter(|(x, y)| {
                (if left { *x <= box_w } else { *x >= 1. - box_w })
                    && (if upper { *y >= 1. - box_h } else { *y <= box_h })
            })
            .count();
        inside as f64 / samples.len() as f64
    };
    let (best, share) = CORNERS
        .into_iter()
        .map(|corner| (corner, overlap(corner)))
        .fold((CORNERS[0], f64::INFINITY), |best, next| {
            if next.1 < best.1 { next } else { best }
        });
    if share > OVERLAP_LIMIT && frame.outside_ok {
        LegendPlacement::Outside
    } else {
        best
    }
}

/// Approximate width of one tick-label character (digits, sign, point) in em.
/// Arial digits are 0.556 em wide; 0.6 em leaves a small safety margin.
const CHAR_EM: f64 = 0.6;
/// Minimum blank space between neighbouring tick labels, in em.
const TICK_GAP_EM: f64 = 1.0;

/// Choose the x-axis tick target (ruviz accepts 3–10) so neighbouring tick
/// labels do not overlap at the figure's width and tick font size.
///
/// The plot area is estimated as the canvas width minus about eight tick-label
/// font sizes for the y label, y tick labels and padding. Starting from ruviz's
/// default of 10, the target is lowered until the spacing between ruviz's own
/// "nice" ticks exceeds the widest formatted label plus a one-em gap. At the
/// 6.4 in default size this keeps ruviz's ticks; a 3.3 in, 9 pt XANES figure
/// drops from 10 eV to 20 eV spacing. This is a rexafs estimate, not a
/// measurement of the rendered text.
pub(crate) fn x_tick_count(width_in: f64, tick_pt: f64, (min, max): (f64, f64)) -> usize {
    let plot_pt = (width_in * 72. - 8. * tick_pt).max(width_in * 36.);
    if max.partial_cmp(&min) != Some(std::cmp::Ordering::Greater) || !plot_pt.is_finite() {
        return 10;
    }
    for target in (3..=10).rev() {
        let ticks: Vec<f64> = ruviz::axes::generate_ticks(min, max, target)
            .into_iter()
            .filter(|t| (min..=max).contains(t))
            .collect();
        if ticks.len() < 2 {
            return target;
        }
        let widest = ruviz::axes::format_tick_labels(&ticks)
            .iter()
            .map(|label| label.chars().count())
            .max()
            .unwrap_or(0) as f64;
        let spacing = plot_pt * (ticks[1] - ticks[0]) / (max - min);
        if spacing >= (widest * CHAR_EM + TICK_GAP_EM) * tick_pt {
            return target;
        }
    }
    3
}

/// Turn a user-typed title or axis label into Typst markup that shows the text
/// literally, while keeping `$…$` spans as Typst math.
///
/// Typst treats `# _ * [ ] @ < > \ ` and backticks as markup, so a label such as
/// `Sample #2 [Ni_foil]` would otherwise be reinterpreted or fail to compile.
/// Text outside dollar signs is wrapped as a literal string; each pair of
/// dollar signs and its contents pass through unchanged, so `$k^2 chi(k)$`
/// still renders as math. `\$` gives a literal dollar sign, and an unpaired
/// `$` is shown literally.
pub(crate) fn typst_label(text: &str) -> String {
    let mut output = String::new();
    let mut literal = String::new();
    let flush = |literal: &mut String, output: &mut String| {
        if !literal.is_empty() {
            output.push_str(&ruviz::render::typst_text::literal_text_snippet(literal));
            literal.clear();
        }
    };
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        if let Some(after) = rest.strip_prefix("\\$") {
            literal.push('$');
            rest = after;
        } else if c == '$'
            && let Some(end) = closing_dollar(&rest[1..])
        {
            flush(&mut literal, &mut output);
            output.push_str(&rest[..end + 2]);
            rest = &rest[end + 2..];
        } else {
            literal.push(c);
            rest = &rest[c.len_utf8()..];
        }
    }
    flush(&mut literal, &mut output);
    output
}

/// Byte offset of the next `$` not preceded by a backslash.
fn closing_dollar(text: &str) -> Option<usize> {
    let mut escaped = false;
    for (index, c) in text.char_indices() {
        match c {
            '$' if !escaped => return Some(index),
            '\\' => escaped = !escaped,
            _ => escaped = false,
        }
    }
    None
}

/// Fixed `[left, right, top, bottom]` panel margins, in inches.
pub(super) type Frame = [f64; 4];

/// Font sizes and paddings of a panel, in points, read from its ruviz config.
/// They feed rexafs's margin estimates, which assume tick labels about
/// [`CHAR_EM`] wide per character and axis titles 1.5 font sizes tall.
pub(super) struct Metrics {
    tick: f64,
    label: f64,
    title: f64,
    tick_pad: f64,
    label_pad: f64,
    title_pad: f64,
}

impl Metrics {
    pub(super) fn of(plot: &Plot) -> Self {
        let config = plot.get_config();
        Self {
            tick: config.typography.tick_size() as f64,
            label: config.typography.label_size() as f64,
            title: config.typography.title_size() as f64,
            tick_pad: config.spacing.tick_pad as f64,
            label_pad: config.spacing.label_pad as f64,
            title_pad: config.spacing.title_pad as f64,
        }
    }

    /// Axis-title font size in points (composed figures reuse it for their
    /// panel labels).
    pub(super) fn label_pt(&self) -> f64 {
        self.label
    }

    /// Tick-label font size in points.
    pub(super) fn tick_pt(&self) -> f64 {
        self.tick
    }

    /// Left margin holding y tick labels `chars` characters wide (at least
    /// three) and the rotated y-axis title.
    pub(super) fn left(&self, chars: usize) -> f64 {
        (chars.max(3) as f64 * CHAR_EM * self.tick
            + self.tick_pad
            + 1.5 * self.label
            + self.label_pad
            + 4.)
            / 72.
    }

    /// Right margin holding half of the last x tick label.
    pub(super) fn right(&self, chars: usize) -> f64 {
        ((chars as f64 * CHAR_EM * self.tick / 2. + 4.) / 72.).max(0.12)
    }

    /// Bottom margin holding the x tick labels and the x-axis title.
    pub(super) fn bottom(&self) -> f64 {
        (1.3 * self.tick + self.tick_pad + 1.5 * self.label + self.label_pad + 4.) / 72.
    }

    /// Top margin: room for a title, or a thin edge without one.
    pub(super) fn top(&self, title: bool) -> f64 {
        if title {
            (1.4 * self.title + self.title_pad + 4.) / 72.
        } else {
            0.1
        }
    }
}

/// Widest label, in characters, of ruviz's `ticks`-target ticks for a range.
pub(super) fn tick_chars(range: Option<(f64, f64)>, ticks: usize) -> Option<usize> {
    range.map(|(lo, hi)| widest_label(&ruviz::axes::generate_ticks(lo, hi, ticks)))
}

/// Share of the canvas height given to the residual strip (rexafs choice).
pub(crate) const RESIDUAL_FRACTION: f64 = 0.28;

/// The two built panels of a fit + residual figure, before their margins are
/// fixed. `frame` holds the margins this figure needs on its own; a composed
/// multi-panel figure can widen them so that neighbouring panels line up.
pub(super) struct FitResidual {
    pub(super) top: Plot,
    pub(super) strip: Plot,
    top_px: f64,
    total_px: f64,
    dpi: f64,
    metrics: Metrics,
    pub frame: Frame,
}

/// Build the fit panel above a residual strip that shares its x axis.
///
/// Both panels are separate ruviz plots with identical fixed left and right
/// margins, so their plot areas and x ticks line up. The upper panel has no
/// x label, and its bottom margin is smaller than ruviz's tick padding, which
/// places its x tick labels outside its canvas. The strip is symmetric about
/// zero, carries a zero line and the x label. [`FitResidual::render`] draws
/// them.
pub(super) fn fit_residual_panels(
    data: &FigureData,
    residual: &[FigureSeries],
    options: &FigureOptions,
) -> Result<FitResidual, String> {
    options.validate()?;
    let (width, height, dpi) = options.dimensions();
    let total_px = (height * dpi).round();
    let top_px = (total_px * (1. - RESIDUAL_FRACTION)).round();
    let mut top_options = FigureOptions {
        // Panel heights are set after building: a short strip can be under
        // the 1-inch minimum that `FigureOptions::validate` enforces.
        width: Some(width),
        height: Some(height),
        xlabel: None,
        ..options.clone()
    };
    if top_options.legend_position == LegendPlacement::Outside {
        top_options.legend_position = LegendPlacement::Auto;
    }
    let top_data = FigureData {
        residual: None,
        ..data.clone()
    };
    let strip_data = FigureData {
        ylabel: "Residual".into(),
        math_ylabel: "Residual".into(),
        series: residual.to_vec(),
        normalized_series: None,
        guides: vec![],
        default_xlim: data.xlim(options),
        residual: None,
        ..data.clone()
    };
    let strip_options = FigureOptions {
        xlabel: options.xlabel.clone(),
        title: None,
        ylabel: None,
        xmin: None,
        xmax: None,
        ymin: None,
        ymax: None,
        legend: false,
        guides: false,
        hidden: Default::default(),
        shown: Default::default(),
        normalized: false,
        ..top_options.clone()
    };
    let top_ylim = options
        .ymin
        .zip(options.ymax)
        .or_else(|| top_data.visible_ylim(&top_options));
    let half = strip_data
        .visible_ylim(&strip_options)
        .map(|(lo, hi)| lo.abs().max(hi.abs()))
        .filter(|v| *v > 0.)
        .unwrap_or(1.);
    let top = top_data.panel(&top_options, true)?;
    let strip = strip_data
        .panel(&strip_options, false)?
        .ylim(-half, half)
        .major_ticks_y(3)
        .hline_styled(0., Color::from_gray(140), 0.8, LineStyle::Solid);

    // Shared horizontal margins, from the wider y tick labels of the two panels.
    let metrics = Metrics::of(&top);
    let ytick_chars = [tick_chars(top_ylim, 10), tick_chars(Some((-half, half)), 3)]
        .into_iter()
        .flatten()
        .max()
        .unwrap_or(5);
    let xtick_chars = tick_chars(data.xlim(options), 10).unwrap_or(4);
    let frame = [
        metrics.left(ytick_chars),
        metrics.right(xtick_chars),
        metrics.top(options.title.is_some()),
        metrics.bottom(),
    ];
    Ok(FitResidual {
        top,
        strip,
        top_px,
        total_px,
        dpi,
        metrics,
        frame,
    })
}

impl FitResidual {
    /// Render both panels with the given margins: `frame[2]` is the upper
    /// panel's top margin and `frame[3]` the strip's bottom margin. Returns
    /// the stacked RGBA image and an SVG that nests both panel SVGs.
    pub(super) fn render(
        self,
        [left, right, top_margin, bottom]: Frame,
    ) -> Result<(Image, String), String> {
        let dpi = self.dpi;
        // Below ruviz's tick padding, so the upper panel's x tick labels fall
        // outside its canvas and the two panels sit close together.
        let gap = 0.5 * self.metrics.tick_pad / 72.;
        let strip_height = (self.total_px - self.top_px) / dpi;
        // A short strip cannot fit "Residual" along its plot area; abbreviate.
        let strip_plot_pt = (strip_height - 0.06 - bottom) * 72.;
        let strip = self
            .strip
            .ylabel(if strip_plot_pt >= 8. * CHAR_EM * self.metrics.label {
                "Residual"
            } else {
                "Res."
            });
        let top = panel_frame(self.top, self.top_px / dpi, [left, right, top_margin, gap]);
        let strip = panel_frame(strip, strip_height, [left, right, 0.06, bottom]);

        let (upper, lower) = (
            top.render().map_err(|e| e.to_string())?,
            strip.render().map_err(|e| e.to_string())?,
        );
        if upper.width != lower.width {
            return Err("The fit and residual panels rendered at different widths.".into());
        }
        let mut pixels = upper.pixels;
        pixels.extend_from_slice(&lower.pixels);
        let (width, height) = (upper.width, upper.height + lower.height);
        let (top_svg, strip_svg) = (
            top.render_to_svg().map_err(|e| e.to_string())?,
            strip.render_to_svg().map_err(|e| e.to_string())?,
        );
        let svg = nest_svgs(
            &[
                (&top_svg, "fit", 0, 0),
                (&strip_svg, "residual", 0, upper.height),
            ],
            "",
            (width, height),
        );
        Ok((Image::new(width, height, pixels), svg))
    }
}

/// Render the fit panel above a residual strip that shares its x axis (see
/// [`fit_residual_panels`]). Returns PNG bytes and an SVG that nests both
/// panel SVGs.
pub(super) fn fit_residual(
    data: &FigureData,
    residual: &[FigureSeries],
    options: &FigureOptions,
) -> Result<(Vec<u8>, String), String> {
    let panels = fit_residual_panels(data, residual, options)?;
    let (dpi, frame) = (panels.dpi, panels.frame);
    let (image, svg) = panels.render(frame)?;
    let png = image
        .encode_png_with_dpi(dpi as f32)
        .map_err(|e| e.to_string())?;
    Ok((
        png,
        physical_svg_size(svg, (image.width, image.height), dpi),
    ))
}

fn widest_label(ticks: &[f64]) -> usize {
    ruviz::axes::format_tick_labels(ticks)
        .iter()
        .map(|label| label.chars().count())
        .max()
        .unwrap_or(0)
}

/// Set a panel's height and fixed `[left, right, top, bottom]` margins, all in
/// inches. The config is edited directly because `Plot::size` clamps heights
/// below 1 inch, which a residual strip may need.
pub(super) fn panel_frame(plot: Plot, height: f64, [left, right, top, bottom]: Frame) -> Plot {
    let mut config = plot.get_config().clone();
    config.figure.height = height as f32;
    config.margins = MarginConfig::Fixed {
        left: left as f32,
        right: right as f32,
        top: top as f32,
        bottom: bottom as f32,
    };
    plot.plot_config(config)
}

/// Nest panel SVGs in one SVG document of `size` pixels. Each part is
/// `(svg, id prefix, x, y)`, with its offset in pixels. Element IDs get the
/// per-panel prefix so glyph and clip definitions from the separate
/// Typst/ruviz documents cannot collide. `extra` is appended last, so it draws
/// above every panel; composed figures put their panel labels there.
///
/// The result keeps pixel `width`/`height`, so it can itself be nested; give
/// a finished figure its printed size with [`physical_svg_size`].
pub(super) fn nest_svgs(
    parts: &[(&str, &str, u32, u32)],
    extra: &str,
    (width, height): (u32, u32),
) -> String {
    let nest = |svg: &str, prefix: &str, x: u32, y: u32| {
        let body = &svg[svg.find("<svg").unwrap_or(0)..];
        body.replacen("<svg", &format!(r#"<svg x="{x}" y="{y}""#), 1)
            .replace(r#"id=""#, &format!(r#"id="{prefix}-"#))
            .replace("url(#", &format!("url(#{prefix}-"))
            .replace(r##"href="#"##, &format!(r##"href="#{prefix}-"##))
    };
    let body: String = parts
        .iter()
        .map(|(svg, prefix, x, y)| nest(svg, prefix, *x, *y))
        .collect();
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{width}" height="{height}">{body}{extra}</svg>"#,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_labels_are_literal_outside_math() {
        assert_eq!(typst_label(""), "");
        assert_eq!(typst_label("Energy (eV)"), "#text(\"Energy (eV)\")");
        assert_eq!(
            typst_label(r#"Ni #2 [foil] *x* _y_ @ref <a> \ "q""#),
            r#"#text("Ni #2 [foil] *x* _y_ @ref <a> \\ \"q\"")"#
        );
        assert_eq!(
            typst_label("Copper · $k^2 chi(k)$ (Å)"),
            "#text(\"Copper · \")$k^2 chi(k)$#text(\" (Å)\")"
        );
        assert_eq!(typst_label("$mu(E)$"), "$mu(E)$");
        assert_eq!(typst_label("Cost $5"), "#text(\"Cost $5\")");
        assert_eq!(typst_label(r"\$5 and \$6"), "#text(\"$5 and $6\")");
        assert_eq!(typst_label(r"$a \$ b$"), r"$a \$ b$");
    }

    #[test]
    fn narrow_energy_axes_get_fewer_ticks() {
        let xanes = (8959., 9059.);
        assert_eq!(x_tick_count(6.4, 9., xanes), 10);
        let narrow = x_tick_count(3.3, 8.1, xanes);
        assert!(narrow < 10, "{narrow}");
        let ticks = ruviz::axes::generate_ticks(xanes.0, xanes.1, narrow);
        assert!(ticks[1] - ticks[0] >= 20., "{ticks:?}");
        // k axes have short labels and keep the default density.
        assert_eq!(x_tick_count(3.3, 8.1, (0., 14.)), 10);
    }

    #[test]
    fn auto_legend_picks_an_empty_corner() {
        let curve = |label: &str, x: Vec<f64>, y: Vec<f64>| FigureSeries {
            key: label.into(),
            label: label.into(),
            x,
            y,
            dashed: false,
            optional: false,
        };
        let frame = |outside_ok| LegendFrame {
            size_in: (3.3, 2.5),
            legend_pt: 8.1,
            tick_pt: 8.1,
            title: false,
            outside_ok,
        };
        // XANES-like edge step: low on the left, rising through the middle,
        // high on the right. Free corners: upper left and lower right.
        let energy: Vec<f64> = (0..400).map(|i| 8300. + i as f64 * 0.25).collect();
        let step: Vec<f64> = energy
            .iter()
            .map(|e| 1. / (1. + (-(e - 8345.) / 2.).exp()))
            .collect();
        let xanes = curve("ni_metal_rt", energy, step);
        let pick = auto_legend(
            &[&xanes],
            Some((8300., 8400.)),
            Some((-0.05, 1.05)),
            &frame(true),
        );
        assert!(
            matches!(
                pick,
                LegendPlacement::LowerRight | LegendPlacement::UpperLeft
            ),
            "{pick:?}"
        );
        // A label nearly as wide as a 3.3 in plot fits no corner: go outside.
        let long = FigureSeries {
            label: "Ni metal foil, room temperature, transmission".into(),
            ..xanes.clone()
        };
        assert_eq!(
            auto_legend(
                &[&long],
                Some((8300., 8400.)),
                Some((-0.05, 1.05)),
                &frame(true)
            ),
            LegendPlacement::Outside
        );
        // |χ(R)| first-shell peak near 2.2 Å on 0–6 Å: the upper right is empty.
        let r: Vec<f64> = (0..300).map(|i| i as f64 * 0.02).collect();
        let peak: Vec<f64> = r
            .iter()
            .map(|r| (-(r - 2.2_f64).powi(2) * 6.).exp())
            .collect();
        let data = curve("Data |χ(R)|", r.clone(), peak.clone());
        let fit = curve("Fit |χ(R)|", r, peak.iter().map(|y| 0.9 * y).collect());
        assert_eq!(
            auto_legend(
                &[&data, &fit],
                Some((0., 6.)),
                Some((-0.05, 1.05)),
                &frame(true)
            ),
            LegendPlacement::UpperRight
        );
        // A curve filling the whole plot leaves no free corner.
        let x: Vec<f64> = (0..200).map(|i| i as f64 / 199.).collect();
        let zigzag: Vec<f64> = x.iter().map(|x| (x * 80.).sin() * 0.5 + 0.5).collect();
        let busy = curve("Busy", x, zigzag);
        let span = (Some((0., 1.)), Some((0., 1.)));
        assert_eq!(
            auto_legend(&[&busy], span.0, span.1, &frame(true)),
            LegendPlacement::Outside
        );
        assert_ne!(
            auto_legend(&[&busy], span.0, span.1, &frame(false)),
            LegendPlacement::Outside
        );
    }

    #[test]
    fn legend_placements_round_trip_and_default_to_auto() {
        assert_eq!(LegendPlacement::default(), LegendPlacement::Auto);
        assert_eq!(LegendPlacement::Auto.position(), LegendPosition::Best);
        for placement in LegendPlacement::ALL {
            let json = serde_json::to_string(&placement).unwrap();
            assert_eq!(
                serde_json::from_str::<LegendPlacement>(&json).unwrap(),
                placement
            );
        }
        assert_eq!(
            serde_json::to_string(&LegendPlacement::UpperLeft).unwrap(),
            "\"upper-left\""
        );
    }
}
