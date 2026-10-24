//! Export the prepared active recipe without recomputing samples or insets.
use super::*;
use crate::publication::figures::composer::{
    CompositeRecipe, CurveLabelMode, DisplayOffset, InsetPlacement, PreparedComposition,
    panel_label, validate_composition_layout,
};

/// Write the named recipe's available panels into a new folder. CSV ordinates
/// precede display stacking; the script adds editable offsets in axis units.
/// Prepared data must belong to this recipe and its current figure settings.
/// Invalid settings, complete curve-end labels that do not fit the native
/// layout, or no visible data fail before creating the directory. Existing
/// destinations are rejected. Returns the generated script path.
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
    validate_composition_layout(recipe, &panels, &grid)?;
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
        let letter = (b'a' + index as u8) as char;
        let series = series_entries(
            &format!("{letter}-series"),
            data,
            &options,
            &panel.offsets,
            &mut files,
        )?;
        let inset = if let Some(inset) = &panel.inset {
            let inset_series = series_entries(
                &format!("{letter}-inset-series"),
                &inset.data,
                &inset.options,
                &inset.offsets,
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
                        grid: Some(false),
                        legend: false,
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
        r##"r"""Reproduce a composed rexafs figure with Python 3, numpy and matplotlib.

Generated by rexafs {version}. Run: python make_figure.py
CSV files next to this script contain complete, unshifted x/y arrays with one
CSV header record. Insets retain their prepared scientific quantity, including Δμ
for differences. Display offsets are added only by this script.

Edit the figure settings and PANELS below, then run again to save figure.pdf,
figure.svg and figure.png next to this script. Size is in millimetres; font
size and line width are in points. Each series offset is in its own y units;
main and inset offsets are independent editable constants. Axis limits set the
view only. Labels use matplotlib mathtext ($...$), as in the single-figure
export. Complete figure labels, resolved colours, grid and legend visibility
match the prepared composition. Ticks point inward on all four sides with
minor ticks; legends are frameless and one point smaller than the figure font.
Matplotlib chooses tick positions and margins, so this is an editable
reproduction rather than a pixel copy of the preview. Curve-end labels are
validated against the native layout before export and are never shortened.
"""
from pathlib import Path
import csv

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib.ticker import AutoMinorLocator, NullLocator
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

# Composed-figure styling. Five subdivisions give four minor ticks per interval.
TICK_DIRECTION = "in"
TICKS_TOP = True
TICKS_RIGHT = True
MINOR_SUBDIVISIONS = 5
LEGEND_FONT_SIZE = FONT_SIZE - 1
LEGEND_FRAME = False
LEGEND_LOCATION = "best"
LEGEND_HANDLE_LENGTH = 1.4
LEGEND_LABEL_SPACING = 0.3
LEGEND_HANDLE_TEXT_PAD = 0.5
GUIDE_COLOR = "#8c8c8c"
GUIDE_WIDTH = 0.8
PANEL_LABEL_SIZE = 1.1 * FONT_SIZE
PANEL_LABEL_Y = 1.04
CURVE_LABEL_GAP_PT = 2
CURVE_LABEL_PAD_PT = 4
CURVE_LABEL_LINE_HEIGHT = 1.2
RESIDUAL_HEIGHT_RATIOS = (3, 1)
RESIDUAL_GAP = 0.08

# Insets retain a white quantity tag and headroom above their curves.
INSET_LINE_SCALE = 0.7
INSET_FONT_SIZE = max(4, 0.8 * FONT_SIZE)
INSET_HEADROOM = 0.2
INSET_TAG_POSITION = (0.04, 0.96)
INSET_TAG_BBOX = dict(facecolor="white", edgecolor="none", pad=1)
INSET_TICK_BBOX = dict(facecolor="white", edgecolor="none", pad=0.5)
SKIPPED = [{notices}]

# Per-panel settings. Each CSV is unshifted; change "offset" to adjust stacking.
# Each "label" is the complete figure label used by native curves and legends.
# CSV headers retain these same complete labels; names are never shortened.
# Inset bounds are (left, bottom, width, height) fractions of the main axes.
# Limits are (min, max), or None for matplotlib's automatic range.
PANELS = [
{entries}]

{style}
{load_series}
{draw}
"##,
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
        entries.push_str(&format!(
            "            {{\"file\": {}, \"label\": {}, \"color\": {}, \"linestyle\": {}, \"offset\": {:?}}},\n",
            py_str(&file), py_str(&plain_text(&curve.label)), py_str(&series_color(curve, options, index)),
            py_str(if curve.dashed { "--" } else { "-" }), shift.map_or(0., |shift| shift.value),
        ));
    }
    Ok(entries)
}

/// Axis settings use the same labels and visible ranges as a single export.
fn axes_settings(data: &FigureData, options: &FigureOptions) -> String {
    let (xlabel, ylabel) = axis_labels(data, options);
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
        "\"xlabel\": {},\n        \"ylabel\": {},\n        \"xlim\": {},\n        \"ylim\": {},\n        \"ylim_fixed\": {},\n        \"grid\": {},\n        \"legend\": {},\n        \"guides\": [{guides}]",
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
        py_bool(options.grid.unwrap_or(false)),
        py_bool(options.legend),
    )
}

/// Python rendering keeps labels and endpoints responsive to edited offsets.
const DRAW: &str = r##"
def draw_curves(ax, settings, width):
    curves = []
    for series in settings["series"]:
        x, y = load_series(series["file"])
        y = y + series["offset"]
        ax.plot(x, y, label=series["label"], color=series["color"],
                linestyle=series["linestyle"], linewidth=width)
        curves.append((x, y, series))
    for guide in settings["guides"]:
        ax.axvline(guide, color=GUIDE_COLOR, linewidth=GUIDE_WIDTH, linestyle="--")
    ax.set_xlabel(settings["xlabel"])
    ax.set_ylabel(settings["ylabel"])
    if settings["xlim"] is not None:
        ax.set_xlim(*settings["xlim"])
    if settings["ylim"] is not None:
        low, high = settings["ylim"]
        if not settings["ylim_fixed"] and curves:
            # Exported automatic limits describe the prepared display; widen
            # them when edited offsets move curves beyond that original view.
            xlow, xhigh = ax.get_xlim()
            drawn = np.concatenate([y[(x >= xlow) & (x <= xhigh)] for x, y, _ in curves])
            drawn = drawn[np.isfinite(drawn)]
            if drawn.size and (drawn.min() < low or drawn.max() > high):
                low, high = min(low, drawn.min()), max(high, drawn.max())
                pad = 0.05 * (high - low)
                low, high = low - pad, high + pad
        ax.set_ylim(low, high)
    ax.grid(settings["grid"])
    ax.tick_params(direction=TICK_DIRECTION, top=TICKS_TOP, right=TICKS_RIGHT, which="both")
    ax.xaxis.set_minor_locator(AutoMinorLocator(MINOR_SUBDIVISIONS))
    ax.yaxis.set_minor_locator(AutoMinorLocator(MINOR_SUBDIVISIONS))
    if settings["legend"]:
        ax.legend(loc=LEGEND_LOCATION, frameon=LEGEND_FRAME, fontsize=LEGEND_FONT_SIZE,
                  handlelength=LEGEND_HANDLE_LENGTH, labelspacing=LEGEND_LABEL_SPACING,
                  handletextpad=LEGEND_HANDLE_TEXT_PAD)
    return curves


def curve_end_labels(ax, curves):
    """Label the rightmost visible point, interpolating at clipped boundaries."""
    xmin, xmax = ax.get_xlim()
    ymin, ymax = ax.get_ylim()
    labels = []
    height_pt = ax.get_window_extent().height / ax.figure.dpi * 72
    gap = CURVE_LABEL_GAP_PT / height_pt
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
            height = CURVE_LABEL_LINE_HEIGHT * FONT_SIZE * (series["label"].count("\n") + 1) / height_pt
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
                    xytext=(CURVE_LABEL_PAD_PT, 0), textcoords="offset points", va="center",
                    color=series["color"], fontsize=FONT_SIZE, annotation_clip=False)


fig, axes = plt.subplots(ROWS, COLS, figsize=(WIDTH_MM / 25.4, HEIGHT_MM / 25.4),
                         squeeze=False, layout="constrained")
end_labels = []
inset_axes = []
for ax, panel in zip(axes.flat, PANELS):
    residual = panel["residual"]
    if residual is not None:
        nested = ax.get_subplotspec().subgridspec(2, 1, height_ratios=RESIDUAL_HEIGHT_RATIOS, hspace=RESIDUAL_GAP)
        ax.remove()
        ax = fig.add_subplot(nested[0])
        strip = fig.add_subplot(nested[1], sharex=ax)
        draw_curves(strip, residual, LINE_WIDTH)
        strip.axhline(0, color=GUIDE_COLOR, linewidth=GUIDE_WIDTH)
    curves = draw_curves(ax, panel, LINE_WIDTH)
    if residual is not None:
        ax.set_xlabel("")
        ax.tick_params(labelbottom=False)
    ax.text(0, PANEL_LABEL_Y, panel["label"], transform=ax.transAxes, fontweight="bold",
            fontsize=PANEL_LABEL_SIZE, va="bottom")
    if panel["curve_end"]:
        end_labels.append((ax, curves))
    inset = panel["inset"]
    if inset is not None:
        inset_ax = ax.inset_axes(inset["bounds"])
        draw_curves(inset_ax, inset, INSET_LINE_SCALE * LINE_WIDTH)
        # Headroom keeps the quantity tag clear of the curves, as in rexafs.
        low, high = inset_ax.get_ylim()
        inset_ax.set_ylim(low, high + INSET_HEADROOM * (high - low))
        inset_ax.tick_params(labelsize=INSET_FONT_SIZE)
        inset_ax.set_yticks([])
        inset_ax.yaxis.set_minor_locator(NullLocator())
        inset_ax.text(*INSET_TAG_POSITION, inset["quantity"], transform=inset_ax.transAxes,
                      va="top", fontsize=INSET_FONT_SIZE,
                      bbox=INSET_TAG_BBOX)
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
        tick.set_bbox(INSET_TICK_BBOX)
for notice in SKIPPED:
    print(f"Skipped: {notice}")
for extension in ("pdf", "svg", "png"):
    fig.savefig(HERE / f"figure.{extension}", dpi=DPI)
print(f"Saved figure.pdf, figure.svg and figure.png in {HERE}")
"##;

#[cfg(test)]
mod tests;
