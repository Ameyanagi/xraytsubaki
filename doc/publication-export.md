# Publication assets and analysis context

Choose **Publish → Export folder…**. A new local folder contains:

| Asset | Contents |
|---|---|
| `analysis.md` | Requested settings, source comments, current model, historical fit inputs, values, uncertainties, path distances and journal |
| `resolved.md` | Per-spectrum processing outputs recomputed at export time |
| `figures/*.png` | 1600 × 1000 spectra, fit overlays and residuals |
| `data/*.json` | Processed arrays and available full fit results |
| `methods.md` | Editable methods draft with missing experimental details identified |
| `references.md`, `references.bib` | Algorithm references and reminders to cite the actual data, structures and FEFF backend |
| `state.json`, `project.xtproj` | Structured analysis context and project |
| `batch.csv` | Batch results when available; the manifest flags stale results |
| `README.md`, `manifest.json` | Figure index and any incomplete exports |

Scope is the current spectrum, marked spectra, assigned fit spectra and recorded results. **Copy Markdown** copies the analysis record without exporting figures. These assets also provide context that an external LLM can read.

An existing destination is never overwritten. Original input and FEFF paths are retained in the project; raw source files and FEFF workspaces are not copied into the bundle. Full processed arrays are included. Archived fit statistics remain exportable when their plot arrays are unavailable; the manifest reports the missing figures. Auto requests and historical settings are explicitly distinguished from current, resolved values. The methods text is a draft, not an invented experimental record.
