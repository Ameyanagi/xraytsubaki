use std::cmp::Ordering;
use std::f64::consts::PI;

use easyfft::dyn_size::realfft::DynRealDft;
use easyfft::num_complex::Complex as FftComplex;
use nalgebra::DVector;
use num_complex::Complex64;

use crate::xafs::xafsutils::ftwindow;
use crate::xafs::xrayfft::{xftf_fast_nalgebra, xftr_fast_nalgebra};

use super::errors::FittingError;
use super::types::{FeffFitDataset, FeffFitTransform, FitSpace};

/// Maximum R of the reported chi(R) arrays (Larch `rmax_out`).
const RMAX_OUT: f64 = 10.0;
/// High-R range used by Larch's `estimate_noise`.
const NOISE_RMIN: f64 = 15.0;
const NOISE_RMAX: f64 = 30.0;

#[derive(Debug, Clone)]
pub struct TransformOutput {
    pub r: DVector<f64>,
    pub kwin: DVector<f64>,
    pub chir: Vec<Complex64>,
    pub chir_re: DVector<f64>,
    pub chir_im: DVector<f64>,
    pub chir_mag: DVector<f64>,
    pub mask_indices: Vec<usize>,
}

/// Transformed arrays of one spectrum for a single k-weight.
#[derive(Debug, Clone)]
pub struct KweightTransform {
    pub kweight: f64,
    /// `k^w * chi(k)` without window (Larch k-space residual input).
    pub chik: DVector<f64>,
    /// chi(R) arrays; `chir` is R-windowed for the fit, the rest are full for plotting.
    pub r_space: TransformOutput,
    /// Real part of chi(q) on the `i * kstep` grid (Larch `xftr_fast` convention), `nfft/2` long.
    pub chiq: DVector<f64>,
    /// Indices of `k`/`chik` inside Larch's `[iqmin, iqmax)` k-range selection.
    pub k_mask: Vec<usize>,
    /// Indices of `chiq` inside the same k-range selection.
    pub q_mask: Vec<usize>,
}

impl KweightTransform {
    /// Number of residual points this block contributes in `fitspace`.
    pub fn residual_len(&self, fitspace: FitSpace) -> usize {
        match fitspace {
            FitSpace::K => self.k_mask.len(),
            FitSpace::R => 2 * self.r_space.mask_indices.len(),
            FitSpace::Q => self.q_mask.len(),
        }
    }
}

/// Transformed arrays of one spectrum for every effective k-weight of a transform.
#[derive(Debug, Clone)]
pub struct DatasetTransform {
    pub blocks: Vec<KweightTransform>,
}

impl DatasetTransform {
    /// Block for the primary (first) k-weight.
    pub fn primary(&self) -> &KweightTransform {
        &self.blocks[0]
    }
}

/// Larch-style noise estimate from the high-R part of chi(R), one entry per k-weight.
#[derive(Debug, Clone, PartialEq)]
pub struct NoiseEstimate {
    pub epsilon_k: Vec<f64>,
    pub epsilon_r: Vec<f64>,
}

pub fn validate_transform(transform: &FeffFitTransform) -> Result<(), FittingError> {
    if transform.kmax.partial_cmp(&transform.kmin) != Some(Ordering::Greater) {
        return Err(FittingError::InvalidTransform {
            reason: format!(
                "kmax ({}) must be larger than kmin ({})",
                transform.kmax, transform.kmin
            ),
        });
    }
    if transform.rmax.partial_cmp(&transform.rmin) != Some(Ordering::Greater) {
        return Err(FittingError::InvalidTransform {
            reason: format!(
                "rmax ({}) must be larger than rmin ({})",
                transform.rmax, transform.rmin
            ),
        });
    }
    if transform.nfft < 16 {
        return Err(FittingError::InvalidTransform {
            reason: "nfft must be at least 16".to_string(),
        });
    }
    for kweight in transform.effective_kweights() {
        if !kweight.is_finite() || kweight < 0.0 {
            return Err(FittingError::InvalidTransform {
                reason: format!("kweight must be finite and non-negative, got {kweight}"),
            });
        }
    }
    if let Some(kstep) = transform.kstep {
        if kstep <= 0.0 {
            return Err(FittingError::InvalidTransform {
                reason: "kstep must be positive when provided".to_string(),
            });
        }
    }
    Ok(())
}

fn kstep_for(transform: &FeffFitTransform, k: &DVector<f64>) -> f64 {
    transform
        .kstep
        .unwrap_or_else(|| if k.len() > 1 { k[1] - k[0] } else { 0.05 })
        .max(1.0e-12)
}

fn window(
    x: &DVector<f64>,
    xmin: f64,
    xmax: f64,
    dx: f64,
    dx2: Option<f64>,
    kind: crate::xafs::xafsutils::FTWindow,
    label: &str,
) -> Result<DVector<f64>, FittingError> {
    ftwindow(
        x,
        Some(xmin),
        Some(xmax),
        Some(dx),
        dx2.or(Some(dx)),
        Some(kind),
    )
    .map_err(|error| FittingError::InvalidTransform {
        reason: format!("failed to create {label}-window: {error}"),
    })
}

/// Larch index bounds `[int(0.01 + kmin/kstep), min(nfft/2, int(0.01 + kmax/kstep)))`.
fn k_index_bounds(transform: &FeffFitTransform, kstep: f64) -> (usize, usize) {
    let iqmin = (0.01 + transform.kmin / kstep).max(0.0).floor() as usize;
    let iqmax = ((0.01 + transform.kmax / kstep).floor() as usize).min(transform.nfft / 2);
    (iqmin, iqmax.max(iqmin))
}

/// Full-length forward transform: returns `(kwin, chi(R) with nfft/2 + 1 bins)`.
fn forward_transform(
    k: &DVector<f64>,
    chi: &DVector<f64>,
    transform: &FeffFitTransform,
    kweight: f64,
    kstep: f64,
) -> Result<(DVector<f64>, Vec<Complex64>), FittingError> {
    let kwin = window(
        k,
        transform.kmin,
        transform.kmax,
        transform.dk,
        transform.dk2,
        transform.window,
        "k",
    )?;
    let weighted = DVector::from_iterator(
        k.len(),
        (0..k.len()).map(|i| chi[i] * k[i].powf(kweight) * kwin[i]),
    );
    let dft = xftf_fast_nalgebra(&weighted, transform.nfft, kstep);
    let chir = dft
        .iter()
        .map(|value| Complex64::new(value.re, value.im))
        .collect::<Vec<_>>();
    Ok((kwin, chir))
}

/// Back-transform of the R-windowed chi(R) following Larch's `xftr_fast` convention:
/// `Re[(4 sqrt(pi) / kstep) * ifft(chir * rwin)]` (only the lower half spectrum is used).
fn larch_chiq(chir: &[Complex64], rwin: &DVector<f64>, nfft: usize, kstep: f64) -> DVector<f64> {
    let half = nfft / 2;
    let zeroth = chir[0].re * rwin[0];
    let bins = (1..=half)
        .map(|j| {
            if j < half && j < chir.len() && j < rwin.len() {
                FftComplex::new(chir[j].re * rwin[j], chir[j].im * rwin[j])
            } else {
                FftComplex::new(0.0, 0.0)
            }
        })
        .collect::<Vec<_>>();
    let dft = DynRealDft::new(zeroth, &bins, nfft);
    let hermitian = xftr_fast_nalgebra(&dft, nfft, kstep);
    let correction = 2.0 * PI.sqrt() / kstep * zeroth / nfft as f64;
    DVector::from_iterator(half, (0..half).map(|n| 2.0 * hermitian[n] + correction))
}

/// Transform one spectrum for a single k-weight (k, R and q arrays).
pub fn apply_kweight_transform(
    k: &DVector<f64>,
    chi: &DVector<f64>,
    transform: &FeffFitTransform,
    kweight: f64,
) -> Result<KweightTransform, FittingError> {
    if k.len() != chi.len() {
        return Err(FittingError::InvalidDataset {
            reason: format!(
                "k/chi length mismatch for transform: {} vs {}",
                k.len(),
                chi.len()
            ),
        });
    }
    if k.len() < 2 {
        return Err(FittingError::InvalidDataset {
            reason: "transform requires at least 2 points".to_string(),
        });
    }

    let nfft = transform.nfft;
    let kstep = kstep_for(transform, k);
    let rstep = PI / (kstep * nfft as f64);
    let (kwin, chir_full) = forward_transform(k, chi, transform, kweight, kstep)?;

    let r_full = DVector::from_iterator(nfft / 2 + 1, (0..=nfft / 2).map(|j| j as f64 * rstep));
    let rwin_full = window(
        &r_full,
        transform.rmin,
        transform.rmax,
        transform.dr,
        transform.dr2,
        transform.rwindow,
        "R",
    )?;

    let irmax = (nfft / 2 + 1)
        .min((1.01 + RMAX_OUT / rstep) as usize)
        .max(1);
    let r = DVector::from_iterator(irmax, (0..irmax).map(|j| j as f64 * rstep));
    let chir_re = DVector::from_iterator(irmax, chir_full[..irmax].iter().map(|v| v.re));
    let chir_im = DVector::from_iterator(irmax, chir_full[..irmax].iter().map(|v| v.im));
    let chir_mag = DVector::from_iterator(irmax, chir_full[..irmax].iter().map(|v| v.norm()));
    // Keep full chi(R) for plotting/output while using an R-windowed copy for fitting residuals.
    let chir = (0..irmax)
        .map(|j| chir_full[j] * rwin_full[j])
        .collect::<Vec<_>>();

    let mask_indices = r
        .iter()
        .enumerate()
        .filter_map(|(index, rv)| (*rv >= transform.rmin && *rv <= transform.rmax).then_some(index))
        .collect::<Vec<_>>();
    if mask_indices.is_empty() {
        return Err(FittingError::InvalidTransform {
            reason: "configured R-range selects zero points".to_string(),
        });
    }

    let chiq = larch_chiq(&chir_full, &rwin_full, nfft, kstep);
    let (iqmin, iqmax) = k_index_bounds(transform, kstep);
    let k_lo = kstep * (iqmin as f64 - 1.0e-3);
    let k_hi = kstep * (iqmax as f64 - 1.0e-3);
    let k_mask = k
        .iter()
        .enumerate()
        .filter_map(|(index, kv)| (*kv > k_lo && *kv < k_hi).then_some(index))
        .collect::<Vec<_>>();
    let q_mask = (iqmin..iqmax.min(chiq.len())).collect::<Vec<_>>();
    let chik = DVector::from_iterator(k.len(), (0..k.len()).map(|i| chi[i] * k[i].powf(kweight)));

    Ok(KweightTransform {
        kweight,
        chik,
        r_space: TransformOutput {
            r,
            kwin,
            chir,
            chir_re,
            chir_im,
            chir_mag,
            mask_indices,
        },
        chiq,
        k_mask,
        q_mask,
    })
}

/// Transform one spectrum for every effective k-weight of `transform`.
pub fn apply_dataset_transform(
    k: &DVector<f64>,
    chi: &DVector<f64>,
    transform: &FeffFitTransform,
) -> Result<DatasetTransform, FittingError> {
    let blocks = transform
        .effective_kweights()
        .into_iter()
        .map(|kweight| apply_kweight_transform(k, chi, transform, kweight))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(DatasetTransform { blocks })
}

/// R-space transform for the primary k-weight (kept for backward compatibility).
pub fn apply_r_transform(
    k: &DVector<f64>,
    chi: &DVector<f64>,
    transform: &FeffFitTransform,
) -> Result<TransformOutput, FittingError> {
    apply_kweight_transform(k, chi, transform, transform.primary_kweight()).map(|out| out.r_space)
}

/// Per-k-weight `epsilon_k` for a dataset: `epsilon_ks[i]`, else `epsilon_k`, else 1.0.
pub fn resolve_epsilon_ks(dataset: &FeffFitDataset) -> Vec<f64> {
    dataset
        .transform
        .effective_kweights()
        .iter()
        .enumerate()
        .map(|(index, _)| {
            dataset
                .epsilon_ks
                .get(index)
                .copied()
                .or(dataset.epsilon_k)
                .unwrap_or(1.0)
                .max(1.0e-12)
        })
        .collect()
}

/// Larch `set_epsilon_k`: `eps_r = eps_k / (2 sqrt(pi w / (kstep (kmax^w - kmin^w))))`, `w = 2 kw + 1`.
pub fn epsilon_r_for_kweight(transform: &FeffFitTransform, kweight: f64, epsilon_k: f64) -> f64 {
    let eps_k = epsilon_k.max(1.0e-12);
    let kstep = transform.kstep.unwrap_or(0.05).max(1.0e-12);
    let w = 2.0 * kweight.max(0.0) + 1.0;
    let kspan = (transform.kmax.powf(w) - transform.kmin.powf(w)).max(1.0e-12);
    let scale = 2.0 * ((PI * w) / (kstep * kspan)).sqrt();
    (eps_k / scale).max(1.0e-12)
}

pub fn epsilon_r_from_epsilon_k(transform: &FeffFitTransform, epsilon_k: Option<f64>) -> f64 {
    epsilon_r_for_kweight(transform, transform.kweight, epsilon_k.unwrap_or(1.0))
}

fn push_block_residual(
    out: &mut Vec<f64>,
    data: &KweightTransform,
    model: Option<&KweightTransform>,
    transform: &FeffFitTransform,
    epsilon_k: f64,
) -> Result<(), FittingError> {
    let epsilon_r = epsilon_r_for_kweight(transform, data.kweight, epsilon_k);
    match transform.fitspace {
        FitSpace::K => {
            for &index in &data.k_mask {
                let model_value = model.map_or(0.0, |m| m.chik.get(index).copied().unwrap_or(0.0));
                out.push((data.chik[index] - model_value) / epsilon_k.max(1.0e-12));
            }
        }
        FitSpace::R => {
            if let Some(model) = model {
                if data.r_space.r.len() != model.r_space.r.len() {
                    return Err(FittingError::InvalidDataset {
                        reason: format!(
                            "data/model R grids differ: {} vs {}",
                            data.r_space.r.len(),
                            model.r_space.r.len()
                        ),
                    });
                }
            }
            for &index in &data.r_space.mask_indices {
                if index >= data.r_space.chir.len() {
                    continue;
                }
                let model_value = match model {
                    Some(m) => match m.r_space.chir.get(index) {
                        Some(value) => *value,
                        None => continue,
                    },
                    None => Complex64::new(0.0, 0.0),
                };
                let diff = data.r_space.chir[index] - model_value;
                out.push(diff.re / epsilon_r);
                out.push(diff.im / epsilon_r);
            }
        }
        FitSpace::Q => {
            for &index in &data.q_mask {
                let model_value = model.map_or(0.0, |m| m.chiq.get(index).copied().unwrap_or(0.0));
                out.push((data.chiq[index] - model_value) / epsilon_r);
            }
        }
    }
    Ok(())
}

/// Residual `transform(data) - transform(model)` concatenated over every k-weight block.
/// With `model = None` the transformed data alone is returned (Larch `data_only=True`).
pub fn residual_for_dataset(
    data: &DatasetTransform,
    model: Option<&DatasetTransform>,
    transform: &FeffFitTransform,
    epsilon_ks: &[f64],
) -> Result<DVector<f64>, FittingError> {
    if let Some(model) = model {
        if model.blocks.len() != data.blocks.len() {
            return Err(FittingError::InvalidDataset {
                reason: format!(
                    "data/model k-weight block count differs: {} vs {}",
                    data.blocks.len(),
                    model.blocks.len()
                ),
            });
        }
    }
    let mut residual = Vec::new();
    for (index, block) in data.blocks.iter().enumerate() {
        let epsilon_k = epsilon_ks
            .get(index)
            .copied()
            .or_else(|| epsilon_ks.first().copied())
            .unwrap_or(1.0);
        push_block_residual(
            &mut residual,
            block,
            model.map(|m| &m.blocks[index]),
            transform,
            epsilon_k,
        )?;
    }
    if residual.is_empty() {
        return Err(FittingError::InvalidTransform {
            reason: format!(
                "{:?}-space residual is empty after masking",
                transform.fitspace
            ),
        });
    }
    Ok(DVector::from_vec(residual))
}

pub fn residual_in_r_space(
    data: &TransformOutput,
    model: &TransformOutput,
    transform: &FeffFitTransform,
    epsilon_k: Option<f64>,
) -> Result<DVector<f64>, FittingError> {
    if data.r.len() != model.r.len() {
        return Err(FittingError::InvalidDataset {
            reason: format!(
                "data/model R grids differ: {} vs {}",
                data.r.len(),
                model.r.len()
            ),
        });
    }

    let sigma = epsilon_r_from_epsilon_k(transform, epsilon_k);
    let mut residual = Vec::with_capacity(data.mask_indices.len() * 2);

    for &index in data.mask_indices.iter() {
        if index >= model.chir.len() || index >= data.chir.len() {
            continue;
        }
        let diff = data.chir[index] - model.chir[index];
        residual.push(diff.re / sigma);
        residual.push(diff.im / sigma);
    }

    if residual.is_empty() {
        return Err(FittingError::InvalidTransform {
            reason: "R-space residual is empty after masking".to_string(),
        });
    }

    Ok(DVector::from_vec(residual))
}

pub fn data_residual_in_r_space(
    data: &TransformOutput,
    transform: &FeffFitTransform,
    epsilon_k: Option<f64>,
) -> Result<DVector<f64>, FittingError> {
    let sigma = epsilon_r_from_epsilon_k(transform, epsilon_k);
    let mut residual = Vec::with_capacity(data.mask_indices.len() * 2);

    for &index in data.mask_indices.iter() {
        if index >= data.chir.len() {
            continue;
        }
        let value = data.chir[index];
        residual.push(value.re / sigma);
        residual.push(value.im / sigma);
    }

    if residual.is_empty() {
        return Err(FittingError::InvalidTransform {
            reason: "R-space data residual is empty after masking".to_string(),
        });
    }

    Ok(DVector::from_vec(residual))
}

pub fn compute_n_idp(transform: &FeffFitTransform) -> f64 {
    1.0 + 2.0
        * (transform.rmax - transform.rmin).max(0.0)
        * (transform.kmax - transform.kmin).max(0.0)
        / PI
}

/// Larch `FeffitDataSet.estimate_noise`: estimate `epsilon_r` from the rms of chi(R) between
/// 15 and 30 Angstrom (scaled by the mean k-window value) and convert it to `epsilon_k` with
/// Parseval's theorem, for every effective k-weight.
pub fn estimate_noise(
    k: &DVector<f64>,
    chi: &DVector<f64>,
    transform: &FeffFitTransform,
) -> Result<NoiseEstimate, FittingError> {
    validate_transform(transform)?;
    let nfft = transform.nfft;
    let kstep = kstep_for(transform, k);
    let rstep = PI / (kstep * nfft as f64);
    let irmin = (0.01 + NOISE_RMIN / rstep) as usize;
    let irmax = ((1.01 + NOISE_RMAX / rstep) as usize).min(nfft / 2);
    if irmax <= irmin {
        return Err(FittingError::InvalidTransform {
            reason: "noise estimate range selects zero points; increase nfft".to_string(),
        });
    }

    let mut epsilon_k = Vec::new();
    let mut epsilon_r = Vec::new();
    for kweight in transform.effective_kweights() {
        let (kwin, chir) = forward_transform(k, chi, transform, kweight, kstep)?;
        let kwin_ave = kwin.sum() * kstep / (transform.kmax - transform.kmin);
        let n = (irmax - irmin).min(chir.len().saturating_sub(irmin));
        let sum_sq = chir[irmin..irmin + n]
            .iter()
            .map(|v| v.re * v.re + v.im * v.im)
            .sum::<f64>();
        let eps_r = (sum_sq / (2 * n).max(1) as f64).sqrt() / kwin_ave.max(1.0e-12);
        let w = 2.0 * kweight + 1.0;
        let scale =
            ((2.0 * PI * w) / (kstep * (transform.kmax.powf(w) - transform.kmin.powf(w)))).sqrt();
        epsilon_r.push(eps_r);
        epsilon_k.push(scale * eps_r);
    }
    Ok(NoiseEstimate {
        epsilon_k,
        epsilon_r,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xafs::xafsutils::FTWindow;

    fn synthetic() -> (DVector<f64>, DVector<f64>) {
        let k = DVector::from_iterator(400, (0..400).map(|i| 0.05 * i as f64));
        let chi = k.map(|kv| (2.0 * 2.5 * kv).sin() * (-0.01 * kv * kv).exp());
        (k, chi)
    }

    #[test]
    fn test_validate_transform_rejects_invalid_ranges() {
        let transform = FeffFitTransform {
            kmin: 10.0,
            kmax: 2.0,
            ..FeffFitTransform::default()
        };
        let err = validate_transform(&transform).unwrap_err();
        assert!(matches!(err, FittingError::InvalidTransform { .. }));
    }

    #[test]
    fn test_validate_transform_accepts_k_and_q_space_and_rejects_negative_kweights() {
        for fitspace in [FitSpace::K, FitSpace::Q, FitSpace::R] {
            let transform = FeffFitTransform {
                fitspace,
                kweights: vec![1.0, 2.0, 3.0],
                ..FeffFitTransform::default()
            };
            validate_transform(&transform).unwrap();
        }
        let transform = FeffFitTransform {
            kweights: vec![1.0, -2.0],
            ..FeffFitTransform::default()
        };
        assert!(validate_transform(&transform).is_err());
    }

    #[test]
    fn test_apply_r_transform_returns_masked_output() {
        let k = DVector::from_iterator(400, (0..400).map(|i| 0.05 * (i as f64 + 1.0)));
        let chi = k.map(|kv| (2.0 * kv).sin() * (-0.01 * kv * kv).exp());

        let transform = FeffFitTransform {
            kmin: 2.0,
            kmax: 14.0,
            window: FTWindow::Hanning,
            rmin: 1.0,
            rmax: 3.0,
            ..FeffFitTransform::default()
        };

        let out = apply_r_transform(&k, &chi, &transform).unwrap();
        assert!(!out.mask_indices.is_empty());
        assert_eq!(out.chir.len(), out.r.len());
    }

    #[test]
    fn test_apply_r_transform_preserves_full_chir_for_plotting() {
        let k = DVector::from_iterator(400, (0..400).map(|i| 0.05 * (i as f64 + 1.0)));
        let chi = k.map(|kv| (2.0 * kv).sin() * (-0.01 * kv * kv).exp());

        let transform = FeffFitTransform {
            kmin: 2.0,
            kmax: 14.0,
            window: FTWindow::Hanning,
            rmin: 1.0,
            rmax: 3.0,
            ..FeffFitTransform::default()
        };

        let out = apply_r_transform(&k, &chi, &transform).unwrap();
        let outside_max = out
            .r
            .iter()
            .enumerate()
            .filter(|(_, rv)| **rv > transform.rmax + 0.2)
            .map(|(idx, _)| out.chir_mag[idx].abs())
            .fold(0.0_f64, f64::max);

        assert!(
            outside_max > 1.0e-6,
            "full chi(R) should be retained outside fit range for plotting"
        );
    }

    #[test]
    fn test_compute_n_idp_positive() {
        let transform = FeffFitTransform {
            kmin: 2.0,
            kmax: 14.0,
            rmin: 1.0,
            rmax: 3.0,
            ..FeffFitTransform::default()
        };
        let n_idp = compute_n_idp(&transform);
        assert!(n_idp > 1.0);
    }

    #[test]
    fn test_epsilon_r_from_epsilon_k_matches_larch_formula() {
        let transform = FeffFitTransform {
            kmin: 3.0,
            kmax: 16.0,
            kweight: 2.0,
            kstep: Some(0.05),
            ..FeffFitTransform::default()
        };
        let eps_k = 0.0015509900716108595;
        let eps_r = epsilon_r_from_epsilon_k(&transform, Some(eps_k));
        assert!((eps_r - 0.04479749340858802).abs() < 1.0e-12);
        // Larch multi-kweight epsilon_r for kweight=(1,2,3) with the same epsilon_k.
        let eps_r1 = epsilon_r_for_kweight(&transform, 1.0, eps_k);
        let eps_r3 = epsilon_r_for_kweight(&transform, 3.0, eps_k);
        assert!((eps_r1 - 0.0036030667300897515).abs() < 1.0e-12);
        assert!((eps_r3 - 0.6058404104653774).abs() < 1.0e-12);
    }

    #[test]
    fn test_k_mask_follows_larch_index_rule() {
        let (k, chi) = synthetic();
        let transform = FeffFitTransform {
            kmin: 3.0,
            kmax: 16.0,
            fitspace: FitSpace::K,
            ..FeffFitTransform::default()
        };
        let out = apply_kweight_transform(&k, &chi, &transform, 2.0).unwrap();
        // Larch: [int(0.01 + 3/0.05), int(0.01 + 16/0.05)) = [60, 320) -> 260 points.
        assert_eq!(out.k_mask.len(), 260);
        assert_eq!(out.k_mask[0], 60);
        assert_eq!(out.q_mask.len(), 260);
        assert_eq!(out.residual_len(FitSpace::K), 260);
        assert_eq!(out.residual_len(FitSpace::Q), 260);
    }

    #[test]
    fn test_chiq_matches_larch_xftr_fast_convention() {
        // Larch: chiq = (4 sqrt(pi) / kstep) * ifft(chir * rwin padded to nfft)[:nfft/2], where
        // only the lower half spectrum is populated.  Compare against a direct DFT evaluation.
        let (k, chi) = synthetic();
        let transform = FeffFitTransform {
            kmin: 2.0,
            kmax: 15.0,
            dk: 1.0,
            rmin: 0.0,
            rmax: 3.0,
            dr: 0.5,
            rwindow: FTWindow::Hanning,
            nfft: 256,
            ..FeffFitTransform::default()
        };
        let nfft = transform.nfft;
        let kstep = 0.05;
        let rstep = PI / (kstep * nfft as f64);
        let (_, chir) = forward_transform(&k, &chi, &transform, 2.0, kstep).unwrap();
        let r_full = DVector::from_iterator(nfft / 2 + 1, (0..=nfft / 2).map(|j| j as f64 * rstep));
        let rwin = window(&r_full, 0.0, 3.0, 0.5, None, FTWindow::Hanning, "R").unwrap();
        let chiq = larch_chiq(&chir, &rwin, nfft, kstep);
        assert_eq!(chiq.len(), nfft / 2);

        for n in [0usize, 7, 40, 100, 127] {
            let mut acc = Complex64::new(0.0, 0.0);
            for j in 0..nfft / 2 {
                let phase = Complex64::new(0.0, 2.0 * PI * (j * n) as f64 / nfft as f64).exp();
                acc += chir[j] * rwin[j] * phase;
            }
            let expected = 4.0 * PI.sqrt() / kstep * acc.re / nfft as f64;
            assert!(
                (chiq[n] - expected).abs() < 1.0e-9 * expected.abs().max(1.0),
                "n={n}: {} vs {expected}",
                chiq[n]
            );
        }
        // For a flat window Larch's convention is twice the windowed k^w chi(k).
        let flat = DVector::from_element(nfft / 2 + 1, 1.0);
        let chiq_flat = larch_chiq(&chir, &flat, nfft, kstep);
        let (kwin, _) = forward_transform(&k, &chi, &transform, 2.0, kstep).unwrap();
        let weighted = |i: usize| chi[i] * k[i].powi(2) * kwin[i];
        let dc = 2.0 * (0..nfft).map(weighted).sum::<f64>() / nfft as f64;
        let nyquist = 2.0
            * (0..nfft)
                .map(|i| weighted(i) * if i % 2 == 0 { 1.0 } else { -1.0 })
                .sum::<f64>()
            / nfft as f64;
        let expected = 2.0 * weighted(60) + dc - nyquist;
        assert!(
            (chiq_flat[60] - expected).abs() < 1.0e-9,
            "{} vs {expected}",
            chiq_flat[60]
        );
    }

    #[test]
    fn test_multi_kweight_residual_concatenates_blocks() {
        let (k, chi) = synthetic();
        let transform = FeffFitTransform {
            kmin: 3.0,
            kmax: 16.0,
            rmin: 1.0,
            rmax: 3.0,
            kweights: vec![1.0, 2.0, 3.0],
            ..FeffFitTransform::default()
        };
        let data = apply_dataset_transform(&k, &chi, &transform).unwrap();
        assert_eq!(data.blocks.len(), 3);
        let single = apply_dataset_transform(
            &k,
            &chi,
            &FeffFitTransform {
                kweights: Vec::new(),
                kweight: 1.0,
                ..transform.clone()
            },
        )
        .unwrap();
        let residual = residual_for_dataset(&data, None, &transform, &[1.0]).unwrap();
        let residual_single = residual_for_dataset(&single, None, &transform, &[1.0]).unwrap();
        assert_eq!(residual.len(), 3 * residual_single.len());
        for i in 0..residual_single.len() {
            assert!((residual[i] - residual_single[i]).abs() < 1.0e-12);
        }
        let zero = residual_for_dataset(&data, Some(&data), &transform, &[1.0]).unwrap();
        assert!(zero.iter().all(|v| v.abs() < 1.0e-12));
    }

    #[test]
    fn test_estimate_noise_returns_one_entry_per_kweight() {
        let (k, chi) = synthetic();
        let transform = FeffFitTransform {
            kmin: 3.0,
            kmax: 16.0,
            kweights: vec![1.0, 2.0, 3.0],
            ..FeffFitTransform::default()
        };
        let noise = estimate_noise(&k, &chi, &transform).unwrap();
        assert_eq!(noise.epsilon_k.len(), 3);
        assert_eq!(noise.epsilon_r.len(), 3);
        assert!(noise.epsilon_k.iter().all(|v| v.is_finite() && *v >= 0.0));
    }
}
