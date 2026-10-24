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
vector editor can select and retype them. Labels with italics, superscripts or
Typst math, including the default axis labels such as *E* (eV) and
*k*²*χ*(*k*) (Å⁻²), stay as glyph outlines (vector character shapes) so they
look the same in every viewer. The default labels are drawn in the figure's
sans-serif font (Arial by default) with italic variables; math you type between
dollar signs uses Typst's serif math font. Editable text uses the viewer's copy of
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

### Legend, ticks, R range and labels (since 0.2.15)

**Legend** places the legend: **Auto** (the default), **Upper left**, **Upper
right**, **Lower left**, **Lower right**, **Outside** or **Off**. Auto is
resolved by rexafs: it estimates the legend box from the legend font size, the
number of entries and the longest label, places it in each inside corner of the
visible axis range, and picks the corner that covers the smallest share of the
plotted curves (each curve is resampled along its line for this count). When
every corner covers more than 0.5 % of the curves, the legend goes above the
plot, which costs height rather than width so a long label cannot squeeze a
narrow figure.
The box size and the 0.5 % limit are rexafs estimates; ruviz's own "best"
placement, used by earlier versions, could put the legend in the middle of a
rising absorption edge. Implemented in
[`auto_legend`](https://github.com/Ameyanagi/rexafs/blob/dev/crates/rexafs-gui/src/publication/figures/layout.rs). **Outside**
puts the legend to the right of the plot and narrows the plot area. The choice
is saved with the figure settings and copied by **Apply style to all figures**.
Earlier versions always used Auto and offered only on/off.

At narrow widths rexafs lowers the number of x-axis ticks so neighbouring tick
labels do not overlap: it estimates each label's width from its digits and the
tick font size and keeps the densest ruviz tick spacing that leaves about one
character of space between labels. For example, a 3.3 in XANES figure with
9 pt text uses 20 eV instead of 10 eV spacing; the 6.4 in default keeps its
ticks. This is a rexafs estimate implemented in
[`x_tick_count`](https://github.com/Ameyanagi/rexafs/blob/dev/crates/rexafs-gui/src/publication/figures/layout.rs).

The processed-spectrum |χ(R)| figure no longer stops at a fixed 6 Å. It shows at
least 0–6 Å, extends to the back-transform R window's upper limit plus 0.5 Å
(rounded up to a whole ångström) when that is larger, and never extends past
the displayed transform range (10 Å by default). Fit |χ(R)| figures use the
same rule with the fit's R range. **X min** and **X max** override either default.

Titles and axis labels you type are shown literally: characters that Typst
would treat as markup, such as `#`, `_`, `*`, `[`, `]`, `@`, `<`, `>` and `\`,
appear as typed. Text between a pair of dollar signs is still Typst math, so
`Copper · $k^2 chi(k)$` works; type `\$` for a literal dollar sign. Earlier
versions passed typed labels to Typst unchanged, so such characters could be
reinterpreted or stop the figure from rendering.

### Fit + residual

After a fit, **More figures** includes **Fit + residual · χ(k)** and **Fit +
residual · |χ(R)|**. Each shows the data and fitted model in the upper panel
and the residual in a strip below (28% of the figure height, a rexafs choice).
Both panels share the x axis: they use the same x limits and margins, and only
the strip carries x tick labels and the x-axis title. The strip is symmetric
about zero, draws a zero line and uses three y ticks; when it is too short for the word, its axis title shortens to "Res.". The k-space residual is the k-weighted data
minus the fitted model, in the units of the upper panel. The R-space residual
is the difference of the magnitudes |χ(R)|, not the magnitude of the complex
residual. The figure's CSV includes the data, fit and residual curves. With
**Outside**, or when Auto finds no free corner, the legend takes the
least-covered inside corner so the panels stay aligned.

## Multi-panel figures (since 0.2.15)

**Multi-panel**, at the top of the Publish controls, composes several figures
into one grid of labelled panels, (a), (b), (c)…, for a journal figure. Panels
come from the figures available in **Single figure**: XANES, flattened μ(E),
χ(k), |χ(R)|, χ(q) and, after a fit, the fit, residual and fit + residual
figures (path contributions are curves you can show in the fit figures). The
initial recipe suggests XANES, χ(k), |χ(R)| and, when a fit exists, Fit + residual
· χ(k). **Add panel** appends a figure; the arrow moves a panel earlier and ×
removes it. Panels fill each row from left to right, at most 12 panels.

**Named recipes (unreleased).** The **Recipe** selector keeps several layouts
in the project, in the order they were created. Selecting one restores its panel
order, dimensions, font, line width and DPI.

- **New** appends and selects a default recipe, using the first unused name
  **Figure N** and the same suggested panels as the initial recipe.
- **Duplicate** appends and selects an independent copy of the current recipe.
  Its name gains ` copy`, then ` copy 2`, ` copy 3`, and so on until unique; a
  long original name is shortened to keep the result within 60 characters.
- **Rename** edits the name inline; **Save name** applies it and **Cancel** discards
  the draft. Names are trimmed, must contain 1–60 Unicode characters and must
  be unique when compared after Unicode lowercase conversion. Validation
  feedback appears beside the draft. Renaming preserves the selection and order.
- **Delete** asks for confirmation in the app, then selects the next recipe,
  or the previous one when deleting the last entry. The only remaining recipe
  cannot be deleted.

**Journal width** sets the total figure width to a publisher's column width.
The values are maximum widths from the publishers' author guides, checked in
October 2026; individual journals can differ, so check the target journal:

| Preset | Width |
| --- | --- |
| ACS · 1 column / 2 columns | 84.7 mm (240 pt) / 177.8 mm (504 pt), [JACS author guidelines](https://researcher-resources.acs.org/publish/author_guidelines?coden=jacsat), "Size" |
| RSC · 1 column / 2 columns | 83 mm / 171 mm, [RSC submission guidelines](https://rsc.org/globalassets/05-journals-books-databases/our-journals/natural-product-reports/npr-viewpoint-submission-guidelines.pdf) |
| Elsevier · 1 column / 2 columns | 90 mm / 190 mm, [Elsevier artwork sizing](https://www.elsevier.com/about/policies-and-standards/author/artwork-and-media-instructions/artwork-sizing) |

**Width (mm)** accepts any other width. **Columns** (1–4, 2 by default) sets
the panels per row; each panel is the width divided by the column count, rounded
down to whole pixels. **Panel height (mm)** is blank (Auto) for ¾ of the panel
width. Each panel must be at least 25.4 mm (1 in) on each side.

**Font size** (8 pt by default), **Line width** (blank keeps the renderer
default) and **DPI** (600 by default) apply to every panel, so all panels share
the same text and curve weights. Everything else inside a panel comes from that
figure's **Single figure** settings: axis labels, axis limits, visible curves,
flattened or normalized μ(E), legend, grid and guides. Titles are left out
because the panel labels identify the panels. Legends stay inside the plot (an
**Outside** or crowded **Auto** legend takes the least-covered inside corner),
because a legend outside one panel would shrink its plot area.

rexafs aligns the panels by giving every panel the same fixed margins: the
widest left margin any panel needs for its y tick labels and axis title, and
likewise for the other sides. The plot areas of a row therefore share their top
and bottom edges, and those of a column share their left and right edges. Each
panel is rendered separately by ruviz and the panels are tiled into one image
and nested into one SVG, the same technique as the fit + residual figure. The
bold panel labels sit at the top-left corner of each panel, at the axis-title
font size. Each recipe stores panels, width, columns, height, font, line width
and DPI; naming a recipe does not change how its panels render. **Single figure**
settings, including captions and visible curves, remain shared across recipes,
so changing them affects every recipe using that figure type.

The `.rxs` project saves named recipes as an ordered `publication.recipes` array
of `{name, recipe}` entries and the selection as the zero-based
`publication.active_recipe` index. An index beyond the saved list selects its
final recipe. An older `publication.composite` loads unchanged as **Figure 1**;
projects without recipe data start with a default **Figure 1**. Saving writes
only the new recipe format; the compact project encoder may omit default
fields. Storage and migration are implemented in
[`recipes.rs`](https://github.com/Ameyanagi/rexafs/blob/dev/crates/rexafs-gui/src/publication/figures/recipes.rs).

The preview is rendered at up to 200 DPI; **Fit** fits it to the pane and
**True size** shows it at 96 pixels per inch, close to its printed size (the
exact size depends on the display). **Save figure** renders at the chosen DPI:

- **PNG** and **TIFF** are raster images with the resolution recorded in the
  file; TIFF is RGB with lossless LZW compression.
- **SVG** is vector, with the printed size in inches (for example
  `width="7in"`); plain labels and the panel labels are editable text.
- **PDF** is vector, converted from the SVG by ruviz (svg2pdf); its page is the
  printed figure size.

With named recipes (unreleased), the suggested filename comes from the recipe
name, for example `figure-2.pdf`. It is lowercased, keeps Unicode letters and
digits, and replaces runs of punctuation or whitespace with a single hyphen,
without leading or trailing hyphens. An empty result uses `figure`; reserved
Windows device names gain a `figure-` prefix.

The caption below the preview joins each panel's caption after its label, for
example "(a) Flattened, edge-step-normalized X-ray absorption spectrum… (b) …";
**Copy caption** copies it. **Save caption…** (unreleased) saves it as plain text,
with a suggested filename such as `figure-2-caption.txt`. Panels whose figure
is unavailable, such as a fit panel before any fit has run, are skipped, listed
under the preview and kept in the recipe. Each panel shows one spectrum:
overlays of several samples with vertical offsets, an inset, per-curve colours
and a matplotlib script for the whole grid are not available yet. The composer
is implemented in
[`composer.rs`](https://github.com/Ameyanagi/rexafs/blob/dev/crates/rexafs-gui/src/publication/figures/composer.rs).

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
| `analysis.md` | Captioned analysis record: requested settings, source comments, current model, historical fit inputs, fit statistics tables, values, uncertainties, path distances and journal |
| `resolved.md` | Per-spectrum processing outputs recomputed at export time |
| `figures/*.png`, `figures/*.svg` | Spectra, fit overlays and residuals using saved dimensions/style, with captions in SVG metadata; default size/resolution when unset |
| `figures/*.csv` | Visible curve data for each exported figure |
| `report.html`, `captions.md` | Browser-printable vector figures with retained aspect ratios, semantic tables, numbered captions, units and uncertainty definitions; for each fit, a statistics table (R-factor, χ², reduced χ² when noise was estimated, N_idp / N_var, fit space, k and R ranges, k-weights, windows and correlations with \|r\| ≥ 0.10); manuscript caption text |
| `data/*.json` | Processed arrays and available full fit results |
| `methods.md` | Methods draft generated from the values actually used: E₀, pre-edge and normalization ranges and polynomial order (or MBACK), background method with R_bkg, k range and k-weight, transform k range, k-weight and window, and for each fit its space, k/R ranges, k-weights, windows, FEFF engine (ReFEFF or FEFF10), varied parameters and N_idp. Steps that were not run are omitted; only cited works are listed. The Experiment section is left for you to complete |
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

The bundled `project.rxs` is always self-contained: it embeds the original
spectra and referenced FEFF inputs with their SHA-256 checksums, as
**Raw: embedded** does, even when the open project links to its sources by
path (the open project and **Publish report…** keep their own setting). The
archive therefore opens in rexafs on another computer without the source
folders. If a source file cannot be read, or the embedded inputs would exceed
the 1 GiB limit, the export stops with an error instead of writing a project
with broken links. The archive is written next to its destination and moved into
place only when complete. See the
[bundle exporter](https://github.com/Ameyanagi/rexafs/blob/dev/crates/rexafs-gui/src/publication/supporting.rs).

[![Full publication editor showing a vector Cu fit figure and output controls](/screenshots/0.2.11/publication.jpg)](/screenshots/0.2.11/publication.jpg)

*Publication editor. Full window, rexafs 0.2.11 on macOS; select to enlarge.*
