# Change: Eliminate Hot-Path Copy Churn in Core Pipeline

## Why
The core pipeline (`normalize -> calc_background -> fft`) still performs repeated vector materialization and clone-heavy conversions in hot paths, especially at boundaries between `DVector<f64>` and `ndarray`-based APIs. This creates avoidable allocation pressure in large batch runs and contributes to runtime instability across benchmark runs.

A focused proposal is needed to remove copy churn without introducing broad representation migration or API-breaking behavior.

## What Changes
- Add borrowed read access paths for `k`/`chi` values so internal pipeline stages can avoid clone-based getters.
- Refactor normalization/background/FFT call paths to consume borrowed views where possible instead of repeated `Array1::from_vec(...clone())` conversions.
- Keep current clone-returning getters as compatibility wrappers, but route internal hot paths through zero-copy/low-copy accessors.
- Add explicit before/after benchmark and allocation validation for this slice.

## Scope
In scope:
- `crates/xraytsubaki/src/xafs/xasspectrum.rs`
- `crates/xraytsubaki/src/xafs/background.rs`
- `crates/xraytsubaki/src/xafs/normalization.rs`
- `crates/xraytsubaki/src/xafs/xrayfft.rs`
- benchmark/allocation validation and profiling notes in core crate docs

Out of scope:
- Full ndarray-to-nalgebra migration
- Solver algorithm changes (LM/direct-solver math changes)
- Python/GUI surface redesign

## Impact
Affected specs:
- `pipeline-copy-elimination` (new)

Affected code:
- Pipeline getter and staging boundaries for normalization/background/FFT
- Benchmark and profiling documentation for copy-elimination evidence

Related active changes:
- `optimize-core-pipeline-performance`
- `update-whole-repo-performance-logic-hardening`
- `refactor-ndarray-to-nalgebra-dvector`

This change is intentionally narrower: it delivers copy-elimination in the current representation strategy, without waiting for full vector-type migration.

## Success Criteria
- Internal pipeline flow avoids repeated clone-based `DVector`↔`Array1` conversions in hot loops.
- Existing public getter behavior remains compatible for current callers.
- Benchmarks and allocation counters show improvement versus pre-change baseline for this slice.
- Numerical outputs remain within existing tolerance expectations.
