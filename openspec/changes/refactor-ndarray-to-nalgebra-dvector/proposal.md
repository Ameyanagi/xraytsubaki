# Change: Complete Nalgebra-First Migration with Optional ndarray Compatibility

## Why

The repository has already started moving runtime paths to `nalgebra::DVector<f64>`, but migration is incomplete and inconsistent:

- `XASSpectrum` is mostly nalgebra-based while many dependent modules still expose ndarray types.
- `cargo check -p xraytsubaki --no-default-features` currently fails (111 compile errors), so nalgebra-only builds are not yet viable.
- ndarray usage is still concentrated in hot and shared modules (`xafsutils`, `xrayfft`, `mathutils`, `background`), creating conversion churn and split APIs.

This change makes migration execution-ready by defining a strict, phased path to a nalgebra-first core while preserving an explicit `ndarray-compat` adapter layer.

## What Changes

- Establish a nalgebra-first core API for XAFS internals (`DVector<f64>` as canonical 1D vector type).
- Restrict ndarray usage to compatibility adapters and feature-gated code paths (`ndarray-compat`).
- Migrate remaining ndarray-heavy modules in phases with mandatory compile/test gates after each phase.
- Keep behavior and scientific outputs stable within existing tolerances; do not change algorithms in this change.
- Keep serialization and external behavior stable unless a compatibility adapter is explicitly introduced.

## Scope

In scope:
- `crates/xraytsubaki/src/xafs/mathutils.rs`
- `crates/xraytsubaki/src/xafs/xafsutils.rs`
- `crates/xraytsubaki/src/xafs/xrayfft.rs`
- `crates/xraytsubaki/src/xafs/normalization.rs`
- `crates/xraytsubaki/src/xafs/background.rs`
- `crates/xraytsubaki/src/xafs/xasspectrum.rs`
- `crates/xraytsubaki/src/xafs/nshare.rs`
- `crates/xraytsubaki/src/prelude.rs`
- JSON/BSON compatibility checks in `crates/xraytsubaki/src/xafs/io/`
- Benchmark/allocation regression checks for pipeline-critical paths

Out of scope:
- Algorithm redesign (AUTOBK/normalization/FFT math changes)
- New numerical methods or fitting models
- Python API redesign
- nalgebra major-version upgrade as part of the migration

## Impact

Affected capability:
- `xas-core`

Affected users:
- Internal Rust callers and external Rust users depending on ndarray-first APIs

Compatibility strategy:
- Nalgebra-first by default once migration gates are met
- Optional ndarray interoperability via `ndarray-compat`

## Success Criteria

- `cargo check -p xraytsubaki --no-default-features` passes.
- `cargo check -p xraytsubaki --features ndarray-compat` passes.
- `cargo test -p xraytsubaki --features ndarray-compat` passes.
- New ndarray usage introduced by this migration is compatibility-only and feature-gated.
- Numerical parity for normalization/background/FFT remains within existing test tolerances.
- Benchmarks/allocation checks show no material regression for pipeline workloads.
