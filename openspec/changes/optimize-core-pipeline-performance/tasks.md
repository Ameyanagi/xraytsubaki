## Coordination Note
- [ ] Keep this checklist synchronized with `update-whole-repo-performance-logic-hardening` slices A-C.
- [ ] Do not execute duplicated tasks independently if already completed under the umbrella change.

## 1. Baseline and Compatibility
- [ ] 1.1 Identify and apply minimal dependency/toolchain compatibility fix so `cargo test -p xraytsubaki` runs on stable.
- [ ] 1.2 Record baseline benchmark metrics for single and parallel pipeline benchmarks.
- [ ] 1.3 Document baseline environment and commands used for reproducibility.

## 2. Batch Reliability and Logic Safety
- [ ] 2.1 Replace panic-based batch processing (`unwrap` in `XASGroup` processing methods) with fallible error propagation.
- [ ] 2.2 Introduce/extend typed batch error structure containing failed spectrum index and source error.
- [ ] 2.3 Add tests covering per-spectrum failure behavior in sequential and parallel processing modes.
- [ ] 2.4 Fix FFT channel getter correctness issue and add regression tests.

## 3. Core Hot-Path Performance
- [ ] 3.1 Remove avoidable hot-loop allocations/conversions in AUTOBK path.
- [ ] 3.2 Reduce repeated vector materialization in normalization and FFT prep paths.
- [ ] 3.3 Replace high-cost membership/index patterns in edge-finding utilities with linear-time alternatives.
- [ ] 3.4 Add micro-bench or focused benchmark cases for changed hotspots.

## 4. CI and Regression Gates
- [ ] 4.1 Update CI to run core crate checks/tests independently from non-core workspace targets.
- [ ] 4.2 Add compatibility checks on stable and beta Rust toolchains.
- [ ] 4.3 Add benchmark reporting and non-blocking regression threshold initially.
- [ ] 4.4 Promote benchmark regression threshold to blocking after baseline stabilization.

## 5. Validation and Documentation
- [ ] 5.1 Verify no production panic paths remain in the covered batch APIs.
- [ ] 5.2 Verify benchmark target: >=25% end-to-end runtime improvement versus baseline on defined large-batch benchmark.
- [ ] 5.3 Update profiling/performance documentation with before/after results and methodology.
