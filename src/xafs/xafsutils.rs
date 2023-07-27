#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

use super::bessel_i0;
use super::io;
use super::mathutils::MathUtils;

use fftconvolve::{fftconvolve, Mode};
use ndarray::{Array, Array1, ArrayBase, Axis, Ix1, OwnedRepr, Slice};
use std::cmp;
use std::error::Error;

pub const TINY_ENERGY: f64 = 0.005;

/// Physical constants used in xraytsubaki
///
/// # Example
/// ```
/// use xraytsubaki::xafs::xafsutils::constants;
///
/// assert_eq!(constants::h, 6.62607015e-34);
/// ```
pub mod constants {
    #[warn(non_upper_case_globals)]

    pub const h: f64 = 6.62607015e-34; // Planck constant
    pub const hbar: f64 = h / (2.0 * std::f64::consts::PI); // reduced Planck constant
    pub const m_e: f64 = 9.1093837015e-31; // electron mass
    pub const e: f64 = 1.602176634e-19; // elementary charge
    pub const KTOE: f64 = 1.0e20 * hbar * hbar / (2.0 * m_e * e); // convert wavenumber to energy
    pub const ETOK: f64 = 1.0 / KTOE; // convert energy to wavenumber
}

/// Trait for xafs utilities
/// functions for f64, Vec<f64>, and ArrayBase<OwnedRepr<f64>, Ix1>
pub trait XAFSUtils {
    fn etok(&self) -> Self;
    fn ktoe(&self) -> Self;
}

impl XAFSUtils for f64 {
    fn etok(&self) -> Self {
        if *self < 0.0 {
            return 0.0;
        }

        self.sqrt() * constants::KTOE
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

impl XAFSUtils for ArrayBase<OwnedRepr<f64>, Ix1> {
    fn etok(&self) -> Self {
        self.mapv(|x| x.sqrt() * constants::KTOE)
    }

    fn ktoe(&self) -> Self {
        self.mapv(|x| x.powi(2) * constants::ETOK)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub enum ConvolveForm {
    #[default]
    Lorentzian,
    Gaussian,
    Voigt,
}
/// Smooth a funtion y(x) by convoluting with a lorentzian, gaussian, or voigt function.
///
/// The function is sampled at intervals xstep, and the convolution is performed
/// using FFT convolution. The function is padded with npad points on each side
/// before convolution. The function is interpolated onto a uniform grid with
/// spacing xstep before convolution. The function is returned on the original
/// grid.
///
/// # Arguments
/// * `x` - x values of the function
/// * `y` - y values of the function
/// * `sigma` - primary width parameter for convolving function (default: 1.0)
/// * `gamma` - secondary width parameter for convolving function (default: sigma)
/// * `xstep` - step size for uniform grid onto which the function is interpolated (default: min(x.di/ff())
/// * `npad` - number of points to pad onto each side of the function before convolution (default: 5)/
/// * `conv_form` - form of the convolving function (default: lorentzian)
///
/// # Returns
/// * Result<Array1<f64>, Box<dyn Error>> - smoothed function
///
/// # Example
/// ```
/// use ndarray::Array1;
/// use xraytsubaki::xafs::xafsutils::{smooth, ConvolveForm};
///
/// let x: Array1<f64> = Array1::range(0.0, 10.0, 1.0);
/// let y: Array1<f64> = Array1::range(0.0, 10.0, 1.0);
///
/// let result = smooth(x, y, None, None, None, None, ConvolveForm::Lorentzian);
/// ```
pub fn smooth<T: Into<Array1<f64>>>(
    x: T,
    y: T,
    sigma: Option<f64>,
    gamma: Option<f64>,
    xstep: Option<f64>,
    npad: Option<i32>,
    conv_form: ConvolveForm,
) -> Result<Array1<f64>, Box<dyn Error>> {
    const TINY: f64 = 1e-12;

    let x: Array1<f64> = x.into();
    let y: Array1<f64> = y.into();
    let npad = npad.unwrap_or(5);

    let x_diff = x.diff();
    let xstep = xstep.unwrap_or(x_diff.min());

    if xstep < TINY {
        todo!("Cannot smooth data: must be strictly increasing. Impliment error handling");
    }

    let sigma = sigma.unwrap_or(1.0);
    let gamma = gamma.unwrap_or(sigma);

    let xmin = xstep * ((x.min() - npad as f64 * xstep) / xstep).floor();
    let xmax = xstep * ((x.max() + npad as f64 * xstep) / xstep).floor();
    let npts1 = 1 + ((xmax - xmin + xstep * 0.1) / xstep).abs() as i32;
    let npts = npts1.min(50 * x.len() as i32);

    let x0: Array1<f64> = Array1::linspace(xmin, xmax, npts as usize);
    let y0: Array1<f64> = x0.interpolate(&x.to_vec(), &y.to_vec())?;

    let sigma = sigma / xstep;
    let gamma = gamma / xstep;

    let wx: Array1<f64> = Array1::range(0.0, 2.0 * npts as f64, 1.0);
    let win: Array1<f64> = match conv_form {
        ConvolveForm::Gaussian => wx.gaussian(npts as f64, sigma),
        ConvolveForm::Voigt => wx.voigt(npts as f64, sigma, gamma),
        ConvolveForm::Lorentzian => wx.lorentzian(npts as f64, sigma),
    };

    let y1 = ndarray::concatenate(
        ndarray::Axis(0),
        &[
            y0.slice_axis(Axis(0), Slice::from(0..npts).step_by(-1)),
            y0.view(),
            y0.slice_axis(Axis(0), Slice::from((-npts as i32)..-1).step_by(-1)),
        ],
    )?;

    let y2 = fftconvolve(&y1, &(&win / win.sum()), Mode::Valid)?;

    let y2 = if y2.len() > x0.len() {
        let nex = ((y2.len() - x0.len()) / 2) as usize;
        let y2 = y2.slice_axis(Axis(0), Slice::from(nex..(nex + x0.len())).step_by(1));
        y2
    } else {
        y2.view()
    };

    Ok(x.interpolate(&x0.to_vec(), &y2.to_vec())?)
}

/// Function to remove duplicated successive values of an array that is expected to be monotonically increasing.
///
/// For repeated value, the second encountered occurrence (at index i) will be increased by an amount that is the larget of:
/// 1. tiny (default 1e-7)
/// 2. frac (default 1e-6) times the difference between the previous and next values.
///
/// # Arguments
/// * `arr` - Array of values to be checked for duplicates
/// * `tiny` - Minimum value to be added to a duplicate value (default 1e-7)
/// * `frac` - Fraction of the difference between the previous and next values to be added to a duplicate value (default 1e-6)
///
/// # Returns
/// * `arr` - Array with duplicates removed
///
/// # Example
/// ```
/// use xraytsubaki::xafs::xafsutils::remove_dups;
/// use ndarray::Array1;
///
/// let arr = Array1::from_vec(vec![0.0, 1.1, 2.2, 2.2, 3.3]);
/// let arr = remove_dups(arr, None, None, None);
/// assert_eq!(arr, Array1::from_vec(vec![0., 1.1, 2.2, 2.2000001, 3.3]));
/// ```
pub fn remove_dups<T: Into<ArrayBase<OwnedRepr<f64>, Ix1>>>(
    arr: T,
    tiny: Option<f64>,
    frac: Option<f64>,
    sort: Option<bool>,
) -> ArrayBase<OwnedRepr<f64>, Ix1> {
    let mut arr = arr.into();
    let tiny = tiny.unwrap_or(1e-7);
    let frac = frac.unwrap_or(1e-6);

    if arr.len() < 2 {
        return arr;
    }

    if let Some(true) = sort {
        let mut arr_sort = arr.to_vec();
        arr_sort.sort_by(|a, b| a.partial_cmp(b).unwrap());
        arr = Array1::from_vec(arr_sort);
    }

    let mut previous_value = f64::NAN;
    let mut previous_add = 0.0;

    let mut add = Array1::zeros(arr.len());

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

    arr = arr + add;

    arr
}

pub fn remove_nan2(
    arr1: &ArrayBase<OwnedRepr<f64>, Ix1>,
    arr2: &ArrayBase<OwnedRepr<f64>, Ix1>,
) -> (Array1<f64>, Array1<f64>) {
    let (arr1, arr2): (Vec<f64>, Vec<f64>) = arr1
        .iter()
        .zip(arr2.iter())
        .filter(|(e, m)| e.is_finite() && m.is_finite())
        .unzip();

    (arr1.into(), arr2.into())
}

/// Function to find the energy step of an array of energies.
/// It ignores the smallest fraction of energy steps (frac_ignore) and then averages the next nave steps.
///
/// # Arguments
/// * `energy` - Array of energies
/// * `frac_ignore` - Fraction of energy steps to ignore (default 0.01)
/// * `nave` - Number of energy steps to average (default 10)
/// * `sort` - Sort the array before finding the energy step (default false)
///
/// # Returns
/// * `estep` - Average energy step
///
/// # Example
/// ```
/// use xraytsubaki::xafs::xafsutils::find_energy_step;
/// use ndarray::array;
///
/// let energy = array![0.0, 1.1, 2.2, 2.2, 3.3];
/// let estep = find_energy_step(energy, None, None, None);
/// assert_eq!(estep, 0.7333333333333334);
/// ```
pub fn find_energy_step<T: Into<ArrayBase<OwnedRepr<f64>, Ix1>>>(
    energy: T,
    frac_ignore: Option<f64>,
    nave: Option<usize>,
    sort: Option<bool>,
) -> f64 {
    let mut energy = energy.into();

    if let Some(true) = sort {
        let mut energy_sort = energy.to_vec();
        energy_sort.sort_by(|a, b| a.partial_cmp(b).unwrap());
        energy = Array1::from_vec(energy_sort);
    }

    let frac_ignore = frac_ignore.unwrap_or(0.01);
    let nave = nave.unwrap_or(10);
    let ediff = &energy.slice(ndarray::s![1..]) - &energy.slice(ndarray::s![..-1]);
    let nskip = (frac_ignore * energy.len() as f64) as usize;

    ediff.to_vec().sort_by(|a, b| a.partial_cmp(b).unwrap());

    let ediff_end = cmp::min(nskip + nave, ediff.len() - 1);

    return ediff.slice(ndarray::s![nskip..ediff_end]).mean().unwrap();
}
/// Calculate the $E_0$, the energy threshold of absoption, or the edge energy, given $\mu(E)$.
///
/// $E_0$ is found as the point with maximum derivative with some checks to avoid spurious glitches.
///
/// # Arguments
/// * `energy` - Array of energies
/// * `mu` - Array of absorption coefficients
///
/// # Returns
/// Result<e0: f64, Box<dyn Error>>
/// * `e0` - Energy threshold of absoption, or the edge energy
///
/// # Example
/// ```
/// use xraytsubaki::xafs::xafsutils::find_e0;
/// use ndarray::Array1;
///
/// let energy:Array1<f64> = Array1::linspace(0.0, 100.0, 1000);
/// let mu = &energy.map(|x| (x-50.0).powi(3) - (x-50.0).powi(2) + x);
/// let result = find_e0(energy.clone(), mu.clone());
/// assert_eq!(result.unwrap(), 0.4004004004004004);
///
/// // Result calculated by Larch is 0.3003003003003003
/// ```

pub fn find_e0<T: Into<ArrayBase<OwnedRepr<f64>, Ix1>>>(
    energy: T,
    mu: T,
) -> Result<f64, Box<dyn Error>> {
    let energy: ArrayBase<OwnedRepr<f64>, Ix1> = energy.into();
    let mu: ArrayBase<OwnedRepr<f64>, Ix1> = mu.into();

    let (e1, ie0, estep) = _find_e0(energy.clone(), mu.clone(), None, None)?;
    let istart = (ie0 as i32 - 75).max(2) as usize;
    let istop = (ie0 + 75).min(energy.len() - 2);

    let (mut e0, ix, ex) = _find_e0(
        energy.slice(ndarray::s![istart..istop]).to_owned(),
        mu.slice(ndarray::s![istart..istop]).to_owned(),
        Some(estep),
        Some(true),
    )?;

    if ix < 1 {
        e0 = energy[istart + 2];
    }

    Ok(e0)
}

/// Internal function used for find_e0.
///
/// # Arguments
/// * `energy` - Array of energies
/// * `mu` - Array of absorption coefficients
/// * `estep` - Energy step (default: find_energy_step(energy)/2.0)
/// * `use_smooth` - Use smoothed derivative (default: false)
///
/// # Returns
/// Result<(e0: f64, imax: usize, estep: f64), Box<dyn Error>>
/// * `e0` - Energy threshold of absoption, or the edge energy
/// * `imax` - Index of maximum derivative
/// * `estep` - Energy step
///
/// # Example
/// ```
/// use xraytsubaki::xafs::xafsutils::_find_e0;
/// use ndarray::Array1;
///
/// let energy:Array1<f64> = Array1::linspace(0.0, 100.0, 1000);
/// let mu = &energy.map(|x| (x-50.0).powi(3) - (x-50.0).powi(2) + x);
///
/// let result = _find_e0(energy.clone(), mu.clone(), None, None);
/// assert_eq!(result.unwrap(), (1.001001001001001, 10, 0.05005005005005005));
///
/// // the result obtained by xraylarch is (1.001001001001001, 10, 0.05005005005004648)
/// ```
pub fn _find_e0<T: Into<ArrayBase<OwnedRepr<f64>, Ix1>> + Clone>(
    energy: T,
    mu: T,
    estep: Option<f64>,
    use_smooth: Option<bool>,
) -> Result<(f64, usize, f64), Box<dyn Error>> {
    let en: ArrayBase<OwnedRepr<f64>, Ix1> = remove_dups(energy.clone().into(), None, None, None);
    let mu: ArrayBase<OwnedRepr<f64>, Ix1> = mu.into();

    let estep = estep.unwrap_or(find_energy_step(energy.clone(), None, None, Some(false)) / 2.0);

    let nmin = 2.max(en.len() / 100);

    let dmu: ArrayBase<OwnedRepr<f64>, Ix1> = if let Some(true) = use_smooth {
        // todo!("smooth not implemented yet");
        smooth(
            energy.into(),
            mu.gradient() / en.gradient(),
            Some(3.0 * estep.clone()),
            None,
            Some(estep),
            None,
            ConvolveForm::Lorentzian,
        )
        .unwrap()
    } else {
        mu.gradient() / en.gradient()
    };

    let dmin = dmu
        .slice(ndarray::s![(nmin as i32)..(1 - nmin as i32)])
        .iter()
        .map(|a| if a.is_finite() { a } else { &-1.0 })
        .min_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap()
        .clone();

    let dm_ptp = dmu
        .slice(ndarray::s![(nmin as i32)..(1 - nmin as i32)])
        .to_vec()
        .ptp();

    let dmu = (dmu - dmin) / dm_ptp;

    let mut dhigh = if en.len() > 20 { 0.60 } else { 0.30 };

    let mut high_deriv_pts: Vec<usize> = dmu
        .indexed_iter()
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
                .indexed_iter()
                .filter(|(_, a)| a > &&dhigh)
                .map(|(i, _)| i)
                .collect();
        }
    }

    if high_deriv_pts.len() < 3 {
        high_deriv_pts = dmu
            .indexed_iter()
            .filter(|(_, a)| a.is_finite())
            .take(1)
            .map(|(i, _)| i)
            .collect();
    }

    let mut imax = 0;
    let mut dmax = 0.0;

    for i in &high_deriv_pts {
        if i < &nmin || i > &(dmu.len() - nmin) {
            continue;
        }

        if &dmu[*i] > &dmax
            && high_deriv_pts.contains(&(i + 1))
            && high_deriv_pts.contains(&(i - 1))
        {
            dmax = dmu[i.clone()];
            imax = i.clone();
        }
    }

    Ok((en[imax], imax, estep))
}

#[derive(Debug, Clone, Copy, Default)]
pub enum FTWindow {
    #[default]
    Hanning, // Hanning window, cosine-squared tamper
    Parzen,       // Parzen window, linear tamper
    Welch,        // Welch window, quadratic tamper
    Gaussian,     // Gaussian window, Gaussian (normal) tamper
    Sine,         // Sine window, sine function window
    KaiserBessel, // Kaiser-Bessel function-derived window
    FHanning,     // I am not sure what this is. It is in the Larch code, but it is not used.
}

pub fn ftwindow(
    x: &ArrayBase<OwnedRepr<f64>, Ix1>,
    xmin: Option<f64>,
    xmax: Option<f64>,
    dx: Option<f64>,
    dx2: Option<f64>,
    window: Option<FTWindow>,
) -> Result<Array1<f64>, Box<dyn Error>> {
    let window = if window.is_none() {
        FTWindow::default()
    } else {
        window.unwrap()
    };

    let mut dx1 = dx.unwrap_or(1.0);
    let mut dx2 = dx2.unwrap_or(dx1.clone());

    let xmin = xmin.unwrap_or(x.min());
    let xmax = xmax.unwrap_or(x.max());

    let xstep = (x[x.len() - 1] - x[0]) / (x.len() as f64 - 1.0);
    let xeps = &xstep * 1e-4;

    let mut x1 = x.min().max(&xmin - &dx1 / 2.0);
    let mut x2 = &xmin + &dx1 / 2.0 + &xeps;
    let mut x3 = &xmax - &dx2 / 2.0 - &xeps;
    let mut x4 = x.max().min(&xmax + &dx2 / 2.0);

    let asint = |val: &f64| ((val + &xeps) / &xstep) as i32;

    match window {
        FTWindow::Gaussian => {
            dx1 = dx1.max(xeps.clone());
        }

        FTWindow::FHanning => {
            if dx1 < 0.0 {
                dx1 = 0.0;
            }
            if dx2 > 1.0 {
                dx2 = 1.0;
            }
            x2 = &x1 + &xeps + &dx1 * (&xmax - &xmin) / 2.0;
            x3 = &x4 - &xeps - &dx2 * (&xmax - &xmin) / 2.0;
        }
        _ => {}
    }

    let (mut i1, mut i2, mut i3, mut i4) = (asint(&x1), asint(&x2), asint(&x3), asint(&x4));
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

    (x1, x2, x3, x4) = (
        x[i1 as usize],
        x[i2 as usize],
        x[i3 as usize],
        x[i4 as usize],
    );
    if x1 == x2 {
        x2 += xeps;
    }

    if x3 == x4 {
        x4 += xeps;
    }

    let mut fwin = Array1::zeros(x.len());

    if i3 > i2 {
        fwin.slice_mut(ndarray::s![i2..i3]).fill(1.0);
    }

    // if nam in ('han', 'fha'):
    //     fwin[i1:i2+1] = sin((pi/2)*(x[i1:i2+1]-x1) / (x2-x1))**2
    //     fwin[i3:i4+1] = cos((pi/2)*(x[i3:i4+1]-x3) / (x4-x3))**2
    // elif nam == 'par':
    //     fwin[i1:i2+1] =     (x[i1:i2+1]-x1) / (x2-x1)
    //     fwin[i3:i4+1] = 1 - (x[i3:i4+1]-x3) / (x4-x3)
    // elif nam == 'wel':
    //     fwin[i1:i2+1] = 1 - ((x[i1:i2+1]-x2) / (x2-x1))**2
    //     fwin[i3:i4+1] = 1 - ((x[i3:i4+1]-x3) / (x4-x3))**2
    // elif nam  in ('kai', 'bes'):
    //     cen  = (x4+x1)/2
    //     wid  = (x4-x1)/2
    //     arg  = 1 - (x-cen)**2 / (wid**2)
    //     arg[where(arg<0)] = 0
    //     if nam == 'bes': # 'bes' : ifeffit 1.0 implementation of kaiser-bessel
    //         fwin = bessel_i0(dx* sqrt(arg)) / bessel_i0(dx)
    //         fwin[where(x<=x1)] = 0
    //         fwin[where(x>=x4)] = 0
    //     else: # better version
    //         scale = max(1.e-10, bessel_i0(dx)-1)
    //         fwin = (bessel_i0(dx * sqrt(arg)) - 1) / scale
    // elif nam == 'sin':
    //     fwin[i1:i4+1] = sin(pi*(x4-x[i1:i4+1]) / (x4-x1))
    // elif nam == 'gau':
    //     cen  = (x4+x1)/2
    //     fwin =  exp(-(((x - cen)**2)/(2*dx1*dx1)))

    match window {
        FTWindow::Hanning | FTWindow::FHanning => {
            fwin.slice_mut(ndarray::s![i1..=i2])
                .assign(&x.slice(ndarray::s![i1..=i2]).mapv(|x| {
                    (std::f64::consts::PI / 2.0 * (x - x1) / (x2 - x1))
                        .sin()
                        .powi(2)
                }));
            fwin.slice_mut(ndarray::s![i3..=i4])
                .assign(&x.slice(ndarray::s![i3..=i4]).mapv(|x| {
                    (std::f64::consts::PI / 2.0 * (x - x3) / (x4 - x3))
                        .cos()
                        .powi(2)
                }));
        }
        FTWindow::Parzen => {
            fwin.slice_mut(ndarray::s![i1..=i2])
                .assign(&x.slice(ndarray::s![i1..=i2]).mapv(|x| (x - x1) / (x2 - x1)));
            fwin.slice_mut(ndarray::s![i3..=i4]).assign(
                &x.slice(ndarray::s![i3..=i4])
                    .mapv(|x| 1.0 - (x - x3) / (x4 - x3)),
            );
        }
        FTWindow::Welch => {
            fwin.slice_mut(ndarray::s![i1..=i2]).assign(
                &x.slice(ndarray::s![i1..=i2])
                    .mapv(|x| 1.0 - ((x - x2) / (x2 - x1)).powi(2)),
            );
            fwin.slice_mut(ndarray::s![i3..=i4]).assign(
                &x.slice(ndarray::s![i3..=i4])
                    .mapv(|x| 1.0 - ((x - x3) / (x4 - x3)).powi(2)),
            );
        }
        FTWindow::KaiserBessel => {
            let cen = (x4 + x1) / 2.0;
            let wid = (x4 - x1) / 2.0;
            let arg = (x - cen)
                .mapv(|x| 1.0 - x.powi(2) / wid.powi(2))
                .mapv(|x| x.max(0.0));
            let scale = (bessel_i0::bessel_i0(dx1) - 1.0).max(1e-10);

            fwin = arg.mapv(|x| (bessel_i0::bessel_i0(dx1 * x.sqrt()) - 1.0) / scale);
        }
        FTWindow::Sine => {
            fwin.slice_mut(ndarray::s![i1..=i4]).assign(
                &x.slice(ndarray::s![i1..=i4])
                    .mapv(|x| (std::f64::consts::PI * (x4 - x) / (x4 - x1)).sin()),
            );
        }
        FTWindow::Gaussian => {
            let cen = (x4 + x1) / 2.0;
            fwin = x.mapv(|x| (-(x - cen).powi(2) / (2.0 * dx1.powi(2))).exp());
        }
        _ => {}
    }

    Ok(fwin)
}

#[cfg(test)]
mod tests {
    use super::*;
    use data_reader::reader::{load_txt_f64, Delimiter, ReaderParams};

    const TOP_DIR: &'static str = env!("CARGO_MANIFEST_DIR");

    const PARAM_LOADTXT: ReaderParams = ReaderParams {
        comments: Some(b'#'),
        delimiter: Delimiter::WhiteSpace,
        skip_footer: None,
        skip_header: None,
        usecols: None,
        max_rows: None,
    };

    #[test]
    fn test_smooth() -> Result<(), Box<dyn std::error::Error>> {
        let criteria = 1e-2;

        let filepath = String::from(TOP_DIR) + "/tests/testfiles/Ru_QAS.dat";
        let expected_filepath = String::from(TOP_DIR) + "/tests/testfiles/Ru_QAS_smooth.txt";
        let expected_filepath_larch =
            String::from(TOP_DIR) + "/tests/testfiles/Ru_QAS_smooth_larch.txt";
        let xafs_group = io::load_spectrum(&filepath)?;

        let expected_data = load_txt_f64(&expected_filepath, &PARAM_LOADTXT)?;
        let expected_data = expected_data.get_col(0);

        let expected_data_larch = load_txt_f64(&expected_filepath_larch, &PARAM_LOADTXT)?;
        let expected_data_larch = expected_data_larch.get_col(0);

        let x = xafs_group.raw_energy.unwrap();
        let y = xafs_group.raw_mu.unwrap();

        let result = smooth(x, y, None, None, None, None, ConvolveForm::Lorentzian)?;

        assert_eq!(result.to_vec(), expected_data);

        result
            .iter()
            .zip(expected_data_larch.iter())
            .for_each(|(a, b)| assert!((a - b).abs() < criteria));

        Ok(())
    }

    #[test]
    fn test_remove_dups() {
        let arr = Array1::from_vec(vec![0.0, 1.1, 2.2, 2.2, 3.3]);
        let arr = remove_dups(arr, None, None, None);
        assert_eq!(arr, Array1::from_vec(vec![0., 1.1, 2.2, 2.2000001, 3.3]));
    }

    #[test]
    fn test_remove_dups_sort() {
        let arr = Array1::from_vec(vec![0.0, 1.1, 2.2, 3.3, 2.2]);
        let arr = remove_dups(arr, None, None, Some(true));
        assert_eq!(arr, Array1::from_vec(vec![0., 1.1, 2.2, 2.2000001, 3.3]));
    }

    #[test]
    fn test_remove_dups_unsorted() {
        let arr = Array1::from_vec(vec![0.0, 1.1, 2.2, 3.3, 2.2]);
        let arr = remove_dups(arr, None, None, Some(false));
        assert_ne!(arr, Array1::from_vec(vec![0., 1.1, 2.2, 2.2000001, 3.3]));
    }

    #[test]
    fn test_find_energy_step() {
        let energy = Array1::from_vec(vec![0.0, 1.0, 2.0, 3.0, 4.0]);
        let step = find_energy_step(energy, None, None, None);
        assert_eq!(step, 1.0);
    }

    #[test]
    fn test_find_energy_step_neg() {
        let energy = Array1::from_vec(vec![0.0, 1.0, 2.0, 3.0, 4.0, 2.0]);
        let step = find_energy_step(energy, None, None, None);
        assert_eq!(step, 1.0);
    }

    #[test]
    fn test_find_energy_step_sort() {
        let energy = Array1::from_vec(vec![0.0, 1.0, 2.0, 3.0, 4.0, 2.0]);
        let step = find_energy_step(energy, Some(0.), None, Some(true));
        assert_eq!(step, 0.75);
    }

    #[test]
    fn test_find_e0() {
        let energy: Array1<f64> = Array1::linspace(0.0, 100.0, 1000);
        let mu = &energy.map(|x| (x - 50.0).powi(3) - (x - 50.0).powi(2) + x);
        let result = find_e0(energy.clone(), mu.clone());
        assert_eq!(result.unwrap(), 0.4004004004004004);

        // Result calculated by Larch is 0.3003003003003003
    }

    #[test]
    fn test_KTOE() {
        let acceptable_error = 1e-12;
        assert!(
            (constants::KTOE - 3.8099821161548597).abs() < acceptable_error,
            "KTOE is not equal to 3.8099821161548597"
        );
    }

    #[test]
    fn test_ftwindow_hanning() {
        let expected_filepath = String::from(TOP_DIR) + "/tests/testfiles/window_Hanning.txt";
        let expected_data = load_txt_f64(&expected_filepath, &PARAM_LOADTXT).unwrap();
        let x = expected_data.get_col(0);
        let y_expected = expected_data.get_col(1);

        let y = ftwindow(
            &Array1::from_vec(x),
            None,
            None,
            None,
            None,
            Some(FTWindow::Hanning),
        )
        .unwrap();

        assert_eq!(y.to_vec(), y_expected);
    }
    #[test]
    fn test_ftwindow_parzen() {
        let expected_filepath = String::from(TOP_DIR) + "/tests/testfiles/window_Parzen.txt";
        let expected_data = load_txt_f64(&expected_filepath, &PARAM_LOADTXT).unwrap();
        let x = expected_data.get_col(0);
        let y_expected = expected_data.get_col(1);

        let y = ftwindow(
            &Array1::from_vec(x),
            None,
            None,
            None,
            None,
            Some(FTWindow::Parzen),
        )
        .unwrap();

        assert_eq!(y.to_vec(), y_expected);
    }
    #[test]
    fn test_ftwindow_welch() {
        let expected_filepath = String::from(TOP_DIR) + "/tests/testfiles/window_Welch.txt";
        let expected_data = load_txt_f64(&expected_filepath, &PARAM_LOADTXT).unwrap();
        let x = expected_data.get_col(0);
        let y_expected = expected_data.get_col(1);

        let y = ftwindow(
            &Array1::from_vec(x),
            None,
            None,
            None,
            None,
            Some(FTWindow::Welch),
        )
        .unwrap();

        assert_eq!(y.to_vec(), y_expected);
    }
    #[test]
    fn test_ftwindow_gaussian() {
        let expected_filepath = String::from(TOP_DIR) + "/tests/testfiles/window_Gaussian.txt";
        let expected_data = load_txt_f64(&expected_filepath, &PARAM_LOADTXT).unwrap();
        let x = expected_data.get_col(0);
        let y_expected = expected_data.get_col(1);

        let y = ftwindow(
            &Array1::from_vec(x),
            None,
            None,
            None,
            None,
            Some(FTWindow::Gaussian),
        )
        .unwrap();

        assert_eq!(y.to_vec(), y_expected);
    }
    #[test]
    fn test_ftwindow_sine() {
        let expected_filepath = String::from(TOP_DIR) + "/tests/testfiles/window_Sine.txt";
        let expected_data = load_txt_f64(&expected_filepath, &PARAM_LOADTXT).unwrap();
        let x = expected_data.get_col(0);
        let y_expected = expected_data.get_col(1);

        let y = ftwindow(
            &Array1::from_vec(x),
            None,
            None,
            None,
            None,
            Some(FTWindow::Sine),
        )
        .unwrap();

        assert_eq!(y.to_vec(), y_expected);
    }

    #[test]
    fn test_ftwindow_kaiserbessel() {
        let expected_filepath = String::from(TOP_DIR) + "/tests/testfiles/window_Kaiser-Bessel.txt";
        let expected_data = load_txt_f64(&expected_filepath, &PARAM_LOADTXT).unwrap();
        let x = expected_data.get_col(0);
        let y_expected = expected_data.get_col(1);

        let y = ftwindow(
            &Array1::from_vec(x),
            None,
            None,
            None,
            None,
            Some(FTWindow::KaiserBessel),
        )
        .unwrap();

        assert_eq!(y.to_vec(), y_expected);
    }
}
