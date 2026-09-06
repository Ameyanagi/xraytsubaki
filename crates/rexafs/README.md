# rexafs for Rust

Rust-powered X-ray absorption analysis, developed under the codename xraytsubaki.
The core includes normalization, AUTOBK, Fourier transforms, group processing,
EXAFS fitting, structure handling, LCF/PCA and spectrum tools.

Publication on crates.io is pending. In this checkout, run `cargo test -p rexafs`.
After publication, add the library with `cargo add rexafs`.

## Start with a spectrum

```rust,no_run
use rexafs::{io, Spectrum};
let mut spectrum = io::read_qas_transmission("scan.dat")?;
spectrum.fft()?;
assert_eq!(spectrum.k().unwrap().len(), spectrum.chi().unwrap().len());
// For your own data: Spectrum::from_arrays(&energy, &mu)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

`fft()` calculates missing normalization and background results using the selected
methods and defaults. `normalize()`, `calc_background()`, `fft()` and `ifft()`
also support explicit chaining. The same stage names are used in Python and
TypeScript. The standalone `process()` facade has been removed.

Configure methods with `NormalizationMethod`, `BackgroundMethod`, `PrePostEdge`,
`AUTOBK` and `XrayFFTF`. Setters invalidate dependent results. Alternative methods
remain selectable; unimplemented methods return explicit errors. Inputs to
`from_arrays` must be finite, equal-length arrays with strictly increasing energy
in eV. Result getters expose the spectrum's intermediate and final arrays.

See the [API guide](../../doc/api.md) for examples, units and ownership.
`Spectrum` and `Group` remain aliases for `XASSpectrum` and `XASGroup`.

## Features and scope

- Default `trust-region`: optional fitting solver support.
- `refeff-runner`: ReFEFF's Rust EXAFS engine, with path outputs for fitting.
- `feff10-runner`: the FEFF10 backend through the `feff10` dependency.
- `plotting`: core plot builders through ruviz.
- `amcsd`, `materials-project`, `cod`: optional structure sources.
- `ndarray-compat`: legacy ndarray calculation path; the default is nalgebra.

Existing FEFF path files can be fitted without compiling a calculation backend.
`FeffFit` and the fitting module support single and joint datasets, independent
batches and k/R/q fit spaces. `FeffFlavor::Feff10` parsing is still separate from
FEFF10 execution; see the historical compatibility notes in the repository.
The native core has broader APIs than the Python and JavaScript bindings.

Licensed under MIT OR Apache-2.0; dependency and fixture notices remain applicable.

## Plotting (Feature-Gated)

Core plotting is available behind the `plotting` feature using `ruviz`.

```bash
cargo run -p rexafs --features plotting --example plot_demo
```

On Apple Silicon, if your linker resolution requires an explicit target linker:

```bash
CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER=clang cargo run -p rexafs --features plotting --example plot_demo
```

`plot_demo` writes outputs to:
- `crates/rexafs/target/plot_demo`

`plot_demo` coverage:
- FEFF85L module runs from full `feff.inp`: `Co`, `FeO_withPb`, `MnO2`, `ZnSe`
- Real fitting via `FeffFit::fit()`: `Cu`, `ZnSe`
- Fit plots per material: `k`, `k + window`, `r`, `r + window range`

To regenerate Cu/ZnSe fit references directly from XrayLarch:

```bash
uv run --with xraylarch python crates/rexafs/scripts/generate_larch_fit_references.py
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
use rexafs::prelude::*;
use rexafs::xafs::io::load_spectrum_QAS_trans;

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
use rexafs::prelude::*;

let mut group = XASGroup::new();
// populate group.spectra ...

group.plot().mu().save_png("group_overlay.png")?;
group.plot().mu().select(&[0, 2]).save_png("group_selected.png")?;
group.plot().mu().stacked(0.25).save_png("group_stacked.png")?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

### FeffFitResult examples

```rust,no_run
use rexafs::prelude::*;

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
