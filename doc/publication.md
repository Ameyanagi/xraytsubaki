# Preparing publication output

Open **Publish** after processing a spectrum or completing a fit. The header
names the report scope (the current group, the marked groups and the recorded
fits). **Publish report…** (since 0.2.12; the 0.2.11 control is a format selector
with **Export…**) writes the complete analysis folder described in
[publication export](publication-export.md): `report.html`, the analysis record,
every figure as PNG, SVG and CSV, tables, captions, resolved data and the
project. After publishing, **Open report** opens `report.html` and **Show
folder** reveals the directory. **Copy analysis record** copies the Markdown
record to the clipboard.

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
the vector drawing's printed size. Text, including axis labels and math, is
typeset by Typst and stored as glyph outlines rather than editable `<text>`
elements; it looks identical everywhere but must be retyped to change wording.

Sizes must be 1–30 inches on each side; DPI must be an integer from 72 to 1200.
The image must contain at most 25 million pixels. Width in pixels is width in
inches multiplied by DPI, and likewise for height. These limits and defaults
come from [FigureOptions](https://github.com/Ameyanagi/rexafs/blob/v0.2.4/crates/rexafs-gui/src/publication/figures.rs#L82).

Controls include labels/title, paired axis limits, legend, grid, processing guides
and individual visible curves. Fit figures expose model/data components and path
contributions without vertical offsets. R-space residuals explicitly represent
the difference of magnitudes, rather than a complex residual. All scientific
curves come from the processing/fit results; branding images are separate assets.

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
