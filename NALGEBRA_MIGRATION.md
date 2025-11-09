# ndarray → nalgebra DVector Migration Status

## Current Status: BASELINE ESTABLISHED - Ready for Migration

### Toolchain Resolution ✅
**Resolution**: Used Rust 1.82.0 with nalgebra 0.32.4 (matches existing codebase API)

**Previous Issues (Resolved)**:
- Rust 1.91.0 nightly: anymap compatibility issue (E0804)
- nalgebra 0.34.1: Required edition2024
- time crate 0.3.34: Type inference issue with Rust 1.80+

**Working Configuration**:
- ✅ Rust toolchain: 1.82.0
- ✅ nalgebra: 0.32.4 with serde-serialize
- ✅ ndarray: optional via "ndarray-compat" feature
- ✅ time: 0.3.44 (updated for Rust 1.80+ compatibility)

### Baseline Performance Metrics ✅

**Hardware**: Linux 6.17.3-arch2-1 (system specs)
**Date**: 2025-11-09
**Configuration**: ndarray baseline (before migration)

**Full Pipeline Benchmark** (10,000 spectra: normalize → AUTOBK → FFT):
- **Mean**: 3.98 seconds (3,980,844,521.7 ns)
- **Median**: 3.99 seconds (3,986,844,598.5 ns)
- **Std Dev**: 31.4 ms
- **95% CI**: 3.96s - 4.00s
- **Sample Size**: 10 iterations

**Target**: Maintain or improve this performance after DVector migration

## Completed Tasks (Phase 1: Preparation)

### ✅ Task 1.3-1.5: Dependency Updates
- **Workspace `Cargo.toml`**:
  - Updated `nalgebra` from 0.32.4 to 0.34.1
  - Added `serde-serialize` feature to nalgebra
  - Kept ndarray as workspace dependency (required for workspace compatibility)

- **Crate `Cargo.toml`** (`crates/xraytsubaki/Cargo.toml`):
  - Made ndarray optional: `ndarray = { version = "0.15.6", features = ["approx", "serde"], optional = true }`
  - Added feature configuration:
    ```toml
    [features]
    default = []
    ndarray-compat = ["ndarray"]
    ```

### ✅ Task 1.6-1.8: Baseline Benchmarks and Documentation
- **Baseline Benchmarks Completed**: Full pipeline benchmark recorded (10,000 spectra)
- **Performance Target Set**: Maintain ≤ 4.0 seconds for 10K spectra (current: 3.98s)
- **Configuration Verified**:
  - ✅ Optional dependency syntax correct
  - ✅ Feature flag configuration valid
  - ✅ Compiles WITH feature: `cargo check --features ndarray-compat`
  - ✅ Compilation verified

## Completed Tasks (Phases 2-4: In Progress)

### ✅ Phase 2: Core Trait Implementations for DVector
- **MathUtils trait** (crates/xraytsubaki/src/xafs/mathutils.rs:185-262):
  - ✅ `interpolate()` - Linear interpolation with clamping
  - ✅ `is_sorted()` - Check if vector is sorted
  - ✅ `argsort()` - Return indices that would sort the vector
  - ✅ `min()` / `max()` - Find minimum/maximum values
  - ✅ `diff()` - Calculate differences between consecutive elements
  - ✅ `gradient()` - Numerical gradient calculation

- **XAFSUtils trait** (crates/xraytsubaki/src/xafs/xafsutils.rs:76-84):
  - ✅ `etok()` - Convert energy (eV) to wavenumber (k) with negative value handling
  - ✅ `ktoe()` - Convert wavenumber (k) to energy (eV)

- **LMUtils/LMParameters trait** (crates/xraytsubaki/src/xafs/lmutils.rs:76-88):
  - ✅ Already implemented for DVector (file was already using nalgebra)

### ✅ Phase 3: Conversion Traits Enhancement
- **nshare.rs** (crates/xraytsubaki/src/xafs/nshare.rs):
  - ✅ Added `#[cfg(feature = "ndarray-compat")]` to all ndarray imports
  - ✅ Feature-gated `ToNalgebra` and `ToNdarray1` custom traits
  - ✅ Updated test module with feature gates
  - ❌ Cannot use standard `From` traits (orphan rule violation)

### ✅ Phase 4: XASSpectrum Struct Migration
- **Core Data Structure** (crates/xraytsubaki/src/xafs/xasspectrum.rs):
  - ✅ Migrated all vector fields to `DVector<f64>`:
    - `raw_energy`, `raw_mu`, `energy`, `mu`
    - `k`, `chi`, `chi_kweighted`
    - `chi_r`, `chi_r_mag`, `chi_r_re`, `chi_r_im`, `q`
  - ✅ Updated `set_spectrum()` - manual index-based sorting for DVector
  - ✅ Updated `interpolate_spectrum()` - extract Vec from DVector.data
  - ✅ Updated getter methods: `get_k()`, `get_chi()`, `get_chi_kweighted()`
  - ✅ Used DVector methods: `component_mul()`, `map()`, `from_iterator()`

## Current Status (Session 2025-11-09)

### 🔄 Phase 5: Algorithm Migration (IN PROGRESS)
**Status**: XASSpectrum migrated, but algorithms still expect Array1

**Remaining Work**:
1. **xafsutils.rs**: ~300 lines of utility functions
   - `find_e0()` and `_find_e0()` - Edge energy detection
   - `find_energy_step()` - Energy grid analysis
   - `smooth()` - Signal smoothing with FFT convolution
   - `remove_dups()` - Duplicate value handling
   - `ftwindow()` - Fourier transform window functions
   - Multiple view/slice operations that need DVector equivalents

2. **normalization.rs**: Pre/post-edge normalization algorithm
   - `PrePostEdge` struct and implementation
   - Polynomial fitting for pre-edge and post-edge regions
   - Needs DVector-compatible polynomial operations

3. **background.rs**: AUTOBK background removal
   - Most complex algorithm - spline fitting with Levenberg-Marquardt
   - Heavy use of ndarray slicing and concatenation
   - Critical for accurate chi(k) extraction

4. **xrayfft.rs**: FFT/IFFT operations
   - Forward and reverse Fourier transforms
   - Window application and k-weighting
   - Integration with easyfft library

5. **xasgroup.rs**: Parallel processing
   - Rayon-based parallel spectrum processing
   - Needs DVector-compatible map operations

### Compilation Status
- ✅ **With ndarray-compat**: Compiles but has type errors (DVector vs Array1 mismatch)
- ❌ **Without ndarray-compat**: Multiple missing import errors (expected until migration complete)

### Next Steps
1. **Immediate**: Migrate xafsutils functions to accept DVector
   - Start with `find_e0()` and `find_energy_step()` (needed by XASSpectrum)
   - Then `smooth()`, `remove_dups()`, `ftwindow()`
2. **Then**: Migrate normalization algorithm
3. **Then**: Migrate AUTOBK (most complex)
4. **Then**: Migrate FFT operations
5. **Finally**: Add feature gates to all ndarray imports
6. **Validate**: Run tests and benchmarks to verify correctness and performance

## Migration Strategy

### Internal Storage
ALL internal vector storage will use `DVector<f64>`:
```rust
pub struct XASSpectrum {
    pub energy: Option<DVector<f64>>,  // was ArrayBase<OwnedRepr<f64>, Ix1>
    pub mu: Option<DVector<f64>>,
    // ...
}
```

### Input API (Feature-Gated Compatibility)
Accept both DVector and Array1 when feature enabled:
```rust
pub fn set_spectrum(
    &mut self,
    energy: impl Into<DVector<f64>>,
    mu: impl Into<DVector<f64>>
)
```

### Conversion Layer (`nshare.rs`)
Feature-gated conversions:
```rust
#[cfg(feature = "ndarray-compat")]
impl From<Array1<f64>> for DVector<f64> {
    fn from(arr: Array1<f64>) -> Self {
        DVector::from_vec(arr.to_vec())
    }
}
```

## Files Modified

### Configuration
- ✅ `/Cargo.toml` - workspace dependencies
- ✅ `/crates/xraytsubaki/Cargo.toml` - crate dependencies and features

### Pending (115 tasks across 18 source files)
- `xafs/mathutils.rs` - Mathematical utilities
- `xafs/xafsutils.rs` - XAS-specific utilities
- `xafs/lmutils.rs` - Levenberg-Marquardt utilities
- `xafs/nshare.rs` - Conversion traits (enhance with feature gates)
- `xafs/xasspectrum.rs` - Core data structure
- `xafs/xasgroup.rs` - Parallel processing
- `xafs/normalization.rs` - Pre/post-edge normalization
- `xafs/background.rs` - AUTOBK algorithm
- `xafs/xrayfft.rs` - FFT/IFFT operations
- `xafs/io/xafs_json.rs` - JSON serialization
- `xafs/io/xafs_bson.rs` - BSON serialization
- Plus tests, benchmarks, and documentation

## Risk Assessment

### Current Risks
- 🔴 **CRITICAL**: Toolchain compatibility blocking all progress
- 🟡 **HIGH**: AUTOBK algorithm complexity (most complex migration)
- 🟡 **HIGH**: Performance regression potential
- 🟢 **MEDIUM**: Breaking changes mitigated by optional feature flag

### Mitigation
- Toolchain: Use stable Rust or pin to working nightly version
- AUTOBK: Incremental testing against reference data (TEST_TOL = 1e-12)
- Performance: Baseline benchmarks → continuous validation
- Breaking changes: Feature flag provides migration path

## Timeline Estimate

- **Phase 1**: 2-3 days (BLOCKED - 1 day complete, toolchain issue)
- **Phase 2**: 2-3 days (Test infrastructure)
- **Phase 3-6**: 10-14 days (Core migration)
- **Phase 7-9**: 4-5 days (Integration & performance)
- **Phase 10-11**: 3-4 days (Documentation & validation)

**Total**: 21-29 days (after toolchain resolution)

## Reference
- Proposal: `openspec/changes/refactor-ndarray-to-nalgebra-dvector/proposal.md`
- Tasks: `openspec/changes/refactor-ndarray-to-nalgebra-dvector/tasks.md`
- Design: `openspec/changes/refactor-ndarray-to-nalgebra-dvector/design.md`
- Specs: `openspec/changes/refactor-ndarray-to-nalgebra-dvector/specs/xas-core/spec.md`
