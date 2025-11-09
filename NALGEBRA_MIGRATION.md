# ndarray → nalgebra DVector Migration Status

## Current Status: BLOCKED - Toolchain Issue

### Issue
The project currently cannot compile due to a Rust nightly (1.91.0) compatibility issue with the `anymap` crate (indirect dependency). This issue exists in the current codebase and is **not** caused by the migration changes.

**Error:**
```
error[E0804]: cannot add auto trait `Send` to dyn bound via pointer cast
   --> anymap-1.0.0-beta.2/src/any.rs:37:40
```

This is a known Rust edition 2024 compatibility problem with the anymap crate.

### Resolution Options
1. **Wait for anymap update**: The anymap crate needs to be updated for Rust 1.91+ compatibility
2. **Use stable Rust**: Switch to stable Rust toolchain (recommended for immediate progress)
3. **Pin Rust version**: Use Rust 1.80 or earlier nightly

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

### Configuration Verified
- ✅ Optional dependency syntax correct
- ✅ Feature flag configuration valid
- ⏸️ Compilation blocked by unrelated toolchain issue

## Next Steps (After Toolchain Resolution)

### Immediate (Phase 1 Remaining)
1. Verify `cargo check` works (default build without ndarray)
2. Verify `cargo check --features ndarray-compat` works
3. Run baseline benchmarks: `cargo bench xas_group_benchmark_parallel`
4. Document baseline performance metrics

### Phase 2: Test Infrastructure (TDD Setup)
- Create `tests/migration/` directory structure
- Write failing tests for DVector implementations
- Test helpers for DVector operations

### Phase 3-11: Implementation
Follow tasks.md sequentially:
- Core utilities (mathutils, xafsutils, lmutils)
- Conversion traits in nshare.rs
- XASSpectrum core structure
- Algorithms (normalization, AUTOBK, FFT)
- Parallel processing (XASGroup)
- Serialization
- Documentation

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
