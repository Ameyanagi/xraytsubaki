# Implementation Tasks

## 1. Pre-Migration Setup and TDD Preparation
- [ ] 1.1 Update workspace `Cargo.toml` with nalgebra 0.34.1
- [ ] 1.2 Update crate `Cargo.toml` with nalgebra 0.34.1
- [ ] 1.3 Audit dependent crates for nalgebra compatibility (levenberg-marquardt, polyfit-rs, rusty-fitpack)
- [ ] 1.4 Create baseline benchmark results with current ndarray implementation
- [ ] 1.5 Create test file structure: `tests/nalgebra_migration/mod.rs`
- [ ] 1.6 Set up test utilities module: `tests/nalgebra_migration/test_utils.rs`

## 2. TDD: Write Tests BEFORE Migration (Following Red-Green-Refactor)
- [ ] 2.1 Create `tests/nalgebra_migration/test_xasspectrum.rs` with DVector-based tests
- [ ] 2.2 Create `tests/nalgebra_migration/test_mathutils.rs` for mathematical operations
- [ ] 2.3 Create `tests/nalgebra_migration/test_xafsutils.rs` for find_e0 and energy_step
- [ ] 2.4 Create `tests/nalgebra_migration/test_normalization.rs` for pre/post-edge normalization
- [ ] 2.5 Create `tests/nalgebra_migration/test_background.rs` for AUTOBK algorithm
- [ ] 2.6 Create `tests/nalgebra_migration/test_xrayfft.rs` for FFT/IFFT operations
- [ ] 2.7 Create `tests/nalgebra_migration/test_serialization.rs` for JSON/BSON I/O
- [ ] 2.8 Verify all new tests compile but FAIL (Red phase - expected)

## 3. Core Data Structure Migration
- [ ] 3.1 Migrate `xafs/mathutils.rs`: Replace Array1 with DVector in utility functions
- [ ] 3.2 Update `xafs/nshare.rs`: Add DVector conversion traits (ToNalgebra, FromNalgebra)
- [ ] 3.3 Migrate `xafs/xasspectrum.rs`: Update XASSpectrum struct fields to DVector
- [ ] 3.4 Update `xafs/xasspectrum.rs`: Migrate set_spectrum, interpolate_spectrum methods
- [ ] 3.5 Update `xafs/xasspectrum.rs`: Migrate getter methods (get_k, get_chi, etc.)
- [ ] 3.6 Run tests: `cargo test nalgebra_migration::test_xasspectrum` (Green phase)
- [ ] 3.7 Run tests: `cargo test nalgebra_migration::test_mathutils` (Green phase)

## 4. Utility Function Migration
- [ ] 4.1 Migrate `xafs/xafsutils.rs`: Update find_e0 function signature and implementation
- [ ] 4.2 Migrate `xafs/xafsutils.rs`: Update find_energy_step function
- [ ] 4.3 Migrate `xafs/xafsutils.rs`: Update argsort and interpolation helpers
- [ ] 4.4 Run tests: `cargo test nalgebra_migration::test_xafsutils` (Green phase)

## 5. Normalization Algorithm Migration
- [ ] 5.1 Migrate `xafs/normalization.rs`: Update NormalizationMethod trait
- [ ] 5.2 Migrate `xafs/normalization.rs`: Update PrePostEdge struct fields
- [ ] 5.3 Migrate `xafs/normalization.rs`: Update normalize method implementation
- [ ] 5.4 Migrate `xafs/normalization.rs`: Update pre_edge and post_edge calculations
- [ ] 5.5 Run tests: `cargo test nalgebra_migration::test_normalization` (Green phase)
- [ ] 5.6 Validate against reference data in `tests/testfiles/Ru_QAS_pre_post_edge_expected.dat`

## 6. Background Removal (AUTOBK) Migration
- [ ] 6.1 Migrate `xafs/background.rs`: Update BackgroundMethod trait
- [ ] 6.2 Migrate `xafs/background.rs`: Update AUTOBK struct fields
- [ ] 6.3 Migrate `xafs/background.rs`: Update calc_background method with DVector
- [ ] 6.4 Migrate `xafs/background.rs`: Update spline fitting with nalgebra vectors
- [ ] 6.5 Migrate `xafs/background.rs`: Verify Levenberg-Marquardt integration with nalgebra
- [ ] 6.6 Migrate `xafs/lmutils.rs`: Update LM utility functions for nalgebra
- [ ] 6.7 Run tests: `cargo test nalgebra_migration::test_background` (Green phase)

## 7. FFT Operations Migration
- [ ] 7.1 Migrate `xafs/xrayfft.rs`: Update XrayFFTF struct fields
- [ ] 7.2 Migrate `xafs/xrayfft.rs`: Update XrayFFTR struct fields
- [ ] 7.3 Migrate `xafs/xrayfft.rs`: Update xftf method for DVector input
- [ ] 7.4 Migrate `xafs/xrayfft.rs`: Update xftr method for DVector input
- [ ] 7.5 Migrate `xafs/xrayfft.rs`: Update getter methods (get_chir, get_r, etc.)
- [ ] 7.6 Run tests: `cargo test nalgebra_migration::test_xrayfft` (Green phase)

## 8. Serialization Migration
- [ ] 8.1 Add nalgebra serde feature if not enabled
- [ ] 8.2 Migrate `xafs/io/xafs_json.rs`: Update JSON serialization for DVector
- [ ] 8.3 Migrate `xafs/io/xafs_bson.rs`: Update BSON serialization for DVector
- [ ] 8.4 Create JSON/BSON format migration guide for data compatibility
- [ ] 8.5 Run tests: `cargo test nalgebra_migration::test_serialization` (Green phase)

## 9. Group Operations Migration
- [ ] 9.1 Migrate `xafs/xasgroup.rs`: Update parallel processing methods (find_e0_par, normalize_par)
- [ ] 9.2 Verify Rayon compatibility with nalgebra DVector operations
- [ ] 9.3 Run integration tests: `cargo test xasgroup`

## 10. Existing Test Suite Validation
- [ ] 10.1 Run full existing test suite: `cargo test`
- [ ] 10.2 Fix any failing tests due to API changes
- [ ] 10.3 Update test data loading in `tests.rs` for DVector
- [ ] 10.4 Verify all normalization tests pass with reference data
- [ ] 10.5 Ensure all tests pass before proceeding to benchmarks

## 11. Performance Benchmarking
- [ ] 11.1 Create new benchmark: `benches/nalgebra_benchmark.rs`
- [ ] 11.2 Add comparative benchmarks (ndarray baseline vs nalgebra)
- [ ] 11.3 Run parallel processing benchmark: `cargo bench xas_group_benchmark_parallel`
- [ ] 11.4 Verify ≥10x speedup over Python baseline is maintained
- [ ] 11.5 Generate benchmark reports and flamegraphs with pprof
- [ ] 11.6 Document performance comparison in `docs/performance_comparison.md`

## 12. Dependency Cleanup
- [ ] 12.1 Remove ndarray from workspace `Cargo.toml` dependencies
- [ ] 12.2 Remove ndarray from crate `Cargo.toml` dependencies
- [ ] 12.3 Clean up unused ndarray imports across all modules
- [ ] 12.4 Remove or update `nshare.rs` ToNalgebra trait (may no longer be needed)
- [ ] 12.5 Run `cargo check` to verify no ndarray dependencies remain
- [ ] 12.6 Run `cargo tree` to verify dependency tree is clean

## 13. Documentation and Final Validation
- [ ] 13.1 Update README.md with nalgebra migration notes
- [ ] 13.2 Create MIGRATION.md guide for library users
- [ ] 13.3 Update module-level documentation for changed APIs
- [ ] 13.4 Update examples (if any) to use DVector
- [ ] 13.5 Run final validation: `cargo test --all-features`
- [ ] 13.6 Run final benchmarks: `cargo bench --all`
- [ ] 13.7 Generate and review API documentation: `cargo doc --no-deps --open`

## Dependencies and Parallelization

### Sequential Dependencies (must complete in order)
1. Tasks 1 → 2 (Setup before TDD)
2. Tasks 2 → 3-8 (Tests before implementation)
3. Tasks 3 → 4-8 (Core structures before algorithms)
4. Tasks 10 → 11 (Tests pass before benchmarking)
5. Tasks 11 → 12 (Performance validation before cleanup)

### Parallelizable Tasks
- Tasks 4, 5, 6, 7 can run in parallel after Task 3 completes
- Task 2.1-2.7 (test creation) can run in parallel
- Task 12.1-12.4 (cleanup) can run in parallel

### Critical Path
1 → 2 → 3 → 6 (AUTOBK is most complex) → 10 → 11 → 12 → 13

Total estimated tasks: 82 individual items across 13 phases
