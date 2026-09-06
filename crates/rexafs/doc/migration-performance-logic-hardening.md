# Migration Notes: Performance + Logic Hardening

## Post-Nalgebra Stability Gates

The default runtime path is nalgebra-first (`ndarray-compat` remains optional compatibility mode).

Canonical repository gates for this stabilized baseline:

- `cargo test -p rexafs`
- `cargo clippy -p rexafs --all-targets -- -D warnings`
- `cargo fmt --all -- --check`
- `cargo check --manifest-path py-rexafs/Cargo.toml`

These commands are mirrored in CI as blocking checks.

Pre-hardening baseline failures that motivated this gate set:

- strict clippy failure in core (`cargo clippy -p rexafs --all-targets -- -D warnings`)
- workspace formatting drift (`cargo fmt --all -- --check`)
- Python binding compile error in `py-rexafs/src/lib.rs` (`chir_mag.as_slice()` treated as `Option`)

## Batch Processing API

`XASGroup` batch methods are now fallible and no longer panic on per-spectrum failures.

- `find_e0`, `normalize`, `calc_background`, `fft`, `ifft`
- `*_seq` and `*_par` variants

These return:

- `Result<&mut Self, BatchProcessError>`
- `BatchProcessError.errors: Vec<BatchSpectrumError>`
- Each `BatchSpectrumError` contains:
  - `index`: failing spectrum index
  - `source`: typed `XAFSError`

## Input Validation Behavior

Core runtime stages now validate:

- `energy.len() == mu.len()`
- finite `energy/mu` values
- monotonic non-decreasing energy assumptions

Validation failures return typed errors before heavy numeric work starts.

## Path API Change

`load_spectrum_QAS_trans` now accepts generic path-like inputs via `AsRef<Path>`.

- Before: `load_spectrum_QAS_trans(path: &String)`
- After: `load_spectrum_QAS_trans<P: AsRef<Path>>(path: P)`

Callers can now pass `&str`, `String`, `&Path`, and `PathBuf` directly.

## AUTOBK Knot Domain

AUTOBK knot-domain construction now has explicit tests for:

- domain bounds
- monotonic ordering
- invalid-range rejection

This prevents silent knot-domain drift during refactors.

## AUTOBK Solver Modes

AUTOBK now supports two solver backends:

- `AUTOBKSolver::LinearDirect` (default)
- `AUTOBKSolver::LegacyLm`

New optional AUTOBK configuration fields:

- `solver`
- `clamp_scale_policy` (`Fixed` or `TwoPass`)
- `linear_regularization`
- `linear_condition_limit`
- `linear_residual_ratio_limit`
- `linear_fallback_to_lm`
- `linear_workspace_cache`

Default behavior uses direct solve and falls back to LM when direct-solver quality/conditioning checks fail.

## Python Binding Surface

Minimal stable batch API is available in `py-rexafs`:

- `run_batch_qas_trans(paths)` -> `(processed_count, errors)`
- `run_pipeline_arrays(energy, mu)` -> dict with `e0`, `k`, `chi`, `chir_mag`

Structured Python error rows include `(index, category, message)`.
