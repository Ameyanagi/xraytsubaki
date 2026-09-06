# Publication assets and analysis context

Use **Publish** to adjust figures and captions, save individual PNG/SVG files, or choose **Export analysis folder…**. See the [publication editor guide](publication.md) for controls, defaults and caption conventions. A new local folder contains:

| Asset | Contents |
|---|---|
| `analysis.md` | Requested settings, source comments, current model, historical fit inputs, values, uncertainties, path distances and journal |
| `resolved.md` | Per-spectrum processing outputs recomputed at export time |
| `figures/*.png`, `figures/*.svg` | Spectra, fit overlays and residuals using the saved dimensions/style; ruviz defaults when unset |
| `report.html`, `captions.md` | Vector figures and tables with numbered captions, plus manuscript caption text |
| `data/*.json` | Processed arrays and available full fit results |
| `methods.md` | Editable methods draft with missing experimental details identified |
| `references.md`, `references.bib` | Algorithm references and reminders to cite the actual data, structures and FEFF backend |
| `state.json`, `project.rxs` | Structured analysis context and project |
| `batch-results.csv` | Batch results when available; the manifest flags stale results |
| `README.md`, `manifest.json` | Figure index and any incomplete exports |

Scope is the current spectrum, marked spectra, assigned fit spectra and recorded results. **Copy Markdown** copies the analysis record without exporting figures. These assets also provide context that an external LLM can read.

An existing destination is never overwritten. The project uses the selected raw-data mode: relative links by default, or losslessly compressed original spectra and referenced FEFF inputs with **Raw: embedded**. Its metadata header records sources and checksums. Full processed arrays are included. Archived fit statistics remain exportable when their plot arrays are unavailable; the manifest reports the missing figures. Auto requests and historical settings are explicitly distinguished from current, resolved values. The methods text is a draft, not an invented experimental record.
