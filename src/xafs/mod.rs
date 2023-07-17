//! EXAFS modules
//!
//!

// Tests are stored in separate tests module
#[cfg(tests)]
mod tests;

#[cfg_attr(debug_assertions, allow(dead_code, unused_imports))]
// Standard library dependencies
use std::cmp;

// External dependencies
use enterpolation::{
    linear::{Linear, LinearError},
    Curve, Generator,
};
use ndarray::{Array, Array1, ArrayBase};

// load dependencies
pub mod background;
pub mod normalization;
pub mod xafsutils;
pub mod xrayfft;

/// XASGroup is a struct that contains all the data and parameters for a single XAS spectrum.
///
/// # Examples
///
/// TODO: Add examples
#[derive(Debug, Clone, PartialEq)]
pub struct XASGroup {
    pub name: Option<String>,
    pub raw_energy: Option<Array1<f64>>,
    pub raw_mu: Option<Array1<f64>>,
    pub energy: Option<Array1<f64>>,
    pub mu: Option<Array1<f64>>,
    pub e0: Option<f64>,
    pub norm: Option<Array1<f64>>,
    pub flat: Option<Array1<f64>>,
    pub k: Option<Array1<f64>>,
    pub chi: Option<Array1<f64>>,
    pub chi_kweighted: Option<Array1<f64>>,
    pub chi_r: Option<Array1<f64>>,
    pub chi_r_mag: Option<Array1<f64>>,
    pub chi_r_re: Option<Array1<f64>>,
    pub chi_r_im: Option<Array1<f64>>,
    pub q: Option<Array1<f64>>,
    pub normalization: Option<normalization::NormalizationMethod>,
    pub background: Option<background::BackgroundMethod>,
    pub xftf: Option<xrayfft::XrayForwardFFT>,
    pub xftr: Option<xrayfft::XrayReverseFFT>,
}

impl Default for XASGroup {
    fn default() -> Self {
        XASGroup {
            name: None,
            raw_energy: None,
            raw_mu: None,
            energy: None,
            mu: None,
            e0: None,
            norm: None,
            flat: None,
            k: None,
            chi: None,
            chi_kweighted: None,
            chi_r: None,
            chi_r_mag: None,
            chi_r_re: None,
            chi_r_im: None,
            q: None,
            normalization: None,
            background: None,
            xftf: None,
            xftr: None,
        }
    }
}

pub fn interpolation(x: &Vec<f64>, y: &Vec<f64>, xnew: &Vec<f64>) -> Result<Vec<f64>, LinearError> {
    let lin = Linear::builder().elements(y).knots(x).build()?;
    let result: Vec<f64> = lin.sample(xnew.clone()).collect();

    Ok(result)
}

impl XASGroup {
    pub fn new() -> XASGroup {
        XASGroup::default()
    }

    pub fn set_name<S: Into<String>>(&mut self, name: S) {
        self.name = Some(name.into());
    }

    pub fn set_spectrum<T: Into<Array1<f64>>, M: Into<Array1<f64>>>(&mut self, energy: T, mu: M) {
        let raw_energy = energy.into();
        let raw_mu = mu.into();

        if !xafsutils::is_sorted(&raw_energy) {
            let sort_idx = xafsutils::argsort(&raw_energy.to_vec());
            self.raw_energy = Some(raw_energy.select(ndarray::Axis(0), &sort_idx));
            self.raw_mu = Some(raw_mu.select(ndarray::Axis(0), &sort_idx));
        } else {
            self.raw_energy = Some(raw_energy);
            self.raw_mu = Some(raw_mu);
        }
        self.energy = self.raw_energy.clone();
        self.mu = self.raw_mu.clone();
    }

    pub fn interpolate_spectrum<T: Into<Array1<f64>>>(
        &mut self,
        energy: T,
    ) -> Result<(), LinearError> {
        self.energy = Some(energy.into());

        let energy = self.energy.clone().unwrap().to_vec();
        let mu = self.raw_mu.clone().unwrap().to_vec();
        let knot = self.raw_energy.clone().unwrap().to_vec();

        println!("energy: {:?}", energy);
        println!("mu: {:?}", mu);

        // let lin = Linear::builder().elements(&mu).knots(&knot).build()?;

        // let result = lin.slice(energy);

        self.mu = Some(Array1::from_vec(interpolation(&knot, &mu, &energy)?));

        Ok(())
    }

    pub fn set_e0<S: Into<f64>>(&mut self, e0: S) {
        self.e0 = Some(e0.into());
    }

    pub fn find_e0(&mut self) {
        // todo!("Implement find_e0");
        // let mut energy = self.energy.clone().unwrap();
        // let mut mu = self.mu.clone().unwrap();

        // if !xafsutils::is_sorted(&energy) {
        //     let sort_idx = xafsutils::argsort(&energy.to_vec());
        //     // energy = energy.slice(s![sort_idx]);
        //     energy = energy.select(ndarray::Axis(0), &sort_idx);
        // }

        // println!("energy: {:?}", energy);

        // let mut e0 = 0.0;
        // let mut max = 0.0;

        // for i in 0..energy.len() {
        //     if mu[i] > max {
        //         max = mu[i];
        //         e0 = energy[i];
        //     }
        // }

        // self.e0 = Some(e0);
    }

    fn find_energy_step(&mut self, frac_ignore: Option<f64>, nave: Option<i32>) {
        todo!("Implement find_energy_step");

        // let frac_ignore = frac_ignore.unwrap_or(0.01);
        // let nave = nave.unwrap_or(10);

        // ediff = self.energy.unwrap()[1:] - self.energy.unwrap()[:-1];
        // nskip = frac_ignore*self.energy.unwrap().len();
    }

    fn _find_e0(&mut self, estep: Option<f64>, use_smooth: Option<bool>) {
        todo!("Implement find_e0");

        // let estep = estep.unwrap_or(0.1);

        // let energy = self.energy.clone().unwrap();
        // let mu = self.mu.clone().unwrap();

        // let mut e0 = 0.0;
        // let mut max = 0.0;

        // for i in 0..energy.len() {
        //     if mu[i] > max {
        //         max = mu[i];
        //         e0 = energy[i];
        //     }
        // }

        // self.e0 = Some(e0);
    }
}

pub fn remove_dups<T: Into<Array1<f64>>>(
    arr: T,
    tiny: Option<f64>,
    frac: Option<f64>,
    sort: Option<bool>,
) -> Array1<f64> {
    // Function to remove duplicated successive values of an array that is expected to be monotonically increasing.
    //
    // For repeated value, the second encountered occurrence (at index i) will be increased by an amount that is the larget of:
    // 1. tiny (default 1e-7)
    // 2. frac (default 1e-6) times the difference between the previous and next values.
    //
    // # Arguments
    // * `arr` - Array of values to be checked for duplicates
    // * `tiny` - Minimum value to be added to a duplicate value (default 1e-7)
    // * `frac` - Fraction of the difference between the previous and next values to be added to a duplicate value (default 1e-6)
    //
    // # Returns
    // * `arr` - Array with duplicates removed
    //
    // # Example
    // ```
    // use xas_tools::utils::remove_dups;
    // use ndarray::array;
    //
    // let arr = Array1::from_vec(vec![0.0, 1.1, 2.2, 2.2, 3.3]);
    // let arr = remove_dups(arr, None, None, None);
    // assert_eq!(arr, Array1::from_vec(vec![0., 1.1, 2.2, 2.2000001, 3.3]));
    // ```

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

pub fn find_energy_step<T: Into<Array1<f64>>>(
    energy: T,
    frac_ignore: Option<f64>,
    nave: Option<usize>,
    sort: Option<bool>,
) -> f64 {
    // Function to find the energy step of an array of energies.
    // It ignores the smallest fraction of energy steps (frac_ignore) and then averages the next nave steps.
    //
    // # Arguments
    // * `energy` - Array of energies
    // * `frac_ignore` - Fraction of energy steps to ignore (default 0.01)
    // * `nave` - Number of energy steps to average (default 10)
    // * `sort` - Sort the array before finding the energy step (default false)
    //
    // # Returns
    // * `estep` - Average energy step
    //
    // # Example
    // ```
    // use xas_tools::utils::find_energy_step;
    // use ndarray::array;
    //
    // let energy = array![0.0, 1.1, 2.2, 2.2, 3.3];
    // let estep = find_energy_step(energy, None, None, None);
    // assert_eq!(estep, 1.1);
    // ```

    let mut energy = energy.into();

    if let Some(true) = sort {
        let mut energy_sort = energy.to_vec();
        energy_sort.sort_by(|a, b| a.partial_cmp(b).unwrap());
        energy = Array1::from_vec(energy_sort);
    }

    let frac_ignore = frac_ignore.unwrap_or(0.01);
    let nave = nave.unwrap_or(10);
    let mut ediff = &energy.slice(ndarray::s![1..]) - &energy.slice(ndarray::s![..-1]);
    let nskip = (frac_ignore * energy.len() as f64) as usize;

    ediff.to_vec().sort_by(|a, b| a.partial_cmp(b).unwrap());

    let ediff_end = cmp::min(nskip + nave, ediff.len() - 1);

    return ediff.slice(ndarray::s![nskip..ediff_end]).mean().unwrap();
}

pub fn find_e0<T: Into<Array1<f64>>>(energy: T, mu: T) {
    // Calculate the $E_0$, the energy threshold of absoption, or the edge energy, given $\mu(E)$.
    //
    // $E_0$ is found as the point with maximum derivative with some checks to avoid spurious glitches.
    //
    // # Arguments
    // * `energy` - Array of energies
    // * `mu` - Array of absorption coefficients
    todo!("find_e0 not implemented yet")
}

pub fn _find_e0<T: Into<Array1<f64>>>(
    energy: T,
    mu: T,
    estep: Option<f64>,
    use_smooth: Option<bool>,
) {
    // Internal function used for find_e0.
    //
    // # Arguments
    // * `energy` - Array of energies
    // * `mu` - Array of absorption coefficients
    // * `estep` - Energy step (default: find_energy_step(energy)/2.0)

    let energy = remove_dups(energy.into(), None, None, None);
    let mu = mu.into();

    if let Some(estep) = estep {
        let estep = estep;
    } else {
        let estep = find_energy_step(energy.clone(), None, None, Some(false)) / 2.0;
    }

    let nmin = cmp::max(2, &energy.len() / 100);

    if let Some(true) = use_smooth {
        todo!("smooth not implemented yet");
        // let mu = smooth(mu, Some(3), Some(0.0), Some(0.0), Some(true));
    } else {
        todo!("deriv not implemented yet");
        // let dmu =
    }

    // let mut e0 = 0.0;
    // let mut max = 0.0;

    // for i in 0..energy.len() {
    //     if mu[i] > max {
    //         max = mu[i];
    //         e0 = energy[i];
    //     }
    // }

    // self.e0 = Some(e0);
}

// Simple unit tests for this file.

#[cfg(test)]

mod tests {
    use enterpolation::Sorted;

    use super::*;

    #[test]
    fn test_xafs_group_name_from_string() {
        let mut xafs_group = XASGroup::new();
        xafs_group.set_name("test".to_string());
        assert_eq!(xafs_group.name, Some("test".to_string()));
    }

    #[test]
    fn test_xafs_group_name_from_str() {
        let mut xafs_group = XASGroup::new();
        xafs_group.set_name("test");
        assert_eq!(xafs_group.name, Some("test".to_string()));

        let name = String::from("test");

        let mut xafs_group = XASGroup::new();
        xafs_group.set_name(name.clone());
        assert_eq!(xafs_group.name, Some("test".to_string()));

        println!("name: {}", name);
    }

    #[test]
    fn test_xafs_group_spectrum_from_vec() {
        let energy: Vec<f64> = vec![1.0, 2.0, 3.0];
        let mu: Array1<f64> = Array1::from_vec(vec![4.0, 5.0, 6.0]);
        let mut xafs_group = XASGroup::new();
        xafs_group.set_spectrum(energy, mu);
        assert_eq!(
            xafs_group.raw_energy,
            Some(Array1::from_vec(vec![1.0, 2.0, 3.0]))
        );
        assert_eq!(
            xafs_group.raw_mu,
            Some(Array1::from_vec(vec![4.0, 5.0, 6.0]))
        );
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
}
