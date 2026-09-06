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
use super::tools;
use super::xafsutils;
use super::xrayfft;
use super::XAFSError;

// Load local traits
use mathutils::MathUtils;
use normalization::Normalization;

/// Data and processing parameters for a single XAS spectrum.
/// Also available as [`crate::Spectrum`]. Use [`Self::from_arrays`] for checked input.
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
    /// Accumulated energy shift (eV) applied by `shift_energy`/`calibrate`/`align_to`.
    pub energy_shift: f64,
    /// Per-point standard deviation of `mu` (set by merge/rebin).
    pub mu_stddev: Option<DVector<f64>>,
    /// Whether `energy`/`mu` are the result of `rebin`.
    pub rebinned: bool,
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

    /// Create a spectrum from finite, equal-length arrays with strictly increasing
    /// energy in eV. Unlike the legacy setter, this validates before indexing.
    pub fn from_arrays(energy: &[f64], mu: &[f64]) -> Result<Self, XAFSError> {
        let energy = DVector::from_column_slice(energy);
        let mu = DVector::from_column_slice(mu);
        Self::validate_energy_mu_inputs(&energy, &mu)?;
        for index in 1..energy.len() {
            if energy[index] == energy[index - 1] {
                return Err(DataError::DuplicateEnergy {
                    index,
                    energy: energy[index],
                }
                .into());
            }
        }
        let mut spectrum = Self::new();
        spectrum.set_spectrum(energy, mu);
        Ok(spectrum)
    }

    /// Borrow the unweighted k grid without cloning its buffer (Å⁻¹).
    pub fn k(&self) -> Option<&[f64]> {
        let background::BackgroundMethod::AUTOBK(autobk) = self.background.as_ref()? else {
            return None;
        };
        #[cfg(not(feature = "ndarray-compat"))]
        {
            Some(autobk.k.as_ref()?.as_slice())
        }
        #[cfg(feature = "ndarray-compat")]
        {
            autobk.k.as_ref()?.as_slice()
        }
    }

    /// Borrow unweighted χ(k) without cloning its buffer.
    pub fn chi(&self) -> Option<&[f64]> {
        let background::BackgroundMethod::AUTOBK(autobk) = self.background.as_ref()? else {
            return None;
        };
        #[cfg(not(feature = "ndarray-compat"))]
        {
            Some(autobk.chi.as_ref()?.as_slice())
        }
        #[cfg(feature = "ndarray-compat")]
        {
            autobk.chi.as_ref()?.as_slice()
        }
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
        self.energy = Some(energy.into());
        let energy = self.energy.as_ref().ok_or_else(|| DataError::MissingData {
            field: "energy".to_string(),
        })?;
        let mu = self.raw_mu.as_ref().ok_or_else(|| DataError::MissingData {
            field: "raw_mu".to_string(),
        })?;
        let knot = self
            .raw_energy
            .as_ref()
            .ok_or_else(|| DataError::MissingData {
                field: "raw_energy".to_string(),
            })?;

        let interpolated = energy
            .interpolate(knot.as_slice(), mu.as_slice())
            .map_err(|e| super::errors::MathError::SplineEvalFailed {
                x: 0.0,
                reason: e.to_string(),
            })?;
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
        let mut xftf = self.xftf.take().unwrap_or_default();

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

    // -----------------------------------------------------------------------
    // Athena-style data-processing tools (see `xafs::tools`)
    // -----------------------------------------------------------------------

    /// Clear every result derived from `energy`/`mu` (normalization outputs,
    /// background, χ(k), χ(R), χ(q)) while keeping the stage parameters, so
    /// the pipeline recomputes from the modified data.
    pub fn invalidate_derived(&mut self) -> &mut Self {
        self.k = None;
        self.chi = None;
        self.chi_kweighted = None;
        self.chi_r = None;
        self.chi_r_mag = None;
        self.chi_r_re = None;
        self.chi_r_im = None;
        self.q = None;
        match self.normalization.as_mut() {
            Some(normalization::NormalizationMethod::PrePostEdge(p)) => {
                p.edge_step = None;
                p.pre_edge = None;
                p.post_edge = None;
                p.norm = None;
                p.flat = None;
                p.pre_coefficients = None;
                p.norm_coefficients = None;
            }
            Some(normalization::NormalizationMethod::MBack(m)) => {
                m.edge_step = None;
                m.norm = None;
                m.flat = None;
            }
            None => {}
        }
        if let Some(background::BackgroundMethod::AUTOBK(a)) = self.background.as_mut() {
            a.bkg = None;
            a.chie = None;
            a.k = None;
            a.chi = None;
        }
        if let Some(f) = self.xftf.as_mut() {
            f.r = None;
            f.chir = None;
            f.chir_mag = None;
            f.kwin = None;
        }
        if let Some(r) = self.xftr.as_mut() {
            r.q = None;
            r.chiq = None;
            r.rwin = None;
        }
        self
    }

    fn working_pair(&self) -> Result<(&DVector<f64>, &DVector<f64>), XAFSError> {
        let energy = self.energy.as_ref().ok_or_else(|| DataError::MissingData {
            field: "energy".to_string(),
        })?;
        let mu = self.mu.as_ref().ok_or_else(|| DataError::MissingData {
            field: "mu".to_string(),
        })?;
        Self::validate_energy_mu_inputs(energy, mu)?;
        Ok((energy, mu))
    }

    fn raw_grid_matches_working(&self) -> bool {
        match (&self.raw_energy, &self.energy) {
            (Some(raw), Some(energy)) => raw == energy,
            _ => false,
        }
    }

    /// Shift the energy axis (working and raw) by `delta_ev`, moving `e0` and
    /// the e0-like stage parameters along with it. The shift accumulates in
    /// `energy_shift`. Derived results are invalidated.
    pub fn shift_energy(&mut self, delta_ev: f64) -> &mut Self {
        if let Some(e) = self.energy.as_mut() {
            e.add_scalar_mut(delta_ev);
        }
        if let Some(e) = self.raw_energy.as_mut() {
            e.add_scalar_mut(delta_ev);
        }
        self.energy_shift += delta_ev;
        if let Some(e0) = self.e0.as_mut() {
            *e0 += delta_ev;
        }
        if let Some(norm) = self.normalization.as_mut() {
            let e0 = norm.get_e0().map(|e0| e0 + delta_ev);
            norm.set_e0(e0);
        }
        if let Some(background::BackgroundMethod::AUTOBK(a)) = self.background.as_mut() {
            a.ek0 = a.ek0.map(|e| e + delta_ev);
        }
        self.invalidate_derived()
    }

    /// Energy of the requested edge feature on the current `energy`/`mu`.
    /// `HalfStep` normalizes a copy of the spectrum if no flattened μ is available.
    pub fn edge_feature_energy(&self, feature: tools::EdgeFeature) -> Result<f64, XAFSError> {
        let (energy, mu) = self.working_pair()?;
        match feature {
            tools::EdgeFeature::DerivativeMax => tools::derivative_max_energy(energy, mu),
            tools::EdgeFeature::SecondDerivativeZero => {
                tools::second_derivative_zero_energy(energy, mu)
            }
            tools::EdgeFeature::HalfStep => {
                let e0 = match self.e0 {
                    Some(e0) => e0,
                    None => tools::derivative_max_energy(energy, mu)?,
                };
                let flat = match self.get_flat() {
                    Some(flat) if flat.len() == energy.len() => flat,
                    _ => {
                        let mut tmp = self.clone();
                        tmp.invalidate_derived();
                        tmp.normalize()?;
                        tmp.get_flat().ok_or_else(|| DataError::MissingData {
                            field: "flat".to_string(),
                        })?
                    }
                };
                tools::half_step_energy(energy, &flat, e0)
            }
        }
    }

    /// Calibrate: shift the spectrum so that `feature` lands on `target_ev`
    /// and set `e0 = target_ev` (as Athena does). Returns the shift applied.
    pub fn calibrate(
        &mut self,
        feature: tools::EdgeFeature,
        target_ev: f64,
    ) -> Result<f64, XAFSError> {
        let current = self.edge_feature_energy(feature)?;
        let shift = target_ev - current;
        self.shift_energy(shift);
        self.e0 = Some(target_ev);
        if let Some(norm) = self.normalization.as_mut() {
            norm.set_e0(Some(target_ev));
        }
        Ok(shift)
    }

    /// Align this spectrum to `reference` by overlaying dμ/dE within
    /// `window` (eV, relative to the reference e0). The best shift (searched
    /// over ±20 eV) is applied with [`Self::shift_energy`] and returned.
    pub fn align_to(
        &mut self,
        reference: &XASSpectrum,
        window: (f64, f64),
    ) -> Result<f64, XAFSError> {
        let (e_dat, mu_dat) = self.working_pair()?;
        let (e_ref, mu_ref) = reference.working_pair()?;
        let ref_e0 = match reference.e0 {
            Some(e0) => e0,
            None => tools::derivative_max_energy(e_ref, mu_ref)?,
        };
        let shift = tools::find_energy_shift(
            e_dat,
            &tools::dmude(e_dat, mu_dat),
            e_ref,
            &tools::dmude(e_ref, mu_ref),
            ref_e0 + window.0,
            ref_e0 + window.1,
            20.0,
            0.1,
        )?;
        self.shift_energy(shift);
        Ok(shift)
    }

    /// Remove the points nearest to `energies` from the working and raw
    /// arrays. Returns the number of working points removed.
    fn remove_points_at(&mut self, energies: &[f64]) -> Result<usize, XAFSError> {
        if energies.is_empty() {
            return Ok(0);
        }
        let (energy, mu) = self.working_pair()?;
        let idx = tools::nearest_indices(energy, energies);
        if energy.len() - idx.len() < 2 {
            return Err(DataError::InsufficientData {
                min: 2,
                actual: energy.len() - idx.len(),
            }
            .into());
        }
        let new_energy = tools::remove_indices(energy, &idx);
        let new_mu = tools::remove_indices(mu, &idx);
        let new_raw = match (self.raw_energy.as_ref(), self.raw_mu.as_ref()) {
            (Some(raw_e), Some(raw_mu))
                if raw_e.len() == raw_mu.len() && raw_e.len() > idx.len() + 1 =>
            {
                let raw_idx = tools::nearest_indices(raw_e, energies);
                Some((
                    tools::remove_indices(raw_e, &raw_idx),
                    tools::remove_indices(raw_mu, &raw_idx),
                ))
            }
            _ => None,
        };
        if let Some((re, rm)) = new_raw {
            self.raw_energy = Some(re);
            self.raw_mu = Some(rm);
        }
        self.energy = Some(new_energy);
        self.mu = Some(new_mu);
        self.invalidate_derived();
        Ok(idx.len())
    }

    /// Deglitch: remove the data points nearest to each of `energies_to_remove`.
    pub fn deglitch_points(&mut self, energies_to_remove: &[f64]) -> Result<usize, XAFSError> {
        self.remove_points_at(energies_to_remove)
    }

    /// Deglitch: remove every point with `e_lo <= E <= e_hi`.
    pub fn deglitch_range(&mut self, e_lo: f64, e_hi: f64) -> Result<usize, XAFSError> {
        let (energy, _) = self.working_pair()?;
        let targets: Vec<f64> = tools::indices_in_range(energy, e_lo, e_hi)
            .into_iter()
            .map(|i| energy[i])
            .collect();
        self.remove_points_at(&targets)
    }

    /// Athena's margin deglitch: fit a line to μ(E) over `[e_lo, e_hi]` and
    /// remove the points lying more than `upper_margin` above or
    /// `lower_margin` below it. Returns the energies removed.
    pub fn deglitch_margin(
        &mut self,
        e_lo: f64,
        e_hi: f64,
        upper_margin: f64,
        lower_margin: f64,
    ) -> Result<Vec<f64>, XAFSError> {
        let (energy, mu) = self.working_pair()?;
        let removed: Vec<f64> =
            tools::margin_outliers(energy, mu, e_lo, e_hi, upper_margin, lower_margin)?
                .into_iter()
                .map(|i| energy[i])
                .collect();
        self.remove_points_at(&removed)?;
        Ok(removed)
    }

    /// Truncate: keep only points with `before <= E <= after` (either bound
    /// may be `None`).
    pub fn truncate(
        &mut self,
        before: Option<f64>,
        after: Option<f64>,
    ) -> Result<&mut Self, XAFSError> {
        let lo = before.unwrap_or(f64::NEG_INFINITY);
        let hi = after.unwrap_or(f64::INFINITY);
        let keep = |e: &DVector<f64>, m: &DVector<f64>| -> (DVector<f64>, DVector<f64>) {
            let idx: Vec<usize> = e
                .iter()
                .enumerate()
                .filter(|(_, v)| **v >= lo && **v <= hi)
                .map(|(i, _)| i)
                .collect();
            (
                DVector::from_iterator(idx.len(), idx.iter().map(|&i| e[i])),
                DVector::from_iterator(idx.len(), idx.iter().map(|&i| m[i])),
            )
        };
        let (energy, mu) = self.working_pair()?;
        let (new_energy, new_mu) = keep(energy, mu);
        if new_energy.len() < 2 {
            return Err(DataError::InsufficientData {
                min: 2,
                actual: new_energy.len(),
            }
            .into());
        }
        let new_raw = match (self.raw_energy.as_ref(), self.raw_mu.as_ref()) {
            (Some(raw_e), Some(raw_mu)) if raw_e.len() == raw_mu.len() => Some(keep(raw_e, raw_mu)),
            _ => None,
        };
        if let Some((re, rm)) = new_raw {
            if re.len() >= 2 {
                self.raw_energy = Some(re);
                self.raw_mu = Some(rm);
            }
        }
        self.energy = Some(new_energy);
        self.mu = Some(new_mu);
        Ok(self.invalidate_derived())
    }

    /// Rebin onto Athena's three-region grid (see [`tools::rebin`]). The raw
    /// arrays are replaced by the rebinned data, `mu_stddev` holds the
    /// per-bin standard deviation and `rebinned` is set.
    pub fn rebin(&mut self, cfg: &tools::RebinConfig) -> Result<&mut Self, XAFSError> {
        let (energy, mu) = self.working_pair()?;
        let cfg = tools::RebinConfig {
            e0: cfg.e0.or(self.e0),
            ..*cfg
        };
        let out = tools::rebin(energy, mu, &cfg)?;
        self.raw_energy = Some(out.energy.clone());
        self.raw_mu = Some(out.mu.clone());
        self.energy = Some(out.energy);
        self.mu = Some(out.mu);
        self.mu_stddev = Some(out.stddev);
        self.rebinned = true;
        Ok(self.invalidate_derived())
    }

    /// Non-mutating variant of [`Self::rebin`].
    pub fn rebinned(&self, cfg: &tools::RebinConfig) -> Result<XASSpectrum, XAFSError> {
        let mut out = self.clone();
        out.rebin(cfg)?;
        if let Some(name) = self.name.as_deref() {
            out.set_name(format!("{name} (rebinned)"));
        }
        Ok(out)
    }

    /// Smooth μ(E) by convolution with a Lorentzian/Gaussian/Voigt of width
    /// `sigma` (and `gamma`), replacing `mu` and `raw_mu`.
    pub fn smooth_mu(
        &mut self,
        form: xafsutils::ConvolveForm,
        sigma: Option<f64>,
        gamma: Option<f64>,
    ) -> Result<&mut Self, XAFSError> {
        let (energy, mu) = self.working_pair()?;
        let smoothed = tools::smooth_mu(energy, mu, form, sigma, gamma)?;
        let new_raw_mu = if self.raw_grid_matches_working() {
            Some(smoothed.clone())
        } else {
            match (self.raw_energy.as_ref(), self.raw_mu.as_ref()) {
                (Some(raw_e), Some(raw_mu)) if raw_e.len() == raw_mu.len() && raw_e.len() >= 3 => {
                    Some(tools::smooth_mu(raw_e, raw_mu, form, sigma, gamma)?)
                }
                _ => None,
            }
        };
        if new_raw_mu.is_some() {
            self.raw_mu = new_raw_mu;
        }
        self.mu = Some(smoothed);
        Ok(self.invalidate_derived())
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

    pub fn get_norm(&self) -> Option<DVector<f64>> {
        #[cfg(feature = "ndarray-compat")]
        {
            self.normalization
                .as_ref()?
                .get_norm()
                .map(|x| DVector::from_vec(x.to_vec()))
        }
        #[cfg(not(feature = "ndarray-compat"))]
        {
            self.normalization.as_ref()?.get_norm().cloned()
        }
    }

    pub fn get_flat(&self) -> Option<DVector<f64>> {
        #[cfg(feature = "ndarray-compat")]
        {
            self.normalization
                .as_ref()?
                .get_flat()
                .map(|x| DVector::from_vec(x.to_vec()))
        }
        #[cfg(not(feature = "ndarray-compat"))]
        {
            self.normalization.as_ref()?.get_flat().cloned()
        }
    }

    pub fn get_pre_edge(&self) -> Option<DVector<f64>> {
        let normalization = self.normalization.as_ref()?;
        match normalization {
            normalization::NormalizationMethod::PrePostEdge(prepost) => {
                #[cfg(feature = "ndarray-compat")]
                {
                    prepost
                        .get_pre_edge()
                        .map(|x| DVector::from_vec(x.to_vec()))
                }
                #[cfg(not(feature = "ndarray-compat"))]
                {
                    prepost.get_pre_edge().cloned()
                }
            }
            _ => None,
        }
    }

    pub fn get_post_edge(&self) -> Option<DVector<f64>> {
        let normalization = self.normalization.as_ref()?;
        match normalization {
            normalization::NormalizationMethod::PrePostEdge(prepost) => {
                #[cfg(feature = "ndarray-compat")]
                {
                    prepost
                        .get_post_edge()
                        .map(|x| DVector::from_vec(x.to_vec()))
                }
                #[cfg(not(feature = "ndarray-compat"))]
                {
                    prepost.get_post_edge().cloned()
                }
            }
            _ => None,
        }
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

    pub fn get_kwin(&self) -> Option<DVector<f64>> {
        #[cfg(feature = "ndarray-compat")]
        {
            self.xftf
                .as_ref()?
                .get_kwin()
                .map(|x| DVector::from_vec(x.to_vec()))
        }
        #[cfg(not(feature = "ndarray-compat"))]
        {
            self.xftf.as_ref()?.get_kwin().cloned()
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
    fn test_interpolate_spectrum_updates_energy_and_mu() {
        let mut spectrum = XASSpectrum::new();
        spectrum.set_spectrum(vec![0.0, 1.0, 2.0, 3.0], vec![0.0, 2.0, 4.0, 6.0]);

        spectrum.interpolate_spectrum(vec![0.5, 1.5, 2.5]).unwrap();

        assert_eq!(
            spectrum.energy.as_ref().unwrap(),
            &DVector::from_vec(vec![0.5, 1.5, 2.5])
        );

        let mu = spectrum.mu.as_ref().unwrap();
        assert_abs_diff_eq!(mu[0], 1.0, epsilon = TEST_TOL);
        assert_abs_diff_eq!(mu[1], 3.0, epsilon = TEST_TOL);
        assert_abs_diff_eq!(mu[2], 5.0, epsilon = TEST_TOL);
    }

    #[test]
    fn test_interpolate_spectrum_missing_raw_mu_keeps_existing_mu() {
        let mut spectrum = XASSpectrum::new();
        spectrum.raw_energy = Some(DVector::from_vec(vec![0.0, 1.0]));
        spectrum.raw_mu = None;
        spectrum.mu = Some(DVector::from_vec(vec![42.0]));

        let err = spectrum.interpolate_spectrum(vec![0.25, 0.75]).unwrap_err();
        assert!(matches!(
            err,
            XAFSError::Data(DataError::MissingData { ref field }) if field == "raw_mu"
        ));

        assert_eq!(
            spectrum.energy.as_ref().unwrap(),
            &DVector::from_vec(vec![0.25, 0.75])
        );
        assert_eq!(
            spectrum.mu.as_ref().unwrap(),
            &DVector::from_vec(vec![42.0])
        );
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
