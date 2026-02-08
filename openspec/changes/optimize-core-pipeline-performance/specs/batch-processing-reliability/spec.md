## ADDED Requirements

### Requirement: Fallible Batch Processing
The system SHALL provide non-panicking error behavior for production batch processing operations in `XASGroup` processing methods.

#### Scenario: Sequential batch processing contains a failing spectrum
- **WHEN** a sequential processing method encounters a recoverable spectrum-level failure
- **THEN** the method returns a typed error instead of panicking
- **AND** the error includes the index of the failing spectrum and the underlying cause

#### Scenario: Parallel batch processing contains a failing spectrum
- **WHEN** a parallel processing method encounters one or more recoverable spectrum-level failures
- **THEN** the method returns a typed aggregated failure instead of panicking
- **AND** each reported failure includes at least spectrum index and source error context

### Requirement: FFT Output Getter Correctness
The system SHALL return correct real and imaginary FFT channel outputs from exposed getter methods.

#### Scenario: Imaginary channel retrieval
- **WHEN** callers request the imaginary FFT channel output
- **THEN** the returned values correspond to the imaginary component
- **AND** regression tests verify the imaginary getter is not equivalent to the real getter for non-zero-imaginary data
