//! Athena-style data-processing tools.
//!
//! This module collects the interactive "data processing" operations that
//! Athena (and Larch) offer on a single spectrum or a set of spectra:
//!
//! * energy shift / calibration / alignment,
//! * deglitching (point, range, margin),
//! * truncation,
//! * rebinning onto a standard three-region XAFS grid,
//! * smoothing of μ(E),
//! * merging several spectra into one (with a per-point standard deviation),
//! * difference spectra.
//!
//! The functions in this module are pure and operate on `DVector<f64>` /
//! slices so they can be reused from the GUI or from Python bindings. Thin
//! convenience methods on [`XASSpectrum`] (in `xasspectrum.rs`) forward to
//! them and take care of keeping `energy`/`mu`/`raw_energy`/`raw_mu`
//! consistent and of invalidating every derived result
//! (normalization outputs, background, χ(k), χ(R), χ(q)).
//!
//! # Semantics where Athena and Larch differ
//!
//! * Rebinning follows Athena's default region boundaries and steps
//!   (pre-edge 10 eV up to E0−30, XANES 0.5 eV up to E0+50, EXAFS 0.05 Å⁻¹)
//!   and Larch's grid construction (three segments, EXAFS uniform in k).
//!   [`RebinMethod::Boxcar`] averages the raw points falling in each bin;
//!   [`RebinMethod::Centroid`] also places the output energy at the centroid
//!   of the raw points in the bin (Larch's "centroid" is instead an
//!   energy-weighted mean of μ on the nominal grid).
//! * Alignment follows Larch's `energy_align`: the derivative dμ/dE of the
//!   spectrum is overlaid onto the reference derivative in a window relative
//!   to the reference E0, with a free scale factor. The optimum is found by a
//!   coarse/fine grid search plus parabolic refinement instead of
//!   Levenberg–Marquardt.
//! * Merging uses a weighted mean; the standard deviation stored on the
//!   merged spectrum is the (frequency-weighted) sample standard deviation,
//!   which for equal weights is the usual `sqrt(Σ(μᵢ−μ̄)²/(N−1))` used by Athena.

use nalgebra::DVector;
use serde::{Deserialize, Serialize};

use super::errors::{DataError, MathError};
use super::mathutils::{index_nearest_sorted, MathUtils};
use super::xafsutils::{self, ConvolveForm, XAFSUtils, TINY_ENERGY};
use super::xasspectrum::XASSpectrum;
use super::XAFSError;

// ---------------------------------------------------------------------------
// Small shared helpers
// ---------------------------------------------------------------------------

fn missing(field: &str) -> XAFSError {
    DataError::MissingData {
        field: field.to_string(),
    }
    .into()
}

fn check_pair(energy: &DVector<f64>, mu: &DVector<f64>, min: usize) -> Result<(), XAFSError> {
    if energy.len() != mu.len() {
        return Err(DataError::LengthMismatch {
            energy_len: energy.len(),
            mu_len: mu.len(),
        }
        .into());
    }
    if energy.len() < min {
        return Err(DataError::InsufficientData {
            min,
            actual: energy.len(),
        }
        .into());
    }
    Ok(())
}

/// Linear interpolation of `(x, y)` onto `xnew` (clamped at the ends).
pub fn interp_linear(
    xnew: &DVector<f64>,
    x: &DVector<f64>,
    y: &DVector<f64>,
) -> Result<DVector<f64>, XAFSError> {
    check_pair(x, y, 2)?;
    xnew.interpolate(x.as_slice(), y.as_slice()).map_err(|e| {
        XAFSError::Math(MathError::SplineEvalFailed {
            x: 0.0,
            reason: e.to_string(),
        })
    })
}

/// Numerical derivative dμ/dE using central differences.
pub fn dmude(energy: &DVector<f64>, mu: &DVector<f64>) -> DVector<f64> {
    let de = energy.gradient();
    let dm = mu.gradient();
    DVector::from_fn(mu.len(), |i, _| {
        if de[i].abs() > 1e-12 {
            dm[i] / de[i]
        } else {
            0.0
        }
    })
}

/// Typical energy step of a grid: the median of the positive point spacings.
pub fn energy_step(energy: &DVector<f64>) -> f64 {
    let mut diffs: Vec<f64> = energy
        .as_slice()
        .windows(2)
        .map(|w| w[1] - w[0])
        .filter(|d| *d > 0.0)
        .collect();
    if diffs.is_empty() {
        return TINY_ENERGY;
    }
    diffs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    diffs[diffs.len() / 2]
}

/// Smooth `y(x)` with the existing convolution smoother, returning nalgebra
/// vectors under both feature configurations.
fn smooth_vec(
    x: &DVector<f64>,
    y: &DVector<f64>,
    sigma: Option<f64>,
    gamma: Option<f64>,
    xstep: Option<f64>,
    form: ConvolveForm,
) -> Result<DVector<f64>, XAFSError> {
    #[cfg(feature = "ndarray-compat")]
    {
        let xa = ndarray::Array1::from_vec(x.as_slice().to_vec());
        let ya = ndarray::Array1::from_vec(y.as_slice().to_vec());
        let out = xafsutils::smooth(xa, ya, sigma, gamma, xstep, None, form)?;
        Ok(DVector::from_vec(out.to_vec()))
    }
    #[cfg(not(feature = "ndarray-compat"))]
    {
        Ok(xafsutils::smooth(x, y, sigma, gamma, xstep, None, form)?)
    }
}

/// Smooth μ(E) by convolution with a Lorentzian, Gaussian or Voigt profile
/// of width `sigma` (`gamma` = Lorentzian width of the Voigt; defaults to
/// `sigma`). Thin wrapper around [`xafsutils::smooth`].
pub fn smooth_mu(
    energy: &DVector<f64>,
    mu: &DVector<f64>,
    form: ConvolveForm,
    sigma: Option<f64>,
    gamma: Option<f64>,
) -> Result<DVector<f64>, XAFSError> {
    check_pair(energy, mu, 3)?;
    smooth_vec(energy, mu, sigma, gamma, None, form)
}

/// Remove the (sorted, deduplicated) `indices` from `v`.
pub fn remove_indices(v: &DVector<f64>, indices: &[usize]) -> DVector<f64> {
    let mut keep = vec![true; v.len()];
    for &i in indices {
        if i < keep.len() {
            keep[i] = false;
        }
    }
    DVector::from_iterator(
        keep.iter().filter(|k| **k).count(),
        v.iter()
            .zip(keep.iter())
            .filter(|(_, k)| **k)
            .map(|(x, _)| *x),
    )
}

/// Indices of the points of the sorted grid `energy` closest to each target
/// energy (sorted, deduplicated).
pub fn nearest_indices(energy: &DVector<f64>, targets: &[f64]) -> Vec<usize> {
    let mut idx: Vec<usize> = targets
        .iter()
        .filter_map(|t| index_nearest_sorted(energy.as_slice(), t).ok())
        .collect();
    idx.sort_unstable();
    idx.dedup();
    idx
}

/// Indices of the points of `energy` lying in `[lo, hi]` (inclusive).
pub fn indices_in_range(energy: &DVector<f64>, lo: f64, hi: f64) -> Vec<usize> {
    let (lo, hi) = if lo <= hi { (lo, hi) } else { (hi, lo) };
    energy
        .iter()
        .enumerate()
        .filter(|(_, e)| **e >= lo && **e <= hi)
        .map(|(i, _)| i)
        .collect()
}

/// Least-squares straight line `y = a + b x` through the given points.
fn linear_fit(x: &[f64], y: &[f64]) -> Option<(f64, f64)> {
    let n = x.len() as f64;
    if x.len() < 2 {
        return None;
    }
    let mx = x.iter().sum::<f64>() / n;
    let my = y.iter().sum::<f64>() / n;
    let sxx: f64 = x.iter().map(|v| (v - mx).powi(2)).sum();
    if sxx <= f64::EPSILON {
        return None;
    }
    let sxy: f64 = x.iter().zip(y).map(|(u, v)| (u - mx) * (v - my)).sum();
    let b = sxy / sxx;
    Some((my - b * mx, b))
}

// ---------------------------------------------------------------------------
// Edge features / calibration
// ---------------------------------------------------------------------------

/// Feature of the absorption edge used as the calibration reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum EdgeFeature {
    /// Maximum of the first derivative dμ/dE (Athena/Larch `find_e0`).
    #[default]
    DerivativeMax,
    /// Zero crossing of the (smoothed) second derivative nearest the
    /// derivative maximum, located by linear interpolation between grid points.
    SecondDerivativeZero,
    /// Energy at which the flattened, normalized μ(E) crosses 0.5.
    HalfStep,
}

/// Energy of the maximum of dμ/dE (wrapper around [`xafsutils::find_e0`]).
pub fn derivative_max_energy(energy: &DVector<f64>, mu: &DVector<f64>) -> Result<f64, XAFSError> {
    check_pair(energy, mu, 3)?;
    Ok(xafsutils::find_e0(energy, mu)?)
}

/// Energy of the zero crossing of the second derivative of μ(E) nearest the
/// derivative maximum.
///
/// The first derivative is smoothed with a Lorentzian of width `3·ΔE/2` in a
/// window of ±75 points around the derivative maximum (mirroring `find_e0`),
/// the second derivative is taken by central differences, and the sign change
/// closest to the derivative maximum is interpolated linearly.
pub fn second_derivative_zero_energy(
    energy: &DVector<f64>,
    mu: &DVector<f64>,
) -> Result<f64, XAFSError> {
    let e0 = derivative_max_energy(energy, mu)?;
    let ie0 = index_nearest_sorted(energy.as_slice(), &e0)?;
    let start = ie0.saturating_sub(75);
    let stop = (ie0 + 76).min(energy.len());
    if stop - start < 5 {
        return Ok(e0);
    }
    let en = DVector::from_vec(energy.as_slice()[start..stop].to_vec());
    let mu_w = DVector::from_vec(mu.as_slice()[start..stop].to_vec());
    let estep = energy_step(&en) / 2.0;
    let d1 = dmude(&en, &mu_w);
    let d1 = smooth_vec(
        &en,
        &d1,
        Some(3.0 * estep),
        None,
        Some(estep),
        ConvolveForm::Lorentzian,
    )
    .unwrap_or(d1);
    let d2 = dmude(&en, &d1);

    // Locate the derivative maximum on the smoothed curve close to e0.
    let ic = ie0 - start;
    let lo = ic.saturating_sub(5);
    let hi = (ic + 6).min(en.len());
    let im = (lo..hi)
        .max_by(|a, b| d1[*a].partial_cmp(&d1[*b]).unwrap())
        .unwrap_or(ic);

    // Nearest +→− sign change of the second derivative around the maximum.
    let mut best: Option<(usize, usize)> = None;
    for k in im.saturating_sub(4)..(im + 4).min(en.len() - 1) {
        if d2[k] >= 0.0 && d2[k + 1] < 0.0 {
            let dist = k.abs_diff(im);
            if best.is_none_or(|(_, d)| dist < d) {
                best = Some((k, dist));
            }
        }
    }
    match best {
        Some((k, _)) => {
            let frac = d2[k] / (d2[k] - d2[k + 1]);
            Ok(en[k] + frac * (en[k + 1] - en[k]))
        }
        None => Ok(en[im]),
    }
}

/// Energy at which the flattened normalized spectrum `flat` first crosses 0.5
/// on the rising edge around `e0`.
pub fn half_step_energy(
    energy: &DVector<f64>,
    flat: &DVector<f64>,
    e0: f64,
) -> Result<f64, XAFSError> {
    check_pair(energy, flat, 3)?;
    let n = energy.len();
    let i0 = index_nearest_sorted(energy.as_slice(), &e0)?;
    // Walk down from e0 until flat < 0.5, then up until flat >= 0.5.
    let mut i = i0;
    while i > 0 && flat[i] >= 0.5 {
        i -= 1;
    }
    while i + 1 < n && flat[i + 1] < 0.5 {
        i += 1;
    }
    if i + 1 >= n {
        return Err(DataError::MissingData {
            field: "half-step crossing of flattened mu".to_string(),
        }
        .into());
    }
    let (y0, y1) = (flat[i], flat[i + 1]);
    let frac = if (y1 - y0).abs() > f64::EPSILON {
        (0.5 - y0) / (y1 - y0)
    } else {
        0.0
    };
    Ok(energy[i] + frac.clamp(0.0, 1.0) * (energy[i + 1] - energy[i]))
}

// ---------------------------------------------------------------------------
// Alignment
// ---------------------------------------------------------------------------

/// Find the energy shift `s` to add to `e_dat` so that `y_dat(E + s)` best
/// overlays `y_ref(E)` (with a free multiplicative scale) over the reference
/// points in `[lo, hi]`.
///
/// A coarse grid search over `±search_range` in steps of `coarse_step`, a fine
/// search in steps of `coarse_step/10`, and a final parabolic refinement are
/// used. Returns the shift in eV.
#[allow(clippy::too_many_arguments)]
pub fn find_energy_shift(
    e_dat: &DVector<f64>,
    y_dat: &DVector<f64>,
    e_ref: &DVector<f64>,
    y_ref: &DVector<f64>,
    lo: f64,
    hi: f64,
    search_range: f64,
    coarse_step: f64,
) -> Result<f64, XAFSError> {
    check_pair(e_dat, y_dat, 3)?;
    check_pair(e_ref, y_ref, 3)?;
    let idx = indices_in_range(e_ref, lo, hi);
    if idx.len() < 3 {
        return Err(DataError::InsufficientData {
            min: 3,
            actual: idx.len(),
        }
        .into());
    }
    let ew = DVector::from_iterator(idx.len(), idx.iter().map(|&i| e_ref[i]));
    let yw = DVector::from_iterator(idx.len(), idx.iter().map(|&i| y_ref[i]));
    let yw_sq: f64 = yw.iter().map(|v| v * v).sum();

    let residual = |s: f64| -> f64 {
        let probe = ew.add_scalar(-s);
        let ys = match probe.interpolate(e_dat.as_slice(), y_dat.as_slice()) {
            Ok(v) => v,
            Err(_) => return f64::INFINITY,
        };
        let ss: f64 = ys.iter().map(|v| v * v).sum();
        if ss <= f64::EPSILON {
            return yw_sq;
        }
        let scale = ys.dot(&yw) / ss;
        ys.iter()
            .zip(yw.iter())
            .map(|(a, b)| (scale * a - b).powi(2))
            .sum()
    };

    let grid_min = |center: f64, half: f64, step: f64| -> f64 {
        let n = ((2.0 * half / step).round() as usize).max(1);
        (0..=n)
            .map(|i| center - half + i as f64 * step)
            .map(|s| (s, residual(s)))
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .map(|(s, _)| s)
            .unwrap_or(center)
    };

    let search_range = search_range.abs().max(coarse_step.abs());
    let coarse_step = coarse_step.abs().max(1e-4);
    let s0 = grid_min(0.0, search_range, coarse_step);
    let fine = coarse_step / 10.0;
    let s1 = grid_min(s0, coarse_step, fine);

    // Parabolic refinement through (s1 - fine, s1, s1 + fine).
    let (fa, fb, fc) = (residual(s1 - fine), residual(s1), residual(s1 + fine));
    let denom = fa - 2.0 * fb + fc;
    let shift = if denom.abs() > f64::EPSILON && fa.is_finite() && fc.is_finite() {
        let delta = 0.5 * fine * (fa - fc) / denom;
        if delta.abs() <= fine {
            s1 + delta
        } else {
            s1
        }
    } else {
        s1
    };
    Ok(shift)
}

// ---------------------------------------------------------------------------
// Deglitch
// ---------------------------------------------------------------------------

/// Athena's margin deglitch: fit a straight line to μ(E) over `[e_lo, e_hi]`
/// and return the indices of the points in that range lying more than
/// `upper_margin` above or `lower_margin` below the line (in μ units).
pub fn margin_outliers(
    energy: &DVector<f64>,
    mu: &DVector<f64>,
    e_lo: f64,
    e_hi: f64,
    upper_margin: f64,
    lower_margin: f64,
) -> Result<Vec<usize>, XAFSError> {
    check_pair(energy, mu, 2)?;
    let idx = indices_in_range(energy, e_lo, e_hi);
    if idx.len() < 2 {
        return Err(DataError::InsufficientData {
            min: 2,
            actual: idx.len(),
        }
        .into());
    }
    let xs: Vec<f64> = idx.iter().map(|&i| energy[i]).collect();
    let ys: Vec<f64> = idx.iter().map(|&i| mu[i]).collect();
    let (a, b) = linear_fit(&xs, &ys).ok_or_else(|| {
        XAFSError::Math(MathError::PolyfitFailed {
            reason: "degenerate energy range for margin deglitch".to_string(),
        })
    })?;
    let upper = upper_margin.abs();
    let lower = lower_margin.abs();
    Ok(idx
        .into_iter()
        .filter(|&i| {
            let r = mu[i] - (a + b * energy[i]);
            r > upper || r < -lower
        })
        .collect())
}

// ---------------------------------------------------------------------------
// Rebin
// ---------------------------------------------------------------------------

/// How the μ value of a rebinned point is formed from the raw points in its bin.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RebinMethod {
    /// Output energy on the nominal grid, μ = mean of the raw points in the bin.
    #[default]
    Boxcar,
    /// Output energy at the mean energy (centroid) of the raw points in the
    /// bin, μ = mean of the raw points in the bin.
    Centroid,
}

/// Configuration of the three-region rebinning grid (Athena defaults).
///
/// All boundaries are relative to `e0`. Pre-edge: `[E_min, pre_end)` in steps
/// of `pre_step`; XANES: `[pre_end, xanes_end)` in steps of `xanes_step`;
/// EXAFS: `[xanes_end, E_max)` uniform in k with `exafs_kstep`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RebinConfig {
    /// Edge energy. `None` → determined with `find_e0`.
    pub e0: Option<f64>,
    /// Pre-edge step (eV). Athena default 10.
    pub pre_step: f64,
    /// End of the pre-edge region relative to e0 (eV). Athena default −30.
    pub pre_end: f64,
    /// XANES step (eV). Athena default 0.5.
    pub xanes_step: f64,
    /// End of the XANES region relative to e0 (eV). Athena default +50.
    pub xanes_end: f64,
    /// EXAFS step in k (Å⁻¹). Athena default 0.05.
    pub exafs_kstep: f64,
    pub method: RebinMethod,
}

impl Default for RebinConfig {
    fn default() -> Self {
        Self {
            e0: None,
            pre_step: 10.0,
            pre_end: -30.0,
            xanes_step: 0.5,
            xanes_end: 50.0,
            exafs_kstep: 0.05,
            method: RebinMethod::Boxcar,
        }
    }
}

/// Result of [`rebin`].
#[derive(Debug, Clone, PartialEq)]
pub struct RebinOutput {
    pub energy: DVector<f64>,
    pub mu: DVector<f64>,
    /// Sample standard deviation of the raw points in each bin (0 when the
    /// bin holds fewer than two points).
    pub stddev: DVector<f64>,
    /// The e0 used to build the grid.
    pub e0: f64,
}

/// Build the nominal rebin grid (absolute energies) for the given data range.
pub fn rebin_grid(emin: f64, emax: f64, cfg: &RebinConfig, e0: f64) -> Vec<f64> {
    let rmin = emin - e0;
    let rmax = emax - e0;
    let pre_step = cfg.pre_step.abs().max(TINY_ENERGY);
    let xanes_step = cfg.xanes_step.abs().max(TINY_ENERGY);
    let kstep = cfg.exafs_kstep.abs().max(1e-4);

    // Region boundaries (relative to e0), clipped into the data range and
    // forced monotonic as in Larch. The pre-edge grid starts at the first
    // multiple of `pre_step` inside the data so that the grid points sit at
    // round offsets from e0.
    let pre1 = (pre_step * (rmin / pre_step).ceil()).max(rmin);
    let mut pre2 = cfg.pre_end.min(rmax);
    let mut exafs1 = cfg.xanes_end.min(rmax);
    if pre2 <= pre1 {
        pre2 = (pre1 + pre_step).min(rmax);
    }
    if exafs1 <= pre2 {
        exafs1 = (pre2 + xanes_step).min(rmax);
    }
    let exafs2 = rmax;

    let mut grid = Vec::new();
    for (start, stop, step, in_k) in [
        (pre1, pre2, pre_step, false),
        (pre2, exafs1, xanes_step, false),
        (exafs1, exafs2, kstep, true),
    ] {
        if stop <= start {
            continue;
        }
        let (a, b) = if in_k {
            (start.etok(), stop.etok())
        } else {
            (start, stop)
        };
        let tol = 1e-9 * step;
        let mut i = 0usize;
        loop {
            let v = a + i as f64 * step;
            if v >= b - tol {
                break;
            }
            let v = if in_k { v.ktoe() } else { v };
            grid.push(e0 + v);
            i += 1;
        }
    }
    grid.dedup_by(|b, a| (*b - *a).abs() < TINY_ENERGY);
    grid
}

/// Rebin `(energy, mu)` onto the three-region grid described by `cfg`.
///
/// Every raw point is assigned to exactly one bin (bin edges half-way between
/// neighbouring grid points). Bins containing raw points are averaged
/// ([`RebinMethod`]); bins without any raw point are filled by linear
/// interpolation of the raw data at the nominal grid energy.
pub fn rebin(
    energy: &DVector<f64>,
    mu: &DVector<f64>,
    cfg: &RebinConfig,
) -> Result<RebinOutput, XAFSError> {
    check_pair(energy, mu, 3)?;
    let e0 = match cfg.e0 {
        Some(e0) => e0,
        None => derivative_max_energy(energy, mu)?,
    };
    let grid = rebin_grid(energy.min(), energy.max(), cfg, e0);
    if grid.len() < 2 {
        return Err(DataError::InsufficientData {
            min: 2,
            actual: grid.len(),
        }
        .into());
    }

    let es = energy.as_slice();
    let n = grid.len();
    let mut out_e = Vec::with_capacity(n);
    let mut out_mu = Vec::with_capacity(n);
    let mut out_sd = Vec::with_capacity(n);
    let mut need_interp: Vec<usize> = Vec::new();

    for i in 0..n {
        let lower = if i == 0 {
            grid[0] - 0.5 * (grid[1] - grid[0])
        } else {
            0.5 * (grid[i - 1] + grid[i])
        };
        let upper = if i + 1 == n {
            grid[n - 1] + 0.5 * (grid[n - 1] - grid[n - 2])
        } else {
            0.5 * (grid[i] + grid[i + 1])
        };
        let j0 = es.partition_point(|e| *e < lower);
        let j1 = es.partition_point(|e| *e < upper);
        let count = j1.saturating_sub(j0);
        if count == 0 {
            need_interp.push(out_e.len());
            out_e.push(grid[i]);
            out_mu.push(f64::NAN);
            out_sd.push(0.0);
            continue;
        }
        let m = mu.as_slice()[j0..j1].iter().sum::<f64>() / count as f64;
        let sd = if count > 1 {
            (mu.as_slice()[j0..j1]
                .iter()
                .map(|v| (v - m).powi(2))
                .sum::<f64>()
                / (count as f64 - 1.0))
                .sqrt()
        } else {
            0.0
        };
        let e_out = match cfg.method {
            RebinMethod::Boxcar => grid[i],
            RebinMethod::Centroid => es[j0..j1].iter().sum::<f64>() / count as f64,
        };
        out_e.push(e_out);
        out_mu.push(m);
        out_sd.push(sd);
    }

    if !need_interp.is_empty() {
        let probe =
            DVector::from_iterator(need_interp.len(), need_interp.iter().map(|&i| out_e[i]));
        let filled = interp_linear(&probe, energy, mu)?;
        for (k, &i) in need_interp.iter().enumerate() {
            out_mu[i] = filled[k];
        }
    }

    Ok(RebinOutput {
        energy: DVector::from_vec(out_e),
        mu: DVector::from_vec(out_mu),
        stddev: DVector::from_vec(out_sd),
        e0,
    })
}

// ---------------------------------------------------------------------------
// Merge
// ---------------------------------------------------------------------------

/// Energy grid of a merged spectrum.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MergeGrid {
    /// Grid of the first (master) spectrum, as in Athena.
    #[default]
    First,
    /// Union of all member grids (duplicates within `TINY_ENERGY` removed).
    Union,
    /// Grid of the member with the smallest typical energy step.
    Finest,
}

/// Weighting of the members of a merge.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub enum MergeWeight {
    /// Plain average.
    #[default]
    Equal,
    /// Athena "importance" weights, one per member.
    Importance(Vec<f64>),
    /// Weight each member by `1/σ²`, where σ is the standard deviation of μ
    /// around a straight line fitted in the pre-edge noise region.
    NoiseInverse,
}

/// Configuration of [`merge_spectra`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MergeConfig {
    pub grid: MergeGrid,
    pub weight: MergeWeight,
    /// Pre-edge region relative to each member's e0 used to estimate noise
    /// for [`MergeWeight::NoiseInverse`].
    pub noise_region: (f64, f64),
}

impl Default for MergeConfig {
    fn default() -> Self {
        Self {
            grid: MergeGrid::First,
            weight: MergeWeight::Equal,
            noise_region: (-200.0, -30.0),
        }
    }
}

fn working_arrays(s: &XASSpectrum) -> Result<(&DVector<f64>, &DVector<f64>), XAFSError> {
    let energy = s.energy.as_ref().ok_or_else(|| missing("energy"))?;
    let mu = s.mu.as_ref().ok_or_else(|| missing("mu"))?;
    check_pair(energy, mu, 2)?;
    Ok((energy, mu))
}

/// Standard deviation of μ around a straight line fitted in the pre-edge
/// region `[e0 + region.0, e0 + region.1]`.
pub fn pre_edge_noise(
    energy: &DVector<f64>,
    mu: &DVector<f64>,
    e0: f64,
    region: (f64, f64),
) -> Result<f64, XAFSError> {
    let idx = indices_in_range(energy, e0 + region.0, e0 + region.1);
    if idx.len() < 3 {
        return Err(DataError::InsufficientData {
            min: 3,
            actual: idx.len(),
        }
        .into());
    }
    let xs: Vec<f64> = idx.iter().map(|&i| energy[i]).collect();
    let ys: Vec<f64> = idx.iter().map(|&i| mu[i]).collect();
    let (a, b) = linear_fit(&xs, &ys).ok_or_else(|| {
        XAFSError::Math(MathError::PolyfitFailed {
            reason: "degenerate pre-edge region".to_string(),
        })
    })?;
    let ss: f64 = xs
        .iter()
        .zip(ys.iter())
        .map(|(x, y)| (y - a - b * x).powi(2))
        .sum();
    Ok((ss / (idx.len() as f64 - 2.0)).sqrt())
}

/// Merge several spectra into one on a common grid.
///
/// Each member's μ(E) is interpolated linearly onto the grid selected by
/// `cfg.grid` (restricted to the energy range shared by all members), the
/// weighted mean is stored as `mu`, and the per-point standard deviation is
/// stored in `mu_stddev`. The merged spectrum inherits the stage
/// configurations of the first member with all outputs cleared.
pub fn merge_spectra(
    members: &[&XASSpectrum],
    cfg: &MergeConfig,
) -> Result<XASSpectrum, XAFSError> {
    if members.is_empty() {
        return Err(DataError::EmptyGroup.into());
    }
    let arrays = members
        .iter()
        .map(|s| working_arrays(s))
        .collect::<Result<Vec<_>, _>>()?;

    let lo = arrays
        .iter()
        .map(|(e, _)| e.min())
        .fold(f64::NEG_INFINITY, f64::max);
    let hi = arrays
        .iter()
        .map(|(e, _)| e.max())
        .fold(f64::INFINITY, f64::min);
    if lo.is_nan() || hi.is_nan() || lo >= hi {
        return Err(DataError::InvalidEnergyRange { min: lo, max: hi }.into());
    }

    let base: Vec<f64> = match cfg.grid {
        MergeGrid::First => arrays[0].0.as_slice().to_vec(),
        MergeGrid::Finest => {
            let (e, _) = arrays
                .iter()
                .min_by(|a, b| energy_step(a.0).partial_cmp(&energy_step(b.0)).unwrap())
                .unwrap();
            e.as_slice().to_vec()
        }
        MergeGrid::Union => {
            let mut all: Vec<f64> = arrays.iter().flat_map(|(e, _)| e.iter().copied()).collect();
            all.sort_by(|a, b| a.partial_cmp(b).unwrap());
            all.dedup_by(|b, a| (*b - *a).abs() < TINY_ENERGY);
            all
        }
    };
    let grid: Vec<f64> = base.into_iter().filter(|e| *e >= lo && *e <= hi).collect();
    if grid.len() < 2 {
        return Err(DataError::InsufficientData {
            min: 2,
            actual: grid.len(),
        }
        .into());
    }
    let grid = DVector::from_vec(grid);

    let weights: Vec<f64> = match &cfg.weight {
        MergeWeight::Equal => vec![1.0; members.len()],
        MergeWeight::Importance(w) => {
            if w.len() != members.len() {
                return Err(DataError::LengthMismatch {
                    energy_len: members.len(),
                    mu_len: w.len(),
                }
                .into());
            }
            w.clone()
        }
        MergeWeight::NoiseInverse => {
            let mut w = Vec::with_capacity(members.len());
            for (s, (e, m)) in members.iter().zip(arrays.iter()) {
                let e0 = match s.e0 {
                    Some(e0) => e0,
                    None => derivative_max_energy(e, m)?,
                };
                let sigma = pre_edge_noise(e, m, e0, cfg.noise_region)?.max(1e-12);
                w.push(1.0 / (sigma * sigma));
            }
            w
        }
    };
    let wsum: f64 = weights.iter().sum();
    if wsum.is_nan() || wsum <= 0.0 || weights.iter().any(|w| *w < 0.0 || !w.is_finite()) {
        return Err(DataError::MissingData {
            field: "positive merge weights".to_string(),
        }
        .into());
    }

    let ys = arrays
        .iter()
        .map(|(e, m)| interp_linear(&grid, e, m))
        .collect::<Result<Vec<_>, _>>()?;

    let n = members.len() as f64;
    let mean = DVector::from_fn(grid.len(), |i, _| {
        ys.iter().zip(&weights).map(|(y, w)| w * y[i]).sum::<f64>() / wsum
    });
    let stddev = if members.len() > 1 {
        DVector::from_fn(grid.len(), |i, _| {
            let ss: f64 = ys
                .iter()
                .zip(&weights)
                .map(|(y, w)| w * (y[i] - mean[i]).powi(2))
                .sum();
            (ss / (wsum * (n - 1.0) / n)).sqrt()
        })
    } else {
        DVector::zeros(grid.len())
    };

    let master = members[0];
    let mut merged = XASSpectrum::new();
    merged.set_name(format!("merge: {} spectra", members.len()));
    merged.set_spectrum(grid, mean);
    merged.mu_stddev = Some(stddev);
    merged.normalization = master.normalization.clone();
    merged.background = master.background.clone();
    merged.xftf = master.xftf.clone();
    merged.xftr = master.xftr.clone();
    merged.invalidate_derived();
    if let Some(norm) = merged.normalization.as_mut() {
        norm.set_e0(None);
    }
    if let Some(super::background::BackgroundMethod::AUTOBK(autobk)) = merged.background.as_mut() {
        autobk.ek0 = None;
    }
    Ok(merged)
}

// ---------------------------------------------------------------------------
// Difference
// ---------------------------------------------------------------------------

/// Which arrays a difference spectrum is formed from.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiffSpace {
    /// Raw μ(E).
    #[default]
    Mu,
    /// Normalized μ(E) (requires `normalize()` on both spectra).
    Norm,
    /// Flattened normalized μ(E) (requires `normalize()` on both spectra).
    Flat,
}

fn space_array(s: &XASSpectrum, space: DiffSpace) -> Result<DVector<f64>, XAFSError> {
    match space {
        DiffSpace::Mu => s.mu.clone().ok_or_else(|| missing("mu")),
        DiffSpace::Norm => s
            .get_norm()
            .ok_or_else(|| missing("norm (run normalize() first)")),
        DiffSpace::Flat => s
            .get_flat()
            .ok_or_else(|| missing("flat (run normalize() first)")),
    }
}

/// Difference spectrum `a − b` on the energy grid of `a`.
///
/// `b` is interpolated linearly onto `a`'s grid. The result carries the
/// difference in `mu`/`raw_mu` and is named `"<a> - <b>"`.
pub fn difference(
    a: &XASSpectrum,
    b: &XASSpectrum,
    space: DiffSpace,
) -> Result<XASSpectrum, XAFSError> {
    let ea = a.energy.as_ref().ok_or_else(|| missing("energy"))?;
    let eb = b.energy.as_ref().ok_or_else(|| missing("energy"))?;
    let ya = space_array(a, space)?;
    let yb = space_array(b, space)?;
    check_pair(ea, &ya, 2)?;
    check_pair(eb, &yb, 2)?;
    let yb_on_a = interp_linear(ea, eb, &yb)?;
    let diff = &ya - &yb_on_a;

    let name = format!(
        "{} - {}",
        a.name.as_deref().unwrap_or("a"),
        b.name.as_deref().unwrap_or("b")
    );
    let mut out = XASSpectrum::new();
    out.set_name(name);
    out.set_spectrum(ea.clone(), diff);
    out.e0 = a.e0;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xafs::io;
    use crate::xafs::tests::TOP_DIR;
    use crate::xafs::xasgroup::XASGroup;
    use approx::assert_abs_diff_eq;

    fn load_ru() -> XASSpectrum {
        let path = String::from(TOP_DIR) + "/tests/testfiles/Ru_QAS.dat";
        io::load_spectrum_QAS_trans(&path).unwrap()
    }

    fn lens(s: &XASSpectrum) -> (usize, usize, usize, usize) {
        (
            s.energy.as_ref().unwrap().len(),
            s.mu.as_ref().unwrap().len(),
            s.raw_energy.as_ref().unwrap().len(),
            s.raw_mu.as_ref().unwrap().len(),
        )
    }

    /// Deterministic pseudo-noise (LCG) so tests do not need a rand dependency.
    fn noise(n: usize, seed: u64, amplitude: f64) -> DVector<f64> {
        let mut state = seed;
        DVector::from_fn(n, |_, _| {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let u = ((state >> 11) as f64) / ((1u64 << 53) as f64);
            amplitude * (2.0 * u - 1.0)
        })
    }

    #[test]
    fn tools_shift_energy_moves_e0_by_shift() {
        let mut s = load_ru();
        s.find_e0().unwrap();
        let e0 = s.e0.unwrap();
        s.shift_energy(2.5);
        assert_abs_diff_eq!(s.energy_shift, 2.5, epsilon = 1e-12);
        assert_abs_diff_eq!(s.e0.unwrap(), e0 + 2.5, epsilon = 1e-12);
        s.find_e0().unwrap();
        assert_abs_diff_eq!(s.e0.unwrap(), e0 + 2.5, epsilon = 1e-9);
        assert!(s.get_norm().is_none());
    }

    #[test]
    fn tools_calibrate_derivative_max_lands_on_target() {
        let mut s = load_ru();
        let target = 22117.0;
        let shift = s.calibrate(EdgeFeature::DerivativeMax, target).unwrap();
        assert!(shift.abs() < 20.0);
        assert_abs_diff_eq!(s.e0.unwrap(), target, epsilon = 1e-12);
        let e = derivative_max_energy(s.energy.as_ref().unwrap(), s.mu.as_ref().unwrap()).unwrap();
        assert_abs_diff_eq!(e, target, epsilon = 0.05);
        assert_abs_diff_eq!(s.energy_shift, shift, epsilon = 1e-12);
    }

    #[test]
    fn tools_calibrate_other_features_are_near_e0() {
        let mut s = load_ru();
        s.find_e0().unwrap();
        let e0 = s.e0.unwrap();
        let mut a = s.clone();
        let sa = a.calibrate(EdgeFeature::SecondDerivativeZero, e0).unwrap();
        assert!(sa.abs() < 3.0, "second-derivative zero shift {sa}");
        let mut b = s.clone();
        let sb = b.calibrate(EdgeFeature::HalfStep, e0).unwrap();
        assert!(sb.abs() < 10.0, "half-step shift {sb}");
    }

    #[test]
    fn tools_align_recovers_applied_shift() {
        let reference = load_ru();
        let mut shifted = reference.clone();
        shifted.shift_energy(3.7);
        let shift = shifted.align_to(&reference, (-15.0, 35.0)).unwrap();
        assert_abs_diff_eq!(shift, -3.7, epsilon = 0.1);
        assert_abs_diff_eq!(shifted.energy_shift, 3.7 + shift, epsilon = 1e-12);
    }

    #[test]
    fn tools_deglitch_points_and_range() {
        let mut s = load_ru();
        let (n, ..) = lens(&s);
        let e = s.energy.as_ref().unwrap().clone();
        let targets = [e[10], e[11] + 0.01, e[300]];
        let removed = s.deglitch_points(&targets).unwrap();
        assert_eq!(removed, 3);
        let (n1, m1, r1, rm1) = lens(&s);
        assert_eq!((n1, m1, r1, rm1), (n - 3, n - 3, n - 3, n - 3));
        let e1 = s.energy.as_ref().unwrap();
        assert!(!e1
            .iter()
            .any(|v| *v == e[10] || *v == e[11] || *v == e[300]));

        let lo = e[400];
        let hi = e[410];
        let removed = s.deglitch_range(lo, hi).unwrap();
        assert_eq!(removed, 11);
        let (n2, m2, r2, rm2) = lens(&s);
        assert_eq!((n2, m2, r2, rm2), (n1 - 11, n1 - 11, n1 - 11, n1 - 11));
        assert!(!s
            .energy
            .as_ref()
            .unwrap()
            .iter()
            .any(|v| *v >= lo && *v <= hi));
    }

    #[test]
    fn tools_deglitch_margin_removes_spike() {
        let mut s = load_ru();
        s.find_e0().unwrap();
        let e0 = s.e0.unwrap();
        let (n, ..) = lens(&s);
        let idx = indices_in_range(s.energy.as_ref().unwrap(), e0 + 200.0, e0 + 600.0);
        let spike = idx[idx.len() / 2];
        let spike_e = s.energy.as_ref().unwrap()[spike];
        s.mu.as_mut().unwrap()[spike] += 0.5;
        s.raw_mu.as_mut().unwrap()[spike] += 0.5;
        let removed = s.deglitch_margin(e0 + 200.0, e0 + 600.0, 0.2, 0.2).unwrap();
        assert_eq!(removed, vec![spike_e]);
        assert_eq!(lens(&s), (n - 1, n - 1, n - 1, n - 1));
    }

    #[test]
    fn tools_truncate_bounds() {
        let mut s = load_ru();
        s.truncate(Some(22000.0), Some(22800.0)).unwrap();
        let e = s.energy.as_ref().unwrap();
        assert!(e.min() >= 22000.0 && e.max() <= 22800.0);
        let (n, m, r, rm) = lens(&s);
        assert_eq!((n, m), (r, rm));
        assert_eq!(n, m);
        assert!(s.truncate(Some(1.0), Some(2.0)).is_err());
    }

    #[test]
    fn tools_rebin_grid_and_agreement() {
        let mut orig = load_ru();
        orig.find_e0().unwrap();
        orig.normalize().unwrap();
        let e0 = orig.e0.unwrap();

        let mut reb = orig.clone();
        reb.rebin(&RebinConfig::default()).unwrap();
        assert!(reb.rebinned);
        assert!(reb.mu_stddev.is_some());
        let e = reb.energy.as_ref().unwrap().clone();
        assert_eq!(e.len(), reb.mu.as_ref().unwrap().len());
        assert!(e.as_slice().windows(2).all(|w| w[1] > w[0]));

        // Spacings: 10 eV in the pre-edge, 0.5 eV in the XANES, 0.05 Å⁻¹ above.
        let tol = 1e-9;
        for w in e.as_slice().windows(2) {
            let (r0, r1) = (w[0] - e0, w[1] - e0);
            let d = w[1] - w[0];
            if r1 <= -30.0 + tol {
                assert_abs_diff_eq!(d, 10.0, epsilon = 1e-6);
            } else if r0 >= -30.0 - tol && r1 <= 50.0 + tol {
                assert_abs_diff_eq!(d, 0.5, epsilon = 1e-6);
            } else if r0 >= 50.0 - tol {
                assert_abs_diff_eq!(r1.etok() - r0.etok(), 0.05, epsilon = 1e-6);
            }
        }

        // Normalized μ of the rebinned spectrum agrees with the original.
        reb.normalize().unwrap();
        let norm_reb = reb.get_norm().unwrap();
        let norm_orig = orig.get_norm().unwrap();
        let on_grid = interp_linear(&e, orig.energy.as_ref().unwrap(), &norm_orig).unwrap();
        let idx = indices_in_range(&e, e0 - 30.0, e0 + 50.0);
        let rms = (idx
            .iter()
            .map(|&i| (norm_reb[i] - on_grid[i]).powi(2))
            .sum::<f64>()
            / idx.len() as f64)
            .sqrt();
        assert!(rms < 0.01, "rms = {rms}");
    }

    #[test]
    fn tools_rebin_centroid_places_points_inside_bins() {
        let s = load_ru();
        let cfg = RebinConfig {
            method: RebinMethod::Centroid,
            ..Default::default()
        };
        let out = rebin(s.energy.as_ref().unwrap(), s.mu.as_ref().unwrap(), &cfg).unwrap();
        assert!(out.energy.as_slice().windows(2).all(|w| w[1] > w[0]));
        assert!(out.mu.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn tools_smooth_mu_reduces_noise() {
        let mut s = load_ru();
        let n = s.mu.as_ref().unwrap().len();
        let noisy = s.mu.as_ref().unwrap() + noise(n, 7, 0.02);
        let energy = s.energy.as_ref().unwrap().clone();
        s.set_spectrum(energy, noisy.clone());
        s.smooth_mu(ConvolveForm::Gaussian, Some(1.0), None)
            .unwrap();
        let sm = s.mu.as_ref().unwrap();
        assert_eq!(sm.len(), n);
        assert_eq!(s.raw_mu.as_ref().unwrap(), sm);
        let rough = |v: &DVector<f64>| v.diff().iter().map(|d| d * d).sum::<f64>();
        assert!(rough(sm) < rough(&noisy));
    }

    #[test]
    fn tools_merge_self_is_identity_with_zero_stddev() {
        let s = load_ru();
        let merged = merge_spectra(&[&s, &s], &MergeConfig::default()).unwrap();
        assert_eq!(merged.name.as_deref(), Some("merge: 2 spectra"));
        let mu = merged.mu.as_ref().unwrap();
        let orig = s.mu.as_ref().unwrap();
        assert_eq!(mu.len(), orig.len());
        for (a, b) in mu.iter().zip(orig.iter()) {
            assert_abs_diff_eq!(a, b, epsilon = 1e-12);
        }
        assert!(merged.mu_stddev.as_ref().unwrap().iter().all(|v| *v == 0.0));
    }

    #[test]
    fn tools_merge_noisy_copies_gives_mean_and_positive_stddev() {
        let base = load_ru();
        let n = base.mu.as_ref().unwrap().len();
        let e = base.energy.as_ref().unwrap().clone();
        let mut a = base.clone();
        let mut b = base.clone();
        let na = noise(n, 1, 0.01);
        let nb = noise(n, 2, 0.01);
        a.set_spectrum(e.clone(), base.mu.as_ref().unwrap() + &na);
        b.set_spectrum(e.clone(), base.mu.as_ref().unwrap() + &nb);

        let mut group = XASGroup::new();
        group.add_spectrum(a.clone()).add_spectrum(b.clone());
        let merged = group.merge(&[0, 1], &MergeConfig::default()).unwrap();
        let mu = merged.mu.as_ref().unwrap();
        let sd = merged.mu_stddev.as_ref().unwrap();
        let am = a.mu.as_ref().unwrap();
        let bm = b.mu.as_ref().unwrap();
        for i in 0..n {
            assert_abs_diff_eq!(mu[i], 0.5 * (am[i] + bm[i]), epsilon = 1e-12);
            assert_abs_diff_eq!(sd[i], (am[i] - bm[i]).abs() / 2f64.sqrt(), epsilon = 1e-12);
        }
        assert!(sd.iter().sum::<f64>() > 0.0);

        // Other grids / weights run and stay finite.
        for grid in [MergeGrid::Union, MergeGrid::Finest] {
            for weight in [
                MergeWeight::Importance(vec![2.0, 1.0]),
                MergeWeight::NoiseInverse,
            ] {
                let cfg = MergeConfig {
                    grid,
                    weight,
                    ..Default::default()
                };
                let m = merge_spectra(&[&a, &b], &cfg).unwrap();
                assert!(m.mu.as_ref().unwrap().iter().all(|v| v.is_finite()));
            }
        }
        assert!(group.merge(&[0, 5], &MergeConfig::default()).is_err());
    }

    #[test]
    fn tools_difference_with_self_is_zero() {
        let mut s = load_ru();
        s.normalize().unwrap();
        for space in [DiffSpace::Mu, DiffSpace::Norm, DiffSpace::Flat] {
            let d = difference(&s, &s, space).unwrap();
            assert!(d.mu.as_ref().unwrap().iter().all(|v| v.abs() < 1e-12));
            assert_eq!(d.energy.as_ref().unwrap(), s.energy.as_ref().unwrap());
        }
        let raw = load_ru();
        assert!(difference(&raw, &raw, DiffSpace::Norm).is_err());
    }

    #[test]
    fn tools_mutation_invalidates_pipeline_outputs() {
        let mut s = load_ru();
        s.normalize().unwrap();
        s.calc_background().unwrap();
        s.fft().unwrap();
        assert!(s.get_chi().is_some());
        assert!(s.get_chir_mag().is_some());
        s.truncate(None, Some(23000.0)).unwrap();
        assert!(s.get_norm().is_none());
        assert!(s.get_chi().is_none());
        assert!(s.get_chir_mag().is_none());
        // The pipeline recomputes cleanly afterwards.
        s.normalize().unwrap();
        s.calc_background().unwrap();
        s.fft().unwrap();
        assert!(s.get_chir_mag().is_some());
    }
}
