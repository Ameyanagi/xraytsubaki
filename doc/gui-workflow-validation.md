# Desktop workflow validation — 2026-09-06

Checks use optimized release builds and native computer use on macOS. Earlier structure, XDI, FEFF/ReFEFF, batch and per-spectrum model checks are recorded in [the fitting redesign](fitting-workspace-redesign.md), [multiple-spectrum fitting](joint-fitting.md), [XDI import](xdi-import.md) and [structure depth views](structure-depth-view.md). This is a record of exercised workflows, not a claim that every possible input or external service was tested.

| Workflow | Observed result |
|---|---|
| Stage tabs | Bordered tabs and a tinted active state were checked in dark and light themes. Hovering Normalize showed current E₀, edge step and normalization windows without switching from Transform; Transform showed its weight and ranges. |
| Range handles | The hit region now includes the visible grab tab above the axis. A direct native drag changed model Rmin from 1.0 to 1.9 Å while retaining the plot axis. Previously it panned the plot. |
| Marked processing settings | Copper spectra with FT weights 1 and 3 show an inline warning immediately left of the input. Clicking it shows both spectrum values. |
| Parameter right-click | Compact popup at the pointer: Apply to marked, Reset to default, Compare marked. Applying just the FT weight makes both spectra agree; Undo restores the different weight and warning. |
| Processing thumbnails | Each of the four thumbnails opens its corresponding single plot: normalized μ, weighted χ(k), χ(k) plus window, or R magnitude. The R thumbnail stays in Transform. |
| Multiple-spectrum preview | Selecting spectrum B shows its own k range 3–12 and weight 3; A retains 2–11 and weight 1. Fit-in-k, Fit-in-R and Fit-in-q update the preview. Imaginary χ(R) appears when enabled. |
| Multiple-spectrum fit | Two Cu spectra, explicit first-shell paths, shared S₀²/ΔE₀ and per-spectrum ΔR/σ². Six varying parameters; converged, R-factor 0.0120770. Results display per-spectrum overlays and errors. |
| Publication export | Native export produced 20 PNGs, Markdown, JSON arrays, project and citations. The manifest explicitly reported four older fits whose curve arrays were not in the session. Representative spectra and fit PNGs were visually inspected. |
| Assistant connection | The separate Experimental window connects through the existing installed Codex account. A real account-status handshake passed separately. |
| Assistant copper workflow | A real model turn selected a Cu reference, calculated ReFEFF paths and completed a first-shell fit with 12 neighbors, k-weight 2, k 2–15 Å⁻¹ and R 1.5–3 Å. R-factor 0.00194353; Cu–Cu distance 2.5506 ± 0.0022 Å. Results and the saved project were inspected. The host required current Normalize/Background/Transform inspections before fitting. |
| Assistant window lifecycle | Native testing exposed an entity-borrow crash when opening the assistant synchronously. Deferring window creation fixed it; opening the window and Show app were rechecked. Reopening Assistant retained the draft and account connection. Focus plots hid both panels. A real review-mode model turn resized the main window to 1000 × 700 and focused it. |

The release test suite passed 109 tests with three intentionally skipped integration tests. The suite covers metal foils with both calculation backends, global/local fitting, processing edit validation and a real copper publication export. Numerical convergence is distinct from model adequacy; the first-shell examples are workflow checks, not full structural interpretations.

Evidence: [parameter popup](validation/parameter-context-menu.jpg), [marked comparison](validation/marked-settings-comparison.jpg), [multiple-spectrum result](validation/multiple-spectra-result.jpg), [assistant-generated copper result](validation/assistant-copper-result.jpg).

Tab evidence: [normalization hover](validation/stage-tab-normalization-hover.jpg), [light theme](validation/stage-tabs-light.jpg). Back-transform Auto hints were corrected to the core defaults (0–20 Å), matching the resolved hover values.

Window-control evidence: [assistant-arranged analysis window](validation/assistant-window-layout.jpg).
