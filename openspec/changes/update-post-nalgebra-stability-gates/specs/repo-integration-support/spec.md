## ADDED Requirements

### Requirement: Python Binding Build Health
The repository SHALL keep the Python binding crate buildable against the current core crate API.

#### Scenario: Python binding compiles
- **WHEN** maintainers run `cargo check --manifest-path py-xraytsubaki/Cargo.toml`
- **THEN** compilation succeeds without type-mismatch failures in the exposed stable API functions

### Requirement: Stable Python Pipeline Function Contract
The Python stable functions SHALL preserve their documented contract while internal build fixes are applied.

#### Scenario: Batch API contract remains stable
- **WHEN** callers invoke `run_batch_qas_trans`
- **THEN** the function returns `(processed_count, errors)`
- **AND** each error row contains `(index, category, message)`

#### Scenario: Array pipeline contract remains stable
- **WHEN** callers invoke `run_pipeline_arrays`
- **THEN** the function returns a dictionary containing `e0`
- **AND** `k`, `chi`, and `chir_mag` are provided when available

### Requirement: Integration Gate Coverage in CI
The repository SHALL run Python binding build checks in CI as part of post-migration stability validation.

#### Scenario: Binding regressions are caught pre-merge
- **WHEN** a pull request changes core or binding code that affects Python integration
- **THEN** CI runs the binding build check
- **AND** failures block merge
