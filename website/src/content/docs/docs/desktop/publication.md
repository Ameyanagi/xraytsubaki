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

SVG files keep plain labels (tick numbers, legend names, and titles or axis
labels you type without math, such as `Energy (eV)`) as editable `<text>`, so a
vector editor can select and retype them. Labels with Typst math, including the
default axis labels such as `$E$ (eV)` and `$k^2 chi(k)$`, stay as glyph
outlines (vector character shapes) so subscripts, superscripts and Greek
letters look the same in every viewer. Editable text uses the viewer's copy of
the figure font (Arial by default); a viewer without it substitutes a similar
font. See the
[SVG text option](https://github.com/Ameyanagi/rexafs/blob/dev/crates/rexafs-gui/src/publication/figures.rs).

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

## Fit parameter table

Since 0.2.15, **Export fit table…** in the Publish header saves the per-path
parameters of the latest recorded fit in three files with the name you choose:
`.tex` (a LaTeX `table` using the booktabs package; add
`\usepackage{booktabs}` to the preamble), `.csv` and `.docx` (a Word table that
Word, LibreOffice and Pages open). Each row is one scattering path; the columns
are the coordination number N, the amplitude reduction factor S₀², the energy
shift ΔE₀ (eV), the distance R = R_eff + ΔR (Å; half the total path length for
multiple scattering) and the mean-square relative displacement σ² (Å², the
Debye–Waller parameter). The caption gives the fit number, group, R factor,
reduced χ², the number of independent points N_idp and of varied parameters
N_vary.

Uncertainties are one standard error from the fit covariance, propagated
through path expressions; they do not include all experimental or model
uncertainty. The tables round them for reading:

1. The uncertainty is rounded to one significant figure, or two when its
   leading digit is 1: 0.00292 → 0.003, 0.0435 → 0.04, 0.0149 → 0.015.
   A carry to a power of ten keeps two figures (0.0096 → 0.010).
2. The value is rounded to the same decimal place, for example
   2.5434 ± 0.0149 → 2.543 ± 0.015.

This is the convention of J. R. Taylor, *An Introduction to Error Analysis*,
2nd ed. (University Science Books, 1997), §2.2, and is consistent with the
BIPM *Guide to the Expression of Uncertainty in Measurement* (JCGM 100:2008,
§7.2.6, [doi:10.59161/JCGM100-2008E](https://doi.org/10.59161/JCGM100-2008E)),
which asks for at most two significant digits in an uncertainty and a value
rounded consistently with it. Parameters held constant are written
"12 (fixed)" rather than "± 0". A value without a usable uncertainty (not
estimated, zero or not finite) is given to four significant figures with a
dagger (†) and a note. The CSV has one row per path and parameter, with the
unrounded value and uncertainty (17 significant digits), the rounded pair and a
status (`fitted`, `fixed`, `no_uncertainty` or `unavailable`). See the
[table formatter](https://github.com/Ameyanagi/rexafs/blob/dev/crates/rexafs-gui/src/publication/fit_table.rs).

## Supporting-information bundle

Since 0.2.15, **Export supporting information…** saves one `.zip` for a
journal's supporting information. It contains the complete analysis folder of
**Publish report…** (report, figures as PNG/SVG/CSV, processed JSON arrays,
`project.rxs`, captions, methods and references) and adds:

- `data/csv/`: plain CSV columns for every included spectrum (energy, μ(E),
  normalized and flattened μ(E); k, χ(k), k-weighted χ(k); R, |χ(R)|, Re and Im
  χ(R)) and for every completed fit (measured and model χ(k), k-weighted, the k
  window, and measured and model |χ(R)|);
- `tables/`: the fit parameter table of every recorded fit, as LaTeX, CSV and
  Word;
- `SUPPORTING-INFORMATION.md`: a list of the contents;
- `manifest-sha256.txt`: the SHA-256 checksum of every other file. In the
  unpacked folder, `shasum -a 256 -c manifest-sha256.txt` (macOS) or
  `sha256sum -c manifest-sha256.txt` (Linux) verifies that no file changed.

Select **Raw: embedded** before exporting if the project file must contain the
original spectra. The archive is written next to its destination and moved into
place only when complete. See the
[bundle exporter](https://github.com/Ameyanagi/rexafs/blob/dev/crates/rexafs-gui/src/publication/supporting.rs).

[![Full publication editor showing a vector Cu fit figure and output controls](/screenshots/0.2.11/publication.jpg)](/screenshots/0.2.11/publication.jpg)

*Publication editor. Full window, rexafs 0.2.11 on macOS; select to enlarge.*
