//! Publication figures keep ruviz's defaults and share one renderer with preview.
use super::*;
use ruviz::prelude::{LineStyle, Plot};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

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

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct FigureOptions {
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub dpi: Option<f64>,
    pub font_size: Option<f64>,
    pub line_width: Option<f64>,
    pub title: Option<String>,
    pub caption: Option<String>,
    pub xlabel: Option<String>,
    pub ylabel: Option<String>,
    pub xmin: Option<f64>,
    pub xmax: Option<f64>,
    pub ymin: Option<f64>,
    pub ymax: Option<f64>,
    pub legend: bool,
    pub grid: Option<bool>,
    pub guides: bool,
    pub hidden: BTreeSet<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct FigureSettings {
    /// Settings are shared by figure type across a multi-spectrum export.
    pub figures: BTreeMap<String, FigureOptions>,
    pub table_captions: BTreeMap<String, String>,
}
impl FigureSettings {
    pub fn options(&self, key: &str) -> FigureOptions {
        self.figures.get(key).cloned().unwrap_or_default()
    }
}

impl FigureOptions {
    pub fn dimensions(&self) -> (f64, f64, f64) {
        let default = Plot::new();
        let figure = &default.get_config().figure;
        (
            self.width.unwrap_or(figure.width as f64),
            self.height.unwrap_or(figure.height as f64),
            self.dpi.unwrap_or(figure.dpi as f64),
        )
    }

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
        Ok(())
    }

    fn apply(&self, mut plot: Plot) -> Result<Plot, String> {
        self.validate()?;
        let (width, height, dpi) = self.dimensions();
        // Unset controls do not override ruviz, including its physical size.
        if self.width.is_some() || self.height.is_some() {
            plot = plot.size(width as f32, height as f32);
        }
        if self.dpi.is_some() {
            plot = plot.dpi(dpi as u32);
        }
        if let Some(size) = self.font_size {
            plot = plot.font_size(size as f32);
        }
        if let Some(title) = &self.title {
            plot = plot.title(title.clone());
        }
        if let Some(label) = &self.xlabel {
            plot = plot.xlabel(label.clone());
        }
        if let Some(label) = &self.ylabel {
            plot = plot.ylabel(label.clone());
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
            plot = plot.legend_best();
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
}

#[derive(Clone)]
pub(crate) struct FigureData {
    pub key: &'static str,
    pub label: String,
    pub xlabel: String,
    pub ylabel: String,
    pub series: Vec<FigureSeries>,
    pub guides: Vec<f64>,
}

impl FigureData {
    /// A factual starting caption, derived only from the curves actually shown.
    pub fn caption(&self, options: &FigureOptions) -> String {
        if let Some(caption) = &options.caption {
            return caption.clone();
        }
        let description = match self.key {
            "mu-energy" => "X-ray absorption spectrum and selected normalization/background curves",
            "normalized-mu" => "Edge-step-normalized X-ray absorption spectrum",
            "chi-k" => "Background-subtracted EXAFS as a function of photoelectron wave number",
            "chi-r" => "Fourier-transformed EXAFS; radial coordinates are not phase corrected",
            "chi-q" => "Back-transformed EXAFS",
            "fit-k" => "EXAFS data, fitted model and selected path contributions in k space",
            "fit-r" => {
                "EXAFS data, fitted model and selected components in R space; radial coordinates are not phase corrected"
            }
            "fit-q" => "Back-transformed EXAFS data and fitted model",
            "residual-k" => "Weighted EXAFS residual, defined as data minus fitted model",
            "residual-r" => {
                "Difference between the magnitudes of the data and fitted Fourier transforms; this is not the magnitude of the complex residual"
            }
            _ => &self.label,
        };
        let curves = self
            .series
            .iter()
            .filter(|s| !s.x.is_empty() && !options.hidden.contains(&s.key))
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
            options.ylabel.as_deref().unwrap_or(&self.ylabel)
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
        let mut plot = options.apply(Plot::new().xlabel(&self.xlabel).ylabel(&self.ylabel))?;
        let mut count = 0;
        for series in &self.series {
            if options.hidden.contains(&series.key) || series.x.is_empty() {
                continue;
            }
            let mut line = plot.line(&series.x, &series.y).label(&series.label);
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
    let plot = data.plot(options)?;
    let mut svg = plot.render_to_svg().map_err(|e| e.to_string())?;
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
    }
    Ok(RenderedFigure {
        png: plot.render_png_bytes().map_err(|e| e.to_string())?,
        svg,
    })
}

pub(crate) fn spectrum_figures(sp: Arc<XASSpectrum>, label: &str) -> Vec<FigureData> {
    let weight = sp.kweight().copied().unwrap_or(2.);
    let opts = plotting::ViewOptions {
        flat: false,
        show_re: true,
        show_im: true,
        show_bkg: true,
        show_pre: true,
        show_post: true,
        show_e0: true,
        show_ranges: true,
        show_kwin: true,
        ..Default::default()
    };
    let traces = [plotting::QuadTrace {
        label: label.into(),
        sp,
        active: true,
    }];
    ["mu-energy", "normalized-mu", "chi-k", "chi-r", "chi-q"]
        .into_iter()
        .zip(plotting::build_quadrant_specs(
            &traces,
            &opts,
            &Theme::light(),
            true,
        ))
        .map(|(key, spec)| FigureData {
            key,
            label: spec.title,
            xlabel: spec.xlabel,
            ylabel: match key {
                "mu-energy" => "μ(E) (arb. units)".into(),
                "normalized-mu" => "Normalized μ(E) (dimensionless)".into(),
                "chi-k" => weighted_chi_label(weight),
                "chi-r" => plotting::chir_label(weight).replace("|χ(R)|", "χ(R)"),
                "chi-q" => format!("Re χ(q) {}", weighted_unit(weight)),
                _ => spec.ylabel,
            },
            guides: spec.vlines.into_iter().map(|(x, _, _, _)| x).collect(),
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
                })
                .collect(),
        })
        .collect()
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
        }
    };
    let figure = |key, label: &str, xlabel: &str, ylabel: String, series| FigureData {
        key,
        label: label.into(),
        xlabel: xlabel.into(),
        ylabel,
        series,
        guides: vec![],
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
            plotting::chir_label(kw).replace("|χ(R)|", "χ(R)"),
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
    fn sample() -> FigureData {
        FigureData {
            key: "test",
            label: "Test".into(),
            xlabel: "Energy (eV)".into(),
            ylabel: "μ(E)".into(),
            guides: vec![],
            series: vec![FigureSeries {
                key: "data".into(),
                label: "Data".into(),
                x: vec![0., 1., 2.],
                y: vec![0., 1., 0.],
                dashed: false,
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
    fn default_publication_size_matches_ruviz_and_custom_dpi_preserves_shape() {
        let defaults = Plot::new();
        let canvas = defaults.get_config().canvas_size();
        let rendered = render_figure(&sample(), &FigureOptions::default()).unwrap();
        assert_eq!(png_size(&rendered.png), canvas);
        assert!(rendered.svg.contains("Energy"));
        let options = FigureOptions {
            width: Some(4.),
            height: Some(3.),
            dpi: Some(200.),
            title: Some("Copper sample".into()),
            xlabel: Some("Photon energy".into()),
            ..Default::default()
        };
        let rendered = render_figure(&sample(), &options).unwrap();
        assert_eq!(png_size(&rendered.png), (800, 600));
        assert!(rendered.svg.contains("Copper sample"));
        assert!(rendered.svg.contains("Photon energy"));
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
