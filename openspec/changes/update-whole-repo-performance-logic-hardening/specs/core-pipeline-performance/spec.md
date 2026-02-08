## ADDED Requirements

### Requirement: Hot-Path Representation Consistency
The runtime pipeline SHALL use a consistent internal numeric representation through normalization, background calculation, and FFT preparation, with conversions limited to boundary adapters.

#### Scenario: Pipeline execution avoids repeated representation churn
- **WHEN** processing large spectrum batches through normalize -> calc_background -> fft
- **THEN** repeated internal representation conversions are eliminated from hot loops
- **AND** benchmarks show reduced allocation pressure

### Requirement: AUTOBK Allocation Reduction
AUTOBK path SHALL avoid repeated full-vector materialization and unnecessary clones in tight loops.

#### Scenario: AUTOBK on large batch workload
- **WHEN** AUTOBK executes across many spectra
- **THEN** slice/view/cache strategies are used instead of repeated `to_vec`/full clones
- **AND** profiling shows reduced overhead for those operations

### Requirement: Linear-Time Membership and Group Mutation
Edge-finding membership checks and group index mutations SHALL avoid avoidable quadratic behavior.

#### Scenario: High-derivative point membership checks
- **WHEN** peak or edge detection evaluates neighboring index membership repeatedly
- **THEN** linear-time mask/set lookups are used
- **AND** runtime scales without O(n^2) membership overhead

#### Scenario: Group remove/move mutation operations
- **WHEN** removing or moving many spectra by index
- **THEN** operations use index bitmaps and single-pass mutation
- **AND** no repeated contains-based scanning is used

### Requirement: Throughput Improvement Target
The optimized core pipeline SHALL improve large-batch parallel runtime by a measurable threshold versus the post-unblock baseline.

#### Scenario: 10k-spectrum parallel benchmark comparison
- **WHEN** optimization slices are complete
- **THEN** parallel benchmark runtime improves by 25-40% versus baseline
- **AND** correctness remains within existing numerical tolerances
