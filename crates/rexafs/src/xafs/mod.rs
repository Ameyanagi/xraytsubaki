//! EXAFS modules
//!
//!

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
pub mod background;
#[cfg(not(feature = "ndarray-compat"))]
pub mod background;
pub mod bessel_i0;
pub mod errors;
pub mod fitting;
mod inverse_fft;
pub mod io;
pub mod lmutils;
#[cfg(feature = "ndarray-compat")]
#[path = "mathutils_ndarray.rs"]
pub mod mathutils;
#[cfg(not(feature = "ndarray-compat"))]
pub mod mathutils;
#[cfg(feature = "ndarray-compat")]
#[path = "normalization_ndarray.rs"]
pub mod normalization;
#[cfg(not(feature = "ndarray-compat"))]
pub mod normalization;
pub mod nshare;
pub(crate) mod spline;
pub mod structure;
pub mod tools;
#[cfg(feature = "ndarray-compat")]
#[path = "xafsutils_ndarray.rs"]
pub mod xafsutils;
#[cfg(not(feature = "ndarray-compat"))]
pub mod xafsutils;
pub mod xasgroup;
pub mod xasparameters;
pub mod xasspectrum;
#[cfg(feature = "ndarray-compat")]
#[path = "xrayfft_ndarray.rs"]
pub mod xrayfft;
#[cfg(not(feature = "ndarray-compat"))]
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

/// Top-level error type that aggregates all domain-specific errors.
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

    #[error("fitting operation failed: {0}")]
    Fitting(#[from] FittingError),

    // Legacy error variants for backwards compatibility
    #[error("not enough data")]
    NotEnoughData,

    #[error("not enough data for XFTF")]
    NotEnoughDataForXFTF,

    #[error("not enough data for XFTR")]
    NotEnoughDataForXFTR,

    #[error("group index out of range")]
    GroupIndexOutOfRange,

    #[error("group is empty")]
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
