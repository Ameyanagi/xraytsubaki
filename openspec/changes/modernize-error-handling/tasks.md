# Implementation Tasks

## Phase 1: Infrastructure Setup (High Priority)

### Task 1.1: Add thiserror dependency
**Goal**: Add thiserror to workspace dependencies

**Steps**:
1. Add `thiserror = "2.0"` to `[workspace.dependencies]` in root `Cargo.toml`
2. Add `thiserror = { workspace = true }` to `crates/xraytsubaki/Cargo.toml`
3. Run `cargo check` to verify dependency resolution
4. Verify no version conflicts with existing dependencies

**Validation**:
- `cargo tree | grep thiserror` shows correct version
- Project compiles successfully

**Estimated time**: 15 minutes

---

### Task 1.2: Create errors module structure
**Goal**: Set up dedicated error types module

**Steps**:
1. Create `crates/xraytsubaki/src/xafs/errors.rs`
2. Add `pub mod errors;` to `crates/xraytsubaki/src/xafs/mod.rs`
3. Add re-exports: `pub use errors::{XAFSError, DataError, ...};` in `mod.rs`
4. Add basic module documentation

**Validation**:
- Module compiles successfully
- Re-exports are accessible from `xafs::` namespace

**Estimated time**: 10 minutes

---

### Task 1.3: Define core error enums with thiserror
**Goal**: Create all error type definitions using thiserror

**Steps**:
1. Define `DataError` enum with `#[derive(Error, Debug, Clone)]` and variants:
   - `InsufficientData { min: usize, actual: usize }`
   - `LengthMismatch { energy_len: usize, mu_len: usize }`
   - `InvalidEnergyRange { min: f64, max: f64 }`
   - `NonFiniteValues { indices: Vec<usize> }`
2. Define `NormalizationError` enum with variants:
   - `E0OutOfRange { e0: f64, data_min: f64, data_max: f64 }`
   - `PreEdgeFitFailed { start: f64, end: f64 }`
   - `PostEdgeFitFailed { order: usize, n_points: usize }`
   - `EdgeStepTooSmall { edge_step: f64, min: f64 }`
3. Define `BackgroundError` enum with variants:
   - `OptimizationFailed { reason: String }`
   - `InvalidRbkg { rbkg: f64 }`
   - `SplineKnotsFailed { kmin: f64, kmax: f64 }`
   - `ConvergenceFailure { iterations: usize }`
4. Define `FFTError` enum with variants:
   - `InsufficientPoints { min: usize, actual: usize, kmin: f64, kmax: f64 }`
   - `InvalidWindow { window: String }`
   - `IFFTSizeMismatch { expected: usize, actual: usize }`
5. Define `IOError` enum with variants:
   - `FileNotFound { path: String }`
   - `ReadFailed { path: String, #[source] source: std::io::Error }`
   - `JsonError(#[from] serde_json::Error)`
   - `BsonError(#[from] bson::de::Error)`
   - `CompressionError(String)`
6. Define `MathError` enum with variants:
   - `InterpolationOutOfBounds { x: f64, xmin: f64, xmax: f64 }`
   - `PolyfitFailed { reason: String }`
   - `SplineEvalFailed { x: f64, reason: String }`
   - `IndexOutOfBounds { index: usize, len: usize }`
7. Add `#[error("...")]` attributes to all variants with descriptive messages

**Validation**:
- All error types compile
- Each variant has error message attribute
- Errors are Clone-able (required for Rayon)
- Running `cargo doc` shows error documentation

**Estimated time**: 45 minutes

---

### Task 1.4: Update XAFSError to aggregate domain errors
**Goal**: Replace manual Error impl with thiserror-based aggregation

**Steps**:
1. Remove existing manual `impl Error for XAFSError` block from `mod.rs`
2. Remove manual `impl fmt::Display for XAFSError` block
3. Update `XAFSError` enum to:
   ```rust
   #[derive(Error, Debug, Clone)]
   pub enum XAFSError {
       #[error("data error: {0}")]
       Data(#[from] DataError),

       #[error("normalization error: {0}")]
       Normalization(#[from] NormalizationError),

       #[error("background removal error: {0}")]
       Background(#[from] BackgroundError),

       #[error("FFT error: {0}")]
       FFT(#[from] FFTError),

       #[error("I/O error: {0}")]
       IO(#[from] IOError),

       #[error("mathematical operation failed: {0}")]
       Math(#[from] MathError),
   }
   ```
4. Remove deprecated error variants (NotEnoughData, etc.) - these will be replaced by domain errors
5. Add `pub type Result<T> = std::result::Result<T, XAFSError>;` type alias

**Validation**:
- No compilation errors
- `XAFSError` implements `Error` trait automatically
- `From` implementations work for domain errors
- No clippy warnings about deprecated methods

**Dependencies**: Tasks 1.1-1.3

**Estimated time**: 30 minutes

---

## Phase 2: Module-by-Module Migration (High Priority)

### Task 2.1: Migrate normalization.rs errors
**Goal**: Replace `Box<dyn Error>` with `NormalizationError` in normalization module

**Steps**:
1. Update function signatures:
   - `normalize()` → `Result<&mut Self, NormalizationError>`
   - `fill_parameter()` → `Result<&mut Self, NormalizationError>`
2. Replace error creation sites with specific error variants:
   - Check e0 range → `NormalizationError::E0OutOfRange`
   - Pre-edge fit failures → `NormalizationError::PreEdgeFitFailed`
   - Post-edge fit failures → `NormalizationError::PostEdgeFitFailed`
   - Edge step validation → `NormalizationError::EdgeStepTooSmall`
3. Replace `.unwrap()` with `?` where functions return Result
4. Update trait definitions if necessary

**Validation**:
- Module compiles successfully
- All tests pass (run `cargo test normalization`)
- Error messages are descriptive when errors occur
- No `Box<dyn Error>` remains in public signatures

**Dependencies**: Task 1.4

**Estimated time**: 1 hour

---

### Task 2.2: Migrate background.rs errors
**Goal**: Replace `Box<dyn Error>` with `BackgroundError` in AUTOBK implementation

**Steps**:
1. Update function signatures:
   - `calc_background()` → `Result<&mut Self, BackgroundError>`
   - `fill_parameter()` → `Result<(), BackgroundError>`
2. Replace panic/todo sites:
   - `todo!("Implement ILPBkg")` → `BackgroundError::OptimizationFailed { reason: "ILPBkg not yet implemented".to_string() }`
3. Replace error creation sites:
   - LM optimization failures → `BackgroundError::ConvergenceFailure`
   - Rbkg validation → `BackgroundError::InvalidRbkg`
   - Spline knot failures → `BackgroundError::SplineKnotsFailed`
4. Update LevenbergMarquardt error handling to return BackgroundError

**Validation**:
- Module compiles successfully
- All tests pass (run `cargo test background`)
- No `panic!()` or `todo!()` in production code paths
- Error messages include iteration counts, parameter values

**Dependencies**: Task 1.4

**Estimated time**: 1.5 hours

---

### Task 2.3: Migrate xrayfft.rs errors
**Goal**: Replace `Box<dyn Error>` with `FFTError` in FFT/IFFT operations

**Steps**:
1. Update function signatures:
   - `fft()` → `Result<&mut Self, FFTError>`
   - `ifft()` → `Result<&mut Self, FFTError>`
   - `xftf()`, `xftr()` → use `FFTError`
2. Replace error creation sites:
   - Insufficient data checks → `FFTError::InsufficientPoints`
   - Window validation → `FFTError::InvalidWindow`
   - IFFT size mismatches → `FFTError::IFFTSizeMismatch`
3. Update existing NotEnoughDataForXFTF/XFTR to use new errors

**Validation**:
- Module compiles successfully
- All tests pass (run `cargo test xrayfft`)
- Error messages include k-range and point counts

**Dependencies**: Task 1.4

**Estimated time**: 45 minutes

---

### Task 2.4: Migrate mathutils.rs errors
**Goal**: Replace `Box<dyn Error>` with `MathError` in mathematical utilities

**Steps**:
1. Update function signatures for:
   - `interpolate()` → `Result<T, MathError>`
   - `index_of()`, `index_nearest()` → use `MathError`
   - Spline operations → use `MathError`
2. Replace error creation sites:
   - Interpolation bounds checks → `MathError::InterpolationOutOfBounds`
   - Polynomial fit failures → `MathError::PolyfitFailed`
   - Spline eval failures → `MathError::SplineEvalFailed`
   - Index validation → `MathError::IndexOutOfBounds`
3. Update trait implementations (`MathUtils` trait)

**Validation**:
- Module compiles successfully
- All tests pass (run `cargo test mathutils`)
- Error messages include relevant numeric values

**Dependencies**: Task 1.4

**Estimated time**: 1 hour

---

### Task 2.5: Migrate io/ module errors
**Goal**: Replace `Box<dyn Error>` with `IOError` in I/O operations

**Steps**:
1. Update `io/mod.rs` loader functions to return `Result<T, IOError>`
2. Update `io/xafs_json.rs`:
   - `read_json()`, `write_json()` → `Result<&mut Self, IOError>`
   - Wrap serde_json errors automatically via `#[from]`
3. Update `io/xafs_bson.rs`:
   - `read_bson()`, `write_bson()` → `Result<&mut Self, IOError>`
   - Wrap bson errors automatically via `#[from]`
4. Update file operations:
   - File::open() errors → `IOError::FileNotFound` or `IOError::ReadFailed`
   - Use `?` operator for automatic conversions

**Validation**:
- Module compiles successfully
- All tests pass (run `cargo test io`)
- Error chains preserve original serde/bson errors
- File paths are included in error messages

**Dependencies**: Task 1.4

**Estimated time**: 45 minutes

---

## Phase 3: API Integration (Medium Priority)

### Task 3.1: Update xasspectrum.rs to use XAFSError
**Goal**: Update XASSpectrum API to use typed errors

**Steps**:
1. Update method signatures to return `Result<T, XAFSError>`
2. Replace panic sites:
   - `panic!("Need to calculate k and chi first")` → `Err(XAFSError::Data(DataError::MissingData(...)))`
   - `panic!("Please provide r and chi_r")` → proper error return
3. Use `?` operator to propagate module errors (automatically converts via `#[from]`)
4. Update `todo!()` sites with proper errors or feature gates

**Validation**:
- XASSpectrum compiles successfully
- Public API maintains compatibility (error variant names)
- No panics in production code paths
- Tests updated for new error types

**Dependencies**: Tasks 2.1-2.5

**Estimated time**: 1 hour

---

### Task 3.2: Update xasgroup.rs to use XAFSError
**Goal**: Update XASGroup API to use typed errors

**Steps**:
1. Update method signatures:
   - `find_e0()`, `normalize()`, `calc_background()`, `fft()`, `ifft()` → `Result<&mut Self, XAFSError>`
   - `get_spectrum()`, `remove_spectrum()` → use `XAFSError`
2. Propagate errors from XASSpectrum calls using `?`
3. Handle parallel operations (Rayon) error collection:
   - Use `try_for_each()` for fallible parallel operations
   - Collect errors from parallel spectrum processing
4. Update `merge()` from `todo!()` to proper error

**Validation**:
- XASGroup compiles successfully
- Parallel operations properly propagate errors
- All tests pass (run `cargo test xasgroup`)
- Error messages are clear for batch operations

**Dependencies**: Task 3.1

**Estimated time**: 1.5 hours

---

### Task 3.3: Update public API traits
**Goal**: Update trait definitions to use typed errors

**Steps**:
1. Update `Normalization` trait in `normalization.rs`:
   - Change error types from `Box<dyn Error>` to `NormalizationError` or `XAFSError`
2. Update `MathUtils` trait in `mathutils.rs`:
   - Change to use `MathError` where appropriate
3. Update `XASJson` and `XASBson` traits in io module:
   - Change to use `IOError`
4. Ensure trait implementors are updated

**Validation**:
- All trait implementations compile
- No breaking changes to trait interface (maintain variant compatibility)
- Tests pass for all trait implementors

**Dependencies**: Tasks 2.1-2.5

**Estimated time**: 45 minutes

---

## Phase 4: Testing & Validation (High Priority)

### Task 4.1: Update existing tests for new error types
**Goal**: Modify tests to work with typed errors

**Steps**:
1. Update test assertions from `Box<dyn Error>` to specific error types
2. Add error variant matching in tests:
   ```rust
   match result {
       Err(XAFSError::Data(DataError::InsufficientData { min, actual })) => {
           assert_eq!(min, 100);
           assert_eq!(actual, 50);
       }
       _ => panic!("unexpected error type"),
   }
   ```
3. Update `.unwrap()` with `.expect("descriptive context")` where helpful
4. Ensure all tests still pass

**Validation**:
- Run `cargo test --all-features` - all tests pass
- No ignored tests
- Error assertions are specific

**Dependencies**: Tasks 3.1-3.3

**Estimated time**: 2 hours

---

### Task 4.2: Add error-specific unit tests
**Goal**: Add tests that specifically validate error behavior

**Steps**:
1. Create `crates/xraytsubaki/src/xafs/errors/tests.rs` (or embedded in errors.rs)
2. Test error message formatting:
   ```rust
   #[test]
   fn test_insufficient_data_message() {
       let err = DataError::InsufficientData { min: 100, actual: 50 };
       assert!(err.to_string().contains("100"));
       assert!(err.to_string().contains("50"));
   }
   ```
3. Test error source chaining:
   ```rust
   #[test]
   fn test_error_source_chain() {
       let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "test");
       let err = IOError::ReadFailed { path: "test.txt".to_string(), source: io_err };
       assert!(err.source().is_some());
   }
   ```
4. Test automatic error conversions via `From`:
   ```rust
   #[test]
   fn test_auto_conversion() {
       let norm_err = NormalizationError::E0OutOfRange { ... };
       let xafs_err: XAFSError = norm_err.into();
       // Verify conversion worked
   }
   ```

**Validation**:
- All error tests pass
- Test coverage includes all error variants
- Error messages are validated

**Dependencies**: Task 1.4

**Estimated time**: 1 hour

---

### Task 4.3: Integration testing with error paths
**Goal**: Ensure error propagation works end-to-end

**Steps**:
1. Create integration tests that intentionally trigger errors:
   - Load spectrum with insufficient data → verify `DataError`
   - Attempt normalization with bad parameters → verify `NormalizationError`
   - Run AUTOBK with invalid rbkg → verify `BackgroundError`
2. Test error propagation through call stack:
   - XASGroup → XASSpectrum → module functions
   - Verify error types are preserved through layers
3. Test parallel error collection in XASGroup

**Validation**:
- Integration tests pass
- Error types are correctly propagated
- Error messages are informative at top level

**Dependencies**: Tasks 3.1-3.2

**Estimated time**: 1.5 hours

---

### Task 4.4: Performance regression testing
**Goal**: Verify zero performance impact from error changes

**Steps**:
1. Run existing benchmarks before and after changes:
   ```bash
   cargo bench --bench xas_group_benchmark_parallel
   ```
2. Compare results - should show <1% variation
3. Profile error path overhead (should be negligible)
4. Document benchmark results in commit message

**Validation**:
- No performance regression >1%
- Error enum size is reasonable (check with `mem::size_of`)
- No unexpected heap allocations in hot paths

**Dependencies**: Tasks 3.1-3.2

**Estimated time**: 30 minutes

---

## Phase 5: Documentation & Cleanup (Medium Priority)

### Task 5.1: Add error documentation
**Goal**: Document error types and error handling patterns

**Steps**:
1. Add module-level documentation to `errors.rs`:
   - Explain error hierarchy
   - Show common error handling patterns
   - Provide examples of matching on errors
2. Add doc comments to each error type:
   - When this error occurs
   - How to recover (if possible)
   - Related errors
3. Add examples to error variants

**Validation**:
- Run `cargo doc --open` and review error documentation
- All error types have doc comments
- Examples compile (via doctest)

**Dependencies**: Task 1.4

**Estimated time**: 1 hour

---

### Task 5.2: Update CHANGELOG and migration guide
**Goal**: Document changes for library users

**Steps**:
1. Add entry to CHANGELOG.md:
   - Breaking changes (if any)
   - New error types
   - Migration guide for existing code
2. Create migration guide showing:
   - Old pattern → New pattern conversions
   - How to match on new error types
   - What changed in function signatures

**Validation**:
- CHANGELOG entry is clear and complete
- Migration examples are tested
- Breaking changes are clearly marked

**Dependencies**: All Phase 4 tasks

**Estimated time**: 45 minutes

---

### Task 5.3: Cleanup deprecated error references
**Goal**: Remove old error handling code

**Steps**:
1. Search for remaining `Box<dyn Error>` uses:
   ```bash
   rg "Box<dyn Error>" --type rust
   ```
2. Verify all are either:
   - Internal/private functions (acceptable)
   - In tests (acceptable)
   - Or migrated to typed errors
3. Remove any dead code from old error implementations
4. Run clippy and fix any new warnings:
   ```bash
   cargo clippy --all-targets --all-features
   ```

**Validation**:
- No clippy warnings related to error handling
- No deprecated Error trait methods in use
- Code compiles cleanly

**Dependencies**: All Phase 3 tasks

**Estimated time**: 30 minutes

---

## Summary

**Total estimated time**: ~16 hours

**Critical path**:
1. Phase 1 (Infrastructure) → Phase 2 (Module migration) → Phase 3 (API integration) → Phase 4 (Testing)

**Parallel opportunities**:
- Phase 2 tasks (2.1-2.5) can be done in any order or in parallel
- Phase 4.2 (error tests) can be done alongside Phase 3
- Phase 5 (documentation) can start once Phase 3 is complete

**Risk areas**:
- Task 3.2 (XASGroup parallel operations) - complex error collection with Rayon
- Task 4.1 (updating existing tests) - may uncover unexpected dependencies
- API compatibility - careful testing needed to ensure no breaking changes
