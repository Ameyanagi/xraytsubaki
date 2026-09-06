use crate::xafs::errors::{DataError, NormalizationError};
use crate::{Result, Spectrum};
use serde::{Deserialize, Serialize};

/// Options for the default normalization → AUTOBK → Fourier pipeline.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct ProcessOptions {
    /// Edge energy in eV. `None` finds the edge from the input spectrum.
    pub e0: Option<f64>,
}

/// Owned arrays from a completed EXAFS processing pipeline.
///
/// `k` (Å⁻¹) pairs with unweighted `chi`. `r` (Å, not phase corrected)
/// pairs with all three Fourier components. The transform uses the core defaults
/// (including its k-weight); no normalization or weighting is applied twice.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessedSpectrum {
    /// Resolved edge energy, in eV.
    pub e0: f64,
    pub k: Vec<f64>,
    pub chi: Vec<f64>,
    pub r: Vec<f64>,
    pub chir_mag: Vec<f64>,
    pub chir_re: Vec<f64>,
    pub chir_im: Vec<f64>,
}

/// Process one spectrum with the default settings.
///
/// Inputs must be equal-length, finite arrays with strictly increasing energy
/// in eV. Invalid inputs return an error; they are never silently sorted.
pub fn process(energy: &[f64], mu: &[f64]) -> Result<ProcessedSpectrum> {
    process_with_options(energy, mu, ProcessOptions::default())
}

/// Process one spectrum, optionally overriding the edge energy in eV.
pub fn process_with_options(
    energy: &[f64],
    mu: &[f64],
    options: ProcessOptions,
) -> Result<ProcessedSpectrum> {
    let mut spectrum = Spectrum::from_arrays(energy, mu)?;
    if let Some(e0) = options.e0 {
        let data_min = energy[0];
        let data_max = energy[energy.len() - 1];
        if !e0.is_finite() || e0 <= data_min || e0 >= data_max {
            return Err(NormalizationError::E0OutOfRange {
                e0,
                data_min,
                data_max,
            }
            .into());
        }
        spectrum.set_e0(e0);
    } else {
        spectrum.find_e0()?;
    }
    spectrum.normalize()?.calc_background()?.fft()?;
    let required = |value: Option<nalgebra::DVector<f64>>, field: &str| {
        value.map(|v| v.as_slice().to_vec()).ok_or_else(|| {
            crate::Error::from(DataError::MissingData {
                field: field.into(),
            })
        })
    };
    Ok(ProcessedSpectrum {
        e0: spectrum
            .get_e0()
            .ok_or_else(|| DataError::MissingData { field: "e0".into() })?,
        k: required(spectrum.get_k(), "k")?,
        chi: required(spectrum.get_chi(), "chi")?,
        r: required(spectrum.get_r(), "r")?,
        chir_mag: required(spectrum.get_chir_mag(), "chir_mag")?,
        chir_re: required(spectrum.get_chir_real(), "chir_re")?,
        chir_im: required(spectrum.get_chir_imag(), "chir_im")?,
    })
}
