//! Error types for XAFS analysis operations.
//!
//! This module defines domain-specific error types using the `thiserror` crate
//! for better error handling throughout the library.

use thiserror::Error;

/// Errors related to data validation and input processing.
#[derive(Error, Debug, Clone)]
pub enum DataError {
    #[error("insufficient data: need at least {min} points, got {actual}")]
    InsufficientData { min: usize, actual: usize },

    #[error("data array length mismatch: energy has {energy_len} points, mu has {mu_len} points")]
    LengthMismatch {
        energy_len: usize,
        mu_len: usize,
    },

    #[error("invalid energy range: min={min}, max={max}")]
    InvalidEnergyRange { min: f64, max: f64 },

    #[error("data contains non-finite values at indices: {indices:?}")]
    NonFiniteValues { indices: Vec<usize> },

    #[error("missing required data: {field}")]
    MissingData { field: String },
}

/// Errors related to pre/post-edge normalization operations.
#[derive(Error, Debug, Clone)]
pub enum NormalizationError {
    #[error("edge energy (e0={e0}) is outside data range [{data_min}, {data_max}]")]
    E0OutOfRange {
        e0: f64,
        data_min: f64,
        data_max: f64,
    },

    #[error("pre-edge fitting failed: not enough points in range [{start}, {end}]")]
    PreEdgeFitFailed { start: f64, end: f64 },

    #[error("post-edge fitting failed: polynomial order {order} too high for {n_points} points")]
    PostEdgeFitFailed { order: usize, n_points: usize },

    #[error("edge step is too small: {edge_step} (minimum: {min})")]
    EdgeStepTooSmall { edge_step: f64, min: f64 },

    #[error("normalization method not implemented: {method}")]
    NotImplemented { method: String },
}

/// Errors related to AUTOBK background removal algorithm.
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

    #[error("background removal feature not implemented: {feature}")]
    NotImplemented { feature: String },
}

/// Errors related to Fourier transform operations.
#[derive(Error, Debug, Clone)]
pub enum FFTError {
    #[error("FFT requires at least {min} points for k-range [{kmin}, {kmax}], got {actual}")]
    InsufficientPoints {
        min: usize,
        actual: usize,
        kmin: f64,
        kmax: f64,
    },

    #[error("invalid FFT window: {window}")]
    InvalidWindow { window: String },

    #[error("IFFT failed: chi(R) array has {actual} points, expected {expected}")]
    IFFTSizeMismatch { expected: usize, actual: usize },
}

/// Errors related to file I/O and serialization operations.
#[derive(Error, Debug, Clone)]
pub enum IOError {
    #[error("file not found: {path}")]
    FileNotFound { path: String },

    #[error("failed to read file {path}: {source}")]
    ReadFailed {
        path: String,
        #[source]
        source: std::io::ErrorKind,
    },

    #[error("JSON deserialization failed: {message}")]
    JsonError { message: String },

    #[error("BSON deserialization failed: {message}")]
    BsonError { message: String },

    #[error("compression error: {message}")]
    CompressionError { message: String },
}

/// Errors related to mathematical operations.
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
        let error = DataError::InsufficientData {
            min: 10,
            actual: 5,
        };
        let cloned = error.clone();
        assert_eq!(error.to_string(), cloned.to_string());
    }
}
