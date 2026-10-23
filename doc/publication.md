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
Typst math, such as the default axis labels `$E$ (eV)`, `$mu(E)$` or
`$k^2 chi(k)$`, stay as glyph outlines (vector shapes of each character) so
subscripts, superscripts and Greek letters render identically everywhere; each
outlined label carries an `aria-label` with its Typst source. Editable text is
drawn by the viewer, so it matches the preview exactly only when the viewer has
the figure font (Arial by default) installed; otherwise the viewer substitutes
a similar font. PNG output is unaffected. This uses ruviz's
`Plot::svg_native_text` option, enabled in
[`FigureData::plot`](https://github.com/Ameyanagi/rexafs/blob/dev/crates/rexafs-gui/src/publication/figures.rs).

Sizes must be 1–30 inches on each side; DPI must be an integer from 72 to 1200.
The image must contain at most 25 million pixels. Width in pixels is width in
inches multiplied by DPI, and likewise for height. These limits and defaults
come from [FigureOptions](https://github.com/Ameyanagi/rexafs/blob/v0.2.4/crates/rexafs-gui/src/publication/figures.rs#L82).

Controls include labels/title, paired axis limits, legend, grid, processing guides
and individual visible curves. Fit figures expose model/data components and path
contributions without vertical offsets. R-space residuals explicitly represent
the difference of magnitudes, rather than a complex residual. All scientific
curves come from the processing/fit results; branding images are separate assets.

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
