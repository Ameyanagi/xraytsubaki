# Changelog

## 0.1.1

- Fix duplicate stationary spectra exposed during fast desktop plot pans by
  upgrading ruviz and ruviz-gpui to 0.13.1. The adapter clears the plot interior
  before compositing the translated preview, including transparent backgrounds.
- Retain linked and embedded 0.1.1 project samples, saved and reopened with the
  current writer; existing format-1 projects preserve their state.
- Use floor when selecting the automatic AUTOBK spline parameter count. For
  rbkg=1 and kmax=12 this selects eight parameters instead of nine. Explicit
  parameter counts and clamp behavior are unchanged.
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
