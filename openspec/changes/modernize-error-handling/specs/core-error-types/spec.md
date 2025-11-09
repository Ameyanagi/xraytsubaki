# Core Error Types Specification

## ADDED Requirements

### Requirement: thiserror Dependency
The project SHALL include `thiserror` version 2.0 or compatible in workspace dependencies.

#### Scenario: thiserror dependency is available
**Given** the workspace Cargo.toml
**When** building the project
**Then** thiserror version 2.0.x is available to all workspace crates
**And** no compilation errors occur due to missing thiserror

### Requirement: XAFSError Modernization
The core `XAFSError` enum SHALL be implemented using thiserror derive macro instead of manual `Error` trait implementation.

#### Scenario: XAFSError uses thiserror
**Given** the file `crates/xraytsubaki/src/xafs/mod.rs`
**When** examining the `XAFSError` enum definition
**Then** it SHALL have `#[derive(Error, Debug, Clone)]` attributes
**And** it SHALL NOT have manual `impl Error for XAFSError` block
**And** each variant SHALL have a `#[error("...")]` attribute with descriptive message

#### Scenario: XAFSError eliminates deprecated methods
**Given** the `XAFSError` type implementation
**When** checking for Error trait methods
**Then** it SHALL NOT implement `description()` method (deprecated)
**And** it SHALL NOT implement `cause()` method (deprecated)
**And** error messages SHALL be provided via `Display` trait (automatic from thiserror)
**And** error sources SHALL be provided via `source()` method (automatic from thiserror)

### Requirement: Error Module Organization
Error types SHALL be organized in a dedicated `errors.rs` module within the `xafs` package.

#### Scenario: errors module exists
**Given** the `crates/xraytsubaki/src/xafs/` directory
**When** listing module files
**Then** an `errors.rs` file SHALL exist
**And** it SHALL be declared as `pub mod errors;` in `xafs/mod.rs`
**And** error types SHALL be re-exported from `xafs/mod.rs` for public API

### Requirement: Error Type Hierarchy
The error system SHALL provide domain-specific error types that compose into the top-level `XAFSError`.

#### Scenario: domain-specific error types exist
**Given** the `xafs/errors.rs` module
**When** examining error type definitions
**Then** `DataError` enum SHALL exist for data validation errors
**And** `NormalizationError` enum SHALL exist for normalization errors
**And** `BackgroundError` enum SHALL exist for AUTOBK errors
**And** `FFTError` enum SHALL exist for Fourier transform errors
**And** `IOError` enum SHALL exist for I/O operations
**And** `MathError` enum SHALL exist for mathematical operations
**And** each SHALL use `#[derive(Error, Debug, Clone)]`

#### Scenario: XAFSError aggregates domain errors
**Given** the `XAFSError` enum
**When** examining its variants
**Then** it SHALL have a variant for each domain error type
**And** variants SHALL use `#[from]` attribute for automatic conversion
**And** variants SHALL have descriptive `#[error]` messages that include the wrapped error

### Requirement: Clone-able Errors
All error types SHALL be `Clone` to support error propagation in parallel contexts.

#### Scenario: errors can be cloned
**Given** any error type (XAFSError, DataError, etc.)
**When** attempting to clone the error
**Then** the operation SHALL succeed
**And** the cloned error SHALL be equivalent to the original
**And** this SHALL support Rayon parallel error collection

## MODIFIED Requirements

### Requirement: Error Display Messages
Error display messages SHALL be context-rich and include relevant parameter values.

#### Scenario: error messages include context
**Given** a `DataError::InsufficientData` error with min=100 and actual=50
**When** formatting the error for display
**Then** the message SHALL include both the minimum required points (100)
**And** the message SHALL include the actual number of points (50)
**And** the message SHALL be actionable for debugging

**Example**:
```
"insufficient data: need at least 100 points, got 50"
```

#### Scenario: numeric errors include values
**Given** a `NormalizationError::E0OutOfRange` error
**When** the error is formatted
**Then** it SHALL include the e0 value that was out of range
**And** it SHALL include the valid data range boundaries
**And** the message SHALL clearly indicate why the operation failed

### Requirement: Error Source Chaining
Errors that wrap other errors SHALL preserve the error source for debugging.

#### Scenario: IO errors preserve source
**Given** an `IOError::ReadFailed` wrapping a `std::io::Error`
**When** calling `.source()` on the error
**Then** it SHALL return Some(&std::io::Error)
**And** the full error chain SHALL be traversable
**And** the original error information SHALL be preserved

## REMOVED Requirements

### Requirement: Manual Error Trait Implementation
Manual implementations of `std::error::Error` SHALL be removed.

#### Scenario: no manual Error impl for XAFSError
**Given** the file `crates/xraytsubaki/src/xafs/mod.rs`
**When** searching for `impl Error for XAFSError`
**Then** no manual implementation block SHALL exist
**And** Error trait SHALL be implemented automatically by thiserror derive

### Requirement: Deprecated Error Methods
Usage of deprecated Error trait methods SHALL be removed.

#### Scenario: no description() method
**Given** any error type in the codebase
**When** checking for `fn description(&self)` implementation
**Then** it SHALL NOT exist
**And** `Display` trait SHALL be used instead

#### Scenario: no cause() method
**Given** any error type in the codebase
**When** checking for `fn cause(&self)` implementation
**Then** it SHALL NOT exist
**And** `source()` method SHALL be used instead (provided by thiserror)
