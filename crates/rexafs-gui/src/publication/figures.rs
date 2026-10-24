//! Publication presets share the same data and renderer with preview and export.
use super::*;
use ruviz::prelude::{LineStyle, Plot};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub(crate) mod composer;
mod labels;
mod layout;
mod recipes;
pub(crate) use layout::{LegendPlacement, typst_label};

/// Selectable curve colours from Okabe & Ito (2008), *Color Universal Design*
/// (<https://jfly.uni-koeln.de/color/>), as display labels and lowercase RGB hex.
/// These explicit choices do not replace the renderer's automatic palette.
pub(crate) const SERIES_PALETTE: [(&str, &str); 8] = [
    ("Black", "#000000"),
    ("Orange", "#e69f00"),
    ("Sky blue", "#56b4e9"),
    ("Bluish green", "#009e73"),
    ("Yellow", "#f0e442"),
    ("Blue", "#0072b2"),
    ("Vermillion", "#d55e00"),
    ("Reddish purple", "#cc79a7"),
];

/// Require an opaque RGB colour in the saved, canonical `#rrggbb` format.
fn series_color(value: &str) -> Result<ruviz::prelude::Color, String> {
    if value.len() != 7
        || !value.starts_with('#')
        || !value.as_bytes()[1..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err("Use a lowercase RGB colour in #rrggbb format, for example #0072b2.".into());
    }
    ruviz::prelude::Color::from_hex(value).map_err(|e| e.to_string())
}

/// Validate values without interpreting series keys, which are opaque strings.
fn validate_series_colors(colors: &BTreeMap<String, String>) -> Result<(), String> {
    for (key, value) in colors {
        series_color(value).map_err(|error| format!("Series colour for {key:?}: {error}"))?;
    }
    Ok(())
}

fn weighted_unit(weight: f64) -> String {
    if weight.abs() < 1e-9 {
        "(dimensionless)".into()
    } else {
        plotting::chir_label(weight - 1.).replace("|χ(R)| ", "")
    }
}
fn weighted_chi_label(weight: f64) -> String {
    format!("{} {}", plotting::chik_label(weight), weighted_unit(weight))
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
/// Presentation settings for one figure type, shared across exported spectra.
///
/// These settings change labels, appearance and visible series, not calculated
/// scientific arrays. CSV uses full visible-series arrays even when plot limits
/// show only a smaller region. See [`Self::validate`] for output-size limits.
pub(crate) struct FigureOptions {
    /// Canvas width in inches; None uses the plotting library's 6.4-inch default.
    pub width: Option<f64>,
    /// Canvas height in inches; None uses the plotting library's 4.8-inch default.
    pub height: Option<f64>,
    /// Integer raster dots per inch; None selects rexafs's 300 DPI default.
    pub dpi: Option<f64>,
    /// Font size in points, 4–48 when supplied; None preserves renderer defaults.
    pub font_size: Option<f64>,
    /// Curve width in points, 0.1–12 when supplied; None preserves defaults.
    pub line_width: Option<f64>,
    /// Display title override; None uses the figure's generated title.
    pub title: Option<String>,
    /// User caption; None uses a factual caption derived from visible curves.
    pub caption: Option<String>,
    /// Horizontal-axis label override; it does not convert the data's units.
    pub xlabel: Option<String>,
    /// Vertical-axis label override; it does not rescale the curve values.
    pub ylabel: Option<String>,
    /// Lower x display limit in the figure's axis units; set together with xmax.
    pub xmin: Option<f64>,
    /// Upper x display limit; must exceed xmin. None uses the figure's range.
    pub xmax: Option<f64>,
    /// Lower y display limit in the figure's axis units; set together with ymax.
    pub ymin: Option<f64>,
    /// Upper y display limit; must exceed ymin. None selects automatic limits.
    pub ymax: Option<f64>,
    /// Show the visible-series legend; true by default.
    pub legend: bool,
    /// Legend placement when shown; Auto (least overlap with curves) by default.
    pub legend_position: LegendPlacement,
    /// Show the plot grid; None defaults to true.
    pub grid: Option<bool>,
    /// Draw applicable processing guides; false by default.
    pub guides: bool,
    /// Series keys explicitly hidden from figures and their CSV exports.
    pub hidden: BTreeSet<String>,
    /// Optional series keys explicitly enabled, unless also hidden.
    pub shown: BTreeSet<String>,
    /// Explicit opaque RGB colours by series key, in lowercase
    /// `#rrggbb` format. Missing keys use the renderer's automatic colours;
    /// unknown keys are retained but do not affect rendering or scientific data.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub series_colors: BTreeMap<String, String>,
    /// Energy figures default to the library's flattened array.
    pub normalized: bool,
}

impl Default for FigureOptions {
    fn default() -> Self {
        Self {
            width: None,
            height: None,
            dpi: None,
            font_size: None,
            line_width: None,
            title: None,
            caption: None,
            xlabel: None,
            ylabel: None,
            xmin: None,
            xmax: None,
            ymin: None,
            ymax: None,
            legend: true,
            legend_position: LegendPlacement::Auto,
            grid: None,
            guides: false,
            hidden: BTreeSet::new(),
            shown: BTreeSet::new(),
            series_colors: BTreeMap::new(),
            normalized: false,
        }
    }
}

/// Publication appearance and named multi-panel recipes saved with a project.
/// Recipes keep insertion order and the active recipe is stored by index, so
/// renaming never changes the selection.
/// New projects start with Figure 1. Loading also accepts the old single
/// `publication.composite` as Figure 1; saving writes only the new format.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(from = "recipes::SavedFigureSettings")]
pub(crate) struct FigureSettings {
    /// Settings are shared by figure type across a multi-spectrum export.
    pub figures: BTreeMap<String, FigureOptions>,
    pub table_captions: BTreeMap<String, String>,
    /// Ordered, non-empty recipes; changed through the recipe methods below.
    recipes: Vec<recipes::NamedRecipe>,
    /// Zero-based selection, adjusted on deletion and clamped when loading.
    active_recipe: usize,
}
impl FigureSettings {
    /// Return an owned copy of settings for a figure type, or its defaults.
    pub fn options(&self, key: &str) -> FigureOptions {
        self.figures.get(key).cloned().unwrap_or_default()
    }

    /// Copy the appearance of figure type `source` (canvas size, DPI, font and
    /// line width, legend and its placement, grid and guides) to every figure type in `keys` and
    /// to every type that already has stored options. Titles, labels, captions,
    /// axis limits, curve visibility and series colours stay per figure,
    /// because they depend on the plotted quantity.
    pub fn apply_style_to_all<'a>(
        &mut self,
        source: &str,
        keys: impl IntoIterator<Item = &'a str>,
    ) {
        let style = self.options(source);
        let targets: BTreeSet<String> = keys
            .into_iter()
            .map(str::to_string)
            .chain(self.figures.keys().cloned())
            .collect();
        for key in targets {
            let options = self.figures.entry(key).or_default();
            options.width = style.width;
            options.height = style.height;
            options.dpi = style.dpi;
            options.font_size = style.font_size;
            options.line_width = style.line_width;
            options.legend = style.legend;
            options.legend_position = style.legend_position;
            options.grid = style.grid;
            options.guides = style.guides;
        }
    }
}

/// Page-layout presets that set a figure's canvas size, raster resolution and
/// font size together. Labels, axis limits and curve visibility are untouched.
/// The values are rexafs choices that fit common journal column widths and a
/// 16:9 slide; check the target journal's figure guidelines before submission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StylePreset {
    /// One journal column: 3.5 × 2.6 in at 600 DPI with 8 pt text.
    SingleColumn,
    /// Two journal columns: 7.0 × 4.8 in at 300 DPI with 9 pt text.
    DoubleColumn,
    /// Presentation slide: 10 × 5.6 in at 200 DPI with 14 pt text.
    Slide,
}

impl StylePreset {
    /// Every preset in display order.
    pub const ALL: [Self; 3] = [Self::SingleColumn, Self::DoubleColumn, Self::Slide];

    /// Short chip label in sentence case.
    pub fn label(self) -> &'static str {
        match self {
            Self::SingleColumn => "Single column",
            Self::DoubleColumn => "Double column",
            Self::Slide => "Slide",
        }
    }

    /// Width and height in inches, raster DPI and font size in points.
    pub fn values(self) -> (f64, f64, f64, f64) {
        match self {
            Self::SingleColumn => (3.5, 2.6, 600., 8.),
            Self::DoubleColumn => (7.0, 4.8, 300., 9.),
            Self::Slide => (10., 5.6, 200., 14.),
        }
    }

    /// Set this preset's size, DPI and font size on `options`.
    pub fn apply(self, options: &mut FigureOptions) {
        let (width, height, dpi, font_size) = self.values();
        options.width = Some(width);
        options.height = Some(height);
        options.dpi = Some(dpi);
        options.font_size = Some(font_size);
    }

    /// True when `options` carries exactly this preset's size, DPI and font size.
    pub fn matches(self, options: &FigureOptions) -> bool {
        let (width, height, dpi, font_size) = self.values();
        options.width == Some(width)
            && options.height == Some(height)
            && options.dpi == Some(dpi)
            && options.font_size == Some(font_size)
    }
}

impl FigureOptions {
    /// Resolve canvas width/height in inches and raster resolution in DPI.
    /// This fills defaults without changing stored options or validating limits.
    pub fn dimensions(&self) -> (f64, f64, f64) {
        let default = Plot::new();
        let figure = &default.get_config().figure;
        (
            self.width.unwrap_or(figure.width as f64),
            self.height.unwrap_or(figure.height as f64),
            self.dpi.unwrap_or(300.),
        )
    }

    /// Validate dimensions, point sizes and paired axis limits before rendering.
    /// Requires finite dimensions of 1–30 inches, integer DPI of 72–1200 and at
    /// most 25 million raster pixels. Axis limits must be finite, paired and
    /// increasing. Explicit series colours must use lowercase `#rrggbb` RGB
    /// notation. Errors leave the options and scientific data unchanged.
    pub fn validate(&self) -> Result<(), String> {
        let (width, height, dpi) = self.dimensions();
        if ![width, height, dpi].iter().all(|v| v.is_finite())
            || !(1.0..=30.0).contains(&width)
            || !(1.0..=30.0).contains(&height)
            || !(72.0..=1200.0).contains(&dpi)
            || dpi.fract() != 0.0
        {
            return Err("Use a width/height of 1–30 inches and an integer DPI of 72–1200.".into());
        }
        if width * height * dpi * dpi > 25_000_000.0 {
            return Err("Reduce size or DPI to keep the figure below 25 million pixels.".into());
        }
        for (name, value, low, high) in [
            ("Font size", self.font_size, 4.0, 48.0),
            ("Line width", self.line_width, 0.1, 12.0),
        ] {
            if value.is_some_and(|v| !v.is_finite() || !(low..=high).contains(&v)) {
                return Err(format!("{name} must be {low}–{high} points, or Auto."));
            }
        }
        for (axis, min, max) in [("X", self.xmin, self.xmax), ("Y", self.ymin, self.ymax)] {
            match (min, max) {
                (None, None) => (),
                (Some(a), Some(b)) if a.is_finite() && b.is_finite() && a < b => (),
                _ => {
                    return Err(format!(
                        "Set both {axis} limits with min < max, or clear both for Auto."
                    ));
                }
            }
        }
        validate_series_colors(&self.series_colors)
    }

    fn apply(&self, mut plot: Plot) -> Result<Plot, String> {
        self.validate()?;
        let (width, height, dpi) = self.dimensions();
        // Keep the library's physical size, with publication output at 300 DPI.
        if self.width.is_some() || self.height.is_some() {
            plot = plot.size(width as f32, height as f32);
        }
        plot = plot.dpi(dpi as u32).grid(self.grid.unwrap_or(true));
        if let Some(size) = self.font_size {
            plot = plot.font_size(size as f32);
        }
        // Typed text is literal; only `$…$` spans are Typst math.
        if let Some(title) = &self.title {
            plot = plot.title(layout::typst_label(title));
        }
        if let Some(label) = &self.xlabel {
            plot = plot.xlabel(layout::typst_label(label));
        }
        if let Some(label) = &self.ylabel {
            plot = plot.ylabel(layout::typst_label(label));
        }
        if let (Some(min), Some(max)) = (self.xmin, self.xmax) {
            plot = plot.xlim(min, max);
        }
        if let (Some(min), Some(max)) = (self.ymin, self.ymax) {
            plot = plot.ylim(min, max);
        }
        if let Some(grid) = self.grid {
            plot = plot.grid(grid);
        }
        if self.legend {
            plot = plot.legend(self.legend_position.position());
        }
        Ok(plot)
    }
}

#[derive(Clone)]
pub(crate) struct FigureSeries {
    pub key: String,
    pub label: String,
    pub x: Vec<f64>,
    pub y: Vec<f64>,
    pub dashed: bool,
    pub optional: bool,
}

impl FigureSeries {
    pub(crate) fn visible(&self, options: &FigureOptions) -> bool {
        !options.hidden.contains(&self.key) && (!self.optional || options.shown.contains(&self.key))
    }
}

#[derive(Clone)]
pub(crate) struct FigureData {
    pub key: &'static str,
    pub label: String,
    pub xlabel: String,
    pub ylabel: String,
    pub math_ylabel: String,
    pub series: Vec<FigureSeries>,
    pub normalized_series: Option<Vec<FigureSeries>>,
    pub guides: Vec<f64>,
    pub default_xlim: Option<(f64, f64)>,
    /// Data − fit curve drawn in a strip below the main panel (fit + residual).
    pub residual: Option<Vec<FigureSeries>>,
}

impl FigureData {
    pub(crate) fn common(&self) -> bool {
        matches!(
            self.key,
            "xanes" | "flattened-mu" | "chi-k" | "chi-r" | "fit-r"
        )
    }

    pub(crate) fn series(&self, options: &FigureOptions) -> &[FigureSeries] {
        if options.normalized
            && let Some(series) = &self.normalized_series
        {
            return series;
        }
        &self.series
    }

    pub(crate) fn ylabel(&self, options: &FigureOptions) -> &str {
        if options.normalized && self.normalized_series.is_some() {
            "Normalized μ(E) (dimensionless)"
        } else {
            &self.ylabel
        }
    }

    /// Typst source of the x label; built-in labels are sans-serif text
    /// markup (see [`labels`]).
    pub(crate) fn math_xlabel(&self) -> &str {
        labels::x_label(self.key).unwrap_or(&self.xlabel)
    }

    pub(crate) fn math_ylabel(&self, options: &FigureOptions) -> &str {
        if options.normalized && self.normalized_series.is_some() {
            "Normalized _μ_(_E_) (dimensionless)"
        } else {
            &self.math_ylabel
        }
    }

    pub(crate) fn xlim(&self, options: &FigureOptions) -> Option<(f64, f64)> {
        options.xmin.zip(options.xmax).or(self.default_xlim)
    }

    /// χ(k) and χ(q) oscillate about zero, so their automatic y range is
    /// symmetric (−a to +a) and the zero line sits mid-axis.
    fn symmetric_y(&self) -> bool {
        matches!(
            self.key,
            "chi-k" | "fit-k" | "residual-k" | "fit-residual-k" | "chi-q" | "fit-q"
        )
    }

    /// Autoscale only the displayed interval; a noisy high-k tail must not
    /// flatten the useful signal. The series and exported CSV keep every point.
    /// k-space figures use a symmetric range (see [`Self::symmetric_y`]).
    pub(crate) fn visible_ylim(&self, options: &FigureOptions) -> Option<(f64, f64)> {
        self.visible_ylim_with_symmetry(options, self.symmetric_y())
    }

    /// Display offsets need the occupied range rather than an artificial
    /// symmetry about zero; ordinary EXAFS panels retain their symmetric range.
    fn visible_ylim_asymmetric(&self, options: &FigureOptions) -> Option<(f64, f64)> {
        self.visible_ylim_with_symmetry(options, false)
    }

    fn visible_ylim_with_symmetry(
        &self,
        options: &FigureOptions,
        symmetric: bool,
    ) -> Option<(f64, f64)> {
        let (xmin, xmax) = self.xlim(options)?;
        let mut ymin = f64::INFINITY;
        let mut ymax = f64::NEG_INFINITY;
        let mut include = |y: f64| {
            if y.is_finite() {
                ymin = ymin.min(y);
                ymax = ymax.max(y);
            }
        };
        for s in self.series(options).iter().filter(|s| s.visible(options)) {
            for (&x, &y) in s.x.iter().zip(&s.y) {
                if (xmin..=xmax).contains(&x) {
                    include(y);
                }
            }
            for (x, y) in s.x.windows(2).zip(s.y.windows(2)) {
                for limit in [xmin, xmax] {
                    if x[0] < limit && limit < x[1] {
                        include(y[0] + (y[1] - y[0]) * (limit - x[0]) / (x[1] - x[0]));
                    }
                }
            }
        }
        if !ymin.is_finite() || !ymax.is_finite() {
            return None;
        }
        if symmetric {
            ymax = ymin.abs().max(ymax.abs());
            ymin = -ymax;
        }
        let pad = if ymax > ymin {
            (ymax - ymin) * 0.05
        } else {
            ymin.abs().max(1.) * 0.05
        };
        Some((ymin - pad, ymax + pad))
    }

    /// One x/y pair per visible curve preserves distinct grids without
    /// interpolation. Seventeen significant digits round-trip f64 values.
    pub(crate) fn csv(&self, options: &FigureOptions) -> Result<String, String> {
        let series: Vec<_> = self
            .series(options)
            .iter()
            .filter(|s| s.visible(options) && !s.x.is_empty())
            .chain(self.residual.iter().flatten())
            .collect();
        if series.is_empty() {
            return Err("Select at least one curve with available data.".into());
        }
        if series.iter().any(|s| s.x.len() != s.y.len()) {
            return Err("A curve has mismatched x and y arrays.".into());
        }
        let quote = crate::fitting::csv_field;
        let mut csv = series
            .iter()
            .flat_map(|s| {
                [
                    quote(&format!("{}: {}", s.label, self.xlabel)),
                    quote(&format!("{}: {}", s.label, self.ylabel(options))),
                ]
            })
            .collect::<Vec<_>>()
            .join(",");
        csv.push('\n');
        for row in 0..series.iter().map(|s| s.x.len()).max().unwrap_or(0) {
            let cells: Vec<_> = series
                .iter()
                .flat_map(|s| {
                    if row < s.x.len() {
                        [format!("{:.16e}", s.x[row]), format!("{:.16e}", s.y[row])]
                    } else {
                        [String::new(), String::new()]
                    }
                })
                .collect();
            csv.push_str(&cells.join(","));
            csv.push('\n');
        }
        Ok(csv)
    }

    /// A factual starting caption, derived only from the curves actually shown.
    pub fn caption(&self, options: &FigureOptions) -> String {
        if let Some(caption) = &options.caption {
            return caption.clone();
        }
        let description = match self.key {
            "mu-energy" => "X-ray absorption spectrum and selected normalization/background curves",
            "xanes" | "flattened-mu" if options.normalized => {
                "Edge-step-normalized X-ray absorption spectrum"
            }
            "xanes" | "flattened-mu" => {
                "Flattened, edge-step-normalized X-ray absorption spectrum with the fitted post-edge trend removed"
            }
            "chi-k" => "Background-subtracted EXAFS as a function of photoelectron wave number",
            "chi-r" => "Fourier-transformed EXAFS; radial coordinates are not phase corrected",
            "chi-q" => "Back-transformed EXAFS",
            "fit-k" => "EXAFS data, fitted model and selected path contributions in k space",
            "fit-r" => {
                "EXAFS data, fitted model and selected components in R space; radial coordinates are not phase corrected"
            }
            "fit-q" => "Back-transformed EXAFS data and fitted model",
            "fit-residual-k" => {
                "EXAFS data and fitted model in k space (upper panel), with the weighted residual, data minus fit, in the same units on a shared horizontal axis (lower panel)"
            }
            "fit-residual-r" => {
                "EXAFS data and fitted model magnitudes in R space (upper panel), with the difference of the magnitudes on a shared horizontal axis (lower panel); this is not the magnitude of the complex residual, and radial coordinates are not phase corrected"
            }
            "residual-k" => "Weighted EXAFS residual, defined as data minus fitted model",
            "residual-r" => {
                "Difference between the magnitudes of the data and fitted Fourier transforms; this is not the magnitude of the complex residual"
            }
            _ => &self.label,
        };
        let curves = self
            .series(options)
            .iter()
            .filter(|s| !s.x.is_empty() && s.visible(options))
            .chain(self.residual.iter().flatten())
            .map(|s| {
                format!(
                    "{} ({})",
                    s.label,
                    if s.dashed { "dashed" } else { "solid" }
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        let mut caption = format!(
            "{description}. Horizontal axis: {}; vertical axis: {}. Curves, in plotting order: {curves}.",
            options.xlabel.as_deref().unwrap_or(&self.xlabel),
            options.ylabel.as_deref().unwrap_or(self.ylabel(options))
        );
        if options.guides && !self.guides.is_empty() {
            caption.push_str(&format!(
                " Vertical guides at {} (horizontal-axis units).",
                self.guides
                    .iter()
                    .map(|v| format!("{v:.4}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        caption
    }

    pub fn plot(&self, options: &FigureOptions) -> Result<Plot, String> {
        self.panel(options, false)
    }

    /// Build one plot panel. `fit_panel` marks the upper panel of a fit +
    /// residual figure: no x-axis title, a shorter canvas, and no Outside legend.
    fn panel(&self, options: &FigureOptions, fit_panel: bool) -> Result<Plot, String> {
        // Typst lays out every label. In SVG, plain labels (tick numbers, legend
        // names, typed titles) become editable `<text>`; math stays outlined.
        let mut plot = Plot::new()
            .typst(true)
            .svg_native_text(true)
            .ylabel(self.math_ylabel(options));
        if !fit_panel {
            plot = plot.xlabel(self.math_xlabel());
        }
        if let Some((min, max)) = self.xlim(options) {
            plot = plot.xlim(min, max);
        }
        if let Some((min, max)) = self.visible_ylim(options) {
            plot = plot.ylim(min, max);
        }
        let mut plot = options.apply(plot)?;
        if let Some(xlim) = self.xlim(options) {
            let tick = plot.get_config().typography.tick_size() as f64;
            plot = plot.major_ticks_x(layout::x_tick_count(options.dimensions().0, tick, xlim));
        }
        if options.legend && options.legend_position == LegendPlacement::Auto {
            let (width, height, _) = options.dimensions();
            let visible: Vec<_> = self
                .series(options)
                .iter()
                .filter(|s| s.visible(options) && !s.x.is_empty())
                .collect();
            let typography = &plot.get_config().typography;
            let frame = layout::LegendFrame {
                size_in: (
                    width,
                    if fit_panel {
                        height * (1. - layout::RESIDUAL_FRACTION)
                    } else {
                        height
                    },
                ),
                legend_pt: typography.legend_size() as f64,
                tick_pt: typography.tick_size() as f64,
                title: options.title.is_some(),
                outside_ok: !fit_panel,
            };
            let ylim = options
                .ymin
                .zip(options.ymax)
                .or_else(|| self.visible_ylim(options));
            let placement = layout::auto_legend(&visible, self.xlim(options), ylim, &frame);
            // Auto's outside fallback goes above the plot: it costs height,
            // not width, so a long label cannot squeeze a narrow figure.
            plot = plot.legend(match placement {
                LegendPlacement::Outside => ruviz::core::LegendPosition::OutsideUpper,
                corner => corner.position(),
            });
        }
        let mut count = 0;
        let palette = plot.get_theme();
        let mut preserve_palette = false;
        for series in self.series(options) {
            if !series.visible(options) || series.x.is_empty() {
                continue;
            }
            let mut line = plot.line(&series.x, &series.y).label(
                ruviz::render::typst_text::literal_text_snippet(&series.label),
            );
            if let Some(color) = options.series_colors.get(&series.key) {
                line = line.color(series_color(color)?);
                preserve_palette = true;
            } else if preserve_palette {
                // ruviz's builder does not advance its palette for explicit
                // colours. Keep every later automatic curve's original slot.
                line = line.color(palette.get_color(count));
            }
            if let Some(width) = options.line_width {
                line = line.line_width(width as f32);
            }
            if series.dashed {
                line = line.line_style(LineStyle::Dashed);
            }
            plot = line.into();
            count += 1;
        }
        if count == 0 {
            return Err("Select at least one curve with available data.".into());
        }
        if options.guides {
            for &x in &self.guides {
                plot = plot.vline_styled(
                    x,
                    ruviz::prelude::Color::from_gray(140),
                    0.8,
                    LineStyle::Dashed,
                );
            }
        }
        Ok(plot)
    }
}

/// Capture the exact PNG displayed in the editor; saving it cannot change layout.
pub(crate) struct RenderedFigure {
    pub png: Vec<u8>,
    pub svg: String,
}
pub(crate) fn render_figure(
    data: &FigureData,
    options: &FigureOptions,
) -> Result<RenderedFigure, String> {
    let (png, mut svg) = match &data.residual {
        Some(residual) => layout::fit_residual(data, residual, options)?,
        None => {
            let plot = data.plot(options)?;
            let config = plot.get_config();
            let svg = physical_svg_size(
                plot.render_to_svg().map_err(|e| e.to_string())?,
                config.canvas_size(),
                config.figure.dpi as f64,
            );
            (plot.render_png_bytes().map_err(|e| e.to_string())?, svg)
        }
    };
    // ruviz names each outlined label by its Typst source; read built-in
    // labels as their plain Unicode text (k²χ(k)), not markup (`_k_#super[2]`).
    for (markup, plain) in [
        (data.math_xlabel(), data.xlabel.as_str()),
        (data.math_ylabel(options), data.ylabel(options)),
    ] {
        let aria = |text: &str| format!(r#"aria-label="{}""#, super::report::html(text));
        svg = svg.replace(&aria(markup), &aria(plain));
    }
    if let Some(start) = svg.find("<svg")
        && let Some(end) = svg[start..].find('>')
    {
        svg.insert_str(
            start + end + 1,
            &format!(
                "<desc>{}</desc>",
                super::report::html(&data.caption(options))
            ),
        );
        if let Some(title) = &options.title {
            svg.insert_str(
                start + end + 1,
                &format!("<title>{}</title>", super::report::html(title)),
            );
        }
    }
    Ok(RenderedFigure { png, svg })
}

/// Give the root `<svg>` element a physical size in inches.
///
/// ruviz draws the SVG in raster pixels (`inches × DPI`) and writes them as
/// unitless `width`/`height`, which SVG treats as CSS pixels (96 per inch), so a
/// 3.3 in figure at 300 DPI would import at about 10 in. This rexafs-specific
/// fix keeps those pixel coordinates as the `viewBox` and sets `width`/`height`
/// to `pixels / DPI` inches, so the drawing scales to the configured size in
/// vector editors, word processors and browsers. An SVG that already has a
/// `viewBox`, or whose size does not match the canvas, is returned unchanged.
fn physical_svg_size(mut svg: String, (width, height): (u32, u32), dpi: f64) -> String {
    let Some(start) = svg.find("<svg") else {
        return svg;
    };
    let Some(end) = svg[start..].find('>').map(|end| start + end) else {
        return svg;
    };
    let pixels = format!(r#"width="{width}" height="{height}""#);
    if svg[start..end].contains("viewBox") {
        return svg;
    }
    if let Some(at) = svg[start..end].find(&pixels).map(|at| start + at) {
        let inches = |px: u32| {
            let value = format!("{:.4}", px as f64 / dpi);
            format!("{}in", value.trim_end_matches('0').trim_end_matches('.'))
        };
        svg.replace_range(
            at..at + pixels.len(),
            &format!(
                r#"width="{}" height="{}" viewBox="0 0 {width} {height}""#,
                inches(width),
                inches(height)
            ),
        );
    }
    svg
}

pub(crate) fn quantity_figures(
    sp: Arc<XASSpectrum>,
    label: &str,
    quantity: Option<crate::params::Quantity>,
) -> Vec<FigureData> {
    let mut figures = spectrum_figures(sp, label);
    if let Some(quantity) = quantity.filter(|q| !q.is_absorption()) {
        figures.retain(|figure| !figure.series.is_empty());
        for figure in &mut figures {
            if figure.key == "mu-energy" {
                figure.label = quantity.label().into();
                figure.ylabel = format!("{} (dimensionless)", quantity.label());
                figure.math_ylabel =
                    ruviz::render::typst_text::literal_text_snippet(&figure.ylabel);
            }
        }
    }
    figures
}

fn publication_kmax(kmax: Option<f64>, data: Option<&[f64]>) -> Option<f64> {
    kmax.filter(|v| v.is_finite() && *v > 0.)
        .or_else(|| {
            data?
                .iter()
                .copied()
                .filter(|v| v.is_finite())
                .reduce(f64::max)
        })
        .filter(|v| *v > 0.)
        .map(|v| v + 1.)
}

/// Upper |χ(R)| display limit: 6 Å, or the fit/window maximum plus 0.5 Å
/// rounded up to a whole ångström when that is larger.
fn publication_rmax(fit_rmax: Option<f64>) -> f64 {
    match fit_rmax.filter(|v| v.is_finite() && *v > 6.) {
        Some(max) => (max + 0.5).ceil(),
        None => 6.,
    }
}

/// Upper |χ(R)| display limit for a processed spectrum. It follows
/// [`publication_rmax`] for the back-transform R window, ignores a window that
/// reaches past the displayed transform (such as the 20 Å library default) and
/// never exceeds that displayed maximum (`rmax_out`, 10 Å by default).
fn spectrum_rmax(window_rmax: Option<f64>, rmax_out: Option<f64>) -> f64 {
    let shown = rmax_out.filter(|v| v.is_finite() && *v > 0.).unwrap_or(10.);
    publication_rmax(window_rmax.filter(|v| *v <= shown)).min(shown)
}

pub(crate) fn spectrum_figures(sp: Arc<XASSpectrum>, label: &str) -> Vec<FigureData> {
    let weight = sp.kweight().copied().unwrap_or(2.);
    let kmax = publication_kmax(sp.xftf.as_ref().and_then(|ft| ft.kmax), sp.k());
    let rmax = spectrum_rmax(
        sp.xftr.as_ref().and_then(|ft| ft.rmax),
        sp.xftf.as_ref().and_then(|ft| ft.rmax_out),
    );
    let e0 = sp
        .normalization
        .as_ref()
        .and_then(|n| n.get_e0())
        .or(sp.e0());
    let norm_end = match sp.normalization.as_ref() {
        Some(rexafs::xafs::normalization::NormalizationMethod::PrePostEdge(p)) => p.norm_end,
        _ => None,
    };
    // Flattened and normalized are exact, distinct library outputs. Missing
    // flattened data must never be relabeled normalized data.
    let energy_figure = sp
        .energy
        .as_ref()
        .zip(sp.flat())
        .and_then(|(energy, flat)| {
            if energy.is_empty() || energy.len() != flat.len() {
                return None;
            }
            let series = |y: Vec<f64>| FigureSeries {
                key: "series-0".into(),
                label: label.into(),
                x: energy.as_slice().to_vec(),
                y,
                dashed: false,
                optional: false,
            };
            let normalized_series = sp
                .norm()
                .filter(|n| n.len() == energy.len())
                .map(|n| vec![series(n.as_slice().to_vec())]);
            let end = e0
                .zip(norm_end)
                .map(|(edge, end)| edge + end)
                .filter(|v| v.is_finite() && *v > energy[0])
                .unwrap_or(energy[energy.len() - 1])
                .min(energy[energy.len() - 1]);
            Some(FigureData {
                key: "flattened-mu",
                label: "Energy · μ(E)".into(),
                xlabel: "Energy (eV)".into(),
                ylabel: "Flattened μ(E) (dimensionless)".into(),
                math_ylabel: format!("Flattened {} (dimensionless)", labels::MU_E),
                series: vec![series(flat.as_slice().to_vec())],
                normalized_series,
                guides: e0.into_iter().collect(),
                default_xlim: (energy[0] < end).then_some((energy[0], end)),
                residual: None,
            })
        });
    let opts = plotting::ViewOptions {
        flat: false,
        show_re: false,
        show_im: false,
        show_bkg: true,
        show_pre: true,
        show_post: true,
        show_e0: true,
        show_ranges: true,
        show_kwin: false,
        ..Default::default()
    };
    let traces = [plotting::QuadTrace {
        color_index: 0,
        color: None,
        label: label.into(),
        sp,
        active: true,
    }];
    let mut figures: Vec<_> = ["mu-energy", "normalized-mu", "chi-k", "chi-r", "chi-q"]
        .into_iter()
        .zip(plotting::build_quadrant_specs(
            &traces,
            &opts,
            &Theme::light(),
            true,
        ))
        .filter(|(key, _)| *key != "normalized-mu")
        .map(|(key, spec)| FigureData {
            key,
            label: spec.title,
            xlabel: spec.xlabel,
            ylabel: match key {
                "mu-energy" => "μ(E) (arb. units)".into(),
                "chi-k" => weighted_chi_label(weight),
                "chi-r" => plotting::chir_label(weight),
                "chi-q" => format!("Re χ(q) {}", weighted_unit(weight)),
                _ => spec.ylabel,
            },
            math_ylabel: match key {
                "mu-energy" => format!("{} (arb. units)", labels::MU_E),
                "chi-k" => labels::chik(weight),
                "chi-r" => labels::chir(weight),
                "chi-q" => labels::chiq(weight),
                _ => String::new(),
            },
            default_xlim: match key {
                "chi-k" => kmax.map(|max| (0., max)),
                "chi-r" => Some((0., rmax)),
                _ => None,
            },
            guides: spec.vlines.into_iter().map(|(x, _, _, _)| x).collect(),
            normalized_series: None,
            residual: None,
            series: spec
                .series
                .into_iter()
                .enumerate()
                .map(|(i, s)| FigureSeries {
                    key: format!("series-{i}"),
                    label: s.label.unwrap_or_else(|| {
                        if i == 0 {
                            label.into()
                        } else {
                            format!("Curve {}", i + 1)
                        }
                    }),
                    x: s.x,
                    y: s.y,
                    dashed: s.dashed,
                    optional: false,
                })
                .collect(),
        })
        .collect();
    if let Some(energy) = energy_figure {
        let mut xanes = energy.clone();
        xanes.key = "xanes";
        xanes.label = "XANES · μ(E)".into();
        xanes.default_xlim = e0.map(|edge| (edge - 20., edge + 80.));
        figures.extend([xanes, energy]);
    }
    figures.sort_by_key(|f| match f.key {
        "xanes" => 0,
        "flattened-mu" => 1,
        "chi-k" => 2,
        "chi-r" => 3,
        "mu-energy" => 4,
        _ => 5,
    });
    figures
}

pub(crate) fn fit_figures(result: &FeffFitResult) -> Vec<FigureData> {
    let kw = result.kweight;
    let k: Vec<_> = result.k.iter().copied().collect();
    let r: Vec<_> = result.r.iter().copied().collect();
    let q: Vec<_> = result.q.iter().copied().collect();
    let weighted = |values: &nalgebra::DVector<f64>| {
        values
            .iter()
            .zip(&k)
            .map(|(y, k)| y * k.powf(kw))
            .collect::<Vec<_>>()
    };
    let magnitude = result
        .data_chir_re
        .iter()
        .zip(result.data_chir_im.iter())
        .map(|(re, im)| re.hypot(*im))
        .collect::<Vec<_>>();
    let data_k = weighted(&result.data_chi);
    let model_k = weighted(&result.model_chi);
    let model_r: Vec<_> = result.model_chir_mag.iter().copied().collect();
    let series = |key: &str, label: &str, x: &[f64], y: &[f64], dashed| {
        let n = x.len().min(y.len());
        FigureSeries {
            key: key.into(),
            label: label.into(),
            x: x[..n].to_vec(),
            y: y[..n].to_vec(),
            dashed,
            optional: key != "data" && key != "fit" && key != "residual",
        }
    };
    let figure = |key, label: &str, xlabel: &str, ylabel: String, series| FigureData {
        key,
        label: label.into(),
        xlabel: xlabel.into(),
        ylabel,
        math_ylabel: match key {
            "fit-k" | "fit-residual-k" => labels::chik(kw),
            "fit-r" | "fit-residual-r" => labels::chir(kw),
            "fit-q" => labels::chiq(kw),
            "residual-k" => format!("Residual {}", labels::chik(kw)),
            "residual-r" => format!("Residual {}", labels::chir(kw)),
            _ => String::new(),
        },
        series,
        normalized_series: None,
        guides: vec![],
        default_xlim: match key {
            "fit-r" | "residual-r" | "fit-residual-r" => Some((0., publication_rmax(result.rmax))),
            "fit-k" | "residual-k" | "fit-residual-k" => {
                publication_kmax(result.kmax, Some(&k)).map(|max| (0., max))
            }
            _ => None,
        },
        residual: None,
    };
    let mut output = vec![
        figure(
            "fit-k",
            "Fit · χ(k)",
            "k (Å⁻¹)",
            weighted_chi_label(kw),
            vec![
                series("data", "Data", &k, &data_k, false),
                series("fit", "Fit", &k, &model_k, true),
            ],
        ),
        figure(
            "fit-r",
            "Fit · |χ(R)|",
            "R (Å)",
            plotting::chir_label(kw),
            vec![
                series("data", "Data |χ(R)|", &r, &magnitude, false),
                series("fit", "Fit |χ(R)|", &r, &model_r, true),
            ],
        ),
        figure(
            "fit-q",
            "Fit · χ(q)",
            "q (Å⁻¹)",
            format!("Re χ(q) {}", weighted_unit(kw)),
            vec![
                series("data", "Data", &q, result.data_chiq.as_slice(), false),
                series("fit", "Fit", &q, result.model_chiq.as_slice(), true),
            ],
        ),
        figure(
            "residual-k",
            "Residual · k",
            "k (Å⁻¹)",
            format!("Residual {}", weighted_chi_label(kw)),
            vec![series(
                "residual",
                "Residual",
                &k,
                &data_k
                    .iter()
                    .zip(&model_k)
                    .map(|(a, b)| a - b)
                    .collect::<Vec<_>>(),
                false,
            )],
        ),
        figure(
            "residual-r",
            "Residual · R",
            "R (Å)",
            format!("Residual {}", plotting::chir_label(kw)),
            vec![series(
                "residual",
                "Residual",
                &r,
                &magnitude
                    .iter()
                    .zip(&model_r)
                    .map(|(a, b)| a - b)
                    .collect::<Vec<_>>(),
                false,
            )],
        ),
    ];
    // Fit + residual: the data and fit curves above a residual strip that
    // shares the x axis. Added before optional components so only the data
    // and fit appear in the upper panel.
    for (key, label, fit, residual) in [
        ("fit-residual-k", "Fit + residual · χ(k)", 0, 3),
        ("fit-residual-r", "Fit + residual · |χ(R)|", 1, 4),
    ] {
        let figure = FigureData {
            key,
            label: label.into(),
            residual: Some(output[residual].series.clone()),
            ..output[fit].clone()
        };
        output.push(figure);
    }
    for (key, label, values) in [
        ("real-data", "Re data", &result.data_chir_re),
        ("real-fit", "Re fit", &result.model_chir_re),
        ("imag-data", "Im data", &result.data_chir_im),
        ("imag-fit", "Im fit", &result.model_chir_im),
    ] {
        output[1]
            .series
            .push(series(key, label, &r, values.as_slice(), true));
    }
    for (index, path) in result.path_contributions.iter().enumerate() {
        let key = format!("path-{index}");
        output[0]
            .series
            .push(series(&key, &path.label, &k, &weighted(&path.chi), true));
        output[1].series.push(series(
            &key,
            &path.label,
            &r,
            path.chir_mag.as_slice(),
            true,
        ));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publication_presets_follow_processing_ranges_and_keep_full_csv_data() {
        let file = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../rexafs/tests/testfiles/xraylarch_d867/xafsdata/cu_150k.xmu");
        let sp = Arc::new(
            crate::params::process_file(
                &file,
                &PipelineParams {
                    fft_kmin: Some(3.),
                    fft_kmax: Some(10.),
                    fft_kweight: Some(3.),
                    ..Default::default()
                },
            )
            .unwrap(),
        );
        let figures = spectrum_figures(sp.clone(), "Cu_foil #1 [test] $sample");
        let options = FigureOptions::default();
        assert!(options.legend && options.grid.unwrap_or(true));
        assert!(!options.guides);
        assert_eq!(options.dimensions().2, 300.);
        assert_eq!(
            figures
                .iter()
                .filter(|f| f.common())
                .map(|f| f.key)
                .collect::<Vec<_>>(),
            ["xanes", "flattened-mu", "chi-k", "chi-r"]
        );
        let xanes = &figures[0];
        let e0 = sp.e0().unwrap();
        assert_eq!(xanes.default_xlim, Some((e0 - 20., e0 + 80.)));
        assert_eq!(
            figures[1].default_xlim,
            Some((
                sp.energy.as_ref().unwrap()[0],
                sp.energy.as_ref().unwrap().max()
            ))
        );
        let chik = &figures[2];
        assert_eq!(chik.default_xlim, Some((0., 11.)));
        assert_eq!(
            chik.series.len(),
            1,
            "FFT window is not a publication curve"
        );
        assert_eq!(
            chik.series[0].y,
            sp.k()
                .unwrap()
                .iter()
                .zip(sp.chi().unwrap())
                .map(|(k, chi)| chi * k.powf(3.))
                .collect::<Vec<_>>()
        );
        assert!(chik.ylabel.contains("k³"));
        assert_eq!(figures[3].default_xlim, Some((0., 6.)));
        assert_eq!(figures[3].series.len(), 1);
        let normalized = FigureOptions {
            normalized: true,
            ..Default::default()
        };
        assert_eq!(
            xanes.series(&normalized)[0].y,
            sp.norm().unwrap().as_slice()
        );
        assert!(xanes.csv(&normalized).unwrap().contains("Normalized μ(E)"));
        assert_eq!(
            xanes.csv(&options).unwrap().lines().count(),
            sp.energy.as_ref().unwrap().len() + 1
        );
        if let Some(dir) = std::env::var_os("REXAFS_PUBLICATION_QA_DIR") {
            let dir = Path::new(&dir);
            fs::create_dir_all(dir).unwrap();
            for f in &figures {
                let rendered = render_figure(f, &options).unwrap();
                assert!(rendered.svg.contains("data-ruviz-text-engine=\"typst\""));
                fs::write(dir.join(format!("{}.png", f.key)), rendered.png).unwrap();
                fs::write(dir.join(format!("{}.svg", f.key)), rendered.svg).unwrap();
            }
            fs::write(
                dir.join("xanes-normalized.png"),
                render_figure(xanes, &normalized).unwrap().png,
            )
            .unwrap();
            let narrow = FigureOptions {
                width: Some(3.3),
                height: Some(2.5),
                font_size: Some(9.),
                ..Default::default()
            };
            fs::write(
                dir.join("xanes-3.3in-9pt.png"),
                render_figure(xanes, &narrow).unwrap().png,
            )
            .unwrap();
        }
    }

    #[test]
    fn visible_interval_sets_y_scale_without_cropping_export_or_user_limits() {
        let mut data = sample();
        data.default_xlim = Some((0., 1.));
        data.series[0].y[2] = 1000.;
        let options = FigureOptions::default();
        assert_eq!(data.visible_ylim(&options), Some((-0.05, 1.05)));
        assert_eq!(data.csv(&options).unwrap().lines().count(), 4);
        let options = FigureOptions {
            xmin: Some(1.),
            xmax: Some(2.),
            ..Default::default()
        };
        assert_eq!(data.xlim(&options), Some((1., 2.)));
        assert!(data.visible_ylim(&options).unwrap().1 > 1000.);
    }

    #[test]
    fn k_space_autoscale_is_symmetric_about_zero() {
        let mut data = sample();
        data.default_xlim = Some((0., 2.));
        data.series[0].y = vec![-0.5, 2., 1.];
        let options = FigureOptions::default();
        let (lo, hi) = data.visible_ylim(&options).unwrap();
        assert!(lo > -1. && hi > 2.);
        data.key = "chi-k";
        let (lo, hi) = data.visible_ylim(&options).unwrap();
        assert_eq!(lo, -hi);
        assert!((hi - 2.2).abs() < 1e-12);
    }

    #[test]
    fn fit_r_defaults_show_magnitudes_and_expand_only_for_longer_fits() {
        let mut result = FeffFitResult {
            r: nalgebra::DVector::from_vec(vec![0., 2., 4., 6., 8.]),
            data_chir_re: nalgebra::DVector::from_vec(vec![0., 1., 0.5, 0.1, 0.]),
            data_chir_im: nalgebra::DVector::from_vec(vec![0., 0.2, 0.1, 0., 0.]),
            model_chir_mag: nalgebra::DVector::from_vec(vec![0., 0.95, 0.4, 0.1, 0.]),
            ..Default::default()
        };
        for (fit_max, display_max) in [
            (None, 6.),
            (Some(4.), 6.),
            (Some(6.), 6.),
            (Some(6.5), 7.),
            (Some(7.), 8.),
        ] {
            result.rmax = fit_max;
            let figures = fit_figures(&result);
            let fit = figures.iter().find(|f| f.key == "fit-r").unwrap();
            let options = FigureOptions::default();
            assert_eq!(fit.default_xlim, Some((0., display_max)));
            assert_eq!(
                fit.series
                    .iter()
                    .filter(|s| s.visible(&options))
                    .map(|s| s.key.as_str())
                    .collect::<Vec<_>>(),
                ["data", "fit"]
            );
            let options = FigureOptions {
                shown: BTreeSet::from(["real-data".into()]),
                ..Default::default()
            };
            assert!(
                fit.series
                    .iter()
                    .find(|s| s.key == "real-data")
                    .unwrap()
                    .visible(&options)
            );
            if fit_max == Some(4.)
                && let Some(dir) = std::env::var_os("REXAFS_PUBLICATION_QA_DIR")
            {
                fs::create_dir_all(&dir).unwrap();
                fs::write(
                    Path::new(&dir).join("fit-r.png"),
                    render_figure(fit, &FigureOptions::default()).unwrap().png,
                )
                .unwrap();
            }
        }
    }
    #[test]
    fn spectrum_r_range_follows_the_transform_window() {
        assert_eq!(spectrum_rmax(None, None), 6.);
        assert_eq!(spectrum_rmax(Some(3.), None), 6.);
        assert_eq!(spectrum_rmax(Some(7.2), None), 8.);
        // The 20 Å library default lies past the displayed transform.
        assert_eq!(spectrum_rmax(Some(20.), None), 6.);
        assert_eq!(spectrum_rmax(Some(7.2), Some(7.5)), 7.5);
        assert_eq!(spectrum_rmax(None, Some(5.)), 5.);
    }

    #[test]
    fn fit_residual_stacks_panels_on_a_shared_axis() {
        let k: Vec<f64> = (0..200).map(|i| i as f64 * 0.05).collect();
        let data: Vec<f64> = k
            .iter()
            .map(|k| (2. * k).sin() * (-0.1 * k).exp())
            .collect();
        let model: Vec<f64> = data.iter().map(|y| 0.9 * y).collect();
        let r: Vec<f64> = (0..100).map(|i| i as f64 * 0.06).collect();
        let peak: Vec<f64> = r
            .iter()
            .map(|r| (-(r - 2.2_f64).powi(2) * 8.).exp())
            .collect();
        let result = FeffFitResult {
            kweight: 2.,
            k: nalgebra::DVector::from_vec(k.clone()),
            data_chi: nalgebra::DVector::from_vec(data),
            model_chi: nalgebra::DVector::from_vec(model),
            r: nalgebra::DVector::from_vec(r.clone()),
            data_chir_re: nalgebra::DVector::from_vec(peak.clone()),
            data_chir_im: nalgebra::DVector::from_vec(vec![0.; r.len()]),
            model_chir_mag: nalgebra::DVector::from_vec(peak.iter().map(|y| 0.8 * y).collect()),
            ..Default::default()
        };
        let figures = fit_figures(&result);
        for key in ["fit-residual-k", "fit-residual-r"] {
            let figure = figures.iter().find(|f| f.key == key).unwrap();
            let residual = figure.residual.as_ref().unwrap();
            assert_eq!(residual[0].key, "residual");
            assert_eq!(
                figure
                    .series
                    .iter()
                    .map(|s| s.key.as_str())
                    .collect::<Vec<_>>(),
                ["data", "fit"]
            );
            let options = FigureOptions {
                width: Some(3.3),
                height: Some(3.),
                font_size: Some(9.),
                series_colors: BTreeMap::from([
                    ("data".into(), "#0072b2".into()),
                    ("fit".into(), "#e69f00".into()),
                    ("residual".into(), "#009e73".into()),
                ]),
                ..Default::default()
            };
            let csv = figure.csv(&options).unwrap();
            assert!(csv.lines().next().unwrap().contains("Residual"));
            assert!(figure.caption(&options).contains("lower panel"));
            let rendered = render_figure(figure, &options).unwrap();
            assert_eq!(png_size(&rendered.png), (990, 900));
            assert!(
                rendered
                    .svg
                    .contains(r#"width="3.3in" height="3in" viewBox="0 0 990 900""#)
            );
            assert!(rendered.svg.starts_with("<svg") && rendered.svg.matches("<svg").count() >= 3);
            assert!(rendered.svg.contains(r#"id="residual-"#));
            for stroke in ["rgb(0,114,178)", "rgb(230,159,0)", "rgb(0,158,115)"] {
                assert!(rendered.svg.contains(&format!(r#"stroke="{stroke}""#)));
            }
            let strip = rendered.svg.split(r#"id="residual-"#).nth(1).unwrap();
            assert!(strip.contains(r#"stroke="rgb(0,158,115)""#));
            // Built-in labels are sans text markup with plain accessible names.
            assert!(!figure.math_ylabel(&options).contains('$'));
            assert!(
                rendered
                    .svg
                    .contains(&format!(r#"aria-label="{}""#, figure.ylabel(&options)))
            );
            assert!(!rendered.svg.contains("#super["));
            if let Some(dir) = std::env::var_os("REXAFS_PUBLICATION_QA_DIR") {
                fs::create_dir_all(&dir).unwrap();
                fs::write(Path::new(&dir).join(format!("{key}.png")), &rendered.png).unwrap();
                fs::write(Path::new(&dir).join(format!("{key}.svg")), &rendered.svg).unwrap();
                fs::write(
                    Path::new(&dir).join(format!("{key}-default.png")),
                    render_figure(figure, &FigureOptions::default())
                        .unwrap()
                        .png,
                )
                .unwrap();
            }
        }
    }

    #[test]
    fn typed_labels_with_typst_markup_characters_render_literally() {
        let options = FigureOptions {
            title: Some("Ni #2 [foil] @beamline".into()),
            xlabel: Some("E_0 * <shift>".into()),
            ylabel: Some(r"Signal \ $mu(E)$".into()),
            ..Default::default()
        };
        let rendered = render_figure(&sample(), &options).unwrap();
        assert!(rendered.svg.contains("Ni #2 [foil] @beamline"));
        assert!(rendered.svg.contains("E_0 * &lt;shift&gt;"));
    }

    #[test]
    fn flattened_publication_preserves_library_values_in_figure_and_csv() {
        let file = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../rexafs/tests/testfiles/xraylarch_d867/xafsdata/cu_150k.xmu");
        let sp = Arc::new(crate::params::process_file(&file, &PipelineParams::default()).unwrap());
        let figures = spectrum_figures(sp.clone(), "Cu, \"reference\"");
        let flat = figures.iter().find(|f| f.key == "flattened-mu").unwrap();
        let norm = &flat.normalized_series.as_ref().unwrap()[0];
        assert_eq!(flat.series[0].y, sp.flat().unwrap().as_slice());
        assert_eq!(norm.y, sp.norm().unwrap().as_slice());
        assert_ne!(flat.series[0].y, norm.y);
        let csv = flat.csv(&FigureOptions::default()).unwrap();
        assert!(csv.starts_with("\"Cu, \"\"reference\"\""));
        for (row, (energy, value)) in csv
            .lines()
            .skip(1)
            .zip(flat.series[0].x.iter().zip(&flat.series[0].y))
        {
            let values: Vec<f64> = row.split(',').map(|s| s.parse().unwrap()).collect();
            assert_eq!(values, [*energy, *value]);
        }
        assert_eq!(csv.lines().count(), sp.energy.as_ref().unwrap().len() + 1);
        let empty = spectrum_figures(Arc::new(XASSpectrum::default()), "unprocessed");
        assert!(empty.iter().all(|f| f.key != "flattened-mu"));
    }

    #[test]
    fn csv_preserves_distinct_grids_and_respects_hidden_curves() {
        let mut figure = sample();
        figure.series.push(FigureSeries {
            key: "short".into(),
            label: "Other".into(),
            x: vec![0.5],
            y: vec![0.25],
            dashed: false,
            optional: false,
        });
        let csv = figure.csv(&FigureOptions::default()).unwrap();
        assert_eq!(csv.lines().count(), 4);
        assert!(csv.lines().nth(2).unwrap().ends_with(",,"));
        let hidden = FigureOptions {
            hidden: BTreeSet::from(["short".into()]),
            ..Default::default()
        };
        assert!(!figure.csv(&hidden).unwrap().contains("Other"));
        let hidden = FigureOptions {
            hidden: BTreeSet::from(["short".into(), "data".into()]),
            ..Default::default()
        };
        assert!(figure.csv(&hidden).is_err());
    }

    fn sample() -> FigureData {
        FigureData {
            key: "test",
            label: "Test".into(),
            xlabel: "Energy (eV)".into(),
            ylabel: "μ(E)".into(),
            math_ylabel: labels::MU_E.into(),
            normalized_series: None,
            default_xlim: None,
            residual: None,
            guides: vec![],
            series: vec![FigureSeries {
                key: "data".into(),
                label: "Data".into(),
                x: vec![0., 1., 2.],
                y: vec![0., 1., 0.],
                dashed: false,
                optional: false,
            }],
        }
    }
    fn png_size(bytes: &[u8]) -> (u32, u32) {
        (
            u32::from_be_bytes(bytes[16..20].try_into().unwrap()),
            u32::from_be_bytes(bytes[20..24].try_into().unwrap()),
        )
    }

    #[test]
    fn figure_series_colors_round_trip_and_old_options_keep_automatic_colors() {
        let old: FigureOptions = serde_json::from_str(r#"{"legend":false}"#).unwrap();
        assert!(old.series_colors.is_empty());
        assert!(
            !serde_json::to_string(&old)
                .unwrap()
                .contains("series_colors")
        );
        let options = FigureOptions {
            series_colors: BTreeMap::from([
                ("data".into(), "#0072b2".into()),
                ("future/key: [opaque]".into(), "#cc79a7".into()),
            ]),
            ..Default::default()
        };
        let json = serde_json::to_string(&options).unwrap();
        assert_eq!(
            serde_json::from_str::<FigureOptions>(&json).unwrap(),
            options
        );
    }

    #[test]
    fn explicit_series_colors_require_canonical_lowercase_rgb() {
        for value in [
            "",
            "#fff",
            "0072b2",
            "#0072B2",
            " #0072b2",
            "#0072b2 ",
            "#0072b280",
            "#gg72b2",
            "#é0a00",
        ] {
            let options = FigureOptions {
                series_colors: BTreeMap::from([("future/key".into(), value.into())]),
                ..Default::default()
            };
            let error = options.validate().unwrap_err();
            assert!(
                error.contains("future/key") && error.contains("#rrggbb"),
                "{error}"
            );
            assert!(sample().plot(&options).is_err(), "{value}");
        }
        for (_, value) in SERIES_PALETTE {
            let options = FigureOptions {
                series_colors: BTreeMap::from([("data".into(), value.into())]),
                ..Default::default()
            };
            assert!(options.validate().is_ok(), "{value}");
        }
    }

    #[test]
    fn explicit_svg_series_color_preserves_unset_curves_and_ignores_unknown_keys() {
        fn curve_strokes(svg: &str) -> Vec<&str> {
            svg.split("<polyline ")
                .skip(1)
                .map(|line| {
                    line.split("stroke=\"")
                        .nth(1)
                        .unwrap()
                        .split('"')
                        .next()
                        .unwrap()
                })
                .collect()
        }
        let mut data = sample();
        for (key, y) in [("fit", vec![0., 0.8, 0.1]), ("other", vec![0.1, 0.5, 0.2])] {
            data.series.push(FigureSeries {
                key: key.into(),
                label: key.into(),
                x: vec![0., 1., 2.],
                y,
                dashed: key == "fit",
                optional: false,
            });
        }
        let defaults = FigureOptions::default();
        let automatic = data.plot(&defaults).unwrap().render_to_svg().unwrap();
        let options = FigureOptions {
            series_colors: BTreeMap::from([
                ("data".into(), "#0072b2".into()),
                ("unknown/key".into(), "#cc79a7".into()),
            ]),
            ..Default::default()
        };
        let svg = data.plot(&options).unwrap().render_to_svg().unwrap();
        let before = curve_strokes(&automatic);
        let after = curve_strokes(&svg);
        assert_eq!(before.len(), 3);
        assert_eq!(after.len(), 3);
        assert_eq!(after[0], "rgb(0,114,178)");
        assert_eq!(after[1..], before[1..]);
        assert!(!svg.contains("rgb(204,121,167)"));
        assert_eq!(data.csv(&options).unwrap(), data.csv(&defaults).unwrap());

        let hidden_defaults = FigureOptions {
            hidden: BTreeSet::from(["data".into()]),
            ..Default::default()
        };
        let hidden_options = FigureOptions {
            series_colors: options.series_colors,
            ..hidden_defaults.clone()
        };
        let before = data
            .plot(&hidden_defaults)
            .unwrap()
            .render_to_svg()
            .unwrap();
        let after = data.plot(&hidden_options).unwrap().render_to_svg().unwrap();
        assert_eq!(curve_strokes(&before), curve_strokes(&after));
    }

    #[test]
    fn publication_defaults_to_300_dpi_and_custom_dpi_preserves_shape() {
        let defaults = Plot::new().dpi(300);
        let canvas = defaults.get_config().canvas_size();
        let rendered = render_figure(&sample(), &FigureOptions::default()).unwrap();
        assert_eq!(png_size(&rendered.png), canvas);
        assert!(rendered.svg.contains(">Energy (eV)</tspan>"));
        assert!(
            rendered.svg.contains(">Data</tspan>"),
            "legend label is text"
        );
        assert!(
            rendered.svg.matches("<text").count() >= 5,
            "tick numbers are text"
        );
        assert!(rendered.svg.contains(r#"role="img" aria-label="μ(E)""#));
        assert!(
            !rendered.svg.contains("_μ_"),
            "markup is not an accessible name"
        );
        let options = FigureOptions {
            width: Some(4.),
            height: Some(3.),
            dpi: Some(200.),
            title: Some("Copper · $k^2 chi(k)$".into()),
            xlabel: Some("Photon energy".into()),
            ..Default::default()
        };
        let rendered = render_figure(&sample(), &options).unwrap();
        assert_eq!(png_size(&rendered.png), (800, 600));
        assert!(rendered.svg.contains("data-ruviz-text-engine=\"typst\""));
        assert!(rendered.svg.contains("class=\"typst-doc\""));
        assert!(rendered.svg.contains("Photon energy"));
        assert!(
            rendered
                .svg
                .contains(r#"width="4in" height="3in" viewBox="0 0 800 600""#)
        );
    }
    #[test]
    fn svg_uses_physical_size_with_pixel_view_box() {
        let options = FigureOptions {
            width: Some(3.3),
            height: Some(2.5),
            ..Default::default()
        };
        let svg = render_figure(&sample(), &options).unwrap().svg;
        let start = svg.find("<svg").unwrap();
        let tag = &svg[start..start + svg[start..].find('>').unwrap()];
        assert!(
            tag.contains(r#"width="3.3in" height="2.5in" viewBox="0 0 990 750""#),
            "{tag}"
        );
        let mut options = FigureOptions::default();
        StylePreset::SingleColumn.apply(&mut options);
        let svg = render_figure(&sample(), &options).unwrap().svg;
        assert!(svg.contains(r#"width="3.5in" height="2.6in" viewBox="0 0 2100 1560""#));
        // An SVG that already has a viewBox is left untouched.
        let sized = r#"<svg width="1in" height="1in" viewBox="0 0 9 9"></svg>"#;
        assert_eq!(physical_svg_size(sized.into(), (9, 9), 300.), sized);
    }
    #[test]
    fn style_presets_and_apply_to_all_copy_only_appearance_fields() {
        let mut settings = FigureSettings::default();
        let source = settings.figures.entry("xanes".into()).or_default();
        StylePreset::SingleColumn.apply(source);
        source.title = Some("Copper".into());
        source.xmin = Some(1.);
        source.xmax = Some(2.);
        source.legend = false;
        source.guides = true;
        source.series_colors.insert("data".into(), "#0072b2".into());
        assert!(StylePreset::SingleColumn.matches(source));
        assert!(!StylePreset::Slide.matches(source));
        assert!(source.validate().is_ok());
        settings.figures.entry("chik".into()).or_default().ylabel = Some("kept".into());
        settings
            .figures
            .entry("chik".into())
            .or_default()
            .series_colors
            .insert("data".into(), "#e69f00".into());
        settings.apply_style_to_all("xanes", ["chir"]);
        for key in ["chik", "chir"] {
            let copied = settings.options(key);
            assert_eq!(copied.width, Some(3.5));
            assert_eq!(copied.dpi, Some(600.));
            assert_eq!(copied.font_size, Some(8.));
            assert!(!copied.legend && copied.guides);
            assert!(copied.title.is_none() && copied.xmin.is_none());
        }
        assert_eq!(settings.options("chik").ylabel.as_deref(), Some("kept"));
        assert_eq!(settings.options("chik").series_colors["data"], "#e69f00");
        assert!(settings.options("chir").series_colors.is_empty());
        for preset in StylePreset::ALL {
            let mut options = FigureOptions::default();
            preset.apply(&mut options);
            assert!(options.validate().is_ok(), "{}", preset.label());
        }
    }
    #[test]
    fn invalid_output_dimensions_and_limits_are_rejected_before_rendering() {
        for options in [
            FigureOptions {
                width: Some(f64::NAN),
                ..Default::default()
            },
            FigureOptions {
                dpi: Some(0.),
                ..Default::default()
            },
            FigureOptions {
                width: Some(30.),
                height: Some(30.),
                dpi: Some(1200.),
                ..Default::default()
            },
            FigureOptions {
                xmin: Some(2.),
                xmax: Some(1.),
                ..Default::default()
            },
            FigureOptions {
                xmin: Some(0.),
                ..Default::default()
            },
        ] {
            assert!(render_figure(&sample(), &options).is_err());
        }
    }
}
