# Design: Backend File Naming Canonicalization

## Context

The project already executes nalgebra-backed modules in default builds. However, defaults are currently sourced from files named with `_nalgebra` suffixes, while legacy ndarray paths keep unsuffixed names. This inverts expectations for new contributors.

## Goals

- Align file names with current runtime reality (default = canonical unsuffixed module names).
- Keep `ndarray-compat` supported with explicit naming.
- Avoid behavioral changes.

## Non-Goals

- Any scientific or numerical logic changes.
- Removal of compatibility backend.

## Decisions

### Decision 1: Canonical Names Represent Default Runtime
Default backend files will use:

- `background.rs`
- `mathutils.rs`
- `normalization.rs`
- `xafsutils.rs`
- `xrayfft.rs`

### Decision 2: Compatibility Files Use Explicit Suffix
ndarray-backed files will use:

- `background_ndarray.rs`
- `mathutils_ndarray.rs`
- `normalization_ndarray.rs`
- `xafsutils_ndarray.rs`
- `xrayfft_ndarray.rs`

### Decision 3: Minimal Routing Changes in `mod.rs`
Use `#[cfg(feature = "ndarray-compat")]` with `#[path = "..._ndarray.rs"]` for compatibility mode and standard module names for default mode.

## Risks and Mitigations

- Risk: incorrect cfg path wiring breaks one backend mode.
  - Mitigation: validate both default and `ndarray-compat` builds after rename.

- Risk: stale references to old filenames.
  - Mitigation: run repository search and update touched docs/notes.

## Validation Plan

- `cargo check -p xraytsubaki`
- `cargo check -p xraytsubaki --features ndarray-compat`
- strict gate suite used by repo CI
- `openspec validate refactor-canonicalize-backend-module-names --strict`
