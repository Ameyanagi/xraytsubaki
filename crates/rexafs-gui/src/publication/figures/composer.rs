//! Multi-panel publication figures: a grid of labelled panels, (a), (b), …,
//! at a journal column width, with one font size, line width and resolution
//! shared by every panel.
//!
//! Each panel is an ordinary publication figure (see [`FigureData`]) rendered
//! by ruviz on its own canvas. rexafs gives every panel the same fixed margins,
//! the widest that any panel needs, so the plot areas of a row share their top
//! and bottom edges and the plot areas of a column share their left and right
//! edges. The panels are then tiled into one raster image and nested into one
//! SVG, the same technique as the fit + residual figure in [`super::layout`].
//! Offsets apply to copied display arrays; the source spectra never change.
use super::layout::{self, Frame, LegendFrame, Metrics};
use super::{FigureData, FigureOptions, FigureSettings, LegendPlacement};
use crate::group_identity::GroupId;
use ruviz::core::Image;
use ruviz::prelude::Plot;
use serde::{Deserialize, Serialize};
use std::borrow::Borrow;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

mod offsets;
#[cfg(test)]
mod print_style;

mod inset;
mod inset_data;

/// Region or comparison displayed on a panel's independent inset axes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum InsetKind {
    /// The panel's own curves near the pre-edge region.
    PreEdge,
    /// The panel's own curves across the edge and white line.
    Edge,
    /// Normalized μ(E) minus the first selected sample's normalized μ(E).
    Difference,
}

impl InsetKind {
    /// Available insets, in control order; None is represented by no inset.
    pub const ALL: [Self; 3] = [Self::PreEdge, Self::Edge, Self::Difference];

    /// Short control label.
    pub fn label(self) -> &'static str {
        match self {
            Self::PreEdge => "Pre-edge",
            Self::Edge => "Edge",
            Self::Difference => "Difference",
        }
    }

    /// Recommended energy limits relative to the first sample's E₀, in eV.
    /// These display windows are rexafs choices, not processing parameters.
    pub fn default_range(self) -> (f64, f64) {
        match self {
            Self::PreEdge => (-6., 11.),
            Self::Edge => (-2., 23.),
            Self::Difference => (-6., 31.),
        }
    }
}

/// Inset position inside the main plot frame, clear of its axis labels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum InsetPlacement {
    #[default]
    LowerRight,
    UpperLeft,
    CenterRight,
}

impl InsetPlacement {
    /// Available placements, starting with the default.
    pub const ALL: [Self; 3] = [Self::LowerRight, Self::UpperLeft, Self::CenterRight];

    /// Short control label.
    pub fn label(self) -> &'static str {
        match self {
            Self::LowerRight => "Lower right",
            Self::UpperLeft => "Upper left",
            Self::CenterRight => "Center right",
        }
    }
}

/// An optional XANES inset. Limits are E − E₀ in eV, with E₀ taken from the
/// first selected sample before reversing stacking positions. Defaults place
/// the inset at the lower right and scale the panel's display offsets to its
/// own single-spectrum y span. Source arrays and processing never change.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct PanelInset {
    /// Zoomed region or normalized difference comparison.
    pub kind: InsetKind,
    /// Increasing (start, end) values of E − E₀, in eV.
    pub energy_range: (f64, f64),
    /// Lower right by default, wholly inside the main plot frame.
    #[serde(default)]
    pub placement: InsetPlacement,
    /// Scale and apply the panel's display offsets; true by default. False
    /// overlays inset curves, leaving the main panel's offsets unchanged.
    #[serde(default = "follow_inset_offset")]
    pub follow_offset: bool,
}

fn follow_inset_offset() -> bool {
    true
}

impl PanelInset {
    /// Construct the recommended range and appearance for this inset kind.
    pub fn new(kind: InsetKind) -> Self {
        Self {
            kind,
            energy_range: kind.default_range(),
            placement: InsetPlacement::default(),
            follow_offset: true,
        }
    }
}

/// Millimetres per inch (exact, by definition of the inch).
pub(crate) const MM_PER_INCH: f64 = 25.4;
/// Most panels one figure can hold, so labels stay single letters (a)–(l).
pub(crate) const MAX_PANELS: usize = 12;
/// Automatic panel height as a fraction of the panel width (rexafs choice: 3:4).
const AUTO_ASPECT: f64 = 0.75;
/// Largest composed image, in pixels (about 160 MB of RGBA while rendering).
const MAX_PIXELS: f64 = 40_000_000.;

/// A published figure-width requirement: `mm` is the full printed width of a
/// figure that spans one column or the whole text width of the page.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct JournalWidth {
    pub publisher: &'static str,
    /// "1 column" or "2 columns".
    pub span: &'static str,
    pub mm: f64,
}

impl JournalWidth {
    /// Chip label, e.g. "ACS · 1 column".
    pub fn label(&self) -> String {
        format!("{} · {}", self.publisher, self.span)
    }
}

/// Maximum figure widths from the publishers' author guides, checked in
/// October 2026:
///
/// - ACS: single-column graphics up to 240 pt (3.33 in, 84.7 mm) and
///   double-column graphics up to 504 pt (7 in, 177.8 mm), from the JACS
///   author guidelines, "Size" section
///   (<https://researcher-resources.acs.org/publish/author_guidelines?coden=jacsat>).
///   Issue #143 listed an older 3.25 in (82.5 mm) value; the current guide's
///   240 pt is used here.
/// - RSC: 8.3 cm and 17.1 cm, from the RSC's Natural Product Reports
///   submission guidelines
///   (<https://rsc.org/globalassets/05-journals-books-databases/our-journals/natural-product-reports/npr-viewpoint-submission-guidelines.pdf>).
/// - Elsevier: 90 mm and 190 mm, from "Artwork sizing"
///   (<https://www.elsevier.com/about/policies-and-standards/author/artwork-and-media-instructions/artwork-sizing>).
///
/// Individual journals can differ; check the target journal's guide.
pub(crate) const JOURNAL_WIDTHS: [JournalWidth; 6] = [
    JournalWidth {
        publisher: "ACS",
        span: "1 column",
        mm: 240. / 72. * MM_PER_INCH,
    },
    JournalWidth {
        publisher: "ACS",
        span: "2 columns",
        mm: 177.8,
    },
    JournalWidth {
        publisher: "RSC",
        span: "1 column",
        mm: 83.,
    },
    JournalWidth {
        publisher: "RSC",
        span: "2 columns",
        mm: 171.,
    },
    JournalWidth {
        publisher: "Elsevier",
        span: "1 column",
        mm: 90.,
    },
    JournalWidth {
        publisher: "Elsevier",
        span: "2 columns",
        mm: 190.,
    },
];

/// How a panel identifies its curves. The default retains its figure legend.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CurveLabelMode {
    #[default]
    Legend,
    /// Complete figure labels beside the last visible point. Labels exceeding
    /// the 35% panel-width allowance produce a validation error.
    CurveEnd,
}

/// Default colours for panels containing multiple samples. rexafs chooses one
/// recipe-wide order: shared IDs selected by a spectrum panel first, then
/// panel-only IDs in panel/list order, deduplicated with missing IDs retained.
/// This project choice keeps automatic colours consistent across panel subsets
/// and reorderings. Explicit full-key per-figure colours and recipe colours
/// take precedence over this palette.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SampleColours {
    /// ruviz's Viridis map, evenly sampled from 0 to 0.82 in recipe-wide order.
    /// rexafs omits the brightest end to improve readability on white paper.
    /// Viridis was designed by Nathaniel Smith and Stéfan van der Walt:
    /// <https://bids.github.io/colormap/>.
    #[default]
    Ordered,
    /// Eight categorical colours from Okabe and Ito's colour-universal design
    /// palette (2008), cycling after eight samples:
    /// <https://jfly.uni-koeln.de/color/>.
    OkabeIto,
    /// Use each sample's current workspace plot colour.
    Workspace,
}

impl SampleColours {
    fn color(self, position: usize, count: usize, workspace: &str) -> String {
        match self {
            Self::Ordered => {
                let t = 0.82 * position as f64 / count.saturating_sub(1).max(1) as f64;
                let color = ruviz::render::ColorMap::viridis().sample(t);
                format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b)
            }
            Self::OkabeIto => {
                const PALETTE: [&str; 8] = [
                    "#000000", "#e69f00", "#56b4e9", "#009e73", "#f0e442", "#0072b2", "#d55e00",
                    "#cc79a7",
                ];
                PALETTE[position % PALETTE.len()].into()
            }
            Self::Workspace => workspace.into(),
        }
    }
}

/// One panel in reading order. Old projects may store only the figure key;
/// missing object fields use the same current-sample, unshifted defaults.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(from = "SavedPanel")]
pub(crate) struct CompositePanel {
    /// Publication figure type, such as `xanes`, `chi-k` or `fit-residual-k`.
    pub key: String,
    /// None inherits recipe samples; an explicitly empty list uses the current
    /// sample. Fit and residual panels always use the current fit instead.
    pub samples: Option<Vec<GroupId>>,
    /// Display displacement between samples, in vertical-axis units; zero
    /// overlays their unshifted curves. Source arrays are never modified.
    pub offset_step: f64,
    /// Put the first sample highest for a positive step, without reordering it.
    pub reverse: bool,
    /// Legend by default; curve-end labels reserve space on the panel's right.
    pub label_mode: CurveLabelMode,
    /// Independent zoom or difference axes; absent in older projects and off
    /// by default. Only energy-domain μ(E) panels support insets.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inset: Option<PanelInset>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum SavedPanel {
    Key(String),
    Options {
        key: String,
        #[serde(default)]
        samples: Option<Vec<GroupId>>,
        #[serde(default)]
        offset_step: f64,
        #[serde(default)]
        reverse: bool,
        #[serde(default)]
        label_mode: CurveLabelMode,
        #[serde(default)]
        inset: Option<PanelInset>,
    },
}

impl From<SavedPanel> for CompositePanel {
    fn from(saved: SavedPanel) -> Self {
        match saved {
            SavedPanel::Key(key) => key.into(),
            SavedPanel::Options {
                key,
                samples,
                offset_step,
                reverse,
                label_mode,
                inset,
            } => Self {
                key,
                samples,
                offset_step,
                reverse,
                label_mode,
                inset,
            },
        }
    }
}

impl From<String> for CompositePanel {
    fn from(key: String) -> Self {
        Self {
            key,
            ..Self::default()
        }
    }
}

impl From<&str> for CompositePanel {
    fn from(key: &str) -> Self {
        key.to_owned().into()
    }
}

impl CompositePanel {
    /// Only raw, flattened and normalized energy-domain μ(E) panels can show
    /// an inset; EXAFS and fit panels hide the control.
    pub fn supports_inset(&self) -> bool {
        matches!(self.key.as_str(), "xanes" | "flattened-mu" | "mu-energy")
    }

    /// Fits and residuals describe one fitted dataset and cannot be overlaid
    /// across samples by the composer.
    pub fn uses_current_fit(&self) -> bool {
        self.key.starts_with("fit-") || self.key.starts_with("residual-")
    }
}

/// Resolved workspace identity and appearance, with that sample's processed
/// publication data. The caller supplies stable IDs, current labels and colours.
#[derive(Clone)]
pub(crate) struct CompositeSample {
    pub id: GroupId,
    /// Resolve an empty sample selection against the current workspace group.
    pub is_current: bool,
    pub label: String,
    /// Workspace plot colour as an opaque lowercase `#rrggbb` value.
    pub color: String,
    pub figures: Vec<Arc<FigureData>>,
}

/// A saved multi-panel figure: which figures form the panels and the
/// appearance they share. Stored with the figure settings in the `.rxs`
/// project as the contents of a named entry in `publication.recipes`.
///
/// Per-figure settings from the single-figure editor still apply inside each
/// panel: axis labels, axis limits, visible curves, guides and the
/// flattened/normalized choice. The recipe replaces size, resolution, font
/// size, line width and grid, and drops titles because panel labels identify
/// panels. Main axes have inward ticks on all sides and four minor ticks per
/// interval. Inset energy ticks point inward, with no grid or legend. Main
/// legends use automatic inside placement. Explicit recipe
/// series colours override per-figure colours and the sample palette.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct CompositeRecipe {
    /// Panel specifications in reading order: left to right, then top to bottom.
    pub panels: Vec<CompositePanel>,
    /// Ordered sample IDs shared by panels without their own selection. Empty
    /// uses the current sample. Missing IDs are retained and reported on render.
    pub samples: Vec<GroupId>,
    /// Short figure labels by durable sample ID. Empty or whitespace-only
    /// values use the workspace label. Captions record differing full names
    /// once; missing IDs and their labels are retained across project saves.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub figure_labels: BTreeMap<GroupId, String>,
    /// Multi-sample palette in recipe-wide sample order; Ordered defaults to
    /// Viridis over 0–0.82. A panel containing one selected sample keeps its
    /// ordinary figure colours.
    pub sample_colours: SampleColours,
    /// Show grid lines in every composed panel, replacing its figure setting;
    /// false by default. Insets keep their grid off. Single-figure exports
    /// retain their own grid setting.
    pub grid: bool,
    /// Repeat multi-sample legends; false shows only the first eligible one.
    /// Fit and single-sample panels retain their own legend visibility; insets
    /// never show a legend.
    pub legend_every_panel: bool,
    /// Panels per row, 1–4; 2 by default. A figure with fewer panels uses
    /// one column per panel.
    pub columns: usize,
    /// Total figure width in millimetres; 177.8 mm (ACS two columns) by default.
    pub width_mm: f64,
    /// Height of each panel in millimetres; None uses ¾ of the panel width.
    pub panel_height_mm: Option<f64>,
    /// Font size in points for every panel, 4–48; 8 pt by default.
    pub font_size: f64,
    /// Curve width in points, 0.1–12; None keeps the renderer default.
    pub line_width: Option<f64>,
    /// Opaque RGB colours by series key, in lowercase `#rrggbb`
    /// format, overriding matching per-figure colours in every panel. Missing
    /// keys inherit figure colours, then the renderer's automatic palette;
    /// unknown keys are retained and ignored when drawing.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub series_colors: BTreeMap<String, String>,
    /// Raster resolution of PNG and TIFF output, an integer 72–1200 DPI;
    /// 600 by default. SVG and PDF are vector and print at the same size.
    pub dpi: f64,
}

impl Default for CompositeRecipe {
    fn default() -> Self {
        Self {
            panels: Vec::new(),
            samples: Vec::new(),
            figure_labels: BTreeMap::new(),
            sample_colours: SampleColours::default(),
            grid: false,
            legend_every_panel: false,
            columns: 2,
            width_mm: 177.8,
            panel_height_mm: None,
            font_size: 8.,
            line_width: None,
            series_colors: BTreeMap::new(),
            dpi: 600.,
        }
    }
}

/// Pixel geometry of a composed figure.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Grid {
    pub columns: usize,
    pub rows: usize,
    /// Width and height of one panel in pixels.
    pub cell_px: (u32, u32),
    pub dpi: f64,
}

impl Grid {
    /// Whole-figure size in pixels.
    pub fn size_px(&self) -> (u32, u32) {
        (
            self.cell_px.0 * self.columns as u32,
            self.cell_px.1 * self.rows as u32,
        )
    }

    /// Whole-figure size in millimetres, as written to the SVG and PDF.
    pub fn size_mm(&self) -> (f64, f64) {
        let (w, h) = self.size_px();
        (
            w as f64 / self.dpi * MM_PER_INCH,
            h as f64 / self.dpi * MM_PER_INCH,
        )
    }
}

impl CompositeRecipe {
    /// Resolve a short figure label without changing the workspace name.
    fn figure_label<'a>(&'a self, sample: &'a CompositeSample) -> &'a str {
        self.figure_labels
            .get(&sample.id)
            .map(String::as_str)
            .filter(|label| !label.trim().is_empty())
            .unwrap_or(&sample.label)
    }

    /// Starting panels for a new recipe: XANES, χ(k) and |χ(R)| of the current
    /// spectrum, plus the k-space fit + residual when a fit is available.
    pub fn suggested_panels<'a>(
        available: impl IntoIterator<Item = &'a str>,
    ) -> Vec<CompositePanel> {
        let available: Vec<&str> = available.into_iter().collect();
        ["xanes", "chi-k", "chi-r", "fit-residual-k"]
            .into_iter()
            .filter(|key| available.contains(key))
            .map(CompositePanel::from)
            .collect()
    }

    /// Panel geometry for `panels` rendered panels at `dpi`.
    ///
    /// Panel width is the figure width divided by the column count, rounded
    /// down to whole pixels, so the composed figure is at most one pixel per
    /// column narrower than `width_mm`. A tolerance of 10⁻⁶ px keeps exact
    /// widths such as 240 pt at 600 DPI (2000 px) from losing a pixel to
    /// floating-point error in the millimetre conversion.
    pub fn grid(&self, panels: usize, dpi: f64) -> Grid {
        let columns = self.columns.clamp(1, 4).min(panels.max(1));
        let rows = panels.max(1).div_ceil(columns);
        let width_px = (self.width_mm / MM_PER_INCH * dpi / columns as f64 + 1e-6).floor();
        let height_px = match self.panel_height_mm {
            Some(mm) => mm / MM_PER_INCH * dpi,
            None => width_px * AUTO_ASPECT,
        };
        Grid {
            columns,
            rows,
            cell_px: (width_px.max(0.) as u32, height_px.round().max(0.) as u32),
            dpi,
        }
    }

    /// Check the recipe before rendering `panels` panels. Each panel must be
    /// at least 1 inch (25.4 mm) on each side, the limit of a single
    /// publication figure, and the composed raster at most 40 million pixels.
    /// Explicit series colours must use lowercase `#rrggbb` RGB notation.
    pub fn validate(&self, panels: usize) -> Result<(), String> {
        if self
            .panels
            .iter()
            .any(|panel| !panel.uses_current_fit() && !panel.offset_step.is_finite())
        {
            return Err("Use a finite display offset in vertical-axis units.".into());
        }
        if panels == 0 {
            return Err("Add at least one panel with available data.".into());
        }
        if panels > MAX_PANELS {
            return Err(format!("Use at most {MAX_PANELS} panels."));
        }
        if !(1..=4).contains(&self.columns) {
            return Err("Use 1–4 columns.".into());
        }
        if !self.dpi.is_finite() || !(72.0..=1200.0).contains(&self.dpi) || self.dpi.fract() != 0. {
            return Err("Use an integer DPI of 72–1200.".into());
        }
        for (name, value, low, high) in [
            ("Font size", Some(self.font_size), 4.0, 48.0),
            ("Line width", self.line_width, 0.1, 12.0),
        ] {
            if value.is_some_and(|v| !v.is_finite() || !(low..=high).contains(&v)) {
                return Err(format!("{name} must be {low}–{high} points."));
            }
        }
        if !self.width_mm.is_finite()
            || self
                .panel_height_mm
                .is_some_and(|h| !h.is_finite() || h > 30. * MM_PER_INCH)
        {
            return Err("Enter a finite width and panel height in millimetres.".into());
        }
        let grid = self.grid(panels, self.dpi);
        let (w, h) = grid.cell_px;
        if (w as f64) < self.dpi || (h as f64) < self.dpi {
            return Err(
                "Each panel must be at least 25.4 mm (1 in) wide and tall: use fewer columns, a wider figure or taller panels."
                    .into(),
            );
        }
        let (w, h) = grid.size_px();
        if w as f64 > 30. * self.dpi || w as f64 * h as f64 > MAX_PIXELS {
            return Err(
                "Reduce the size or DPI to keep the figure below 40 million pixels and 30 in wide."
                    .into(),
            );
        }
        super::validate_series_colors(&self.series_colors)
    }

    /// Settings for one panel: the figure type's own options with this
    /// recipe's size, resolution, font size, line width and series colours,
    /// and no title. Recipe colours replace matching figure keys only.
    pub(crate) fn panel_options(&self, mut base: FigureOptions, grid: &Grid) -> FigureOptions {
        base.series_colors.extend(self.series_colors.clone());
        FigureOptions {
            width: Some(grid.cell_px.0 as f64 / grid.dpi),
            height: Some(grid.cell_px.1 as f64 / grid.dpi),
            dpi: Some(grid.dpi),
            font_size: Some(self.font_size),
            line_width: self.line_width,
            title: None,
            grid: Some(self.grid),
            ..base
        }
    }
}

/// Panel label for the zero-based panel `index`: "(a)", "(b)", …
pub(crate) fn panel_label(index: usize) -> String {
    format!("({})", (b'a' + (index % 26) as u8) as char)
}

/// A rendered multi-panel figure.
pub(crate) struct ComposedFigure {
    /// Straight-alpha RGBA pixels of the whole figure.
    pub image: Image,
    /// Vector figure with a physical size in inches; plain labels are live text.
    pub svg: String,
    pub grid: Grid,
    /// Recipe panels skipped because the figure is not available (for example
    /// a fit panel before any fit has run).
    pub missing: Vec<String>,
    /// Caption with one "(a) …" sentence group per panel.
    pub caption: String,
}

impl ComposedFigure {
    pub fn png(&self) -> Result<Vec<u8>, String> {
        self.image
            .encode_png_with_dpi(self.grid.dpi as f32)
            .map_err(|e| e.to_string())
    }

    /// LZW-compressed RGB TIFF with the resolution recorded in pixels per inch.
    pub fn tiff(&self) -> Result<Vec<u8>, String> {
        use tiff::encoder::{Compression, Rational, TiffEncoder, colortype::RGB8};
        let rgb: Vec<u8> = self
            .image
            .pixels
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|[r, g, b, _]| [*r, *g, *b])
            .collect();
        let mut bytes = std::io::Cursor::new(Vec::new());
        let error = |e: tiff::TiffError| e.to_string();
        let mut encoder = TiffEncoder::new(&mut bytes)
            .map_err(error)?
            .with_compression(Compression::Lzw);
        let mut image = encoder
            .new_image::<RGB8>(self.image.width, self.image.height)
            .map_err(error)?;
        image.resolution(
            tiff::tags::ResolutionUnit::Inch,
            Rational {
                n: self.grid.dpi as u32,
                d: 1,
            },
        );
        image.write_data(&rgb).map_err(error)?;
        Ok(bytes.into_inner())
    }

    /// Vector PDF converted from the SVG by ruviz (svg2pdf); its page has the
    /// figure's physical size.
    pub fn pdf(&self) -> Result<Vec<u8>, String> {
        ruviz::export::svg_to_pdf(&self.svg).map_err(|e| e.to_string())
    }

    /// File bytes for an export format: `png`, `svg`, `pdf` or `tiff`.
    pub fn bytes(&self, extension: &str) -> Result<Vec<u8>, String> {
        match extension {
            "png" => self.png(),
            "svg" => Ok(self.svg.clone().into_bytes()),
            "pdf" => self.pdf(),
            "tiff" => self.tiff(),
            other => Err(format!("Unsupported figure format .{other}.")),
        }
    }
}

/// One panel, built but not yet framed.
enum Built {
    Single(Box<Plot>, Vec<EndLabel>),
    FitResidual(Box<layout::FitResidual>),
}

/// Display arrays and merged options for one available recipe panel. Preparation
/// copies figure data so offsets cannot change a cached processed spectrum.
pub(crate) struct PreparedPanel {
    /// Original recipe index; available panels may have skipped missing entries.
    pub index: usize,
    pub data: FigureData,
    pub options: FigureOptions,
    pub offset_caption: Option<String>,
    /// Shifts in source-series order; None leaves the ordinates unchanged.
    pub offsets: Vec<Option<DisplayOffset>>,
    /// Curves and independent limits resolved once by the preview worker.
    pub inset: Option<inset::PreparedInset>,
}

/// A transient display shift. Retaining the original y values lets data exports
/// preserve them exactly, without subtracting a rounded floating-point offset.
pub(crate) struct DisplayOffset {
    /// Additive displacement in the curve's vertical-axis units.
    pub value: f64,
    pub unshifted_y: Vec<f64>,
}

/// Available panels and reasons for omitted samples or panels. The editor reuses
/// the background preview's preparation, including full sample/series colour keys.
pub(crate) struct PreparedComposition {
    pub panels: Vec<PreparedPanel>,
    pub missing: Vec<String>,
    /// Renamed samples in first-use order: short figure label and full workspace
    /// name. Captions include this key once, independently of panel count.
    pub sample_names: Vec<(String, String)>,
}

/// Preserve ordinary single-source helper labels while replacing its main label
/// only when the recipe explicitly supplies one, in both normalization variants.
fn apply_figure_label(recipe: &CompositeRecipe, sample: &CompositeSample, data: &mut FigureData) {
    if recipe
        .figure_labels
        .get(&sample.id)
        .is_some_and(|label| !label.trim().is_empty())
    {
        for series in std::iter::once(&mut data.series).chain(data.normalized_series.iter_mut()) {
            if let Some(main) = series.first_mut() {
                main.label = recipe.figure_label(sample).into();
            }
        }
    }
}

fn record_sample_name(
    recipe: &CompositeRecipe,
    sample: &CompositeSample,
    visible: bool,
    seen: &mut BTreeSet<GroupId>,
    names: &mut Vec<(String, String)>,
) {
    let label = recipe.figure_label(sample);
    if visible && label != sample.label && seen.insert(sample.id.clone()) {
        names.push((label.into(), sample.label.clone()));
    }
}

/// One palette order for the whole composition, so a shared legend remains
/// meaningful when later panels select a subset or reorder their samples.
/// rexafs puts used shared IDs first, then panel-only IDs in reading order.
/// Missing IDs keep their slots and do not recolour surviving samples.
fn sample_colour_order(recipe: &CompositeRecipe) -> Vec<&GroupId> {
    let panel_ids: Vec<_> = recipe
        .panels
        .iter()
        .filter(|panel| !panel.uses_current_fit())
        .flat_map(|panel| panel.samples.as_deref().unwrap_or(&recipe.samples))
        .collect();
    let used: BTreeSet<_> = panel_ids.iter().copied().collect();
    let mut seen = BTreeSet::new();
    recipe
        .samples
        .iter()
        .filter(|id| used.contains(id))
        .chain(panel_ids)
        .filter(|id| seen.insert(*id))
        .collect()
}

/// Resolve panel sources without rendering. One selected sample retains all its
/// original curves and keys. Multiple selected samples contribute only their
/// first (main) curve, keyed as `group-id/series-key`, in saved sample order.
/// Labels come from the recipe or workspace; colours use the sample palette,
/// then full-key figure overrides followed by recipe overrides. Selected samples without
/// main-curve data are skipped and reported. Fits retain current data.
pub(crate) fn prepare_panels(
    recipe: &CompositeRecipe,
    current_figures: &[impl Borrow<FigureData>],
    samples: &[CompositeSample],
    settings: &FigureSettings,
) -> Result<PreparedComposition, String> {
    let mut prepared = PreparedComposition {
        panels: Vec::new(),
        missing: Vec::new(),
        sample_names: Vec::new(),
    };
    let mut named_samples = BTreeSet::new();
    let colour_order = sample_colour_order(recipe);
    for (index, panel) in recipe.panels.iter().enumerate() {
        if !panel.uses_current_fit() && !panel.offset_step.is_finite() {
            return Err(format!(
                "Panel {}: use a finite display offset in vertical-axis units.",
                panel.key
            ));
        }
        let mut options = settings.options(&panel.key);
        options.grid = Some(recipe.grid);
        let ids = panel.samples.as_deref().unwrap_or(&recipe.samples);
        if panel.uses_current_fit() || ids.is_empty() {
            match current_figures
                .iter()
                .map(Borrow::borrow)
                .find(|data| data.key == panel.key)
            {
                Some(data) => {
                    let mut data = data.clone();
                    if !panel.uses_current_fit()
                        && let Some(sample) = samples.iter().find(|sample| sample.is_current)
                    {
                        apply_figure_label(recipe, sample, &mut data);
                        record_sample_name(
                            recipe,
                            sample,
                            data.series(&options)
                                .first()
                                .is_some_and(|curve| curve.visible(&options)),
                            &mut named_samples,
                            &mut prepared.sample_names,
                        );
                    }
                    prepared.panels.push(PreparedPanel {
                        index,
                        data,
                        options,
                        offset_caption: None,
                        offsets: Vec::new(),
                        inset: None,
                    });
                }
                None => prepared.missing.push(panel.key.clone()),
            }
            continue;
        }
        let mut found = Vec::new();
        for (position, id) in ids.iter().enumerate() {
            let Some(sample) = samples.iter().find(|sample| sample.id == *id) else {
                prepared
                    .missing
                    .push(format!("{}: sample {} is missing", panel.key, id.as_str()));
                continue;
            };
            let Some(data) = sample
                .figures
                .iter()
                .find(|data| data.key == panel.key)
                .filter(|data| {
                    data.series(&options)
                        .first()
                        .is_some_and(|curve| !curve.x.is_empty())
                })
            else {
                prepared.missing.push(format!("{}: no processed data for {} ({}); select and process this sample if automatic processing could not prepare it", panel.key, sample.label, id.as_str()));
                continue;
            };
            found.push((position, sample, data));
        }
        // One explicitly selected sample retains ordinary helpers, colours and
        // optional curves; only an explicit figure label replaces its main name.
        if ids.len() == 1 {
            if let Some((_, sample, data)) = found.first() {
                let mut data = data.as_ref().clone();
                apply_figure_label(recipe, sample, &mut data);
                record_sample_name(
                    recipe,
                    sample,
                    data.series(&options)
                        .first()
                        .is_some_and(|curve| curve.visible(&options)),
                    &mut named_samples,
                    &mut prepared.sample_names,
                );
                prepared.panels.push(PreparedPanel {
                    index,
                    data,
                    options,
                    offset_caption: None,
                    offsets: Vec::new(),
                    inset: None,
                });
            }
            continue;
        }
        let Some((_, _, first)) = found.first() else {
            continue;
        };
        let mut data = first.as_ref().clone();
        data.ylabel = first.ylabel(&options).to_owned();
        data.math_ylabel = first.math_ylabel(&options).to_owned();
        data.series.clear();
        data.normalized_series = None;
        data.residual = None;
        data.guides.clear();
        data.default_xlim = None;
        let mut offsets = Vec::new();
        let mut display_offsets = Vec::new();
        let mut applied_offset = false;
        for (position, sample, source) in found {
            let Some(main) = source
                .series(&options)
                .first()
                .filter(|curve| !curve.x.is_empty())
            else {
                prepared.missing.push(format!(
                    "{}: no main curve for {} ({})",
                    panel.key,
                    sample.label,
                    sample.id.as_str()
                ));
                continue;
            };
            let mut curve = main.clone();
            curve.key = format!("{}/{}", sample.id.as_str(), main.key);
            curve.label = recipe.figure_label(sample).into();
            curve.optional = false;
            let offset_index = if panel.reverse {
                ids.len() - 1 - position
            } else {
                position
            };
            let offset = panel.offset_step * offset_index as f64;
            if !offset.is_finite() {
                return Err(format!(
                    "Panel {}: the display offset is too large.",
                    panel.key
                ));
            }
            display_offsets.push((offset != 0.).then(|| DisplayOffset {
                value: offset,
                unshifted_y: curve.y.clone(),
            }));
            if offset != 0. {
                for value in &mut curve.y {
                    let shifted = *value + offset;
                    if value.is_finite() && !shifted.is_finite() {
                        return Err(format!(
                            "Panel {}: the display offset is too large.",
                            panel.key
                        ));
                    }
                    *value = shifted;
                }
                applied_offset |= curve.visible(&options);
            }
            if curve.visible(&options) {
                let formatted = offsets::format_offset(offset, panel.offset_step);
                offsets.push(format!(
                    "{}: {}{}",
                    curve.label,
                    if formatted.starts_with('-') { "" } else { "+" },
                    formatted
                ));
            }
            // Multi-sample keys intentionally do not inherit a short main-curve
            // override: that would silently give every sample the same colour.
            options
                .series_colors
                .entry(curve.key.clone())
                .or_insert_with(|| {
                    let slot = colour_order
                        .iter()
                        .position(|id| **id == sample.id)
                        .unwrap();
                    recipe
                        .sample_colours
                        .color(slot, colour_order.len(), &sample.color)
                });
            record_sample_name(
                recipe,
                sample,
                curve.visible(&options),
                &mut named_samples,
                &mut prepared.sample_names,
            );
            data.series.push(curve);
            if let Some((min, max)) = source.default_xlim {
                data.default_xlim = Some(match data.default_xlim {
                    Some((low, high)) => (low.min(min), high.max(max)),
                    None => (min, max),
                });
            }
            data.guides.extend(source.guides.iter().copied());
        }
        if data.series.is_empty() {
            continue;
        }
        data.guides.sort_by(f64::total_cmp);
        data.guides.dedup();
        if applied_offset
            && options.ymin.is_none()
            && options.ymax.is_none()
            && let Some((min, max)) = data.visible_ylim_asymmetric(&options)
        {
            options.ymin = Some(min);
            options.ymax = Some(max);
        }
        prepared.panels.push(PreparedPanel {
            index, data, options,
            offsets: display_offsets,
            inset: None,
            offset_caption: applied_offset.then(|| format!(
                "Spectra offset vertically by {} in vertical-axis units for clarity. Display offsets in sample order: {}.", offsets::format_offset(panel.offset_step, panel.offset_step), offsets.join(", ")
            )),
        });
    }
    let mut legend_shown = false;
    for panel in &mut prepared.panels {
        let spec = &recipe.panels[panel.index];
        if !spec.uses_current_fit() {
            if spec.label_mode == CurveLabelMode::CurveEnd {
                panel.options.legend = false;
            } else if spec.samples.as_deref().unwrap_or(&recipe.samples).len() > 1
                && panel.options.legend
            {
                let multiple_visible = panel
                    .data
                    .series
                    .iter()
                    .filter(|curve| curve.visible(&panel.options) && !curve.x.is_empty())
                    .count()
                    > 1;
                panel.options.legend =
                    recipe.legend_every_panel || (multiple_visible && !legend_shown);
                legend_shown |= panel.options.legend;
            }
        }
        panel
            .options
            .series_colors
            .extend(recipe.series_colors.clone());
        panel.inset = inset_data::prepare(
            &recipe.panels[panel.index],
            recipe,
            panel,
            current_figures,
            samples,
        )?;
    }
    Ok(prepared)
}

/// Compose the recipe's panels from `figures` (the figures available for the
/// current spectrum and fit) using the per-figure options in `settings`.
///
/// `dpi` overrides the recipe's resolution, for a quicker preview; physical
/// sizes and the layout are unchanged. Panels missing from `figures` are
/// skipped and listed in [`ComposedFigure::missing`]; the remaining panels are
/// labelled consecutively. Errors leave the recipe and data unchanged.
#[cfg(test)]
pub(crate) fn compose(
    recipe: &CompositeRecipe,
    figures: &[impl Borrow<FigureData>],
    settings: &FigureSettings,
    dpi: Option<f64>,
) -> Result<ComposedFigure, String> {
    compose_samples(recipe, figures, &[], settings, dpi)
}

/// Compose current figures and resolved samples. `dpi` can lower preview
/// resolution while preserving physical dimensions. Missing inputs are skipped
/// and reported; an entirely missing composition returns a useful error.
pub(crate) fn compose_samples(
    recipe: &CompositeRecipe,
    current_figures: &[impl Borrow<FigureData>],
    samples: &[CompositeSample],
    settings: &FigureSettings,
    dpi: Option<f64>,
) -> Result<ComposedFigure, String> {
    let prepared = prepare_panels(recipe, current_figures, samples, settings)?;
    compose_prepared(recipe, &prepared, dpi)
}

/// Render previously resolved panels without repeating sample preparation.
/// The editor can retain this preparation for readiness and colour controls
/// even when rendering fails. `dpi` overrides the recipe's output resolution.
pub(crate) fn compose_prepared(
    recipe: &CompositeRecipe,
    prepared: &PreparedComposition,
    dpi: Option<f64>,
) -> Result<ComposedFigure, String> {
    let PreparedComposition {
        panels: chosen,
        missing,
        sample_names,
    } = prepared;
    recipe.validate(chosen.len()).map_err(|error| {
        if chosen.is_empty() && !missing.is_empty() {
            format!("{error} Missing: {}.", missing.join("; "))
        } else {
            error
        }
    })?;
    if dpi.is_some_and(|value| {
        !value.is_finite() || !(72.0..=1200.0).contains(&value) || value.fract() != 0.
    }) {
        return Err("Use an integer preview DPI of 72–1200.".into());
    }
    if let Some(dpi) = dpi {
        let preview = CompositeRecipe {
            dpi,
            ..recipe.clone()
        };
        preview.validate(chosen.len())?;
    }
    let grid = recipe.grid(chosen.len(), dpi.unwrap_or(recipe.dpi));

    let metrics = Metrics::of(&Plot::new().font_size(recipe.font_size as f32));
    let label_pt = metrics.label_pt();
    let mut captions = Vec::new();
    for (index, prepared) in chosen.iter().enumerate() {
        let data = &prepared.data;
        let options = recipe.panel_options(prepared.options.clone(), &grid);
        let mut caption = format!("{} {}", panel_label(index), data.caption(&options));
        if let Some(offsets) = &prepared.offset_caption {
            caption.push(' ');
            caption.push_str(offsets);
        }
        if let Some(inset) = &prepared.inset {
            caption.push(' ');
            caption.push_str(&inset.caption);
        }
        captions.push(caption);
    }
    let chosen_refs: Vec<_> = chosen.iter().collect();
    let (built, frame) = composition_layout(recipe, &chosen_refs, &grid)?;

    let (width, height) = grid.size_px();
    let (cell_w, cell_h) = grid.cell_px;
    let mut pixels = vec![255u8; width as usize * height as usize * 4];
    let mut svgs = Vec::new();
    let cell_height_in = cell_h as f64 / grid.dpi;
    for (index, panel) in built.into_iter().enumerate() {
        let (mut image, mut svg) = match panel {
            Built::Single(plot, labels) => {
                let plot = layout::panel_frame(*plot, cell_height_in, frame);
                let mut image = plot.render().map_err(|e| e.to_string())?;
                let mut svg = plot.render_to_svg().map_err(|e| e.to_string())?;
                render_end_labels(
                    &mut image,
                    &mut svg,
                    &labels,
                    frame,
                    &grid,
                    recipe.font_size,
                )?;
                (image, svg)
            }
            Built::FitResidual(panels) => panels.render(frame)?,
        };
        if let Some(inset) = &chosen[index].inset {
            inset::render(
                &mut image,
                &mut svg,
                inset,
                frame,
                &grid,
                recipe.font_size,
                recipe.line_width,
            )?;
        }
        let x = (index % grid.columns) as u32 * cell_w;
        let y = (index / grid.columns) as u32 * cell_h;
        blit(&mut pixels, width, &image, (x, y), (cell_w, cell_h));
        svgs.push((svg, format!("panel-{}", index + 1), x, y));
    }

    // Panel labels: bold, at the top-left corner of each panel.
    let scale = grid.dpi / 72.;
    let size_px = label_pt * scale;
    let mut labels = String::new();
    for (index, (_, _, x, y)) in svgs.iter().enumerate() {
        let (lx, ly) = (
            *x as f64 + LABEL_INSET_PT * scale,
            *y as f64 + LABEL_INSET_PT * scale,
        );
        let text = panel_label(index);
        let raster = ruviz::render::typst_text::render_raster(
            &format!("#text(weight: \"bold\", \"{text}\")"),
            size_px as f32,
            ruviz::prelude::Color::BLACK,
            0.,
            "panel label",
        )
        .map_err(|e| e.to_string())?;
        let pixmap = &raster.pixmap;
        blend(
            &mut pixels,
            (width, height),
            (pixmap.data(), pixmap.width(), pixmap.height()),
            (lx, ly),
        );
        // Live SVG text; the baseline sits about 0.8 em below the label top.
        labels.push_str(&format!(
            r##"<text x="{lx:.1}" y="{:.1}" font-family="Arial, Helvetica, sans-serif" font-size="{size_px:.1}" font-weight="bold" fill="#000000">{text}</text>"##,
            ly + 0.8 * size_px
        ));
    }
    let parts: Vec<(&str, &str, u32, u32)> = svgs
        .iter()
        .map(|(svg, prefix, x, y)| (svg.as_str(), prefix.as_str(), *x, *y))
        .collect();
    if !sample_names.is_empty() {
        captions.push(format!(
            "Sample labels: {}.",
            sample_names
                .iter()
                .map(|(short, full)| format!("{short} ({full})"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    let caption = captions.join(" ");
    let mut svg = super::physical_svg_size(
        layout::nest_svgs(&parts, &labels, (width, height)),
        (width, height),
        grid.dpi,
    );
    if let Some(start) = svg.find("<svg")
        && let Some(end) = svg[start..].find('>')
    {
        svg.insert_str(
            start + end + 1,
            &format!("<desc>{}</desc>", super::super::report::html(&caption)),
        );
    }
    Ok(ComposedFigure {
        image: Image::new(width, height, pixels),
        svg,
        grid,
        missing: missing.clone(),
        caption,
    })
}

/// Resolve the native panels, shared margins and complete direct labels before
/// rendering. The Python exporter reuses this layout at the recipe's DPI.
fn composition_layout(
    recipe: &CompositeRecipe,
    chosen: &[&PreparedPanel],
    grid: &Grid,
) -> Result<(Vec<Built>, Frame), String> {
    // Panel labels use the axis-title size and need room above every panel.
    let metrics = Metrics::of(&Plot::new().font_size(recipe.font_size as f32));
    let label_pt = metrics.label_pt();
    let mut frame: Frame = [
        0.,
        0.,
        (LABEL_INSET_PT + 1.2 * label_pt + 0.6 * metrics.tick_pt() + 2.) / 72.,
        0.,
    ];
    let mut built = Vec::new();
    for prepared in chosen {
        let spec = recipe
            .panels
            .get(prepared.index)
            .ok_or("Prepared panel does not belong to this recipe.")?;
        let data = &prepared.data;
        let mut options = recipe.panel_options(prepared.options.clone(), grid);
        options.validate()?;
        let (panel, needs) = match &data.residual {
            Some(residual) => {
                // Prevent the ordinary figure's Auto fallback from reserving an
                // outside legend before applying composed-figure styling.
                options.legend_position = LegendPlacement::UpperRight;
                let mut panels = layout::fit_residual_panels(data, residual, &options)?;
                panels.top = composed_legend(
                    journal_axes(panels.top),
                    options.legend,
                    recipe.font_size,
                    true,
                );
                panels.strip = journal_axes(panels.strip);
                let needs = panels.frame;
                (Built::FitResidual(Box::new(panels)), needs)
            }
            None => {
                let end_labels =
                    if !spec.uses_current_fit() && spec.label_mode == CurveLabelMode::CurveEnd {
                        options.legend = false;
                        curve_end_labels(data, &mut options, recipe.font_size)?
                    } else {
                        Vec::new()
                    };
                let inset = prepared.inset.as_ref().map(|inset| inset.placement);
                // Resolve the main legend before the inset covers its canvas.
                // Other composed panels use ruviz's automatic inside position.
                options.legend_position = if inset.is_some() {
                    LegendPlacement::Auto
                } else {
                    LegendPlacement::UpperRight
                };
                resolve_inside_legend(data, &mut options, inset);
                let plot = composed_legend(
                    journal_axes(data.panel(&options, false)?),
                    options.legend,
                    recipe.font_size,
                    inset.is_none(),
                );
                let metrics = Metrics::of(&plot);
                let ylim = options
                    .ymin
                    .zip(options.ymax)
                    .or_else(|| data.visible_ylim(&options));
                let needs = [
                    metrics.left(layout::tick_chars(ylim, 10).unwrap_or(5)),
                    metrics.right(layout::tick_chars(data.xlim(&options), 10).unwrap_or(4)),
                    0.,
                    metrics.bottom(),
                ];
                (Built::Single(Box::new(plot), end_labels), needs)
            }
        };
        for (shared, need) in frame.iter_mut().zip(needs) {
            *shared = shared.max(need);
        }
        built.push(panel);
    }

    if built
        .iter()
        .any(|panel| matches!(panel, Built::Single(_, labels) if !labels.is_empty()))
    {
        let panel_width = grid.cell_px.0 as f64 / grid.dpi;
        let available_margin = panel_width - frame[0] - 0.5;
        if frame[1] > available_margin {
            return Err(CURVE_END_WIDTH_ERROR.into());
        }
        // rexafs reserves at most 35% of a panel for names, including padding.
        // Narrow panels use less so at least half an inch remains for data.
        let label_width = (panel_width * 0.35).min(available_margin) - END_LABEL_PADDING_IN;
        for panel in &mut built {
            if let Built::Single(_, labels) = panel {
                for label in labels {
                    validate_end_label(label, label_width)?;
                    frame[1] = frame[1].max(label.width_in + END_LABEL_PADDING_IN);
                }
            }
        }
    }

    let available_height = grid.cell_px.1 as f64 - (frame[2] + frame[3]) * grid.dpi;
    for panel in &built {
        if let Built::Single(_, labels) = panel {
            validate_end_label_height(labels, available_height, grid.dpi)?;
        }
    }
    Ok((built, frame))
}

/// Validate the same panel geometry and complete curve-end labels as native
/// rendering before the Python exporter creates any files.
pub(crate) fn validate_composition_layout(
    recipe: &CompositeRecipe,
    chosen: &[&PreparedPanel],
    grid: &Grid,
) -> Result<(), String> {
    composition_layout(recipe, chosen, grid).map(|_| ())
}

/// Gap between a panel's top-left corner and its label, in points.
const LABEL_INSET_PT: f64 = 2.;
/// Four points between the axes and names, and four points at the panel edge.
const END_LABEL_PADDING_IN: f64 = 8. / 72.;
const CURVE_END_WIDTH_ERROR: &str = "Curve-end labels leave less than 0.5 in of plot width. Widen the figure, use fewer columns, reduce the font size or choose a legend.";

/// One literal label, measured at the output DPI before margins are fixed.
struct EndLabel {
    text: String,
    color: ruviz::prelude::Color,
    raster: ruviz::render::typst_text::TypstRasterOutput,
    width_in: f64,
    /// Position of the last visible point from the top of the visible y range.
    y_fraction: f64,
}

const CURVE_END_LABEL_TOO_LONG: &str =
    "Curve-end labels are too long for this panel. Shorten the figure labels or use a legend.";

/// Require complete labels in both raster and vector output. Geometry checks
/// separately retain at least half an inch for the plotted data.
fn validate_end_label(label: &EndLabel, max_width_in: f64) -> Result<(), String> {
    if label.width_in > max_width_in {
        Err(CURVE_END_LABEL_TOO_LONG.into())
    } else {
        Ok(())
    }
}

fn curve_end_labels(
    data: &FigureData,
    options: &mut FigureOptions,
    font_pt: f64,
) -> Result<Vec<EndLabel>, String> {
    let visible: Vec<_> = data
        .series(options)
        .iter()
        .enumerate()
        .filter(|(_, curve)| curve.visible(options) && !curve.x.is_empty())
        .collect();
    let xlim = data.xlim(options).or_else(|| {
        let mut bounds: Option<(f64, f64)> = None;
        for value in visible
            .iter()
            .flat_map(|(_, curve)| curve.x.iter().copied())
            .filter(|value| value.is_finite())
        {
            bounds = Some(bounds.map_or((value, value), |(min, max)| {
                (min.min(value), max.max(value))
            }));
        }
        bounds.map(|(min, max)| {
            let pad = if min < max {
                (max - min) * 0.05
            } else {
                min.abs().max(1.) * 0.05
            };
            (min - pad, max + pad)
        })
    });
    let Some((xmin, xmax)) = xlim else {
        return Ok(Vec::new());
    };
    // Fix automatically inferred limits before adding labels. Their physical
    // placement must use the same bounds as the plot, including explicit cuts.
    options.xmin = Some(xmin);
    options.xmax = Some(xmax);
    let Some((ymin, ymax)) = options
        .ymin
        .zip(options.ymax)
        .or_else(|| data.visible_ylim(options))
    else {
        return Ok(Vec::new());
    };
    options.ymin = Some(ymin);
    options.ymax = Some(ymax);
    let dpi = options.dimensions().2;
    let palette = Plot::new().get_theme();
    let mut labels = Vec::new();
    for (index, (_, curve)) in visible.iter().enumerate() {
        let mut endpoint: Option<(f64, f64)> = None;
        let mut include = |x: f64, y: f64| {
            if x.is_finite()
                && y.is_finite()
                && x >= xmin
                && x <= xmax
                && y >= ymin
                && y <= ymax
                && endpoint.is_none_or(|(previous, _)| x >= previous)
            {
                endpoint = Some((x, y));
            }
        };
        for (&x, &y) in curve.x.iter().zip(&curve.y) {
            include(x, y);
        }
        // Interpolate at visible boundaries; the final stored point may be far
        // outside a user's chosen horizontal or vertical limits.
        for (xs, ys) in curve.x.windows(2).zip(curve.y.windows(2)) {
            for x in [xmin, xmax] {
                if (xs[0] < x && x < xs[1]) || (xs[1] < x && x < xs[0]) {
                    include(x, ys[0] + (ys[1] - ys[0]) * (x - xs[0]) / (xs[1] - xs[0]));
                }
            }
            for y in [ymin, ymax] {
                if (ys[0] < y && y < ys[1]) || (ys[1] < y && y < ys[0]) {
                    include(xs[0] + (xs[1] - xs[0]) * (y - ys[0]) / (ys[1] - ys[0]), y);
                }
            }
        }
        let Some((_, y)) = endpoint else {
            continue;
        };
        let color = match options.series_colors.get(&curve.key) {
            Some(color) => super::series_color(color)?,
            None => palette.get_color(index),
        };
        let raster = ruviz::render::typst_text::render_raster(
            &ruviz::render::typst_text::literal_text_snippet(&curve.label),
            (font_pt * dpi / 72.) as f32,
            color,
            0.,
            "curve-end label",
        )
        .map_err(|error| error.to_string())?;
        labels.push(EndLabel {
            text: curve.label.clone(),
            color,
            width_in: raster.pixmap.width() as f64 / dpi,
            raster,
            y_fraction: (ymax - y) / (ymax - ymin),
        });
    }
    Ok(labels)
}

/// Reject overlapping complete labels before either export is written.
fn validate_end_label_height(labels: &[EndLabel], available: f64, dpi: f64) -> Result<(), String> {
    if labels.is_empty() {
        return Ok(());
    }
    let gap = 2. * dpi / 72.;
    let required = labels
        .iter()
        .map(|label| label.raster.pixmap.height() as f64)
        .sum::<f64>()
        + gap * labels.len().saturating_sub(1) as f64;
    if required > available {
        return Err("Curve-end labels do not fit vertically. Use taller panels, fewer samples, a smaller font or a legend.".into());
    }
    Ok(())
}

/// Keep direct labels within the reserved margin, separating vertically
/// adjacent names by two points without changing curve or axis coordinates.
fn render_end_labels(
    image: &mut Image,
    svg: &mut String,
    labels: &[EndLabel],
    frame: Frame,
    grid: &Grid,
    font_pt: f64,
) -> Result<(), String> {
    if labels.is_empty() {
        return Ok(());
    }
    let scale = grid.dpi / 72.;
    let top = frame[2] * grid.dpi;
    let bottom = image.height as f64 - frame[3] * grid.dpi;
    let available = bottom - top;
    let gap = 2. * scale;
    validate_end_label_height(labels, available, grid.dpi)?;
    let mut order: Vec<_> = (0..labels.len()).collect();
    order.sort_by(|&a, &b| labels[a].y_fraction.total_cmp(&labels[b].y_fraction));
    let mut positions = Vec::new();
    for (position, &index) in order.iter().enumerate() {
        let height = labels[index].raster.pixmap.height() as f64;
        let desired =
            (top + available * labels[index].y_fraction - height / 2.).clamp(top, bottom - height);
        let previous = if position == 0 {
            top
        } else {
            positions[position - 1]
                + labels[order[position - 1]].raster.pixmap.height() as f64
                + gap
        };
        positions.push(desired.max(previous));
    }
    for position in (0..positions.len()).rev() {
        let height = labels[order[position]].raster.pixmap.height() as f64;
        let limit = if position + 1 == positions.len() {
            bottom - height
        } else {
            positions[position + 1] - gap - height
        };
        positions[position] = positions[position].min(limit);
    }
    let x = image.width as f64 - frame[1] * grid.dpi + 4. * scale;
    let mut markup = String::new();
    for (&index, &y) in order.iter().zip(&positions) {
        let label = &labels[index];
        let pixmap = &label.raster.pixmap;
        blend(
            &mut image.pixels,
            (image.width, image.height),
            (pixmap.data(), pixmap.width(), pixmap.height()),
            (x, y),
        );
        let color = format!(
            "#{:02x}{:02x}{:02x}",
            label.color.r, label.color.g, label.color.b
        );
        let font_px = font_pt * scale;
        let line_height = pixmap.height() as f64 / label.text.split('\n').count() as f64;
        markup.push_str(&format!(r#"<text class="curve-end-label" x="{x:.3}" y="{:.3}" xml:space="preserve" font-family="Arial, Helvetica, sans-serif" font-size="{font_px:.3}" fill="{color}">"#, y + 0.8 * line_height));
        for (line, text) in label.text.split('\n').enumerate() {
            markup.push_str(&format!(
                r#"<tspan x="{x:.3}" dy="{}">{}</tspan>"#,
                if line == 0 { 0. } else { line_height },
                super::super::report::html(text)
            ));
        }
        markup.push_str("</text>");
    }
    let end = svg
        .rfind("</svg>")
        .ok_or("The panel SVG has no closing element.")?;
    svg.insert_str(end, &markup);
    Ok(())
}

/// Resolve an Auto or Outside legend to an inside corner. An outside legend
/// would shrink one panel's plot area and break the alignment with its
/// neighbours, so the composer keeps legends inside (as the fit + residual
/// figure does).
pub(crate) fn resolve_inside_legend(
    data: &FigureData,
    options: &mut FigureOptions,
    inset: Option<InsetPlacement>,
) {
    if !options.legend
        || !matches!(
            options.legend_position,
            LegendPlacement::Auto | LegendPlacement::Outside
        )
    {
        return;
    }
    let (width, height, _) = options.dimensions();
    let probe = Plot::new().font_size(options.font_size.unwrap_or(10.) as f32);
    let typography = &probe.get_config().typography;
    let visible: Vec<_> = data
        .series(options)
        .iter()
        .filter(|s| s.visible(options) && !s.x.is_empty())
        .collect();
    let ylim = options
        .ymin
        .zip(options.ymax)
        .or_else(|| data.visible_ylim(options));
    options.legend_position = layout::auto_legend(
        &visible,
        data.xlim(options),
        ylim,
        &LegendFrame {
            size_in: (width, height),
            legend_pt: typography.legend_size() as f64,
            tick_pt: typography.tick_size() as f64,
            title: false,
            outside_ok: false,
        },
    );
    // An automatic legend must not disappear beneath the inset's opaque
    // canvas. Move a conflicting corner across the plot; explicit user
    // placements remain under the single-figure editor's control.
    options.legend_position = match (inset, options.legend_position) {
        (Some(InsetPlacement::LowerRight), LegendPlacement::LowerRight) => {
            LegendPlacement::UpperLeft
        }
        (Some(InsetPlacement::UpperLeft), LegendPlacement::UpperLeft) => {
            LegendPlacement::LowerRight
        }
        (
            Some(InsetPlacement::CenterRight),
            LegendPlacement::UpperRight | LegendPlacement::LowerRight,
        ) => LegendPlacement::UpperLeft,
        (_, position) => position,
    };
}

/// Journal axes apply only to composed panels, including residual strips and insets.
/// Four minor ticks per major interval are a fixed rexafs choice; unlike
/// Matplotlib's AutoMinorLocator, this does not adapt to each major step:
/// <https://matplotlib.org/stable/api/ticker_api.html#matplotlib.ticker.AutoMinorLocator>.
fn journal_axes(plot: Plot) -> Plot {
    plot.tick_direction_inside()
        .ticks_all_sides()
        .minor_ticks(4)
}

/// The single extension point for composed legend appearance and placement.
fn composed_legend(plot: Plot, visible: bool, font_pt: f64, best_position: bool) -> Plot {
    if !visible {
        return plot;
    }
    // Future ruviz style hook: legend_style(LegendStyle::invisible()) and
    // legend_spacing(LegendSpacing { .. }) belong here once released.
    let plot = plot.legend_font_size((font_pt - 1.).max(5.) as f32);
    // An inset panel already has a corner chosen to avoid its opaque canvas.
    if best_position {
        plot.legend_best()
    } else {
        plot
    }
}

/// Copy `image` into the RGBA canvas at `origin`, cropped to one cell.
fn blit(canvas: &mut [u8], canvas_width: u32, image: &Image, origin: (u32, u32), cell: (u32, u32)) {
    let columns = image.width.min(cell.0) as usize * 4;
    for row in 0..image.height.min(cell.1) as usize {
        let src = row * image.width as usize * 4;
        let dst = ((origin.1 as usize + row) * canvas_width as usize + origin.0 as usize) * 4;
        canvas[dst..dst + columns].copy_from_slice(&image.pixels[src..src + columns]);
    }
}

/// Draw a premultiplied-alpha pixmap over the opaque RGBA canvas ("source
/// over" compositing: result = source + destination × (1 − source alpha)).
/// `pixmap` is `(premultiplied RGBA bytes, width, height)`.
fn blend(
    canvas: &mut [u8],
    (width, height): (u32, u32),
    (data, pw, ph): (&[u8], u32, u32),
    at: (f64, f64),
) {
    let (x0, y0) = (at.0.round() as i64, at.1.round() as i64);
    for py in 0..ph as i64 {
        for px in 0..pw as i64 {
            let (x, y) = (x0 + px, y0 + py);
            if x < 0 || y < 0 || x >= width as i64 || y >= height as i64 {
                continue;
            }
            let src = ((py * pw as i64 + px) * 4) as usize;
            let dst = ((y * width as i64 + x) * 4) as usize;
            let alpha = data[src + 3] as u32;
            for channel in 0..3 {
                let under = canvas[dst + channel] as u32;
                canvas[dst + channel] =
                    (data[src + channel] as u32 + under * (255 - alpha) / 255) as u8;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::publication::figures::{FigureSeries, fit_figures, spectrum_figures};
    use std::{path::Path, sync::Arc};

    pub(super) fn copper_figures() -> Vec<FigureData> {
        let file = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../rexafs/tests/testfiles/xraylarch_d867/xafsdata/cu_150k.xmu");
        let sp = Arc::new(
            crate::params::process_file(&file, &crate::params::PipelineParams::default()).unwrap(),
        );
        spectrum_figures(sp, "Cu foil, 150 K")
    }

    fn synthetic_fit() -> Vec<FigureData> {
        let k: Vec<f64> = (0..280).map(|i| i as f64 * 0.05).collect();
        let data: Vec<f64> = k
            .iter()
            .map(|k| (2. * 2.55 * k).sin() * (-0.01 * k * k).exp() * 0.8 / k.max(0.5))
            .collect();
        let model: Vec<f64> = data.iter().map(|y| 0.92 * y).collect();
        let r: Vec<f64> = (0..120).map(|i| i as f64 * 0.05).collect();
        let peak: Vec<f64> = r
            .iter()
            .map(|r| (-(r - 2.2_f64).powi(2) * 8.).exp())
            .collect();
        fit_figures(&rexafs::prelude::FeffFitResult {
            kweight: 2.,
            kmax: Some(13.),
            k: nalgebra::DVector::from_vec(k),
            data_chi: nalgebra::DVector::from_vec(data),
            model_chi: nalgebra::DVector::from_vec(model),
            r: nalgebra::DVector::from_vec(r.clone()),
            data_chir_re: nalgebra::DVector::from_vec(peak.clone()),
            data_chir_im: nalgebra::DVector::from_vec(vec![0.; r.len()]),
            model_chir_mag: nalgebra::DVector::from_vec(peak.iter().map(|y| 0.85 * y).collect()),
            ..Default::default()
        })
    }

    pub(super) fn sample(id: u64, label: &str, color: &str) -> CompositeSample {
        let main = FigureSeries {
            key: "series-0".into(),
            label: "Original label".into(),
            x: vec![0., 1., 2.],
            y: vec![0., 1., 2.],
            dashed: false,
            optional: false,
        };
        CompositeSample {
            id: GroupId::legacy_result(id),
            is_current: false,
            label: label.into(),
            color: color.into(),
            figures: vec![Arc::new(FigureData {
                key: "xanes",
                e0: Some(1.),
                label: "Energy".into(),
                xlabel: "Energy (eV)".into(),
                ylabel: "Flattened μ(E) (dimensionless)".into(),
                math_ylabel: "Flattened μ(E)".into(),
                series: vec![
                    main.clone(),
                    FigureSeries {
                        key: "helper".into(),
                        label: "Helper".into(),
                        y: vec![2., 2., 2.],
                        ..main.clone()
                    },
                ],
                normalized_series: Some(vec![FigureSeries {
                    y: vec![1., 2., 3.],
                    ..main
                }]),
                guides: vec![],
                default_xlim: Some((0., 2.)),
                residual: None,
            })],
        }
    }

    #[test]
    fn inset_defaults_resolve_from_first_sample_e0_and_preserve_old_recipes() {
        let samples = [sample(1, "A", "#0072b2"), sample(2, "B", "#e69f00")];
        let current: Vec<FigureData> = vec![];
        for (kind, expected) in [
            (InsetKind::PreEdge, (-5., 12.)),
            (InsetKind::Edge, (-1., 24.)),
            (InsetKind::Difference, (-5., 32.)),
        ] {
            let recipe = CompositeRecipe {
                samples: samples.iter().map(|sample| sample.id.clone()).collect(),
                panels: vec![CompositePanel {
                    inset: Some(PanelInset::new(kind)),
                    reverse: true,
                    .."xanes".into()
                }],
                ..Default::default()
            };
            let prepared =
                prepare_panels(&recipe, &current, &samples, &FigureSettings::default()).unwrap();
            let inset = prepared.panels[0].inset.as_ref().unwrap();
            assert_eq!(inset.data.xlim(&inset.options), Some(expected));
            assert_eq!(inset.placement, InsetPlacement::LowerRight);
            assert!(recipe.panels[0].inset.as_ref().unwrap().follow_offset);
        }
        let old: CompositeRecipe =
            serde_json::from_str(r#"{"panels":["xanes",{"key":"mu-energy","offset_step":0.3}]}"#)
                .unwrap();
        assert!(old.panels.iter().all(|panel| panel.inset.is_none()));
        assert!(!serde_json::to_string(&old).unwrap().contains("inset"));
    }

    #[test]
    fn inset_offsets_scale_unshifted_spans_and_follow_reverse_and_toggle() {
        let samples = [sample(1, "A", "#0072b2"), sample(2, "B", "#e69f00")];
        let original = samples[0].figures[0]
            .csv(&FigureOptions::default())
            .unwrap();
        let current: Vec<FigureData> = vec![];
        let mut recipe = CompositeRecipe {
            samples: samples.iter().map(|sample| sample.id.clone()).collect(),
            panels: vec![CompositePanel {
                offset_step: 0.4,
                inset: Some(PanelInset {
                    energy_range: (-1., 0.),
                    ..PanelInset::new(InsetKind::PreEdge)
                }),
                .."xanes".into()
            }],
            ..Default::default()
        };
        for (reverse, follow, expected) in [
            (false, true, [0., 0.2]),
            (true, true, [0.2, 0.]),
            (true, false, [0., 0.]),
        ] {
            recipe.panels[0].reverse = reverse;
            recipe.panels[0].inset.as_mut().unwrap().follow_offset = follow;
            let prepared =
                prepare_panels(&recipe, &current, &samples, &FigureSettings::default()).unwrap();
            let inset = prepared.panels[0].inset.as_ref().unwrap();
            for (curve, offset) in inset.data.series.iter().zip(expected) {
                assert_eq!(curve.y, [offset, 1. + offset, 2. + offset]);
            }
            assert_eq!(
                inset.options.series_colors,
                prepared.panels[0].options.series_colors
            );
        }
        assert_eq!(
            samples[0].figures[0]
                .csv(&FigureOptions::default())
                .unwrap(),
            original
        );
        recipe.panels[0].inset.as_mut().unwrap().follow_offset = true;
        recipe.panels[0].inset.as_mut().unwrap().kind = InsetKind::Difference;
        let prepared =
            prepare_panels(&recipe, &current, &samples, &FigureSettings::default()).unwrap();
        // Identical normalized spectra have zero difference span and no shift.
        assert!(
            prepared.panels[0]
                .inset
                .as_ref()
                .unwrap()
                .data
                .series
                .iter()
                .all(|curve| curve.y == [0., 0., 0.])
        );
    }

    #[test]
    fn difference_uses_normalized_first_sample_on_overlapping_grids_even_for_raw_panels() {
        let mut reference = sample(1, "Reference", "#0072b2");
        let mut other = sample(2, "Other", "#e69f00");
        Arc::make_mut(&mut reference.figures[0])
            .normalized_series
            .as_mut()
            .unwrap()[0]
            .y = vec![0., 2., 4.];
        let data = Arc::make_mut(&mut other.figures[0]);
        data.e0 = Some(100.);
        data.normalized_series.as_mut().unwrap()[0].x = vec![0.5, 1.5, 2.5];
        data.normalized_series.as_mut().unwrap()[0].y = vec![2., 6., 9.];
        for sample in [&mut reference, &mut other] {
            let mut raw = sample.figures[0].as_ref().clone();
            raw.key = "mu-energy";
            raw.series[0].y = vec![100., 200., 300.];
            raw.series[0].dashed = true;
            raw.normalized_series = None;
            sample.figures.push(Arc::new(raw));
        }
        let samples = [reference, other];
        let current: Vec<FigureData> = vec![];
        for key in ["xanes", "mu-energy"] {
            let recipe = CompositeRecipe {
                samples: samples.iter().map(|sample| sample.id.clone()).collect(),
                panels: vec![CompositePanel {
                    reverse: true,
                    offset_step: 10.,
                    inset: Some(PanelInset {
                        follow_offset: false,
                        ..PanelInset::new(InsetKind::Difference)
                    }),
                    ..key.into()
                }],
                ..Default::default()
            };
            let prepared =
                prepare_panels(&recipe, &current, &samples, &FigureSettings::default()).unwrap();
            let inset = prepared.panels[0].inset.as_ref().unwrap();
            assert_eq!(inset.data.xlim(&inset.options), Some((-5., 32.)));
            assert_eq!(inset.quantity, "Δμ");
            assert_eq!(inset.data.series[0].y, [0., 0., 0.]);
            assert_eq!(inset.data.series[1].x, [0.5, 1.5]);
            assert_eq!(inset.data.series[1].y, [1., 3.]);
            assert_eq!(inset.data.series[0].dashed, key == "mu-energy");
        }
    }

    #[test]
    fn inset_validation_requires_two_difference_samples_valid_limits_and_original_reference() {
        let samples = [sample(1, "A", "#0072b2"), sample(2, "B", "#e69f00")];
        let current: Vec<FigureData> = vec![];
        let mut recipe = CompositeRecipe {
            samples: vec![samples[0].id.clone()],
            panels: vec![CompositePanel {
                inset: Some(PanelInset::new(InsetKind::Difference)),
                .."xanes".into()
            }],
            ..Default::default()
        };
        let failure = |recipe: &CompositeRecipe| {
            prepare_panels(recipe, &current, &samples, &FigureSettings::default())
                .err()
                .unwrap()
        };
        assert!(failure(&recipe).contains("at least two samples"));
        recipe.samples.insert(0, GroupId::legacy_result(99));
        assert!(failure(&recipe).contains("first selected sample"));
        recipe.samples = samples.iter().map(|sample| sample.id.clone()).collect();
        recipe.panels[0].inset.as_mut().unwrap().energy_range = (2., 1.);
        assert!(failure(&recipe).contains("minimum < maximum"));
        recipe.panels[0].inset.as_mut().unwrap().energy_range = (100., 110.);
        assert!(failure(&recipe).contains("overlapping energy data"));
    }

    #[test]
    fn inset_exports_keep_canvas_size_and_nested_live_svg_text() {
        let figures = copper_figures();
        let samples: Vec<_> = [1, 2]
            .into_iter()
            .map(|id| CompositeSample {
                id: GroupId::legacy_result(id),
                is_current: false,
                label: format!("Sample {id}"),
                color: if id == 1 { "#0072b2" } else { "#e69f00" }.into(),
                figures: figures.iter().cloned().map(Arc::new).collect(),
            })
            .collect();
        let mut recipe = CompositeRecipe {
            samples: samples.iter().map(|sample| sample.id.clone()).collect(),
            panels: vec!["xanes".into(), "chi-k".into()],
            dpi: 100.,
            ..Default::default()
        };
        let settings = FigureSettings::default();
        let plain = compose_samples(&recipe, &figures, &samples, &settings, None).unwrap();
        for (kind, placement) in InsetKind::ALL.into_iter().zip(InsetPlacement::ALL) {
            recipe.panels[0].inset = Some(PanelInset::new(kind));
            recipe.panels[0].inset.as_mut().unwrap().placement = placement;
            let prepared = prepare_panels(&recipe, &figures, &samples, &settings).unwrap();
            let mut options = recipe.panel_options(prepared.panels[0].options.clone(), &plain.grid);
            resolve_inside_legend(&prepared.panels[0].data, &mut options, Some(placement));
            assert!(!matches!(
                (placement, options.legend_position),
                (InsetPlacement::LowerRight, LegendPlacement::LowerRight)
                    | (InsetPlacement::UpperLeft, LegendPlacement::UpperLeft)
                    | (
                        InsetPlacement::CenterRight,
                        LegendPlacement::UpperRight | LegendPlacement::LowerRight
                    )
            ));
            let rendered = compose_samples(&recipe, &figures, &samples, &settings, None).unwrap();
            assert_eq!(
                (rendered.image.width, rendered.image.height),
                (plain.image.width, plain.image.height)
            );
            assert_ne!(rendered.image.pixels, plain.image.pixels);
            assert!(rendered.svg.contains("class=\"composer-inset\""));
            assert!(rendered.svg.contains("class=\"inset-x-tick-label\""));
            let background = rendered
                .svg
                .find("<rect class=\"inset-quantity-background\"")
                .unwrap();
            let label = rendered
                .svg
                .find("<text class=\"inset-quantity-label\"")
                .unwrap();
            assert!(background < label);
            assert!(rendered.svg[background..label].contains("fill=\"#ffffff\""));
            assert!(rendered.svg.contains("class=\"inset-quantity-label\""));
            assert!(!rendered.svg.contains("<image"));
            assert!(rendered.caption.contains("Inset: "));
            assert!(rendered.png().unwrap().starts_with(b"\x89PNG"));
            assert!(!rendered.tiff().unwrap().is_empty());
            assert!(rendered.pdf().unwrap().starts_with(b"%PDF-"));
            let preview =
                compose_samples(&recipe, &figures, &samples, &settings, Some(72.)).unwrap();
            assert_eq!(preview.grid.size_px(), recipe.grid(2, 72.).size_px());
            if let Some(dir) = std::env::var_os("REXAFS_COMPOSER_SHOTS") {
                let dir = Path::new(&dir);
                std::fs::create_dir_all(dir).unwrap();
                for extension in ["png", "svg", "pdf", "tiff"] {
                    std::fs::write(
                        dir.join(format!("inset-{}.{extension}", kind.label().to_lowercase())),
                        rendered.bytes(extension).unwrap(),
                    )
                    .unwrap();
                }
            }
        }
        // The low-DPI cases above exercise the supersampled inset path; the
        // largest supported DPI renders it directly at the requested size.
        let high = compose_samples(&recipe, &figures, &samples, &settings, Some(1200.)).unwrap();
        assert_eq!(high.grid.size_px(), recipe.grid(2, 1200.).size_px());
        assert!(
            high.image
                .pixels
                .chunks_exact(4)
                .all(|pixel| pixel[3] == 255)
        );
    }

    #[test]
    fn panel_objects_migrate_strings_and_preserve_ordered_sample_ids() {
        let old: CompositeRecipe =
            serde_json::from_str(r#"{"panels":["xanes",{"key":"chi-k"}]}"#).unwrap();
        assert_eq!(
            old.panels,
            [CompositePanel::from("xanes"), CompositePanel::from("chi-k")]
        );
        assert!(old.samples.is_empty());
        let recipe: CompositeRecipe = serde_json::from_str(r#"{
            "samples":["result:3","result:1"],
            "panels":[{"key":"xanes","samples":["result:2","result:3"],"offset_step":0.3,"reverse":true,"label_mode":"curve_end"},{"key":"chi-r","samples":[]}]
        }"#).unwrap();
        assert_eq!(
            recipe.samples,
            [GroupId::legacy_result(3), GroupId::legacy_result(1)]
        );
        assert_eq!(
            recipe.panels[0].samples.as_ref().unwrap(),
            &[GroupId::legacy_result(2), GroupId::legacy_result(3)]
        );
        assert_eq!(recipe.panels[1].samples, Some(Vec::new()));
        assert_eq!(recipe.panels[0].label_mode, CurveLabelMode::CurveEnd);
        let saved = serde_json::to_value(&recipe).unwrap();
        assert!(saved["panels"][0].is_object());
        assert_eq!(
            serde_json::from_value::<CompositeRecipe>(saved).unwrap(),
            recipe
        );
    }

    #[test]
    fn preparation_uses_saved_order_offsets_and_full_color_keys_without_mutating_sources() {
        let a = sample(1, "A", "#0072b2");
        let b = sample(2, "B", "#e69f00");
        let a_before = a.figures[0].csv(&FigureOptions::default()).unwrap();
        let samples = [a.clone(), b.clone()];
        let mut recipe = CompositeRecipe {
            samples: vec![b.id.clone(), a.id.clone()],
            panels: vec![CompositePanel {
                offset_step: 0.3,
                .."xanes".into()
            }],
            series_colors: BTreeMap::from([("result:2/series-0".into(), "#009e73".into())]),
            dpi: 72.,
            ..Default::default()
        };
        let mut settings = FigureSettings::default();
        settings.figures.insert(
            "xanes".into(),
            FigureOptions {
                series_colors: BTreeMap::from([
                    ("series-0".into(), "#cc79a7".into()),
                    ("result:1/series-0".into(), "#d55e00".into()),
                ]),
                ..Default::default()
            },
        );
        let empty: Vec<FigureData> = vec![];
        let prepared = prepare_panels(&recipe, &empty, &samples, &settings).unwrap();
        let panel = &prepared.panels[0];
        assert_eq!(
            panel
                .data
                .series
                .iter()
                .map(|curve| curve.key.as_str())
                .collect::<Vec<_>>(),
            ["result:2/series-0", "result:1/series-0"]
        );
        assert_eq!(
            panel
                .data
                .series
                .iter()
                .map(|curve| curve.label.as_str())
                .collect::<Vec<_>>(),
            ["B", "A"]
        );
        assert_eq!(panel.data.series[0].y, [0., 1., 2.]);
        assert_eq!(panel.data.series[1].y, [0.3, 1.3, 2.3]);
        assert_eq!(panel.options.series_colors["result:2/series-0"], "#009e73");
        assert_eq!(panel.options.series_colors["result:1/series-0"], "#d55e00");
        assert!(panel.offset_caption.as_ref().unwrap().contains("by 0.3"));
        recipe.panels[0].reverse = true;
        let reversed = prepare_panels(&recipe, &empty, &samples, &settings).unwrap();
        assert_eq!(reversed.panels[0].data.series[0].y, [0.3, 1.3, 2.3]);
        assert_eq!(reversed.panels[0].data.series[1].y, [0., 1., 2.]);
        assert_eq!(
            a.figures[0].csv(&FigureOptions::default()).unwrap(),
            a_before
        );
        let rendered = compose_samples(&recipe, &empty, &samples, &settings, None).unwrap();
        assert!(
            rendered
                .caption
                .contains("Spectra offset vertically by 0.3")
        );
        assert!(rendered.svg.contains(r#"stroke="rgb(0,158,115)""#));
        assert!(rendered.svg.contains(r#"stroke="rgb(213,94,0)""#));
    }

    #[test]
    fn single_sample_and_explicit_current_panels_keep_original_figure_behavior() {
        let current = sample(1, "Current", "#0072b2");
        let chosen = sample(2, "Different workspace label", "#e69f00");
        let recipe = CompositeRecipe {
            samples: vec![chosen.id.clone()],
            panels: vec![
                "xanes".into(),
                CompositePanel {
                    samples: Some(vec![]),
                    .."xanes".into()
                },
            ],
            dpi: 72.,
            ..Default::default()
        };
        let settings = FigureSettings::default();
        let prepared = prepare_panels(
            &recipe,
            &current.figures,
            std::slice::from_ref(&chosen),
            &settings,
        )
        .unwrap();
        assert_eq!(
            prepared.panels[0]
                .data
                .csv(&prepared.panels[0].options)
                .unwrap(),
            chosen.figures[0].csv(&FigureOptions::default()).unwrap()
        );
        assert_eq!(
            prepared.panels[1]
                .data
                .csv(&prepared.panels[1].options)
                .unwrap(),
            current.figures[0].csv(&FigureOptions::default()).unwrap()
        );
        assert!(prepared.panels[0].options.series_colors.is_empty());
        assert!(prepared.panels[0].offset_caption.is_none());
        let single = CompositeRecipe {
            panels: vec!["xanes".into()],
            ..recipe.clone()
        };
        let old = compose(
            &CompositeRecipe {
                samples: vec![],
                ..single.clone()
            },
            &chosen.figures,
            &settings,
            None,
        )
        .unwrap();
        let new = compose_samples(&single, &current.figures, &[chosen], &settings, None).unwrap();
        assert_eq!(old.svg, new.svg);
        assert_eq!(old.caption, new.caption);
        assert_eq!(old.image.pixels, new.image.pixels);
    }

    #[test]
    fn single_sample_unavailable_figure_is_skipped_and_reported() {
        for empty_series in [true, false] {
            let mut chosen = sample(1, "A", "#0072b2");
            let mut unavailable = chosen.figures[0].as_ref().clone();
            unavailable.key = "chi-r";
            unavailable.normalized_series = None;
            if empty_series {
                unavailable.series.clear();
            } else {
                for curve in &mut unavailable.series {
                    curve.x.clear();
                    curve.y.clear();
                }
            }
            chosen.figures.push(Arc::new(unavailable));
            let recipe = CompositeRecipe {
                samples: vec![chosen.id.clone()],
                panels: vec!["xanes".into(), "chi-r".into()],
                dpi: 72.,
                ..Default::default()
            };
            let settings = FigureSettings::default();
            let current: Vec<FigureData> = vec![];
            let prepared =
                prepare_panels(&recipe, &current, std::slice::from_ref(&chosen), &settings)
                    .unwrap();
            assert_eq!(prepared.panels.len(), 1);
            assert_eq!(prepared.panels[0].data.key, "xanes");
            assert_eq!(prepared.missing.len(), 1);
            assert!(prepared.missing[0].contains("chi-r: no processed data for A (result:1)"));
            assert!(prepared.missing[0].contains("select and process"));
            let rendered = compose_samples(&recipe, &current, &[chosen], &settings, None).unwrap();
            assert_eq!(rendered.missing, prepared.missing);
            assert_eq!(rendered.grid.columns, 1);
            assert_eq!(rendered.grid.rows, 1);
            assert!(rendered.caption.starts_with("(a)"));
            assert!(!rendered.caption.contains("(b)"));
        }
    }

    #[test]
    fn multisample_main_curves_match_processed_spectrum_data_including_normalization() {
        let original = copper_figures();
        let samples: Vec<_> = [1, 2]
            .into_iter()
            .map(|id| CompositeSample {
                id: GroupId::legacy_result(id),
                is_current: false,
                label: format!("Sample {id}"),
                color: "#0072b2".into(),
                figures: original.iter().cloned().map(Arc::new).collect(),
            })
            .collect();
        let recipe = CompositeRecipe {
            samples: samples.iter().map(|sample| sample.id.clone()).collect(),
            panels: ["mu-energy", "chi-r", "xanes"]
                .map(CompositePanel::from)
                .into(),
            ..Default::default()
        };
        let mut settings = FigureSettings::default();
        settings.figures.insert(
            "xanes".into(),
            FigureOptions {
                normalized: true,
                ..Default::default()
            },
        );
        let prepared = prepare_panels(&recipe, &original, &samples, &settings).unwrap();
        for panel in &prepared.panels {
            let source = original
                .iter()
                .find(|data| data.key == panel.data.key)
                .unwrap();
            let main = &source.series(&settings.options(source.key))[0];
            assert_eq!(panel.data.series.len(), 2);
            for curve in &panel.data.series {
                assert_eq!(curve.x, main.x);
                assert_eq!(curve.y, main.y);
            }
            assert_eq!(
                panel.data.ylabel(&panel.options),
                source.ylabel(&settings.options(source.key))
            );
        }
    }

    #[test]
    fn missing_sources_are_reported_and_fit_panels_ignore_sample_settings() {
        let a = sample(1, "A", "#0072b2");
        let missing = GroupId::legacy_result(99);
        let recipe = CompositeRecipe {
            samples: vec![missing.clone(), a.id.clone()],
            panels: vec![
                "chi-r".into(),
                CompositePanel {
                    offset_step: 0.3,
                    .."xanes".into()
                },
                CompositePanel {
                    offset_step: f64::NAN,
                    reverse: true,
                    label_mode: CurveLabelMode::CurveEnd,
                    .."fit-residual-k".into()
                },
            ],
            // The residual strip must meet ruviz's 100-pixel canvas minimum.
            dpi: 150.,
            ..Default::default()
        };
        let fits = synthetic_fit();
        let prepared = prepare_panels(
            &recipe,
            &fits,
            std::slice::from_ref(&a),
            &FigureSettings::default(),
        )
        .unwrap();
        assert_eq!(
            prepared
                .panels
                .iter()
                .map(|panel| panel.index)
                .collect::<Vec<_>>(),
            [1, 2]
        );
        assert!(
            prepared
                .missing
                .iter()
                .any(|message| message.contains("result:99"))
        );
        assert!(
            prepared
                .missing
                .iter()
                .any(|message| message.contains("no processed data")
                    && message.contains("select and process"))
        );
        assert_eq!(prepared.panels[0].data.series[0].y, [0.3, 1.3, 2.3]);
        let fit = fits
            .iter()
            .find(|data| data.key == "fit-residual-k")
            .unwrap();
        assert_eq!(
            prepared.panels[1]
                .data
                .csv(&prepared.panels[1].options)
                .unwrap(),
            fit.csv(&FigureOptions::default()).unwrap()
        );
        assert!(prepared.panels[1].offset_caption.is_none());
        compose_samples(&recipe, &fits, &[a], &FigureSettings::default(), None).unwrap();
        let unavailable = CompositeRecipe {
            panels: vec!["xanes".into()],
            samples: vec![missing],
            ..Default::default()
        };
        let error = compose_samples(&unavailable, &fits, &[], &FigureSettings::default(), None)
            .err()
            .unwrap();
        assert!(error.contains("at least one panel") && error.contains("result:99"));
    }

    #[test]
    fn curve_end_labels_escape_names_follow_visible_limits_and_reserve_space() {
        let a = sample(1, "A <foil> & \"edge\" #[$]", "#0072b2");
        let b = sample(2, "B", "#e69f00");
        let recipe = CompositeRecipe {
            samples: vec![a.id.clone(), b.id.clone()],
            panels: vec![CompositePanel {
                label_mode: CurveLabelMode::CurveEnd,
                .."xanes".into()
            }],
            dpi: 100.,
            ..Default::default()
        };
        let mut settings = FigureSettings::default();
        settings.figures.insert(
            "xanes".into(),
            FigureOptions {
                xmin: Some(0.),
                xmax: Some(1.5),
                ymin: Some(0.),
                ymax: Some(3.),
                ..Default::default()
            },
        );
        let empty: Vec<FigureData> = vec![];
        let prepared = prepare_panels(&recipe, &empty, &[a.clone(), b.clone()], &settings).unwrap();
        let mut options =
            recipe.panel_options(prepared.panels[0].options.clone(), &recipe.grid(1, 100.));
        let labels =
            curve_end_labels(&prepared.panels[0].data, &mut options, recipe.font_size).unwrap();
        assert_eq!(labels.len(), 2);
        assert!((labels[0].y_fraction - 0.5).abs() < 1e-12);
        options.ymax = Some(1.);
        let clipped =
            curve_end_labels(&prepared.panels[0].data, &mut options, recipe.font_size).unwrap();
        assert_eq!(clipped.len(), 2);
        assert_eq!(clipped[0].y_fraction, 0.);
        let rendered = compose_samples(&recipe, &empty, &[a.clone(), b], &settings, None).unwrap();
        assert_eq!(rendered.svg.matches("class=\"curve-end-label\"").count(), 2);
        assert!(
            rendered
                .svg
                .contains("A &lt;foil&gt; &amp; &quot;edge&quot; #[$]")
        );
        assert!(rendered.pdf().unwrap().starts_with(b"%PDF"));
        if let Some(dir) = std::env::var_os("REXAFS_COMPOSER_SHOTS") {
            let dir = Path::new(&dir);
            std::fs::create_dir_all(dir).unwrap();
            for extension in ["png", "svg"] {
                std::fs::write(
                    dir.join(format!("composed-samples-curve-end.{extension}")),
                    rendered.bytes(extension).unwrap(),
                )
                .unwrap();
            }
        }
    }

    #[test]
    fn long_curve_end_names_require_short_labels_without_truncation() {
        let samples: Vec<_> = (1..=6)
            .map(|id| {
                sample(
                    id,
                    &format!("Synthetic {id:02} · CuO 100.0% · Cu₂O 0.0% · Cu 0.0%"),
                    "#0072b2",
                )
            })
            .collect();
        let mut recipe = CompositeRecipe {
            samples: samples.iter().map(|sample| sample.id.clone()).collect(),
            panels: vec![
                CompositePanel {
                    label_mode: CurveLabelMode::CurveEnd,
                    offset_step: 1.5,
                    .."xanes".into()
                };
                2
            ],
            dpi: 200.,
            ..Default::default()
        };
        let settings = FigureSettings::default();
        let empty: Vec<FigureData> = vec![];
        for dpi in [100., recipe.dpi] {
            let error = compose_samples(&recipe, &empty, &samples, &settings, Some(dpi))
                .err()
                .unwrap();
            assert_eq!(error, CURVE_END_LABEL_TOO_LONG);
        }
        recipe.figure_labels = samples
            .iter()
            .enumerate()
            .map(|(i, sample)| (sample.id.clone(), format!("Frame {}", i + 1)))
            .collect();
        for dpi in [100., recipe.dpi] {
            let rendered =
                compose_samples(&recipe, &empty, &samples, &settings, Some(dpi)).unwrap();
            assert_eq!(
                rendered.svg.matches("class=\"curve-end-label\"").count(),
                12
            );
            for sample in &samples {
                let short = &recipe.figure_labels[&sample.id];
                assert!(rendered.svg.contains(&format!(">{short}</tspan>")));
                assert!(
                    rendered
                        .caption
                        .contains(&format!("{short} ({})", sample.label))
                );
                assert_eq!(rendered.caption.matches(&sample.label).count(), 1);
            }
        }
    }

    #[test]
    fn complete_end_labels_preserve_literal_text_and_report_the_allowance() {
        let data = sample(1, "A long sample name", "#0072b2");
        let mut options = FigureOptions::default();
        let labels = curve_end_labels(&data.figures[0], &mut options, 8.).unwrap();
        let text = labels[0].text.clone();
        assert_eq!(
            validate_end_label(&labels[0], 0.).unwrap_err(),
            CURVE_END_LABEL_TOO_LONG
        );
        assert_eq!(labels[0].text, text);
        assert!(validate_end_label(&labels[0], labels[0].width_in).is_ok());
    }

    #[test]
    fn grid_divides_the_journal_width_into_whole_pixel_panels() {
        let recipe = CompositeRecipe::default();
        assert_eq!(recipe.width_mm, JOURNAL_WIDTHS[1].mm);
        let grid = recipe.grid(4, 600.);
        assert_eq!((grid.columns, grid.rows), (2, 2));
        // 177.8 mm = 7 in = 4200 px at 600 DPI; ¾ of a 2100 px panel is 1575 px.
        assert_eq!(grid.cell_px, (2100, 1575));
        assert_eq!(grid.size_px(), (4200, 3150));
        assert!((grid.size_mm().0 - 177.8).abs() < 1e-9);
        // Fewer panels than columns: one column per panel.
        assert_eq!(recipe.grid(1, 600.).columns, 1);
        // ACS single column, 240 pt = 2000 px at 600 DPI, keeps every pixel.
        let acs = CompositeRecipe {
            width_mm: JOURNAL_WIDTHS[0].mm,
            columns: 1,
            ..Default::default()
        };
        assert_eq!(acs.grid(3, 600.).cell_px.0, 2000);
        let three = CompositeRecipe {
            columns: 3,
            width_mm: 171.,
            panel_height_mm: Some(40.),
            ..Default::default()
        };
        let grid = three.grid(5, 300.);
        assert_eq!((grid.columns, grid.rows), (3, 2));
        assert_eq!(grid.cell_px, (673, 472));
        assert!(grid.size_mm().0 <= 171. && grid.size_mm().0 > 171. - 3. * MM_PER_INCH / 300.);
    }

    #[test]
    fn recipe_validation_explains_each_limit() {
        let recipe = CompositeRecipe::default();
        assert!(recipe.validate(4).is_ok());
        assert!(
            recipe
                .validate(0)
                .unwrap_err()
                .contains("at least one panel")
        );
        assert!(recipe.validate(13).is_err());
        let narrow = CompositeRecipe {
            columns: 4,
            width_mm: 82.5,
            ..Default::default()
        };
        assert!(narrow.validate(4).unwrap_err().contains("25.4 mm"));
        for bad in [
            CompositeRecipe {
                dpi: 600.5,
                ..Default::default()
            },
            CompositeRecipe {
                font_size: 2.,
                ..Default::default()
            },
            CompositeRecipe {
                line_width: Some(f64::NAN),
                ..Default::default()
            },
            CompositeRecipe {
                columns: 0,
                ..Default::default()
            },
            CompositeRecipe {
                dpi: 1200.,
                panel_height_mm: Some(200.),
                ..Default::default()
            },
        ] {
            assert!(bad.validate(4).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn recipes_round_trip_and_old_projects_get_defaults() {
        let recipe = CompositeRecipe {
            panels: vec!["xanes".into(), "chi-r".into()],
            samples: Vec::new(),
            columns: 1,
            width_mm: 83.,
            panel_height_mm: Some(60.),
            font_size: 7.,
            line_width: Some(1.2),
            series_colors: BTreeMap::from([
                ("data".into(), "#0072b2".into()),
                ("future/key: [opaque]".into(), "#cc79a7".into()),
            ]),
            dpi: 300.,
            ..Default::default()
        };
        let json = serde_json::to_string(&recipe).unwrap();
        assert_eq!(
            serde_json::from_str::<CompositeRecipe>(&json).unwrap(),
            recipe
        );
        let settings: FigureSettings = serde_json::from_str(r#"{"figures":{}}"#).unwrap();
        assert_eq!(settings.active_recipe().recipe, CompositeRecipe::default());
        let old: CompositeRecipe = serde_json::from_str(r#"{"panels":["chi-k"]}"#).unwrap();
        assert!(old.series_colors.is_empty());
        assert!(
            !serde_json::to_string(&old)
                .unwrap()
                .contains("series_colors")
        );
        assert_eq!(
            CompositeRecipe::suggested_panels(["chi-r", "xanes", "fit-k", "fit-residual-k"]),
            ["xanes", "chi-r", "fit-residual-k"].map(CompositePanel::from)
        );
        assert_eq!(panel_label(0), "(a)");
        assert_eq!(panel_label(3), "(d)");
    }

    #[test]
    fn recipe_series_colors_override_matching_figure_keys_only() {
        let recipe = CompositeRecipe {
            series_colors: BTreeMap::from([
                ("data".into(), "#0072b2".into()),
                ("residual".into(), "#009e73".into()),
                ("unknown/key".into(), "#cc79a7".into()),
            ]),
            ..Default::default()
        };
        let base = FigureOptions {
            series_colors: BTreeMap::from([
                ("data".into(), "#000000".into()),
                ("fit".into(), "#e69f00".into()),
            ]),
            ..Default::default()
        };
        let merged = recipe.panel_options(base.clone(), &recipe.grid(1, 72.));
        assert_eq!(merged.series_colors["data"], "#0072b2");
        assert_eq!(merged.series_colors["fit"], "#e69f00");
        assert_eq!(merged.series_colors["residual"], "#009e73");
        assert_eq!(merged.series_colors["unknown/key"], "#cc79a7");
        assert_eq!(base.series_colors["data"], "#000000");
        assert_eq!(
            CompositeRecipe::default()
                .panel_options(base.clone(), &recipe.grid(1, 72.))
                .series_colors,
            base.series_colors
        );
        let invalid = CompositeRecipe {
            series_colors: BTreeMap::from([("data".into(), "#0072B2".into())]),
            ..Default::default()
        };
        let error = invalid.validate(1).unwrap_err();
        assert!(error.contains("data") && error.contains("#rrggbb"));
    }

    #[test]
    fn composed_fit_and_residual_use_recipe_colors_and_figure_fallbacks() {
        let recipe = CompositeRecipe {
            panels: vec!["fit-residual-k".into()],
            dpi: 72.,
            line_width: Some(2.),
            series_colors: BTreeMap::from([
                ("data".into(), "#0072b2".into()),
                ("residual".into(), "#009e73".into()),
            ]),
            ..Default::default()
        };
        let mut settings = FigureSettings::default();
        settings.figures.insert(
            "fit-residual-k".into(),
            FigureOptions {
                series_colors: BTreeMap::from([
                    ("data".into(), "#cc79a7".into()),
                    ("fit".into(), "#e69f00".into()),
                    ("residual".into(), "#d55e00".into()),
                ]),
                ..Default::default()
            },
        );
        let composed = compose(&recipe, &synthetic_fit(), &settings, None).unwrap();
        for stroke in ["rgb(0,114,178)", "rgb(230,159,0)", "rgb(0,158,115)"] {
            assert!(composed.svg.contains(&format!(r#"stroke="{stroke}""#)));
        }
        for replaced in ["rgb(204,121,167)", "rgb(213,94,0)"] {
            assert!(!composed.svg.contains(&format!(r#"stroke="{replaced}""#)));
        }
        let strip = composed
            .svg
            .split(r#"id="panel-1-residual-"#)
            .nth(1)
            .unwrap();
        assert!(strip.contains(r#"stroke="rgb(0,158,115)""#));
        for rgb in [[0, 114, 178], [230, 159, 0], [0, 158, 115]] {
            assert!(
                composed
                    .image
                    .pixels
                    .chunks_exact(4)
                    .any(|pixel| pixel[..3] == rgb && pixel[3] == 255),
                "chosen curve colour {rgb:?} reaches the PNG's raster source"
            );
        }
        let mut decoder =
            tiff::decoder::Decoder::new(std::io::Cursor::new(composed.tiff().unwrap())).unwrap();
        let tiff::decoder::DecodingResult::U8(rgb) = decoder.read_image().unwrap() else {
            panic!("publication TIFF must contain 8-bit RGB pixels");
        };
        let expected: Vec<u8> = composed
            .image
            .pixels
            .chunks_exact(4)
            .flat_map(|pixel| pixel[..3].iter().copied())
            .collect();
        assert_eq!(rgb, expected, "TIFF preserves the chosen raster colours");
    }

    #[test]
    fn composed_panels_align_and_export_every_format() {
        let mut figures = copper_figures();
        figures.extend(synthetic_fit());
        let recipe = CompositeRecipe {
            panels: ["xanes", "chi-k", "chi-r", "fit-residual-k", "missing-key"]
                .map(CompositePanel::from)
                .into(),
            dpi: 300.,
            ..Default::default()
        };
        let mut settings = FigureSettings::default();
        // A per-figure title is dropped; an Outside legend is kept inside.
        settings.figures.insert(
            "chi-k".into(),
            FigureOptions {
                title: Some("Dropped title".into()),
                legend_position: LegendPlacement::Outside,
                ..Default::default()
            },
        );
        let composed = compose(&recipe, &figures, &settings, None).unwrap();
        assert_eq!(composed.missing, ["missing-key"]);
        assert_eq!(composed.grid.size_px(), (2100, 1576));
        assert_eq!(
            (composed.image.width, composed.image.height),
            composed.grid.size_px()
        );
        assert!(composed.caption.starts_with("(a) "));
        assert!(
            composed
                .caption
                .contains(" (d) EXAFS data and fitted model")
        );
        let svg = &composed.svg;
        assert!(svg.contains(r#"width="7in" height="5.2533in" viewBox="0 0 2100 1576""#));
        for label in ["(a)", "(b)", "(c)", "(d)"] {
            assert!(svg.contains(&format!(">{label}</text>")), "{label}");
        }
        assert!(!svg.contains("Dropped title"));
        // Four panels; the fit + residual panel nests its two parts.
        assert_eq!(svg.matches(r#"<svg x=""#).count(), 6);
        assert!(svg.contains(r#"id="panel-4-residual-"#));
        // Only the root has a physical size; a nested panel sized in inches
        // would shrink to a third of its cell in vector output.
        assert_eq!(svg.matches(r#"in" height=""#).count(), 1);
        assert!(svg.contains("<desc>(a) "));

        // Panels in a row share the plot-area top edge, and panels in a column
        // share the left edge: the first dark axis pixel of each panel lines up.
        let dark = |x: u32, y: u32| {
            let i = ((y * composed.image.width + x) * 4) as usize;
            composed.image.pixels[i..i + 3].iter().all(|v| *v < 80)
        };
        let (cw, ch) = composed.grid.cell_px;
        // The y-axis spine: the first vertical dark run left of the panel centre.
        let left_edge = |x0: u32, y0: u32| {
            let y = y0 + ch / 2;
            (x0..x0 + cw / 2)
                .rev()
                .find(|&x| (0..8).all(|d| dark(x, y + d)))
        };
        // The top spine: the first horizontal dark run below the panel top.
        let top_edge = |x0: u32, y0: u32| {
            let x = x0 + cw / 2;
            (y0 + 1..y0 + ch / 2).find(|&y| (0..8).all(|d| dark(x + d, y)))
        };
        let a = left_edge(0, 0).unwrap();
        assert_eq!(left_edge(0, ch), Some(a), "(a) and (c) share a left edge");
        assert_eq!(
            left_edge(cw, 0),
            Some(a + cw),
            "(a) and (b) have equal margins"
        );
        assert_eq!(
            left_edge(cw, ch),
            Some(a + cw),
            "(b) and (d) share a left edge"
        );
        let top = top_edge(0, 0).unwrap();
        assert_eq!(top_edge(cw, 0), Some(top), "(a) and (b) share a top edge");
        assert_eq!(
            top_edge(cw, ch),
            Some(top + ch),
            "(c) and (d) share a top edge"
        );

        let png = composed.png().unwrap();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        let tiff = composed.tiff().unwrap();
        assert_eq!(&tiff[..4], b"II*\0");
        let pdf = composed.pdf().unwrap();
        assert!(pdf.starts_with(b"%PDF"));
        // 7 in = 504 pt page width.
        let text = String::from_utf8_lossy(&pdf);
        let media = text.find("/MediaBox").map(|i| &text[i..i + 40]);
        assert!(media.is_some_and(|m| m.contains(" 504")), "{media:?}");
        assert!(composed.bytes("jpeg").is_err());

        if let Some(dir) = std::env::var_os("REXAFS_COMPOSER_SHOTS") {
            let dir = Path::new(&dir);
            std::fs::create_dir_all(dir).unwrap();
            for extension in ["png", "svg", "pdf", "tiff"] {
                std::fs::write(
                    dir.join(format!("composed-4-panel.{extension}")),
                    composed.bytes(extension).unwrap(),
                )
                .unwrap();
            }
            let single = CompositeRecipe {
                panels: ["xanes", "chi-k", "chi-r"].map(CompositePanel::from).into(),
                columns: 1,
                width_mm: JOURNAL_WIDTHS[0].mm,
                panel_height_mm: Some(55.),
                ..Default::default()
            };
            let composed = compose(&single, &figures, &FigureSettings::default(), None).unwrap();
            for extension in ["png", "svg"] {
                std::fs::write(
                    dir.join(format!("composed-acs-1-column.{extension}")),
                    composed.bytes(extension).unwrap(),
                )
                .unwrap();
            }
        }
    }

    #[test]
    fn preview_dpi_keeps_the_physical_layout() {
        let figures = copper_figures();
        let recipe = CompositeRecipe {
            panels: vec!["xanes".into(), "chi-r".into()],
            ..Default::default()
        };
        let preview = compose(&recipe, &figures, &FigureSettings::default(), Some(100.)).unwrap();
        assert_eq!(preview.grid.size_px(), (700, 263));
        assert!(preview.svg.contains(r#"width="7in""#));
        let empty: Vec<FigureData> = vec![];
        let missing = compose(&recipe, &empty, &FigureSettings::default(), None);
        assert!(missing.err().unwrap().contains("at least one panel"));
    }
}
