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

use easyfft::dyn_size::realfft::DynRealDft;
// External dependencies
use ndarray::{ArrayBase, Axis, Ix1, OwnedRepr};

// load dependencies
pub mod background;
pub mod bessel_i0;
pub mod io;
pub mod lmutils;
pub mod mathutils;
pub mod normalization;
pub mod nshare;
pub mod xafsutils;
pub mod xasgroup;
pub mod xasspectrum;
pub mod xrayfft;
// Load local traits
use mathutils::MathUtils;
use normalization::Normalization;
use xafsutils::XAFSUtils;

pub enum XAFSError {
    NotEnoughData,
    NotEnoughDataForXFTF,
    NotEnoughDataForXFTR,
    GroupIndexOutOfRange,
}

impl Error for XAFSError {
    fn description(&self) -> &str {
        match *self {
            XAFSError::NotEnoughData => "Not enough data",
            XAFSError::NotEnoughDataForXFTF => "Not enough data for XFTF",
            XAFSError::NotEnoughDataForXFTR => "Not enough data for XFTR",
            XAFSError::GroupIndexOutOfRange => "Group index out of range",
        }
    }

    fn cause(&self) -> Option<&dyn Error> {
        None
    }

    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use data_reader::reader::{load_txt_f64, Delimiter, ReaderParams};

    pub const TOP_DIR: &'static str = env!("CARGO_MANIFEST_DIR");
    pub const PARAM_LOADTXT: ReaderParams = ReaderParams {
        comments: Some(b'#'),
        delimiter: Delimiter::WhiteSpace,
        skip_footer: None,
        skip_header: None,
        usecols: None,
        max_rows: None,
        row_format: true,
    };
    pub const TEST_TOL: f64 = 1e-16;
}
