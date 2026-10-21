//! Error types for XAFS analysis operations.
//!
//! This module defines domain-specific error types using the `thiserror` crate
//! for better error handling throughout the library.

use enterpolation::linear::LinearError;
use thiserror::Error;

/// Errors related to data validation and input processing.
#[derive(Error, Debug, Clone)]
pub enum DataError {
    /// A constant energy-axis correction cannot be represented safely.
    #[error("invalid energy offset {offset_ev} eV: {reason}")]
    InvalidEnergyOffset {
        /// Requested total energy offset, in eV.
        offset_ev: f64,
        /// Nonfinite input, overflow or loss of energy-grid spacing.
        reason: String,
    },

    #[error("insufficient data: need at least {min} points, got {actual}")]
    /// The operation received fewer samples than its minimum requirement.
    InsufficientData {
        /// Minimum required sample count.
        min: usize,
        /// Actual sample count.
        actual: usize,
    },

    #[error("data array length mismatch: energy has {energy_len} points, mu has {mu_len} points")]
    /// Paired arrays or related lists have different lengths. Some tools reuse these field names for non-energy arrays.
    LengthMismatch {
        /// Length of the first array/list, normally energy.
        energy_len: usize,
        /// Length of the second array/list, normally absorption.
        mu_len: usize,
    },

    #[error("invalid energy range: min={min}, max={max}")]
    /// The selected energy bounds do not describe a usable interval.
    InvalidEnergyRange {
        /// Lower energy bound, in eV.
        min: f64,
        /// Upper energy bound, in eV.
        max: f64,
    },

    #[error(
        "energy must be monotonic non-decreasing, but energy[{index}]={curr} is smaller than previous value {prev}"
    )]
    /// An energy sample is smaller than its predecessor; reorder paired energy and absorption together.
    NonMonotonicEnergy {
        /// Zero-based failing index.
        index: usize,
        /// Previous energy sample, in eV.
        prev: f64,
        /// Current energy sample, in eV.
        curr: f64,
    },

    #[error("duplicate energy at index {index}: {energy}; expected strictly increasing energy")]
    /// The checked constructor found repeated energy; combine or remove duplicate measurements explicitly.
    DuplicateEnergy {
        /// Zero-based failing index.
        index: usize,
        /// Repeated energy value, in eV.
        energy: f64,
    },

    #[error("data contains non-finite values at indices: {indices:?}")]
    /// Input validation found NaN or infinite values; indices refer to the original paired samples.
    NonFiniteValues {
        /// Zero-based indices containing non-finite values.
        indices: Vec<usize>,
    },

    #[error("missing required data: {field}")]
    /// A required array, stage result or configuration value is unavailable.
    MissingData {
        /// Missing field or result name.
        field: String,
    },

    #[error("index out of range: index={index}, length={length}")]
    /// A checked collection/list operation received an invalid zero-based index.
    IndexOutOfRange {
        /// Zero-based failing index.
        index: usize,
        /// Number of elements in the collection.
        length: usize,
    },

    #[error("group is empty")]
    /// The requested operation needs at least one spectrum.
    EmptyGroup,

    #[error("feature not implemented: {feature}")]
    /// The selected feature exists as a placeholder and has no implementation.
    NotImplemented {
        /// Name of the unimplemented feature.
        feature: String,
    },
}

/// Errors related to pre/post-edge normalization operations.
#[derive(Error, Debug, Clone)]
pub enum NormalizationError {
    #[error("edge energy (e0={e0}) is outside data range [{data_min}, {data_max}]")]
    /// The edge energy is non-finite or not strictly inside the measured energy interval.
    E0OutOfRange {
        /// Configured edge energy, in eV.
        e0: f64,
        /// Minimum measured energy, in eV.
        data_min: f64,
        /// Maximum measured energy, in eV.
        data_max: f64,
    },

    #[error("pre-edge fitting failed: not enough points in range [{start}, {end}]")]
    /// The selected absolute pre-edge interval contains insufficient usable points for a baseline fit.
    PreEdgeFitFailed {
        /// Requested absolute start energy, in eV.
        start: f64,
        /// Requested absolute end energy, in eV.
        end: f64,
    },

    #[error("post-edge fitting failed: polynomial order {order} too high for {n_points} points")]
    /// The requested post-edge polynomial cannot be determined from the available points.
    PostEdgeFitFailed {
        /// Requested polynomial degree.
        order: usize,
        /// Number of usable selected points.
        n_points: usize,
    },

    #[error("{field}: {reason}")]
    /// A fit range is reversed or holds too few measured points, even after the
    /// automatic limiting to the measured data (since 0.2.15). `field` names the
    /// setting to change, for example `pre_edge_end` or `norm_start`.
    InvalidFitRange {
        /// Setting name, for example `pre_edge_start` or `norm_start`.
        field: String,
        /// Explanation that includes the range used and the measured limit.
        reason: String,
    },

    #[error("edge step is too small: {edge_step} (minimum: {min})")]
    /// The normalization scale is too small for stable division; inspect the selected edge and fitting ranges.
    EdgeStepTooSmall {
        /// Estimated or configured scale, in the input absorption units.
        edge_step: f64,
        /// Minimum accepted scale in the input absorption units.
        min: f64,
    },

    #[error("normalization method not implemented: {method}")]
    /// The chosen normalization method is a placeholder; no default algorithm was substituted.
    NotImplemented {
        /// Name of the selected normalization method.
        method: String,
    },

    #[error("normalization failed: {message}")]
    /// A normalization failure represented by its diagnostic text, including converted lower-level errors.
    Other {
        /// Human-readable diagnostic from the failing operation.
        message: String,
    },
}

/// Errors related to AUTOBK background removal algorithm.
#[derive(Error, Debug, Clone)]
pub enum BackgroundError {
    #[error("AUTOBK optimization failed: {reason}")]
    /// The background optimizer failed; the diagnostic describes the solver or objective failure.
    OptimizationFailed {
        /// Human-readable failure reason.
        reason: String,
    },

    #[error("AUTOBK direct linear solver failed: {reason}")]
    /// The direct spline-coefficient solve failed numerically.
    DirectSolverFailed {
        /// Human-readable failure reason.
        reason: String,
    },

    #[error(
        "AUTOBK direct solver rejected ill-conditioned system: condition proxy {condition_proxy} exceeds limit {limit}"
    )]
    /// A legacy direct-solver conditioning check exceeded its configured limit. The proxy is not an exact matrix condition number.
    DirectSolverIllConditioned {
        /// Dimensionless conditioning proxy reported by the legacy solve.
        condition_proxy: f64,
        /// Configured maximum acceptable conditioning proxy.
        limit: f64,
    },

    #[error("invalid rbkg parameter: {rbkg} (must be > 0)")]
    /// The requested low-R background cutoff is invalid.
    InvalidRbkg {
        /// Requested background cutoff, in Å.
        rbkg: f64,
    },

    #[error("{field}: {reason}")]
    /// The background k range is reversed or lies beyond the measured data
    /// (since 0.2.15). `field` names the setting to change, such as `kmin`.
    InvalidFitRange {
        /// Setting name, for example `kmin`.
        field: String,
        /// Explanation that includes the requested value and the measured limit.
        reason: String,
    },

    #[error("spline knot calculation failed: insufficient k-range [{kmin}, {kmax}]")]
    /// The selected wave-number range cannot support the requested spline basis.
    SplineKnotsFailed {
        /// Wave-number interval start, in Å⁻¹; can be zero when the range could not be resolved.
        kmin: f64,
        /// Wave-number interval end, in Å⁻¹; can be zero when the range could not be resolved.
        kmax: f64,
    },

    #[error("Levenberg-Marquardt did not converge after {iterations} iterations")]
    /// A Levenberg–Marquardt background solve reached its reported iteration limit without convergence.
    ConvergenceFailure {
        /// Number of optimizer iterations reported at failure.
        iterations: usize,
    },

    #[error("background removal feature not implemented: {feature}")]
    /// The selected background feature is not implemented; no alternative is silently chosen.
    NotImplemented {
        /// Name of the unimplemented feature.
        feature: String,
    },

    #[error("background calculation failed: {message}")]
    /// A background failure represented by text, including converted data, normalization or interpolation errors.
    Other {
        /// Human-readable diagnostic from the failing operation.
        message: String,
    },
}

/// Errors related to Fourier transform operations.
#[derive(Error, Debug, Clone)]
pub enum FFTError {
    #[error("invalid FFT parameter {parameter}: {reason}")]
    /// A Fourier setting is invalid or incompatible with the supplied grid.
    InvalidParameter {
        /// Name of the invalid Fourier setting.
        parameter: String,
        /// Human-readable failure reason.
        reason: String,
    },

    #[error("FFT requires at least {min} points for k-range [{kmin}, {kmax}], got {actual}")]
    /// The forward transform received too few wave-number samples.
    InsufficientPoints {
        /// Minimum required sample count.
        min: usize,
        /// Actual sample count.
        actual: usize,
        /// Wave-number interval start, in Å⁻¹; can be zero when the range could not be resolved.
        kmin: f64,
        /// Wave-number interval end, in Å⁻¹; can be zero when the range could not be resolved.
        kmax: f64,
    },

    #[error("invalid FFT window: {window}")]
    /// The requested Fourier window is not supported by the operation.
    InvalidWindow {
        /// Requested window name.
        window: String,
    },

    #[error("IFFT failed: chi(R) array has {actual} points, expected {expected}")]
    /// The stored complex Fourier representation has the wrong length for the inverse transform.
    IFFTSizeMismatch {
        /// Required number of complex Fourier samples.
        expected: usize,
        /// Actual number of complex Fourier samples.
        actual: usize,
    },

    #[error("interpolation failed: {reason}")]
    /// Fourier grid preparation or paired input validation failed.
    InterpolationFailed {
        /// Human-readable failure reason.
        reason: String,
    },

    #[error("window function calculation failed: {reason}")]
    /// The window could not be evaluated for the supplied grid/settings.
    WindowCalculationFailed {
        /// Human-readable failure reason.
        reason: String,
    },
}

/// Errors related to file I/O and serialization operations.
#[derive(Error, Debug, Clone)]
pub enum IOError {
    #[error("file not found: {path}")]
    /// The requested file could not be found.
    FileNotFound {
        /// Filesystem path involved in the operation.
        path: String,
    },

    #[error("failed to read file {path}: {kind}")]
    /// Opening or reading a file failed. Legacy JSON/BSON writers also use this variant for file-creation failures.
    ReadFailed {
        /// Filesystem path involved in the operation.
        path: String,
        /// Underlying operating-system I/O error category.
        kind: std::io::ErrorKind,
    },

    #[error("JSON deserialization failed: {message}")]
    /// JSON serialization or deserialization failed; the legacy display label mentions only deserialization.
    JsonError {
        /// Human-readable diagnostic from the failing operation.
        message: String,
    },

    #[error("BSON deserialization failed: {message}")]
    /// BSON serialization or deserialization failed; the legacy display label mentions only deserialization.
    BsonError {
        /// Human-readable diagnostic from the failing operation.
        message: String,
    },

    #[error("compression error: {message}")]
    /// Compression or decompression failed, including malformed compressed input.
    CompressionError {
        /// Human-readable diagnostic from the failing operation.
        message: String,
    },

    #[error("failed to write file {path}: {kind}")]
    /// Writing, creating or flushing an output file failed.
    WriteFailed {
        /// Filesystem path involved in the operation.
        path: String,
        /// Underlying operating-system I/O error category.
        kind: std::io::ErrorKind,
    },

    #[error("not an Athena project file: {reason}")]
    /// The input does not identify a supported Athena project.
    NotAthenaProject {
        /// Human-readable failure reason.
        reason: String,
    },

    #[error("Athena project parse error at line {line}: {message}")]
    /// An Athena project statement could not be parsed at the reported source line.
    AthenaParse {
        /// One-based source line number.
        line: usize,
        /// Human-readable diagnostic from the failing operation.
        message: String,
    },

    #[error("cannot export to Athena project: {reason}")]
    /// Spectrum content cannot be represented by the Athena exporter.
    AthenaExport {
        /// Human-readable failure reason.
        reason: String,
    },
}

/// Errors related to mathematical operations.
#[derive(Error, Debug, Clone)]
pub enum MathError {
    #[error("interpolation failed: x value {x} is outside range [{xmin}, {xmax}]")]
    /// A query falls outside the interpolation interval for an operation that forbids extrapolation.
    InterpolationOutOfBounds {
        /// Query coordinate in the units of the interpolation/spline input.
        x: f64,
        /// Lower interpolation bound in the query coordinate units.
        xmin: f64,
        /// Upper interpolation bound in the query coordinate units.
        xmax: f64,
    },

    #[error("polynomial fit failed: {reason}")]
    /// A polynomial fit failed, for example because its coordinates are degenerate.
    PolyfitFailed {
        /// Human-readable failure reason.
        reason: String,
    },

    #[error("spline evaluation failed at x={x}: {reason}")]
    /// Spline construction/evaluation failed. Some wrappers use x = 0 as a placeholder rather than a failing sample.
    SplineEvalFailed {
        /// Query coordinate in the units of the interpolation/spline input.
        x: f64,
        /// Human-readable failure reason.
        reason: String,
    },

    #[error("index {index} out of bounds for array of length {len}")]
    /// An index is invalid for the given array length.
    IndexOutOfBounds {
        /// Zero-based failing index.
        index: usize,
        /// Number of elements in the array or collection.
        len: usize,
    },
}

impl From<MathError> for NormalizationError {
    fn from(value: MathError) -> Self {
        Self::Other {
            message: value.to_string(),
        }
    }
}

impl From<DataError> for NormalizationError {
    fn from(value: DataError) -> Self {
        Self::Other {
            message: value.to_string(),
        }
    }
}

impl From<&'static str> for NormalizationError {
    fn from(value: &'static str) -> Self {
        Self::Other {
            message: value.to_string(),
        }
    }
}

impl From<Box<dyn std::error::Error>> for NormalizationError {
    fn from(value: Box<dyn std::error::Error>) -> Self {
        Self::Other {
            message: value.to_string(),
        }
    }
}

impl From<LinearError> for NormalizationError {
    fn from(value: LinearError) -> Self {
        Self::Other {
            message: value.to_string(),
        }
    }
}

impl From<MathError> for BackgroundError {
    fn from(value: MathError) -> Self {
        Self::Other {
            message: value.to_string(),
        }
    }
}

impl From<DataError> for BackgroundError {
    fn from(value: DataError) -> Self {
        Self::Other {
            message: value.to_string(),
        }
    }
}

impl From<NormalizationError> for BackgroundError {
    fn from(value: NormalizationError) -> Self {
        Self::Other {
            message: value.to_string(),
        }
    }
}

impl From<LinearError> for BackgroundError {
    fn from(value: LinearError) -> Self {
        Self::Other {
            message: value.to_string(),
        }
    }
}

impl From<Box<dyn std::error::Error>> for BackgroundError {
    fn from(value: Box<dyn std::error::Error>) -> Self {
        Self::Other {
            message: value.to_string(),
        }
    }
}

/// Errors raised by the linear-combination-fitting / PCA analysis module.
#[derive(Error, Debug, Clone)]
pub enum AnalysisError {
    /// Normalization or background subtraction failed on a temporary input copy.
    #[error("could not prepare {spectrum}: {source}")]
    Preparation {
        /// Input name, or an explicit unnamed-input label.
        spectrum: String,
        /// Original processing error.
        #[source]
        source: Box<super::XAFSError>,
    },
    /// Analysis data or their edge origin are not usable.
    #[error("invalid analysis input {spectrum}: {reason}")]
    InvalidInput {
        /// Input name.
        spectrum: String,
        /// Contextual validation failure.
        reason: String,
    },
    /// Bounds must be finite and strictly increasing; they are never swapped.
    #[error("analysis range [{lo}, {hi}] must be finite and increasing")]
    InvalidRange {
        /// Requested lower bound, in axis units or energy offsets.
        lo: f64,
        /// Requested upper bound, in the same units.
        hi: f64,
    },
    /// The entire requested interval must be measured; extrapolation is disabled.
    #[error("{spectrum} covers [{available_lo}, {available_hi}], not the requested [{lo}, {hi}]; extrapolation is disabled")]
    IncompleteCoverage {
        /// Input name.
        spectrum: String,
        /// Requested absolute lower bound, in eV or Å⁻¹.
        lo: f64,
        /// Requested absolute upper bound.
        hi: f64,
        /// First available axis value.
        available_lo: f64,
        /// Last available axis value.
        available_hi: f64,
    },

    #[error("missing array for analysis: {field}")]
    /// The chosen analysis space requires an array that has not been calculated.
    MissingArray {
        /// Missing field or result name.
        field: String,
    },

    #[error("no standards / spectra supplied")]
    /// No standards or training spectra were supplied.
    NoSpectra,

    #[error("insufficient spectra: need at least {min}, got {actual}")]
    /// Too few spectra were supplied for the analysis.
    InsufficientSpectra {
        /// Minimum required spectrum count.
        min: usize,
        /// Number of supplied spectra.
        actual: usize,
    },

    #[error("fit range [{lo}, {hi}] selects only {n_points} points (need at least {min})")]
    /// The selected analysis interval contains fewer usable points than required.
    EmptyRange {
        /// Lower bound in the selected analysis-axis units (eV or Å⁻¹).
        lo: f64,
        /// Upper bound in the selected analysis-axis units (eV or Å⁻¹).
        hi: f64,
        /// Number of usable selected points.
        n_points: usize,
        /// Minimum required sample count.
        min: usize,
    },

    #[error(
        "weight constraints are infeasible: sum-to-one target {target} not within [{lo}, {hi}]"
    )]
    /// The sum-to-one target lies outside the combined permitted weight bounds.
    InfeasibleConstraints {
        /// Required dimensionless sum of component weights.
        target: f64,
        /// Sum of component lower weight bounds, dimensionless.
        lo: f64,
        /// Sum of component upper weight bounds, dimensionless.
        hi: f64,
    },

    #[error("{count} standard combinations requested, exceeding the cap of {max}")]
    /// The requested combinatorial search exceeds the configured cap.
    TooManyCombinations {
        /// Number of candidate combinations.
        count: usize,
        /// Configured maximum number of candidate combinations.
        max: usize,
    },

    #[error("requested {requested} components but the model has only {available}")]
    /// The requested reconstruction rank exceeds the stored component count.
    TooManyComponents {
        /// Requested number of components.
        requested: usize,
        /// Number of components present in the model.
        available: usize,
    },

    #[error("linear algebra failed: {reason}")]
    /// The matrix solve or decomposition failed numerically.
    LinearAlgebra {
        /// Human-readable failure reason.
        reason: String,
    },

    #[error(transparent)]
    /// A typed lower-level spectrum error propagated into the analysis.
    Xafs(#[from] super::XAFSError),
}

impl From<DataError> for AnalysisError {
    fn from(value: DataError) -> Self {
        Self::Xafs(value.into())
    }
}

impl From<MathError> for AnalysisError {
    fn from(value: MathError) -> Self {
        Self::Xafs(value.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_error_insufficient_data() {
        let error = DataError::InsufficientData {
            min: 100,
            actual: 50,
        };
        let msg = error.to_string();
        assert!(msg.contains("100"));
        assert!(msg.contains("50"));
        assert!(msg.contains("insufficient data"));
    }

    #[test]
    fn test_data_error_length_mismatch() {
        let error = DataError::LengthMismatch {
            energy_len: 100,
            mu_len: 95,
        };
        let msg = error.to_string();
        assert!(msg.contains("100"));
        assert!(msg.contains("95"));
        assert!(msg.contains("mismatch"));
    }

    #[test]
    fn test_normalization_error_e0_out_of_range() {
        let error = NormalizationError::E0OutOfRange {
            e0: 8000.0,
            data_min: 8100.0,
            data_max: 8500.0,
        };
        let msg = error.to_string();
        assert!(msg.contains("8000"));
        assert!(msg.contains("8100"));
        assert!(msg.contains("8500"));
    }

    #[test]
    fn test_background_error_convergence_failure() {
        let error = BackgroundError::ConvergenceFailure { iterations: 500 };
        let msg = error.to_string();
        assert!(msg.contains("500"));
        assert!(msg.contains("converge"));
    }

    #[test]
    fn test_fft_error_insufficient_points() {
        let error = FFTError::InsufficientPoints {
            min: 64,
            actual: 32,
            kmin: 0.0,
            kmax: 15.0,
        };
        let msg = error.to_string();
        assert!(msg.contains("64"));
        assert!(msg.contains("32"));
    }

    #[test]
    fn test_io_error_file_not_found() {
        let error = IOError::FileNotFound {
            path: "/path/to/file.dat".to_string(),
        };
        let msg = error.to_string();
        assert!(msg.contains("file.dat"));
        assert!(msg.contains("not found"));
    }

    #[test]
    fn test_math_error_interpolation_out_of_bounds() {
        let error = MathError::InterpolationOutOfBounds {
            x: 100.0,
            xmin: 0.0,
            xmax: 50.0,
        };
        let msg = error.to_string();
        assert!(msg.contains("100"));
        assert!(msg.contains("50"));
    }

    #[test]
    fn test_error_is_clone() {
        let error = DataError::InsufficientData { min: 10, actual: 5 };
        let cloned = error.clone();
        assert_eq!(error.to_string(), cloned.to_string());
    }
}
