# FEFF Fitting MVP (Rust Core)

This document describes the scope and compatibility guarantees of the first FEFF fitting implementation added in `add-feff85l-xafs-fitting`.

## MVP Scope

Implemented now:
- Rust-core APIs in `xraytsubaki::xafs::fitting`.
- FEFF85L command discovery and module execution from a caller-provided executable path.
- FEFF85L path file parsing for `feffNNNN.dat` style files.
- FEFF path model synthesis with `path2chi` and summed `ff2chi`.
- Single-dataset nonlinear fitting in R-space with configurable transform window/range.
- Shared fit variables plus expression-bound path parameters.
- Reference fixtures and parity-oriented regression checks against xraylarch-generated data.

Out of scope in MVP:
- Python binding surface for fitting APIs.
- Multi-dataset/global fits.
- Non-R fitspaces (`k`, `q`, wavelet).
- FEFF10 execution support.

## FEFF Flavor Compatibility

- `FeffFlavor::Feff85L`: supported in this MVP.
- `FeffFlavor::Feff10`: intentionally returns a typed unsupported error.

This explicit dispatch behavior is intentional to keep caller contracts stable while FEFF10 parsing is implemented in a follow-up.

## FEFF10 Follow-up Path

The extension path is:
1. Implement FEFF10 parser normalization into the existing `FeffDat` model.
2. Keep `path2chi`, `ff2chi`, and `feffit` API signatures unchanged.
3. Add FEFF10 fixtures and parity tests beside FEFF85L fixtures.

No breaking API changes are expected for Rust callers when FEFF10 support is added.
