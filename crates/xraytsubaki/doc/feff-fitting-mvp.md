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

## Builder API and Canonical Examples

The redesigned fitting surface adds additive builder APIs (`FeffFit`, `Param`) and
multi-dataset fitting while keeping the existing free-function flow available.

Canonical usage examples are maintained in:
- `plans/fitting-api-redesign.md`

Those examples define the intended ergonomic workflows:
- path-model chaining and clone-and-reuse,
- tuple shorthand via `set_inits`,
- `Param`-based parameter declarations,
- single-dataset fitting with builder chaining,
- multi-dataset global fitting with shared parameters.

## Migration Guidance (Legacy -> Builder)

Legacy flow remains valid:
- Build `FeffFitDataset` and `FitVariables` manually.
- Call `feffit(&dataset, &vars)`.

Builder flow is additive:
- Build with `FeffFit::new().data(...).add_path(...).set_inits(...).fit()`.
- Use `add_dataset(...)` for global multi-dataset fitting.
- Use `params([Param::new(...), Param::fixed(...), Param::expr(...)])` for concise variable setup.

Compatibility policy:
- Existing entrypoints and single-dataset access patterns remain supported.
- Builder/multi-dataset APIs are additive.
- Deprecation is documentation-first; no mandatory migration is introduced in this change.

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
