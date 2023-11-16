#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

use crate::xafs::mathutils::index_of;

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
    #![allow(non_upper_case_globals)]

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
/// assert_eq!(estep, 0.7333333333333333);
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
    let mut ediff = (&energy.slice(ndarray::s![1..]) - &energy.slice(ndarray::s![..-1]))
        .to_owned()
        .to_vec();

    let nskip = (frac_ignore * energy.len() as f64) as usize;

    ediff.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let ediff_end = cmp::min(nskip + nave, ediff.len() - 1);

    return ediff[nskip..ediff_end].iter().sum::<f64>() / (ediff_end - nskip) as f64;
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
/// assert_eq!(result.unwrap(), (1.001001001001001, 10, 0.05005005005004648));
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

#[derive(Debug, Clone, Copy, Default, PartialEq)]
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

impl FTWindow {
    pub fn window(
        &self,
        x: &ArrayBase<OwnedRepr<f64>, Ix1>,
        xmin: Option<f64>,
        xmax: Option<f64>,
        dx: Option<f64>,
        dx2: Option<f64>,
    ) -> Result<Array1<f64>, Box<dyn Error>> {
        ftwindow(x, xmin, xmax, dx, dx2, Some(self.clone()))
    }
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
    }

    Ok(fwin)
}

// def rebin_xafs(energy, mu=None, group=None, e0=None, pre1=None, pre2=-30,
//     pre_step=2, xanes_step=None, exafs1=15, exafs2=None,
//     exafs_kstep=0.05, method='centroid'):
// """rebin XAFS energy and mu to a 'standard 3 region XAFS scan'

// Arguments
// ---------
// energy       input energy array
// mu           input mu array
// group        output group
// e0           energy reference -- all energy values are relative to this
// pre1         start of pre-edge region [1st energy point]
// pre2         end of pre-edge region, start of XANES region [-30]
// pre_step     energy step for pre-edge region [2]
// xanes_step   energy step for XANES region [see note]
// exafs1       end of XANES region, start of EXAFS region [15]
// exafs2       end of EXAFS region [last energy point]
// exafs_kstep  k-step for EXAFS region [0.05]
// method       one of 'boxcar', 'centroid' ['centroid']

// Returns
// -------
// None

// A group named 'rebinned' will be created in the output group, with the
// following  attributes:
// energy  new energy array
// mu      mu for energy array
// e0      e0 copied from current group

// (if the output group is None, _sys.xafsGroup will be written to)

// Notes
// ------
// 1 If the first argument is a Group, it must contain 'energy' and 'mu'.
// See First Argrument Group in Documentation

// 2 If xanes_step is None, it will be found from the data as E0/25000,
// truncated down to the nearest 0.05: xanes_step = 0.05*max(1, int(e0/1250.0))

// 3 The EXAFS region will be spaced in k-space

// 4 The rebinned data is found by determining which segments of the
// input energy correspond to each bin in the new energy array. That
// is, each input energy is assigned to exactly one bin in the new
// array.  For each new energy bin, the new value is selected from the
// data in the segment as either
// a) linear interpolation if there are fewer than 3 points in the segment.
// b) mean value ('boxcar')
// c) centroid ('centroid')

// """
// energy, mu, group = parse_group_args(energy, members=('energy', 'mu'),
//                               defaults=(mu,), group=group,
//                              fcn_name='rebin_xafs')

// if e0 is None:
// e0 = getattr(group, 'e0', None)

// if e0 is None:
// raise ValueError("need e0")

// if pre1 is None:
// pre1 = pre_step*int((min(energy) - e0)/pre_step)

// if exafs2 is None:
// exafs2 = max(energy) - e0

// # determine xanes step size:
// #  find mean of energy difference within 10 eV of E0
// nx1 = index_of(energy, e0-10)
// nx2 = index_of(energy, e0+10)
// de_mean = np.diff(energy[nx1:nx1]).mean()
// if xanes_step is None:
// xanes_step = 0.05 * max(1, int(e0 / 1250.0))  # E0/25000, round down to 0.05

// # create new energy array from the 3 segments (pre, xanes, exafs)
// en = []
// for start, stop, step, isk in ((pre1, pre2, pre_step, False),
//                         (pre2, exafs1, xanes_step, False),
//                         (exafs1, exafs2, exafs_kstep, True)):
// if isk:
//  start = etok(start)
//  stop = etok(stop)

// npts = 1 + int(0.1  + abs(stop - start) / step)
// reg = np.linspace(start, stop, npts)
// if isk:
//  reg = ktoe(reg)
// en.extend(e0 + reg[:-1])

// # find the segment boundaries of the old energy array
// bounds = [index_of(energy, e) for e in en]
// mu_out = []
// err_out = []

// j0 = 0
// for i in range(len(en)):
// if i == len(en) - 1:
//  j1 = len(energy) - 1
// else:
//  j1 = int((bounds[i] + bounds[i+1] + 1)/2.0)
// if i == 0 and j0 == 0:
//  j0 = index_of(energy, en[0]-5)
// # if not enough points in segment, do interpolation
// if (j1 - j0) < 3:
//  jx = j1 + 1
//  if (jx - j0) < 3:
//      jx += 1

//  val = interp1d(energy[j0:jx], mu[j0:jx], en[i])
//  err = mu[j0:jx].std()
//  if np.isnan(val):
//      j0 = max(0, j0-1)
//      jx = min(len(energy), jx+1)
//      val = interp1d(energy[j0:jx], mu[j0:jx], en[i])
//      err = mu[j0:jx].std()
// else:
//  if method.startswith('box'):
//      val =  mu[j0:j1].mean()
//  else:
//      val = (mu[j0:j1]*energy[j0:j1]).mean()/energy[j0:j1].mean()
// mu_out.append(val)
// err_out.append(mu[j0:j1].std())
// j0 = j1

// newname = group.__name__ + '_rebinned'
// group.rebinned = Group(energy=np.array(en), mu=np.array(mu_out),
//                 delta_mu=np.array(err_out), e0=e0,
//                 __name__=newname)
// return

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum RebinMethod {
    Boxcar,
    #[default]
    Centroid,
}

pub fn rebin(
    energy: ArrayBase<OwnedRepr<f64>, Ix1>,
    mu: ArrayBase<OwnedRepr<f64>, Ix1>,
    e0: f64,
    pre1: Option<f64>,
    pre2: Option<f64>,
    pre_step: Option<f64>,
    xanes_step: Option<f64>,
    exafs1: Option<f64>,
    exafs2: Option<f64>,
    exafs_kstep: Option<f64>,
    method: RebinMethod,
) -> Result<(Array1<f64>, Array1<f64>, Array1<f64>), Box<dyn Error>> {
    let pre2: f64 = pre2.unwrap_or(-30.0);
    let pre_step = pre_step.unwrap_or(2.0);
    let exafs1 = exafs1.unwrap_or(15.0);
    let exafs_kstep = exafs_kstep.unwrap_or(0.05);

    let pre1 = pre1.unwrap_or(pre_step * ((energy.min() - e0) / pre_step).floor());
    let exafs2 = exafs2.unwrap_or(energy.max() - e0);

    // let xanes_step = if xanes_step.is_none() {
    //     let xanes_x1 = index_of(&energy.to_vec(), &(e0 - 10.0));
    //     let xanes_x2 = index_of(&energy.to_vec(), &(e0 + 10.0));

    //     let de_mean = (&energy.slice(ndarray::s![xanes_x1..xanes_x2]).to_owned() - e0).mean();

    //     0.05 * f64::max(1.0, (e0 / 1250.0).floor())
    // } else {
    //     xanes_step.unwrap()
    // };

    // let mut en = Array1::zeros(0);

    // for (start, stop, step, is_kspace) in [
    //     (pre1, pre2, pre_step, false),
    //     (pre2, exafs1, xanes_step, false),
    //     (exafs1, exafs2, exafs_kstep, true),
    // ] {
    //     let (start, stop) = if is_kspace {
    //         (etok(start), etok(stop))
    //     } else {
    //         (start, stop)
    //     };

    //     let npts = 1 + ((stop - start) / step + 0.1).abs().floor() as usize;
    //     let reg = Array1::linspace(start, stop, npts);
    //     let reg = if is_kspace { ktoe(reg) } else { reg };

    //     en.extend(e0 + &reg.slice(ndarray::s![..-1]));
    // }

    // let bounds = en
    //     .iter()
    //     .map(|e| index_of(&energy.to_vec(), e))
    //     .collect::<Vec<usize>>();

    // let mut mu_out = Array1::zeros(0);
    // let mut err_out = Array1::zeros(0);

    // let mut j0 = 0;

    todo!("finish rebin function")

    // for i in 0..en.len() {
    //     let j1 = if i == en.len() - 1 {
    //         energy.len() - 1
    //     } else {
    //         ((bounds[i] + bounds[i + 1] + 1) / 2).floor() as usize
    //     };

    //     if i == 0 && j0 == 0 {
    //         j0 = index_of(&energy.to_vec(), &(en[0] - 5.0));
    //     }

    //     if (j1 - j0) < 3 {
    //         let jx = j1 + 1;
    //         let jx = if (jx - j0) < 3 {
    //             jx + 1
    //         } else {
    //             jx
    //         };

    //         let val = interp1d(
    //             &energy.slice(ndarray:: s![j0..jx]).to_owned(),
    //             &mu.slice(ndarray::s![j0..jx]).to_owned(),
    //             en[i],
    //         )?;

    //         let err = mu.slice(ndarray::s![j0..jx]).to_owned().std_axis(Axis(0));

    //         if val.is_nan() {
    //             j0 = f64::max(0.0, j0 as f64 - 1.0) as usize;
    //             let jx = f64::min(energy.len() as f64, jx as f64 + 1.0) as usize;
    //             let val = interp1d(
    //                 &energy.slice(ndarray:: s![j0..jx]).to_owned(),
    //                 &mu.slice(ndarray::s![j0..jx]).to_owned(),
    //                 en[i],
    //             )?;
    //             let err = mu.slice(ndarray::s![j0..jx]).to_owned().std_axis(Axis(0));
    //         }

    //         mu_out.push(val);
    //         err_out.push(err);
    //     } else {
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xafs::tests::PARAM_LOADTXT;
    use crate::xafs::tests::TEST_TOL;
    use crate::xafs::tests::TOP_DIR;
    use approx::{assert_abs_diff_eq, assert_abs_diff_ne};
    use data_reader::reader::{load_txt_f64, Delimiter, ReaderParams};
    const ACCEPTABLE_MU_DIFF: f64 = 1e-2;
    const TEST_TOL_FTWINDOW: f64 = 1e-15;

    #[test]
    fn test_smooth() -> Result<(), Box<dyn std::error::Error>> {
        let filepath = String::from(TOP_DIR) + "/tests/testfiles/Ru_QAS.dat";
        let expected_filepath = String::from(TOP_DIR) + "/tests/testfiles/Ru_QAS_smooth.txt";
        let expected_filepath_larch =
            String::from(TOP_DIR) + "/tests/testfiles/Ru_QAS_smooth_larch.txt";
        let xafs_group = io::load_spectrum_QAS_trans(&filepath)?;

        let expected_data = load_txt_f64(&expected_filepath, &PARAM_LOADTXT)?;
        let expected_data = expected_data.get_col(0);

        let expected_data_larch = load_txt_f64(&expected_filepath_larch, &PARAM_LOADTXT)?;
        let expected_data_larch = expected_data_larch.get_col(0);

        let x = xafs_group.raw_energy.unwrap();
        let y = xafs_group.raw_mu.unwrap();

        let result = smooth(x, y, None, None, None, None, ConvolveForm::Lorentzian)?;

        result
            .iter()
            .zip(expected_data)
            .for_each(|(a, b)| assert_abs_diff_eq!(a, &b, epsilon = TEST_TOL));

        result
            .iter()
            .zip(expected_data_larch.iter())
            .for_each(|(a, b)| assert_abs_diff_eq!(a, &b, epsilon = ACCEPTABLE_MU_DIFF));

        Ok(())
    }

    #[test]
    fn test_remove_dups() {
        let arr = Array1::from_vec(vec![0.0, 1.1, 2.2, 2.2, 3.3]);
        let arr = remove_dups(arr, None, None, None);
        let expected = Array1::from_vec(vec![0.0, 1.1, 2.2, 2.2000001, 3.3]);

        arr.iter().zip(expected.iter()).for_each(|(a, b)| {
            assert_abs_diff_eq!(a, &b, epsilon = TEST_TOL);
        });
    }

    #[test]
    fn test_remove_dups_sort() {
        let arr = Array1::from_vec(vec![0.0, 1.1, 2.2, 3.3, 2.2]);
        let arr = remove_dups(arr, None, None, Some(true));
        let expected = Array1::from_vec(vec![0.0, 1.1, 2.2, 2.2000001, 3.3]);

        arr.iter().zip(expected.iter()).for_each(|(a, b)| {
            assert_abs_diff_eq!(a, &b, epsilon = TEST_TOL);
        });
    }

    #[test]
    fn test_remove_dups_unsorted() {
        let arr = Array1::from_vec(vec![0.0, 1.1, 2.2, 3.3, 2.2]);
        let arr = remove_dups(arr, None, None, Some(false));
        let expected = Array1::from_vec(vec![0.0, 1.1, 2.2, 2.2000001, 3.3]);

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
        assert_eq!(step, 0.25);
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

        assert_abs_diff_eq!(result.unwrap(), 0.4004004004004004, epsilon = TEST_TOL);
    }

    #[allow(non_snake_case)]
    #[test]
    fn test_KTOE() {
        let expected_KTOE = 3.8099821161548606;

        assert_abs_diff_eq!(constants::KTOE, expected_KTOE, epsilon = TEST_TOL);
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

        y.iter()
            .zip(y_expected.iter())
            .for_each(|(a, b)| assert_abs_diff_eq!(a, &b, epsilon = TEST_TOL));
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

        y.iter()
            .zip(y_expected.iter())
            .for_each(|(a, b)| assert_abs_diff_eq!(a, &b, epsilon = TEST_TOL));
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

        y.iter()
            .zip(y_expected.iter())
            .for_each(|(a, b)| assert_abs_diff_eq!(a, &b, epsilon = TEST_TOL));
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

        y.iter()
            .zip(y_expected.iter())
            .for_each(|(a, b)| assert_abs_diff_eq!(a, &b, epsilon = TEST_TOL));
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

        y.iter()
            .zip(y_expected.iter())
            .for_each(|(a, b)| assert_abs_diff_eq!(a, &b, epsilon = TEST_TOL));
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

        y.iter()
            .zip(y_expected.iter())
            .for_each(|(a, b)| assert_abs_diff_eq!(a, &b, epsilon = TEST_TOL_FTWINDOW));
    }
}
