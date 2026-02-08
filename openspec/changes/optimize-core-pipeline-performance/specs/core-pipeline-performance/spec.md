## ADDED Requirements

### Requirement: Pipeline Performance Baseline and Targets
The system SHALL establish and maintain reproducible performance baselines for the core XAS pipeline (`normalize -> calc_background -> fft`) and validate improvement targets against those baselines.

#### Scenario: Baseline capture before optimization
- **WHEN** performance optimization work begins
- **THEN** single and parallel benchmark baselines are recorded with environment details and benchmark commands
- **AND** those baselines are stored in repository documentation or CI artifacts

#### Scenario: Throughput target validation
- **WHEN** optimization changes are proposed for merge
- **THEN** benchmark comparison SHALL demonstrate at least 25% faster end-to-end runtime for the defined large-batch benchmark versus baseline

### Requirement: Hot-Path Allocation and Conversion Reduction
The system SHALL reduce avoidable allocations and repeated vector conversions in runtime-critical paths used by normalization, AUTOBK, and FFT preparation.

#### Scenario: AUTOBK conversion pressure reduction
- **WHEN** AUTOBK background processing runs on large batches
- **THEN** the implementation avoids repeated full-vector materialization in tight loops where equivalent view/slice access is possible
- **AND** benchmark or profiling evidence shows reduced overhead in the optimized path

#### Scenario: Normalization and FFT prep allocation reduction
- **WHEN** normalization and FFT preparation execute in pipeline processing
- **THEN** avoidable intermediate allocations in repeated operations are reduced
- **AND** updated benchmarks or profiling reflect reduced runtime contribution for those stages
