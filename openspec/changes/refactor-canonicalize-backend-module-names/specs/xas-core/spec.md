## ADDED Requirements

### Requirement: Canonical Default Backend Module Naming
The system SHALL use canonical unsuffixed XAFS module file names for the default nalgebra runtime backend.

#### Scenario: Default build resolves canonical modules
- **WHEN** the crate is built without `ndarray-compat`
- **THEN** `background`, `mathutils`, `normalization`, `xafsutils`, and `xrayfft` resolve to unsuffixed module files
- **AND** no `_nalgebra`-suffixed module filenames are required for default routing

### Requirement: Explicit ndarray Compatibility Module Naming
The system SHALL use explicit `*_ndarray` module file naming for compatibility backend sources.

#### Scenario: Compatibility build resolves ndarray modules
- **WHEN** the crate is built with `--features ndarray-compat`
- **THEN** `background`, `mathutils`, `normalization`, `xafsutils`, and `xrayfft` resolve to `*_ndarray.rs` files
- **AND** compatibility behavior remains buildable

### Requirement: Behavior-Preserving Naming Refactor
The backend file naming refactor SHALL preserve existing runtime behavior and quality gate outcomes.

#### Scenario: Quality gates remain green after rename
- **WHEN** maintainers run standard quality gates after the rename
- **THEN** core tests, strict clippy, formatting check, and Python binding check succeed
