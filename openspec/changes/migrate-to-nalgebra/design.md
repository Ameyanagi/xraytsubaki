# Design: ndarray to nalgebra Migration

## Context

The xraytsubaki project currently uses `ndarray` for 1D vector operations in XAS data analysis, while also depending on `nalgebra` (used by `levenberg-marquardt` for optimization). This dual dependency creates:

- **Binary bloat**: Two linear algebra libraries with overlapping functionality
- **API inconsistency**: Different vector types for different operations
- **Maintenance burden**: Potential version conflicts and duplicate abstractions

The migration consolidates on `nalgebra` as the single linear algebra foundation, leveraging its:
- BLAS/LAPACK backend support for performance
- Advanced decompositions and linear algebra operations
- Better static typing and compile-time optimizations
- Existing integration with scientific computing crates

**Stakeholders**: XAS researchers using xraytsubaki library, Python wrapper users (py-xraytsubaki), developers maintaining the codebase

## Goals / Non-Goals

### Goals
- **Complete migration**: Replace all `ndarray::Array1<f64>` with `nalgebra::DVector<f64>`
- **Maintain performance**: Preserve or improve 10x+ speedup over Python baseline
- **TDD compliance**: Write tests before implementation following red-green-refactor
- **API clarity**: Use `DVector<f64>` consistently for dynamic-length vectors
- **Preserve correctness**: All existing tests must pass with new implementation
- **Benchmark validation**: Comprehensive performance comparison with old implementation

### Non-Goals
- **Static sizing**: Not migrating to `SVector` (XAS spectra have variable length)
- **2D/Matrix operations**: Focus on 1D vectors only (no Array2 → DMatrix migration)
- **GPU acceleration**: Not implementing GPU backends in this change
- **Breaking serialization**: Maintain JSON/BSON format compatibility where possible
- **Python API changes**: py-xraytsubaki wrapper is out of scope (future work)

## Decisions

### Decision 1: Use DVector<f64> for All Spectral Data
**Rationale**: XAS spectra have variable length (different energy ranges, sampling rates). DVector provides:
- Heap-allocated, dynamic sizing
- Efficient resizing and concatenation
- Compatible with existing algorithms (AUTOBK, FFT)
- Serde support for serialization

**Alternatives considered**:
- `SVector<f64, N>`: Rejected - requires compile-time known size
- Keep ndarray: Rejected - doesn't solve dual-dependency problem
- `Vec<f64>`: Rejected - loses linear algebra operations

### Decision 2: Update nalgebra to 0.34.1 (Latest Stable)
**Rationale**:
- Security fixes and performance improvements since 0.32.4
- Better SIMD support and const generics
- Improved serde integration
- API stability guarantees (0.x versions maintain compatibility)

**Migration impact**:
- Review levenberg-marquardt (0.13.1) compatibility
- Check if polyfit-rs, rusty-fitpack need updates
- Test BLAS backend integration

### Decision 3: TDD with Separate Test Modules
**Rationale**: Following strict TDD discipline ensures:
- Tests written before implementation (red-green-refactor)
- Clear specifications for expected behavior
- Regression protection during migration
- Separate test files prevent pollution of existing test suite

**Structure**:
```
tests/
  nalgebra_migration/
    mod.rs                      # Module registration
    test_utils.rs               # Shared test utilities
    test_xasspectrum.rs         # Core data structure tests
    test_normalization.rs       # Algorithm tests
    test_background.rs          # AUTOBK tests
    ...
```

### Decision 4: Phased Migration Strategy
**Rationale**: Minimize risk and enable incremental validation

**Phase order**:
1. **Core structures** (mathutils, xasspectrum): Foundation for all operations
2. **Utilities** (xafsutils): Used by higher-level algorithms
3. **Algorithms** (normalization, background, FFT): Build on core structures
4. **I/O** (serialization): Adapt to new data types
5. **Validation** (tests, benchmarks): Ensure correctness and performance

Each phase completes TDD cycle before moving to next phase.

### Decision 5: Preserve Existing Tests as Regression Suite
**Rationale**:
- Existing tests validate against reference data (Ru_QAS dataset)
- Provide regression protection during migration
- Verify numerical accuracy is maintained

**Approach**:
- Keep `xafs/tests.rs` unchanged initially
- Update only when API changes require it
- Use `approx` crate for floating-point comparisons
- Maintain same tolerance thresholds (TEST_TOL, TEST_TOL_LESS_ACC)

### Decision 6: Benchmark-Driven Performance Validation
**Rationale**: Performance is critical success criterion

**Metrics**:
- **Baseline**: Current ndarray implementation (save before migration)
- **Target**: ≥10x speedup over NumPy + xraylarch (145s for 10,000 spectra)
- **Acceptance**: nalgebra ≤110% of ndarray baseline (allow 10% regression)

**Benchmarks**:
- Parallel processing (existing): `xas_group_benchmark_parallel`
- Comparative (new): ndarray vs nalgebra head-to-head
- Profiling: pprof flamegraphs for bottleneck analysis

## Risks / Trade-offs

### Risk 1: Performance Regression
**Risk**: nalgebra may be slower than ndarray for certain operations
**Likelihood**: Medium
**Impact**: High (violates performance goal)
**Mitigation**:
- Run benchmarks early in migration (after core structures)
- Use BLAS backend for nalgebra (configure with `openblas` or `netlib` feature)
- Profile hotspots with pprof, optimize critical paths
- Rollback option: Keep ndarray branch until performance validated

### Risk 2: Serialization Format Breaking Changes
**Risk**: DVector serialization differs from Array1, breaking existing data files
**Likelihood**: High
**Impact**: Medium (users must re-serialize data)
**Mitigation**:
- Test JSON/BSON format compatibility early
- Provide migration tool if format changes
- Document migration path in MIGRATION.md
- Consider custom serde serializer for backwards compatibility

### Risk 3: Dependency Version Conflicts
**Risk**: nalgebra 0.34.1 incompatible with levenberg-marquardt or other deps
**Likelihood**: Low
**Impact**: High (blocks migration)
**Mitigation**:
- Audit dependencies in setup phase (Task 1.3)
- Test levenberg-marquardt with nalgebra 0.34.1 early
- Contact maintainers if incompatibility found
- Fork/vendor dependencies as last resort

### Risk 4: FFT Library Integration Issues
**Risk**: easyfft may have coupling with ndarray types
**Likelihood**: Medium
**Impact**: Medium (complex refactor required)
**Mitigation**:
- Review easyfft API for DVector compatibility
- Test FFT operations early (Task 7)
- Consider alternative FFT libraries if needed (rustfft)
- Implement adapter layer if direct integration fails

### Risk 5: Rayon Parallel Processing Compatibility
**Risk**: Rayon iterator methods may not work efficiently with DVector
**Likelihood**: Low
**Impact**: High (breaks parallel speedup)
**Mitigation**:
- Test parallel operations in Task 9.2
- Benchmark parallel vs sequential to verify scaling
- Use manual chunking if rayon integration suboptimal

## Migration Plan

### Phase 1: Setup and Baseline (Tasks 1-2)
1. Update dependencies in Cargo.toml
2. Run baseline benchmarks, save results
3. Create test infrastructure
4. Write all TDD tests (should FAIL initially)

**Validation**: Tests compile, baseline benchmarks saved

### Phase 2: Core Migration (Tasks 3-4)
1. Migrate mathutils and xasspectrum (foundation)
2. Update xafsutils (depends on core)
3. Run new tests (should PASS)

**Validation**: `cargo test nalgebra_migration::test_{xasspectrum,mathutils,xafsutils}` passes

### Phase 3: Algorithm Migration (Tasks 5-7)
1. Migrate normalization (moderate complexity)
2. Migrate background/AUTOBK (highest complexity - LM integration)
3. Migrate FFT operations
4. Run new tests for each

**Validation**: All algorithm tests pass, numerical results match reference data

### Phase 4: I/O and Integration (Tasks 8-9)
1. Update serialization
2. Migrate group operations
3. Run integration tests

**Validation**: Full test suite passes, serialization round-trips correctly

### Phase 5: Performance Validation (Task 11)
1. Run comparative benchmarks
2. Profile with pprof
3. Optimize hotspots if needed
4. **Go/No-Go Decision**: Proceed only if performance acceptable

**Validation**: nalgebra performance ≤110% of ndarray baseline

### Phase 6: Cleanup and Documentation (Tasks 12-13)
1. Remove ndarray dependency
2. Update documentation
3. Create migration guide

**Validation**: `cargo check` passes, no ndarray references remain

### Rollback Plan
If performance validation fails (Phase 5):
1. Revert to git tag before migration
2. Keep ndarray for performance-critical paths
3. Use nalgebra only for algorithms requiring it (LM)
4. Document decision and revisit after nalgebra optimization

## Open Questions

1. **BLAS Backend**: Which BLAS implementation should we use? (openblas, netlib, intel-mkl)
   - Consider cross-platform support (Linux, macOS, Windows)
   - Benchmark different backends
   - Document build requirements

2. **Custom Serde Serializers**: Should we implement custom serialization for backwards compatibility?
   - Test existing JSON/BSON files in Task 8.5
   - Measure impact on users
   - Decide based on migration cost vs compatibility value

3. **Static Analysis**: Should we add additional lints or clippy rules for nalgebra best practices?
   - Prevent accidental performance pitfalls
   - Ensure consistent usage patterns

4. **Python Wrapper Impact**: How will py-xraytsubaki be affected?
   - PyO3 integration with nalgebra
   - NumPy array conversion
   - Defer to separate change or document requirements

## Success Criteria

✅ **Correctness**: All tests pass (existing + new TDD tests)
✅ **Performance**: nalgebra ≤110% of ndarray baseline, maintain 10x+ vs Python
✅ **Completeness**: Zero ndarray dependencies remain
✅ **Quality**: Benchmark reports and migration documentation complete
✅ **TDD Compliance**: All tests written before implementation, follow red-green-refactor
