# Design: ndarray → nalgebra DVector Migration

## Context

The xraytsubaki codebase currently uses `ndarray::Array1<f64>` (specifically `ArrayBase<OwnedRepr<f64>, Ix1>`) for all 1D vector operations. While ndarray is excellent for NumPy-style array programming, the project needs more specialized linear algebra capabilities that nalgebra provides.

**Key Constraints**:
- Must maintain scientific numerical precision (`TEST_TOL = 1e-12`)
- Must preserve ≥10x Python speedup (10,000 spectra in ~7.5s)
- Rayon parallel processing must continue to function efficiently
- Breaking change acceptable as project is pre-1.0

**Stakeholders**:
- Library users (will need to update code)
- Python bindings (py-xraytsubaki, via PyO3)
- Benchmark/performance tooling
- XAS scientific community (xraylarch compatibility)

## Goals / Non-Goals

### Goals
1. **Internal Standardization**: Use `DVector<f64>` for ALL internal vector operations
2. **Zero Performance Regression**: Maintain or improve current performance benchmarks
3. **Numerical Accuracy**: Preserve scientific precision to 1e-12 tolerance
4. **Clean API**: Consistent use of `DVector<f64>` across all public interfaces
5. **Optional Compatibility**: Support `Array1<f64>` inputs via optional `ndarray-compat` feature
6. **Binary Size Optimization**: Default build without ndarray dependency for pure nalgebra users

### Non-Goals
- **Multi-dimensional arrays**: Only migrating 1D vectors (Array1), not Array2/ArrayN
- **Python binding changes**: Keep PyO3 interface stable where possible
- **Algorithm changes**: Pure refactoring, no algorithmic improvements
- **GUI integration**: Desktop app development deferred
- **WebAssembly**: WASM support deferred to future work

## Decisions

### Decision 1: Use `DVector<f64>` for all internal storage with optional Array1 input support

**Rationale**:
- `DVector` is heap-allocated dynamic vector, equivalent to `Array1` in flexibility
- All XAS vectors have runtime-determined sizes (spectrum lengths vary)
- Static-size `SVector` inappropriate for scientific data
- `DVector<f64>` provides clear type semantics vs generic `ArrayBase<OwnedRepr<f64>, Ix1>`
- Feature-gated `Into<DVector<f64>>` trait bounds allow both DVector and Array1 inputs when compatibility needed

**Alternatives Considered**:
- `SVector<f64, N>`: Rejected - requires compile-time known sizes
- Keep `ndarray` for some internal operations: Rejected - defeats standardization goal
- Wrapper type `XasVector(DVector<f64>)`: Rejected - unnecessary abstraction layer
- Always require ndarray: Rejected - increases binary size for users who don't need compatibility

### Decision 2: Make ndarray an optional dependency via `ndarray-compat` feature flag

**Rationale**:
- Default build without ndarray reduces binary size for pure nalgebra users
- Users who need Array1 compatibility can opt-in via feature flag
- Maintains backward compatibility for existing users during transition period
- Allows gradual migration path for library consumers
- Feature-gated `From` implementations enable zero-cost abstraction when feature enabled

**Implementation**:
```toml
[dependencies]
nalgebra = { version = "0.34.1", features = ["serde-serialize"] }
ndarray = { version = "0.15.6", optional = true }

[features]
default = []
ndarray-compat = ["ndarray"]
```

```rust
// In nshare.rs
#[cfg(feature = "ndarray-compat")]
impl From<Array1<f64>> for DVector<f64> {
    fn from(arr: Array1<f64>) -> Self {
        DVector::from_vec(arr.to_vec())
    }
}

// In API functions
pub fn set_spectrum(
    &mut self,
    energy: impl Into<DVector<f64>>,
    mu: impl Into<DVector<f64>>
) {
    self.energy = Some(energy.into());
    self.mu = Some(mu.into());
}
```

**Alternatives Considered**:
- Remove ndarray completely: Rejected - too disruptive for existing users
- Keep ndarray as required dependency: Rejected - defeats binary size optimization goal
- Use runtime feature detection: Rejected - adds complexity and runtime overhead

### Decision 3: Migrate AUTOBK spline fitting to pure nalgebra

**Rationale**:
- AUTOBK is most complex algorithm, uses spline fitting with Levenberg-Marquardt
- `levenberg-marquardt` crate already uses nalgebra `DVector`
- Existing ndarray-based spline code can translate directly to DVector operations
- Analytical Jacobian computation already uses matrix operations suitable for nalgebra

**Implementation**:
```rust
// Current (ndarray)
let mut spl_y: Array1<f64> = Array1::ones(Ix1(nspl as usize));
let mut spl_k: Array1<f64> = Array1::zeros(nspl as usize);

// New (nalgebra)
let mut spl_y: DVector<f64> = DVector::from_element(nspl as usize, 1.0);
let mut spl_k: DVector<f64> = DVector::zeros(nspl as usize);
```

**Risk**: Numerical differences in spline basis evaluation
**Mitigation**: Extensive testing against reference data from xraylarch

### Decision 4: Update serialization format for DVector

**Rationale**:
- nalgebra `DVector` with serde feature serializes differently than ndarray `Array1`
- Breaking change acceptable pre-1.0
- Provides cleaner JSON/BSON representation

**Format Comparison**:
```json
// ndarray Array1 serialization
{"energy": {"v": 1, "dim": [100], "data": [...]}}

// nalgebra DVector serialization
{"energy": {"data": {"v": [...]}, "nrows": 100, "ncols": 1}}
```

**Migration Path**:
- Provide deserialization support for both formats during transition
- Document format change in migration guide
- Offer conversion utility for existing data files

### Decision 5: Maintain existing trait abstractions

**Rationale**:
- `MathUtils`, `XAFSUtils`, `Normalization` traits provide good abstractions
- Re-implement these traits for `DVector<f64>` instead of `Array1<f64>`
- Maintains API ergonomics and extensibility

**Implementation**:
```rust
impl XAFSUtils for DVector<f64> {
    fn is_sorted(&self) -> bool {
        // DVector-specific implementation
    }

    fn argsort(&self) -> Vec<usize> {
        // DVector-specific implementation
    }
}
```

### Decision 6: TDD migration approach

**Rationale**:
- Write DVector-based tests BEFORE migrating implementation
- Ensures no functionality regression
- Provides clear validation criteria
- Red-Green-Refactor cycle for each module

**Process**:
1. Write failing tests for DVector API
2. Migrate implementation to make tests pass
3. Validate against reference data
4. Move to next module

## Risks / Trade-offs

### Risk 1: Performance Regression from DVector Operations
**Impact**: High - Could violate ≥10x Python speedup requirement
**Probability**: Medium - nalgebra generally fast, but vector operations differ
**Mitigation**:
- Comprehensive benchmarking before/after migration
- Profile with pprof to identify bottlenecks
- Enable BLAS backend for nalgebra if needed
- Rayon parallelism should offset minor operation overhead

### Risk 2: AUTOBK Numerical Accuracy
**Impact**: High - Core algorithm, must match xraylarch results
**Probability**: Medium - Spline fitting sensitive to numerical precision
**Mitigation**:
- Extensive validation against test files: `Ru_QAS_pre_post_edge_expected.dat`
- Maintain `TEST_TOL = 1e-12` for critical calculations
- Compare residuals and chi-square values with reference implementation
- Incremental testing during spline coefficient updates

### Risk 3: Breaking User Code
**Impact**: Medium - All library users must update
**Probability**: High - Intentional breaking change
**Mitigation**:
- Clear migration guide with before/after examples
- Semantic versioning: bump to 0.2.0
- Announce in README and CHANGELOG
- Provide migration script for common patterns

### Risk 4: Rayon Parallel Processing Compatibility
**Impact**: High - Parallel processing is key performance feature
**Probability**: Low - DVector is Send + Sync, should work with Rayon
**Mitigation**:
- Early testing of `find_e0_par`, `normalize_par`, `calc_background_par`
- Benchmark parallel performance specifically
- Monitor memory allocation patterns in parallel contexts

### Risk 5: Dependency Ecosystem Compatibility
**Impact**: Medium - Some dependencies may expect ndarray
**Probability**: Low - Most XAS-specific code is internal
**Mitigation**:
- Audit: `polyfit-rs`, `rusty-fitpack`, `data_reader` compatibility
- Implement conversion adapters if needed
- Fork/patch dependencies if necessary (last resort)

## Migration Plan

### Phase 1: Foundation (Low Risk)
**Duration**: 2-3 days
**Deliverables**:
- Updated dependencies
- Baseline benchmarks
- TDD test infrastructure
- Utility function migrations (mathutils, xafsutils, lmutils)

**Validation**:
- `cargo check` passes
- Utility tests pass
- No performance regression in utilities

### Phase 2: Core Data Structures (Medium Risk)
**Duration**: 3-4 days
**Deliverables**:
- `XASSpectrum` with DVector fields
- `XASGroup` updated
- Getter/setter methods migrated

**Validation**:
- Integration tests pass
- Serialization round-trips correctly
- Memory usage comparable

### Phase 3: Algorithms (High Risk)
**Duration**: 5-7 days
**Deliverables**:
- Normalization migrated and validated
- AUTOBK migrated and validated
- FFT operations migrated and validated

**Validation**:
- All algorithm tests pass
- Reference data validation within tolerance
- Numerical accuracy preserved

### Phase 4: Integration & Performance (Critical)
**Duration**: 2-3 days
**Deliverables**:
- Full test suite passing
- Benchmark comparison report
- Performance validation

**Validation**:
- `cargo test --all-features` passes
- ≥10x Python speedup maintained
- No clippy warnings

### Phase 5: Feature Configuration & Documentation (Low Risk)
**Duration**: 2-3 days
**Deliverables**:
- ndarray configured as optional dependency with `ndarray-compat` feature
- Feature-gated conversions in nshare.rs
- Migration guide with both migration paths
- Updated API documentation

**Validation**:
- `cargo tree` shows no ndarray by default
- `cargo tree --features ndarray-compat` includes ndarray
- Both feature configurations pass tests
- Documentation builds cleanly for both configurations
- Migration guide tested for both paths

**Total Estimated Duration**: 14-20 days

### Rollback Plan
If critical issues discovered during Phase 3-4:
1. Revert to pre-migration git tag
2. Keep nalgebra alongside ndarray temporarily
3. Migrate incrementally with feature flags
4. Extend timeline for more thorough testing

## Open Questions

1. **polyfit-rs compatibility**: Does polynomial fitting library work with DVector?
   - **Resolution**: Test early, implement adapter if needed

2. **rusty-fitpack spline fitting**: Native ndarray support?
   - **Resolution**: Check API, may need conversion layer

3. **serde_arrow integration**: Arrow columnar format with DVector?
   - **Resolution**: Verify during serialization migration, may need custom impl

4. **Python bindings impact**: How does PyO3 handle DVector?
   - **Resolution**: Test py-xraytsubaki early, ensure smooth FFI

5. **BLAS backend configuration**: Should we enable BLAS for performance?
   - **Resolution**: Benchmark with/without, enable if significant improvement

6. **Float precision differences**: Any f64 operation differences?
   - **Resolution**: Monitor test tolerances, adjust if needed

## Success Metrics

**Quantitative**:
- ✅ ndarray absent in default `cargo tree`, present with `--features ndarray-compat`
- ✅ 100% test pass rate (all 82+ existing tests) in both feature configurations
- ✅ ≤7.5s for 10,000 spectra on M1 MacBook Pro (10 cores)
- ✅ Memory usage ≤ baseline (current ndarray implementation)
- ✅ Numerical accuracy within 1e-12 for critical calculations
- ✅ 0 clippy warnings with both default and `--all-features`
- ✅ Feature-gated conversions work correctly (Array1 → DVector with feature enabled)

**Qualitative**:
- ✅ Clean, idiomatic nalgebra usage throughout codebase
- ✅ Comprehensive migration guide covers both migration paths
- ✅ Maintainable code with clear DVector semantics
- ✅ Scientific accuracy validated against xraylarch reference
- ✅ Smooth transition path for existing users via feature flag
