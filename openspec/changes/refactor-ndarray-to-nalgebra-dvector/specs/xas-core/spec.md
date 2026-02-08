## ADDED Requirements

### Requirement: Nalgebra-First Internal Vector Standard
The system SHALL treat `nalgebra::DVector<f64>` as the canonical internal 1D vector representation for XAS core computation paths.

#### Scenario: Core spectrum and pipeline stages use DVector internally
- **WHEN** XAS data moves through spectrum, normalization, background, and FFT processing
- **THEN** internal vector storage and computation use `DVector<f64>` as the primary type
- **AND** ndarray types are not required for nalgebra-only builds

### Requirement: Feature-Gated ndarray Compatibility Boundary
The system SHALL isolate ndarray-dependent interop to `ndarray-compat`-gated compatibility adapters.

#### Scenario: Compatibility wrappers are gated
- **WHEN** ndarray interop is needed for legacy callers
- **THEN** ndarray-specific APIs and conversions are available only with `ndarray-compat`
- **AND** the nalgebra-only build surface does not expose unconditional ndarray requirements

### Requirement: Dual Build-Mode Support During Migration
The system SHALL keep both nalgebra-only and compatibility-enabled builds valid throughout migration phases.

#### Scenario: Nalgebra-only build check
- **WHEN** `cargo check -p xraytsubaki --no-default-features` is run
- **THEN** the crate compiles without unresolved ndarray symbols or feature-gated trait gaps

#### Scenario: Compatibility build check
- **WHEN** `cargo check -p xraytsubaki --features ndarray-compat` is run
- **THEN** compatibility adapters compile and existing ndarray-facing callers remain supported

### Requirement: Numerical Parity for Core Algorithms
The system SHALL preserve numerical behavior for normalization, background removal, and FFT within existing project tolerances during migration.

#### Scenario: Regression tests for core stages
- **WHEN** migration changes are applied to core algorithms
- **THEN** existing tests for normalization/background/FFT remain green within current tolerance constants
- **AND** migration does not intentionally alter scientific algorithm behavior

### Requirement: Serialization Compatibility Expectations
The system SHALL preserve JSON/BSON round-trip behavior for XAS data structures, or provide explicit compatibility handling when representation changes are unavoidable.

#### Scenario: Round-trip safety
- **WHEN** spectrum/group data is written then read using project JSON/BSON paths
- **THEN** vector data round-trips correctly for supported migration modes
- **AND** representation differences are handled explicitly by compatibility code when needed

### Requirement: Performance Regression Guardrail
The system SHALL validate migration steps against existing benchmark/allocation checks for pipeline-critical workloads.

#### Scenario: Benchmark and allocation verification
- **WHEN** nalgebra migration phases are completed for pipeline-critical modules
- **THEN** benchmark/allocation checks are rerun and compared with baseline data
- **AND** no material regression is introduced without explicit approval
