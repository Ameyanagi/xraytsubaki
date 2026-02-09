# Change: Canonicalize Backend Module File Names

## Why

The runtime is already nalgebra-first by default, but source file naming still reflects an older transition state (`*_nalgebra.rs` for default paths). This creates avoidable confusion for contributors and makes default module resolution less obvious.

## What Changes

- Rename nalgebra-default XAFS module files to canonical names without backend suffixes.
- Rename legacy ndarray-backed module files to explicit `*_ndarray.rs` names.
- Update `crates/xraytsubaki/src/xafs/mod.rs` backend routing so:
  - default build maps to canonical module files;
  - `ndarray-compat` maps to `*_ndarray.rs`.
- Keep behavior and public APIs unchanged (file-layout refactor only).

## Scope

In scope:
- `crates/xraytsubaki/src/xafs/{background,mathutils,normalization,xafsutils,xrayfft}*`
- `crates/xraytsubaki/src/xafs/mod.rs`
- minimal doc updates referencing old file names

Out of scope:
- algorithm changes
- API behavior changes
- removal of `ndarray-compat`

## Impact

Affected capability:
- `xas-core`

Affected code:
- XAFS module file layout and compile-time backend selection

## Success Criteria

- No `*_nalgebra.rs` files remain in `crates/xraytsubaki/src/xafs/`.
- Default build and test still pass.
- `--features ndarray-compat` build still passes.
- Strict quality gates remain green:
  - `cargo test -p xraytsubaki`
  - `cargo clippy -p xraytsubaki --all-targets -- -D warnings`
  - `cargo fmt --all -- --check`
  - `cargo check --manifest-path py-xraytsubaki/Cargo.toml`
