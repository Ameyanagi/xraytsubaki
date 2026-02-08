# Migration Notes: Performance + Logic Hardening

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

## Python Binding Surface

Minimal stable batch API is available in `py-xraytsubaki`:

- `run_batch_qas_trans(paths)` -> `(processed_count, errors)`
- `run_pipeline_arrays(energy, mu)` -> dict with `e0`, `k`, `chi`, `chir_mag`

Structured Python error rows include `(index, category, message)`.
