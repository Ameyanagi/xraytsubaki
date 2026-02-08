## ADDED Requirements

### Requirement: Stable Build and Test Unblock
The system SHALL provide a dependency graph for the core crate that compiles and tests on current stable Rust without legacy FFT transitive incompatibilities.

#### Scenario: Core stable test execution succeeds
- **WHEN** maintainers run `cargo test -p xraytsubaki` on stable Rust
- **THEN** the command succeeds without `anymap`/legacy FFT compatibility errors

### Requirement: Core Toolchain Matrix
The system SHALL run core crate compile/test checks on stable and beta toolchains in CI.

#### Scenario: Pull request compatibility coverage
- **WHEN** a pull request touches core crate code
- **THEN** CI runs stable and beta jobs for the core crate
- **AND** failures block merge

### Requirement: Baseline and Regression Gate Workflow
The system SHALL capture benchmark baselines after compile unblock and evaluate regressions in CI.

#### Scenario: Baseline capture after unblock
- **WHEN** build/test reliability is restored
- **THEN** baseline metrics are recorded for single and parallel benchmarks
- **AND** metrics include p50/p95 runtime, allocation count, and max RSS

#### Scenario: Regression signal progression
- **WHEN** benchmark checks run in CI
- **THEN** regression thresholds are first informational
- **AND** later promoted to blocking after baseline stabilization
