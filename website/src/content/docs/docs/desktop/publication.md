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

### Curve colours (unreleased)

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
matplotlib export. A **Single figure** matplotlib export currently includes only
the upper panel of a fit + residual figure; standalone residual figures export
their curve. **Apply style to all figures** preserves each figure's curve colours. Figures without overrides retain their existing automatic colours.
The saved `series_colors` map uses curve keys as plain strings and lowercase
`#rrggbb` sRGB values; malformed values stop rendering/export with an error,
and entries for curves that are no longer available have no visual effect.
Implemented in
[`FigureOptions` and `FigureData::plot`](https://github.com/Ameyanagi/rexafs/blob/dev/crates/rexafs-gui/src/publication/figures.rs).

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

**Samples and stacking (unreleased).** **Samples** offers the current group
first, then the marked groups in workspace order. Check only the samples to
include. **All** selects the offered groups in that order; **None** clears the
list. An empty list means the current group only, preserving older recipes.
Checking a sample appends its durable group ID to the saved list. The recipe
keeps that order when marks or the current group change.

Enter an optional short **Figure label** beside each sample, for example `Cu foil`
instead of a long acquisition filename. Multi-sample legends and curve-end labels
use it; a blank label uses the full workspace group name and follows later group
renames. Figure labels belong to this recipe, are saved by group ID in
`figure_labels`, and do not rename workspace groups. The combined caption
identifies each short label and its full sample name once per figure when they
differ; offset lists refer to the short labels. In a one-sample composer panel,
an explicit figure label replaces the main curve's label; a blank label keeps
that figure's ordinary curve labels.

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

**Offset step** is a display displacement between successive samples:
zero overlays the original curves. A step of 0.3 adds 0, 0.3, 0.6, and so on to
the samples' vertical values in saved order. **Reverse order** assigns these
offsets in reverse, putting the first sample highest for a positive step.
Negative steps move successive curves downward. Missing samples keep their
positions in the saved order. Offsets use the panel's displayed vertical-axis
units, including any k weighting; they modify copied display arrays only, leaving
processed spectra and fit results unchanged. The combined caption records a
non-zero applied offset. **Curve labels** defaults to **Legend**; **Curve ends**
places the complete figure labels at the right side of the plot with space
reserved across the grid. Labels and padding may use at most 35% of a panel's
width. Labels are never shortened: when they exceed that allowance, rendering
stops with “Curve-end labels are too long for this panel. Shorten the figure
labels or use a legend.” The raster and SVG use the same complete labels.
The plot must also retain at least 0.5 in of width. If it cannot, use a wider
figure, fewer columns, a smaller font or a legend. Labels that cannot fit
vertically require taller panels, fewer samples, a smaller font or a legend.
These display choices are implemented in
[`prepare_panels` and `compose_samples`](https://github.com/Ameyanagi/rexafs/blob/dev/crates/rexafs-gui/src/publication/figures/composer.rs).

**Panel insets (unreleased).** Select an energy-domain panel to add one inset.
**Inset** defaults to **None** and is available only for XANES, flattened μ(E)
and raw μ(E) panels (saved keys `xanes`, `flattened-mu` and `mu-energy`).
Choose a rexafs preset, then edit its energy limits relative to E₀, the
absorption-edge energy of the panel's first selected sample:

| Inset | Default energy window |
| --- | --- |
| **Pre-edge** | E₀ − 6 to E₀ + 11 eV |
| **Edge** (edge and white line) | E₀ − 2 to E₀ + 23 eV |
| **Difference** (difference from first sample) | E₀ − 6 to E₀ + 31 eV |

These windows are display defaults chosen by rexafs, rather than scientific
analysis ranges. Every sample uses the same absolute energy window. **Reverse
order** changes only the offsets; it does not change the first sample, E₀ or
the difference reference. The first selected sample must be available.

**Pre-edge** and **Edge** magnify the panel's displayed quantity.
**Difference** requires at least two selected samples and
plots each sample's normalized μ(E) minus the first sample's normalized μ(E),
including when the main panel shows raw or flattened absorption. It evaluates
the first sample on each other sample's measured energy grid by linear
interpolation, keeps only their overlapping measured energy interval and never
extrapolates. The inset label **Δμ** denotes this dimensionless difference in
normalized absorption. This display calculation changes neither processed
spectra nor the source arrays.

**Inset curves follow the stacking offset** is on by default. It applies the
panel's saved sample order, reverse setting and offset step, scaling that step by
the inset's largest individual unshifted visible-curve y span divided by the main
panel's largest individual unshifted visible-curve y span. Each span is the
curve's maximum minus minimum within that view's energy limits; curves are
measured separately before choosing the largest span, excluding display offsets
and axis padding. If either span is zero, the inset offset is zero. Turn this
option off to overlay the inset curves without offsets.

**Placement** defaults to **Lower right**; **Upper left** and **Center right**
are also available. The inset has a white background, its own energy tick
labels and a small in-plot quantity tag, **μ(E)** or **Δμ**, with no y tick
labels. It never shows a legend, including when **Legend in every panel** is
on. The tag has a padded, opaque white box to keep curves from obscuring the
text, and automatic vertical headroom leaves space above the curves for it.
The main-panel legend is placed automatically inside the plot, away from the
inset. Inset curves retain the main panel's figure labels, resolved print colours
and line styles at a thinner width. The inset's grid stays off even when the
main-panel **Grid** setting is on, and its energy ticks point inward.
Insets appear in the preview and the normal
PNG, TIFF, SVG and PDF exports; plain SVG labels remain editable text. The combined
caption describes the inset and rounds its energy limits to 0.1 eV, omitting
trailing fractional zeros. The plotted limits retain their full precision.
The inset's settings are saved with that panel in the
`.rxs` project. Implemented in
[`composer.rs`](https://github.com/Ameyanagi/rexafs/blob/dev/crates/rexafs-gui/src/publication/figures/composer.rs).

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
the same text and curve weights. Panels use the figure's **Single figure**
settings for axis labels, axis limits, visible curves, curve colours, flattened
or normalized μ(E), and processing guides. Titles are left out because the panel
labels identify the panels.

**Grid, ticks and legends (unreleased).** The recipe's **Grid** is off by default
and overrides each figure's grid setting within the composition. Main-panel major
and minor ticks point inward on all four sides. rexafs chooses four evenly spaced minor
ticks between major ticks because ruviz sets a fixed count per plot. This is a
project choice: Matplotlib's
[`AutoMinorLocator`](https://matplotlib.org/stable/api/ticker_api.html#matplotlib.ticker.AutoMinorLocator)
adapts the count, using four minor ticks when the major interval is 1, 2.5, 5
or 10 times a power of ten, and three otherwise.

For panels selecting several samples, the default shows a legend only in the
first panel with at least two visible samples whose **Curve labels** mode is
**Legend** and whose figure legend is enabled. Enable **Legend in every panel**
to show a legend in every panel that selects several samples and permits a
legend. Curve-end panels do not show a legend. One-sample and fit panels retain their figure's legend
visibility. Composer legends use text one point smaller than the figure font,
with a minimum of 5 pt, and stay inside the plot. Automatic placement chooses the
inside position that covers the least plotted data. Composer legends use this
placement for every figure legend setting so that panels remain aligned. The
renderer currently draws a box around legends. These settings apply to the composition; **Single figure** keeps its
own grid, ticks and legend behavior. The shared composer legend styling is
implemented in
[`composer.rs`](https://github.com/Ameyanagi/rexafs/blob/dev/crates/rexafs-gui/src/publication/figures/composer.rs).

**Curve colours** lists the series in the chosen panels, including residual strips.
Choose a swatch to set a shared colour for that series key in every panel. This
recipe setting overrides the figure's own colour; **Auto** removes only the
shared override.

**Sample colours (unreleased)** selects the automatic colours for multi-sample
panels. rexafs chooses one colour order for the recipe: shared sample IDs selected
by at least one spectrum panel come first, in the shared list's order, followed
by panel-only sample IDs in panel reading order and then selection order. Each
sample appears once; missing samples retain their colour slots. This is a rexafs
choice that keeps a sample's automatic colour consistent when a panel selects a
subset or changes its sample order.

- **Ordered** (the default) samples viridis at evenly spaced positions from 0 to
  0.82 across that recipe-wide order. Viridis is the sequential map designed by
  Nathaniel Smith and Stéfan van der Walt; see their
  [colormap design notes](https://bids.github.io/colormap/). Ending at 0.82 is a
  rexafs choice to avoid the palest yellow at the bright end on white paper.
- **Okabe–Ito** cycles through the eight-colour categorical palette from Okabe
  and Ito (2008),
  [“Color Universal Design”](https://jfly.uni-koeln.de/color/), for samples that
  represent separate categories.
- **Workspace** keeps each group's workspace plot colour.

The multi-sample curve key is `<group id>/<series key>`. An explicit colour
saved under that full key in the figure settings overrides the sample palette;
a matching recipe colour overrides both. A short key such as `main` does not
colour every sample alike. One-sample panels retain their **Single figure**
colours and automatic palette. Sample palettes and shared colours are saved with
the recipe and used in PNG, TIFF, SVG and PDF exports. Removing a panel keeps its
saved colours for later reuse. Sample colour resolution is implemented in
[`prepare_panels`](https://github.com/Ameyanagi/rexafs/blob/dev/crates/rexafs-gui/src/publication/figures/composer.rs).

rexafs aligns the panels by giving every panel the same fixed margins: the
widest left margin any panel needs for its y tick labels and axis title, and
likewise for the other sides. The plot areas of a row therefore share their top
and bottom edges, and those of a column share their left and right edges. Each
panel is rendered separately by ruviz and the panels are tiled into one image
and nested into one SVG, the same technique as the fit + residual figure. The
bold panel labels sit at the top-left corner of each panel, at the axis-title
font size. Each recipe stores panels, width, columns, height, font, line width,
DPI, grid and legend repetition settings, sample palette, figure labels, shared
colours and ordered sample IDs. Each panel stores its figure key, optional own
sample IDs, offset step, reverse flag, label mode and inset; naming a recipe
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
samples, zero offset, forward order, legend labels and no inset. Older panel
objects without an inset also load with no inset. New saves write panel objects,
with sample IDs as ordered arrays rather than labels or workspace row
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

**Data + Python script… (unreleased).** Beside the image saves, this action
exports the active named recipe to a new folder, suggested as
`figure-2-matplotlib` for **Figure 2**. An existing folder is rejected. The
export uses the same prepared curves as the current preview and is available
when that preview is ready. It writes `make_figure.py` and one CSV with two
columns, x and y, for each exported curve in a main panel, residual strip or
inset (`a-series-01.csv`, `a-residual-series-01.csv`,
`a-inset-series-01.csv`, and so on). CSV values are unshifted: they contain the plotted scientific quantity,
including any normalization, k weighting or normalized difference, without the
vertical display offsets. The export does not modify the project's processed
spectra or fit results.

All presentation settings are editable at the top of the script: the recipe's
printed size, panel arrangement, DPI, font and line widths, grid, labels, resolved
colours, line styles, limits, legend style, inset placement and separate offsets
for main and inset curves. Change an offset there to restack curves without
changing the CSV data. Fit + residual panels retain both axes. Available panels
fill the grid in recipe order; unavailable panels are omitted without empty
slots, and the script notes skipped panels and samples. Check these notices
before using the figure.

The exported colours are the resolved recipe, sample and figure colours used by
the preview, including residual strips and insets. The recipe's grid setting is
off by default. Major and minor ticks point inward on all four sides; the script
uses Matplotlib's
[`AutoMinorLocator`](https://matplotlib.org/stable/api/ticker_api.html#matplotlib.ticker.AutoMinorLocator)
for minor ticks. Insets retain their resolved colours and line styles, inward
ticks, and no grid or legend. Their quantity tag keeps its opaque white box and
automatic vertical headroom above the curves.

Legend visibility follows the native preparation's per-panel choices, including
**Legend in every panel** and **Curve labels**; insets never show a legend. The
Python script uses frameless legends with text at `FONT_SIZE - 1` points,
`loc="best"`, `handlelength=1.4`, `labelspacing=0.3` and `handletextpad=0.5`.
These are the Python legend settings; the native renderer currently draws the
boxed legends described above.

Curve-end labels use complete figure labels, and CSV headers retain those same
complete names. Export applies the native curve-end width and vertical-fit
validation before writing files: labels and padding must fit within 35% of the
panel width and leave at least 0.5 in for the plot. Labels are never shortened or
truncated. If validation fails, shorten the recipe's figure labels, adjust the
figure dimensions or font size, reduce the sample count, or choose a legend,
then export again. Each curve's complete `label` remains editable in the script.

Run the script with Python 3, numpy and matplotlib; rexafs is not required. For
example, run `uv run --with numpy --with matplotlib python make_figure.py`.
It reads the CSV files beside itself and saves `figure.pdf`, `figure.svg` and
`figure.png` there at the recipe's printed size and DPI. Matplotlib lays out
ticks and margins using its own font metrics, so the result is an editable
reproduction whose layout can differ from the rexafs preview. Implemented in
[`write_composite_python_export`](https://github.com/Ameyanagi/rexafs/blob/dev/crates/rexafs-gui/src/publication/python_export/composite.rs).

The caption below the preview joins each panel's caption after its label, for
example "(a) Flattened, edge-step-normalized X-ray absorption spectrum… (b) …";
**Copy caption** copies it. **Save caption…** saves it as plain text,
with a suggested filename such as `figure-2-caption.txt`. Panels whose figure
is unavailable, such as a fit panel before any fit has run, are skipped, listed
under the preview and kept in the recipe. Missing samples are also listed and
retained in the recipe. The composer is implemented in
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
