# Design: Stabilize the Post-Migration Baseline

## Context

The runtime is already nalgebra-first by default, with `ndarray-compat` as an optional compatibility mode. The immediate gap is not architectural direction, but operational reliability:

- strict lint/format gates are not green;
- Python bindings do not currently compile;
- CI does not fully enforce the strict local gate set as a single contract.

## Goals

- Lock nalgebra-first as the operational baseline for default builds.
- Make strict quality checks pass and enforce them in CI.
- Keep Python stable function behavior buildable and regression-resistant.

## Non-Goals

- Further ndarray-to-nalgebra migration redesign.
- Scientific algorithm changes for normalization, AUTOBK, or FFT.
- New Python API expansion beyond the existing stable functions.

## Decisions

### Decision 1: Stabilize, Do Not Re-Migrate
This change explicitly treats the nalgebra migration as complete for default runtime behavior. Work is limited to reliability gates and integration correctness.

### Decision 2: One Canonical Strict Gate Set
The following commands define success for this slice:

- `cargo test -p xraytsubaki`
- `cargo clippy -p xraytsubaki --all-targets -- -D warnings`
- `cargo fmt --all -- --check`
- `cargo check --manifest-path py-xraytsubaki/Cargo.toml`

CI will mirror these checks directly.

### Decision 3: Preserve Python Surface While Fixing Build
`run_batch_qas_trans` and `run_pipeline_arrays` remain the stable Python entry points; fixes are implementation-level and compatibility-preserving.

## Risks and Mitigations

- Risk: Strict clippy cleanup touches broad test/runtime surfaces.
  - Mitigation: Keep edits minimal and targeted to reported failures only.

- Risk: Python binding fixes accidentally change returned payload shape.
  - Mitigation: Add/maintain contract checks for expected keys and error tuple structure.

- Risk: CI runtime increases from extra checks.
  - Mitigation: Reuse existing core job sequencing and keep checks scoped to core + Python bindings.

## Validation Plan

- Run all canonical strict gate commands locally.
- Run `openspec validate update-post-nalgebra-stability-gates --strict`.
- Confirm CI workflow includes and blocks on the same gate set.
