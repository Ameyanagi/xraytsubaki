# Implementation Tasks

## 1. Preparation & Environment Setup
- [ ] 1.1 Create baseline performance benchmarks with current ndarray implementation
- [ ] 1.2 Document current benchmark results (10,000 spectra timing, memory usage)
- [ ] 1.3 Update workspace `Cargo.toml`: nalgebra = "0.34.1"
- [ ] 1.4 Update crate `Cargo.toml`: nalgebra 0.34.1, make ndarray optional
- [ ] 1.5 Add feature configuration: `[features]` default = [], ndarray-compat = ["ndarray"]
- [ ] 1.6 Audit dependency compatibility: levenberg-marquardt, polyfit-rs, rusty-fitpack
- [ ] 1.7 Run `cargo check` to verify dependency resolution
- [ ] 1.8 Run `cargo check --features ndarray-compat` to verify feature configuration
- [ ] 1.9 Create migration tracking document: `NALGEBRA_MIGRATION.md`

## 2. Test Infrastructure (TDD Setup)
- [ ] 2.1 Create test module structure: `tests/migration/mod.rs`
- [ ] 2.2 Create `tests/migration/test_utils.rs` with DVector test helpers
- [ ] 2.3 Write tests for `mathutils.rs` DVector implementations (gaussian, lorentzian, voigt)
- [ ] 2.4 Write tests for `xafsutils.rs` DVector implementations (find_e0, energy_step, smooth)
- [ ] 2.5 Write tests for `XASSpectrum` with DVector fields
- [ ] 2.6 Write tests for normalization with DVector
- [ ] 2.7 Write tests for AUTOBK with DVector
- [ ] 2.8 Write tests for FFT operations with DVector
- [ ] 2.9 Verify all new tests compile but FAIL (Red phase expected)

## 3. Core Utility Migration
- [ ] 3.1 Migrate `xafs/mathutils.rs`: Implement `MathUtils` trait for `DVector<f64>`
- [ ] 3.2 Migrate gaussian function to use `DVector<f64>`
- [ ] 3.3 Migrate lorentzian function to use `DVector<f64>`
- [ ] 3.4 Migrate voigt function to use `DVector<f64>`
- [ ] 3.5 Run tests: `cargo test migration::test_mathutils`
- [ ] 3.6 Migrate `xafs/lmutils.rs`: Update LM parameter traits for DVector
- [ ] 3.7 Verify Levenberg-Marquardt integration with DVector
- [ ] 3.8 Run tests: `cargo test lmutils`

## 4. XAFSUtils Migration
- [ ] 4.1 Migrate `find_e0` function signature to accept `DVector<f64>`
- [ ] 4.2 Update `find_e0` implementation for DVector operations
- [ ] 4.3 Migrate `find_energy_step` to DVector
- [ ] 4.4 Migrate `smooth` function to DVector input/output
- [ ] 4.5 Update interpolation helpers for DVector
- [ ] 4.6 Migrate argsort utility to work with DVector
- [ ] 4.7 Implement `XAFSUtils` trait for `DVector<f64>`
- [ ] 4.8 Run tests: `cargo test migration::test_xafsutils`
- [ ] 4.9 Run tests: `cargo test xafsutils`

## 5. Conversion Utilities Enhancement
- [ ] 5.1 Review `xafs/nshare.rs` conversion traits
- [ ] 5.2 Implement `From<Array1<f64>> for DVector<f64>` with `#[cfg(feature = "ndarray-compat")]`
- [ ] 5.3 Implement `From<DVector<f64>> for Array1<f64>` with `#[cfg(feature = "ndarray-compat")]`
- [ ] 5.4 Update `ToNalgebra` trait for feature-gated usage
- [ ] 5.5 Update `ToNdarray1` trait for feature-gated usage
- [ ] 5.6 Add tests for conversions with feature enabled
- [ ] 5.7 Verify conversions disabled without feature (compilation check)
- [ ] 5.8 Document conversion utilities in migration guide

## 6. XASSpectrum Core Structure Migration
- [ ] 6.1 Update `XASSpectrum` struct fields from `ArrayBase<OwnedRepr<f64>, Ix1>` to `DVector<f64>`
- [ ] 6.2 Fields to migrate: raw_energy, raw_mu, energy, mu, k, chi, chi_kweighted, chi_r, chi_r_mag, chi_r_re, chi_r_im, q
- [ ] 6.3 Update `set_spectrum` method to accept `impl Into<DVector<f64>>` (supports both DVector and Array1 with feature)
- [ ] 6.4 Update `interpolate_spectrum` for DVector
- [ ] 6.5 Update `find_e0` method to work with DVector fields
- [ ] 6.6 Update all getter methods to return `DVector<f64>` references
- [ ] 6.7 Update `get_chi_kweighted` calculation for DVector
- [ ] 6.8 Run tests: `cargo test migration::test_xasspectrum`
- [ ] 6.9 Run tests with feature: `cargo test --features ndarray-compat migration::test_xasspectrum`
- [ ] 6.10 Run tests: `cargo test xasspectrum`

## 7. Normalization Algorithm Migration
- [ ] 7.1 Update `Normalization` trait methods to accept `&DVector<f64>`
- [ ] 7.2 Migrate `PrePostEdge` struct fields to DVector (pre_edge, post_edge, norm, flat)
- [ ] 7.3 Update `normalize` method implementation for DVector
- [ ] 7.4 Migrate pre-edge polynomial fitting to DVector
- [ ] 7.5 Migrate post-edge normalization to DVector
- [ ] 7.6 Update `get_norm` and `get_flat` return types to `&DVector<f64>`
- [ ] 7.7 Run tests: `cargo test migration::test_normalization`
- [ ] 7.8 Validate against reference data: `tests/testfiles/Ru_QAS_pre_post_edge_expected.dat`
- [ ] 7.9 Run tests: `cargo test normalization`

## 8. Background Removal (AUTOBK) Migration - CRITICAL
- [ ] 8.1 Update `BackgroundMethod` trait methods to accept `&DVector<f64>`
- [ ] 8.2 Migrate `AUTOBK` struct fields to DVector (chi_std, k_std, bkg, chie, k, chi)
- [ ] 8.3 Migrate `AUTOBKSpline` struct: coefs, knots, kraw, mu, kout, ftwin → DVector
- [ ] 8.4 Update `calc_background` method for DVector inputs
- [ ] 8.5 Migrate spline coefficient calculation to DVector
- [ ] 8.6 Update `residual_vec` to use DVector operations
- [ ] 8.7 Update `residual_jacobian` for DVector (critical for LM optimization)
- [ ] 8.8 Verify Levenberg-Marquardt minimizer integration with DVector
- [ ] 8.9 Test spline basis function generation with DVector
- [ ] 8.10 Run tests: `cargo test migration::test_background`
- [ ] 8.11 Validate AUTOBK convergence and accuracy
- [ ] 8.12 Run tests: `cargo test background`

## 9. FFT Operations Migration
- [ ] 9.1 Update `XrayFFTF` struct fields to DVector (r, chir_mag, kwin)
- [ ] 9.2 Update `XrayFFTR` struct fields to DVector (q, chiq, rwin)
- [ ] 9.3 Migrate `xftf` method to accept `&DVector<f64>` for k and chi
- [ ] 9.4 Migrate `xftr` method to accept DVector inputs
- [ ] 9.5 Update windowing functions for DVector
- [ ] 9.6 Migrate `xftf_fast_nalgebra` helper function
- [ ] 9.7 Migrate `xftr_fast_nalgebra` helper function
- [ ] 9.8 Update getter methods: get_r, get_chir_mag, get_chir_real, get_chir_imag → DVector refs
- [ ] 9.9 Run tests: `cargo test migration::test_xrayfft`
- [ ] 9.10 Run tests: `cargo test xrayfft`

## 10. XASGroup Parallel Processing Migration
- [ ] 10.1 Update `XASGroup` struct to use DVector in spectrum storage
- [ ] 10.2 Verify Rayon compatibility with DVector operations
- [ ] 10.3 Update `find_e0_par` parallel method
- [ ] 10.4 Update `normalize_par` parallel method
- [ ] 10.5 Update `calc_background_par` parallel method
- [ ] 10.6 Run integration tests: `cargo test xasgroup`
- [ ] 10.7 Verify parallel processing performance

## 11. Serialization Migration
- [ ] 11.1 Verify nalgebra serde feature enabled
- [ ] 11.2 Update `xafs/io/xafs_json.rs` for DVector serialization
- [ ] 11.3 Update `xafs/io/xafs_bson.rs` for DVector serialization
- [ ] 11.4 Test JSON round-trip serialization
- [ ] 11.5 Test BSON round-trip serialization
- [ ] 11.6 Create data format migration utilities
- [ ] 11.7 Document serialization format changes
- [ ] 11.8 Run tests: `cargo test migration::test_serialization`

## 12. Existing Test Suite Validation
- [ ] 12.1 Run full test suite: `cargo test`
- [ ] 12.2 Fix any failing tests due to API changes
- [ ] 12.3 Update test data loading for DVector
- [ ] 12.4 Verify all normalization tests pass with reference data
- [ ] 12.5 Verify all AUTOBK tests pass with expected accuracy
- [ ] 12.6 Verify all FFT tests pass
- [ ] 12.7 Check numerical precision with `TEST_TOL = 1e-12`
- [ ] 12.8 Ensure zero test failures before benchmarking

## 13. Performance Benchmarking
- [ ] 13.1 Create comparative benchmark: `benches/nalgebra_comparison.rs`
- [ ] 13.2 Benchmark: Single spectrum processing (ndarray baseline vs nalgebra)
- [ ] 13.3 Benchmark: Parallel 10,000 spectra processing
- [ ] 13.4 Run benchmark: `cargo bench xas_group_benchmark_parallel`
- [ ] 13.5 Verify ≥10x Python speedup maintained (≤7.5s for 10,000 spectra on M1)
- [ ] 13.6 Generate flamegraphs with pprof for performance analysis
- [ ] 13.7 Document benchmark results in `docs/PERFORMANCE_COMPARISON.md`
- [ ] 13.8 Identify any performance regressions and optimize

## 14. Dependency Configuration & Cleanup
- [ ] 14.1 Ensure ndarray is marked as optional in workspace `Cargo.toml`
- [ ] 14.2 Ensure ndarray is marked as optional in crate `Cargo.toml`
- [ ] 14.3 Wrap all `use ndarray::*` imports with `#[cfg(feature = "ndarray-compat")]`
- [ ] 14.4 Ensure `nshare.rs` conversions are feature-gated properly
- [ ] 14.5 Run `cargo check` to verify no ndarray references in default build
- [ ] 14.6 Run `cargo check --features ndarray-compat` to verify feature build
- [ ] 14.7 Run `cargo tree` to confirm ndarray NOT in default dependency tree
- [ ] 14.8 Run `cargo tree --features ndarray-compat` to confirm ndarray IS present with feature
- [ ] 14.9 Run `cargo clippy` to check for warnings (default)
- [ ] 14.10 Run `cargo clippy --features ndarray-compat` to check with feature
- [ ] 14.11 Fix any clippy warnings in both configurations

## 15. Documentation & Migration Guide
- [ ] 15.1 Update README.md with nalgebra migration announcement
- [ ] 15.2 Create `MIGRATION_GUIDE.md` for library users
- [ ] 15.3 Document both migration paths: (1) full nalgebra migration, (2) ndarray-compat feature
- [ ] 15.4 Document breaking API changes with examples for both paths
- [ ] 15.5 Document feature flag usage in Cargo.toml
- [ ] 15.6 Update module-level documentation (`//!` doc comments)
- [ ] 15.7 Update function-level documentation (`///` doc comments)
- [ ] 15.8 Update examples in `examples/` directory (if any) with both approaches
- [ ] 15.9 Generate API documentation: `cargo doc --no-deps --open`
- [ ] 15.10 Generate API docs with feature: `cargo doc --no-deps --features ndarray-compat`
- [ ] 15.11 Review generated docs for correctness in both configurations

## 16. Final Validation & Release Prep
- [ ] 16.1 Run complete validation (default): `cargo test`
- [ ] 16.2 Run complete validation (with feature): `cargo test --features ndarray-compat`
- [ ] 16.3 Run complete validation (all features): `cargo test --all-features`
- [ ] 16.4 Run benchmarks (default): `cargo bench`
- [ ] 16.5 Run benchmarks (with feature): `cargo bench --features ndarray-compat`
- [ ] 16.6 Run clippy (default): `cargo clippy`
- [ ] 16.7 Run clippy (with feature): `cargo clippy --features ndarray-compat`
- [ ] 16.8 Verify documentation builds (default): `cargo doc --no-deps`
- [ ] 16.9 Verify documentation builds (with feature): `cargo doc --no-deps --features ndarray-compat`
- [ ] 16.10 Test Python bindings compatibility (py-xraytsubaki)
- [ ] 16.11 Update CHANGELOG.md with breaking changes and feature documentation
- [ ] 16.12 Tag migration milestone
- [ ] 16.13 Create migration summary report with both migration paths

## Dependencies & Execution Strategy

### Sequential Dependencies (MUST complete in order)
1. Phase 1 (Tasks 1-2) → Phase 2 (Tasks 3-5) - Setup before migration
2. Tasks 3-5 → Tasks 6-11 - Utilities before data structures
3. Task 6 → Tasks 7-10 - Core structure before algorithms
4. Tasks 7-10 → Task 12 - Algorithms before test validation
5. Task 12 → Task 13 - Tests pass before performance validation
6. Task 13 → Task 14 - Performance validated before cleanup
7. Task 14 → Tasks 15-16 - Cleanup before documentation

### Parallelizable Tasks
- Tasks 3.1-3.8 (mathutils) can run parallel to 4.1-4.9 (xafsutils) after prep complete
- Tasks 7, 8, 9 (normalization, background, FFT) can run in parallel after Task 6
- Tasks 11.1-11.5 (serialization tests) can run parallel to Task 10
- Tasks 14.1-14.4 (cleanup) can run in parallel
- Tasks 15.1-15.7 (documentation) can run in parallel

### Critical Path
1 → 2 → 5 (conversions) → 6 (XASSpectrum core) → 8 (AUTOBK - most complex) → 12 (validation) → 13 (performance) → 14 (feature config) → 16 (release)

**Total Tasks**: 115 across 16 phases
**Estimated Complexity**: High (AUTOBK migration + feature-gating is most challenging)
**Risk Level**: Medium (reduced from Medium-High due to optional compatibility layer)
