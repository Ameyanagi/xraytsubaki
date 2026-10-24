# Publication assets and analysis context

Use **Publish** to adjust figures and captions, save individual PNG/SVG files, or choose **Export analysis folder…**. See the [publication editor guide](publication.md) for controls, defaults and caption conventions. A new local folder contains:

| Asset | Contents |
|---|---|
| `analysis.md` | Requested settings, source comments, current model, historical fit inputs, fit statistics tables, values, uncertainties, path distances and journal |
| `resolved.md` | Per-spectrum processing outputs recomputed at export time |
| `figures/*.png`, `figures/*.svg` | Spectra, fit overlays and residuals using the saved dimensions/style; native canvas size and rexafs’s 300 DPI when unset |
| `figures/composed/` (unreleased) | Every renderable named recipe as PNG/SVG/PDF with a combined plain-text caption; matplotlib folders contain an editable script and unshifted curve CSVs when available |
| `report.html`, `captions.md` | Browser-printable vector figures with retained aspect ratios, semantic tables, numbered captions, units and uncertainty definitions; for each fit, a statistics table (R-factor, χ², reduced χ² when noise was estimated, N_idp / N_var, fit space, k and R ranges, k-weights, windows and correlations with \|r\| ≥ 0.10); manuscript caption text |
| `data/*.json` | Processed arrays and available full fit results |
| `methods.md` | Methods draft generated from the values actually used: E₀, pre-edge and normalization ranges and polynomial order (or MBACK), background method with R_bkg, k range and k-weight, transform k range, k-weight and window, and for each fit its space, k/R ranges, k-weights, windows, FEFF engine (ReFEFF or FEFF10), varied parameters and N_idp. Steps that were not run are omitted; only cited works are listed. The Experiment section is left for you to complete |
| `references.md`, `references.bib` | Algorithm references and reminders to cite the actual data, structures and FEFF backend |
| `state.json`, `project.rxs` | Structured analysis context and project |
| `batch-results.csv` | Batch results when available; the manifest flags stale results |
| `README.md`, `manifest.json` | Figure index and any incomplete exports |

Scope is the current spectrum, marked spectra, assigned fit spectra and recorded results. In the unreleased composer export, all named recipes follow the single figures with their names and captions. Recipe samples resolve from the current workspace, including unmarked or uncached groups and groups absent from the saved project. This does not expand the saved project's normal source scope; static figures and matplotlib data retain those plotted curves. Failed recipes and missing panels appear in export notices while other recipes continue. See the [publication editor guide](publication.md) for file naming and source requirements. **Copy Markdown** copies the analysis record without exporting figures. These assets also provide context that an external LLM can read.

An existing destination is never overwritten. The project uses the selected raw-data mode: relative links by default, or losslessly compressed original spectra and referenced FEFF inputs with **Raw: embedded**. Its metadata header records sources and checksums. Full processed arrays are included. Archived fit statistics remain exportable when their plot arrays are unavailable; the manifest reports the missing figures. Auto requests and historical settings are explicitly distinguished from current, resolved values. The methods text is a draft, not an invented experimental record.

An export can finish with notices about failed spectra, unavailable historical
curves or failed figures; review `manifest.json`, `README.md` and `report.html`
before using its contents. A filesystem failure can instead stop the export
early and leave a partial directory, possibly without a manifest. Resolve the
reported cause and retry into a new directory. The analysis folder is written
file by file; it does not have the atomic replacement/backup behavior of a saved
`.rxs` project. See the [folder exporter](https://github.com/Ameyanagi/rexafs/blob/v0.2.4/crates/rexafs-gui/src/publication.rs#L248).
