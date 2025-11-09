//! EXAFS modules
//!
//!

#![allow(dead_code)]
#![allow(unused_imports)]

// Tests are stored in separate tests module
#[cfg(tests)]
mod tests;

#[cfg_attr(debug_assertions, allow(dead_code, unused_imports))]
// Standard library dependencies
use std::error::Error;
use std::fmt;

// Error handling
use thiserror::Error;

use easyfft::dyn_size::realfft::DynRealDft;
// External dependencies
use ndarray::{ArrayBase, Axis, Ix1, OwnedRepr};

// load dependencies
pub mod background;
pub mod bessel_i0;
pub mod errors;
pub mod io;
pub mod lmutils;
pub mod mathutils;
pub mod normalization;
pub mod nshare;
pub mod xafsutils;
pub mod xasgroup;
pub mod xasparameters;
pub mod xasspectrum;
pub mod xrayfft;

// Load local traits
use mathutils::MathUtils;
use normalization::Normalization;
use xafsutils::XAFSUtils;

// Re-export error types for public API
pub use errors::{
    BackgroundError, DataError, FFTError, IOError, MathError, NormalizationError,
};

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

// Additional error conversions
impl From<Box<dyn std::error::Error>> for XAFSError {
    fn from(err: Box<dyn std::error::Error>) -> Self {
        XAFSError::Math(MathError::PolyfitFailed {
            reason: err.to_string(),
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
