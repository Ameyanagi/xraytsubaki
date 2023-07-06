//! EXAFS modules
//!
//!

// Tests are stored in separate tests module
#[cfg(tests)]
mod tests;

// External dependencies
use enterpolation::{
    linear::{Linear, LinearError},
    Curve, Generator,
};
use ndarray::Array1;

// load dependencies
pub mod background;
pub mod normalization;
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
    let lin = Linear::builder().elements(x).knots(y).build()?;
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
        self.raw_energy = Some(energy.into());
        self.raw_mu = Some(mu.into());
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
        Ok(())
    }

    pub fn set_e0<S: Into<f64>>(&mut self, e0: S) {
        self.e0 = Some(e0.into());
    }
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
}
