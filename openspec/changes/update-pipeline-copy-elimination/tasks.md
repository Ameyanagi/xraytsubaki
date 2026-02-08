## 1. Baseline Capture
- [ ] 1.1 Capture pre-change benchmark medians for `xas_group_benchmark_single`, `xas_group_benchmark_parallel`, and `autobk_stage_benchmark`.
- [ ] 1.2 Capture pre-change allocation counters via `cargo run -p xraytsubaki --example alloc_baseline --release`.
- [ ] 1.3 Store baseline command set and environment in profiling notes.

## 2. Borrowed Access Surface
- [ ] 2.1 Add borrowed `k`/`chi` accessor paths in background/spectrum modules for internal pipeline usage.
- [ ] 2.2 Keep existing clone-returning getters and route them through compatibility wrappers.
- [ ] 2.3 Update internal FFT call path to use borrowed accessors by default.

## 3. Stage Boundary Copy Elimination
- [ ] 3.1 Refactor normalization call path to consume borrowed views where ownership is not required.
- [ ] 3.2 Refactor AUTOBK input conversion path to avoid repeated full-vector materialization.
- [ ] 3.3 Reduce avoidable cloning in FFT real/imag getter internals.

## 4. Correctness Validation
- [ ] 4.1 Add/adjust tests asserting borrowed-path output parity with existing owned-path behavior.
- [ ] 4.2 Verify no behavioral regressions in existing normalization/background/FFT tests.

## 5. Performance Validation and Documentation
- [ ] 5.1 Re-run benchmark suite and compare medians against baseline.
- [ ] 5.2 Re-run allocation instrumentation and compare call/byte counts against baseline.
- [ ] 5.3 Update profiling documentation with dated before/after results for this slice.

## 6. Proposal Integrity
- [ ] 6.1 Run `openspec validate update-pipeline-copy-elimination --strict` and resolve all issues.
