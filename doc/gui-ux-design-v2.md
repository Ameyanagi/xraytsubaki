# rexafs GUI — UX / UI Design v2 (proposal)

Historical design record. For the current interface, start at the [documentation index](README.md).

Status: **proposal, 2026-09-02** — awaiting review. Supersedes the shell/workspace
parts of `gui-ux-design.md` (2026-06-10); the scaling/async principles of that
document still apply and are not repeated here.

Interactive prototype (real Ru K-edge data from `Ru_QAS.dat`, draggable
ranges, all six stages): see the "rexafs Studio" artifact linked from the
session that produced this document.

---

## 1. Why a v2

The v1 shell organizes the app around three *jobs* (Explore / Operando / Fit).
That matches how the code is structured, not how analysts think. Every
reference tool — Athena, Artemis, Larix, Fastosh, ProQEXAFS — and every
tutorial presents XAS analysis as a **pipeline of stages** whose parameters
ripple downstream:

    import → calibrate/align → normalize → background (AUTOBK) → FT (k→R, R→q) → fit / LCF / PCA
                                        ↑ operando series apply the same pipeline to N spectra

The audit of the current GUI (2026-09-02) found the concrete symptoms of the
mismatch:

- The Explore context panel is a single tall stack of Import/Norm/Bkg/FFT
  fields; the four plots are always shown but none is *the* plot for the
  parameter being edited. Users read numbers from a plot and type them into
  a field (Athena has "pluck" for this; we have nothing).
- Edit scope is implicit. Whether an edit hits one spectrum, the selection,
  or the global defaults depends on a small `global/override` chip.
- Athena's most-used idiom — *current* group vs *marked* groups, with plot
  buttons that act on either — has no equivalent. Comparison is a side effect
  of multi-select.
- Half the core library is not reachable: back-transform to q, edge-step
  override, standard-constrained AUTOBK, joint fits, multi-k-weight fits
  (core has only one k-weight; Artemis defaults to 1,2,3 simultaneously),
  path parameters `ei/third/fourth`, correlation matrix, BSON/JSON export.
- Missing entirely (core and GUI): merge, calibrate, deglitch, truncate,
  rebin (core `todo!`), smooth UI, self-absorption, LCF, PCA, difference
  spectra — i.e. most of Athena's Data and Analysis menus.
- No undo, no command palette, no menus, no journal. `app.rs` is 8 k lines.

## 2. What the field expects (research summary)

Verified against the Athena/Artemis guides, Demeter's shipped defaults,
Larch/Larix docs and source, and the Fastosh/ProQEXAFS/autoXAS papers.

**Mental model to keep (users know it from 15 000+ Athena citations):**
- A **group** = one spectrum + its parameters. The **current** group fills the
  parameter panel; **marked** groups (checkbox) are what bulk actions and
  overlay plots act on. Athena colors this orange (current) / purple (marked).
- **Pluck / pin**: click a field, click the plot, the x-value fills the field.
- Right-click a parameter → *set marked / set all / reset to default*.
- **Frozen** groups are skipped by bulk operations.
- The **quad plot** (E, k, R, q) is the default view after import.
- k-weight buttons 0/1/2/3 affect plotting and FT only, not AUTOBK.
- **GDS** parameters (`guess / def / set`) referenced by expressions in path
  cells; fit **history** with restore; correlations ≥ 0.4 reported; a fit
  "happiness" traffic light.
- Plot conventions: |χ(R)| for display, |χ(R)| + Re[χ(R)] for fits ("Rmr"),
  residual offset below, path contributions stacked below the data, fit
  window shown; axis labels `k (Å⁻¹)`, `k²χ(k) (Å⁻²)`, `|χ(R)| (Å⁻³)`.

**Defaults to ship (Athena unless noted):** pre-edge −150…−30 eV; norm
150 eV…end; quadratic post-edge; E₀ = max dμ/dE; Rbkg 1.0; AUTOBK k-weight 1
(Larch) or 2 (Athena), clamps none/strong; FT k 3…kmax−2, Hanning, dk 1,
k-weight 2, rmax 10; back-FT R 1…3, dr 0; fit: R-space, k-weights 1+2+3,
Nvar ≤ ⅔ Nidp with Nidp = 2ΔkΔR/π.

**Pain points to avoid:** window sprawl (Artemis), plot-button × option ×
scope combinatorics (Athena), silently switching panels when the current
group changes, unclear edit scope, three different k-weights (bkg/plot/fit)
that look the same, no undo, opaque project files, batch/series bolted on.

**Modern additions worth copying:** Larix's per-group journal and replayable
buffer, parameter presets, dark mode; Fastosh's time-ordered matrix view
(heatmap, chunk merge, trends, MCR-ALS); Viper's "every stage visible at once
while you drag".

## 3. Design principles for v2

1. **The pipeline is the navigation.** Six stages across the top:
   `Data · Normalize · Background · Transform · Fit · Series`. Selecting a
   stage sets *the* plot(s) and *the* parameters. Each stage tab carries a
   status dot (ok / auto / needs attention) and a one-line summary
   (`E₀ 22118.8 · step 0.86`) so the whole state is readable at a glance.
2. **Direct manipulation beats typing.** Every range parameter is a shaded
   region with draggable edges on its own plot: pre-edge and norm ranges and
   the E₀ marker on μ(E); Rbkg on |χ(R)|; kmin/kmax with the window drawn on
   χ(k); the back-FT R-window; the fit k- and R-ranges. Dragging updates the
   field live and vice-versa. This replaces pluck.
3. **Ripple is always visible.** A four-thumbnail strip below the main plot
   (norm μ(E) → χ(k) → windowed k²χ(k) → |χ(R)|) updates with every change
   and doubles as a stage switcher. This is Viper's idea and our quad plot.
4. **Explicit scope.** The inspector header reads "Editing *Ru foil*" and
   offers *Apply to marked (n)* and *Reset*. The plot bar has a
   `current | marked (n)` switch. Frozen groups show a lock.
5. **One window.** Groups left, plot centre, inspector right, status bar. No
   floating windows; tools (align, deglitch, …) open as a preview mode on the
   plot with Apply / Cancel — never destructive.
6. **Reproducible.** Every applied change appends to a journal (status bar
   "Journal · 14 steps"), which is also the undo stack and the batch recipe
   for Series.
7. **Series is a first-class stage**, not a separate app: same pipeline,
   applied to a time-ordered matrix, with heatmap, cursor, trends, batch
   with preview/cancel/partial results.

## 4. Shell

```
┌ rexafs › project.rxs                        [⌘K search actions…]  ↶ ↷ ☾ ┐
├ 1 Data ● | 2 Normalize ● E₀… | 3 Background ● | 4 Transform ● | 5 Fit ● | 6 Series ● ┤
├──────────┬───────────────────────────────────────────┬──────────────────┤
│ GROUPS   │ plot bar: current|marked · μ/norm/flat ·   │ Editing <group>  │
│ ☑ ■ name │           derivative · pre/post · offset   │ [Apply to marked]│
│ ☑ ■ name │ ┌───────────────────────────────────────┐ │ ─ Edge ──── auto │
│ ☐ ■ name │ │  main plot for the stage              │ │  E₀   [22118.8]  │
│ ☐ ■ …    │ │  shaded draggable ranges, E₀ handle   │ │  step [0.8697]   │
│          │ │  hover crosshair + readout            │ │ ─ Pre-edge line  │
│          │ └───────────────────────────────────────┘ │  start [-150] eV │
│ Merge    │ ┌ norm μ(E) ┐┌ χ(k) ┐┌ k²χ·win ┐┌ |χ(R)| ┐ │  …               │
│ Align…   │ └───────────┘└──────┘└─────────┘└────────┘ │ ─ Result ─────── │
├──────────┴───────────────────────────────────────────┴──────────────────┤
│ ● Idle · 3 marked · current: Ru foil          Journal · 14 steps  cache … │
└───────────────────────────────────────────────────────────────────────────┘
```

- Top bar: project name + save state, ⌘K command palette (actions, groups,
  parameters), undo/redo, theme.
- Stage strip: `⌘1…⌘6`. Thumbnails: click to jump.
- Groups panel (248 px): filter, list rows = mark checkbox · colour swatch ·
  name · meta (points / "2 scans" / "240 frames"), derived groups indented
  (`↳ merge: …`), series groups distinguished. Bottom: Merge marked, Align…,
  Compare (toggles the plot scope). Keyboard: ↑/↓ change current, Space
  marks, ⌘A / ⇧⌘A mark all / none.
- Inspector (312 px): sticky header with scope + Apply/Reset; sections in
  pipeline order for the stage; a *Result* card with the derived numbers
  (E₀, edge step, white-line, first-shell peak, Nidp …). Numeric fields:
  monospace, right-aligned, stepper arrows, ↑/↓ keys, `auto` shown in accent
  colour with an `↺ auto` reset; units outside the box; commit on Enter/blur.
- Status bar: job state, selection summary, journal, cache budget, core
  version/FEFF backend.

## 5. Stages

| Stage | Main plot(s) | Draggable | Inspector |
|---|---|---|---|
| **Data** | μ(E) / norm / flat, derivative toggle, pre/post lines | — | Import mapping (mode, energy, I₀/I<sub>t</sub>, reference; detected header shown), **Processing tools** list (align, calibrate, deglitch, truncate, rebin, smooth, self-absorption, merge, difference), metadata |
| **Normalize** | same plot with pre-edge (blue) and norm (orange) regions, E₀ line | pre1/pre2, nor1/nor2, E₀ | Edge (E₀ auto/override, edge step), pre-edge line (start/end/Victoreen), normalization (start/end/order), result |
| **Background** | μ(E) + spline μ₀(E) over |χ(R)| with `R < Rbkg` region; or χ(k) view | Rbkg, spline kmin/kmax (also drawn on μ(E) via k→E) | AUTOBK (Rbkg, k range, k-weight, knots), clamps & window, standard χ(k), solver + timing |
| **Transform** | k²χ(k) with window curve + FT range; χ(R) mag/Re/Im; χ(q) overlay; "k + R" split | kmin/kmax, back-FT rmin/rmax | Forward FT (k range, dk, window, rmax, k-weight 0-3), Back FT (R range, dR), result (first-shell peak, Nidp) |
| **Fit** | k²χ(k) data/fit/residual; |χ(R)| data/fit/paths/residual (Re/Im toggle) | fit k-range, fit R-range | Paths table (enable, Reff, N, S₀²/ΔE₀/ΔR/σ² cells holding names or expressions), Parameters table (guess/def/set, value ± σ, bounds), fit settings (ranges, space R/k/q, window, k-weights 1·2·3 multi), result card with warnings (correlation > 0.9), history/journal |
| **Series** | heatmap (frames × E/k/R) with cursor row; frame spectrum vs references; trends (E₀ shift, LCF fraction, fitted N…) | cursor, ⇧-drag time range | Cursor card (frame, time, T, E₀, white line; "Open frame in Normalize"), trends table, batch (scope, preview every Nth, progress, cancel keeps partials) |

Plot bar per stage: quantity segment (μ/norm/flat; μ₀/χ(k)/|χ(R)|; k/R/q/k+R),
k-weight 0-3, Re/Im chips, window chip, derivative chip, `offset` for stacked
comparison, legend, Export ▾ (PNG/SVG/CSV of what is shown).

## 6. Visual system

- Type: IBM Plex Sans for chrome, IBM Plex Mono (tabular) for every number.
  12.5 px base, 26–28 px rows.
- Dark (instrument) and light (publication) themes from one token set:
  `bg / surface / raised / border / text / muted / accent / ok / warn / err /
  plot-bg / grid / axis / region / handle`.
- Trace colours: validated colour-blind-safe categorical set (blue, orange,
  aqua, yellow, magenta, violet) stepped per theme; fit = orange, residual =
  grey dashed, paths = following slots. Group colour is stable per group.
- Semantic dots: ok (green) / auto (accent) / attention (amber) — used on
  stage tabs, thumbnails, and results.
- Handles: small rounded tabs at the top of each region edge, grow on hover,
  cursor `ew-resize`; crosshair + readout tooltip everywhere else.

## 7. Toolkit: GPUI + ruviz-gpui 0.12 (decision)

The deciding capability is **direct manipulation on plots** (regions,
handles, hover readout, heatmap cursor). The workspace still pins
`ruviz`/`ruviz-gpui` **0.5.0**; the current release is **0.12.0**
(2026-08-22, same zed gpui rev `3060e417…`, so the pin does not move) and it
closes the wishlist from ruviz issue #97. What 0.12 gives us, mapped to the
design:

| Design need | ruviz-gpui 0.12 API |
|---|---|
| pixel ↔ data mapping | `RuvizPlot::data_at(window_pos)` / `screen_at(data_pos)` |
| hover readout, click-to-pluck | `plot_builder(..).on_plot_hover(..)` / `.on_plot_click(..)` → `PlotPointerEvent { data_position, hit, mouse_button, viewport }` |
| draggable range regions and E₀ / Rbkg lines | session annotations `Annotation::{VSpan, HSpan, VLine, HLine, Text}` with `add_annotation` / `update_annotation` / `remove_annotation` (overlay-only invalidation, no base re-render) |
| live parameter edits without losing zoom | `set_plot_keep_view` |
| series toggling from the legend | `hit_test`, `series_visible(index, bool)`, `legend_entry_at` |
| heatmap with physical axes and cursor | `HeatmapConfig::extent(..)`, `HeatmapOrigin`, heatmap hit testing |
| tooltips led by series label | built-in hover tooltip (0.11) |

One gap remains: left-drag is bound to pan (`ActiveDrag::LeftPan`). Handle
dragging needs either `InteractionOptions { pan: false, .. }` while a handle
is armed (drive it from hover events with `mouse_button == Some(Left)`), or a
small ruviz-gpui addition — "annotation drag capture" — which is the cleaner
fix and worth filing upstream.

Migration step before M1: bump the workspace to ruviz 0.12 / ruviz-gpui 0.12
and fix the API changes since 0.5 (`PlotData::Static(Arc<Vec<f64>>)`,
`ComputedStyle::patch_edge_color`, `HeatmapConfig::origin`, bar edge
defaults). The core `plot/` module (feature `plotting`) needs the same bump.

## 8. Core work the GUI depends on

Blocking for the stages above (in priority order):

1. `XASGroup::merge` (with standard deviation), energy calibrate/align
   helpers (currently GUI-side), `rebin` (core `todo!`), deglitch/truncate.
2. Fit: multiple k-weights (`kweight: Vec<f64>` in `FeffFitTransform`), k-
   and q-space fit spaces, `FeffFlavor` from the runner that produced the
   paths, expose `ei/third/fourth`.
3. LCF (normalized/derivative/χ(k) spaces, sum-to-one, per-standard ΔE₀,
   combinatorial), then PCA.
4. Self-absorption correction; smoothing UI over existing `smooth()`.
5. Athena `.prj` import/export (test file already in `tests/testfiles`),
   BSON/JSON export wired to the GUI.

## 9. Suggested milestones

- **M0 ruviz 0.12 migration** — bump `ruviz`/`ruviz-gpui` 0.5 → 0.12 in the
  workspace, port the core `plot/` module, verify gpui still resolves once.
- **M1 Shell & Normalize** — stage strip, groups panel with current/marked,
  inspector, journal/undo, μ(E) plot with draggable ranges, thumbnails.
- **M2 Background & Transform** — Rbkg/k-range handles, window overlay,
  Re/Im/q views, k-weight keys, Apply-to-marked, presets.
- **M3 Data tools** — import mapping UI, align/calibrate/merge/deglitch/
  truncate/rebin as preview tools; `.prj` import.
- **M4 Fit** — paths/parameters tables, multi-k-weight, history, happiness,
  correlation warnings, report export.
- **M5 Series** — heatmap stage selector, trends, batch with preview.
- **M6 Analysis** — LCF, PCA, self-absorption.

## 10. Open questions for review

1. Confirm the visual direction from the prototype (density, dark-first
   instrument look, IBM Plex type) before M1 starts.
2. Should Series replace the current Operando workspace entirely, or keep a
   separate large-catalog browser for 10⁶-file directories?
3. Athena `.prj` compatibility: import only, or round-trip?
4. Priority between Fit (M4) and Data tools (M3) — beamline QC needs M3
   first; structural work needs M4.
