#![allow(dead_code)]
#![allow(unused_imports)]

use std::borrow::Borrow;
#[cfg_attr(debug_assertions, allow(dead_code, unused_imports))]
// Standard library dependencies
use std::error::Error;

// External dependencies
use easyfft::dyn_size::realfft::DynRealDft;
use ndarray::{ArrayBase, Axis, Ix1, OwnedRepr, ViewRepr};
use serde::{Deserialize, Serialize};

// load dependencies
use super::background;
use super::io;
use super::lmutils;
use super::mathutils;
use super::normalization;
use super::nshare;
use super::xafsutils;
use super::xrayfft;

// Load local traits
use mathutils::MathUtils;
use normalization::Normalization;

/// XASGroup is a struct that contains all the data and parameters for a single XAS spectrum.
///
/// # Examples
///
/// TODO: Add examples
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct XASSpectrum {
    pub name: Option<String>,
    pub raw_energy: Option<ArrayBase<OwnedRepr<f64>, Ix1>>,
    pub raw_mu: Option<ArrayBase<OwnedRepr<f64>, Ix1>>,
    pub energy: Option<ArrayBase<OwnedRepr<f64>, Ix1>>,
    pub mu: Option<ArrayBase<OwnedRepr<f64>, Ix1>>,
    pub e0: Option<f64>,
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
    pub xftf: Option<xrayfft::XrayFFTF>,
    pub xftr: Option<xrayfft::XrayFFTR>,
}

impl Default for XASSpectrum {
    fn default() -> Self {
        XASSpectrum {
            name: None,
            raw_energy: None,
            raw_mu: None,
            energy: None,
            mu: None,
            e0: None,
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

impl XASSpectrum {
    pub fn new() -> XASSpectrum {
        XASSpectrum::default()
    }

    pub fn set_name<S: Into<String>>(&mut self, name: S) -> &mut Self {
        self.name = Some(name.into());
        self
    }

    pub fn set_spectrum<
        T: Into<ArrayBase<OwnedRepr<f64>, Ix1>>,
        M: Into<ArrayBase<OwnedRepr<f64>, Ix1>>,
    >(
        &mut self,
        energy: T,
        mu: M,
    ) -> &mut Self {
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

        self
    }

    pub fn interpolate_spectrum<T: Into<ArrayBase<OwnedRepr<f64>, Ix1>>>(
        &mut self,
        energy: T,
    ) -> Result<&mut Self, Box<dyn Error>> {
        self.energy = Some(energy.into());

        let energy = self.energy.clone().unwrap();
        let mu = self.raw_mu.clone().unwrap().to_vec();
        let knot = self.raw_energy.clone().unwrap().to_vec();

        self.mu = Some(energy.interpolate(&knot, &mu).unwrap());

        Ok(self)
    }

    pub fn set_e0<S: Into<f64>>(&mut self, e0: S) -> &mut Self {
        self.e0 = Some(e0.into());

        self
    }

    pub fn find_e0(&mut self) -> Result<&mut Self, Box<dyn Error>> {
        self.e0 = Some(xafsutils::find_e0(
            self.energy.clone().unwrap(),
            self.mu.clone().unwrap(),
        )?);

        Ok(self)
    }

    fn find_energy_step(&mut self, frac_ignore: Option<f64>, nave: Option<usize>) -> f64 {
        let energy = self.energy.clone().unwrap();
        xafsutils::find_energy_step(energy, frac_ignore, nave, None)
    }

    pub fn set_normalization_method(
        &mut self,
        method: Option<normalization::NormalizationMethod>,
    ) -> Result<&mut Self, Box<dyn Error>> {
        if let Some(method) = method {
            self.normalization = Some(method);
        } else {
            let normalization_method = normalization::PrePostEdge::new();
            self.normalization = Some(normalization::NormalizationMethod::PrePostEdge(
                normalization_method,
            ));
        }

        let e0 = self.e0;
        self.normalization.as_mut().unwrap().set_e0(e0);

        Ok(self)
    }

    pub fn normalize(&mut self) -> Result<&mut Self, Box<dyn Error>> {
        if self.normalization.is_none() {
            self.set_normalization_method(None)?;
        }

        let energy = self.energy.clone().unwrap();
        let mu = self.mu.clone().unwrap();

        self.normalization
            .as_mut()
            .unwrap()
            .normalize(&energy, &mu)?;

        Ok(self)
    }

    pub fn set_background_method(
        &mut self,
        method: Option<background::BackgroundMethod>,
    ) -> Result<&mut Self, Box<dyn Error>> {
        if let Some(method) = method {
            self.background = Some(method);
        } else {
            let backgound_method = background::AUTOBK::new();
            self.background = Some(background::BackgroundMethod::AUTOBK(backgound_method));
        }

        Ok(self)
    }

    pub fn calc_background(&mut self) -> Result<&mut Self, Box<dyn Error>> {
        if self.background.is_none() {
            self.set_background_method(None)?;
        }

        let energy = self.energy.clone().unwrap();
        let mu = self.mu.clone().unwrap();

        self.background
            .as_mut()
            .unwrap()
            .calc_background(&energy, &mu, &mut self.normalization)?;

        Ok(self)
    }

    pub fn fft(&mut self) -> Result<&mut Self, Box<dyn Error>> {
        let k = self.get_k();
        let chi = self.get_chi();

        if k.is_none() || chi.is_none() {
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Need to calculate k and chi first",
            )));
        }

        let k = k.unwrap();
        let chi = chi.unwrap();

        if self.xftf.is_none() {
            self.xftf = Some(xrayfft::XrayFFTF::new());
        }

        self.xftf.as_mut().unwrap().xftf(k.view(), chi.view());

        Ok(self)
    }

    pub fn ifft(&mut self) -> Result<&mut Self, Box<dyn Error>> {
        if self.xftf.is_none() {
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Please provide r and chi_r - run fft() first",
            )));
        }

        let r = self.xftf.as_ref().unwrap().get_r();
        let chi_r = self.xftf.as_ref().unwrap().get_chir();

        if r.is_none() || chi_r.is_none() {
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Need to calculate r and chi_r first",
            )));
        }

        let r = r.unwrap();
        let chi_r = chi_r.unwrap();

        if self.xftr.is_none() {
            self.xftr = Some(xrayfft::XrayFFTR::new());
        }

        self.xftr.as_mut().unwrap().xftr(r.view(), chi_r);

        Ok(self)
    }

    pub fn get_e0(&self) -> Option<f64> {
        self.e0
    }

    pub fn get_k(&self) -> Option<ArrayBase<OwnedRepr<f64>, Ix1>> {
        self.background.as_ref()?.get_k()
    }

    pub fn get_chi(&self) -> Option<ArrayBase<OwnedRepr<f64>, Ix1>> {
        self.background.as_ref()?.get_chi()
    }

    pub fn get_kweight(&self) -> Option<&f64> {
        self.xftf.as_ref()?.get_kweight()
    }

    pub fn get_chi_kweighted(&self) -> Option<ArrayBase<OwnedRepr<f64>, Ix1>> {
        let k = self.get_k()?;
        let chi = self.get_chi()?;
        let kweight = self.get_kweight()?;

        Some(chi * k.mapv(|x| x.powf(kweight.to_owned())))
    }

    pub fn get_chir(&self) -> Option<&DynRealDft<f64>> {
        self.xftf.as_ref()?.get_chir()
    }

    pub fn get_chir_mag(&self) -> Option<ArrayBase<ViewRepr<&f64>, Ix1>> {
        self.xftf.as_ref()?.get_chir_mag()
    }

    pub fn get_chir_real(&self) -> Option<ArrayBase<OwnedRepr<f64>, Ix1>> {
        self.xftf.as_ref()?.get_chir_real()
    }

    pub fn get_chir_imag(&self) -> Option<ArrayBase<OwnedRepr<f64>, Ix1>> {
        self.xftf.as_ref()?.get_chir_imag()
    }

    pub fn get_r(&self) -> Option<ArrayBase<ViewRepr<&f64>, Ix1>> {
        self.xftf.as_ref()?.get_r()
    }

    pub fn get_q(&self) -> Option<ArrayBase<ViewRepr<&f64>, Ix1>> {
        self.xftr.as_ref()?.get_q()
    }

    pub fn get_chiq(&self) -> Option<ArrayBase<OwnedRepr<f64>, Ix1>> {
        self.xftr.as_ref()?.get_chiq()
    }
}

pub enum XAFSError {
    NotEnoughData,
    NotEnoughDataForXFTF,
    NotEnoughDataForXFTR,
}

// Simple unit tests for this file.

#[cfg(test)]
pub mod tests {

    use super::*;
    use crate::xafs::io;
    use crate::xafs::tests::PARAM_LOADTXT;
    use crate::xafs::tests::TEST_TOL;
    use crate::xafs::tests::TEST_TOL_LESS_ACC;
    use crate::xafs::tests::TOP_DIR;
    use data_reader::reader::{load_txt_f64, Delimiter, ReaderParams};
    use ndarray::{Array1, ArrayBase, Ix1, OwnedRepr};

    use approx::assert_abs_diff_eq;

    #[test]
    fn test_xafs_group_name_from_string() {
        let mut xafs_group = XASSpectrum::new();
        xafs_group.set_name("test".to_string());
        assert_eq!(xafs_group.name, Some("test".to_string()));
    }

    #[test]
    fn test_xafs_group_name_from_str() {
        let mut xafs_group = XASSpectrum::new();
        xafs_group.set_name("test");
        assert_eq!(xafs_group.name, Some("test".to_string()));

        let name = String::from("test");

        let mut xafs_group = XASSpectrum::new();
        xafs_group.set_name(name.clone());
        assert_eq!(xafs_group.name, Some("test".to_string()));

        println!("name: {}", name);
    }

    #[test]
    fn test_xafs_group_spectrum_from_vec() {
        let energy: Vec<f64> = vec![1.0, 2.0, 3.0];
        let mu: ArrayBase<OwnedRepr<f64>, Ix1> = Array1::from_vec(vec![4.0, 5.0, 6.0]);
        let mut xafs_group = XASSpectrum::new();
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
    fn test_xafs_group_normalization() {
        let test_file = String::from(TOP_DIR) + "/tests/testfiles/Ru_QAS.dat";
        let mut xafs_group = io::load_spectrum_QAS_trans(&test_file).unwrap();

        let _ = xafs_group.normalize();

        let reference_path =
            String::from(TOP_DIR) + "/tests/testfiles/Ru_QAS_pre_post_edge_expected.dat";
        let reference = load_txt_f64(&reference_path, &PARAM_LOADTXT).unwrap();

        let expected_norm = reference.get_col(4);

        xafs_group
            .normalization
            .unwrap()
            .get_norm()
            .to_owned()
            .unwrap()
            .to_vec()
            .iter()
            .zip(expected_norm.iter())
            .for_each(|(x, y)| assert_abs_diff_eq!(x, y, epsilon = TEST_TOL_LESS_ACC));
    }
}
