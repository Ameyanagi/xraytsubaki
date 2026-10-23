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
    /// Recommended default: ruviz's "best" placement, which scores the inside
    /// corners and edges against a mask of the drawn curves and picks the
    /// position covering the least data.
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

/// Share of the canvas height given to the residual strip (rexafs choice).
pub(crate) const RESIDUAL_FRACTION: f64 = 0.28;

/// Render the fit panel above a residual strip that shares its x axis.
///
/// Both panels are separate ruviz plots with identical fixed left and right
/// margins, so their plot areas and x ticks line up. The upper panel has no
/// x label, and its bottom margin is smaller than ruviz's tick padding, which
/// places its x tick labels outside its canvas. The strip is symmetric about
/// zero, carries a zero line and the x label. Returns PNG bytes and an SVG
/// that nests both panel SVGs.
pub(super) fn fit_residual(
    data: &FigureData,
    residual: &[FigureSeries],
    options: &FigureOptions,
) -> Result<(Vec<u8>, String), String> {
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
    let top = top_data.panel(&top_options, false)?;
    let strip = strip_data
        .panel(&strip_options, true)?
        .ylim(-half, half)
        .major_ticks_y(3)
        .hline_styled(0., Color::from_gray(140), 0.8, LineStyle::Solid);

    // Shared horizontal margins, from the wider y tick labels of the two panels.
    let config = top.get_config();
    let (tick, label, title) = (
        config.typography.tick_size() as f64,
        config.typography.label_size() as f64,
        config.typography.title_size() as f64,
    );
    let (tick_pad, label_pad, title_pad) = (
        config.spacing.tick_pad as f64,
        config.spacing.label_pad as f64,
        config.spacing.title_pad as f64,
    );
    let ytick_chars = [top_ylim, Some((-half, half))]
        .into_iter()
        .zip([10, 3])
        .filter_map(|(range, ticks)| range.map(|r| (r, ticks)))
        .map(|((lo, hi), ticks)| widest_label(&ruviz::axes::generate_ticks(lo, hi, ticks)))
        .max()
        .unwrap_or(5)
        .max(3);
    let xtick_chars = data
        .xlim(options)
        .map(|(lo, hi)| widest_label(&ruviz::axes::generate_ticks(lo, hi, 10)))
        .unwrap_or(4);
    let left =
        (ytick_chars as f64 * CHAR_EM * tick + tick_pad + 1.5 * label + label_pad + 4.) / 72.;
    let right = ((xtick_chars as f64 * CHAR_EM * tick / 2. + 4.) / 72.).max(0.12);
    let top_margin = if options.title.is_some() {
        (1.4 * title + title_pad + 4.) / 72.
    } else {
        0.1
    };
    // Below ruviz's tick padding, so the upper panel's x tick labels fall
    // outside its canvas and the two panels sit close together.
    let gap = 0.5 * tick_pad / 72.;
    let bottom = (1.3 * tick + tick_pad + 1.5 * label + label_pad + 4.) / 72.;
    let strip_height = (total_px - top_px) / dpi;
    // A short strip cannot fit "Residual" along its plot area; abbreviate.
    let strip_plot_pt = (strip_height - 0.06 - bottom) * 72.;
    let strip = strip.ylabel(if strip_plot_pt >= 8. * CHAR_EM * label {
        "Residual"
    } else {
        "Res."
    });
    let top = panel_frame(top, top_px / dpi, [left, right, top_margin, gap]);
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
    let png = Image::new(upper.width, upper.height + lower.height, pixels)
        .encode_png_with_dpi(dpi as f32)
        .map_err(|e| e.to_string())?;
    let svg = stack_svgs(
        &top.render_to_svg().map_err(|e| e.to_string())?,
        &strip.render_to_svg().map_err(|e| e.to_string())?,
        (upper.width, upper.height, lower.height),
        dpi,
    );
    Ok((png, svg))
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
fn panel_frame(plot: Plot, height: f64, [left, right, top, bottom]: [f64; 4]) -> Plot {
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

/// Nest two panel SVGs, one above the other, in a single SVG document.
/// Element IDs get a per-panel prefix so glyph and clip definitions from the
/// two Typst/ruviz documents cannot collide.
fn stack_svgs(upper: &str, lower: &str, (width, top, bottom): (u32, u32, u32), dpi: f64) -> String {
    let nest = |svg: &str, prefix: &str, y: u32| {
        let body = &svg[svg.find("<svg").unwrap_or(0)..];
        body.replacen("<svg", &format!(r#"<svg x="0" y="{y}""#), 1)
            .replace(r#"id=""#, &format!(r#"id="{prefix}-"#))
            .replace("url(#", &format!("url(#{prefix}-"))
            .replace(r##"href="#"##, &format!(r##"href="#{prefix}-"##))
    };
    let height = top + bottom;
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{width}" height="{height}">{}{}</svg>"#,
        nest(upper, "fit", 0),
        nest(lower, "residual", top),
    );
    physical_svg_size(svg, (width, height), dpi)
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
