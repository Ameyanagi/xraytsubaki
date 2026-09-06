# Desktop UX audit — 7 September 2026

[Open screenshot gallery](index.html). `steps.json` contains timestamped observations and accessibility snapshots for every capture. Screenshots 1–34 document the released v0.1.1 baseline. Screenshots 35 onward document the patched development app, including failures found during QA and subsequent corrections.

## Verified findings

| Priority | Finding | Evidence | Planned correction |
|---|---|---|---|
| High | Reopening a saved linked project loses its per-spectrum R-window override and leaves a blank plot after index refresh; saved file contains the correct override | 25–31 | Preserve selection and overrides across path aliases and catalog refresh, then reload |
| High | Reference mode replaces the sample from a multi-channel QAS file and retains the same filename label | 9–12 | Import separately identified sample/reference groups with independent settings and provenance |
| High | Fit range checks do not compare R min with the spectrum's AUTOBK Rbkg | Code inspection, user report | Enforce the floor for numeric/plot input and each single, batch and joint solver input |
| Medium | Folder-only import picker disables individual files; opening a folder replaces the session | 13; code inspection | Append selected files/folders and preserve existing groups/settings |
| Medium | No AUTOBK k-min/k-max plot handles on χ(k) | 18 | Add draggable ranges synchronized with numeric controls |
| Medium | Selection controls clip at the group-panel edge; bulk unmark is hidden in the palette; no visible invert | 1–7 | Visible, scoped all/none/invert controls and hidden-mark counts |
| Medium | Marked (0) still plots the current spectrum | 7 | Make the displayed scope explicit |
| Medium | Directories become scans, even for Cu and Ni spectra with different edges | 8 | Require deliberate series grouping / surface incompatible-edge groups |
| Medium | Missing edge-step override and clamp-point count; inverse FFT window exists in params but has no selector; additional library inputs lack controls | 14,17,23 | Complete library-input parity; disable controls unused by the selected algorithm |
| Medium | Selecting a nonlinear background solver changes the clamp model while λ stays editable; long menu values clip | 21–22 | Explain and display effective settings, disable inactive controls |
| Medium | Transform N-independent display assumes R=1–3 while inverse FFT resolves Auto to 0–20; qmax not exposed | 23–24 | Use resolved settings consistently and expose inverse transform inputs |
| Medium | Publish omits flattened μ(E) and numerical CSV export | 33 | Separate normalized/flattened figures using exact library arrays; CSV alongside PNG/SVG |
| Low | Edited project has no clear dirty indicator or project name | 27 | Show project identity and unsaved state |
| Low | Publish has a long scrolling control column with figure/style/caption/curve options interleaved | 33 | Improve grouping and keep export scope clear |

## Behaviors that worked

- Normalization maximum is available numerically and updates after Enter (15).
- Invalid numeric text is rejected with an error and preserves the prior value (16).
- Marked overlays work; hidden marks persist when filtering (4–5), though scope needs clearer wording.
- Ordinary plot pan/reverse did not reproduce duplicated spectra (19). This was not a frame-level high-speed stress benchmark.

## Validated first set of fixes

Tracking issue: [#23](https://github.com/Ameyanagi/rexafs/issues/23).

- 133 GUI regression tests passed, 3 optional/local-fixture tests skipped (246.37 s).
- After later focused changes: all 7 Publish tests and the background-handle regression passed. Clippy completed with existing warnings; the GUI does not currently enforce a warning-free baseline.
- Reopened linked project restores the selected Ru spectrum and R=1–3 Å override (35,39).
- Select all / Invert / Deselect all and Cmd+A verified (36–39).
- AUTOBK k max drags inward to 12 and outward to 15 Å⁻¹ while retaining measured axis coverage (53–54). Earlier handle bugs and their corrections are recorded in 41,46–47.
- Back-transform R max drags from 3 to 4 Å (55).
- Fit R min below Rbkg is flagged, numeric entry is rejected, and the handle stops at Rbkg (50–52). Worker tests cover single and per-dataset joint validation.
- Flattened Publish preview, PNG and CSV export verified (42–45). The complete bundle contains six PNG/SVG/CSV figures with no export notices. Its 645-point flattened CSV equals the standalone export.
- [Flattened Ru figure](publication-example/flattened-ru.png) · [numerical CSV](publication-example/flattened-ru.csv). Public QAS test fixture, no private experimental data.

## Work in progress

- Branch: `feature/data-import-updates` (no release from this branch yet).
- First set of fixes above is ready for pull-request CI; no release from this branch yet.
- Edge-step and endpoint clamp-count GUI inputs are wired, with a direct pipeline regression check.
- Remaining: append/multi-channel import, complete advanced-parameter parity, further Publish/layout polish, Stable/Nightly update channels and CI workflows.
- Still to audit: full Fit/Series workflows, exported output and embedded-project round-trip, light theme and update UI; then repeat affected states on the patched app.

The original audit process is isolated from the user's older installed rexafs application. Interactions pause when the user takes control. Private file-picker directories are not included in the published captures.
