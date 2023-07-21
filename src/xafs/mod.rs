//! EXAFS modules
//!
//!

// Tests are stored in separate tests module
#[cfg(tests)]
mod tests;

#[cfg_attr(debug_assertions, allow(dead_code, unused_imports))]
// Standard library dependencies
use std::cmp;
use std::error::Error;

// External dependencies
use ndarray::{Array1, ArrayBase, Ix1, OwnedRepr};

// load dependencies
pub mod background;
pub mod io;
pub mod mathutils;
pub mod normalization;
pub mod xafsutils;
pub mod xrayfft;

// Load local traits
use mathutils::MathUtils;

/// XASGroup is a struct that contains all the data and parameters for a single XAS spectrum.
///
/// # Examples
///
/// TODO: Add examples
#[derive(Debug, Clone, PartialEq)]
pub struct XASGroup {
    pub name: Option<String>,
    pub raw_energy: Option<ArrayBase<OwnedRepr<f64>, Ix1>>,
    pub raw_mu: Option<ArrayBase<OwnedRepr<f64>, Ix1>>,
    pub energy: Option<ArrayBase<OwnedRepr<f64>, Ix1>>,
    pub mu: Option<ArrayBase<OwnedRepr<f64>, Ix1>>,
    pub e0: Option<f64>,
    pub norm: Option<ArrayBase<OwnedRepr<f64>, Ix1>>,
    pub flat: Option<ArrayBase<OwnedRepr<f64>, Ix1>>,
    pub k: Option<ArrayBase<OwnedRepr<f64>, Ix1>>,
    pub chi: Option<ArrayBase<OwnedRepr<f64>, Ix1>>,
    pub chi_kweighted: Option<ArrayBase<OwnedRepr<f64>, Ix1>>,
    pub chi_r: Option<ArrayBase<OwnedRepr<f64>, Ix1>>,
    pub chi_r_mag: Option<ArrayBase<OwnedRepr<f64>, Ix1>>,
    pub chi_r_re: Option<ArrayBase<OwnedRepr<f64>, Ix1>>,
    pub chi_r_im: Option<ArrayBase<OwnedRepr<f64>, Ix1>>,
    pub q: Option<ArrayBase<OwnedRepr<f64>, Ix1>>,
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

impl XASGroup {
    pub fn new() -> XASGroup {
        XASGroup::default()
    }

    pub fn set_name<S: Into<String>>(&mut self, name: S) {
        self.name = Some(name.into());
    }

    pub fn set_spectrum<
        T: Into<ArrayBase<OwnedRepr<f64>, Ix1>>,
        M: Into<ArrayBase<OwnedRepr<f64>, Ix1>>,
    >(
        &mut self,
        energy: T,
        mu: M,
    ) {
        let raw_energy = energy.into();
        let raw_mu = mu.into();

        if !raw_energy.is_sorted() {
            let sort_idx = raw_energy.argsort();
            self.raw_energy = Some(raw_energy.select(ndarray::Axis(0), &sort_idx));
            self.raw_mu = Some(raw_mu.select(ndarray::Axis(0), &sort_idx));
        } else {
            self.raw_energy = Some(raw_energy);
            self.raw_mu = Some(raw_mu);
        }
        self.energy = self.raw_energy.clone();
        self.mu = self.raw_mu.clone();
    }

    pub fn interpolate_spectrum<T: Into<ArrayBase<OwnedRepr<f64>, Ix1>>>(
        &mut self,
        energy: T,
    ) -> Result<(), Box<dyn Error>> {
        self.energy = Some(energy.into());

        let energy = self.energy.clone().unwrap();
        let mu = self.raw_mu.clone().unwrap().to_vec();
        let knot = self.raw_energy.clone().unwrap().to_vec();

        self.mu = Some(energy.interpolate(&knot, &mu).unwrap());

        Ok(())
    }

    pub fn set_e0<S: Into<f64>>(&mut self, e0: S) {
        self.e0 = Some(e0.into());
    }

    pub fn find_e0(&mut self) -> Result<(), Box<dyn Error>> {
        self.e0 = Some(xafsutils::find_e0(
            self.energy.clone().unwrap(),
            self.mu.clone().unwrap(),
        )?);

        Ok(())
    }

    fn find_energy_step(&mut self, frac_ignore: Option<f64>, nave: Option<usize>) -> f64 {
        let energy = self.energy.clone().unwrap();
        xafsutils::find_energy_step(energy, frac_ignore, nave, None)
    }
}

pub enum XAFSError {
    NotEnoughData,
}

// Simple unit tests for this file.

#[cfg(test)]

mod tests {

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
        let mu: ArrayBase<OwnedRepr<f64>, Ix1> = Array1::from_vec(vec![4.0, 5.0, 6.0]);
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
}
