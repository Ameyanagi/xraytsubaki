//! Processing, analysis, fitting, structures and interchange for XAS spectra.
//!
//! Most applications begin with [`crate::Spectrum`] and [`crate::Group`]. This
//! namespace also exposes lower-level algorithms and historical compatibility
//! helpers. Select stage settings through spectrum setters so dependent results
//! are invalidated automatically. Direct helper calls do not provide that cache
//! management or necessarily the checked constructor's input validation.
//!
//! The default implementation uses nalgebra. Cargo feature `ndarray-compat`
//! selects alternative normalization, background and Fourier modules under the
//! same paths; its historical defaults and solver policies can differ. See the
//! selected module's documentation before comparing outputs.

#![allow(dead_code)]
#![allow(unused_imports)]

#[cfg_attr(debug_assertions, allow(dead_code, unused_imports))]
// Standard library dependencies
use std::error::Error;
use std::fmt;

// Error handling
use thiserror::Error;

use easyfft::dyn_size::realfft::DynRealDft;
// External dependencies

// load dependencies
pub mod analysis;
#[cfg(feature = "ndarray-compat")]
#[path = "background_ndarray.rs"]
/// AUTOBK background settings, solver policies and outputs.
pub mod background;
#[cfg(not(feature = "ndarray-compat"))]
/// AUTOBK background settings, solver policies and outputs.
pub mod background;
/// Modified Bessel evaluation used by Fourier windows.
pub mod bessel_i0;
pub mod errors;
mod fft_grid;
pub mod fitting;
pub mod fluorescence;
mod inverse_fft;
pub mod io;
/// Historical Levenberg–Marquardt solver settings.
pub mod lmutils;
#[cfg(feature = "ndarray-compat")]
#[path = "mathutils_ndarray.rs"]
pub mod mathutils;
#[cfg(not(feature = "ndarray-compat"))]
pub mod mathutils;
pub mod mback;
#[cfg(feature = "ndarray-compat")]
#[path = "normalization_ndarray.rs"]
/// Pre/post-edge normalization and algorithm selection.
pub mod normalization;
#[cfg(not(feature = "ndarray-compat"))]
/// Pre/post-edge normalization and algorithm selection.
pub mod normalization;
pub mod nshare;
pub mod ranges;
pub mod rmc;
pub(crate) mod spline;
pub mod structure;
pub mod tools;
pub mod transform;
pub mod wavelet;
#[cfg(feature = "ndarray-compat")]
#[path = "xafsutils_ndarray.rs"]
/// Wave-number conversion, windows, smoothing and edge-location helpers.
pub mod xafsutils;
#[cfg(not(feature = "ndarray-compat"))]
/// Wave-number conversion, windows, smoothing and edge-location helpers.
pub mod xafsutils;
/// Ordered spectra with sequential and parallel processing.
pub mod xasgroup;
pub mod xasparameters;
/// One spectrum, mutable stage settings and calculated results.
pub mod xasspectrum;
#[cfg(feature = "ndarray-compat")]
#[path = "xrayfft_ndarray.rs"]
/// Forward and inverse XAFS Fourier settings and results.
pub mod xrayfft;
#[cfg(not(feature = "ndarray-compat"))]
/// Forward and inverse XAFS Fourier settings and results.
pub mod xrayfft;

// Load local traits
use mathutils::MathUtils;
use normalization::Normalization;
use xafsutils::XAFSUtils;

// Re-export error types for public API
pub use errors::{
    AnalysisError, BackgroundError, DataError, FFTError, IOError, MathError, NormalizationError,
};
pub use fitting::errors::FittingError;

/// Top-level error type that aggregates domain-specific errors.
/// Match variants for programmatic handling and use Display for a user-facing
/// diagnostic. A failed stage does not guarantee rollback of earlier stages.
/// Legacy boxed-error conversions preserve text but wrap it as a missing-data
/// diagnostic; inspect the originating API rather than assuming every error has
/// retained its original typed cause.
#[derive(Error, Debug, Clone)]
pub enum XAFSError {
    #[error("data error: {0}")]
    /// Input data, indexing or prerequisite availability failed.
    Data(#[from] DataError),

    #[error("normalization error: {0}")]
    /// Absorption normalization failed.
    Normalization(#[from] NormalizationError),

    #[error("background removal error: {0}")]
    /// Smooth-background estimation failed.
    Background(#[from] BackgroundError),

    #[error("FFT error: {0}")]
    /// Forward/inverse transform validation or calculation failed.
    FFT(#[from] FFTError),

    #[error("I/O error: {0}")]
    /// Reading, writing or interchange conversion failed.
    IO(#[from] IOError),

    #[error("mathematical operation failed: {0}")]
    /// An interpolation, polynomial fit or numerical helper failed.
    Math(#[from] MathError),

    #[error("fitting operation failed: {0}")]
    /// The scattering-path fit or fitting configuration failed.
    Fitting(#[from] FittingError),

    // Legacy error variants for backwards compatibility
    #[error("not enough data")]
    /// Historical insufficient-input variant; newer APIs generally return `Data`.
    NotEnoughData,

    #[error("not enough data for XFTF")]
    /// Historical forward-transform input failure; newer APIs generally return `FFT`.
    NotEnoughDataForXFTF,

    #[error("not enough data for XFTR")]
    /// Historical inverse-transform input failure; newer APIs generally return `FFT`.
    NotEnoughDataForXFTR,

    #[error("group index out of range")]
    /// Historical group index failure; newer checked methods generally return `Data`.
    GroupIndexOutOfRange,

    #[error("group is empty")]
    /// Historical empty-group failure; newer checked methods generally return `Data`.
    GroupIsEmpty,
}

impl From<Box<dyn std::error::Error>> for XAFSError {
    fn from(value: Box<dyn std::error::Error>) -> Self {
        XAFSError::Data(DataError::MissingData {
            field: value.to_string(),
        })
    }
}

/// Convenience type alias for Results using XAFSError.
pub type Result<T> = std::result::Result<T, XAFSError>;

#[cfg(test)]
pub mod tests {
    use super::*;
    use data_reader::reader::{load_txt_f64, Delimiter, ReaderParams};

    pub const TOP_DIR: &str = env!("CARGO_MANIFEST_DIR");
    pub const PARAM_LOADTXT: ReaderParams = ReaderParams {
        comments: Some(b'#'),
        delimiter: Delimiter::WhiteSpace,
        skip_footer: None,
        skip_header: None,
        usecols: None,
        max_rows: None,
        row_format: true,
    };
    pub const TEST_TOL: f64 = 1e-12;

    pub const TEST_TOL_LESS_ACC: f64 = 1e-8;
}
