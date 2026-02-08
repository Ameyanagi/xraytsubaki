# Change: Performance and Logic Hardening Program for Whole Repository

## Why
The repository has a known build reliability blocker and several correctness/performance risks in the core XAS path. Optimization work is currently hard to validate repeatedly because baseline metrics and CI gates are not fully established. This change creates a phased, measurable program to first unblock build/test reliability, then harden logic correctness and panic behavior, and finally optimize hot paths without scientific-result drift.

## What Changes
- Unblock core crate build/test by removing the legacy FFT dependency chain that introduces `anymap` incompatibility.
- Establish baseline and CI gates for runtime, allocation, and compatibility (stable + beta).
- Eliminate panic-on-error behavior in `XASGroup` batch methods and return structured aggregate errors.
- Fix confirmed FFT getter correctness issue (`get_chir_imag` returning real component).
- Reduce hot-path conversions and allocations in normalization, AUTOBK, FFT prep, and edge-finding utilities.
- Apply small API cleanups for path handling and seq/par behavior consistency.
- Split CI so core crate feedback is isolated from GUI failures and add support expectations for Python/GUI integration boundaries.

## Scope
In scope:
- `crates/xraytsubaki` core runtime behavior and performance
- benchmark/profiling workflow and CI gating
- targeted repo-wide support work for Python bindings and GUI integration boundaries

Out of scope:
- new scientific algorithms beyond current methods
- full GUI redesign
- large Python API redesign beyond minimum stable batch interface

## Impact
Affected areas:
- Core pipeline modules: `xasspectrum`, `background`, `normalization`, `xrayfft`, `xasgroup`, `xafsutils`, `io`
- CI workflow under `.github/workflows/rust.yml`
- Python and GUI integration entry points

Breaking changes:
- Small API break accepted for batch processing error returns and `AsRef<Path>` input cleanup where it improves correctness/safety.

## Overlap Synchronization (Active Changes)
This change is synchronized with existing pending changes to avoid duplicate implementation scope:

- `optimize-core-pipeline-performance`
  - Treated as a core-focused subset of this umbrella plan (Slices A-C).
  - Core implementation should be tracked here as the canonical sequence.
- `modernize-error-handling`
  - Owns detailed error taxonomy and `thiserror` migration mechanics.
  - This change consumes those outcomes for panic elimination and batch failure reporting behavior.
- `refactor-ndarray-to-nalgebra-dvector`
  - Owns broad representation migration and compatibility strategy.
  - This change only requires hot-path conversion reduction and can proceed incrementally without waiting for full repo migration.

## Success Criteria
- `cargo test -p xraytsubaki` succeeds on stable and beta in CI.
- Bench baseline exists for `xas_group_benchmark_single` and `xas_group_benchmark_parallel` with p50/p95 runtime, allocation count, and max RSS.
- Batch processing APIs no longer panic for recoverable per-spectrum failures.
- FFT imaginary getter returns imaginary data and has regression coverage.
- Measured 25-40% runtime improvement on the 10k-spectrum parallel benchmark versus post-unblock baseline, with no tolerance-breaking scientific drift.
