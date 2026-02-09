## ADDED Requirements

### Requirement: Post-Migration Strict Core Gate
The system SHALL provide a strict, reproducible quality gate for the core crate in its nalgebra-first default configuration.

#### Scenario: Strict core gate passes locally
- **WHEN** maintainers run the canonical gate commands on stable Rust
- **THEN** `cargo test -p xraytsubaki` succeeds
- **AND** `cargo clippy -p xraytsubaki --all-targets -- -D warnings` succeeds
- **AND** `cargo fmt --all -- --check` succeeds

### Requirement: Optional Compatibility Build Guard
The system SHALL retain build viability for optional compatibility mode while default runtime remains nalgebra-first.

#### Scenario: Optional compatibility build remains available
- **WHEN** maintainers build with `--features ndarray-compat`
- **THEN** core crate compilation succeeds
- **AND** compatibility-specific failures are surfaced before merge

### Requirement: CI Gate Parity
The system SHALL enforce in CI the same strict core commands used for local acceptance.

#### Scenario: CI blocks on strict gate failure
- **WHEN** a pull request introduces formatting, lint, or test regressions in core code
- **THEN** CI fails the relevant job
- **AND** merge is blocked until the strict gate is green
