use pyo3::prelude::*;
use pyo3::exceptions::PyValueError;
use xraytsubaki::xafs::xafsutils;

/// Converts a string to a FTWindow enum value
/// 
/// This is a helper function used by both the functions and builders modules
pub fn str_to_window(window_str: &str) -> Result<xafsutils::FTWindow, PyErr> {
    match window_str.to_lowercase().as_str() {
        "hanning" => Ok(xafsutils::FTWindow::Hanning),
        "sine" => Ok(xafsutils::FTWindow::Sine),
        "kaiser-bessel" | "kaiserbessel" => Ok(xafsutils::FTWindow::KaiserBessel),
        "gaussian" => Ok(xafsutils::FTWindow::Gaussian),
        "parzen" => Ok(xafsutils::FTWindow::Parzen),
        "welch" => Ok(xafsutils::FTWindow::Welch),
        _ => Err(PyValueError::new_err(format!("Unknown window type: {}", window_str))),
    }
}

// You can add more utility functions here as needed, such as error handling helpers,
// data conversion utilities, or other common functionality shared across modules