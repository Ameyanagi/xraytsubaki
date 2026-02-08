# Design: Nalgebra-First Core with Feature-Gated ndarray Compatibility

## Context

The codebase is in an intermediate state:

- Core spectrum storage has largely moved to `DVector<f64>`.
- Several algorithm and utility modules still require ndarray types in public/internal boundaries.
- `--no-default-features` build is currently broken, which blocks a true nalgebra-first default.

The migration must reduce risk by sequencing high-churn files and validating at each step.

## Goals / Non-Goals

Goals:
- Make nalgebra the canonical internal vector representation.
- Isolate ndarray to `ndarray-compat` adapters.
- Preserve numerical behavior and performance characteristics.
- Maintain clear rollback points between phases.

Non-goals:
- Rewriting scientific algorithms.
- Introducing new public computation semantics.
- Bundling dependency major-version upgrades.

## Migration Strategy

### Phase A: Build-Mode Foundation

Objective: make both feature modes compile predictably before deep refactors.

- Normalize feature-gated exports/imports in `prelude.rs`, `nshare.rs`, and module interfaces.
- Eliminate unconditional ndarray type leakage from nalgebra-first code paths.
- Add/standardize build gates:
  - `cargo check -p xraytsubaki --no-default-features`
  - `cargo check -p xraytsubaki --features ndarray-compat`

Exit criteria:
- Both check commands succeed.

### Phase B: Utility and FFT Boundary Migration

Objective: remove most ndarray pressure from shared helpers and transform pipeline edges.

- Convert utility internals in `mathutils.rs` and `xafsutils.rs` to nalgebra-first implementations.
- Keep ndarray-specific helpers under `#[cfg(feature = "ndarray-compat")]`.
- Migrate `xrayfft.rs` storage and prep paths to nalgebra-first vectors.

Exit criteria:
- Existing FFT and utility tests pass under `--features ndarray-compat`.
- No new unconditional ndarray imports in touched files.

### Phase C: Normalization and Background Core

Objective: migrate remaining core pipeline state carriers.

- Convert normalization held vectors (`pre_edge`, `post_edge`, `norm`, `flat`) to nalgebra-backed storage or nalgebra-first access surfaces.
- Convert AUTOBK vector fields and internal flow to nalgebra-first, with ndarray compatibility adapters only where explicitly required.
- Preserve numerical parity against existing fixtures/tests.

Exit criteria:
- Normalization/background tests pass with existing tolerances.
- Pipeline integration tests pass.

### Phase D: Public Surface Stabilization and Default Flip

Objective: finalize API boundary and default feature behavior.

- Keep nalgebra as canonical public/internal representation.
- Keep ndarray interop behind `ndarray-compat` adapters.
- Flip default feature behavior only after previous gates are green.

Exit criteria:
- Default build runs without ndarray dependency.
- Compat build remains functional.

## API and Interface Decisions

- Canonical 1D vector type: `nalgebra::DVector<f64>`.
- ndarray-facing APIs in touched modules remain only as compatibility wrappers under `ndarray-compat`.
- Conversion traits in `nshare.rs` are feature-gated and not required in nalgebra-only builds.

## Validation Plan

Required after each phase:
- `cargo check -p xraytsubaki --no-default-features`
- `cargo check -p xraytsubaki --features ndarray-compat`

Required before completion:
- `cargo test -p xraytsubaki --features ndarray-compat`
- Benchmarks/allocation checks used in pipeline performance tracking

## Risks and Mitigations

- Risk: numerical drift from type/boundary changes.
  - Mitigation: preserve algorithms; rely on existing high-precision tests and fixtures.

- Risk: incomplete feature gating causes nalgebra-only build failures.
  - Mitigation: enforce dual-build checks as mandatory phase gates.

- Risk: performance regression from accidental copies.
  - Mitigation: keep borrowed surfaces where possible; run benchmark/allocation regression checks.
