# Spec: XAFS Performance

## ADDED Requirements

### Requirement: Precomputed Spline Basis
The AUTOBK algorithm SHALL precompute the spline basis Jacobian matrix once during initialization rather than computing it on every iteration.

#### Scenario: Initial spectrum background calculation
**WHEN** a user calls `calc_background()` on an XAS spectrum for the first time
**THEN** the spline basis Jacobian matrix shall be computed once during AUTOBKSpline construction
**AND** subsequent Jacobian evaluations shall reference the precomputed matrix
**AND** the total optimization time shall be reduced by at least 25%

#### Scenario: Batch processing of multiple spectra
**WHEN** processing 100 spectra in parallel using `XASGroup::calc_background()`
**THEN** each spectrum's precomputed basis shall be independent
**AND** memory usage shall increase by approximately 800KB per spectrum
**AND** total batch processing time shall be reduced by at least 25%

### Requirement: Optimized Memory Allocation
The Jacobian computation SHALL minimize heap allocations by pre-allocating buffers with final sizes.

#### Scenario: Jacobian column processing
**WHEN** computing Jacobian columns during optimization iteration
**THEN** output buffers shall be pre-allocated with final size
**AND** direct memory writes shall be used instead of vector extensions
**AND** the number of heap allocations per iteration shall be reduced by at least 50%

### Requirement: FFT Plan Caching
The AUTOBK algorithm SHALL reuse FFT plans across multiple FFT operations within a single optimization.

#### Scenario: Multiple FFT operations in Jacobian computation
**WHEN** computing Jacobian matrix with 50+ spline coefficients
**THEN** FFT plan shall be created once and cached
**AND** all subsequent FFT operations in the same optimization shall reuse the cached plan
**AND** FFT setup overhead shall be reduced by at least 50%

### Requirement: Numerical Accuracy Preservation
All performance optimizations SHALL preserve numerical accuracy to within 1e-10 relative error of the original implementation.

#### Scenario: Optimization result validation
**WHEN** comparing optimized implementation against reference implementation
**THEN** the final chi(k) values shall match within 1e-10 relative error
**AND** the number of LM iterations shall match exactly
**AND** convergence criteria shall be identical

#### Scenario: Edge case handling
**WHEN** processing edge cases (very small spectra, extreme parameters)
**THEN** numerical accuracy shall remain within 1e-10 relative error
**AND** no numerical instabilities shall be introduced

### Requirement: Memory Layout Optimization
When cache profiling indicates significant cache misses, the precomputed basis matrix SHALL be transposed to optimize memory access patterns.

#### Scenario: Cache-efficient column iteration
**WHEN** cache profiling indicates significant cache misses during column iteration
**THEN** the implementation may transpose the precomputed basis matrix
**AND** column iteration shall use contiguous row access
**AND** cache miss rate shall be reduced by at least 20%
**AND** numerical results shall remain unchanged

### Requirement: Performance Regression Testing
The codebase SHALL include automated performance benchmarks to prevent regression.

#### Scenario: CI pipeline performance validation
**WHEN** a pull request is submitted
**THEN** benchmarks shall run comparing against main branch baseline
**AND** any performance regression greater than 5% shall fail the CI check
**AND** performance improvements shall update the baseline

#### Scenario: Benchmark coverage
**WHEN** running the benchmark suite
**THEN** benchmarks shall cover Jacobian computation
**AND** benchmarks shall cover full AUTOBK optimization
**AND** benchmarks shall test multiple spectrum sizes and parameter combinations

### Requirement: Thread Safety Documentation
The AUTOBKSpline implementation SHALL document thread safety constraints due to interior mutability.

#### Scenario: Concurrent usage documentation
**WHEN** a developer reads the AUTOBKSpline documentation
**THEN** it shall clearly state the struct is not thread-safe due to RefCell usage
**AND** it shall specify that one instance should be used per thread
**AND** it shall document that Rayon parallelism occurs at the spectrum level, not within optimization

### Requirement: Memory Overhead Documentation
The documentation SHALL clearly state the memory trade-off for performance optimization.

#### Scenario: User decision-making
**WHEN** a user reads the AUTOBK documentation
**THEN** it shall state that precomputation adds ~800KB per spectrum
**AND** it shall explain that this is exchanged for 25-35% performance improvement
**AND** it shall provide guidance for memory-constrained environments

### Requirement: Compatibility Guarantee
The performance optimizations SHALL maintain complete backward compatibility with the existing API.

#### Scenario: Existing code continues to work
**WHEN** a user upgrades to the optimized version
**THEN** all existing calls to `calc_background()` shall work unchanged
**AND** all existing parameter configurations shall work unchanged
**AND** no breaking changes shall be introduced to public API

#### Scenario: Serialization compatibility
**WHEN** spectra are saved to BSON format
**THEN** the precomputed_basis field shall not be serialized
**AND** deserialized spectra shall recompute the basis on load
**AND** serialized format shall remain unchanged

## MODIFIED Requirements

### Requirement: AUTOBKSpline Structure
The `AUTOBKSpline` struct SHALL include a field for the precomputed basis Jacobian matrix.

**Before**: Struct contained only knots, coefficients, order, and runtime data
**After**: Struct includes `precomputed_basis: DMatrix<f64>` field

#### Scenario: Struct initialization
**WHEN** an AUTOBKSpline instance is created
**THEN** the precomputed_basis field shall be populated with the basis Jacobian matrix
**AND** the matrix dimensions shall match (num_points × num_coefs)
**AND** the matrix shall be immutable after initialization

### Requirement: Jacobian Computation Method
The `residual_jacobian` method SHALL reference the precomputed basis instead of calling `splev_jacobian`.

**Before**: Called `splev_jacobian()` on every iteration with vector clones
**After**: References `&self.precomputed_basis` directly

#### Scenario: Jacobian evaluation during optimization
**WHEN** the LM solver requests a Jacobian matrix
**THEN** the method shall use a reference to precomputed_basis
**AND** no `splev_jacobian` call shall occur
**AND** no vector clones shall be performed
**AND** the result shall be numerically identical to the previous implementation
