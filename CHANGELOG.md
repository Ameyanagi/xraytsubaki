# Changelog

## Unreleased

- Ask to save, discard or cancel before closing the window or quitting with
  unsaved project changes; empty or unmodified sessions close without asking.
- Use sentence-case scientific labels with units for Normalize, Background and
  Transform parameters (for example "Spline k max", "Victoreen n"),
  and show a hover explanation on every label with its unit, Auto value and the
  matching Larch or Athena name.
- Show each group on two lines: the name, shortened at the end, then its source
  file, quantity and E₀ when known, so similarly named references stay
  distinguishable. Only the current group is highlighted; marks use checkboxes.
- Write publication SVG width and height in inches with a pixel `viewBox`, so a
  3.3 in figure imports at 3.3 in instead of about 10 in.
- Write plain publication SVG labels as editable `<text>` (ruviz 0.15.1);
  labels with math or markup stay outlined.
- Report degenerate normalization, Wavelet failures and XANES peak-fit failures
  in the problems drawer, and add a "Show problems" command.
- Run a queued background recompute after a failed job, so settings changed
  during the failure are not left stale.
- Show reduced χ² as "n/a (noise not estimated)" when ε was not estimated from
  the data.
- Save fit results with the project, so a reopened project lists its fits in
  Publish and shows the last fit without refitting or a stale flag.
- Fill the Used column of the copied analysis record with resolved Auto values
  and data-limited ranges.
- Redraw MCR/PCA/LCF analysis boundaries when a form opens or changes, and stop
  showing an analysis result card while Deglitch or Align is open.
- Name fit parameters by symbol, unit and path (for example "ΔR (Å) · Cu–Cu
  2.557 Å") and fitted paths by their scatterers and distance.
- Clear stale RMC run messages when a run ends, and report the outcome and
  completed attempts in the footer.
- Merge Normalize, Background and Transform into one Process stage with linked
  μ(E), normalized μ(E), kʷχ(k) and |χ(R)| plots that show every range on every
  plot it affects; ⌘1–⌘5 now select Data, Process, Fit, Series and Publish.
- Warn "Normalization unreliable" when the post-edge fit has too few points or
  too short a span, or the edge step is not trustworthy, both in the problems
  drawer and under Post-edge start.
- Add Preferences (Settings…, ⌘, / Ctrl+,) and an About panel with version,
  commit, update channel, licence and third-party notices.
- Add a legend placement setting whose Auto choice avoids the data, use fewer x
  ticks on narrow figures, extend the |χ(R)| range to the back-transform window,
  and show typed figure labels literally.
- Add "Fit + residual" figures for χ(k) and |χ(R)|.
- Scale k-space publication figures symmetrically about zero.
- Export a figure as CSV data plus an editable matplotlib script.
- Export the latest fit's parameters as a LaTeX, CSV or Word table with
  uncertainties rounded to their significant figures, and export a
  supporting-information zip with the report, a self-contained project and CSV
  data.
- Add fit statistics tables to reports, and write the methods paragraph from
  the settings actually applied.
- Draw built-in plot labels and legends in Arial-style sans-serif text instead
  of serif Typst math, in the app, exported figures and matplotlib scripts.
- Add a Multi-panel mode to Publish that lays out XANES, χ(k), χ(R) and fit
  figures as labelled (a), (b), (c) panels at journal column widths, with a
  combined caption and PNG, TIFF, SVG and PDF export.

## 0.2.14 (2026-09-24)

- Keep Live acquisition visible in a compact sidebar or separate monitor while
  working elsewhere. Select a processing reference independently of the current
  spectrum and freeze its settings when the session starts.
- Import transmission, fluorescence and reference together; choose individual
  scans, running averages or fixed-size batches. Pause/resume retains accepted
  revisions, and rewritten files replace their original averaging contribution.
- Reuse reviewed EXAFS models for incoming Live outputs or a frozen Series.
  Show fitted curves and parameter expressions with covariance-derived error
  bars, retaining gaps for failed fits and full results in saved projects.
- Set each plot-axis endpoint to a number or Auto, including a zero baseline
  with an automatic maximum. Live views share ranges within the current session.
- Add optional connected apps and reviewed local-file import to the Assistant.
  Automatic model selection prefers GPT-6 Sol when the connected catalog offers
  it; explicit model selections remain saved.
- Preserve identical embedded source revisions on project resave, rejecting
  conflicting payloads instead of overwriting a previous project.
- Correct a Windows-only path assumption in the Live batch-validation test.

See the [release notes](doc/release-notes-0.2.14.md) and
[qualification record](doc/validation/2026-09-24-release-0.2.14/review.md).

## 0.2.13 (2026-09-22)

- Bundle three measured CuO/Cu₂O/Cu references and 50 deterministic synthetic
  spectra mixed in raw absorption, with visible known fractions and ordinary
  automatic processing settings.
- Start PCA plots on a linear scale. Give MCR-ALS an independent Auto range
  covering the full common energy interval of the selected spectra.
- Add Help → Storage to open managed storage locations and remove reviewed
  installer downloads and inactive updater app copies while preserving projects,
  recovery files, scientific caches and the running application.
- Document PCA, blind MCR-ALS, known-reference LCF and Series differences for the
  copper example, with reproducible numerical results and ruviz figures in
  SVG, PDF and 600 dpi PNG formats.
- Update hdf5-pure, ureq, rand and rand_chacha dependencies; retain historical
  random-number streams and saved-state compatibility during the RNG migration.
- Add GitHub sponsorship and star links to the README and documentation.

See the [release notes](doc/release-notes-0.2.13.md) and
[qualification record](doc/validation/2026-09-22-release-0.2.13/review.md).

## 0.2.12 (release preparation)

- Platforms: require Apple Silicon for macOS desktop releases and Python wheels.
  Remove Intel Mac build, runtime and signing jobs. Historical 0.2.11 downloads
  and their original validation requirements remain available.
- RMC: choose cache memory and catalogue capacity from available memory; expose
  manual limits, preparation progress and repeated-cache-miss warnings.
- RMC: add element-pair structural overlays, distance/step heatmaps,
  coordination and distance histories with checkpoint and CSV/JSON export.
- RMC: expose genetic and hybrid EA–RMC search with saved population recovery.
- Series: add one-based frame navigation and responsive stacked plots.
- Numeric controls: show resolved Auto values and recoverable validation errors.
- Publish: one **Publish report…** action writes the complete analysis folder,
  followed by **Open report** and **Show folder**; per-figure PNG/SVG/CSV saves
  sit beside the preview; add Single column / Double column / Slide presets and
  **Apply style to all figures**.
- Fit: rename the two "Fit mode" controls to **Method** and **Spectra**, show the
  blocking reason under the disabled action, offer **Run fit again** for stale
  results, print fit warnings as sentences, and keep path identities in tooltips.
- RMC: disable result actions until a run exists, move setting explanations into
  tooltips, present run details as label/value rows, and correct field labels.
- Shell: persist the theme, rebuild wavelet panels on theme change, raise the
  text floor to 11 px, darken light-theme muted text, keep the group name on its
  own inspector row, hide developer counters unless `REXAFS_DEBUG_STATS` is set,
  colour error and success status text, rank palette matches by title, and use
  correct plurals throughout.
- Normalize: MBACK pre-fills the absorber and edge from the spectrum's E₀ when
  the source declares none.
- Plots and Series: integer frame and component ticks, grouped Transform toolbar,
  a first-run range-handle hint, and a one-line Series definition.

- Preserve the complete signed macOS app bundle when launching the update
  helper, correcting the startup failure in 0.2.10 and 0.2.11.
- Keep **Update and restart** available after a download completes on macOS,
  Windows and Linux, with a reason when the installation cannot be updated.
- Qualify the Mac helper from both signed ZIP and installed DMG outputs.
- Refresh desktop documentation and screenshots with explicit build and input
  provenance; retain historical signed 0.2.11 workflow captures.

See the [release notes](doc/release-notes-0.2.12.md) and
[qualification record](doc/validation/2026-09-19-release-0.2.12/review.md).

## 0.2.11 — 2026-09-19

- Copy RMC fit ranges from the spectrum’s Transform settings and expand ReFEFF
  coverage for the Fourier taper and theoretical energy shift.
- Support every requested absorber in larger cells and reuse identical electronic
  inputs while retaining each site’s path catalogue.
- Add configurable CPU workers, with automatic allocation for new desktop jobs
  and optional path parallelism within the same worker pool.
- Add explicit calibration preview/application, bounded automatic coordinate
  moves, optional local refinement, and periodic theoretical ΔE₀ optimization
  with S₀² fixed. Preserve checkpoint settings and matching parameter/curve exports.
- Keep next actions at the upper right of the RMC workflow and document the
  assumptions, resource limits and historical defaults.
- Use FEFF10 0.2.4 and its corrected Windows helper to prevent intermittent
  `gg.bin` header failures. Package self-checks reject undefined format labels.

See the [release notes](doc/release-notes-0.2.11.md) and
[qualification record](doc/validation/2026-09-19-release-0.2.11/review.md).

## 0.2.7 (release preparation)

- Compact the Assistant into Model, Reasoning and Access menus beneath the
  message field; preserve keyboard navigation and keep Parameters accessible.
- Discover runtimes needed by Codex launchers when macOS opens rexafs from
  Finder, and display bounded startup diagnostics.
- Require unambiguous detector roles, retain distinct stored signals, reject
  malformed leading rows across comments, and preserve edited Athena arrays.
- Use `dev` for nightly desktop builds and promote reviewed stable releases
  through `dev` → `main`. Update `encoding_rs` to 0.8.41.

See the [release notes](doc/release-notes-0.2.7.md) and
[qualification record](doc/validation/2026-09-15-release-0.2.7/review.md).

## 0.2.6

The [published release notes](doc/release-notes-0.2.6.md) record the universal
measurement reader, plotted imports, four ABI3 Python wheels and fixture provenance.

## 0.2.5

- Add native Linux and Windows ARM64 desktop packages and Windows installers.
  Windows ARM64 uses native rexafs/ReFEFF and the x64 FEFF10 helper through
  Windows 11 emulation. Include matching Microsoft runtime DLLs in Windows ZIPs.
- Add a browser processing preview with local text/CSV import, explicit columns
  and units, cancellable WebAssembly processing, plots and CSV/provenance exports.
  ReFEFF browser support remains upstream work.
- Shorten repeated website copy while retaining scientific guides and original
  screenshots. Lead with open source, format support, speed and a small footprint.
- Validate coordinated versions across Cargo, npm, lockfiles and Python metadata
  before release builds; retain linked and embedded 0.2.5 project fixtures.
- Bundle FEFF10 with the Windows desktop. The MSVC build runs FEFF10 through
  the verified upstream `feff10-rs.exe` helper process and its MinGW runtime
  libraries in `resources/feff10`, with their notices; ReFEFF is unchanged.
  Every desktop package now includes both calculation engines.
- Expand source-owned API help for errors, numerical utilities, structures,
  fitting results and desktop/maintenance operations. Correct QAS sorting,
  retained automatic settings and policy-specific AUTOBK scaling explanations.
- Use uv's project workflow (`uv init`, `uv add`, `uv run`) in Python quickstarts,
  and enforce generated-reference inheritance and documented Rust APIs in CI.
- Correct Rust Fourier plot-axis units to include the integration measure:
  Å⁻⁽ʷ⁺¹⁾ for k-weight `w`. Calculated amplitudes are unchanged.
- Accept `AUTOBK` and `PrePostEdge` directly in Rust spectrum settings setters,
  preserving existing enum, `Some(...)` and `None` calls on both backends.
- Audit API help and scientific guides against the implementation, including
  defaults, Fourier scaling, fitting statistics, ownership and desktop workflows.
  Generate substantive stable Python/TypeScript help from maintained docstrings
  while preserving released signatures, and provide a separate Next Rust reference.
- Fix Rust reference layout and backend selection, add direct homepage API links,
  and recommend uv and Bun for library installation.

- Establish a repository-wide English and scientific-documentation baseline.
  Explain processing equations and fit-statistic conventions with source links
  and citations; correct the older uncertainty notes' mathematical errors.

- Document desktop, Python, npm and Rust installation, with quick starts and
  recommended processing defaults.
- Add Python keyword-only and TypeScript options constructors for processing
  settings. Accept `PrePostEdge` and `AUTOBK` settings directly in spectrum
  setters while preserving existing Rust-style method selectors.
- Add documented Python Literal choices, runtime docstrings/signature defaults,
  and TypeScript option interfaces/JSDoc. Check completion, hover help and strict
  typing against installed wheels and npm tarballs in release CI.
- Expose `XrayFFTR` and `Spectrum.set_ifft()` across Rust, Python and Wasm;
  inverse settings invalidate inverse results while preserving forward results.
- Retain concise historical 0.1.3 benchmark evidence and a reproducible
  normalization research prototype; numerical processing defaults are unchanged.

## 0.2.4

- Fit the desktop window to the available display area and show import, project
  and example actions in the empty workspace.
- Use platform monospace fonts; clip long text inside fields, keep the caret
  visible while editing, and correct character hit testing in translated fields.
- Keep stage shortcuts working after a focused plot control disappears during
  navigation by returning focus to the workspace before switching.
- Bound folder-scan batches and stop cancelled scans while traversing unrelated
  files, limiting queued catalog memory when the UI is busy.
- Keep Unix pprof dependencies out of Windows tests and benchmarks. Add Windows
  core checks and a Linux graphical smoke test with retained screenshots.
- Install reproducible pre-commit/pre-push checks for formatting, release tooling,
  compatibility fixtures, strict Clippy and the numerical core.

## 0.2.3

- Deliver the Windows launch, shortcut, selection and accessibility changes
  prepared for 0.2.2, which was withheld after Linux qualification failed.
- Preserve the native accessibility adapter return type on Linux/FreeBSD as
  well as macOS/Windows. Add a native Linux GUI compile check to regular CI.
- Retain linked and embedded 0.2.3 compatibility projects; numerical defaults
  and dependencies are unchanged from 0.2.1.

## 0.2.2 (unpublished)

The immutable tag was retained after the Linux desktop compile check failed.
Version 0.2.3 includes the correction and must pass a new complete release build.

- Open Windows desktop sessions without a console window. Preserve redirected
  diagnostic output and verify the GUI subsystem in the packaged executable.
- Use Ctrl for Windows/Linux command shortcuts and multi-selection, including
  Undo, Open, Save, stage navigation, and clipboard actions. Preserve macOS Cmd
  shortcuts and use each platform's word-navigation keys.
- Reduce accessibility update overhead by sharing the complete activation
  snapshot, comparing nodes by identity, and skipping unchanged native updates.
- Wait for Windows installer diagnostic processes and capture their output.
  Add opt-in graphics-adapter diagnostics and retain native Windows interaction
  checks with measured debug/release timing limits.
- Retain linked and embedded 0.2.2 projects with the existing format-1 state.
  Numerical defaults and solver dependencies are unchanged.

## 0.2.1

- Start in the complete, empty Data workspace and keep Help and theme switching
  accessible. Simplify processing controls, contextual Fit steps, and publication
  previews; synchronize Assistant and structure inputs with the active theme.
- Require explicit main and optional import channels. Add an undoable Remove
  marked action and preview processing operations before applying them.
- Resolve Auto normalization maximum to the spectrum endpoint. Add an optional
  AUTOBK weight link to FFT that preserves the independent value when unlinked.
- Add persistent spectrum color cycles and reversible gradients, plus publication
  presets for XANES, full energy, weighted chi(k), chi(R), and R-space fits.
  Export full CSV arrays alongside 300-DPI figures with Typst labels.
- Add structure center focus and camera-relative depth cues, preserve the bond
  appearance, and reduce structure paint overhead during rotation and zoom.
- Update ruviz and ruviz-gpui to 0.14.1 for consistent font handling, international
  fallback, complete angstrom labels, and PNG strokes without sharp join spikes.
- Add continuous testing for the core plotting feature and retain linked and
  embedded 0.2.1 projects covering weight links, palettes, and publication style.

## 0.2.0

- Unify Groups with durable identities, source/channel stacks, Results, independent
  current/focus/marks, processing locks, and undoable row actions.
- Append imports with per-file receipts and explicit pending-layout review,
  representative selection, partial acceptance, Reload, and Locate.
- Add a focused mapping editor with original table values, full raw μ(E) preview,
  explicit axis conversion, validated channel roles, and unique fluorescence ROIs.
  Apply single or frozen batch repairs as one undoable edit; create only missing
  channels with independent processing settings.
- Persist versioned project/optional machine recipes and exact application members.
  Named compatible layouts import directly; conflicts, changed units, and unnamed
  layouts require review. Saved mappings remain authoritative on reopen.
- Explain known Merge incompatibility before running, retain declared XDI edge
  identities and materialized provenance, and persist historical parser totals
  and bounded line examples with their checked mapping.
- Reuse fixed-penalty column scaling and SVD factors for compatible geometry while
  retaining per-spectrum numerical checks and per-call condition limits.
- Fix Assistant catalog lookup across native Windows path separators; isolate
  FEFF and project-cache roots in storage tests.
- Retain linked and embedded 0.2.0 compatibility fixtures covering Groups, recipes,
  applications, pending sources, declared edges, and saved parser evidence.

## 0.1.4

- Bind tools to the intended spectrum and parameter revision, refuse incompatible
  merges, preserve channel mapping when copying stage settings, and journal
  mapping edits with lock checks and undo. Record result quantities and inputs;
  report parser diagnostics and excluded rows.
- Expose coordination number N for each fitting path and copy fit reports as
  Markdown with parameters, uncertainties and fit statistics.
- Add the experimental Assistant: automatic Codex connection, model/reasoning
  choices, typed transcript with thinking and tool activity, app-authored
  permissions/receipts, web search and structure fetching. Extended access is
  off by default and requires explicit session consent.
- Dock Assistant beside the analysis, resize or pop it out while retaining its
  conversation. Save the newest five conversations per project by default, with
  a configurable limit and saved-thread resume or previous-context fallback.
- Remove the unmaintained derivative macro dependency, pin GitHub Actions to
  commit SHAs, and enable scheduled dependency update PRs.
- Retain linked and embedded 0.1.4 project fixtures covering saved conversations
  and per-path coordination number. Existing format-1 projects remain readable.

## 0.1.3

- Include ReFEFF 0.3.0 and FEFF10 0.2.3 in Mac and Linux builds, retain separate
  calculation sources, and honor cooperative ReFEFF timeouts. Windows uses
  ReFEFF; its MSVC build cannot link the current MinGW FEFF10 prebuilt.
- Verify compiled engines and calculations after archive extraction. Retain
  linked/embedded fixtures and signed, notarized Mac ZIPs and DMG installers.
- Make portable Windows/Linux desktop previews available for platform testing;
  add a per-user Windows setup installer with Unicode-path, payload, shortcut,
  reinstall and uninstall-preservation checks. Native interactive qualification
  for Windows/Linux remains pending.

## 0.1.2

- Use a common χ(k) display weight for comparison overlays and explicitly label
  mixed Fourier weights. Isolate palette input from the underlying plots.
- Expose background k-origin E0, embedded χ standards, advanced solver settings,
  and inverse-transform q range, grid, R weight and independent right taper.
- Correct inverse FFT frequency-bin windowing, full high-R filtering, R weighting,
  resizing and exact q-grid generation in both array backends.
- Add Stable/Nightly release discovery, checksum-verified Mac downloads and machine
  update preferences. Build signed/notarized Nightly apps daily with a separate
  application identity and immutable GitHub prereleases.
- Retain linked and embedded 0.1.2 project samples with new processing controls
  and independent reference-channel identities.

- Import multiple files and folders by appending to the current session; retain
  per-file settings, detect named reference channels, and deduplicate repeated imports.
- Keep sample, fluorescence and reference channels as separate lazy groups with
  independent processing settings, project identities and joint-fit assignments.
  Include all marked additional groups in publication exports with channel provenance.

- Add visible Select all, Deselect all and Invert group controls, with scoped
  keyboard shortcuts and explicit overlay scope.
- Restore per-spectrum project settings across directory aliases and catalog
  refreshes, reload the current plot, and invalidate raw caches when indices change.
- Edit AUTOBK k limits and inverse-transform R limits by dragging plot handles.
  Keep the full measured k range visible while changing the background window.
- Require fit R min to be at least the spectrum's AUTOBK Rbkg, including numeric
  edits, plot drags, restored models, and individual batch/joint solver inputs.
- Expose edge-step overrides, endpoint clamp-point counts and the inverse FFT
  window in the GUI; show when fixed λ is inactive for a legacy clamp model.
- Add flattened normalized μ(E) to Publish as a separate figure using the exact
  library output. Export visible curve data as CSV alongside PNG/SVG figures,
  preserve figure choices across spectrum changes, and flag stale fit figures.

## 0.1.1

- Fix duplicate stationary spectra exposed during fast desktop plot pans by
  upgrading ruviz and ruviz-gpui to 0.13.1. The adapter clears the plot interior
  before compositing the translated preview, including transparent backgrounds.
- Retain linked and embedded 0.1.1 project samples, saved and reopened with the
  current writer; existing format-1 projects preserve their state.
- Use floor when selecting the automatic AUTOBK spline parameter count. For
  rbkg=1 and kmax=12 this selects eight parameters instead of nine. Explicit
  parameter counts remain configurable.
- Use a single linear solve with a configurable [fixed endpoint penalty](doc/autobk-fixed-penalty.md)
  for new AUTOBK analyses (`clamp_lambda = 0.001`, zero disables clamping).
  Match the study's cubic interpolation, floor cutoff, and final endpoint.
  Expose λ in Rust, Python, JavaScript and the desktop inspector. Existing saved
  projects retain their legacy clamp model; new projects persist the new model.
- Update energy/k conversion to CODATA 2022 and share the constants between
  array backends and Athena export.
- Add reproducible [Larch/rexafs benchmark matrices and CPU profiles](doc/benchmarks/2026-09-06-larch/README.md),
  including numerical output comparisons and controlled explanations of AUTOBK
  and Fourier-window differences. Retain a separate [clamp study with known
  synthetic backgrounds](doc/benchmarks/2026-09-07-clamp-study/README.md), direct
  penalty prototypes, profiles and validation of the knot-count change.

## 0.1.0 — release preparation

- Adopt **rexafs** for the Rust crate, Python import, npm package, desktop binary
  and documentation. Preserve xraytsubaki as the development codename.
- Add a shared `Spectrum` workflow across Rust, Python and JavaScript:
  `from_arrays` followed by `normalize()`, `calc_background()` or `fft()`.
  Stages compute missing prerequisites and expose configuration and result getters.
  Validate finite arrays with strictly increasing energy and support an E0 override.
- Add concise Rust `Spectrum`, `Group`, `Error`, `Result` and module entry points.
  Preserve existing advanced APIs and add borrowed `k()` / `chi()` accessors.
- Repair the installed Python module contract, add typed NumPy results and expose
  `rexafs.io.read_qas_transmission`. Remove the unreleased standalone `process`
  facade and legacy Python free pipeline/batch wrappers in favor of `Spectrum`.
- Add browser/Node Wasm packaging and TypeScript declarations, tested from the
  actual npm tarball and in Chromium.
- Upgrade to Rust 1.98.1 and current compatible Rust dependencies; adapt solver,
  ndarray, serialization and PyO3 APIs. Record constrained dependencies separately.
- Read legacy settings when needed, prefer `REXAFS_*`
  variables and retain `XTS_*` fallbacks. Add Windows home/cache handling.
- Fix an ndarray FFT bounds panic for inputs longer than the transform size.
- Remove rusty-fitpack: use local B-splines with a direct QR coefficient solve,
  preserving interpolation boundaries and the existing AUTOBK optimizer.
- Remove GPL desktop tracing dependencies through a documented Apache sum_tree
  patch. Require dependency license checks across all features and platforms.
- Align the FEFF DogLeg parameter tolerance with its numerical Jacobian step to
  prevent near-solution joint fits from stalling at floating-point resolution.
- Add generated rexafs app/release artwork and packaged platform icon resources.
- Keep absorber highlighting in geometric depth order for painting and hit testing.
- Add a publication figure editor with ruviz defaults, size/DPI/label/curve controls,
  PNG/SVG saving and reports with numbered figure/table captions.
- Harden credential settings permissions at creation and repair older Unix files.
- Reject stale FEFF completions after workspace changes and isolate each job’s files.
- Establish `.rxs` as the first release project format, with relative source links
  by default, optional compressed originals and source metadata headers. Remove
  unreleased suffixes. Gate future releases on retained fixture pairs, relocation
  and load/save tests, checked versions, atomic saves and previous-save backups.
- Minimize project JSON without rounding data: omit safe defaults, preserve exact
  floating-point values and validate the reconstructed state before every save.
- Add GitHub multi-platform qualification, portable desktop archives, extracted
  package checks, dependency notices and checksums. Publish from a successful
  GitHub build of the exact version-tag commit and reuse it across channels.

The repository is now [`Ameyanagi/rexafs`](https://github.com/Ameyanagi/rexafs),
with matching package metadata. The `r` stands for Rust and reinventing the wheel.
Publication, platform qualification, distribution license review and domain
deployment are tracked in the [release runbook](doc/releasing.md).
