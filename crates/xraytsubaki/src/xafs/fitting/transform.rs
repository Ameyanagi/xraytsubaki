use nalgebra::DVector;
use num_complex::Complex64;

use crate::xafs::xrayfft::XrayFFTF;

use super::errors::FittingError;
use super::types::{FeffFitTransform, FitSpace};

#[derive(Debug, Clone)]
pub struct TransformOutput {
    pub r: DVector<f64>,
    pub chir: Vec<Complex64>,
    pub chir_re: DVector<f64>,
    pub chir_im: DVector<f64>,
    pub chir_mag: DVector<f64>,
    pub mask_indices: Vec<usize>,
}

pub fn validate_transform(transform: &FeffFitTransform) -> Result<(), FittingError> {
    if transform.fitspace != FitSpace::R {
        return Err(FittingError::InvalidTransform {
            reason: "MVP only supports R-space fitting".to_string(),
        });
    }
    if !(transform.kmax > transform.kmin) {
        return Err(FittingError::InvalidTransform {
            reason: format!(
                "kmax ({}) must be larger than kmin ({})",
                transform.kmax, transform.kmin
            ),
        });
    }
    if !(transform.rmax > transform.rmin) {
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
    if transform.kweight < 0.0 {
        return Err(FittingError::InvalidTransform {
            reason: "kweight must be non-negative".to_string(),
        });
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

pub fn apply_r_transform(
    k: &DVector<f64>,
    chi: &DVector<f64>,
    transform: &FeffFitTransform,
) -> Result<TransformOutput, FittingError> {
    if k.len() != chi.len() {
        return Err(FittingError::InvalidDataset {
            reason: format!(
                "k/chi length mismatch for transform: {} vs {}",
                k.len(),
                chi.len()
            ),
        });
    }

    let mut fft = XrayFFTF::new();
    fft.kmin = Some(transform.kmin);
    fft.kmax = Some(transform.kmax);
    fft.kweight = Some(transform.kweight);
    fft.dk = Some(transform.dk);
    fft.dk2 = transform.dk2;
    fft.window = Some(transform.window);
    fft.nfft = Some(transform.nfft);
    fft.kstep = transform.kstep;

    fft.xftf(k, chi)
        .map_err(|error| FittingError::InvalidTransform {
            reason: error.to_string(),
        })?;

    let r = fft
        .get_r()
        .cloned()
        .ok_or_else(|| FittingError::InvalidTransform {
            reason: "missing r grid after forward transform".to_string(),
        })?;

    let chir_dft = fft
        .get_chir()
        .ok_or_else(|| FittingError::InvalidTransform {
            reason: "missing chi(R) after forward transform".to_string(),
        })?;

    let chir_raw: Vec<Complex64> = chir_dft
        .iter()
        .take(r.len())
        .map(|value| Complex64::new(value.re, value.im))
        .collect();

    let mut chir_re = DVector::from_iterator(chir_raw.len(), chir_raw.iter().map(|value| value.re));
    let mut chir_im = DVector::from_iterator(chir_raw.len(), chir_raw.iter().map(|value| value.im));

    let r_window = crate::xafs::xafsutils::ftwindow(
        &r,
        Some(transform.rmin),
        Some(transform.rmax),
        Some(transform.dr),
        transform.dr2,
        Some(transform.rwindow),
    )
    .map_err(|error| FittingError::InvalidTransform {
        reason: format!("failed to create R-window: {error}"),
    })?;

    for i in 0..chir_re.len() {
        chir_re[i] *= r_window[i];
        chir_im[i] *= r_window[i];
    }

    let chir = chir_re
        .iter()
        .zip(chir_im.iter())
        .map(|(re, im)| Complex64::new(*re, *im))
        .collect::<Vec<_>>();
    let chir_mag = DVector::from_iterator(chir.len(), chir.iter().map(|value| value.norm()));

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

    Ok(TransformOutput {
        r,
        chir,
        chir_re,
        chir_im,
        chir_mag,
        mask_indices,
    })
}

pub fn residual_in_r_space(
    data: &TransformOutput,
    model: &TransformOutput,
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

    let sigma = epsilon_k.unwrap_or(1.0).max(1.0e-12);
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

pub fn compute_n_idp(transform: &FeffFitTransform) -> f64 {
    1.0 + 2.0
        * (transform.rmax - transform.rmin).max(0.0)
        * (transform.kmax - transform.kmin).max(0.0)
        / std::f64::consts::PI
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xafs::xafsutils::FTWindow;

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
}
