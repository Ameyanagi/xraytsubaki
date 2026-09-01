//! Multi-spectrum analysis: linear combination fitting (LCF) and principal
//! component analysis (PCA) of XANES / EXAFS spectra.
//!
//! Both tools operate on one of the [`AnalysisSpace`]s of a spectrum
//! (normalized μ, flattened μ, dμ/dE of normalized μ, or k-weighted χ(k))
//! over a fit range, with every spectrum interpolated linearly onto the grid
//! of a reference spectrum (the unknown for LCF, the first spectrum for PCA).
//!
//! * [`lcf`] / [`lcf_combinatorial`] — Athena / Larch style linear combination
//!   fitting with bounded weights, optional sum-to-one constraint and optional
//!   per-standard energy shifts.
//! * [`pca_train`] / [`PcaModel`] — SVD based principal component analysis,
//!   Malinowski's indicator function and target transformation.

pub mod lcf;
pub mod pca;

use std::borrow::Borrow;

use nalgebra::DVector;
use serde::{Deserialize, Serialize};

use super::errors::AnalysisError;
use super::normalization::Normalization;
use super::tools::{dmude, interp_linear};
use super::xasspectrum::XASSpectrum;

pub use lcf::{lcf, lcf_combinatorial, LcfComponent, LcfConfig, LcfResult};
pub use pca::{pca_train, PcaConfig, PcaFit, PcaModel};

/// Which array of a spectrum an analysis is performed on.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub enum AnalysisSpace {
    /// Normalized μ(E) (requires `normalize()`).
    #[default]
    Norm,
    /// Flattened normalized μ(E) (requires `normalize()`).
    Flat,
    /// Derivative dμ/dE of the normalized μ(E) (requires `normalize()`).
    Deriv,
    /// k-weighted χ(k)·k^kweight on the k grid (requires `calc_background()`).
    Chi { kweight: f64 },
}

/// Alias used by the LCF API.
pub type LcfSpace = AnalysisSpace;

impl AnalysisSpace {
    /// Whether the x axis of this space is k (Å⁻¹) rather than energy (eV).
    pub fn is_k_space(&self) -> bool {
        matches!(self, Self::Chi { .. })
    }

    /// Athena's default fit range: −20…+30 eV relative to E₀ for energy
    /// spaces, 3…12 Å⁻¹ for χ(k).
    pub fn default_range(&self) -> (f64, f64) {
        if self.is_k_space() {
            (3.0, 12.0)
        } else {
            (-20.0, 30.0)
        }
    }

    /// Arrays `(x, y)` of `spectrum` in this space.
    pub fn arrays(
        &self,
        spectrum: &XASSpectrum,
    ) -> Result<(DVector<f64>, DVector<f64>), AnalysisError> {
        let missing = |field: &str| AnalysisError::MissingArray {
            field: field.to_string(),
        };
        match self {
            Self::Norm | Self::Flat | Self::Deriv => {
                let energy = spectrum.energy.clone().ok_or_else(|| missing("energy"))?;
                let y = match self {
                    Self::Flat => spectrum
                        .get_flat()
                        .ok_or_else(|| missing("flat (run normalize() first)"))?,
                    _ => spectrum
                        .get_norm()
                        .ok_or_else(|| missing("norm (run normalize() first)"))?,
                };
                if energy.len() != y.len() {
                    return Err(super::errors::DataError::LengthMismatch {
                        energy_len: energy.len(),
                        mu_len: y.len(),
                    }
                    .into());
                }
                let y = if matches!(self, Self::Deriv) {
                    dmude(&energy, &y)
                } else {
                    y
                };
                Ok((energy, y))
            }
            Self::Chi { kweight } => {
                let k = spectrum
                    .get_k()
                    .ok_or_else(|| missing("k (run calc_background() first)"))?;
                let chi = spectrum
                    .get_chi()
                    .ok_or_else(|| missing("chi (run calc_background() first)"))?;
                if k.len() != chi.len() {
                    return Err(super::errors::DataError::LengthMismatch {
                        energy_len: k.len(),
                        mu_len: chi.len(),
                    }
                    .into());
                }
                let y = DVector::from_fn(k.len(), |i, _| chi[i] * k[i].powf(*kweight));
                Ok((k, y))
            }
        }
    }
}

/// E₀ of a spectrum: the explicit `e0`, else the one found during normalization.
pub(crate) fn spectrum_e0(spectrum: &XASSpectrum) -> Option<f64> {
    spectrum
        .e0
        .or_else(|| spectrum.normalization.as_ref().and_then(|n| n.get_e0()))
}

/// Name of a spectrum, or `fallback` if it has none.
pub(crate) fn spectrum_label(spectrum: &XASSpectrum, fallback: String) -> String {
    spectrum.name.clone().unwrap_or(fallback)
}

/// Build the analysis grid of `reference` in `space`, restricted to `range`
/// (relative to E₀ for energy spaces, absolute k for χ; `None` → Athena's
/// defaults). Returns `(x, y)` of the reference on that grid.
pub(crate) fn reference_grid(
    reference: &XASSpectrum,
    space: AnalysisSpace,
    range: Option<(f64, f64)>,
    min_points: usize,
) -> Result<(DVector<f64>, DVector<f64>), AnalysisError> {
    let (x, y) = space.arrays(reference)?;
    let (lo, hi) = range.unwrap_or_else(|| space.default_range());
    let (lo, hi) = if space.is_k_space() {
        (lo, hi)
    } else {
        let e0 = spectrum_e0(reference).ok_or_else(|| AnalysisError::MissingArray {
            field: "e0 (run normalize() or set_e0() first)".to_string(),
        })?;
        (e0 + lo, e0 + hi)
    };
    let (lo, hi) = if lo <= hi { (lo, hi) } else { (hi, lo) };
    let idx: Vec<usize> = (0..x.len()).filter(|&i| x[i] >= lo && x[i] <= hi).collect();
    if idx.len() < min_points {
        return Err(AnalysisError::EmptyRange {
            lo,
            hi,
            n_points: idx.len(),
            min: min_points,
        });
    }
    let xs = DVector::from_iterator(idx.len(), idx.iter().map(|&i| x[i]));
    let ys = DVector::from_iterator(idx.len(), idx.iter().map(|&i| y[i]));
    Ok((xs, ys))
}

/// Arrays of `spectrum` in `space`, interpolated linearly onto `grid`
/// (clamped at the ends of the spectrum's own range).
pub(crate) fn on_grid(
    spectrum: &XASSpectrum,
    space: AnalysisSpace,
    grid: &DVector<f64>,
) -> Result<DVector<f64>, AnalysisError> {
    let (x, y) = space.arrays(spectrum)?;
    Ok(interp_linear(grid, &x, &y)?)
}

/// Athena's R-factor: Σ(data − fit)² / Σ data².
pub fn r_factor(data: &DVector<f64>, fit: &DVector<f64>) -> f64 {
    let num: f64 = data
        .iter()
        .zip(fit.iter())
        .map(|(d, f)| (d - f).powi(2))
        .sum();
    let den: f64 = data.iter().map(|d| d * d).sum();
    if den > 0.0 {
        num / den
    } else {
        f64::NAN
    }
}

/// Collect `&XASSpectrum` references from any slice of owned or borrowed spectra.
pub(crate) fn as_refs<S: Borrow<XASSpectrum>>(spectra: &[S]) -> Vec<&XASSpectrum> {
    spectra.iter().map(|s| s.borrow()).collect()
}
