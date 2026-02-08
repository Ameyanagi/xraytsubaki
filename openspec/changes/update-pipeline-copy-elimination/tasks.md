## 1. Baseline Capture
- [x] 1.1 Capture pre-change benchmark medians for `xas_group_benchmark_single`, `xas_group_benchmark_parallel`, and `autobk_stage_benchmark`.
- [x] 1.2 Capture pre-change allocation counters via `cargo run -p xraytsubaki --example alloc_baseline --release`.
- [x] 1.3 Store baseline command set and environment in profiling notes.

## 2. Borrowed Access Surface
- [x] 2.1 Add borrowed `k`/`chi` accessor paths in background/spectrum modules for internal pipeline usage.
- [x] 2.2 Keep existing clone-returning getters and route them through compatibility wrappers.
- [x] 2.3 Update internal FFT call path to use borrowed accessors by default.

## 3. Stage Boundary Copy Elimination
- [x] 3.1 Refactor normalization call path to consume borrowed views where ownership is not required.
- [x] 3.2 Refactor AUTOBK input conversion path to avoid repeated full-vector materialization.
- [x] 3.3 Reduce avoidable cloning in FFT real/imag getter internals.
- [x] 3.4 Ensure new `ndarray` usage in touched paths is compatibility-only and guarded by `ndarray-compat`.

## 4. Correctness Validation
- [x] 4.1 Add/adjust tests asserting borrowed-path output parity with existing owned-path behavior.
- [x] 4.2 Verify no behavioral regressions in existing normalization/background/FFT tests.
- [x] 4.3 Verify changed files introduce no new non-gated `ndarray` usage (diff review + targeted `rg` check).

## 5. Performance Validation and Documentation
- [x] 5.1 Re-run benchmark suite and compare medians against baseline.
- [x] 5.2 Re-run allocation instrumentation and compare call/byte counts against baseline.
- [x] 5.3 Update profiling documentation with dated before/after results for this slice.

## 6. Proposal Integrity
- [x] 6.1 Run `openspec validate update-pipeline-copy-elimination --strict` and resolve all issues.
