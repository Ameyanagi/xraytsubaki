---
title: "Publication and exports"
description: "Create figures, captions, data tables and reproducible analysis exports."
audience: user
---

Open **Publish** after processing or fitting. The header names the report
scope. Since 0.2.12, **Publish report…** writes the complete analysis folder listed
below, then **Open last
published report** opens `report.html` and **Show folder** reveals the directory.
**Copy analysis record** copies the Markdown record. Since 0.2.15 it processes the included spectra first, so the **Used** column
of each processing table gives the values processing applied: resolved Auto
values, and fit ranges limited to the measured data on a short scan. Earlier
versions left that column as "Unavailable" in the copied record.

The header records when the last report was published.
Changing figure settings, captions, report selection, processing or fit inputs
shows **Changes since last publish**. **Publish changes…** exports the current
report to a new folder; the previous folder remains available through **Open last
published report** until another export succeeds. Edits made during an export
remain unpublished. These controls are implemented in the
[Publish stage](https://github.com/Ameyanagi/rexafs/blob/v0.2.13/crates/rexafs-gui/src/app/shell/publish.rs).

Select a figure, adjust its style, and use **Save figure** **PNG**, **SVG** or
**CSV** beside the preview to save that figure alone. The preview preserves the
aspect ratio and uses the same PNG bytes as the individual save.

**CSV** saves an x/y column pair for each visible curve with data, including
its name and axis label. Curves keep their own grids and full arrays; blank
cells pad shorter curves. Plot limits do not crop the CSV, and hidden curves
are omitted. At least one visible curve with data is required. See the [figure CSV
exporter](https://github.com/Ameyanagi/rexafs/blob/v0.2.4/crates/rexafs-gui/src/publication/figures.rs#L277).
The separate batch-results table contains fitted values and reported errors.

## Figure size and content

Three compact icons represent **Single column**,
**Double column** and **Slide**; hover to see the preset name and dimensions.
The selected preset remains highlighted, and each icon retains an accessible
name. These presets set size,
resolution and font (rexafs choices, not journal specifications), and **Apply
style to all figures** copies the selected figure's style to every figure type.
That action sits beside the presets. The scope label
explains that edits apply to the selected figure type across the report's spectra.
Copying style preserves each figure's axis limits, labels, captions and curve
visibility; see [FigureSettings](https://github.com/Ameyanagi/rexafs/blob/v0.2.13/crates/rexafs-gui/src/publication/figures.rs).

[![Publication editor with three compact preset icons, per-figure saves and Publish report](/screenshots/0.2.12/publication.jpg)](/screenshots/0.2.12/publication.jpg)

*Full unedited signed 0.2.12 window using public room-temperature Cu data.
The double-column preset sets 7 × 4.8 inches at 300 DPI. Changing the grid after
export correctly marks the report as changed.
[Build and input provenance](/licenses/#desktop-0212-release-captures).*

Blank size controls use ruviz's 6.4 × 4.8 inch canvas. rexafs explicitly sets the
default raster resolution to **300 DPI**, producing **1920 × 1440 pixels**.
Width and height use inches; font and line widths use points. Set DPI to the
journal's requested resolution, or use SVG for scalable line art. Resizing the
preview does not alter export dimensions. Reset restores the figure's defaults.

Sizes must be 1–30 inches on each side; DPI must be an integer from 72 to 1200.
The image must contain at most 25 million pixels. Width in pixels is width in
inches multiplied by DPI, and likewise for height. These limits and defaults
come from [FigureOptions](https://github.com/Ameyanagi/rexafs/blob/v0.2.4/crates/rexafs-gui/src/publication/figures.rs#L82).

Controls include labels/title, paired axis limits, legend, grid, processing guides
and visible curves. Fit figures show model/data components and path contributions
without vertical offsets. R-space residuals show the difference of magnitudes,
not a complex residual. Scientific curves come from processing/fit results.

## Captions

Enter figure and processing/parameter/path-table captions in the editor; press
**Enter** to apply. Blank captions describe the selected curves or table
definitions. **Copy caption** copies the current figure's caption. Settings and
captions are saved in `.rxs` and apply to the same figure/table type across a
multi-spectrum export. Add sample-specific details to individual captions in
the exported report.

Supply sample preparation, temperature, beamline conditions and conclusions
from the experimental record. Standard errors represent fit covariance, not all
experimental/model uncertainty; R-space coordinates are not phase corrected.
Review missing arrays, stale-fit notices, scientific validity and the journal's
requirements before using an export in a paper.

## Analysis folder contents

**Publish report…** (0.2.11: **Analysis folder** then **Export…**) exports to a
new directory:

| Asset | Contents |
|---|---|
| `analysis.md` | Captioned analysis record: requested settings, source comments, current model, historical fit inputs, values, uncertainties, path distances and journal |
| `resolved.md` | Per-spectrum processing outputs recomputed at export time |
| `figures/*.png`, `figures/*.svg` | Spectra, fit overlays and residuals using saved dimensions/style, with captions in SVG metadata; default size/resolution when unset |
| `figures/*.csv` | Visible curve data for each exported figure |
| `report.html`, `captions.md` | Browser-printable vector figures with retained aspect ratios, semantic tables, numbered captions, units and uncertainty definitions; manuscript caption text |
| `data/*.json` | Processed arrays and available full fit results |
| `methods.md` | Editable methods draft with missing experimental details identified |
| `references.md`, `references.bib` | Algorithm references and reminders to cite the actual data, structures and FEFF backend |
| `state.json`, `project.rxs` | Structured analysis context and project |
| `batch-results.csv` | Batch results when available; the manifest flags stale results |
| `README.md`, `manifest.json` | Figure index, figure/table numbering, captions, presentation settings, source/fit associations and unavailable or stale results |

The export covers the current spectrum, marked spectra, assigned fit spectra
and recorded results. **Copy Markdown** copies the analysis record without
exporting figures. These files can also supply context to an external language model.

Existing destinations are never overwritten. The project uses the selected
[raw-data mode](/docs/desktop/projects/#raw-data-paths-or-embedded-originals):
relative links by default, or compressed original spectra and referenced FEFF
inputs with **Raw: embedded**. Its header records sources and checksums.
Processed arrays are included in full. Archived fit statistics remain exportable
when plot arrays are unavailable; the manifest reports missing figures. Auto
requests and historical settings are distinguished from current, resolved values.
Complete the methods draft from your experimental record.

Review `manifest.json`, `README.md` and `report.html` for failed spectra, missing
historical curves or failed figures. A filesystem failure can stop the export
with a partial directory and no manifest; resolve the cause and retry into a
new directory. Folder exports are written file by file and lack the atomic
replacement/backup behavior of `.rxs` saves. See the [folder
exporter](https://github.com/Ameyanagi/rexafs/blob/v0.2.4/crates/rexafs-gui/src/publication.rs#L248).


[![Full publication editor showing a vector Cu fit figure and output controls](/screenshots/0.2.11/publication.jpg)](/screenshots/0.2.11/publication.jpg)

*Publication editor. Full window, rexafs 0.2.11 on macOS; select to enlarge.*
