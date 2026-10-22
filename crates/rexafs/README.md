# rexafs for Rust

[Published user guide](https://rexafs.com/docs/libraries/rust/) ·
[Versioned API reference](https://rexafs.com/docs/reference/)

Rust-powered X-ray absorption analysis, developed under the codename xraytsubaki.
The core includes normalization, AUTOBK, Fourier transforms, group processing,
EXAFS fitting, structure handling, LCF/PCA and spectrum tools.

Install the library with `cargo add rexafs`. In this checkout, run
`cargo test -p rexafs` to exercise the core regression suite.

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
TypeScript. There is no separate `process()` facade.

Configure methods with `NormalizationMethod`, `BackgroundMethod`, `PrePostEdge`,
`AUTOBK`, `XrayFFTF` and `XrayFFTR` (the inverse-transform settings). In 0.2.5 and
later, pass settings directly:

```rust,no_run
use rexafs::{AUTOBK, PrePostEdge, Spectrum};
# let energy = [1.0, 2.0, 3.0];
# let mu = [0.0, 0.5, 1.0];
let mut spectrum = Spectrum::from_arrays(&energy, &mu)?;
let mut background = AUTOBK::new();
background.rbkg = Some(1.2); // Low-R background cutoff, in angstroms.
spectrum.set_normalization_method(PrePostEdge::new())?;
spectrum.set_background_method(background)?;
# Ok::<(), rexafs::Error>(())
```

Rust setters move configurations into the spectrum; clone a reusable setting
explicitly. Version 0.2.4 uses `Some(BackgroundMethod::AUTOBK(background))` and
`Some(NormalizationMethod::PrePostEdge(parameters))`. Those forms and `None` for
default settings remain supported. Setters invalidate dependent results. Alternative methods
remain selectable; unimplemented methods return explicit errors. Inputs to
`from_arrays` must be finite, equal-length arrays with strictly increasing energy
in eV. `k()`, `chi()` and `chir()` borrow stored buffers; the other public array
getters return owned copies. A getter returns `None` when its result is unavailable
and never computes it implicitly.

See the [API guide](https://github.com/Ameyanagi/rexafs/blob/main/doc/api.md) for examples, units and ownership.
`Spectrum` and `Group` remain aliases for `XASSpectrum` and `XASGroup`.

## What the calculations mean

Normalization subtracts a fitted pre-edge baseline and divides absorption by
its edge step. AUTOBK estimates the smooth background to obtain the EXAFS
oscillations, chi(k). The Fourier transform weights and windows those oscillations
to display them against R; its peaks are not automatically phase-corrected bond
lengths. An inverse transform filters selected R contributions back into q space.

The [processing theory guide](https://rexafs.com/docs/science/processing/) explains the
equations, symbols, units, assumptions and implementation choices, with scientific
references. Use the [fitting-statistics guide](https://rexafs.com/docs/science/fitting-statistics/)
when interpreting structural fits and uncertainties.

## Features and scope

- Default `trust-region`: optional fitting solver support.
- `refeff-runner`: ReFEFF's Rust EXAFS engine, with path outputs for fitting and
  the `rexafs::rmc` coordinate-refinement backend (since 0.2.10).
- `feff10-runner`: the FEFF10 backend through the `feff10` dependency.
- `plotting`: core plot builders through ruviz.
- `amcsd`, `materials-project`, `cod`: optional structure sources.
- `ndarray-compat`: legacy ndarray calculation path; the default is nalgebra.

Existing FEFF path files can be fitted without compiling a calculation backend.
`FeffFit` and the fitting module support single and joint datasets, independent
batches and k/R/q fit spaces. `FeffFlavor::Feff10` parsing is still separate from
FEFF10 execution; see the historical compatibility notes in the repository.
The native core has broader APIs than the Python and JavaScript bindings.

Version 0.2.10 includes a reverse Monte Carlo (RMC) engine with ReFEFF as its primary calculator. It supports constrained atomic
moves, finite and periodic geometry, weighted structures, k/R/q/wavelet objectives,
resumable sessions, evolutionary search and structural reports. ReFEFF offers
exact local-input caching and prepared path updates at fixed potentials.
`PreparedRefeffCalculator` with `AccelerationSettings::default()` is the recommended
exact caching path. Adaptive scattering is experimental, disabled by default,
and requires explicit opt-in with exact accuracy audits. See the
[RMC guide](../../doc/rmc.md) for Rust examples, performance measurements,
scientific assumptions and validation limits. The desktop exposes guided
single-spectrum RMC with live results and saved-run recovery; evolutionary and
weighted-structure controls currently remain Rust APIs.

Licensed under MIT OR Apache-2.0; dependency and fixture notices remain applicable.

## Plotting (Feature-Gated)

Core plotting is available behind the `plotting` feature using `ruviz`.
The complete `plot_demo` also runs external FEFF85L modules. Install the FEFF
binaries supplied with XrayLarch and set `REXAFS_FEFF8L_RDINP` to the path of
`feff8l_rdinp` if the example cannot discover it. The plotting builders themselves
do not require that executable when plotting existing spectra or fit results.

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

FEFF fit compatibility is regression-tested against these regenerated Cu/ZnSe fixtures:
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
- `save_png()` combines multiple panels. `to_svg()` and `render_plot()` require a single panel; `to_svg_panels()` returns a separate SVG string for each panel. In SVG output, labels without Typst math or markup (tick numbers, plain legend names) are editable `<text>`; labels with math or markup characters such as `$`, `[` or `#` stay as glyph outlines.

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

## Universal measurement reader (since 0.2.6)

Version 0.2.6 adds content-detected beamline text, CSV, Athena, XTUNES
and HDF5 import through the shared Rust reader. Read a document, inspect its
scans, columns, units and warnings, then select a signal mapping. Ambiguous
channels require explicit selection; detector images require reduction before
creating an absorption spectrum. Reading performs no processing or corrections.
See the [reader guide](../../doc/measurement-reader.md) for language examples,
unit conversions, dataset selection, fixture coverage and limits. This API is
not included in the published 0.2.5 packages.

Larix 1.0 session import is available in the shared reader since 0.2.6, including
stored absorption, complex saved arrays and inert session metadata. See the
[Larix import guide](../../doc/larix-import.md).
