# Preparing publication output

Open **Publish** after processing a spectrum or completing a fit. Select a figure,
adjust its controls, and save **PNG** or **SVG**. The preview preserves the image's
aspect ratio and uses the same rendered PNG bytes as the individual PNG save.

Blank controls use ruviz's native defaults, including its 6.4 × 4.8 inch canvas at
100 DPI (640 × 480 pixels with the current dependency). Width and height use
inches; font and line widths use points. Set DPI to the journal's requested raster
resolution, or use the vector SVG for scalable line art. Changing the preview
window does not alter export dimensions. Reset restores the selected figure's
defaults.

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
