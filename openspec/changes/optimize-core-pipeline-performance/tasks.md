## Coordination Note
- [x] Keep this checklist synchronized with `update-whole-repo-performance-logic-hardening` slices A-C.
- [x] Do not execute duplicated tasks independently if already completed under the umbrella change.

## 1. Baseline and Compatibility
- [x] 1.1 Identify and apply minimal dependency/toolchain compatibility fix so `cargo test -p xraytsubaki` runs on stable.
- [x] 1.2 Record baseline benchmark metrics for single and parallel pipeline benchmarks.
- [x] 1.3 Document baseline environment and commands used for reproducibility.

## 2. Batch Reliability and Logic Safety
- [x] 2.1 Replace panic-based batch processing (`unwrap` in `XASGroup` processing methods) with fallible error propagation.
- [x] 2.2 Introduce/extend typed batch error structure containing failed spectrum index and source error.
- [x] 2.3 Add tests covering per-spectrum failure behavior in sequential and parallel processing modes.
- [x] 2.4 Fix FFT channel getter correctness issue and add regression tests.

## 3. Core Hot-Path Performance
- [x] 3.1 Remove avoidable hot-loop allocations/conversions in AUTOBK path.
- [x] 3.2 Reduce repeated vector materialization in normalization and FFT prep paths.
- [x] 3.3 Replace high-cost membership/index patterns in edge-finding utilities with linear-time alternatives.
- [x] 3.4 Add micro-bench or focused benchmark cases for changed hotspots.

## 4. CI and Regression Gates
- [x] 4.1 Update CI to run core crate checks/tests independently from non-core workspace targets.
- [x] 4.2 Add compatibility checks on stable and beta Rust toolchains.
- [x] 4.3 Add benchmark reporting and non-blocking regression threshold initially.
- [x] 4.4 Promote benchmark regression threshold to blocking after baseline stabilization.

## 5. Validation and Documentation
- [x] 5.1 Verify no production panic paths remain in the covered batch APIs.
- [x] 5.2 Verify benchmark target: >=25% end-to-end runtime improvement versus baseline on defined large-batch benchmark.
- [x] 5.3 Update profiling/performance documentation with before/after results and methodology.
