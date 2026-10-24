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
//! Layout and appearance change; the plotted arrays never do.
use super::layout::{self, Frame, LegendFrame, Metrics};
use super::{FigureData, FigureOptions, FigureSettings, LegendPlacement};
use ruviz::core::Image;
use ruviz::prelude::Plot;
use serde::{Deserialize, Serialize};
use std::borrow::Borrow;
use std::collections::BTreeMap;

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

/// A saved multi-panel figure: which figures form the panels and the
/// appearance they share. Stored with the figure settings in the `.rxs`
/// project as the contents of a named entry in `publication.recipes`.
///
/// Per-figure settings from the single-figure editor still apply inside each
/// panel: axis labels, axis limits, visible curves, legend placement, grid,
/// guides and the flattened/normalized choice. The recipe replaces the size,
/// resolution, font size and line width, and drops titles, because panel
/// labels identify the panels. Explicit recipe series colours override the
/// corresponding per-figure colours; other series keep their figure settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct CompositeRecipe {
    /// Figure keys (such as `xanes`, `chi-k`, `fit-residual-k`), one panel
    /// each, in reading order: left to right, then top to bottom.
    pub panels: Vec<String>,
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
    /// Starting panels for a new recipe: XANES, χ(k) and |χ(R)| of the current
    /// spectrum, plus the k-space fit + residual when a fit is available.
    pub fn suggested_panels<'a>(available: impl IntoIterator<Item = &'a str>) -> Vec<String> {
        let available: Vec<&str> = available.into_iter().collect();
        ["xanes", "chi-k", "chi-r", "fit-residual-k"]
            .into_iter()
            .filter(|key| available.contains(key))
            .map(str::to_string)
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
    fn panel_options(&self, mut base: FigureOptions, grid: &Grid) -> FigureOptions {
        base.series_colors.extend(self.series_colors.clone());
        FigureOptions {
            width: Some(grid.cell_px.0 as f64 / grid.dpi),
            height: Some(grid.cell_px.1 as f64 / grid.dpi),
            dpi: Some(grid.dpi),
            font_size: Some(self.font_size),
            line_width: self.line_width,
            title: None,
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
    Single(Box<Plot>),
    FitResidual(Box<layout::FitResidual>),
}

/// Compose the recipe's panels from `figures` (the figures available for the
/// current spectrum and fit) using the per-figure options in `settings`.
///
/// `dpi` overrides the recipe's resolution, for a quicker preview; physical
/// sizes and the layout are unchanged. Panels missing from `figures` are
/// skipped and listed in [`ComposedFigure::missing`]; the remaining panels are
/// labelled consecutively. Errors leave the recipe and data unchanged.
pub(crate) fn compose(
    recipe: &CompositeRecipe,
    figures: &[impl Borrow<FigureData>],
    settings: &FigureSettings,
    dpi: Option<f64>,
) -> Result<ComposedFigure, String> {
    let mut missing = Vec::new();
    let mut chosen = Vec::new();
    for key in &recipe.panels {
        match figures.iter().map(Borrow::borrow).find(|f| f.key == key) {
            Some(figure) => chosen.push(figure),
            None => missing.push(key.clone()),
        }
    }
    recipe.validate(chosen.len())?;
    let grid = recipe.grid(chosen.len(), dpi.unwrap_or(recipe.dpi));

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
    let mut captions = Vec::new();
    for (index, data) in chosen.iter().enumerate() {
        let mut options = recipe.panel_options(settings.options(data.key), &grid);
        captions.push(format!("{} {}", panel_label(index), data.caption(&options)));
        let (panel, needs) = match &data.residual {
            Some(residual) => {
                let panels = layout::fit_residual_panels(data, residual, &options)?;
                let needs = panels.frame;
                (Built::FitResidual(Box::new(panels)), needs)
            }
            None => {
                resolve_inside_legend(data, &mut options);
                let plot = data.panel(&options, false)?;
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
                (Built::Single(Box::new(plot)), needs)
            }
        };
        for (shared, need) in frame.iter_mut().zip(needs) {
            *shared = shared.max(need);
        }
        built.push(panel);
    }

    let (width, height) = grid.size_px();
    let (cell_w, cell_h) = grid.cell_px;
    let mut pixels = vec![255u8; width as usize * height as usize * 4];
    let mut svgs = Vec::new();
    let cell_height_in = cell_h as f64 / grid.dpi;
    for (index, panel) in built.into_iter().enumerate() {
        let (image, svg) = match panel {
            Built::Single(plot) => {
                let plot = layout::panel_frame(*plot, cell_height_in, frame);
                let image = plot.render().map_err(|e| e.to_string())?;
                (image, plot.render_to_svg().map_err(|e| e.to_string())?)
            }
            Built::FitResidual(panels) => panels.render(frame)?,
        };
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
        missing,
        caption,
    })
}

/// Gap between a panel's top-left corner and its label, in points.
const LABEL_INSET_PT: f64 = 2.;

/// Resolve an Auto or Outside legend to an inside corner. An outside legend
/// would shrink one panel's plot area and break the alignment with its
/// neighbours, so the composer keeps legends inside (as the fit + residual
/// figure does).
fn resolve_inside_legend(data: &FigureData, options: &mut FigureOptions) {
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
    use crate::publication::figures::{fit_figures, spectrum_figures};
    use std::{path::Path, sync::Arc};

    fn copper_figures() -> Vec<FigureData> {
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
            ["xanes", "chi-r", "fit-residual-k"]
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
                .map(String::from)
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
                panels: ["xanes", "chi-k", "chi-r"].map(String::from).into(),
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
