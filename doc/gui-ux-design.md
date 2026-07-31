# xraytsubaki GUI — UX / UI Design

Status: agreed 2026-06-10. This document is the reference for all GUI milestones;
the implementation plan (milestones M0–M6) builds toward this design.

## Users and workflows

The app serves three distinct jobs, and the shell is organized around them:

1. **Explore** — load a spectrum (or a few), tune normalization / AUTOBK / FFT
   parameters, and watch every stage of the pipeline react live. The mental
   model is Athena's, modernized: parameters on the right, plots in the middle,
   instant feedback.
2. **Operando** — open a scan of 10³–10⁶ spectra from a time-series /
   operando experiment, get an overview (heatmap of time × energy/k/R), scrub
   through time, and spot where chemistry happens.
3. **Fit** — define a FEFF path model, fit one representative spectrum
   interactively, then batch-fit an entire scan and study fitted parameters as
   a function of time.

## Shell layout

```
┌──┬─────────────┬──────────────────────────────┬───────────────┐
│▣ │ DATA        │  workspace content           │ CONTEXT PANEL │
│Ex│ ▾ scan_001  │  (per-workspace)             │ (per-         │
│Op│   sp_00001  │                              │  workspace)   │
│Ft│  ▸sp_00002  │                              │               │
│  │ 1,248,332   │                              │               │
├──┴─────────────┴──────────────────────────────┴───────────────┤
│ ● job/status line                jobs:N   mem   selection     │
└────────────────────────────────────────────────────────────────┘
```

- **Icon rail** (far left, ~48 px): Explore / Operando / Fit workspace
  switcher + settings at the bottom. Active workspace highlighted with the
  accent color. Keyboard: `⌘1/⌘2/⌘3`.
- **Data panel** (~240 px, collapsible `⌘B`): shared across workspaces.
  Two tabs: **Files** (virtualized flat list of the catalog) and **Scans**
  (groups; expanding shows members). A live counter at the bottom shows
  catalog size and scan progress while background indexing runs. Clicking an
  entry lazily loads + processes it. Multi-select (⇧/⌘ click) feeds batch
  operations and comparison overlays.
- **Workspace content** (center, flexible).
- **Context panel** (~280 px, collapsible `⌘J`): parameters in Explore,
  scan/overview controls in Operando, model + variables in Fit.
- **Status bar**: latest job message, running-job count, memory use,
  current selection. Errors surface here non-modally (click to expand a log).

## Explore workspace

- Center is a **2×2 grid**: μ(E) raw+pre/post edges · normalized/flattened
  μ(E) · k-weighted χ(k) · |χ(R)| (+Re part toggle). All four react to every
  parameter change so a tweak's ripple through the pipeline is visible.
- **Maximize**: clicking a quadrant title (or keys `1–4`) zooms that plot to
  the full center area; `0` or Esc returns to the grid. Each plot is a
  ruviz-gpui interactive plot (pan/zoom/hover/save PNG via context menu).
- Context panel sections (top→bottom = pipeline order): **Normalization**
  (E0 + auto/manual toggle, pre-edge range, norm range), **Background**
  (rbkg, k-range, kstep, spline knots, solver), **FFT** (k-window, dk,
  k-weight, window function, rmax). Numeric fields commit on Enter/blur;
  recompute is debounced (~200 ms) and runs in the background — stale results
  are discarded, never block typing.
- Per-spectrum override vs. global defaults: a small chip on each section
  header shows `global` / `override`; editing while a spectrum is selected
  creates an override, with "reset to global" on the chip.
- Multi-selection overlays traces (matplotlib default color cycle) for direct
  comparison; the params panel then edits the global set.

## Operando workspace

- Center-left: **heatmap** (time index × x-axis of the chosen stage: E, k, or
  R; stage selector above). Downsampled (≤512×512) and computed in the
  background with a progress overlay; recomputed when params change.
- A **time cursor** line on the heatmap + slider beneath; dragging scrubs.
- Center-right: the spectrum at the cursor time (stage-matched plot), and
  below it a **trend strip** — edge position / whiteline intensity, or after
  batch fitting, any fitted parameter vs. time.
- Clicking a time row selects that spectrum (syncs with data panel and
  Explore).

## Fit workspace

- Center: data vs. model overlay in k and in R (side by side; toggle to
  stack), with toggleable per-path contribution traces; below, a residual
  strip. Fit ranges (kmin–kmax, rmin–rmax) shown as shaded regions on the
  plots, editable from the panel.
- Context panel: **Paths** (imported FEFF .dat files; each path row expands to
  its s02/e0/Δr/σ² cells — a cell holds a number or an expression string),
  **Variables** table (name, value, vary ✓, min/max, expr; auto-populated
  from expressions), **Ranges & weights**, then a **Fit** button group:
  *Fit selected* / *Batch fit scan…*.
- Results: parameter ± stderr list with R-factor and reduced χ² up top;
  warnings inline. Batch fits stream into a results table (one row per
  spectrum) + trend plot in Operando; export CSV.

## Visual design

- **Theme system from day one**: a `Theme` struct of semantic tokens
  (bg/surface/raised, border, text primary/muted, accent, success/warn/error,
  plot canvas + grid + trace cycle), with **Dark** (default; slate
  instrument-style, accent ~#5BA9F7) and **Light** (publication-style; white
  plot canvases, matplotlib-like) presets. Toggle in settings; plots restyle
  with the theme.
- Typography: system UI font for chrome; monospace for numbers in tables and
  the status bar. Plot trace colors follow the matplotlib default cycle in
  both themes.
- Density: compact (26 px rows) — this is an instrument tool, not a consumer
  app.

## Scaling UX: a million files must feel small

The defining requirement: catalogs of 10⁶ spectra must feel as responsive as
ten. Concretely:

- **Open is instant.** Choosing a directory returns control immediately; the
  index builds in the background, streaming counts into the data panel
  ("412,331 files · indexing…"). The user can click and analyze the first
  spectrum while the scanner is still walking. Indexes persist next to the
  project file, so reopening a million-file project is < 1 s, with a
  background freshness re-check.
- **Nobody scrolls a million rows.** The flat Files list is virtualized but
  it is the *fallback*. The primary unit is the **scan**: one row per scan
  ("scan_017 · 48,201 spectra · 09:12–14:47") with member counts, expandable
  on demand. Navigation inside a scan happens through the **heatmap and the
  time slider**, not the list — the overview *is* the scrollbar.
- **Search-first.** `⌘P` focuses a filter box with fuzzy matching on
  path/scan name and index-range syntax (`scan_01[100..500]`, `*.dat 22000+`).
  Filtering operates on the in-memory index — results update per keystroke
  even at 10⁶ entries.
- **Selections are expressions, not click lists.** Besides ⇧/⌘-click, the
  selection bar offers: whole scan, time range (drag on the heatmap), every
  N-th, and "current filter results". A selection chip shows
  "scan_017 [every 10th] · 4,820 spectra" and is what batch operations
  consume. ⌘-clicking a million rows is not a workflow.
- **Preview before commit.** Any batch operation (recompute, batch fit,
  export) first shows its scope ("4,820 spectra, est. ~12 s") and runs on a
  small evenly-spaced sample when a "preview" toggle is on, so parameter
  mistakes cost seconds, not hours.
- **Everything long-running is cancellable and partial.** Progress bars with
  throughput and ETA; cancel keeps completed results; failures are collected
  per-spectrum (core's `BatchProcessError`) into a filterable problems list —
  one bad file in 10⁵ never aborts the run, and the user can see exactly
  which rows failed and why.
- **Memory is bounded and visible.** Raw and processed caches are LRU with
  fixed budgets; the status bar shows occupancy ("cache 3.1/4 GB"). Browsing
  a million files never grows memory past the budget — eviction is silent and
  re-loading is cheap.
- **Keyboard scrubbing.** In Operando: ←/→ step one time index, ⇧←/→ step
  ~1 %, Home/End jump to extremes — processed frames around the cursor are
  prefetched so stepping feels instant.

## Interaction principles

- **Never block the UI**: every parse/compute/fit is a background job with a
  generation counter; the latest request wins. The status bar always shows
  what is running.
- **Lazy by default**: opening a directory only indexes it; nothing is parsed
  until shown. Caches (raw LRU, per-stage processed LRU keyed by parameter
  fingerprints) make revisits instant.
- **Progressive disclosure**: Explore needs zero setup; Operando appears once
  a scan grouping exists; Fit's complexity stays in its own workspace.
- **Everything visible is exportable**: plot context menu → save PNG/copy;
  results table → CSV; processed spectra → BSON/JSON via core.
