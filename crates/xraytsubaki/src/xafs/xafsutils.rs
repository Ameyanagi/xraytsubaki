#![allow(dead_code)]
#![allow(unused_imports)]

use std::cmp;
use std::error::Error;

use nalgebra::DVector;
use serde::{Deserialize, Serialize};

use super::bessel_i0;
use super::mathutils::{index_nearest_sorted, MathUtils};

pub const TINY_ENERGY: f64 = 0.005;

pub mod constants {
    #![allow(non_upper_case_globals)]

    pub const h: f64 = 6.62607015e-34;
    pub const hbar: f64 = h / (2.0 * std::f64::consts::PI);
    pub const m_e: f64 = 9.1093837015e-31;
    pub const e: f64 = 1.602176634e-19;
    pub const KTOE: f64 = 1.0e20 * hbar * hbar / (2.0 * m_e * e);
    pub const ETOK: f64 = 1.0 / KTOE;
}

pub trait XAFSUtils {
    fn etok(&self) -> Self;
    fn ktoe(&self) -> Self;
}

impl XAFSUtils for f64 {
    fn etok(&self) -> Self {
        if *self < 0.0 {
            0.0
        } else {
            self.sqrt() * constants::KTOE
        }
    }

    fn ktoe(&self) -> Self {
        self.powi(2) * constants::ETOK
    }
}

impl XAFSUtils for Vec<f64> {
    fn etok(&self) -> Self {
        self.iter().map(|x| x.etok()).collect()
    }

    fn ktoe(&self) -> Self {
        self.iter().map(|x| x.ktoe()).collect()
    }
}

impl XAFSUtils for DVector<f64> {
    fn etok(&self) -> Self {
        self.map(|x| {
            if x < 0.0 {
                0.0
            } else {
                x.sqrt() * constants::KTOE
            }
        })
    }

    fn ktoe(&self) -> Self {
        self.map(|x| x.powi(2) * constants::ETOK)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub enum ConvolveForm {
    #[default]
    Lorentzian,
    Gaussian,
    Voigt,
}

pub fn smooth(
    x: &DVector<f64>,
    y: &DVector<f64>,
    sigma: Option<f64>,
    gamma: Option<f64>,
    xstep: Option<f64>,
    npad: Option<i32>,
    conv_form: ConvolveForm,
) -> Result<DVector<f64>, Box<dyn Error>> {
    const TINY: f64 = 1e-12;

    let npad = npad.unwrap_or(5).max(1) as usize;
    if x.len() != y.len() || x.len() < 3 {
        return Err("x/y length mismatch for smoothing".into());
    }

    let x_diff = x.diff();
    let xstep = xstep.unwrap_or(x_diff.min());
    if xstep < TINY {
        return Err("Cannot smooth data: x must be strictly increasing".into());
    }

    let sigma = sigma.unwrap_or(1.0);
    let gamma = gamma.unwrap_or(sigma);

    let xmin = xstep * ((x.min() - npad as f64 * xstep) / xstep).floor();
    let xmax = xstep * ((x.max() + npad as f64 * xstep) / xstep).floor();
    let npts1 = 1 + ((xmax - xmin + xstep * 0.1) / xstep).abs() as i32;
    let npts = (npts1.min(50 * x.len() as i32)).max(2) as usize;

    let x0 = linspace(xmin, xmax, npts);
    let y0 = x0.interpolate(x.as_slice(), y.as_slice())?;

    let sigma = sigma / xstep;
    let gamma = gamma / xstep;

    let wx = dvector_arange(0.0, 2.0 * npts as f64, 1.0);
    let win = match conv_form {
        ConvolveForm::Gaussian => wx.gaussian(npts as f64, sigma),
        ConvolveForm::Voigt => wx.voigt(npts as f64, sigma, gamma),
        ConvolveForm::Lorentzian => wx.lorentzian(npts as f64, sigma),
    };

    let mut y1 = Vec::with_capacity(npts + y0.len() + npts.saturating_sub(1));
    y1.extend(y0.as_slice()[0..npts].iter().rev().copied());
    y1.extend(y0.iter().copied());
    if y0.len() > 1 {
        let tail_start = y0.len().saturating_sub(npts);
        let tail_end = y0.len() - 1;
        y1.extend(y0.as_slice()[tail_start..tail_end].iter().rev().copied());
    }
    let y1 = DVector::from_vec(y1);

    let win_sum = win.iter().sum::<f64>();
    let kernel = if win_sum.abs() > f64::EPSILON {
        &win / win_sum
    } else {
        win.clone()
    };

    let mut y2 = convolve_valid(&y1, &kernel)?;
    if y2.len() > x0.len() {
        let nex = (y2.len() - x0.len()) / 2;
        y2 = y2.rows(nex, x0.len()).into_owned();
    }

    Ok(x.interpolate(x0.as_slice(), y2.as_slice())?)
}

fn convolve_valid(
    signal: &DVector<f64>,
    kernel: &DVector<f64>,
) -> Result<DVector<f64>, Box<dyn Error>> {
    if signal.is_empty() || kernel.is_empty() || signal.len() < kernel.len() {
        return Err("invalid convolution input lengths".into());
    }

    let out_len = signal.len() - kernel.len() + 1;
    let mut out = DVector::zeros(out_len);
    for i in 0..out_len {
        let mut acc = 0.0;
        for j in 0..kernel.len() {
            acc += signal[i + j] * kernel[kernel.len() - 1 - j];
        }
        out[i] = acc;
    }

    Ok(out)
}

fn dvector_arange(start: f64, stop: f64, step: f64) -> DVector<f64> {
    if !step.is_finite() || step <= 0.0 {
        return DVector::zeros(0);
    }
    let mut values = Vec::new();
    let mut value = start;
    while value < stop {
        values.push(value);
        value += step;
    }
    DVector::from_vec(values)
}

fn linspace(start: f64, end: f64, n: usize) -> DVector<f64> {
    if n <= 1 {
        return DVector::from_vec(vec![start]);
    }
    let step = (end - start) / (n as f64 - 1.0);
    DVector::from_iterator(n, (0..n).map(|i| start + i as f64 * step))
}

pub fn find_energy_step(
    energy: &DVector<f64>,
    frac_ignore: Option<f64>,
    nave: Option<usize>,
    sort: Option<bool>,
) -> f64 {
    let energy = if let Some(true) = sort {
        let mut energy_sort = energy.as_slice().to_vec();
        energy_sort.sort_by(|a, b| a.partial_cmp(b).unwrap());
        DVector::from_vec(energy_sort)
    } else {
        energy.clone()
    };

    let frac_ignore = frac_ignore.unwrap_or(0.01);
    let nave = nave.unwrap_or(10);

    let mut ediff: Vec<f64> = Vec::with_capacity(energy.len().saturating_sub(1));
    for i in 1..energy.len() {
        ediff.push(energy[i] - energy[i - 1]);
    }
    if ediff.is_empty() {
        return TINY_ENERGY;
    }

    let nskip = (frac_ignore * energy.len() as f64) as usize;
    ediff.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let ediff_end = std::cmp::min(nskip + nave, ediff.len() - 1);
    if ediff_end <= nskip {
        TINY_ENERGY
    } else {
        ediff[nskip..ediff_end].iter().sum::<f64>() / (ediff_end - nskip) as f64
    }
}

pub fn remove_dups(
    arr: &DVector<f64>,
    tiny: Option<f64>,
    frac: Option<f64>,
    sort: Option<bool>,
) -> DVector<f64> {
    let tiny = tiny.unwrap_or(1e-7);
    let frac = frac.unwrap_or(1e-6);

    if arr.len() < 2 {
        return arr.clone();
    }

    let arr = if let Some(true) = sort {
        let mut arr_sort = arr.as_slice().to_vec();
        arr_sort.sort_by(|a, b| a.partial_cmp(b).unwrap());
        DVector::from_vec(arr_sort)
    } else {
        arr.clone()
    };

    let mut previous_value = f64::NAN;
    let mut previous_add = 0.0;
    let mut add = DVector::zeros(arr.len());

    for i in 1..arr.len() {
        if !arr[i - 1].is_nan() {
            previous_value = arr[i - 1];
            previous_add = add[i - 1];
        }
        let value = arr[i];
        if value.is_nan() || previous_value.is_nan() {
            continue;
        }
        let diff = (value - previous_value).abs();
        if diff < tiny {
            add[i] = previous_add + f64::max(tiny, frac * diff);
        }
    }

    arr + add
}

pub fn _find_e0(
    energy: &DVector<f64>,
    mu: &DVector<f64>,
    estep: Option<f64>,
    use_smooth: Option<bool>,
) -> Result<(f64, usize, f64), Box<dyn Error>> {
    if energy.len() != mu.len() {
        return Err("energy and mu length mismatch".into());
    }
    if energy.len() < 3 {
        return Err("need at least 3 points".into());
    }

    let en = remove_dups(energy, None, None, None);
    let estep = estep.unwrap_or(find_energy_step(energy, None, None, Some(false)) / 2.0);
    let nmin = 2.max(en.len() / 100);

    let mu_grad = mu.gradient();
    let en_grad = en.gradient();

    let raw_dmu = DVector::from_fn(mu_grad.len(), |i, _| {
        if en_grad[i].abs() > 1e-12 {
            mu_grad[i] / en_grad[i]
        } else {
            0.0
        }
    });

    let dmu = if let Some(true) = use_smooth {
        smooth(
            energy,
            &raw_dmu,
            Some(3.0 * estep),
            None,
            Some(estep),
            None,
            ConvolveForm::Lorentzian,
        )
        .unwrap_or_else(|_| {
            DVector::from_fn(mu_grad.len(), |i, _| {
                if en_grad[i].abs() > 1e-12 {
                    mu_grad[i] / en_grad[i]
                } else {
                    0.0
                }
            })
        })
    } else {
        DVector::from_fn(mu_grad.len(), |i, _| {
            if en_grad[i].abs() > 1e-12 {
                mu_grad[i] / en_grad[i]
            } else {
                0.0
            }
        })
    };

    let middle_start = nmin.min(dmu.len());
    let middle_end = dmu.len().saturating_sub(nmin);
    let middle_slice: Vec<f64> = if middle_end > middle_start {
        dmu.as_slice()[middle_start..middle_end]
            .iter()
            .copied()
            .filter(|value| value.is_finite())
            .collect()
    } else {
        dmu.iter()
            .copied()
            .filter(|value| value.is_finite())
            .collect()
    };
    let dmin = middle_slice
        .iter()
        .copied()
        .reduce(f64::min)
        .unwrap_or(-1.0);
    let dm_min = middle_slice
        .iter()
        .copied()
        .fold(f64::INFINITY, |acc, value| acc.min(value));
    let dm_max = middle_slice
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, |acc, value| acc.max(value));
    let dm_ptp = (dm_max - dm_min).abs().max(f64::EPSILON);
    let dmu = DVector::from_fn(dmu.len(), |i, _| (dmu[i] - dmin) / dm_ptp);

    let mut dhigh = if en.len() > 20 { 0.60 } else { 0.30 };
    let mut high_deriv_pts: Vec<usize> = dmu
        .iter()
        .enumerate()
        .filter(|(_, a)| a > &&dhigh)
        .map(|(i, _)| i)
        .collect();

    if high_deriv_pts.len() < 3 {
        for _ in 0..2 {
            if high_deriv_pts.len() > 3 {
                break;
            }
            dhigh *= 0.5;
            high_deriv_pts = dmu
                .iter()
                .enumerate()
                .filter(|(_, a)| a > &&dhigh)
                .map(|(i, _)| i)
                .collect();
        }
    }

    if high_deriv_pts.len() < 3 {
        high_deriv_pts = dmu
            .iter()
            .enumerate()
            .filter(|(_, a)| a.is_finite())
            .take(1)
            .map(|(i, _)| i)
            .collect();
    }

    let mut high_deriv_mask = vec![false; dmu.len()];
    for &idx in &high_deriv_pts {
        if idx < high_deriv_mask.len() {
            high_deriv_mask[idx] = true;
        }
    }

    let mut imax = 0;
    let mut dmax = 0.0;

    let upper = dmu.len().saturating_sub(nmin);
    for i in &high_deriv_pts {
        if i < &nmin || i > &upper {
            continue;
        }

        let idx = *i;
        let has_prev = idx > 0 && high_deriv_mask[idx - 1];
        let has_next = idx + 1 < high_deriv_mask.len() && high_deriv_mask[idx + 1];
        if dmu[idx] > dmax && has_prev && has_next {
            dmax = dmu[idx];
            imax = *i;
        }
    }

    Ok((en[imax], imax, estep))
}

pub fn find_e0(energy: &DVector<f64>, mu: &DVector<f64>) -> Result<f64, Box<dyn Error>> {
    if energy.len() != mu.len() {
        return Err("energy and mu length mismatch".into());
    }

    let (e1, ie0, estep) = _find_e0(energy, mu, None, None)?;
    let n = energy.len();
    if n < 3 {
        return Ok(e1);
    }

    let istart = (ie0 as i32 - 75).max(2) as usize;
    let istop = (ie0 + 75).min(n - 2);
    if istop <= istart || istart + 2 >= n {
        return Ok(e1);
    }

    let energy_slice = DVector::from_iterator(
        istop - istart,
        energy.as_slice()[istart..istop].iter().cloned(),
    );
    let mu_slice =
        DVector::from_iterator(istop - istart, mu.as_slice()[istart..istop].iter().cloned());
    if energy_slice.len() < 3 {
        return Ok(e1);
    }

    let (mut e0, ix, _ex) = match _find_e0(&energy_slice, &mu_slice, Some(estep), Some(true)) {
        Ok(value) => value,
        Err(_) => return Ok(e1),
    };
    if ix < 1 {
        e0 = energy[istart + 2];
    }
    Ok(e0)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub enum FTWindow {
    #[default]
    Hanning,
    Parzen,
    Welch,
    Gaussian,
    Sine,
    KaiserBessel,
    FHanning,
}

impl FTWindow {
    pub fn window(
        &self,
        x: &DVector<f64>,
        xmin: Option<f64>,
        xmax: Option<f64>,
        dx: Option<f64>,
        dx2: Option<f64>,
    ) -> Result<DVector<f64>, Box<dyn Error>> {
        ftwindow(x, xmin, xmax, dx, dx2, Some(*self))
    }
}

pub fn ftwindow(
    x: &DVector<f64>,
    xmin: Option<f64>,
    xmax: Option<f64>,
    dx: Option<f64>,
    dx2: Option<f64>,
    window: Option<FTWindow>,
) -> Result<DVector<f64>, Box<dyn Error>> {
    if x.is_empty() {
        return Ok(DVector::zeros(0));
    }
    if x.len() == 1 {
        return Ok(DVector::from_element(1, 1.0));
    }

    let window = window.unwrap_or_default();
    let mut dx1 = dx.unwrap_or(1.0);
    let mut dx2 = dx2.unwrap_or(dx1);

    let xmin = xmin.unwrap_or(x.min());
    let xmax = xmax.unwrap_or(x.max());
    let xstep = (x[x.len() - 1] - x[0]) / (x.len() as f64 - 1.0);
    let xeps = xstep * 1e-4;

    let mut x1 = x.min().max(xmin - dx1 / 2.0);
    let mut x2 = xmin + dx1 / 2.0 + xeps;
    let mut x3 = xmax - dx2 / 2.0 - xeps;
    let mut x4 = x.max().min(xmax + dx2 / 2.0);

    let asint = |val: f64| ((val + xeps) / xstep) as i32;

    match window {
        FTWindow::Gaussian => {
            dx1 = dx1.max(xeps);
        }
        FTWindow::FHanning => {
            if dx1 < 0.0 {
                dx1 = 0.0;
            }
            if dx2 > 1.0 {
                dx2 = 1.0;
            }
            x2 = x1 + xeps + dx1 * (xmax - xmin) / 2.0;
            x3 = x4 - xeps - dx2 * (xmax - xmin) / 2.0;
        }
        _ => {}
    }

    let mut i1 = asint(x1);
    let mut i2 = asint(x2);
    let mut i3 = asint(x3);
    let mut i4 = asint(x4);

    i1 = i1.max(0);
    i2 = i2.max(0);
    i3 = i3.min((x.len() - 1) as i32);
    i4 = i4.min((x.len() - 1) as i32);

    if i1 == i2 {
        i1 = (i2 - 1).max(0);
    }
    if i3 == i4 {
        i3 = (i4 - 1).max(i2);
    }

    x1 = x[i1 as usize];
    x2 = x[i2 as usize];
    x3 = x[i3 as usize];
    x4 = x[i4 as usize];

    if x1 == x2 {
        x2 += xeps;
    }
    if x3 == x4 {
        x4 += xeps;
    }

    let mut out = DVector::zeros(x.len());

    if i3 > i2 {
        for i in i2..i3 {
            out[i as usize] = 1.0;
        }
    }

    let i1u = i1 as usize;
    let i2u = i2 as usize;
    let i3u = i3 as usize;
    let i4u = i4 as usize;

    match window {
        FTWindow::Hanning | FTWindow::FHanning => {
            for i in i1u..=i2u {
                let value = (std::f64::consts::PI / 2.0 * (x[i] - x1) / (x2 - x1))
                    .sin()
                    .powi(2);
                out[i] = value;
            }
            for i in i3u..=i4u {
                let value = (std::f64::consts::PI / 2.0 * (x[i] - x3) / (x4 - x3))
                    .cos()
                    .powi(2);
                out[i] = value;
            }
        }
        FTWindow::Parzen => {
            for i in i1u..=i2u {
                out[i] = (x[i] - x1) / (x2 - x1);
            }
            for i in i3u..=i4u {
                out[i] = 1.0 - (x[i] - x3) / (x4 - x3);
            }
        }
        FTWindow::Welch => {
            for i in i1u..=i2u {
                out[i] = 1.0 - ((x[i] - x2) / (x2 - x1)).powi(2);
            }
            for i in i3u..=i4u {
                out[i] = 1.0 - ((x[i] - x3) / (x4 - x3)).powi(2);
            }
        }
        FTWindow::KaiserBessel => {
            let cen = (x4 + x1) / 2.0;
            let wid = (x4 - x1) / 2.0;
            let scale = (bessel_i0::bessel_i0(dx1) - 1.0).max(1e-10);
            for (i, xv) in x.iter().enumerate() {
                let arg = (1.0 - ((*xv - cen).powi(2) / wid.powi(2))).max(0.0);
                out[i] = (bessel_i0::bessel_i0(dx1 * arg.sqrt()) - 1.0) / scale;
            }
        }
        FTWindow::Sine => {
            for i in i1u..=i4u {
                out[i] = (std::f64::consts::PI * (x4 - x[i]) / (x4 - x1)).sin();
            }
        }
        FTWindow::Gaussian => {
            let cen = (x4 + x1) / 2.0;
            for (i, xv) in x.iter().enumerate() {
                out[i] = (-(*xv - cen).powi(2) / (2.0 * dx1.powi(2))).exp();
            }
        }
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;
    use data_reader::reader::load_txt_f64;

    use crate::xafs::tests::{PARAM_LOADTXT, TEST_TOL, TOP_DIR};

    const TEST_TOL_FTWINDOW: f64 = 1e-15;

    fn assert_window_matches(path_suffix: &str, window: FTWindow, epsilon: f64) {
        let expected_filepath = String::from(TOP_DIR) + path_suffix;
        let expected_data = load_txt_f64(&expected_filepath, &PARAM_LOADTXT).unwrap();
        let x = DVector::from_vec(expected_data.get_col(0));
        let y_expected = expected_data.get_col(1);

        let y = ftwindow(&x, None, None, None, None, Some(window)).unwrap();
        y.iter()
            .zip(y_expected.iter())
            .for_each(|(a, b)| assert_abs_diff_eq!(a, b, epsilon = epsilon));
    }

    #[test]
    fn test_ftwindow_hanning() {
        assert_window_matches(
            "/tests/testfiles/window_Hanning.txt",
            FTWindow::Hanning,
            TEST_TOL,
        );
    }

    #[test]
    fn test_ftwindow_parzen() {
        assert_window_matches(
            "/tests/testfiles/window_Parzen.txt",
            FTWindow::Parzen,
            TEST_TOL,
        );
    }

    #[test]
    fn test_ftwindow_welch() {
        assert_window_matches(
            "/tests/testfiles/window_Welch.txt",
            FTWindow::Welch,
            TEST_TOL,
        );
    }

    #[test]
    fn test_ftwindow_gaussian() {
        assert_window_matches(
            "/tests/testfiles/window_Gaussian.txt",
            FTWindow::Gaussian,
            TEST_TOL,
        );
    }

    #[test]
    fn test_ftwindow_sine() {
        assert_window_matches("/tests/testfiles/window_Sine.txt", FTWindow::Sine, TEST_TOL);
    }

    #[test]
    fn test_ftwindow_kaiserbessel() {
        assert_window_matches(
            "/tests/testfiles/window_Kaiser-Bessel.txt",
            FTWindow::KaiserBessel,
            TEST_TOL_FTWINDOW,
        );
    }

    #[test]
    fn test_find_energy_step_sort() {
        let energy = DVector::from_vec(vec![0.0, 1.0, 2.0, 3.0, 4.0, 2.0]);
        let step = find_energy_step(&energy, Some(0.0), None, Some(true));
        assert_abs_diff_eq!(step, 0.75, epsilon = TEST_TOL);
    }

    #[test]
    fn test_find_e0() {
        let energy = DVector::from_iterator(1000, (0..1000).map(|i| i as f64 * 100.0 / 999.0));
        let mu = energy.map(|x| (x - 50.0).powi(3) - (x - 50.0).powi(2) + x);
        let result = find_e0(&energy, &mu).unwrap();
        assert_abs_diff_eq!(result, 0.4004004004004004, epsilon = TEST_TOL);
    }

    #[test]
    fn test_find_e0_length_mismatch_returns_error() {
        let energy = DVector::from_vec(vec![1.0, 2.0, 3.0]);
        let mu = DVector::from_vec(vec![1.0, 2.0]);
        assert!(_find_e0(&energy, &mu, None, None).is_err());
        assert!(find_e0(&energy, &mu).is_err());
    }

    #[test]
    fn test_smooth_smoke() {
        let x = dvector_arange(0.0, 10.0, 1.0);
        let mut y = DVector::zeros(x.len());
        y[5] = 1.0;

        let smoothed = smooth(&x, &y, None, None, None, None, ConvolveForm::Lorentzian).unwrap();
        assert_eq!(smoothed.len(), y.len());
        assert!(smoothed.iter().all(|value| value.is_finite()));
        assert!(smoothed[5] < y[5]);
        assert!(smoothed[5] > smoothed[0]);
    }
}
