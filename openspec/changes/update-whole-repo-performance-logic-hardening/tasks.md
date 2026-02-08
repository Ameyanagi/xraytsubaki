## 0. Change Coordination
- [x] 0.1 Confirm overlap boundaries with `optimize-core-pipeline-performance`, `modernize-error-handling`, and `refactor-ndarray-to-nalgebra-dvector` before code edits.
- [x] 0.2 Ensure duplicated tasks are executed in one canonical change stream to avoid conflicting implementations.
- [x] 0.3 Keep cross-references updated in each active proposal when scope ownership changes.

## 1. Slice A: Build Unblock and Baseline
- [x] 1.1 Remove or replace the FFT dependency path that introduces `easyfft 0.2.x`/`anymap` incompatibility so `cargo test -p xraytsubaki` passes on stable.
- [x] 1.2 Ensure only one FFT stack is used in core runtime path and document the chosen stack.
- [x] 1.3 Add CI matrix for core crate on stable and beta Rust.
- [x] 1.4 Split CI jobs so core crate compile/test is isolated from GUI-specific failures.
- [x] 1.5 Capture post-unblock baseline for `xas_group_benchmark_single` and `xas_group_benchmark_parallel` including p50/p95 runtime, allocation count, and max RSS.

## 2. Slice B: Correctness and Panic Elimination
- [x] 2.1 Introduce batch aggregate error type with spectrum index and source error.
- [x] 2.2 Update `XASGroup` seq/par/default processing methods to return fallible aggregate results instead of panicking.
- [x] 2.3 Fix `get_chir_imag` to return imaginary component and add regression tests.
- [x] 2.4 Replace non-test runtime `unwrap` in `xasspectrum`, `background`, `normalization`, and `xasgroup` with typed error propagation.
- [x] 2.5 Add invariant validation for length mismatch, monotonic energy expectations, and non-finite values before heavy numeric stages.
- [x] 2.6 Add seq/par numerical equivalence tests within tolerance.

## 3. Slice C: High-Impact Performance Refactor
- [x] 3.1 Standardize one internal numeric representation for runtime pipeline execution and move conversions to I/O boundaries.
- [x] 3.2 Remove repeated vector materialization and clone-heavy operations in AUTOBK hot path using slices/views/cacheable buffers.
- [x] 3.3 Replace O(n^2) membership checks in edge/peak finding with linear-time mask/set strategies.
- [x] 3.4 Refactor `XASGroup` remove/move operations to index bitmap + single-pass mutation.
- [x] 3.5 Benchmark and profile changed paths; attach before/after metrics.

## 4. Slice D: API Consistency and Integration Support
- [x] 4.1 Update path-taking I/O APIs from `&String` to `impl AsRef<Path>` and adjust call sites.
- [x] 4.2 Ensure seq/par/default methods have consistent error semantics and deterministic state transitions.
- [x] 4.3 Validate AUTOBK knot-domain behavior with explicit numerical tests.
- [x] 4.4 Define minimal stable Python batch API surface and document zero-copy expectations where feasible.
- [x] 4.5 Ensure GUI path calls optimized core APIs asynchronously for large datasets without coupling GUI to core perf-critical logic.

## 5. Validation and Rollout Controls
- [x] 5.1 Run unit/integration tests and criterion benchmarks after each slice.
- [x] 5.2 Publish benchmark artifacts in CI and start with informational regression threshold.
- [x] 5.3 Promote regression threshold to blocking after baseline stabilization.
- [x] 5.4 Document migration notes for API adjustments and batch error behavior.
