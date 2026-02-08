# Design: Pipeline Copy-Elimination Slice

## Context
Current core runtime still contains repeated ownership conversions at stage boundaries:
- `XASSpectrum::normalize` materializes owned `Array1` from cloned `DVector` storage.
- `AUTOBK::calc_background` clones `DVector` data into owned arrays before processing.
- Getter paths (`get_k`/`get_chi`) allocate owned vectors that are immediately re-borrowed for FFT.
- FFT real/imag getters clone FFT storage before projection.

These patterns are safe but expensive under large batch workloads, where per-spectrum overhead compounds significantly.

## Goals / Non-Goals
Goals:
- Remove avoidable full-vector copies in runtime-critical stage boundaries.
- Preserve current user-facing behavior for clone-returning APIs.
- Keep change set local and low risk.

Non-Goals:
- Rewriting all math internals around a single vector type.
- Changing scientific algorithms or tolerances.
- Introducing new external dependencies.

## Decisions
- Decision: Introduce borrowed accessor paths for pipeline internals.
  - Rationale: Internal stages can consume views, avoiding intermediate owned materialization.
  - Compatibility: Existing owned getters remain available.

- Decision: Adjust normalization/background stage interfaces to accept borrowed inputs where feasible.
  - Rationale: Callers already hold contiguous buffers; borrowing avoids repeated `to_vec`/`from_vec` churn.
  - Trade-off: Some local owned buffers may remain where downstream routines require ownership.

- Decision: Validate this slice with both runtime and allocation metrics.
  - Rationale: Copy-elimination can improve allocation pressure even when runtime gains are modest; both metrics are required.

## Data Flow Changes
Before:
1. Pipeline stage requests `DVector` via clone-returning getter.
2. Stage converts cloned vector into owned `Array1`.
3. Stage function consumes owned array.

After:
1. Pipeline stage requests borrowed view (`ArrayView1`/equivalent) from source storage.
2. Stage function consumes view directly when possible.
3. Owned buffers are created only for algorithms that strictly require ownership.

## Risks / Trade-offs
- Risk: Lifetime/borrow complexity may increase implementation friction.
  - Mitigation: Keep borrowed access localized and preserve compatibility wrappers.

- Risk: Mixed borrowed/owned code paths could reduce readability.
  - Mitigation: Add concise comments at conversion boundaries describing why ownership is required.

- Risk: Benchmark variance may hide small improvements.
  - Mitigation: require median comparison plus allocator-counter comparison.

## Validation Plan
- Run unit/integration tests for normalization/background/FFT behavior.
- Re-run core benchmarks:
  - `xas_group_benchmark_single`
  - `xas_group_benchmark_parallel`
  - `autobk_stage_benchmark`
- Re-run allocator instrumentation example (`alloc_baseline`) and compare allocation counts/bytes.
- Record before/after data in profiling documentation.
