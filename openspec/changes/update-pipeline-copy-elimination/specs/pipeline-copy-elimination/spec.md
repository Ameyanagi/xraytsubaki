## ADDED Requirements

### Requirement: Borrowed Pipeline Accessors for Hot Paths
The system SHALL expose borrowed accessor paths for hot-path `k`/`chi` retrieval so internal stage transitions avoid clone-based materialization.

#### Scenario: Internal FFT path uses borrowed spectrum data
- **WHEN** `XASSpectrum` executes the `fft` stage after background calculation
- **THEN** internal retrieval of `k` and `chi` uses borrowed views from stored background outputs
- **AND** the stage does not require clone-returning getters for normal execution

#### Scenario: Compatibility getters remain available
- **WHEN** external callers request existing owned getters for `k` and `chi`
- **THEN** the API still returns owned vectors with equivalent numerical content
- **AND** behavior remains backward compatible for existing caller code

### Requirement: Stage Boundary Conversion Reduction
The system SHALL reduce avoidable full-vector conversions at normalization and background stage boundaries.

#### Scenario: Normalization input conversion path
- **WHEN** normalization is invoked with validated spectrum inputs
- **THEN** the implementation consumes borrowed contiguous views where possible
- **AND** avoids repeated full clones solely for type adaptation

#### Scenario: AUTOBK input conversion path
- **WHEN** AUTOBK background processing executes across batch workloads
- **THEN** input conversion avoids repeated full-vector materialization in hot execution paths
- **AND** ownership conversion is limited to places where downstream algorithms require owned buffers

### Requirement: Copy-Elimination Performance Evidence
The system SHALL provide before/after evidence that copy-elimination changes improve or maintain runtime and reduce allocation pressure for representative workloads.

#### Scenario: Benchmark comparison for copy-elimination slice
- **WHEN** copy-elimination changes are completed
- **THEN** benchmark medians are captured for single, parallel, and AUTOBK-stage benchmarks using the documented command set
- **AND** no benchmark regresses beyond configured project thresholds

#### Scenario: Allocation comparison for copy-elimination slice
- **WHEN** allocation instrumentation is run before and after the change
- **THEN** allocation call and byte metrics are compared and documented
- **AND** results demonstrate reduced or equivalent allocation pressure for the optimized path
