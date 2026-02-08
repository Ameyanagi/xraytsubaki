## ADDED Requirements

### Requirement: Stable Toolchain Build Compatibility
The system SHALL support successful build and test execution for the core crate on the current stable Rust toolchain.

#### Scenario: Core crate test execution on stable
- **WHEN** maintainers run core crate tests on stable Rust
- **THEN** `cargo test -p xraytsubaki` succeeds without toolchain-level dependency compatibility errors

### Requirement: CI Compatibility and Performance Regression Detection
The system SHALL include CI checks that detect compatibility breakage and performance regressions in the core crate.

#### Scenario: Toolchain compatibility matrix
- **WHEN** CI runs for pull requests affecting core pipeline code
- **THEN** core crate checks execute on stable and beta toolchains
- **AND** failures block merge

#### Scenario: Benchmark regression visibility
- **WHEN** CI runs benchmark validation for core pipeline performance
- **THEN** benchmark outputs are published as artifacts or logs
- **AND** configured regression thresholds are evaluated against baseline metrics
