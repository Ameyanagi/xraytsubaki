//! Export the prepared active recipe without recomputing samples or insets.
use super::*;
use crate::publication::figures::LegendPlacement;
use crate::publication::figures::composer::{
    CompositeRecipe, CurveLabelMode, DisplayOffset, InsetPlacement, PreparedComposition,
    curve_end_label_texts, panel_label, resolve_inside_legend,
};

/// Write the named recipe's available panels into a new folder. CSV ordinates
/// precede display stacking; the script adds editable offsets in axis units.
/// Prepared data must belong to this recipe and its current figure settings.
/// Invalid settings or no visible data fail before creating the directory;
/// existing destinations are rejected. Returns the generated script path.
pub(crate) fn write_composite_python_export(
    name: &str,
    recipe: &CompositeRecipe,
    prepared: &PreparedComposition,
    folder: &Path,
) -> Result<std::path::PathBuf, String> {
    write_files(&composite_python_export(name, recipe, prepared)?, folder)
}

/// Generate a script-first bundle, compressing unavailable panels as the preview
/// does. Notices retain the preparation's missing samples and panel names.
fn composite_python_export(
    name: &str,
    recipe: &CompositeRecipe,
    prepared: &PreparedComposition,
) -> Result<Vec<(String, String)>, String> {
    let mut missing = prepared.missing.clone();
    let panels: Vec<_> = prepared
        .panels
        .iter()
        .filter(|panel| {
            let available = panel
                .data
                .series(&panel.options)
                .iter()
                .any(|curve| curve.visible(&panel.options) && !curve.x.is_empty());
            if !available {
                missing.push(format!("{}: no visible curves with data", panel.data.key));
            }
            available
        })
        .collect();
    recipe.validate(panels.len()).map_err(|error| {
        if missing.is_empty() {
            error
        } else {
            format!("{error} Missing: {}.", missing.join("; "))
        }
    })?;
    let grid = recipe.grid(panels.len(), recipe.dpi);
    let (width_mm, height_mm) = grid.size_mm();
    let (width_mm, height_mm) = (round_default(width_mm), round_default(height_mm));
    let default = Plot::new();
    let line_width = recipe
        .line_width
        .unwrap_or_else(|| round_default(default.get_config().lines.data_width as f64));
    let mut files = Vec::new();
    let mut entries = String::new();
    let end_labels = curve_end_label_texts(recipe, &panels, &grid)?;
    for (index, panel) in panels.iter().enumerate() {
        let spec = recipe
            .panels
            .get(panel.index)
            .ok_or("Prepared panel does not belong to this recipe.")?;
        let data = &panel.data;
        let mut options = recipe.panel_options(panel.options.clone(), &grid);
        options.validate()?;
        let curve_end = !spec.uses_current_fit() && spec.label_mode == CurveLabelMode::CurveEnd;
        if curve_end {
            options.legend = false;
        }
        resolve_inside_legend(
            data,
            &mut options,
            panel.inset.as_ref().map(|inset| inset.placement),
        );
        let letter = (b'a' + index as u8) as char;
        let series = series_entries(
            &format!("{letter}-series"),
            data,
            &options,
            &panel.offsets,
            curve_end.then_some(&end_labels[index]),
            &mut files,
        )?;
        let inset = if let Some(inset) = &panel.inset {
            let inset_series = series_entries(
                &format!("{letter}-inset-series"),
                &inset.data,
                &inset.options,
                &inset.offsets,
                None,
                &mut files,
            )?;
            let bounds = match inset.placement {
                // Leave room below the inset frame for its tick labels and
                // their white backgrounds inside the main plotting area.
                InsetPlacement::LowerRight => "(0.54, 0.14, 0.43, 0.36)",
                InsetPlacement::UpperLeft => "(0.03, 0.60, 0.43, 0.36)",
                InsetPlacement::CenterRight => "(0.54, 0.32, 0.43, 0.36)",
            };
            format!(
                "{{\"bounds\": {bounds}, \"quantity\": {}, {}, \"series\": [\n{inset_series}        ]}}",
                py_str(inset.quantity),
                // Inset y limits are never typed by the user; leave them
                // automatic so the script fits them to its offset curves.
                axes_settings(
                    &inset.data,
                    &FigureOptions {
                        ymin: None,
                        ymax: None,
                        ..inset.options.clone()
                    }
                )
            )
        } else {
            "None".into()
        };
        let residual = if let Some(residual) = &data.residual {
            let strip = FigureData {
                series: residual.clone(),
                normalized_series: None,
                residual: None,
                ylabel: "Residual".into(),
                math_ylabel: "Residual".into(),
                guides: Vec::new(),
                default_xlim: data.xlim(&options),
                ..data.clone()
            };
            let strip_options = FigureOptions {
                ymin: None,
                ymax: None,
                ylabel: None,
                normalized: false,
                legend: false,
                guides: false,
                hidden: Default::default(),
                shown: Default::default(),
                ..options.clone()
            };
            let half = strip
                .visible_ylim(&strip_options)
                .map(|(lo, hi)| lo.abs().max(hi.abs()))
                .filter(|value| *value > 0.)
                .unwrap_or(1.);
            let strip_options = FigureOptions {
                ymin: Some(-half),
                ymax: Some(half),
                ..strip_options
            };
            let residual_series = series_entries(
                &format!("{letter}-residual-series"),
                &strip,
                &strip_options,
                &[],
                None,
                &mut files,
            )?;
            format!(
                "{{{}, \"series\": [\n{residual_series}        ]}}",
                axes_settings(&strip, &strip_options)
            )
        } else {
            "None".into()
        };
        entries.push_str(&format!(
            "    {{\n        \"label\": {},\n        {},\n        \"curve_end\": {},\n        \"series\": [\n{series}        ],\n        \"inset\": {inset},\n        \"residual\": {residual},\n    }},\n",
            py_str(&panel_label(index)), axes_settings(data, &options), py_bool(curve_end),
        ));
    }
    let notices = missing
        .iter()
        .map(|notice| py_str(notice))
        .collect::<Vec<_>>()
        .join(", ");
    let script = format!(
        r#"r"""Reproduce a composed rexafs figure with Python 3, numpy and matplotlib.

Generated by rexafs {version}. Run: python make_figure.py
CSV files next to this script contain complete, unshifted x/y arrays with one
CSV header record. Insets retain their prepared scientific quantity, including Δμ
for differences. Display offsets are added only by this script.

Edit the figure settings and PANELS below, then run again to save figure.pdf,
figure.svg and figure.png next to this script. Size is in millimetres; font
size and line width are in points. Each series offset is in its own y units;
main and inset offsets are independent editable constants. Axis limits set the
view only. Labels use matplotlib mathtext ($...$), as in the single-figure
export. Fonts, ticks and margins are chosen by matplotlib, so this is an
editable reproduction rather than a pixel copy of the preview.
"""
from pathlib import Path
import csv

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np

HERE = Path(__file__).resolve().parent

# Figure settings
RECIPE_NAME = {name}
ROWS = {rows}
COLS = {columns}
WIDTH_MM = {width_mm:?}
HEIGHT_MM = {height_mm:?}
DPI = {dpi:?}
FONT_SIZE = {font_size:?}
LINE_WIDTH = {line_width:?}
SKIPPED = [{notices}]

# Per-panel settings. Each CSV is unshifted; change "offset" to adjust stacking.
# Curve-end "label" matches the shortened native text. "full_label" retains
# the complete name for editing and legends; CSV headers also keep full names.
# Inset bounds are (left, bottom, width, height) fractions of the main axes.
# Limits are (min, max), or None for matplotlib's automatic range.
PANELS = [
{entries}]

{style}
{load_series}
{draw}
"#,
        version = env!("CARGO_PKG_VERSION"),
        name = py_str(name),
        rows = grid.rows,
        columns = grid.columns,
        dpi = recipe.dpi,
        font_size = recipe.font_size,
        style = PYTHON_STYLE,
        draw = DRAW,
        load_series = PYTHON_LOAD_SERIES,
    );
    files.insert(0, (SCRIPT_NAME.into(), script));
    Ok(files)
}

/// Emit full arrays separately from the additive, editable presentation shift.
fn series_entries(
    prefix: &str,
    data: &FigureData,
    options: &FigureOptions,
    offsets: &[Option<DisplayOffset>],
    end_labels: Option<&std::collections::BTreeMap<usize, String>>,
    files: &mut Vec<(String, String)>,
) -> Result<String, String> {
    let mut entries = String::new();
    for (index, (source_index, curve)) in data
        .series(options)
        .iter()
        .enumerate()
        .filter(|(_, curve)| curve.visible(options) && !curve.x.is_empty())
        .enumerate()
    {
        let file = format!("{prefix}-{:02}.csv", index + 1);
        let shift = offsets.get(source_index).and_then(Option::as_ref);
        let y = shift.map_or(curve.y.as_slice(), |shift| shift.unshifted_y.as_slice());
        files.push((
            file.clone(),
            series_csv(curve, &data.xlabel, data.ylabel(options), y)?,
        ));
        let label = end_labels
            .and_then(|labels| labels.get(&source_index))
            .unwrap_or(&curve.label);
        let full_label = end_labels.map_or_else(String::new, |_| {
            format!(", \"full_label\": {}", py_str(&plain_text(&curve.label)))
        });
        entries.push_str(&format!(
            "            {{\"file\": {}, \"label\": {}{full_label}, \"color\": {}, \"linestyle\": {}, \"offset\": {:?}}},\n",
            py_str(&file), py_str(&plain_text(label)), py_str(&series_color(curve, options, index)),
            py_str(if curve.dashed { "--" } else { "-" }), shift.map_or(0., |shift| shift.value),
        ));
    }
    Ok(entries)
}

/// Axis settings use the same labels and visible ranges as a single export.
fn axes_settings(data: &FigureData, options: &FigureOptions) -> String {
    let (xlabel, ylabel) = axis_labels(data, options);
    let legend = match options.legend_position {
        LegendPlacement::Auto | LegendPlacement::Outside => "best",
        LegendPlacement::UpperLeft => "upper left",
        LegendPlacement::UpperRight => "upper right",
        LegendPlacement::LowerLeft => "lower left",
        LegendPlacement::LowerRight => "lower right",
    };
    let guides = if options.guides {
        data.guides
            .iter()
            .map(|guide| format!("{guide:?}"))
            .collect::<Vec<_>>()
            .join(", ")
    } else {
        String::new()
    };
    format!(
        "\"xlabel\": {},\n        \"ylabel\": {},\n        \"xlim\": {},\n        \"ylim\": {},\n        \"ylim_fixed\": {},\n        \"grid\": {},\n        \"legend\": {},\n        \"legend_location\": {},\n        \"guides\": [{guides}]",
        py_str(&xlabel),
        py_str(&ylabel),
        py_limit(data.xlim(options)),
        py_limit(
            options
                .ymin
                .zip(options.ymax)
                .or_else(|| data.visible_ylim(options))
        ),
        py_bool(options.ymin.zip(options.ymax).is_some()),
        py_bool(options.grid.unwrap_or(true)),
        py_bool(options.legend),
        py_str(legend),
    )
}

/// Python rendering keeps labels and endpoints responsive to edited offsets.
const DRAW: &str = r##"
def draw_curves(ax, settings, width):
    curves = []
    for series in settings["series"]:
        x, y = load_series(series["file"])
        y = y + series["offset"]
        ax.plot(x, y, label=series.get("full_label", series["label"]), color=series["color"],
                linestyle=series["linestyle"], linewidth=width)
        curves.append((x, y, series))
    for guide in settings["guides"]:
        ax.axvline(guide, color="#8c8c8c", linewidth=0.8, linestyle="--")
    ax.set_xlabel(settings["xlabel"])
    ax.set_ylabel(settings["ylabel"])
    if settings["xlim"] is not None:
        ax.set_xlim(*settings["xlim"])
    if settings["ylim"] is not None:
        low, high = settings["ylim"]
        if not settings["ylim_fixed"] and curves:
            # Automatic limits describe unshifted data; widen them to the
            # curves as drawn, after the offsets above.
            xlow, xhigh = ax.get_xlim()
            drawn = np.concatenate([y[(x >= xlow) & (x <= xhigh)] for x, y, _ in curves])
            drawn = drawn[np.isfinite(drawn)]
            if drawn.size and (drawn.min() < low or drawn.max() > high):
                low, high = min(low, drawn.min()), max(high, drawn.max())
                pad = 0.05 * (high - low)
                low, high = low - pad, high + pad
        ax.set_ylim(low, high)
    ax.grid(settings["grid"])
    if settings["legend"]:
        ax.legend(loc=settings["legend_location"])
    return curves


def curve_end_labels(ax, curves):
    """Label the rightmost visible point, interpolating at clipped boundaries."""
    xmin, xmax = ax.get_xlim()
    ymin, ymax = ax.get_ylim()
    labels = []
    height_pt = ax.get_window_extent().height / ax.figure.dpi * 72
    gap = 2 / height_pt
    for x, y, series in curves:
        points = list(zip(x, y))
        for x0, x1, y0, y1 in zip(x[:-1], x[1:], y[:-1], y[1:]):
            for bound in (xmin, xmax):
                if min(x0, x1) < bound < max(x0, x1):
                    points.append((bound, y0 + (y1 - y0) * (bound - x0) / (x1 - x0)))
            for bound in (ymin, ymax):
                if min(y0, y1) < bound < max(y0, y1):
                    points.append((x0 + (x1 - x0) * (bound - y0) / (y1 - y0), bound))
        visible = [(px, py) for px, py in points
                   if np.isfinite(px) and np.isfinite(py)
                   and xmin <= px <= xmax and ymin <= py <= ymax]
        if visible:
            _, end_y = max(visible, key=lambda point: point[0])
            height = 1.2 * FONT_SIZE * (series["label"].count("\n") + 1) / height_pt
            labels.append([(end_y - ymin) / (ymax - ymin), height, series])
    labels.sort(key=lambda item: item[0])
    bottom = 0
    for label in labels:
        label[0] = max(label[0], bottom + label[1] / 2)
        bottom = label[0] + label[1] / 2 + gap
    top = 1
    for label in reversed(labels):
        label[0] = min(label[0], top - label[1] / 2)
        top = label[0] - label[1] / 2 - gap
    for position, _, series in labels:
        ax.annotate(series["label"], xy=(1, position), xycoords="axes fraction",
                    xytext=(4, 0), textcoords="offset points", va="center",
                    color=series["color"], fontsize=FONT_SIZE, annotation_clip=False)


fig, axes = plt.subplots(ROWS, COLS, figsize=(WIDTH_MM / 25.4, HEIGHT_MM / 25.4),
                         squeeze=False, layout="constrained")
end_labels = []
inset_axes = []
for ax, panel in zip(axes.flat, PANELS):
    residual = panel["residual"]
    if residual is not None:
        nested = ax.get_subplotspec().subgridspec(2, 1, height_ratios=(3, 1), hspace=0.08)
        ax.remove()
        ax = fig.add_subplot(nested[0])
        strip = fig.add_subplot(nested[1], sharex=ax)
        draw_curves(strip, residual, LINE_WIDTH)
        strip.axhline(0, color="#8c8c8c", linewidth=0.8)
    curves = draw_curves(ax, panel, LINE_WIDTH)
    if residual is not None:
        ax.set_xlabel("")
        ax.tick_params(labelbottom=False)
    ax.text(0, 1.04, panel["label"], transform=ax.transAxes, fontweight="bold",
            fontsize=1.1 * FONT_SIZE, va="bottom")
    if panel["curve_end"]:
        end_labels.append((ax, curves))
    inset = panel["inset"]
    if inset is not None:
        inset_ax = ax.inset_axes(inset["bounds"])
        draw_curves(inset_ax, inset, 0.7 * LINE_WIDTH)
        # Headroom keeps the quantity tag clear of the curves, as in rexafs.
        low, high = inset_ax.get_ylim()
        inset_ax.set_ylim(low, high + 0.2 * (high - low))
        inset_ax.tick_params(labelsize=max(4, 0.8 * FONT_SIZE))
        inset_ax.set_yticks([])
        inset_ax.text(0.04, 0.96, inset["quantity"], transform=inset_ax.transAxes,
                      va="top", fontsize=max(4, 0.8 * FONT_SIZE),
                      bbox=dict(facecolor="white", edgecolor="none", pad=1))
        inset_axes.append(inset_ax)
for ax in list(axes.flat)[len(PANELS):]:
    ax.set_visible(False)
# Resolve axes heights before separating direct labels by their text height.
fig.canvas.draw()
for ax, curves in end_labels:
    curve_end_labels(ax, curves)
# Inset tick labels sit over the main plot; give them a white background.
for inset_ax in inset_axes:
    for tick in inset_ax.get_xticklabels():
        tick.set_bbox(dict(facecolor="white", edgecolor="none", pad=0.5))
for notice in SKIPPED:
    print(f"Skipped: {notice}")
for extension in ("pdf", "svg", "png"):
    fig.savefig(HERE / f"figure.{extension}", dpi=DPI)
print(f"Saved figure.pdf, figure.svg and figure.png in {HERE}")
"##;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::group_identity::GroupId;
    use crate::publication::figures::FigureSettings;
    use crate::publication::figures::composer::{
        CompositePanel, CompositeSample, InsetKind, PanelInset, prepare_panels,
    };
    use std::sync::Arc;

    /// Synthetic values keep display/export checks independent of processing.
    fn figure(key: &'static str, label: &str, y: Vec<f64>) -> FigureData {
        FigureData {
            key,
            label: label.into(),
            xlabel: "Energy (eV)".into(),
            ylabel: "μ(E) (dimensionless)".into(),
            math_ylabel: "_μ_(_E_) (dimensionless)".into(),
            series: vec![FigureSeries {
                key: "mu".into(),
                label: label.into(),
                x: vec![7100., 7101., 7102., 7103., 7104.],
                y,
                dashed: false,
                optional: false,
            }],
            normalized_series: None,
            e0: Some(7101.),
            guides: vec![7101.],
            default_xlim: Some((7100., 7104.)),
            residual: None,
        }
    }

    /// Three available panels, a missing fit, two samples, stacking and an inset.
    fn fixture() -> (CompositeRecipe, PreparedComposition) {
        let mut first = figure(
            "xanes",
            "A, foil $1\ncontinued",
            vec![1e-20, 1., 2., 1., 0.],
        );
        let mut second = figure("xanes", "B", vec![0.5, 1.5, 2.5, 1.5, 0.5]);
        first.normalized_series = Some(first.series.clone());
        second.normalized_series = Some(second.series.clone());
        let samples: Vec<_> = [first, second]
            .into_iter()
            .enumerate()
            .map(|(index, data)| CompositeSample {
                id: GroupId::legacy_result(index as u64 + 1),
                label: data.label.clone(),
                color: if index == 0 { "#0072b2" } else { "#d55e00" }.into(),
                figures: vec![Arc::new(data)],
            })
            .collect();
        let recipe = CompositeRecipe {
            panels: vec![
                CompositePanel {
                    offset_step: 0.3,
                    reverse: true,
                    label_mode: CurveLabelMode::CurveEnd,
                    inset: Some(PanelInset {
                        energy_range: (-1., 3.),
                        placement: InsetPlacement::UpperLeft,
                        ..PanelInset::new(InsetKind::Edge)
                    }),
                    ..CompositePanel::from("xanes")
                },
                "fit-q".into(),
                CompositePanel {
                    samples: Some(Vec::new()),
                    ..CompositePanel::from("chi-k")
                },
                "fit-residual-k".into(),
            ],
            samples: samples.iter().map(|sample| sample.id.clone()).collect(),
            columns: 2,
            width_mm: 177.8,
            panel_height_mm: Some(50.8),
            dpi: 300.,
            font_size: 9.,
            line_width: Some(1.25),
            series_colors: [("result:1/mu".into(), "#4f2b7c".into())].into(),
        };
        let chi = figure("chi-k", "χ(k)", vec![0., 1., -1., 1., 0.]);
        let mut fit = figure("fit-residual-k", "Fit", vec![0., 1., 2., 1., 0.]);
        let mut fitted = fit.series[0].clone();
        fitted.key = "fit".into();
        fitted.label = "Fit curve".into();
        fitted.dashed = true;
        fit.series.push(fitted);
        fit.ylabel = "k²χ(k) (Å⁻²)".into();
        fit.math_ylabel = "_k_#super[2]_χ_(_k_) (Å#super[−2])".into();
        let mut residual = fit.series[0].clone();
        residual.key = "residual".into();
        residual.y = vec![-0.1, 0., 0.1, 0., -0.1];
        fit.residual = Some(vec![residual]);
        let mut settings = FigureSettings::default();
        settings.figures = [(
            "chi-k".into(),
            FigureOptions {
                xlabel: Some("$k$ ($\"Å\"^(-1)$)".into()),
                ylabel: Some("$k^2 chi(k)$".into()),
                xmin: Some(7100.5),
                xmax: Some(7103.5),
                ymin: Some(-2.),
                ymax: Some(2.),
                guides: true,
                grid: Some(false),
                legend_position: LegendPlacement::LowerLeft,
                ..Default::default()
            },
        )]
        .into();
        let prepared = prepare_panels(&recipe, &[chi, fit], &samples, &settings).unwrap();
        (recipe, prepared)
    }

    #[test]
    fn grid_script_preserves_style_offsets_insets_and_missing_notices() {
        let (recipe, prepared) = fixture();
        let files = composite_python_export("SI Figure \"S4\"\n$", &recipe, &prepared).unwrap();
        let script = &files[0].1;
        for expected in [
            "ROWS = 2",
            "COLS = 2",
            "WIDTH_MM = 177.8",
            "HEIGHT_MM = 101.6",
            "DPI = 300.0",
            "FONT_SIZE = 9.0",
            r#""font.family": ["Arial", "DejaVu Sans"],"#,
            "LINE_WIDTH = 1.25",
            "plt.subplots(ROWS, COLS, figsize=(WIDTH_MM / 25.4, HEIGHT_MM / 25.4)",
            "ax.inset_axes(inset[\"bounds\"])",
            "\"bounds\": (0.03, 0.60, 0.43, 0.36)",
            "\"label\": \"(a)\"",
            "\"label\": \"(b)\"",
            "\"label\": \"(c)\"",
            "\"curve_end\": True",
            "y = y + series[\"offset\"]",
            "\"offset\": 0.3",
            "\"color\": \"#4f2b7c\"",
            "\"color\": \"#d55e00\"",
            "\"linestyle\": \"--\"",
            "\"xlim\": (7100.5, 7103.5)",
            "\"ylim\": (-2.0, 2.0)",
            "\"grid\": False",
            "\"guides\": [7101.0]",
            "\"legend_location\": \"lower left\"",
            r#"r"$k^2\,\chi(k)$""#,
            "SKIPPED = [\"fit-q\"]",
            "print(f\"Skipped: {notice}\")",
            "subgridspec(2, 1, height_ratios=(3, 1)",
            "for extension in (\"pdf\", \"svg\", \"png\")",
            "dpi=DPI",
        ] {
            assert!(
                script.contains(expected),
                "missing {expected:?} in\n{script}"
            );
        }
        assert!(!script.contains("\"label\": \"(d)\""));
        assert!(script.contains("RECIPE_NAME = \"SI Figure \\\"S4\\\"\\n$\""));
        assert_eq!(
            files
                .iter()
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>(),
            [
                SCRIPT_NAME,
                "a-series-01.csv",
                "a-series-02.csv",
                "a-inset-series-01.csv",
                "a-inset-series-02.csv",
                "b-series-01.csv",
                "c-series-01.csv",
                "c-series-02.csv",
                "c-residual-series-01.csv",
            ]
        );
        // Retain an optional local smoke-test artifact without requiring Python
        // or its packages in the Rust test suite.
        if let Some(root) = std::env::var_os("REXAFS_COMPOSITE_PYTHON_EXPORT_DIR") {
            fs::create_dir_all(&root).unwrap();
            write_composite_python_export(
                "SI Figure S4",
                &recipe,
                &prepared,
                &Path::new(&root).join("journal-grid"),
            )
            .unwrap();
        }
    }

    #[test]
    fn csv_values_precede_main_and_inset_stacking_exactly() {
        let (recipe, prepared) = fixture();
        let files = composite_python_export("Figure 1", &recipe, &prepared).unwrap();
        assert_eq!(prepared.panels[0].data.series[0].y[0], 0.3);
        assert_eq!(
            prepared.panels[0].offsets[0].as_ref().unwrap().unshifted_y[0],
            1e-20
        );
        for name in ["a-series-01.csv", "a-inset-series-01.csv"] {
            let csv = &files.iter().find(|(file, _)| file == name).unwrap().1;
            let mut reader = csv::Reader::from_reader(csv.as_bytes());
            assert_eq!(reader.headers().unwrap().len(), 2);
            assert_eq!(
                &reader.headers().unwrap()[0],
                "A, foil $1\ncontinued: Energy (eV)"
            );
            let values: Vec<(f64, f64)> = reader.deserialize().map(Result::unwrap).collect();
            assert_eq!(
                values,
                [
                    (7100., 1e-20),
                    (7101., 1.),
                    (7102., 2.),
                    (7103., 1.),
                    (7104., 0.)
                ]
            );
        }
    }

    #[test]
    fn curve_end_text_matches_native_and_full_names_remain_editable() {
        use crate::publication::figures::composer::compose_prepared;

        let samples: Vec<_> = (1..=3)
            .map(|id| {
                let label =
                    format!("Synthetic {id:02} · Cu₂O cafe\u{301} 100.0% · CuO 0.0% · Cu 0.0%");
                CompositeSample {
                    id: GroupId::legacy_result(id),
                    label: label.clone(),
                    color: "#0072b2".into(),
                    figures: vec![Arc::new(figure(
                        "xanes",
                        &label,
                        vec![if id == 1 { 100. } else { id as f64 / 3. }; 5],
                    ))],
                }
            })
            .collect();
        let mut recipe = CompositeRecipe {
            samples: samples.iter().map(|sample| sample.id.clone()).collect(),
            panels: vec![
                CompositePanel {
                    label_mode: CurveLabelMode::CurveEnd,
                    .."xanes".into()
                },
                "xanes".into(),
            ],
            dpi: 100.,
            ..Default::default()
        };
        // Hide the last curve, clip the first one, and repeat the middle ID.
        // Only source indices 1 and 3 receive native direct labels.
        recipe.samples.push(samples[1].id.clone());
        let mut settings = FigureSettings::default();
        settings.figures.insert(
            "xanes".into(),
            FigureOptions {
                ymin: Some(0.),
                ymax: Some(2.),
                hidden: ["result:3/mu".into()].into(),
                ..Default::default()
            },
        );
        let empty: Vec<FigureData> = Vec::new();
        // The narrow grid exercises the available-margin cap as well as 35%.
        for width_mm in [177.8, 66.04] {
            recipe.width_mm = width_mm;
            recipe.panel_height_mm = Some(50.8);
            let prepared = prepare_panels(&recipe, &empty, &samples, &settings).unwrap();
            let files = composite_python_export("Long names", &recipe, &prepared).unwrap();
            let script = &files[0].1;
            let native = compose_prepared(&recipe, &prepared, None).unwrap();
            let records: Vec<serde_json::Value> = script
                .lines()
                .filter(|line| line.contains("\"file\": "))
                .map(|line| serde_json::from_str(line.trim().trim_end_matches(',')).unwrap())
                .collect();
            assert_eq!(records.len(), 6);
            assert_eq!(native.svg.matches("class=\"curve-end-label\"").count(), 2);
            for record in &records[..3] {
                let label = record["label"].as_str().unwrap();
                let full = record["full_label"].as_str().unwrap();
                assert!(samples.iter().any(|sample| sample.label == full));
                if full == samples[0].label {
                    assert_eq!(label, full); // Fully clipped; no native annotation.
                } else {
                    assert!(label.ends_with('…'), "{label}");
                    assert!(full.starts_with(label.trim_end_matches('…')));
                    assert!(native.svg.contains(&format!(">{label}</tspan>")));
                }
                let csv = &files
                    .iter()
                    .find(|(name, _)| name == record["file"].as_str().unwrap())
                    .unwrap()
                    .1;
                let mut reader = csv::Reader::from_reader(csv.as_bytes());
                assert_eq!(reader.headers().unwrap()[0], format!("{full}: Energy (eV)"));
            }
            // Legend entries keep complete names, including repeated IDs.
            for record in &records[3..] {
                assert!(record.get("full_label").is_none());
                assert!(samples.iter().any(|sample| sample.label == record["label"]));
            }
            assert!(script.contains("label=series.get(\"full_label\", series[\"label\"])"));
        }
    }

    #[test]
    fn difference_inset_exports_prepared_quantity_and_independent_offsets() {
        let (mut recipe, _) = fixture();
        recipe.panels[0].inset = Some(PanelInset {
            kind: InsetKind::Difference,
            follow_offset: false,
            energy_range: (-1., 3.),
            placement: InsetPlacement::CenterRight,
        });
        let first = figure("xanes", "A", vec![0., 1., 2., 1., 0.]);
        let second = figure("xanes", "B", vec![0.5, 1.5, 3., 1.5, 0.5]);
        let samples: Vec<_> = [first, second]
            .into_iter()
            .enumerate()
            .map(|(index, mut data)| {
                data.normalized_series = Some(data.series.clone());
                CompositeSample {
                    id: GroupId::legacy_result(index as u64 + 1),
                    label: data.label.clone(),
                    color: "#0072b2".into(),
                    figures: vec![Arc::new(data)],
                }
            })
            .collect();
        let empty: Vec<FigureData> = Vec::new();
        let prepared =
            prepare_panels(&recipe, &empty, &samples, &FigureSettings::default()).unwrap();
        let files = composite_python_export("Difference", &recipe, &prepared).unwrap();
        let script = &files[0].1;
        assert!(script.contains("\"quantity\": \"Δμ\""));
        assert!(script.contains("\"bounds\": (0.54, 0.32, 0.43, 0.36)"));
        assert!(script.contains("\"offset\": 0.3"));
        assert!(
            prepared.panels[0]
                .inset
                .as_ref()
                .unwrap()
                .offsets
                .iter()
                .all(Option::is_none)
        );
        let csv = &files
            .iter()
            .find(|(name, _)| name == "a-inset-series-02.csv")
            .unwrap()
            .1;
        let mut reader = csv::Reader::from_reader(csv.as_bytes());
        assert_eq!(&reader.headers().unwrap()[1], "B: Δμ");
        let values: Vec<(f64, f64)> = reader.deserialize().map(Result::unwrap).collect();
        assert_eq!(
            values.iter().map(|point| point.1).collect::<Vec<_>>(),
            [0.5, 0.5, 1., 0.5, 0.5]
        );
    }

    #[test]
    fn empty_and_hidden_panels_are_skipped_and_all_missing_is_an_error() {
        let (mut recipe, mut prepared) = fixture();
        prepared.panels[0]
            .options
            .hidden
            .extend(["result:1/mu".into(), "result:2/mu".into()]);
        prepared.panels[1].data.series[0].x.clear();
        prepared.panels[1].data.series[0].y.clear();
        let files = composite_python_export("Available fit", &recipe, &prepared).unwrap();
        assert!(files[0].1.contains("ROWS = 1\nCOLS = 1"));
        assert!(files[0].1.contains("xanes: no visible curves with data"));
        assert!(files[0].1.contains("chi-k: no visible curves with data"));
        assert!(files[0].1.contains("\"label\": \"(a)\""));
        assert!(!files[0].1.contains("\"label\": \"(b)\""));
        prepared.panels.clear();
        assert!(
            composite_python_export("Empty", &recipe, &prepared)
                .unwrap_err()
                .contains("Missing: fit-q")
        );
        recipe.width_mm = f64::NAN;
        assert!(composite_python_export("Invalid", &recipe, &fixture().1).is_err());
    }

    #[test]
    fn repeated_sample_ids_keep_independent_offsets_after_visibility_filtering() {
        let source = figure("xanes", "Repeated", vec![0., 1., 2., 1., 0.]);
        let sample = CompositeSample {
            id: GroupId::legacy_result(1),
            label: "Repeated".into(),
            color: "#0072b2".into(),
            figures: vec![Arc::new(source)],
        };
        let recipe = CompositeRecipe {
            panels: vec![CompositePanel {
                offset_step: 0.3,
                ..CompositePanel::from("xanes")
            }],
            samples: vec![sample.id.clone(), sample.id.clone()],
            ..Default::default()
        };
        let empty: Vec<FigureData> = Vec::new();
        let prepared =
            prepare_panels(&recipe, &empty, &[sample], &FigureSettings::default()).unwrap();
        let files = composite_python_export("Repeated", &recipe, &prepared).unwrap();
        let series: Vec<_> = files[0]
            .1
            .lines()
            .filter(|line| line.contains("\"file\": "))
            .collect();
        assert_eq!(series.len(), 2);
        assert!(series[0].contains("\"offset\": 0.0"));
        assert!(series[1].contains("\"offset\": 0.3"));
        assert_eq!(files[1].1, files[2].1);

        let (recipe, mut prepared) = fixture();
        prepared.panels[0]
            .options
            .hidden
            .insert("result:1/mu".into());
        let files = composite_python_export("Hidden first", &recipe, &prepared).unwrap();
        let first = files[0]
            .1
            .lines()
            .find(|line| line.contains("\"file\": "))
            .unwrap();
        assert!(first.contains("\"offset\": 0.0"));
        assert!(first.contains("#d55e00"));
    }

    #[test]
    fn invalid_data_fail_before_writing_and_existing_exports_are_preserved() {
        let temp = tempfile::tempdir().unwrap();
        let folder = temp.path().join("Figure 1");
        let (recipe, mut prepared) = fixture();
        prepared.panels[0].data.series[0].x.pop();
        assert!(write_composite_python_export("Figure 1", &recipe, &prepared, &folder).is_err());
        assert!(!folder.exists());
        let (_, prepared) = fixture();
        let path = write_composite_python_export("Figure 1", &recipe, &prepared, &folder).unwrap();
        fs::write(&path, "user edits").unwrap();
        assert!(write_composite_python_export("Figure 1", &recipe, &prepared, &folder).is_err());
        assert_eq!(fs::read_to_string(path).unwrap(), "user edits");
    }
}
