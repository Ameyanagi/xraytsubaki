# Change: Post-Nalgebra Stability and Quality Gate Enforcement

## Why

The migration to a nalgebra-first runtime path is already in place, but repository quality gates are not yet consistently green:

- `cargo test -p xraytsubaki` passes, but strict lint and format gates currently fail.
- `cargo check --manifest-path py-xraytsubaki/Cargo.toml` currently fails due to a Python binding compile issue.
- The documented Python stable API contract exists, but the build is not currently aligned with that contract.

This change hardens the post-migration baseline so the current architecture is reliable and enforceable in CI.

## What Changes

- Treat nalgebra-first runtime as the baseline architecture and avoid re-opening representation migration.
- Define and enforce strict core quality gates (`test`, `clippy -D warnings`, `fmt --check`) on stable Rust.
- Restore Python binding build health while preserving the documented stable function surface.
- Add CI enforcement for these gates so regressions are caught before merge.

## Scope

In scope:
- Core gate compliance for `crates/xraytsubaki`
- Python binding build correctness for `py-xraytsubaki`
- CI updates in `.github/workflows/rust.yml` for enforceable post-migration gates
- Documentation updates needed to keep behavior/build expectations synchronized

Out of scope:
- New scientific algorithms or numerical model changes
- Broad GUI redesign or GUI feature work
- Removing `ndarray-compat` support in this slice
- New public Python API surface beyond current stable functions

## Impact

Affected capabilities:
- `build-and-baseline-gating`
- `repo-integration-support`

Affected code:
- `crates/xraytsubaki/src/xafs/mod.rs` and other lint-failing locations
- `py-xraytsubaki/src/lib.rs` and related binding files
- `.github/workflows/rust.yml`

## Success Criteria

- `cargo test -p xraytsubaki` succeeds on stable.
- `cargo clippy -p xraytsubaki --all-targets -- -D warnings` succeeds on stable.
- `cargo fmt --all -- --check` succeeds.
- `cargo check --manifest-path py-xraytsubaki/Cargo.toml` succeeds.
- CI enforces these commands as blocking checks for relevant pull requests.
