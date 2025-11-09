# Domain-Specific Error Types Specification

## ADDED Requirements

### Requirement: DataError Type
A `DataError` enum SHALL capture all data validation and input error conditions.

#### Scenario: insufficient data points error
**Given** a function requiring minimum data points
**When** the input array has fewer points than required
**Then** a `DataError::InsufficientData` error SHALL be returned
**And** the error SHALL include the minimum required count
**And** the error SHALL include the actual count received

#### Scenario: array length mismatch error
**Given** operations requiring equal-length arrays (e.g., energy and mu)
**When** the arrays have different lengths
**Then** a `DataError::LengthMismatch` error SHALL be returned
**And** the error SHALL include both array lengths for debugging

#### Scenario: invalid data range error
**Given** energy or k-space range validation
**When** min >= max or range is invalid
**Then** a `DataError::InvalidEnergyRange` or similar error SHALL be returned
**And** the error SHALL include the problematic values

#### Scenario: non-finite values error
**Given** numerical data that must be finite
**When** NaN or Inf values are detected
**Then** a `DataError::NonFiniteValues` error SHALL be returned
**And** the error SHALL indicate which indices contain bad values

### Requirement: NormalizationError Type
A `NormalizationError` enum SHALL capture pre/post-edge normalization failure modes.

#### Scenario: edge energy out of range
**Given** a specified edge energy (e0)
**When** e0 is outside the data energy range
**Then** a `NormalizationError::E0OutOfRange` error SHALL be returned
**And** the error SHALL include the e0 value and valid range

#### Scenario: pre-edge fitting failure
**Given** pre-edge normalization with specified energy range
**When** insufficient data points exist in the range
**Then** a `NormalizationError::PreEdgeFitFailed` error SHALL be returned
**And** the error SHALL include the problematic range

#### Scenario: post-edge polynomial order too high
**Given** post-edge normalization with polynomial fitting
**When** the polynomial order exceeds available data points
**Then** a `NormalizationError::PostEdgeFitFailed` error SHALL be returned
**And** the error SHALL include the order and number of points

#### Scenario: edge step too small
**Given** edge step calculation during normalization
**When** the edge step is below minimum threshold
**Then** a `NormalizationError::EdgeStepTooSmall` error SHALL be returned
**And** the error SHALL include the calculated step and minimum threshold

### Requirement: BackgroundError Type
A `BackgroundError` enum SHALL capture AUTOBK algorithm failure modes.

#### Scenario: optimization convergence failure
**Given** AUTOBK Levenberg-Marquardt optimization
**When** the algorithm fails to converge
**Then** a `BackgroundError::ConvergenceFailure` error SHALL be returned
**And** the error SHALL include the number of iterations attempted

#### Scenario: invalid rbkg parameter
**Given** AUTOBK rbkg parameter validation
**When** rbkg <= 0 or is otherwise invalid
**Then** a `BackgroundError::InvalidRbkg` error SHALL be returned
**And** the error SHALL include the problematic rbkg value

#### Scenario: spline knot calculation failure
**Given** spline fitting for background removal
**When** the k-range is insufficient for knot placement
**Then** a `BackgroundError::SplineKnotsFailed` error SHALL be returned
**And** the error SHALL include the kmin and kmax values

#### Scenario: optimization failure with reason
**Given** any AUTOBK optimization failure
**When** the specific reason is known
**Then** a `BackgroundError::OptimizationFailed` error SHALL be returned
**And** the error SHALL include a descriptive reason string

### Requirement: FFTError Type
An `FFTError` enum SHALL capture Fourier transform operation failures.

#### Scenario: insufficient points for FFT
**Given** FFT operation with k-range requirements
**When** the input data has too few points
**Then** an `FFTError::InsufficientPoints` error SHALL be returned
**And** the error SHALL include minimum, actual counts, and k-range

#### Scenario: invalid FFT window function
**Given** FFT with window function specification
**When** an unknown or invalid window is specified
**Then** an `FFTError::InvalidWindow` error SHALL be returned
**And** the error SHALL include the problematic window name

#### Scenario: IFFT size mismatch
**Given** inverse FFT operation
**When** the chi(R) array size doesn't match expectations
**Then** an `FFTError::IFFTSizeMismatch` error SHALL be returned
**And** the error SHALL include expected and actual sizes

### Requirement: IOError Type
An `IOError` enum SHALL capture file I/O and serialization failures.

#### Scenario: file not found
**Given** file reading operation
**When** the specified file does not exist
**Then** an `IOError::FileNotFound` error SHALL be returned
**And** the error SHALL include the file path

#### Scenario: file read failure with source
**Given** file reading operation
**When** an I/O error occurs (permissions, disk error, etc.)
**Then** an `IOError::ReadFailed` error SHALL be returned
**And** the error SHALL include the file path
**And** the error SHALL preserve the source `std::io::Error` via `#[source]`

#### Scenario: JSON deserialization failure
**Given** JSON file parsing
**When** serde_json returns an error
**Then** an `IOError::JsonError` error SHALL be returned
**And** the error SHALL wrap the `serde_json::Error` via `#[from]`

#### Scenario: BSON deserialization failure
**Given** BSON file parsing
**When** bson::de returns an error
**Then** an `IOError::BsonError` error SHALL be returned
**And** the error SHALL wrap the `bson::de::Error` via `#[from]`

### Requirement: MathError Type
A `MathError` enum SHALL capture mathematical operation failures.

#### Scenario: interpolation out of bounds
**Given** interpolation operation at point x
**When** x is outside the data range [xmin, xmax]
**Then** a `MathError::InterpolationOutOfBounds` error SHALL be returned
**And** the error SHALL include x, xmin, and xmax values

#### Scenario: polynomial fit failure
**Given** polynomial fitting operation
**When** the fit fails (e.g., singular matrix)
**Then** a `MathError::PolyfitFailed` error SHALL be returned
**And** the error SHALL include a descriptive reason

#### Scenario: spline evaluation failure
**Given** spline evaluation at point x
**When** the evaluation fails
**Then** a `MathError::SplineEvalFailed` error SHALL be returned
**And** the error SHALL include the x value and failure reason

#### Scenario: array index out of bounds
**Given** array indexing operation
**When** the index exceeds array length
**Then** a `MathError::IndexOutOfBounds` error SHALL be returned
**And** the error SHALL include the index and array length

## MODIFIED Requirements

### Requirement: Error Conversion Attributes
Domain errors SHALL use `#[from]` attribute for common error sources where applicable.

#### Scenario: automatic std::io::Error conversion
**Given** a function returning `Result<T, IOError>`
**When** a `std::io::Error` is encountered
**Then** it SHALL automatically convert to `IOError` via `?` operator
**And** the `#[from]` attribute SHALL enable this conversion

#### Scenario: automatic serde_json::Error conversion
**Given** a function returning `Result<T, IOError>`
**When** a `serde_json::Error` is encountered
**Then** it SHALL automatically convert to `IOError::JsonError` via `?`
**And** the conversion SHALL preserve the original error as source

### Requirement: Error Message Formatting
Each error variant SHALL have a `#[error("...")]` attribute with a clear, actionable message.

#### Scenario: error messages use named fields
**Given** an error variant with fields
**When** the error is displayed
**Then** the message SHALL reference field values using `{field_name}` syntax
**And** the message SHALL be human-readable and descriptive

**Example**:
```rust
#[error("insufficient data for FFT: expected at least {min} points, got {actual}")]
InsufficientPoints { min: usize, actual: usize },
```

#### Scenario: error messages for unit variants
**Given** an error variant with no fields
**When** the error is displayed
**Then** the message SHALL be a static string describing the error condition

## REMOVED Requirements

None - this capability adds new error types without removing existing functionality.
