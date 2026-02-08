# Change: Optimize Core Pipeline Performance and Logic Reliability

## Status
Synchronized with `update-whole-repo-performance-logic-hardening`.
This change remains as a core-focused subset reference, but execution order and acceptance tracking are canonicalized in the umbrella change to avoid duplicate implementation tracks.

## Why
The current codebase has known bottlenecks and reliability gaps in the core XAS processing path (`normalize -> calc_background -> fft`) that limit safe high-throughput usage:

- Existing profiling identifies AUTOBK/minimization as dominant runtime cost.
- Core runtime code still contains panic-prone paths (`unwrap`) in batch processing and several heavy allocation/conversion patterns in hot loops.
- Current CI cannot provide a stable baseline because the workspace fails to build under the current toolchain due to a dependency chain involving `anymap`.

This change defines a focused, measurable plan to improve throughput and logic safety without broad architectural churn.

## What Changes
This proposal adds three capabilities:

1. **Core pipeline performance capability**
- Define required performance baseline collection and benchmark regression gates.
- Require removal of avoidable hot-path allocations/conversions in AUTOBK, normalization, and FFT preparation paths.

2. **Batch processing reliability capability**
- Replace panic-based parallel/sequence group processing behavior with typed fallible behavior.
- Correct known logic inconsistencies (for example, incorrect imaginary-channel getter behavior in FFT outputs).

3. **Build compatibility capability**
- Require toolchain-compatible dependency graph for core crate test/build execution.
- Require CI coverage that detects compatibility and performance regressions early.

## Scope
In scope:
- `crates/xraytsubaki` runtime performance and logic behavior
- benchmark and profiling workflow under `crates/xraytsubaki/benches` and `doc/profiling.md`
- dependency compatibility and CI for core crate build/test/benchmark validation

Out of scope:
- Implementing incomplete new scientific algorithms (for example ILPBkg)
- Expanding GUI feature set
- Full Python API redesign

## Impact
Affected areas:
- Core algorithms: normalization, background (AUTOBK), FFT, utility math/indexing
- Batch APIs in `XASGroup`
- Build/CI configuration

Related changes:
- `modernize-error-handling` (complements error propagation and panic elimination)
- `refactor-ndarray-to-nalgebra-dvector` (complements allocation/conversion reduction)

Sequencing expectation:
- This change may proceed in parallel, but implementation tasks that depend on error type upgrades or vector migration should be ordered after the relevant tasks from those changes are available.
- For overlap with build/baseline/correctness/perf slices, use `update-whole-repo-performance-logic-hardening` as the source of truth.

## Success Criteria
- Core crate builds and tests on current stable toolchain.
- Benchmarks for single and parallel pipeline runs are reproducible in CI.
- Throughput improvement target: at least 25% faster end-to-end runtime for benchmarked large-batch pipeline relative to post-unblock baseline.
- No panic from production `XASGroup` processing methods for recoverable per-spectrum failures.
- Logic correctness checks cover FFT real/imag channel getters and batch error behavior.
