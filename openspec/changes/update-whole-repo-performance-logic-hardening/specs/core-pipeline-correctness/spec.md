## ADDED Requirements

### Requirement: Non-Panicking Batch Processing
`XASGroup` batch processing methods SHALL return structured aggregate failures instead of panicking on recoverable spectrum-level errors.

#### Scenario: Sequential batch has one failing spectrum
- **WHEN** a sequential batch operation encounters a recoverable spectrum failure
- **THEN** the method returns a structured error containing failed index and cause
- **AND** the method does not panic

#### Scenario: Parallel batch has multiple failing spectra
- **WHEN** a parallel batch operation encounters one or more recoverable failures
- **THEN** the method returns an aggregate structured error with per-spectrum index and cause
- **AND** the method does not panic

### Requirement: FFT Imaginary Getter Correctness
FFT getter methods SHALL return correct component channels.

#### Scenario: Imaginary channel retrieval
- **WHEN** callers request imaginary `chi(R)` values
- **THEN** returned values correspond to the imaginary component
- **AND** regression tests prove it differs from the real component for nontrivial complex output

### Requirement: Runtime Invariant Validation
Core pipeline stages SHALL validate key data invariants before heavy numeric operations.

#### Scenario: Mismatched or non-finite inputs
- **WHEN** energy/mu lengths mismatch or inputs contain non-finite values
- **THEN** typed validation errors are returned
- **AND** heavy numeric processing is skipped

#### Scenario: Non-monotonic energy assumptions violated
- **WHEN** a stage requiring monotonic energy receives non-monotonic input
- **THEN** a typed validation error is returned with actionable context
