//! Shared inverse-FFT validation and real-spectrum resizing for both array backends.
use super::{errors::FFTError, xrayfft::XrayFFTR};
use easyfft::{dyn_size::realfft::DynRealDft, num_complex::Complex, prelude::DynRealIfft};

pub(super) fn validate(r: &[f64], settings: &XrayFFTR) -> Result<(), FFTError> {
    let invalid = |parameter: &str, reason: &str| FFTError::InvalidParameter {
        parameter: parameter.into(),
        reason: reason.into(),
    };
    if r.len() < 2 || r.iter().any(|v| !v.is_finite()) {
        return Err(invalid(
            "r",
            "requires at least two finite, uniformly spaced values starting at zero",
        ));
    }
    let step = r[1] - r[0];
    let tolerance = 1e-8 * step.abs().max(1.0);
    if step <= 0.0
        || r[0].abs() > tolerance
        || r.windows(2)
            .any(|pair| ((pair[1] - pair[0]) - step).abs() > tolerance)
    {
        return Err(invalid(
            "r",
            "must be a uniformly increasing grid starting at zero",
        ));
    }
    if settings.nfft.is_some_and(|n| n < 2) {
        return Err(invalid("nfft", "must be at least 2"));
    }
    for (name, value, positive) in [
        ("kstep", settings.kstep, true),
        ("qmax_out", settings.qmax_out, false),
        ("rweight", settings.rweight, false),
        ("dr", settings.dr, false),
        ("dr2", settings.dr2, false),
        ("rmin", settings.rmin, false),
        ("rmax", settings.rmax, false),
    ] {
        if value.is_some_and(|v| !v.is_finite() || v < 0.0 || (positive && v == 0.0)) {
            return Err(invalid(
                name,
                "must be finite and nonnegative (kstep must be positive)",
            ));
        }
    }
    if settings
        .rmin
        .zip(settings.rmax)
        .is_some_and(|(lo, hi)| lo >= hi)
    {
        return Err(invalid("rmin/rmax", "requires rmin < rmax"));
    }
    if let Some(kstep) = settings.kstep {
        let implied = std::f64::consts::PI / (kstep * settings.nfft.unwrap_or(2048) as f64);
        if (implied - step).abs() > tolerance {
            return Err(invalid(
                "kstep/nfft",
                "must match the input R-grid spacing; leave kstep on Auto when changing nfft",
            ));
        }
    }
    Ok(())
}

pub(super) fn q_grid(maximum: f64, step: f64, available: usize) -> Vec<f64> {
    let length = (maximum / step)
        .floor()
        .min(available.saturating_sub(1) as f64) as usize
        + 1;
    (0..length).map(|i| i as f64 * step).collect()
}

/// Rebuild a real DFT for the requested output size. Include DC and every
/// positive-frequency bin; the easyfft frequency-bin accessor excludes endpoints.
/// A new even-length transform's Nyquist bin must be real.
pub(super) fn inverse(chir: &DynRealDft<f64>, nfft: usize, kstep: f64) -> Vec<f64> {
    let mut bins = vec![Complex::new(0.0, 0.0); nfft / 2];
    for (destination, source) in bins.iter_mut().zip(chir.iter().skip(1)) {
        *destination = *source;
    }
    if let Some(nyquist) = bins.last_mut().filter(|_| nfft.is_multiple_of(2)) {
        nyquist.im = 0.0;
    }
    let resized = DynRealDft::new(*chir.get_offset(), &bins, nfft);
    let scale = std::f64::consts::PI.sqrt() / kstep / nfft as f64;
    resized
        .real_ifft()
        .iter()
        .map(|value| value * scale)
        .collect()
}
