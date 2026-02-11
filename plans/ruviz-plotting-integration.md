# Plan: Integrate ruviz as Core Plotting Library

## Context

xraytsubaki is a high-performance XAS analysis library that is purely computational — it has no plotting in the core crate. This plan adds ruviz (v0.1.4 from crates.io) as an optional `plotting` feature in the core crate, providing publication-quality XAS visualizations. GUI migration and Python bindings are out of scope for this PR.

Key alignment: both projects use nalgebra 0.32 `DVector<f64>`, and ruviz has `nalgebra_support` which implements `Data1D` for `DVector<f64>` — enabling zero-copy data passing.

---

## Phase 1: Dependency & Module Skeleton

### 1.1 Add ruviz workspace dependency
**File: `Cargo.toml`**
```toml
ruviz = { version = "0.1.4", default-features = false }
```

### 1.2 Add optional dependency + feature flag to core crate
**File: `crates/xraytsubaki/Cargo.toml`**
```toml
[dependencies]
ruviz = { workspace = true, optional = true, features = ["nalgebra_support", "svg"] }

[features]
default = []
ndarray-compat = ["ndarray"]
plotting = ["ruviz"]
```

### 1.3 Create plot module skeleton
**File: `crates/xraytsubaki/src/lib.rs`** — add:
```rust
#[cfg(feature = "plotting")]
pub mod plot;
```

**New files under `crates/xraytsubaki/src/plot/`:**
```
plot/
├── mod.rs              # Module root, re-exports
├── traits.rs           # PlotXAS trait definition
├── builder.rs          # XASPlotBuilder (builder pattern, panel/option chaining)
├── errors.rs           # PlotError type
├── config.rs           # Default axis labels, layout rules, themes
├── spectrum_plots.rs   # Builder rendering logic for XASSpectrum data
├── group_plots.rs      # Builder rendering logic for XASGroup data
├── fitting_plots.rs    # Builder rendering logic for FeffFitResult data
└── panels.rs           # Subplot layout helpers (grid arrangement)
```

### 1.4 Add plot exports to prelude
**File: `crates/xraytsubaki/src/prelude.rs`** — add:
```rust
#[cfg(feature = "plotting")]
pub use crate::plot::{PlotXAS, XASPlotBuilder, PlotError};
```

### Verification
```bash
cargo check -p xraytsubaki --features plotting
```

---

## Phase 2: Core Plotting API

### 2.1 PlotError (`plot/errors.rs`)
Wrap ruviz errors + add `MissingData` variant for when spectrum fields are `None`.

### 2.2 XASPlotConfig (`plot/config.rs`)
```rust
pub struct XASPlotConfig {
    pub width: u32,          // pixels, default 800
    pub height: u32,         // pixels, default 600
    pub kweight: f64,        // k-weighting for chi(k), default 2.0
    pub show_legend: bool,   // default true
    pub show_components: bool, // show re/im in R-space, default false
    pub show_window: bool,   // overlay FT window, default false
}
```

### 2.3 PlotXAS trait + Builder pattern (`plot/traits.rs`, `plot/builder.rs`)

The `PlotXAS` trait provides a single `.plot()` method that returns a builder. The builder lets you chain what to plot, then finalize with `.render()` (returns `ruviz::core::Plot`) or `.save()` (writes to file).

```rust
/// Trait implemented by XASSpectrum, XASGroup, FeffFitResult
pub trait PlotXAS {
    fn plot(&self) -> XASPlotBuilder<'_>;
}
```

**Builder — chain plot types, customize, then render:**
```rust
pub struct XASPlotBuilder<'a> { /* borrows source data */ }

impl<'a> XASPlotBuilder<'a> {
    // --- Panel selectors (each adds a subplot panel) ---
    pub fn mu(self) -> Self;              // energy vs μ(E)
    pub fn norm(self) -> Self;            // energy vs normalized μ(E)
    pub fn k(self) -> Self;               // k vs χ(k)·k^w
    pub fn r(self) -> Self;               // R vs |χ(R)|

    // --- Per-panel options (apply to last added panel) ---
    pub fn kweight(self, w: f64) -> Self;        // k-weighting for k panel
    pub fn components(self, show: bool) -> Self;  // show re/im in R panel
    pub fn edges(self, show: bool) -> Self;       // show pre/post edge lines in norm panel
    pub fn window(self, show: bool) -> Self;      // overlay FT window on k panel
    pub fn stacked(self, offset: f64) -> Self;    // vertical offset (XASGroup)
    pub fn select(self, indices: &[usize]) -> Self; // subset of spectra (XASGroup)

    // --- Global options ---
    pub fn title(self, title: &str) -> Self;
    pub fn width(self, px: u32) -> Self;
    pub fn height(self, px: u32) -> Self;

    // --- Finalize ---
    pub fn render(self) -> Result<ruviz::core::Plot, PlotError>;  // returns ruviz Plot
    pub fn save(self, path: &str) -> Result<(), PlotError>;        // renders + saves
    pub fn to_svg(self) -> Result<String, PlotError>;              // renders to SVG string
}
```

**Layout rules:**
- Single panel call → one plot
- Multiple panel calls → auto subplot grid (e.g., `.mu().k().r()` → 1×3 or 2×2)

### 2.4 Usage Examples

```rust
// === XASSpectrum — single panel ===
spectrum.plot().mu().save("mu.png")?;
spectrum.plot().norm().edges(true).save("norm.png")?;
spectrum.plot().k().kweight(3.0).window(true).save("chi_k.png")?;
spectrum.plot().r().components(true).save("chi_r.png")?;

// === XASSpectrum — multi-panel (auto subplot) ===
spectrum.plot()
    .mu()
    .norm().edges(true)
    .k().kweight(2.0)
    .r()
    .save("overview.png")?;  // 2×2 grid

// === XASSpectrum — get ruviz Plot for further customization ===
let plot = spectrum.plot().mu().render()?;
// ... customize with ruviz API ...
plot.save("custom.png")?;

// === XASGroup — overlay all spectra ===
group.plot().mu().save("all_mu.png")?;
group.plot().norm().save("all_norm.png")?;

// === XASGroup — select subset ===
group.plot().mu().select(&[0, 2, 5]).save("selected.png")?;

// === XASGroup — stacked comparison ===
group.plot().norm().stacked(0.5).save("stacked.png")?;

// === XASGroup — multi-panel ===
group.plot().mu().k().r().save("group_overview.png")?;

// === FeffFitResult — data vs model ===
result.plot().k().r().save("fit.png")?;

// === Generic code ===
fn save_overview(data: &impl PlotXAS) -> Result<(), PlotError> {
    data.plot().mu().k().r().save("overview.png")
}
save_overview(&spectrum)?;  // single trace per panel
save_overview(&group)?;     // overlaid traces per panel
```

### 2.5 Fitting data rendering (`plot/fitting_plots.rs`)

`FeffFitResult` implements `PlotXAS`, so `.plot().k().r().save()` works.
- `.k()` panel shows **data_chi** and **model_chi** as two traces (data=solid, model=dashed)
- `.r()` panel shows **data |χ(R)|** and **model |χ(R)|** as two traces
- `.r().components(true)` adds re/im components for both data and model
- `.k().paths(true)` overlays individual `PathContribution` traces

### 2.6 Subplot layout (`plot/panels.rs`)
- Auto-arranges panels: 1→1×1, 2→1×2, 3→1×3, 4→2×2
- Uses `ruviz::subplots()` / `SubplotFigure` for multi-panel PNG/PDF
- Provides individual SVG fallback for GUI embedding (each panel as separate SVG)

---

## Implementation Order

1. **Phase 1** — Dependency setup + module skeleton + `cargo check`
2. **Phase 2.1–2.3** — PlotError, config, PlotXAS trait + XASSpectrum impl (core 4 plot types)
3. **Phase 2.3 cont.** — XASGroup impl
4. **Phase 2.4** — Fitting plots
5. **Phase 2.5** — Multi-panel composites

> GUI migration and Python bindings are deferred to follow-up PRs.

---

## Key Files to Modify

| File | Change |
|------|--------|
| `Cargo.toml` (workspace) | Add `ruviz` workspace dep |
| `crates/xraytsubaki/Cargo.toml` | Add optional `ruviz` dep + `plotting` feature |
| `crates/xraytsubaki/src/lib.rs` | Add `#[cfg(feature = "plotting")] pub mod plot;` |
| `crates/xraytsubaki/src/prelude.rs` | Re-export plot types under `plotting` gate |
| `crates/xraytsubaki/src/plot/mod.rs` | Module root + re-exports |
| `crates/xraytsubaki/src/plot/traits.rs` | PlotXAS trait definition |
| `crates/xraytsubaki/src/plot/builder.rs` | XASPlotBuilder (core builder logic) |
| `crates/xraytsubaki/src/plot/errors.rs` | PlotError type |
| `crates/xraytsubaki/src/plot/config.rs` | Axis labels, layout rules, themes |
| `crates/xraytsubaki/src/plot/spectrum_plots.rs` | Rendering for XASSpectrum data |
| `crates/xraytsubaki/src/plot/group_plots.rs` | Rendering for XASGroup data |
| `crates/xraytsubaki/src/plot/fitting_plots.rs` | Rendering for FeffFitResult |
| `crates/xraytsubaki/src/plot/panels.rs` | Subplot layout helpers |

## Existing Code to Reuse

- `XASSpectrum` fields (`energy`, `mu`, `k`, `chi`, `chi_kweighted`, `chi_r_*`, `norm`, `flat`) — `crates/xraytsubaki/src/xafs/xasspectrum.rs:40-58`
- `PrePostEdge` struct with `pre_edge`, `post_edge` vectors for overlay lines — `normalization.rs:147-162`
- `FeffFitResult` / `DatasetResult` / `PathContribution` with `data_chi`, `model_chi`, `data_chir_*`, `model_chir_*` — `fitting/types.rs:658-787`
- `XASGroup.spectra: Vec<XASSpectrum>` — `xasgroup.rs:55-57`

## Verification

```bash
# Phase 1: Compilation
cargo check -p xraytsubaki --features plotting

# Phase 2: Tests
cargo test -p xraytsubaki --features plotting

# Phase 2: Visual smoke test — run an example that loads test data and saves plots
# (create examples/plot_demo.rs that loads from tests/testfiles/ and saves PNG/SVG)
```
