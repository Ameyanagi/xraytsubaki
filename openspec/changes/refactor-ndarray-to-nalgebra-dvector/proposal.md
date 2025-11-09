# Change: Refactor from ndarray to nalgebra DVector for All Internal Vector Operations

## Why

**Current State Analysis**:
- Codebase uses `ndarray::Array1<f64>` (specifically `ArrayBase<OwnedRepr<f64>, Ix1>`) for all 1D vector operations across 10 core modules
- 176 occurrences of `Array1` type usage detected
- Existing `nshare.rs` provides conversion utilities but nalgebra is underutilized
- Dual dependency on both `ndarray` (0.15.6) and `nalgebra` (0.32.4) increases binary size and complexity

**Migration Rationale**:
1. **Internal Standardization**: Use `DVector<f64>` for all internal vector operations, providing consistent linear algebra semantics
2. **Optional Compatibility**: Make `ndarray` an optional dependency via `ndarray-compat` feature flag for backward compatibility
3. **Binary Size Optimization**: Default build without ndarray dependency reduces binary size for pure nalgebra users
4. **Linear Algebra Capability**: `nalgebra` provides comprehensive linear algebra operations (decompositions, eigenvalues, advanced matrix operations) that `ndarray` lacks
5. **Ecosystem Alignment**: Several dependencies already use `nalgebra`:
   - `levenberg-marquardt` (0.13.1) - Already uses `DVector` for optimization
   - Better integration with scientific computing ecosystem
6. **Performance**: `nalgebra` with BLAS backend provides comparable or superior performance for dense linear algebra
7. **Type Safety**: `DVector<f64>` provides clearer semantics for vector operations vs generic array types
8. **Future-Proofing**: Better support for GPU acceleration, static typing, and cross-platform optimization

**Performance Requirement Preservation**:
Must maintain existing ≥10x speedup over Python baseline (10,000 spectra in ~7.5s on M1 MacBook Pro)

## What Changes

### Breaking API Changes
**BREAKING**: All public API signatures using `ArrayBase<OwnedRepr<f64>, Ix1>` will change to `nalgebra::DVector<f64>`

Affected public types:
- `XASSpectrum` struct fields (energy, mu, k, chi, chi_r, etc.)
- `XASGroup` parallel processing methods
- `XrayFFTF` / `XrayFFTR` FFT/IFFT operations
- All `BackgroundMethod` and `NormalizationMethod` trait methods
- Serialization formats (JSON/BSON vector representations)

### Internal Refactoring
- Replace all `Array1::` constructors with `DVector::` equivalents
- Update trait implementations (`MathUtils`, `XAFSUtils`, `Normalization`) for `DVector<f64>`
- Migrate utility functions (find_e0, energy_step, interpolation, smoothing)
- Update FFT operations to work with `DVector` inputs
- Refactor AUTOBK spline fitting to use `DVector` for coefficients and knots
- Update Levenberg-Marquardt integration (already uses nalgebra internally)

### Dependency Updates
- Make `ndarray` (0.15.6) an **optional** dependency with feature flag `ndarray-compat`
- Update `nalgebra` to 0.34.1 (latest stable) from 0.32.4
- Configure Cargo.toml:
  ```toml
  [dependencies]
  nalgebra = { version = "0.34.1", features = ["serde-serialize"] }
  ndarray = { version = "0.15.6", optional = true }

  [features]
  default = []
  ndarray-compat = ["ndarray"]
  ```
- Verify compatibility with:
  - `levenberg-marquardt` 0.13.1
  - `polyfit-rs` 0.2.1 (may need nalgebra integration)
  - `rusty-fitpack` 0.1.2 (spline fitting)
  - `serde_arrow` 0.10.0 (columnar data)

### Files Requiring Changes (18 files)

**Core Data Structures** (3 files):
- `xafs/xasspectrum.rs` (line 37-50: field definitions, methods throughout)
- `xafs/xasgroup.rs` (parallel processing container)
- `xafs/xasparameters.rs` (parameter structs)

**Algorithms** (6 files):
- `xafs/background.rs` (AUTOBK implementation, spline fitting)
- `xafs/normalization.rs` (pre/post-edge normalization)
- `xafs/xrayfft.rs` (FFT/IFFT operations)
- `xafs/mathutils.rs` (gaussian, lorentzian, voigt functions)
- `xafs/xafsutils.rs` (find_e0, energy_step, smooth, interpolation)
- `xafs/lmutils.rs` (Levenberg-Marquardt parameter handling)

**Utilities** (3 files):
- `xafs/nshare.rs` (conversion traits - **enhance** with feature-gated Array1↔DVector conversions)
- `xafs/io/xafs_json.rs` (JSON serialization)
- `xafs/io/xafs_bson.rs` (BSON serialization)

**Configuration** (2 files):
- `Cargo.toml` (workspace dependencies)
- `crates/xraytsubaki/Cargo.toml` (crate dependencies)

**Testing & Benchmarks** (4 files):
- `xafs/tests.rs` (integration tests)
- `xafs/mod.rs` (test constants, setup)
- `benches/xas_group_benchmark_parallel.rs` (performance validation)
- New: `benches/nalgebra_comparison.rs` (comparative benchmarks)

## Impact

### Affected Capabilities
- **xas-data-structures**: Core `XASSpectrum` and `XASGroup` types
- **xas-analysis-pipeline**: All EXAFS analysis functions (find_e0, normalization, AUTOBK, FFT)
- **xas-serialization**: JSON/BSON vector field encoding
- **xas-performance**: Parallel processing with Rayon, benchmark targets
- **xas-math-utilities**: Mathematical operations and transformations

### Migration Strategy

**Phase 1: Preparation & Validation** (Low Risk)
1. Update dependencies in `Cargo.toml`
2. Create baseline benchmarks with current ndarray implementation
3. Set up TDD test infrastructure for migration validation
4. Audit dependent crates for nalgebra compatibility

**Phase 2: Core Infrastructure** (Medium Risk)
5. Migrate utility modules (`mathutils`, `xafsutils`, `lmutils`)
6. Update conversion traits in `nshare.rs`
7. Run incremental tests after each utility migration

**Phase 3: Data Structures** (Medium-High Risk)
8. Migrate `XASSpectrum` struct fields and methods
9. Update `XASGroup` parallel processing
10. Validate against reference test data

**Phase 4: Algorithms** (High Risk - Core Functionality)
11. Migrate normalization (`normalization.rs`)
12. Migrate background removal (`background.rs` - most complex, AUTOBK with splines)
13. Migrate FFT operations (`xrayfft.rs`)
14. Comprehensive validation against test files

**Phase 5: I/O & Serialization** (Low-Medium Risk)
15. Update JSON/BSON serialization
16. Create data format migration guide
17. Test round-trip serialization

**Phase 6: Performance Validation** (Critical)
18. Run parallel processing benchmarks
19. Verify ≥10x Python speedup maintained
20. Profile with pprof, generate flamegraphs
21. Compare ndarray baseline vs nalgebra performance

**Phase 7: Cleanup & Feature Configuration** (Low Risk)
22. Configure ndarray as optional dependency with `ndarray-compat` feature
23. Enhance `nshare.rs` with feature-gated conversions
24. Clean up unused imports
25. Update documentation
26. Test both feature configurations: `cargo test` (default) and `cargo test --all-features`
27. Final validation: `cargo test --all-features && cargo bench --all-features`

### Backwards Compatibility

**API Changes with Compatibility Options**:

Internal fields change from `ArrayBase<OwnedRepr<f64>, Ix1>` to `DVector<f64>`, but input compatibility is maintained via:
- **Default behavior**: Accept `DVector<f64>` inputs only
- **With `ndarray-compat` feature**: Accept both `Array1<f64>` and `DVector<f64>` via `Into<DVector<f64>>` trait

**Migration Paths**:

**Option 1: Migrate to nalgebra (Recommended)**
```rust
// Before (ndarray)
use ndarray::Array1;
let energy = Array1::from_vec(vec![1.0, 2.0, 3.0]);
let mu = Array1::from_vec(vec![4.0, 5.0, 6.0]);
spectrum.set_spectrum(energy, mu);

// After (nalgebra)
use nalgebra::DVector;
let energy = DVector::from_vec(vec![1.0, 2.0, 3.0]);
let mu = DVector::from_vec(vec![4.0, 5.0, 6.0]);
spectrum.set_spectrum(energy, mu);
```

**Option 2: Keep using Array1 with feature flag (Compatibility)**
```rust
// Cargo.toml
[dependencies]
xraytsubaki = { version = "0.2", features = ["ndarray-compat"] }

// Code unchanged - Array1 still works
use ndarray::Array1;
let energy = Array1::from_vec(vec![1.0, 2.0, 3.0]);
let mu = Array1::from_vec(vec![4.0, 5.0, 6.0]);
spectrum.set_spectrum(energy, mu); // Automatically converts to DVector internally
```

**Breaking Changes**:
- Accessing internal vector fields directly returns `DVector<f64>` (not `Array1`)
- Serialization format changes (DVector format differs from Array1)
- `ToNdarray1` trait behavior changes with feature flag

### Risk Mitigation

**High-Risk Areas**:
1. **AUTOBK Algorithm** (`background.rs`):
   - Most complex algorithm with spline fitting and Levenberg-Marquardt optimization
   - Already has some nalgebra integration via `levenberg-marquardt` crate
   - Mitigation: Extensive testing against reference data, incremental migration

2. **Performance Regression**:
   - Risk: nalgebra overhead could reduce parallel processing efficiency
   - Mitigation: Comparative benchmarking before/after, profiling, BLAS backend optimization

3. **Numerical Precision**:
   - Risk: Subtle differences in vector operations could affect scientific accuracy
   - Mitigation: Maintain `TEST_TOL = 1e-12` validation, compare against xraylarch reference

4. **Serialization Compatibility**:
   - Risk: Breaking existing saved data formats
   - Mitigation: Provide migration tools, versioned formats, backward-compatible deserialization

### Success Criteria

✅ All 82+ existing tests pass with nalgebra implementation
✅ Performance ≥10x Python baseline maintained (10,000 spectra in ≤7.5s)
✅ Numerical accuracy within `TEST_TOL = 1e-12` for critical calculations
✅ ndarray optional in `cargo tree` (absent by default, present with `--features ndarray-compat`)
✅ Both feature configurations pass tests: default and `--all-features`
✅ Clean `cargo clippy` and `cargo check` for both configurations
✅ Comprehensive migration guide for library users with both migration paths
✅ Benchmark comparison report showing nalgebra vs ndarray performance
✅ Feature-gated conversions work correctly (Array1 → DVector when feature enabled)
