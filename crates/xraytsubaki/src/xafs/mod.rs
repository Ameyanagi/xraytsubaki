//! EXAFS modules
//!
//!

#![allow(dead_code)]
#![allow(unused_imports)]

// Tests are stored in separate tests module
// Test constants and helpers are defined in the tests module at the end of this file

use pest::error;
use std::fmt;
#[cfg_attr(debug_assertions, allow(dead_code, unused_imports))]
// Standard library dependencies

// Error related dependencies
use thiserror::Error;

use easyfft::dyn_size::realfft::DynRealDft;
// External dependencies
use ndarray::{ArrayBase, Axis, Ix1, OwnedRepr};

// load dependencies
pub mod background;
pub mod bessel_i0;
pub mod fitting;
pub mod io;
pub mod lmutils;
pub mod mathutils;
pub mod multispectrum;
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

#[derive(Debug, Error)]
pub enum XAFSError {
    #[error("Not enought data")]
    NotEnoughData,
    #[error("Not enought data to perform XFTF")]
    NotEnoughDataForXFTF,
    #[error("Not enought data to perform XFTR")]
    NotEnoughDataForXFTR,
    #[error("Group index is out of range")]
    GroupIndexOutOfRange,
    #[error("The group is empty")]
    GroupIsEmpty,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

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
