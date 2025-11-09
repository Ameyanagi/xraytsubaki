#![allow(dead_code)]
#![allow(unused_imports)]

use std::borrow::Borrow;
#[cfg_attr(debug_assertions, allow(dead_code, unused_imports))]
// Standard library dependencies
use std::error::Error;

// External dependencies
use easyfft::dyn_size::realfft::DynRealDft;
use nalgebra::DVector;
#[cfg(feature = "ndarray-compat")]
use ndarray::{ArrayBase, Axis, Ix1, OwnedRepr, ViewRepr};
use serde::{Deserialize, Serialize};

// load dependencies
use super::background;
use super::errors::DataError;
use super::io;
use super::lmutils;
use super::mathutils;
use super::normalization;
use super::nshare;
use super::xafsutils;
use super::xrayfft;
use super::XAFSError;

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
#[derive(Default)]
pub struct XASSpectrum {
    pub name: Option<String>,
    pub raw_energy: Option<DVector<f64>>,
    pub raw_mu: Option<DVector<f64>>,
    pub energy: Option<DVector<f64>>,
    pub mu: Option<DVector<f64>>,
    pub e0: Option<f64>,
    pub k: Option<DVector<f64>>,
    pub chi: Option<DVector<f64>>,
    pub chi_kweighted: Option<DVector<f64>>,
    pub chi_r: Option<DVector<f64>>,
    pub chi_r_mag: Option<DVector<f64>>,
    pub chi_r_re: Option<DVector<f64>>,
    pub chi_r_im: Option<DVector<f64>>,
    pub q: Option<DVector<f64>>,
    pub normalization: Option<normalization::NormalizationMethod>,
    pub background: Option<background::BackgroundMethod>,
    pub xftf: Option<xrayfft::XrayFFTF>,
    pub xftr: Option<xrayfft::XrayFFTR>,
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
        T: Into<DVector<f64>>,
        M: Into<DVector<f64>>,
    >(
        &mut self,
        energy: T,
        mu: M,
    ) -> &mut Self {
        let raw_energy = energy.into();
        let raw_mu = mu.into();

        if !raw_energy.is_sorted() {
            let sort_idx = raw_energy.argsort();
            // For DVector, we need to manually sort by indices
            self.raw_energy = Some(DVector::from_iterator(
                sort_idx.len(),
                sort_idx.iter().map(|&i| raw_energy[i])
            ));
            self.raw_mu = Some(DVector::from_iterator(
                sort_idx.len(),
                sort_idx.iter().map(|&i| raw_mu[i])
            ));
        } else {
            self.raw_energy = Some(raw_energy);
            self.raw_mu = Some(raw_mu);
        }
        self.energy = self.raw_energy.clone();
        self.mu = self.raw_mu.clone();

        self
    }

    pub fn interpolate_spectrum<T: Into<DVector<f64>>>(
        &mut self,
        energy: T,
    ) -> Result<&mut Self, XAFSError> {
        self.energy = Some(energy.into());

        let energy = self.energy.clone().unwrap();
        let mu = self.raw_mu.clone().unwrap().data.as_vec().to_vec();
        let knot = self.raw_energy.clone().unwrap().data.as_vec().to_vec();

        self.mu = Some(energy.interpolate(&knot, &mu).unwrap());

        Ok(self)
    }

    pub fn set_e0<S: Into<f64>>(&mut self, e0: S) -> &mut Self {
        self.e0 = Some(e0.into());

        self
    }

    pub fn find_e0(&mut self) -> Result<&mut Self, XAFSError> {
        let energy = self.energy.as_ref().unwrap();
        let mu = self.mu.as_ref().unwrap();
        self.e0 = Some(xafsutils::find_e0(energy, mu)?);

        Ok(self)
    }

    fn find_energy_step(&mut self, frac_ignore: Option<f64>, nave: Option<usize>) -> f64 {
        let energy = self.energy.as_ref().unwrap();
        xafsutils::find_energy_step(energy, frac_ignore, nave, None)
    }

    pub fn set_normalization_method(
        &mut self,
        method: Option<normalization::NormalizationMethod>,
    ) -> Result<&mut Self, XAFSError> {
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

    pub fn normalize(&mut self) -> Result<&mut Self, XAFSError> {
        if self.normalization.is_none() {
            self.set_normalization_method(None)?;
        }

        // Convert DVector to Array1 for normalization (temporary until trait is migrated)
        #[cfg(feature = "ndarray-compat")]
        {
            use ndarray::Array1;
            let energy = Array1::from_vec(self.energy.as_ref().unwrap().data.as_vec().clone());
            let mu = Array1::from_vec(self.mu.as_ref().unwrap().data.as_vec().clone());

            self.normalization
                .as_mut()
                .unwrap()
                .normalize(&energy, &mu)?;
        }

        Ok(self)
    }

    pub fn set_background_method(
        &mut self,
        method: Option<background::BackgroundMethod>,
    ) -> Result<&mut Self, XAFSError> {
        if let Some(method) = method {
            self.background = Some(method);
        } else {
            let backgound_method = background::AUTOBK::new();
            self.background = Some(background::BackgroundMethod::AUTOBK(backgound_method));
        }

        Ok(self)
    }

    pub fn calc_background(&mut self) -> Result<&mut Self, XAFSError> {
        if self.background.is_none() {
            self.set_background_method(None)?;
        }

        let energy = self.energy.as_ref().unwrap();
        let mu = self.mu.as_ref().unwrap();

        self.background
            .as_mut()
            .unwrap()
            .calc_background(energy, mu, &mut self.normalization)?;

        Ok(self)
    }

    pub fn fft(&mut self) -> Result<&mut Self, XAFSError> {
        let k = self.get_k();
        let chi = self.get_chi();

        if k.is_none() || chi.is_none() {
            return Err(DataError::MissingData {
                field: "k and chi (need to calculate background first)".to_string(),
            }.into());
        }

        let k = k.unwrap();
        let chi = chi.unwrap();

        if self.xftf.is_none() {
            self.xftf = Some(xrayfft::XrayFFTF::new());
        }

        // Convert DVector to Array1 for FFT (temporary until xrayfft is migrated)
        #[cfg(feature = "ndarray-compat")]
        {
            use ndarray::Array1;
            let k_array = Array1::from_vec(k.data.as_vec().clone());
            let chi_array = Array1::from_vec(chi.data.as_vec().clone());
            self.xftf.as_mut().unwrap().xftf(k_array.view(), chi_array.view());
        }

        Ok(self)
    }

    pub fn ifft(&mut self) -> Result<&mut Self, XAFSError> {
        if self.xftf.is_none() {
            return Err(DataError::MissingData {
                field: "xftf (need to run fft() first)".to_string(),
            }.into());
        }

        let r = self.xftf.as_ref().unwrap().get_r();
        let chi_r = self.xftf.as_ref().unwrap().get_chir();

        if r.is_none() || chi_r.is_none() {
            return Err(DataError::MissingData {
                field: "r and chi_r (fft() may have failed)".to_string(),
            }.into());
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

    pub fn get_k(&self) -> Option<DVector<f64>> {
        self.background.as_ref()?.get_k()
    }

    pub fn get_chi(&self) -> Option<DVector<f64>> {
        self.background.as_ref()?.get_chi()
    }

    pub fn get_kweight(&self) -> Option<&f64> {
        self.xftf.as_ref()?.get_kweight()
    }

    pub fn get_chi_kweighted(&self) -> Option<DVector<f64>> {
        let k = self.get_k()?;
        let chi = self.get_chi()?;
        let kweight = self.get_kweight()?;

        Some(chi.component_mul(&k.map(|x| x.powf(kweight.to_owned()))))
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
        let mu: Vec<f64> = vec![4.0, 5.0, 6.0];
        let mut xafs_group = XASSpectrum::new();
        xafs_group.set_spectrum(energy, mu);
        assert_eq!(
            xafs_group.raw_energy,
            Some(DVector::from_vec(vec![1.0, 2.0, 3.0]))
        );
        assert_eq!(
            xafs_group.raw_mu,
            Some(DVector::from_vec(vec![4.0, 5.0, 6.0]))
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
