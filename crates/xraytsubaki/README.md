# xraytsubaki: Fast XAS Data Analysis Tool

xraytsubaki is a Rust-based program that implements the core functionalities of [xraylarch](https://xraypy.github.io/xraylarch/). The primary aim of this project is to expedite the processing of extensive XAS data sets. The project's name, xraytsubaki, draws inspiration from [tsubaki](https://en.wikipedia.org/wiki/Camellia_japonica).

## Project Genesis and Objectives

The inception of this project was triggered when I needed to process over 1000 spectra from in-situ measurements. The data loading and processing in xraylarch were too time-consuming, not to mention also for demeter. The goal was to develop a tool capable of processing data within a reasonable timeframe. While this project does not seek to replace xraylarch, it does aim to provide a phenomenally fast core API for xraylarch's backend to augment its capacity.

Additionally, this project seeks to leverage Rust's ecosystem to create a generalized library compatible with other languages such as Python and Javascript. This will facilitate a shift away from exclusive Python-based analysis.

## Key Features

- [x] Standard EXAFS analysis (find_e0, preedge postedge normalization, AUTOBK, FFT, IFFT)
- [x] Parallel processing using Rayon. (For example, M1 Macbook Pro with 10 cores can process 10000 spectra in 20 seconds, which is ~x10 enhancement without parallelization. Numpy + xraylarch takes 145 seconds.)
- [x] Optimization on AUTOBK. The AUTOBK process were optimized with providing an analytical Jacobian to speed up the minimization process by Leverberg-Marquardt algorithm.
- [x] FEFF85L path-based EXAFS fitting in Rust core (`xfeffdat` parsing, `path2chi`, `ff2chi`, single-dataset R-space fit with shared expression variables).
- [x] FEFF85L pure-Rust module execution workflow (`resolve_feff_commands`, `run_feff`, `run_feff_and_load_paths`) starting from a provided FEFF executable path.
- [x] Optional FEFFRS pipeline backend (`FeffExecutionMode::Feff10Pipeline`, feature `feff10-runner`) using the published prebuilt FEFF modules.
- [x] Optional pure-Rust ReFEFF backend (`FeffExecutionMode::RefeffPipeline`, feature `refeff-runner`) using ReFEFF's EXAFS-only in-memory API.

## FEFF Fitting MVP Boundaries

- Rust core only in this release (`crates/xraytsubaki`).
- FEFF85L execution path supports deterministic module resolution (`feff8l_rdinp`, `feff8l_pot`, `feff8l_xsph`, `feff8l_pathfinder`, `feff8l_genfmt`, `feff8l_ff2x`) and output discovery.
- FEFFRS and ReFEFF are independent feature flags and can be enabled separately or together.
- Both execution paths auto-enforce `PRINT` `ipr6 >= 3` to ensure `feffNNNN.dat` output generation.
- ReFEFF is compiled with its minimal `exafs` engine feature and persists only fitting path files by default. Set `FeffRunRequest::keep_all_outputs` to write its complete EXAFS artifact set.
- Existing parse-only workflows with pre-generated `feffNNNN.dat` files remain supported.
- Single-dataset R-space fitting only.
- `FeffFlavor::Feff10` parsing remains reserved and returns typed unsupported errors in this MVP.

See `crates/xraytsubaki/doc/feff-fitting-mvp.md` for details and FEFF10 follow-up compatibility notes.

## Future Developments

- [ ] EXAFS helper funtions (rebinning and more)
- [ ] Develop a Python wrapper for the library.
- [ ] Develop a web assembly version of the library for web application usage.

## Licensing

(To be completed...)

## Plotting (Feature-Gated)

Core plotting is available behind the `plotting` feature using `ruviz`.

```bash
cargo run -p xraytsubaki --features plotting --example plot_demo
```

On Apple Silicon, if your linker resolution requires an explicit target linker:

```bash
CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER=clang cargo run -p xraytsubaki --features plotting --example plot_demo
```

`plot_demo` writes outputs to:
- `crates/xraytsubaki/target/plot_demo`

`plot_demo` coverage:
- FEFF85L module runs from full `feff.inp`: `Co`, `FeO_withPb`, `MnO2`, `ZnSe`
- Real fitting via `FeffFit::fit()`: `Cu`, `ZnSe`
- Fit plots per material: `k`, `k + window`, `r`, `r + window range`

To regenerate Cu/ZnSe fit references directly from XrayLarch:

```bash
uv run --with xraylarch python crates/xraytsubaki/scripts/generate_larch_fit_references.py
```

Strict FEFF fit parity is regression-tested against these regenerated Cu/ZnSe fixtures:
- compared fields: `amp`, `de0`, `sig2`, `dr` values and `stderr`
- compared stats: `chi_square`, `reduced_chi_square`, `n_idp`, `r_factor`
- tolerance policy: relative tolerance `20%` with absolute fallback `1e-8` (`de0` value uses `0.2 eV` absolute fallback near zero)

### Important behavior

- Plotting APIs are available through `PlotXAS` with a mutable entrypoint: `plot(&mut self)`.
- Plot text rendering uses `typst(true)` by default for scientific notation-friendly labels/ticks.
- Plotting auto-computes missing intermediates when required:
  - `mu()` may call `normalize()` and renders flattened `mu(E)` by default
  - `norm()` may call `normalize()`
  - `k()` may call `calc_background()`
  - `r()` may call `calc_background()` and `fft()`
- `k()` panels use symmetric y-limits (`-y_lim..y_lim`) and y-axis units derived from `kweight`.
- `FeffFitResult::plot().k()` defaults to fit/dataset `kweight` unless `.kweight(...)` overrides it.
- `r()` panels default to `xlim(0.0, 6.0)`.
- `r()` defaults to magnitude traces. Calling `.real()` and/or `.imag()` switches to those components unless `.mag()` is also included (e.g. `.r().mag().real().imag()`).
- `FeffFitResult::plot().r()` includes path `|chi(R)|` traces when magnitude is active.
- Window overlays are disabled by default.
- `.window(true)` is an alias that enables both `.window_fn(true)` and `.window_box(true)` for `k()` panels.
- `.window_fn(...)` is supported only on `k()` panels.
- `.window_box(...)` is supported on `k()` panels, and on `r()` panels for `FeffFitResult` plots; it renders two range markers (min/max), not a rectangle.
- `FeffFitResult` now includes `varying_names`, `covariance`, and `correlation` (matrix order follows `varying_names`).
- Multi-panel output is PNG-only in this phase.

### XASSpectrum examples

```rust,no_run
use xraytsubaki::prelude::*;
use xraytsubaki::xafs::io::load_spectrum_QAS_trans;

let path = format!("{}/tests/testfiles/Ru_QAS.dat", env!("CARGO_MANIFEST_DIR"));
let mut spectrum = load_spectrum_QAS_trans(path)?;

spectrum.plot().mu().save_png("flat_mu.png")?;
spectrum.plot().norm().edges(true).save_png("norm_edges.png")?;
spectrum.plot().k().kweight(2.0).window(true).save_png("chi_k.png")?;
spectrum.plot().r().save_png("chi_r_mag.png")?;
spectrum.plot().r().real().save_png("chi_r_real.png")?;
spectrum.plot().r().mag().real().imag().save_png("chi_r_all.png")?;

spectrum
    .plot()
    .mu()
    .norm()
    .k()
    .r()
    .title("overview")
    .save_png("overview.png")?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

### XASGroup examples

```rust,no_run
use xraytsubaki::prelude::*;

let mut group = XASGroup::new();
// populate group.spectra ...

group.plot().mu().save_png("group_overlay.png")?;
group.plot().mu().select(&[0, 2]).save_png("group_selected.png")?;
group.plot().mu().stacked(0.25).save_png("group_stacked.png")?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

### FeffFitResult examples

```rust,no_run
use xraytsubaki::prelude::*;

let mut fit = FeffFitResult::default();
// populate fit result vectors or datasets ...

fit.plot().k().save_png("fit_k.png")?; // uses fit kweight by default
fit.plot().k().window(true).save_png("fit_k_window.png")?; // with window
fit.plot().r().save_png("fit_r.png")?; // includes path |chi(R)| traces
fit.plot().r().window_box(true).save_png("fit_r_window.png")?; // with range markers
fit.plot().r().real().save_png("fit_r_real.png")?;
fit.plot().r().mag().real().imag().save_png("fit_r_all.png")?;
fit.plot().k().dataset(0).save_png("fit_dataset0_k.png")?;
# Ok::<(), Box<dyn std::error::Error>>(())
```
