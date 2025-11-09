# Error Handling Architecture Design

## Overview
This document outlines the architectural approach for modernizing error handling across the xraytsubaki library using `thiserror`.

## Design Principles

### 1. Library-First Error Design
**Principle**: Provide structured, matchable errors that library consumers can handle programmatically.

**Rationale**:
- This is a library, not an application - callers need to distinguish error types
- Scientific workflows may want to recover from specific errors (e.g., insufficient data)
- Python bindings will need to map Rust errors to Python exceptions

**Implementation**: Use `thiserror` enum types with distinct variants for each error category.

### 2. Domain-Specific Error Types
**Principle**: Each major module has its own error type matching its failure modes.

**Error Type Hierarchy**:
```
XAFSError (top-level, re-exports domain errors)
├── DataError          (data validation, parsing)
├── NormalizationError (pre/post-edge fitting)
├── BackgroundError    (AUTOBK algorithm)
├── FFTError          (Fourier transform operations)
├── IOError           (file I/O, serialization)
└── MathError         (mathematical operations)
```

**Rationale**:
- Matches the existing module structure (background.rs, normalization.rs, etc.)
- Allows module-specific error handling without cross-contamination
- Future modules can add their own error types without breaking existing code

### 3. Context-Rich Error Messages
**Principle**: Every error includes actionable information for debugging.

**Pattern**:
```rust
#[error("insufficient data for FFT: expected at least {min} points, got {actual}")]
InsufficientData { min: usize, actual: usize },
```

**Rationale**:
- Scientific computing errors often require parameter inspection
- Helps users diagnose issues without debugger
- Aligns with Rust 2024 error best practices

### 4. Automatic Error Conversion
**Principle**: Use `#[from]` for common error sources to enable `?` operator.

**Pattern**:
```rust
#[error("I/O error: {0}")]
Io(#[from] std::io::Error),

#[error("JSON deserialization failed: {0}")]
Json(#[from] serde_json::Error),
```

**Rationale**:
- Reduces boilerplate in error propagation
- Makes code more readable (fewer `.map_err()` calls)
- Standard pattern in Rust ecosystem

**Limitation**: Cannot have multiple variants from the same source type (thiserror constraint).

### 5. Zero Runtime Overhead
**Principle**: No backtrace, no dynamic allocation beyond error enum itself.

**Rationale**:
- Performance-critical scientific computing (10,000 spectra in 7.5s)
- Error paths should not impact hot paths
- Benchmarks showed 700MB binary bloat and significant slowdown with backtraces

**Trade-off**: Less debugging info, but acceptable for library use case.

## Error Type Design

### Core Error (`XAFSError`)
Located in `xafs/mod.rs`, serves as the main error type for the crate.

```rust
use thiserror::Error;

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

### Data Validation Errors (`DataError`)
```rust
#[derive(Error, Debug, Clone)]
pub enum DataError {
    #[error("insufficient data: need at least {min} points, got {actual}")]
    InsufficientData { min: usize, actual: usize },

    #[error("data array length mismatch: energy has {energy_len} points, mu has {mu_len} points")]
    LengthMismatch { energy_len: usize, mu_len: usize },

    #[error("invalid energy range: min={min}, max={max}")]
    InvalidEnergyRange { min: f64, max: f64 },

    #[error("data contains non-finite values at indices: {indices:?}")]
    NonFiniteValues { indices: Vec<usize> },
}
```

### Normalization Errors (`NormalizationError`)
```rust
#[derive(Error, Debug, Clone)]
pub enum NormalizationError {
    #[error("edge energy (e0={e0}) is outside data range [{data_min}, {data_max}]")]
    E0OutOfRange { e0: f64, data_min: f64, data_max: f64 },

    #[error("pre-edge fitting failed: not enough points in range [{start}, {end}]")]
    PreEdgeFitFailed { start: f64, end: f64 },

    #[error("post-edge fitting failed: polynomial order {order} too high for {n_points} points")]
    PostEdgeFitFailed { order: usize, n_points: usize },

    #[error("edge step is too small: {edge_step} (minimum: {min})")]
    EdgeStepTooSmall { edge_step: f64, min: f64 },
}
```

### Background Removal Errors (`BackgroundError`)
```rust
#[derive(Error, Debug, Clone)]
pub enum BackgroundError {
    #[error("AUTOBK optimization failed: {reason}")]
    OptimizationFailed { reason: String },

    #[error("invalid rbkg parameter: {rbkg} (must be > 0)")]
    InvalidRbkg { rbkg: f64 },

    #[error("spline knot calculation failed: insufficient k-range [{kmin}, {kmax}]")]
    SplineKnotsFailed { kmin: f64, kmax: f64 },

    #[error("Levenberg-Marquardt did not converge after {iterations} iterations")]
    ConvergenceFailure { iterations: usize },
}
```

### FFT Errors (`FFTError`)
```rust
#[derive(Error, Debug, Clone)]
pub enum FFTError {
    #[error("FFT requires at least {min} points for k-range [{kmin}, {kmax}], got {actual}")]
    InsufficientPoints { min: usize, actual: usize, kmin: f64, kmax: f64 },

    #[error("invalid FFT window: {window}")]
    InvalidWindow { window: String },

    #[error("IFFT failed: chi(R) array has {actual} points, expected {expected}")]
    IFFTSizeMismatch { expected: usize, actual: usize },
}
```

### I/O Errors (`IOError`)
```rust
#[derive(Error, Debug, Clone)]
pub enum IOError {
    #[error("file not found: {path}")]
    FileNotFound { path: String },

    #[error("failed to read file {path}: {source}")]
    ReadFailed { path: String, #[source] source: std::io::Error },

    #[error("JSON deserialization failed: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("BSON deserialization failed: {0}")]
    BsonError(#[from] bson::de::Error),

    #[error("compression error: {0}")]
    CompressionError(String),
}
```

### Math Errors (`MathError`)
```rust
#[derive(Error, Debug, Clone)]
pub enum MathError {
    #[error("interpolation failed: x value {x} is outside range [{xmin}, {xmax}]")]
    InterpolationOutOfBounds { x: f64, xmin: f64, xmax: f64 },

    #[error("polynomial fit failed: {reason}")]
    PolyfitFailed { reason: String },

    #[error("spline evaluation failed at x={x}: {reason}")]
    SplineEvalFailed { x: f64, reason: String },

    #[error("index {index} out of bounds for array of length {len}")]
    IndexOutOfBounds { index: usize, len: usize },
}
```

## Migration Strategy

### Phase 1: Core Infrastructure (Priority: High)
1. Add `thiserror = "2.0"` to `Cargo.toml`
2. Create new error types in `xafs/errors.rs` module
3. Update `XAFSError` in `xafs/mod.rs` to use thiserror
4. Verify compilation

### Phase 2: Module-by-Module Migration (Priority: High)
1. **normalization.rs** - Replace `Box<dyn Error>` with `NormalizationError`
2. **background.rs** - Replace with `BackgroundError`, remove `todo!()` panics
3. **xrayfft.rs** - Replace with `FFTError`
4. **mathutils.rs** - Replace with `MathError`
5. **io/** - Replace with `IOError`

### Phase 3: Error Propagation (Priority: Medium)
1. Replace `Box<dyn Error>` return types with `Result<T, XAFSError>`
2. Add `?` operators where beneficial
3. Remove unnecessary `.unwrap()` in favor of `?`
4. Replace `panic!()` with proper error returns

### Phase 4: Testing & Validation (Priority: High)
1. Update all tests to expect new error types
2. Add error-specific unit tests
3. Verify error messages in integration tests
4. Performance benchmarks (should show zero regression)

## Compatibility Considerations

### Public API
Maintain existing error variant names to avoid breaking changes:
```rust
// Before
pub enum XAFSError {
    NotEnoughData,
    NotEnoughDataForXFTF,
    // ...
}

// After (compatible)
pub enum XAFSError {
    Data(DataError),  // Can match with DataError::InsufficientData
    FFT(FFTError),    // Can match with FFTError::InsufficientPoints
    // ...
}
```

### Error Matching
Preserve ability to match on error types:
```rust
// Still works
match error {
    XAFSError::Data(DataError::InsufficientData { .. }) => { /* handle */ },
    XAFSError::FFT(_) => { /* handle all FFT errors */ },
    _ => { /* default */ }
}
```

## Performance Impact
- **Compile time**: Minimal increase (thiserror is proc-macro)
- **Binary size**: <10KB increase (no backtrace, minimal code generation)
- **Runtime**: Zero overhead (thiserror generates same code as manual impl)
- **Error path**: No heap allocations beyond enum discriminant

## Future Extensions
1. **Result type aliases**: `pub type Result<T> = std::result::Result<T, XAFSError>`
2. **Error recovery strategies**: Fallback for non-critical errors
3. **Python error mapping**: Map `XAFSError` variants to Python exception classes
4. **Error reporting helpers**: Pretty-print error chains for CLI tools
5. **Error metrics**: Track error frequencies in production (if needed)

## References
- [thiserror documentation](https://docs.rs/thiserror/)
- [GreptimeDB error handling](https://greptime.com/blogs/2024-05-07-error-rust)
- [Rust Error Handling Guide 2025](https://markaicode.com/rust-error-handling-2025-guide/)
- [Error Handling for Large Rust Projects](https://bugenzhao.com/2024/04/24/error-handling-1/)
