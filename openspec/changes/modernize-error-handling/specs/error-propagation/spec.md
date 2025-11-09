# Error Propagation Specification

## ADDED Requirements

### Requirement: Typed Error Returns
Functions SHALL return typed errors instead of `Box<dyn Error>` where the error type provides value to callers.

#### Scenario: module functions return domain errors
**Given** a function in `normalization.rs`
**When** the function signature is examined
**Then** it SHALL return `Result<T, NormalizationError>` instead of `Result<T, Box<dyn Error>>`
**And** callers SHALL be able to match on specific error variants
**And** this SHALL apply to all public module functions

#### Scenario: XAFSError aggregates module errors
**Given** public API functions in `xasspectrum.rs` or `xasgroup.rs`
**When** these functions call module functions
**Then** they SHALL return `Result<T, XAFSError>`
**And** module errors SHALL automatically convert via `#[from]` attribute
**And** callers SHALL see a consistent top-level error type

### Requirement: Question Mark Operator Usage
Error propagation SHALL use the `?` operator instead of manual `.map_err()` where automatic conversion is available.

#### Scenario: automatic error conversion with ?
**Given** a function returning `Result<T, XAFSError>`
**When** calling a function that returns `Result<U, NormalizationError>`
**Then** the `?` operator SHALL automatically convert to `XAFSError`
**And** no explicit `.map_err()` call SHALL be required
**And** the code SHALL be more concise and readable

#### Scenario: ? operator replaces unwrap where appropriate
**Given** a function call that can fail
**When** the calling context has a compatible error return type
**Then** the `?` operator SHALL be used instead of `.unwrap()`
**And** the error SHALL propagate to the caller
**And** this SHALL reduce panic risk in library code

### Requirement: Panic Elimination in Library Code
Public library functions SHALL NOT use `panic!()` or `todo!()` for error conditions that can be represented as errors.

#### Scenario: replace panic with error return
**Given** code that currently uses `panic!("Need to calculate k and chi first")`
**When** modernizing error handling
**Then** it SHALL return an appropriate error (e.g., `DataError::MissingData`)
**And** callers SHALL be able to handle the error gracefully
**And** the library SHALL not crash the calling application

#### Scenario: replace todo with error or feature flag
**Given** code that currently uses `todo!("Implement ILPBkg")`
**When** the feature is not yet implemented
**Then** it SHALL either return an error (e.g., `BackgroundError::NotImplemented`)
**Or** be gated behind a feature flag that is not enabled by default
**And** calling unimplemented features SHALL not panic

### Requirement: Test Code Exception
Test code and benchmarks SHALL be exempt from panic elimination requirements and MAY continue using `.unwrap()` and `.expect()` where appropriate.

#### Scenario: tests can use unwrap
**Given** a test function in a `#[cfg(test)]` module
**When** calling operations that should succeed in the test context
**Then** `.unwrap()` or `.expect("descriptive message")` MAY be used
**And** this SHALL simplify test code
**And** failures SHALL provide clear context via expect messages

#### Scenario: benchmarks can use unwrap
**Given** a benchmark in `benches/` directory
**When** setting up test data or scenarios
**Then** `.unwrap()` MAY be used for operations that must succeed
**And** this SHALL keep benchmark code focused on performance measurement

## MODIFIED Requirements

### Requirement: Error Context Preservation
When converting between error types, context SHALL be preserved through error source chains.

#### Scenario: wrapping errors preserve source
**Given** an `IOError::ReadFailed` that wraps `std::io::Error`
**When** this is converted to `XAFSError::IO`
**Then** the full error chain SHALL be preserved
**And** calling `.source()` repeatedly SHALL traverse the chain
**And** root cause SHALL be accessible for debugging

#### Scenario: error messages compose
**Given** a nested error chain (e.g., XAFSError → IOError → std::io::Error)
**When** formatting the error for display
**Then** each layer SHALL contribute its context
**And** the full error description SHALL be informative
**And** thiserror SHALL handle composition automatically

### Requirement: API Compatibility
Error type changes SHALL maintain compatibility where possible to avoid breaking changes.

#### Scenario: existing error variants preserved
**Given** the current `XAFSError` enum variants
**When** migrating to thiserror
**Then** variant names SHALL be preserved where semantically equivalent
**And** error matching in existing code SHALL continue to work
**And** only implementation details SHALL change, not the public API

#### Scenario: new variants added non-breakingly
**Given** new domain-specific error types
**When** adding error variants
**Then** they SHALL be added as new variants (non-breaking)
**And** existing error handling code SHALL continue to compile
**And** this SHALL be a backwards-compatible addition

### Requirement: Result Type Alias
A type alias SHALL be provided for common Result types to reduce verbosity.

#### Scenario: XAFSResult type alias
**Given** the `xafs` module
**When** defining a Result type alias
**Then** `pub type Result<T> = std::result::Result<T, XAFSError>` SHALL exist
**And** functions MAY use `Result<T>` instead of `Result<T, XAFSError>`
**And** this SHALL reduce boilerplate in function signatures

## REMOVED Requirements

### Requirement: Box<dyn Error> Usage
Direct use of `Box<dyn Error>` SHALL be removed from public library functions.

#### Scenario: no Box dyn Error in public APIs
**Given** public functions in `xafs` modules
**When** examining return types
**Then** `Box<dyn Error>` SHALL NOT appear in public function signatures
**And** typed errors SHALL be used instead
**And** this SHALL enable better error handling for library consumers

#### Scenario: internal functions may use Box dyn Error temporarily
**Given** internal/private helper functions
**When** complete migration is not yet feasible
**Then** they MAY temporarily continue using `Box<dyn Error>`
**And** these SHALL be migrated in follow-up work
**And** they SHALL NOT be exposed in public API

### Requirement: Manual map_err Calls
Explicit `.map_err()` calls SHALL be removed where automatic error conversion is available.

#### Scenario: no map_err for automatic conversions
**Given** a function with `#[from]` error conversion
**When** propagating that error
**Then** `.map_err()` SHALL NOT be used
**And** the `?` operator alone SHALL suffice
**And** this SHALL reduce boilerplate code

**Before**:
```rust
std::fs::read_to_string(path).map_err(|e| IOError::ReadFailed { path: path.to_string(), source: e })?
```

**After**:
```rust
std::fs::read_to_string(path)?  // Automatic via #[from]
```
