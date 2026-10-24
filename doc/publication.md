# Preparing publication output

Open **Publish** after processing a spectrum or completing a fit. The header
names the report scope (the current group, the marked groups and the recorded
fits). **Publish report…** (since 0.2.12; the 0.2.11 control is a format selector
with **Export…**) writes the complete analysis folder described in
[publication export](publication-export.md): `report.html`, the analysis record,
every figure as PNG, SVG and CSV, tables, captions, resolved data and the
project. After publishing, **Open report** opens `report.html` and **Show
folder** reveals the directory. **Copy analysis record** copies the Markdown
record to the clipboard. Since 0.2.15 it processes the included spectra first, so the **Used** column
of each processing table gives the values processing applied: resolved Auto
values, and fit ranges limited to the measured data on a short scan. Earlier
versions left that column as "Unavailable" in the copied record.

Select a figure in the left column, adjust its style, and use **Save figure**
**PNG**, **SVG** or **CSV** beside the preview to save that figure alone. The
preview preserves the image's aspect ratio and uses the same rendered PNG bytes
as the PNG save. CSV exports one x/y column pair for each selected visible
curve, including its name and axis label. Different curves keep their own grids and full arrays;
blank cells pad shorter curves. Changing the plot limits does not crop the CSV,
and hidden curves are omitted. At least one visible curve with data is required.
This behavior is defined by the [figure CSV
exporter](https://github.com/Ameyanagi/rexafs/blob/v0.2.4/crates/rexafs-gui/src/publication/figures.rs#L277).
It is separate from the batch-results table, which contains fitted values and
their reported errors.

**Data + Python script…** (since 0.2.15) finishes a figure outside rexafs. It
asks for a new folder name (an existing folder is rejected) and writes one
`series-NN.csv` per visible curve (two columns, x and y, with a one-line
header and the full arrays) plus `make_figure.py`. The script redraws the figure
with [matplotlib](https://matplotlib.org/) using the configured size in inches,
DPI, font size, line width, curve colours and line styles, title, axis labels,
axis limits, legend, grid and processing guides. All settings sit in a short
block at the top of the script, with a header comment explaining how to edit
them. Run it with any Python 3 that has numpy and matplotlib, for example
`uv run --with numpy --with matplotlib python make_figure.py`; it writes
`figure.pdf`, `figure.svg` and `figure.png` next to itself.

Axis labels are converted from Typst to matplotlib
[mathtext](https://matplotlib.org/stable/users/explain/text/mathtext.html), its
TeX-like math syntax: the built-in label `_k_#super[2]_χ_(_k_) (Å#super[−2])`
becomes `$k^{2}\chi$($k$) (Å$^{-2}$)`, and math you type, such as
`$k^(2) chi(k)$`, becomes `$k^{2}\,\chi(k)$`. The conversion covers the markup
rexafs writes (italics, `#super[…]`, and in math superscripts, subscripts, Greek
letter names, `abs(…)`, `op("…")` and quoted text); a typed label using other
Typst math falls back to the plain Unicode label. Typed text outside `$…$` and
curve names are written as plain text, so a `$` in them is escaped as `\$`. The
script sets matplotlib's `mathtext.fontset` to `custom` with the sans-serif
font, so math matches the text instead of switching to serif Computer Modern.
The result is a close, editable reproduction rather than a pixel copy:
matplotlib chooses its own tick positions, margins and fonts (Arial, then
Helvetica, then DejaVu Sans). The generator is
[`python_export.rs`](https://github.com/Ameyanagi/rexafs/blob/dev/crates/rexafs-gui/src/publication/python_export.rs).

The **Single column** (3.5 × 2.6 in, 600 DPI, 8 pt), **Double column** (7.0 ×
4.8 in, 300 DPI, 9 pt) and **Slide** (10 × 5.6 in, 200 DPI, 14 pt) presets set
the size, resolution and font of the selected figure; they are rexafs choices,
not journal specifications, so check the target journal's figure guide.
**Apply style to all figures** copies the selected figure's size, resolution,
font, line width, legend, grid and guide settings to every figure type.
Blank size controls use ruviz's 6.4 × 4.8 inch canvas. rexafs explicitly sets the
default raster resolution to **300 DPI**, producing **1920 × 1440 pixels**.
Width and height use inches; font and line widths use points. Set DPI to the
journal's requested raster resolution, or use the vector SVG for scalable line art. Changing the preview
window does not alter export dimensions. Reset restores the selected figure's
defaults.

SVG files record the figure's physical size in inches, for example
`width="3.5in" height="2.6in"` for **Single column**, so vector editors, word
processors and browsers place them at the chosen size. Drawing coordinates stay
in output pixels (inches × DPI) through the SVG `viewBox`, so DPI does not change
the vector drawing's printed size.

All labels are laid out by [Typst](https://typst.app/docs/), a typesetting
engine. Labels without math or markup, such as tick numbers, legend names and
titles or axis labels you type (for example `Energy (eV)`), are written as
editable SVG `<text>` elements at the positions Typst computed, so vector
editors and browsers can select, search and retype them. Labels containing
markup or math stay as glyph outlines (vector shapes of each character) so
italics, superscripts and Greek letters render identically everywhere. Each
outlined label carries an `aria-label`: the plain Unicode text, such as `k²χ(k)
(Å⁻²)`, for built-in labels, and the Typst source for typed math. Editable text
is drawn by the viewer, so it matches the preview exactly only when the viewer
has the figure font (Arial by default) installed; otherwise the viewer
substitutes a similar font. PNG output is unaffected. This uses ruviz's
`Plot::svg_native_text` option, enabled in
[`FigureData::plot`](https://github.com/Ameyanagi/rexafs/blob/dev/crates/rexafs-gui/src/publication/figures.rs).

Every label uses the figure's sans-serif font (Arial by default) (since 0.2.15).
Typst always draws math (`$…$`) in its serif New Computer Modern Math font, and
no sans-serif math font is bundled, so the built-in axis labels are written as
Typst text markup instead: `_E_ (eV)`, `_k_ (Å#super[−1])`,
`_k_#super[2]_χ_(_k_) (Å#super[−2])`, `|_χ_(_R_)| (Å#super[−3])` and `Normalized
_μ_(_E_)`, where `_…_` sets italics and `#super[…]` a superscript with a true
minus sign (−, U+2212). Variables are italic and units upright, as ISO 80000-1
and the IUPAC Green Book (Cohen et al., *Quantities, Units and Symbols in
Physical Chemistry*, 3rd ed., RSC 2007) recommend. Earlier versions wrote these
labels as math, so they rendered in a serif font. Math you type between dollar
signs is still Typst math and still serif. The label builders are in
[`figures/labels.rs`](https://github.com/Ameyanagi/rexafs/blob/dev/crates/rexafs-gui/src/publication/figures/labels.rs).

Sizes must be 1–30 inches on each side; DPI must be an integer from 72 to 1200.
The image must contain at most 25 million pixels. Width in pixels is width in
inches multiplied by DPI, and likewise for height. These limits and defaults
come from [FigureOptions](https://github.com/Ameyanagi/rexafs/blob/v0.2.4/crates/rexafs-gui/src/publication/figures.rs#L82).

Controls include labels/title, paired axis limits, legend, grid, processing guides
and individual visible curves. Fit figures expose model/data components and path
contributions without vertical offsets. R-space residuals explicitly represent
the difference of magnitudes, rather than a complex residual. All scientific
curves come from the processing/fit results; branding images are separate assets.

## Curve colours (unreleased)

In **Single figure**, click the small swatch beside a curve's visibility control
under **Curves**, then choose one of eight colours or **Auto**. The A swatch
means Auto; it clears that curve's override and restores its automatic colour.
Fit + residual figures also expose a swatch for the residual strip. The fixed
palette is black, orange, sky blue, bluish green, yellow, blue, vermillion and
reddish purple, from Okabe and Ito (2008),
["Color Universal Design"](https://jfly.uni-koeln.de/color/). Hover a swatch for
its colour name and sRGB hexadecimal value.

Colours are saved per figure type in the `.rxs` project and used by previews,
image exports, report and supporting-information figures, and the editable
matplotlib export. The matplotlib script currently includes only the upper
panel of a fit + residual figure; standalone residual figures export their
curve. **Apply style to all figures** preserves each figure's curve colours. Figures without overrides retain their existing automatic colours.
The saved `series_colors` map uses curve keys as plain strings and lowercase
`#rrggbb` sRGB values; malformed values stop rendering/export with an error,
and entries for curves that are no longer available have no visual effect.
Implemented in
[`FigureOptions` and `FigureData::plot`](https://github.com/Ameyanagi/rexafs/blob/dev/crates/rexafs-gui/src/publication/figures.rs).

## Legend, ticks, R range and labels (since 0.2.15)

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

## Fit + residual

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

**Samples and stacking (unreleased).** **Samples** offers the current group
first, then the marked groups in workspace order. Check only the samples to
include. **All** selects the offered groups in that order; **None** clears the
list. An empty list means the current group only, preserving older recipes.
Checking a sample appends its durable group ID to the saved list. The recipe
keeps that order when marks or the current group change; renaming a group updates
its label when the figure is rendered.

Select a panel row to edit its sample and stacking settings. **Use these samples
in every panel** is on by default: the panel inherits the recipe's **Samples**
list. Turn it off to copy that list into a separate selection for this panel.
Re-enabling it restores the shared selection. Saved samples remain selected even
when they are no longer current or marked. Removed samples are skipped and listed
as missing. rexafs queues processing for available selected groups with no cached
result. If a group still has no data, select it in the workspace, inspect its
processing error and process it successfully before exporting.

With one sample, each panel keeps its existing curves, helper lines and curve
keys. With several selected samples, each contributes only the main curve for
the figure; background, pre-edge, post-edge and other helper curves are omitted.
Choose samples with matching plotted quantities and k weights: a panel shares the
first available sample's axis labels and does not convert other samples' units.
Fit, residual and fit + residual panels always show one fitted dataset and ignore
the sample selection and stacking settings. **Single figure** stays single-sample.

**Offset step (y units)** is a display displacement between successive samples:
zero overlays the original curves. A step of 0.3 adds 0, 0.3, 0.6, and so on to
the samples' vertical values in saved order. **Reverse order** assigns these
offsets in reverse, putting the first sample highest for a positive step.
Negative steps move successive curves downward. Missing samples keep their
positions in the saved order. Offsets use the panel's displayed vertical-axis
units, including any k weighting; they modify copied display arrays only, leaving
processed spectra and fit results unchanged. The combined caption records a
non-zero applied offset. **Curve labels** defaults to **Legend**; **Curve ends** places
literal curve names at the right side of the plot with space reserved across the
grid. If names cannot fit at the chosen width, use a wider figure or the legend.
These display choices are implemented in
[`prepare_panels` and `compose_samples`](https://github.com/Ameyanagi/rexafs/blob/dev/crates/rexafs-gui/src/publication/figures/composer.rs).

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
curve colours, flattened or normalized μ(E), legend, grid and guides. Titles are left out
because the panel labels identify the panels. Legends stay inside the plot (an
**Outside** or crowded **Auto** legend takes the least-covered inside corner),
because a legend outside one panel would shrink its plot area.

**Colours** lists the series in the chosen panels, including residual strips.
Choose a swatch to set a shared colour for that series key in every panel. This
recipe setting overrides the figure's own colour; **Auto** removes only the
shared override. For several samples, the default is each group's workspace plot
colour, and the curve key is `<group id>/<series key>`; a full-key figure or recipe
colour overrides that default. One-sample panels retain their **Single figure**
colours and automatic palette. Shared colours are saved with the recipe and used
in PNG, TIFF, SVG and PDF exports. Removing a panel keeps its saved colours for
later reuse.

rexafs aligns the panels by giving every panel the same fixed margins: the
widest left margin any panel needs for its y tick labels and axis title, and
likewise for the other sides. The plot areas of a row therefore share their top
and bottom edges, and those of a column share their left and right edges. Each
panel is rendered separately by ruviz and the panels are tiled into one image
and nested into one SVG, the same technique as the fit + residual figure. The
bold panel labels sit at the top-left corner of each panel, at the axis-title
font size. Each recipe stores panels, width, columns, height, font, line width,
DPI, shared colours and ordered sample IDs. Each panel stores its figure key,
optional own sample IDs, offset step, reverse flag and label mode; naming a recipe
does not change how its panels render. **Single figure**
settings, including captions and visible curves, remain shared across recipes,
so changing them affects every recipe using that figure type.

The `.rxs` project saves named recipes as an ordered `publication.recipes` array
of `{name, recipe}` entries and the selection as the zero-based
`publication.active_recipe` index. An index beyond the saved list selects its
final recipe. An older `publication.composite` loads unchanged as **Figure 1**;
projects without recipe data start with a default **Figure 1**. Saving writes
only the new recipe format; the compact project encoder may omit default
fields. Legacy panels saved as plain figure-key strings still load with shared
samples, zero offset, forward order and legend labels. New saves write panel
objects, with sample IDs as ordered arrays rather than labels or workspace row
numbers. Storage and migration are implemented in
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

The suggested filename comes from the recipe
name, for example `figure-2.pdf`. It is lowercased, keeps Unicode letters and
digits, and replaces runs of punctuation or whitespace with a single hyphen,
without leading or trailing hyphens. An empty result uses `figure`; reserved
Windows device names gain a `figure-` prefix.

The caption below the preview joins each panel's caption after its label, for
example "(a) Flattened, edge-step-normalized X-ray absorption spectrum… (b) …";
**Copy caption** copies it. **Save caption…** saves it as plain text,
with a suggested filename such as `figure-2-caption.txt`. Panels whose figure
is unavailable, such as a fit panel before any fit has run, are skipped, listed
under the preview and kept in the recipe. Missing samples are also listed and
retained in the recipe. An inset and a matplotlib script for the whole grid are
not available yet. The composer is implemented in
[`composer.rs`](https://github.com/Ameyanagi/rexafs/blob/dev/crates/rexafs-gui/src/publication/figures/composer.rs).

## Captions

Enter a figure caption and processing/parameter/path-table captions in the editor;
press **Enter** to apply text. Blank captions use factual descriptions of the
selected curves or table definitions. **Copy caption** transfers the current
figure's caption. Settings and captions are saved in `.rxs` files and apply to
the same figure/table type across a multi-spectrum export. For different samples,
use general captions here and complete individual exported captions in the report.

**Export analysis folder** creates a new directory containing:

- `report.html`: a report with vector figures, numbered figure captions and
  semantic tables with captions, units and uncertainty definitions; printable
  through a browser, with image aspect ratios retained;
- `README.md`, `analysis.md` and `captions.md`: linked figures, a captioned analysis
  record and captions ready to edit into a manuscript;
- PNG/SVG figure pairs, with descriptive caption metadata in the SVG;
- `manifest.json`: figure/table numbering, captions, presentation settings,
  source/fit associations and any unavailable or stale results;
- processed arrays, available fit arrays, requested/resolved settings, fit history,
  the project, a methods draft and reference files.

Captions identify data and conventions; they do not infer sample preparation,
temperature, beamline conditions or scientific conclusions. Complete those from
the experimental record. Standard errors represent the fit covariance, not all
experimental/model uncertainty. R-space coordinates are not phase corrected.
Review unavailable arrays and stale-fit notices before selecting results for a
paper. The export preserves evidence for that review; it does not certify a fit's
scientific validity or compliance with every journal's submission rules.

An export can finish with notices about failed spectra, unavailable historical
curves or failed figures; review `manifest.json`, `README.md` and `report.html`
before using its contents. A filesystem failure can instead stop the export
early and leave a partial directory, possibly without a manifest. Resolve the
reported cause and retry into a new directory. The analysis folder is written
file by file; it does not have the atomic replacement/backup behavior of a saved
`.rxs` project. See the [folder exporter](https://github.com/Ameyanagi/rexafs/blob/v0.2.4/crates/rexafs-gui/src/publication.rs#L248).

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
