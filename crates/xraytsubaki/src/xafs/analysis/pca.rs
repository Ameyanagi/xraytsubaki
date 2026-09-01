//! Principal component analysis of a set of spectra (Athena's PCA tool,
//! Larch's `pca_train` / `pca_fit`).
//!
//! # Algorithm
//!
//! The spectra are taken in the chosen [`AnalysisSpace`], interpolated onto
//! the first spectrum's grid restricted to the fit range, and stacked into a
//! data matrix `D` (n_spectra × n_points). Optionally the mean spectrum is
//! subtracted (`PcaConfig::center`; **off** by default, like Athena, so that
//! a set of mixtures of *m* pure species has rank *m*; Larch's `pca_train`
//! centres). The thin SVD `D = U Σ Vᵀ` gives the components (rows of `Vᵀ`,
//! orthonormal), the eigenvalues `λᵢ = σᵢ² / n_spectra` of the covariance
//! matrix, the explained variance `σᵢ² / Σσ²` and the scores `U Σ`.
//!
//! The Malinowski indicator function for `k` retained components is
//!
//! ```text
//! IND(k) = sqrt( Σ_{j>k} λⱼ / (n_points · (n_spectra − k)) ) / (n_spectra − k)²
//! ```
//!
//! whose minimum estimates the number of significant components.
//!
//! A target transform ([`PcaModel::target_transform`]) projects a spectrum
//! onto the first `n` components (weights = `C · (y − mean)` since the
//! components are orthonormal) and reports the reconstruction quality.

use std::borrow::Borrow;

use nalgebra::{DMatrix, DVector, SVD};
use serde::{Deserialize, Serialize};

use super::{as_refs, on_grid, r_factor, reference_grid, spectrum_label, AnalysisSpace};
use crate::xafs::errors::AnalysisError;
use crate::xafs::xasspectrum::XASSpectrum;

/// Configuration of a PCA training.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PcaConfig {
    /// Array to analyse.
    pub space: AnalysisSpace,
    /// Range: relative to the first spectrum's E₀ for energy spaces,
    /// absolute k for `Chi`. `None` → Athena's defaults.
    pub range: Option<(f64, f64)>,
    /// Subtract the mean spectrum before the SVD (default `false`).
    pub center: bool,
}

impl Default for PcaConfig {
    fn default() -> Self {
        Self {
            space: AnalysisSpace::Norm,
            range: None,
            center: false,
        }
    }
}

/// Trained PCA model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PcaModel {
    pub space: AnalysisSpace,
    /// Whether `mean` was subtracted before the decomposition.
    pub centered: bool,
    /// Common grid (energy or k).
    pub x: DVector<f64>,
    /// Labels of the training spectra.
    pub labels: Vec<String>,
    /// Training data on the grid, one row per spectrum (n_spectra × n_points).
    pub data: DMatrix<f64>,
    /// Mean spectrum (zeros when not centred).
    pub mean: DVector<f64>,
    /// Orthonormal components, one row per component (n_components × n_points),
    /// ordered by decreasing eigenvalue.
    pub components: DMatrix<f64>,
    /// Eigenvalues of the covariance matrix, `σᵢ² / n_spectra`.
    pub eigenvalues: Vec<f64>,
    /// Fraction of the total variance carried by each component.
    pub variance_explained: Vec<f64>,
    /// Running sum of `variance_explained`.
    pub cumulative_variance: Vec<f64>,
    /// Malinowski indicator function; `ind[k]` is IND for `k` components,
    /// `k = 0 … n_spectra − 1`.
    pub ind: Vec<f64>,
    /// Scores (n_spectra × n_components): the training data expressed in the
    /// component basis, `data − mean = scores · components`.
    pub scores: DMatrix<f64>,
}

/// Reconstruction of a spectrum from a subset of components.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PcaFit {
    pub n_components: usize,
    pub x: DVector<f64>,
    /// Input spectrum on the model grid.
    pub data: DVector<f64>,
    /// `mean + Σ wᵢ Cᵢ`.
    pub fit: DVector<f64>,
    /// `data − fit`.
    pub residual: DVector<f64>,
    /// Projection weights on the retained components.
    pub weights: Vec<f64>,
    /// Σ residual².
    pub chi_square: f64,
    /// χ² / (n_points − n_components).
    pub reduced_chi_square: f64,
    /// Σ residual² / Σ data².
    pub r_factor: f64,
}

/// Train a PCA model on `spectra`.
pub fn pca_train<S: Borrow<XASSpectrum>>(
    spectra: &[S],
    cfg: &PcaConfig,
) -> Result<PcaModel, AnalysisError> {
    let refs = as_refs(spectra);
    let n = refs.len();
    if n < 2 {
        return Err(AnalysisError::InsufficientSpectra { min: 2, actual: n });
    }
    let (x, y0) = reference_grid(refs[0], cfg.space, cfg.range, 2)?;
    let p = x.len();

    let mut data = DMatrix::zeros(n, p);
    data.set_row(0, &y0.transpose());
    for (i, s) in refs.iter().enumerate().skip(1) {
        let y = on_grid(s, cfg.space, &x)?;
        data.set_row(i, &y.transpose());
    }
    let labels = refs
        .iter()
        .enumerate()
        .map(|(i, s)| spectrum_label(s, format!("spectrum{i}")))
        .collect();

    let mean = if cfg.center {
        DVector::from_fn(p, |j, _| data.column(j).sum() / n as f64)
    } else {
        DVector::zeros(p)
    };
    let mut centered = data.clone();
    if cfg.center {
        for i in 0..n {
            let row = centered.row(i) - mean.transpose();
            centered.set_row(i, &row);
        }
    }

    let svd = SVD::new(centered.clone(), true, true);
    let (u, v_t) = match (svd.u, svd.v_t) {
        (Some(u), Some(v_t)) => (u, v_t),
        _ => {
            return Err(AnalysisError::LinearAlgebra {
                reason: "SVD did not return singular vectors".to_string(),
            })
        }
    };
    let sigma = svd.singular_values;
    let n_comp = sigma.len();

    let sigma_sq: Vec<f64> = sigma.iter().map(|s| s * s).collect();
    let total: f64 = sigma_sq.iter().sum();
    let eigenvalues: Vec<f64> = sigma_sq.iter().map(|s| s / n as f64).collect();
    let variance_explained: Vec<f64> = sigma_sq
        .iter()
        .map(|s| if total > 0.0 { s / total } else { 0.0 })
        .collect();
    let cumulative_variance: Vec<f64> = variance_explained
        .iter()
        .scan(0.0, |acc, v| {
            *acc += v;
            Some(*acc)
        })
        .collect();

    // Scores = U Σ (n × n_comp).
    let mut scores = u.columns(0, n_comp).into_owned();
    for j in 0..n_comp {
        let mut col = scores.column_mut(j);
        col *= sigma[j];
    }

    let ind = malinowski_ind(&eigenvalues, n, p);

    Ok(PcaModel {
        space: cfg.space,
        centered: cfg.center,
        x,
        labels,
        data,
        mean,
        components: v_t.rows(0, n_comp).into_owned(),
        eigenvalues,
        variance_explained,
        cumulative_variance,
        ind,
        scores,
    })
}

/// Malinowski's IND for `k = 0 … n_spectra − 1` retained components.
fn malinowski_ind(eigenvalues: &[f64], n_spectra: usize, n_points: usize) -> Vec<f64> {
    let c = n_spectra;
    let r = n_points as f64;
    (0..c)
        .map(|k| {
            let tail: f64 = eigenvalues.iter().skip(k).sum();
            let remaining = (c - k) as f64;
            (tail / (r * remaining)).sqrt() / (remaining * remaining)
        })
        .collect()
}

impl PcaModel {
    pub fn n_spectra(&self) -> usize {
        self.data.nrows()
    }

    pub fn n_components(&self) -> usize {
        self.components.nrows()
    }

    /// Number of significant components suggested by the minimum of the
    /// Malinowski indicator function (at least 1).
    pub fn suggested_components_ind(&self) -> usize {
        let mut best = 1;
        let mut best_val = f64::INFINITY;
        for (k, &v) in self.ind.iter().enumerate().skip(1) {
            if v.is_finite() && v < best_val {
                best_val = v;
                best = k;
            }
        }
        best.min(self.n_components().max(1))
    }

    /// Smallest number of components whose cumulative explained variance
    /// reaches `threshold` (e.g. `0.999`).
    pub fn suggested_components_variance(&self, threshold: f64) -> usize {
        self.cumulative_variance
            .iter()
            .position(|&v| v >= threshold)
            .map(|i| i + 1)
            .unwrap_or(self.n_components())
    }

    /// Reconstruct a spectrum given on the model grid from its first
    /// `n_components` components.
    pub fn reconstruct(
        &self,
        y: &DVector<f64>,
        n_components: usize,
    ) -> Result<PcaFit, AnalysisError> {
        if n_components > self.n_components() {
            return Err(AnalysisError::TooManyComponents {
                requested: n_components,
                available: self.n_components(),
            });
        }
        if y.len() != self.x.len() {
            return Err(AnalysisError::LinearAlgebra {
                reason: format!(
                    "spectrum has {} points, model grid has {}",
                    y.len(),
                    self.x.len()
                ),
            });
        }
        let comps = self.components.rows(0, n_components);
        let centered = y - &self.mean;
        let w = comps * &centered;
        let fit = &self.mean + comps.transpose() * &w;
        let residual = y - &fit;
        let chi_square: f64 = residual.iter().map(|r| r * r).sum();
        let dof = y.len().saturating_sub(n_components).max(1) as f64;
        Ok(PcaFit {
            n_components,
            x: self.x.clone(),
            r_factor: r_factor(y, &fit),
            data: y.clone(),
            fit,
            residual,
            weights: w.iter().copied().collect(),
            chi_square,
            reduced_chi_square: chi_square / dof,
        })
    }

    /// Reconstruct training spectrum `index` from its first `n_components` components.
    pub fn reconstruct_training(
        &self,
        index: usize,
        n_components: usize,
    ) -> Result<PcaFit, AnalysisError> {
        if index >= self.n_spectra() {
            return Err(crate::xafs::errors::DataError::IndexOutOfRange {
                index,
                length: self.n_spectra(),
            }
            .into());
        }
        let y = self.data.row(index).transpose();
        self.reconstruct(&y, n_components)
    }

    /// Target transform: project `spectrum` (interpolated onto the model grid)
    /// onto the first `n_components` components and report the fit quality.
    pub fn target_transform(
        &self,
        spectrum: &XASSpectrum,
        n_components: usize,
    ) -> Result<PcaFit, AnalysisError> {
        let y = on_grid(spectrum, self.space, &self.x)?;
        self.reconstruct(&y, n_components)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ind_minimum_at_true_rank() {
        // Two large eigenvalues, three noise-level ones.
        let eig = [10.0, 1.0, 1e-4, 1e-4, 1e-4];
        let ind = malinowski_ind(&eig, 5, 200);
        let argmin = ind
            .iter()
            .enumerate()
            .skip(1)
            .min_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .unwrap()
            .0;
        assert_eq!(argmin, 2);
    }
}
