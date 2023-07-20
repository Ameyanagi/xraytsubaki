use super::io;
use super::mathutils::MathUtils;

use fftconvolve::{fftconvolve, Mode};
use ndarray::{Array, Array1, ArrayBase, Axis, Ix1, OwnedRepr, Slice};
use std::cmp;
use std::error::Error;

const TINY_ENERGY: f64 = 0.005;

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
    println!("e1: {}, ie0: {}, estep: {}", e1, ie0, estep);
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

#[cfg(test)]
mod tests {
    use super::*;
    use data_reader::reader::{load_txt_f64, Delimiter, ReaderParams};
    use std::fs::File;
    use std::io::prelude::*;

    const TOP_DIR: &'static str = env!("CARGO_MANIFEST_DIR");

    const param_loadtxt: ReaderParams = ReaderParams {
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

        let expected_data = load_txt_f64(&expected_filepath, &param_loadtxt)?;
        let expected_data = expected_data.get_col(0);

        let expected_data_larch = load_txt_f64(&expected_filepath_larch, &param_loadtxt)?;
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
}
