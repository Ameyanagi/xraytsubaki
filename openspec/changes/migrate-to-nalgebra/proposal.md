# Change: Migrate from ndarray to nalgebra for Linear Algebra Operations

## Why

The project currently uses both `ndarray` and `nalgebra` as dependencies, creating redundancy and increased binary size. Consolidating to `nalgebra` (the more feature-complete linear algebra library) will:

- **Reduce Dependencies**: Remove `ndarray` dependency entirely, simplifying the dependency tree
- **Leverage nalgebra Features**: Access to advanced linear algebra operations, decompositions, and optimized BLAS/LAPACK backends
- **Maintain Performance**: `nalgebra` with BLAS backend provides comparable or better performance for scientific computing
- **Future-Proof**: Better support for static typing, GPU acceleration, and cross-platform optimization
- **Consistency**: Several existing dependencies (`levenberg-marquardt`) already use `nalgebra`

## What Changes

- **BREAKING**: Replace all `ndarray::Array1<f64>` with `nalgebra::DVector<f64>` across 10 XAS modules
- **BREAKING**: Update public API signatures for `XASSpectrum`, `XASGroup`, and related types
- Update dependency versions:
  - `nalgebra`: 0.32.4 → 0.34.1 (latest stable)
  - Review and update `levenberg-marquardt`, `polyfit-rs`, `rusty-fitpack` for compatibility
- Implement comprehensive test suite following TDD:
  - Create separate test modules for each migrated component
  - Maintain existing tests as regression suite
  - Add new tests before migration for each module
- Update benchmarks to validate performance:
  - Preserve existing Criterion benchmarks
  - Add comparative benchmarks (ndarray baseline vs nalgebra)
  - Ensure ≥10x speedup over Python is maintained

## Impact

### Affected Specs
- `xas-data-structures` - Core data types for XAS spectroscopy
- `xas-analysis-pipeline` - EXAFS analysis workflow (find_e0, normalization, AUTOBK, FFT)
- `xas-serialization` - JSON/BSON serialization of vector data
- `xas-performance` - Parallel processing and benchmark targets

### Affected Code
- `crates/xraytsubaki/src/xafs/background.rs` - AUTOBK algorithm with spline fitting
- `crates/xraytsubaki/src/xafs/normalization.rs` - Pre/post-edge normalization
- `crates/xraytsubaki/src/xafs/xasspectrum.rs` - Core XASSpectrum data structure
- `crates/xraytsubaki/src/xafs/xafsutils.rs` - Utility functions (find_e0, energy_step)
- `crates/xraytsubaki/src/xafs/mathutils.rs` - Mathematical operations
- `crates/xraytsubaki/src/xafs/xrayfft.rs` - FFT/IFFT operations
- `crates/xraytsubaki/src/xafs/nshare.rs` - ndarray/nalgebra conversion utilities
- `crates/xraytsubaki/src/xafs/io/xafs_json.rs` - JSON serialization
- `crates/xraytsubaki/src/xafs/io/xafs_bson.rs` - BSON serialization
- `crates/xraytsubaki/src/xafs/tests.rs` - Existing test infrastructure
- `crates/xraytsubaki/Cargo.toml` - Dependency specifications
- `Cargo.toml` (workspace) - Shared dependency versions
- `benches/xas_group_benchmark_parallel.rs` - Performance benchmarks

### Migration Strategy
- **Phase 1**: TDD test creation (separate test modules)
- **Phase 2**: Core data structure migration (`XASSpectrum`, utility modules)
- **Phase 3**: Algorithm migration (normalization, background, FFT)
- **Phase 4**: Serialization and I/O updates
- **Phase 5**: Performance validation and benchmark comparison
- **Phase 6**: Dependency cleanup and documentation

### Backwards Compatibility
**BREAKING CHANGE**: Public API signatures will change. Users of the library will need to update code that:
- Creates `XASSpectrum` objects with energy/mu vectors
- Accesses raw vector data from spectrum objects
- Uses serialized data formats (JSON/BSON with vector fields)

Migration path for library users:
```rust
// Before (ndarray)
use ndarray::Array1;
let energy = Array1::from_vec(vec![1.0, 2.0, 3.0]);
spectrum.set_spectrum(energy, mu);

// After (nalgebra)
use nalgebra::DVector;
let energy = DVector::from_vec(vec![1.0, 2.0, 3.0]);
spectrum.set_spectrum(energy, mu);
```
