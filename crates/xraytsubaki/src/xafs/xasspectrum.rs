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
use ndarray::{ArrayBase, Ix1, ViewRepr};
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
    fn validate_energy_mu_inputs(
        energy: &DVector<f64>,
        mu: &DVector<f64>,
    ) -> Result<(), XAFSError> {
        if energy.len() != mu.len() {
            return Err(DataError::LengthMismatch {
                energy_len: energy.len(),
                mu_len: mu.len(),
            }
            .into());
        }
        if energy.len() < 2 {
            return Err(DataError::InsufficientData {
                min: 2,
                actual: energy.len(),
            }
            .into());
        }

        let non_finite = energy
            .iter()
            .zip(mu.iter())
            .enumerate()
            .filter_map(|(index, (e, m))| (!e.is_finite() || !m.is_finite()).then_some(index))
            .collect::<Vec<_>>();
        if !non_finite.is_empty() {
            return Err(DataError::NonFiniteValues {
                indices: non_finite,
            }
            .into());
        }

        for index in 1..energy.len() {
            let prev = energy[index - 1];
            let curr = energy[index];
            if curr < prev {
                return Err(DataError::NonMonotonicEnergy { index, prev, curr }.into());
            }
        }

        Ok(())
    }

    pub fn new() -> XASSpectrum {
        XASSpectrum::default()
    }

    pub fn set_name<S: Into<String>>(&mut self, name: S) -> &mut Self {
        self.name = Some(name.into());
        self
    }

    pub fn set_spectrum<T: Into<DVector<f64>>, M: Into<DVector<f64>>>(
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
                sort_idx.iter().map(|&i| raw_energy[i]),
            ));
            self.raw_mu = Some(DVector::from_iterator(
                sort_idx.len(),
                sort_idx.iter().map(|&i| raw_mu[i]),
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
        let energy = energy.into();
        let mu = self.raw_mu.as_ref().ok_or_else(|| DataError::MissingData {
            field: "raw_mu".to_string(),
        })?;
        let knot = self.raw_energy.as_ref().ok_or_else(|| DataError::MissingData {
            field: "raw_energy".to_string(),
        })?;

        let interpolated = energy.interpolate(knot.as_slice(), mu.as_slice()).map_err(|e| {
            super::errors::MathError::SplineEvalFailed {
                x: 0.0,
                reason: e.to_string(),
            }
        })?;
        self.energy = Some(energy);
        self.mu = Some(interpolated);

        Ok(self)
    }

    pub fn set_e0<S: Into<f64>>(&mut self, e0: S) -> &mut Self {
        self.e0 = Some(e0.into());

        self
    }

    pub fn find_e0(&mut self) -> Result<&mut Self, XAFSError> {
        let energy = self.energy.as_ref().ok_or_else(|| DataError::MissingData {
            field: "energy".to_string(),
        })?;
        let mu = self.mu.as_ref().ok_or_else(|| DataError::MissingData {
            field: "mu".to_string(),
        })?;
        Self::validate_energy_mu_inputs(energy, mu)?;
        self.e0 = Some(xafsutils::find_e0(energy, mu)?);

        Ok(self)
    }

    fn find_energy_step(
        &mut self,
        frac_ignore: Option<f64>,
        nave: Option<usize>,
    ) -> Result<f64, XAFSError> {
        let energy = self.energy.as_ref().ok_or_else(|| DataError::MissingData {
            field: "energy".to_string(),
        })?;
        if energy.len() < 2 {
            return Err(DataError::InsufficientData {
                min: 2,
                actual: energy.len(),
            }
            .into());
        }
        let non_finite = energy
            .iter()
            .enumerate()
            .filter_map(|(index, value)| (!value.is_finite()).then_some(index))
            .collect::<Vec<_>>();
        if !non_finite.is_empty() {
            return Err(DataError::NonFiniteValues {
                indices: non_finite,
            }
            .into());
        }
        for index in 1..energy.len() {
            let prev = energy[index - 1];
            let curr = energy[index];
            if curr < prev {
                return Err(DataError::NonMonotonicEnergy { index, prev, curr }.into());
            }
        }

        Ok(xafsutils::find_energy_step(energy, frac_ignore, nave, None))
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
        if let Some(normalization_method) = self.normalization.as_mut() {
            normalization_method.set_e0(e0);
        } else {
            return Err(DataError::MissingData {
                field: "normalization method".to_string(),
            }
            .into());
        }

        Ok(self)
    }

    pub fn normalize(&mut self) -> Result<&mut Self, XAFSError> {
        if self.normalization.is_none() {
            self.set_normalization_method(None)?;
        }

        let energy = self.energy.as_ref().ok_or_else(|| DataError::MissingData {
            field: "energy".to_string(),
        })?;
        let mu = self.mu.as_ref().ok_or_else(|| DataError::MissingData {
            field: "mu".to_string(),
        })?;
        Self::validate_energy_mu_inputs(energy, mu)?;

        self.normalization
            .as_mut()
            .ok_or_else(|| DataError::MissingData {
                field: "normalization method".to_string(),
            })?
            .normalize(energy, mu)?;

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

        let energy = self.energy.as_ref().ok_or_else(|| DataError::MissingData {
            field: "energy".to_string(),
        })?;
        let mu = self.mu.as_ref().ok_or_else(|| DataError::MissingData {
            field: "mu".to_string(),
        })?;
        Self::validate_energy_mu_inputs(energy, mu)?;

        self.background
            .as_mut()
            .ok_or_else(|| DataError::MissingData {
                field: "background method".to_string(),
            })?
            .calc_background(energy, mu, &mut self.normalization)?;

        Ok(self)
    }

    pub fn fft(&mut self) -> Result<&mut Self, XAFSError> {
        let mut xftf = self.xftf.take().unwrap_or_else(xrayfft::XrayFFTF::new);

        #[cfg(feature = "ndarray-compat")]
        {
            let k = self.get_k_view().ok_or_else(|| DataError::MissingData {
                field: "k (need to calculate background first)".to_string(),
            })?;
            let chi = self.get_chi_view().ok_or_else(|| DataError::MissingData {
                field: "chi (need to calculate background first)".to_string(),
            })?;
            xftf.xftf(k, chi)?;
        }
        #[cfg(not(feature = "ndarray-compat"))]
        {
            let k = self.get_k().ok_or_else(|| DataError::MissingData {
                field: "k (need to calculate background first)".to_string(),
            })?;
            let chi = self.get_chi().ok_or_else(|| DataError::MissingData {
                field: "chi (need to calculate background first)".to_string(),
            })?;
            xftf.xftf(&k, &chi)?;
        }

        self.xftf = Some(xftf);
        Ok(self)
    }

    pub fn ifft(&mut self) -> Result<&mut Self, XAFSError> {
        if self.xftf.is_none() {
            return Err(DataError::MissingData {
                field: "xftf (need to run fft() first)".to_string(),
            }
            .into());
        }

        let xftf = self.xftf.as_ref().ok_or_else(|| DataError::MissingData {
            field: "xftf configuration".to_string(),
        })?;

        if self.xftr.is_none() {
            self.xftr = Some(xrayfft::XrayFFTR::new());
        }

        #[cfg(feature = "ndarray-compat")]
        self.xftr
            .as_mut()
            .ok_or_else(|| DataError::MissingData {
                field: "xftr configuration".to_string(),
            })?
            .xftr(
                xftf.get_r().ok_or_else(|| DataError::MissingData {
                    field: "r (fft() may have failed)".to_string(),
                })?,
                xftf.get_chir().ok_or_else(|| DataError::MissingData {
                    field: "chi_r (fft() may have failed)".to_string(),
                })?,
            )?;

        #[cfg(not(feature = "ndarray-compat"))]
        self.xftr
            .as_mut()
            .ok_or_else(|| DataError::MissingData {
                field: "xftr configuration".to_string(),
            })?
            .xftr(
                xftf.get_r().ok_or_else(|| DataError::MissingData {
                    field: "r (fft() may have failed)".to_string(),
                })?,
                xftf.get_chir().ok_or_else(|| DataError::MissingData {
                    field: "chi_r (fft() may have failed)".to_string(),
                })?,
            )?;

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

    #[cfg(feature = "ndarray-compat")]
    pub fn get_k_view(&self) -> Option<ArrayBase<ViewRepr<&f64>, Ix1>> {
        self.background.as_ref()?.get_k_view()
    }

    #[cfg(feature = "ndarray-compat")]
    pub fn get_chi_view(&self) -> Option<ArrayBase<ViewRepr<&f64>, Ix1>> {
        self.background.as_ref()?.get_chi_view()
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

    pub fn get_chir_mag(&self) -> Option<DVector<f64>> {
        #[cfg(feature = "ndarray-compat")]
        {
            self.xftf
                .as_ref()?
                .get_chir_mag()
                .map(|x| DVector::from_vec(x.to_vec()))
        }
        #[cfg(not(feature = "ndarray-compat"))]
        {
            self.xftf.as_ref()?.get_chir_mag().cloned()
        }
    }

    pub fn get_chir_real(&self) -> Option<DVector<f64>> {
        #[cfg(feature = "ndarray-compat")]
        {
            self.xftf
                .as_ref()?
                .get_chir_real()
                .map(|x| DVector::from_vec(x.to_vec()))
        }
        #[cfg(not(feature = "ndarray-compat"))]
        {
            self.xftf.as_ref()?.get_chir_real()
        }
    }

    pub fn get_chir_imag(&self) -> Option<DVector<f64>> {
        #[cfg(feature = "ndarray-compat")]
        {
            self.xftf
                .as_ref()?
                .get_chir_imag()
                .map(|x| DVector::from_vec(x.to_vec()))
        }
        #[cfg(not(feature = "ndarray-compat"))]
        {
            self.xftf.as_ref()?.get_chir_imag()
        }
    }

    pub fn get_r(&self) -> Option<DVector<f64>> {
        #[cfg(feature = "ndarray-compat")]
        {
            self.xftf
                .as_ref()?
                .get_r()
                .map(|x| DVector::from_vec(x.to_vec()))
        }
        #[cfg(not(feature = "ndarray-compat"))]
        {
            self.xftf.as_ref()?.get_r().cloned()
        }
    }

    pub fn get_q(&self) -> Option<DVector<f64>> {
        #[cfg(feature = "ndarray-compat")]
        {
            self.xftr
                .as_ref()?
                .get_q()
                .map(|x| DVector::from_vec(x.to_vec()))
        }
        #[cfg(not(feature = "ndarray-compat"))]
        {
            self.xftr.as_ref()?.get_q().cloned()
        }
    }

    pub fn get_chiq(&self) -> Option<DVector<f64>> {
        #[cfg(feature = "ndarray-compat")]
        {
            self.xftr
                .as_ref()?
                .get_chiq()
                .map(|x| DVector::from_vec(x.to_vec()))
        }
        #[cfg(not(feature = "ndarray-compat"))]
        {
            self.xftr.as_ref()?.get_chiq()
        }
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
    #[cfg(feature = "ndarray-compat")]
    fn test_xafs_group_normalization() {
        let test_file = String::from(TOP_DIR) + "/tests/testfiles/Ru_QAS.dat";
        let mut xafs_group = io::load_spectrum_QAS_trans(&test_file).unwrap();

        let _ = xafs_group.normalize();

        let reference_path =
            String::from(TOP_DIR) + "/tests/testfiles/Ru_QAS_pre_post_edge_expected.dat";
        let reference = load_txt_f64(&reference_path, &PARAM_LOADTXT).unwrap();

        let expected_norm = reference.get_col(4);

        let normalization = xafs_group.normalization.unwrap();
        let norm = normalization.get_norm().unwrap();
        norm.iter()
            .zip(expected_norm.iter())
            .for_each(|(x, y)| assert_abs_diff_eq!(x, y, epsilon = TEST_TOL_LESS_ACC));
    }

    #[test]
    #[cfg(not(feature = "ndarray-compat"))]
    fn test_xafs_group_normalization_nalgebra_smoke() {
        let test_file = String::from(TOP_DIR) + "/tests/testfiles/Ru_QAS.dat";
        let mut xafs_group = io::load_spectrum_QAS_trans(&test_file).unwrap();

        xafs_group.normalize().unwrap();
        let norm = xafs_group
            .normalization
            .as_ref()
            .and_then(|method| method.get_norm())
            .unwrap();
        assert_eq!(norm.len(), xafs_group.energy.as_ref().unwrap().len());
        assert!(norm.iter().all(|value| value.is_finite()));
    }

    #[test]
    fn test_find_e0_rejects_non_monotonic_energy() {
        let mut spectrum = XASSpectrum::new();
        spectrum.energy = Some(DVector::from_vec(vec![1.0, 3.0, 2.0]));
        spectrum.mu = Some(DVector::from_vec(vec![1.0, 2.0, 3.0]));

        let err = spectrum.find_e0().unwrap_err();
        assert!(matches!(
            err,
            XAFSError::Data(DataError::NonMonotonicEnergy { .. })
        ));
    }

    #[test]
    fn test_normalize_rejects_non_finite_input() {
        let mut spectrum = XASSpectrum::new();
        spectrum.energy = Some(DVector::from_vec(vec![1.0, 2.0, 3.0]));
        spectrum.mu = Some(DVector::from_vec(vec![1.0, f64::NAN, 3.0]));

        let err = spectrum.normalize().unwrap_err();
        assert!(matches!(
            err,
            XAFSError::Data(DataError::NonFiniteValues { .. })
        ));
    }

    #[test]
    fn test_calc_background_rejects_length_mismatch() {
        let mut spectrum = XASSpectrum::new();
        spectrum.energy = Some(DVector::from_vec(vec![1.0, 2.0, 3.0]));
        spectrum.mu = Some(DVector::from_vec(vec![1.0, 2.0]));

        let err = spectrum.calc_background().unwrap_err();
        assert!(matches!(
            err,
            XAFSError::Data(DataError::LengthMismatch { .. })
        ));
    }

    #[test]
    #[cfg(feature = "ndarray-compat")]
    fn test_borrowed_k_chi_views_match_owned_getters() -> Result<(), Box<dyn std::error::Error>> {
        let path = String::from(TOP_DIR) + "/tests/testfiles/Ru_QAS.dat";
        let mut spectrum = io::load_spectrum_QAS_trans(&path)?;
        spectrum.calc_background()?;

        let k_owned = spectrum.get_k().unwrap();
        let chi_owned = spectrum.get_chi().unwrap();
        let k_view = spectrum.get_k_view().unwrap();
        let chi_view = spectrum.get_chi_view().unwrap();

        assert_eq!(k_owned.len(), k_view.len());
        assert_eq!(chi_owned.len(), chi_view.len());

        for (owned, view) in k_owned.iter().zip(k_view.iter()) {
            assert_abs_diff_eq!(owned, view, epsilon = TEST_TOL);
        }
        for (owned, view) in chi_owned.iter().zip(chi_view.iter()) {
            assert_abs_diff_eq!(owned, view, epsilon = TEST_TOL_LESS_ACC);
        }

        spectrum.fft()?;
        assert!(spectrum.get_chir_mag().is_some());
        Ok(())
    }
}
