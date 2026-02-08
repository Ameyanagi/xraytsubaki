## 1. Baseline and Gate Setup
- [x] 1.1 Record current failure baseline for `cargo check -p xraytsubaki --no-default-features`.
- [x] 1.2 Confirm `cargo check -p xraytsubaki --features ndarray-compat` still succeeds before migration edits.
- [x] 1.3 Add a short migration note documenting required dual-build gates for every phase.

## 2. Phase A: Build-Mode Foundation
- [x] 2.1 Fix `prelude.rs` exports so `ToNalgebra`/`ToNdarray1` are only re-exported under `ndarray-compat`.
- [x] 2.2 Ensure `nshare.rs` has no symbols required by nalgebra-only builds behind incompatible cfg boundaries.
- [x] 2.3 Remove/guard unconditional ndarray type signatures in `xasspectrum.rs` and related module surfaces.
- [x] 2.4 Run `cargo check -p xraytsubaki --no-default-features` and fix all compile errors in this phase scope.
- [x] 2.5 Run `cargo check -p xraytsubaki --features ndarray-compat` and fix regressions introduced by phase A.

## 3. Phase B: Utility and FFT Migration
- [x] 3.1 Refactor `mathutils.rs` to nalgebra-first internals for touched vector paths.
- [x] 3.2 Refactor `xafsutils.rs` to nalgebra-first internals for touched vector paths.
- [x] 3.3 Move ndarray-only utility helpers behind `#[cfg(feature = "ndarray-compat")]`.
- [x] 3.4 Refactor `xrayfft.rs` core storage and transform prep paths to nalgebra-first vectors.
- [x] 3.5 Keep ndarray-facing FFT compatibility surfaces gated by `ndarray-compat`.
- [x] 3.6 Validate with `cargo check` in both feature modes.
- [x] 3.7 Run targeted tests for utilities and FFT under `--features ndarray-compat`.

## 4. Phase C: Normalization and Background Migration
- [x] 4.1 Refactor normalization internals to nalgebra-first held/output vectors.
- [x] 4.2 Refactor AUTOBK/background held vectors and core flow to nalgebra-first.
- [x] 4.3 Keep any required ndarray compatibility adapters gated by `ndarray-compat`.
- [x] 4.4 Run `cargo check` in both feature modes.
- [x] 4.5 Run normalization/background/spectrum tests under `--features ndarray-compat`.

## 5. Integration and Performance Validation
- [x] 5.1 Run `cargo test -p xraytsubaki --features ndarray-compat`.
- [x] 5.2 Re-run pipeline benchmarks/allocation checks used by project profiling docs.
- [x] 5.3 Confirm no material performance regression versus pre-phase baseline.
- [x] 5.4 Confirm no new non-gated ndarray usage in touched migration files via `rg` review.

## 6. Default Feature Finalization
- [x] 6.1 Flip default feature behavior to nalgebra-first only after phases 2–5 are green.
- [x] 6.2 Verify nalgebra-only default with `cargo check -p xraytsubaki --no-default-features` and default `cargo check -p xraytsubaki`.
- [x] 6.3 Verify compatibility mode with `cargo check -p xraytsubaki --features ndarray-compat` and targeted tests.
- [x] 6.4 Update migration docs/changelog notes describing nalgebra-first default and ndarray compatibility path.

## 7. Proposal Integrity
- [x] 7.1 Run `openspec validate refactor-ndarray-to-nalgebra-dvector --strict`.
- [x] 7.2 Resolve all validation issues before implementation starts.
