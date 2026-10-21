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
use super::errors::{DataError, NormalizationError};
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
/// The default is empty; processing creates missing default stage configurations.
/// Methods mutate this spectrum and getters never run a calculation implicitly.
///
/// Prefer getters over the legacy result fields: authoritative outputs live inside
/// `normalization`, `background`, `xftf` and `xftr`. Directly editing public inputs
/// or settings requires [`Self::invalidate_derived`] before processing again.
/// Raw arrays are a working baseline for interpolation, not an immutable archive:
/// calibration, deglitching, truncation, smoothing and rebinning can modify them.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
#[derive(Default)]
pub struct XASSpectrum {
    /// Optional display name; changing it does not affect numerical results.
    pub name: Option<String>,
    /// Owned baseline energy grid in eV. Data-treatment methods may modify it.
    pub raw_energy: Option<DVector<f64>>,
    /// Owned baseline absorption values, paired with `raw_energy`.
    pub raw_mu: Option<DVector<f64>>,
    /// Owned current energy grid in eV used by processing stages.
    pub energy: Option<DVector<f64>>,
    /// Owned current absorption values, paired with `energy`; units match the input.
    pub mu: Option<DVector<f64>>,
    /// Selected or estimated edge energy in eV. Prefer [`Self::set_e0`] to edit it.
    pub e0: Option<f64>,
    /// Legacy result slot; use [`Self::k`] for the background wave-number grid in Å⁻¹.
    pub k: Option<DVector<f64>>,
    /// Legacy result slot; use [`Self::chi`] for dimensionless unweighted EXAFS.
    pub chi: Option<DVector<f64>>,
    /// Legacy result slot; use [`Self::chi_kweighted`] to calculate weighted EXAFS.
    pub chi_kweighted: Option<DVector<f64>>,
    /// Legacy result slot; use [`Self::chir`] for the stored complex Fourier data.
    pub chi_r: Option<DVector<f64>>,
    /// Legacy result slot; use [`Self::chir_mag`] for Fourier magnitudes.
    pub chi_r_mag: Option<DVector<f64>>,
    /// Legacy result slot; use [`Self::chir_real`] for the real Fourier component.
    pub chi_r_re: Option<DVector<f64>>,
    /// Legacy result slot; use [`Self::chir_imag`] for the imaginary Fourier component.
    pub chi_r_im: Option<DVector<f64>>,
    /// Legacy result slot; use [`Self::q`] for the inverse-transform grid in Å⁻¹.
    pub q: Option<DVector<f64>>,
    /// Normalization settings and cached outputs; `None` selects default pre/post-edge normalization when needed.
    pub normalization: Option<normalization::NormalizationMethod>,
    /// Background settings and cached outputs; `None` selects default AUTOBK when needed.
    pub background: Option<background::BackgroundMethod>,
    /// Forward-transform settings and outputs; `None` selects [`xrayfft::XrayFFTF::default`].
    pub xftf: Option<xrayfft::XrayFFTF>,
    /// Inverse-transform settings and outputs; `None` selects [`xrayfft::XrayFFTR::default`].
    pub xftr: Option<xrayfft::XrayFFTR>,
    /// Accumulated energy correction in eV. Prefer [`Self::energy_offset`] and
    /// [`Self::set_energy_offset`]; assigning this legacy field does not move arrays.
    pub energy_shift: f64,
    /// Per-point spread stored by merge/rebin, in input absorption units.
    /// Other data edits do not consistently propagate, resize or clear this field;
    /// verify alignment with `energy`/`mu` before reuse. It is not automatically
    /// consumed as an uncertainty model by spectrum processing.
    pub mu_stddev: Option<DVector<f64>>,
    /// Marker set by rebinning. Later data replacement does not reset it, so it
    /// records a past operation rather than validating the current arrays.
    pub rebinned: bool,
    /// Explicit normalization scale, distinct from the scale inferred by a stage.
    #[serde(skip_serializing_if = "Option::is_none")]
    normalization_edge_step_override: Option<f64>,
    /// Last normalization output, used to recognize later public-field edits.
    #[serde(skip_serializing_if = "Option::is_none")]
    normalization_edge_step_last_result: Option<f64>,
    /// Input already expressed in edge-step units. `None` retains raw-input
    /// behavior; use `from_prepared` to establish this invariant with validation.
    #[serde(skip_serializing_if = "Option::is_none")]
    prepared_space: Option<super::analysis::AnalysisSpace>,
    /// Explicit pre/post-edge fitting of prepared input. False preserves its
    /// values and unit step; true is selected by `set_normalization_method`.
    refit_prepared: bool,
    /// Acquisition interpretation, populated for explicit transmission imports.
    /// Unknown remains distinct from fluorescence and electron-yield detector ratios.
    #[serde(skip_serializing_if = "super::fluorescence::AbsorptionMode::is_unknown")]
    absorption_mode: super::fluorescence::AbsorptionMode,
    /// Immutable history of a thick-sample XANES correction; survives normalization
    /// and data edits, blocks repeated correction and unqualified EXAFS processing.
    #[serde(skip_serializing_if = "Option::is_none")]
    fluorescence_correction: Option<Box<super::fluorescence::FluorescenceCorrectionResult>>,
    /// Inherited scientific domain restriction, independent of direct correction history.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    xanes_only: bool,
}

impl XASSpectrum {
    /// Acquisition interpretation attached to these arrays. Unknown means missing
    /// evidence, not an automatically selected fluorescence mode.
    pub fn absorption_mode(&self) -> super::fluorescence::AbsorptionMode {
        self.absorption_mode
    }

    /// Explicitly attach or revise the acquisition interpretation. Does not change
    /// arrays or processing caches. A correction record remains attached; changing
    /// this declaration cannot enable repeated correction or unqualified EXAFS.
    pub fn set_absorption_mode(&mut self, mode: super::fluorescence::AbsorptionMode) -> &mut Self {
        self.absorption_mode = mode;
        self
    }

    /// Correct fluorescence over-absorption into an independent, unnormalized spectrum.
    ///
    /// Internal conventional normalization runs automatically; the original spectrum
    /// stays unchanged. Calling this method on Unknown input explicitly interprets it
    /// as fluorescence and records that assumption. Known transmission, prepared
    /// norm/flat input and already corrected spectra are rejected. Select measured
    /// geometry and emission in `settings`; no angle or sample formula is inferred.
    /// The output retains original arrays and atomic/internal-fit provenance through
    /// [`Self::fluorescence_correction`]. Call `normalize()` to run the independent
    /// final normalization (polynomial by default; MBACK can be selected normally).
    /// Corrected-array uncertainties are unavailable. This XANES-only branch cannot
    /// run AUTOBK, EXAFS transforms or wavelets; use the retained original instead.
    pub fn correct_fluorescence(
        &self,
        settings: &super::fluorescence::FluorescenceCorrection,
    ) -> Result<Self, super::fluorescence::FluorescenceError> {
        use super::fluorescence::{AbsorptionMode, FluorescenceError};
        let fail = |message: &str| FluorescenceError(message.into());
        if self.absorption_mode == AbsorptionMode::Transmission {
            return Err(fail("transmission data cannot use fluorescence correction"));
        }
        if self.is_xanes_only() {
            return Err(fail(
                "this lineage is already corrected; start from its original uncorrected spectrum",
            ));
        }
        if self.prepared_space.is_some() {
            return Err(fail(
                "provide uncorrected fluorescence mu, not normalized or flattened input",
            ));
        }
        let energy = self.energy.as_ref().ok_or_else(|| fail("missing energy"))?;
        let mu = self.mu.as_ref().ok_or_else(|| fail("missing absorption"))?;
        let mut settings = settings.clone();
        if settings.e0.is_none() {
            settings.e0 = self.e0;
        }
        let mut result = settings.apply(energy.as_slice(), mu.as_slice())?;
        result.input_mode = self.absorption_mode;
        if self.absorption_mode == AbsorptionMode::Unknown {
            result.warnings.push(
                "The caller explicitly interpreted unknown acquisition provenance as fluorescence."
                    .into(),
            );
        }
        let mut output = Self::from_arrays(&result.energy, &result.corrected_mu)
            .map_err(|e| FluorescenceError(e.to_string()))?;
        output.name = self.name.clone();
        output.e0 = Some(result.internal.e0);
        output.energy_shift = self.energy_shift;
        output.absorption_mode = AbsorptionMode::Fluorescence;
        output.fluorescence_correction = Some(Box::new(result));
        Ok(output)
    }

    /// Original correction record. Later data edits do not rewrite this historical
    /// result; its own energy grid identifies its arrays. None means no native correction.
    pub fn fluorescence_correction(
        &self,
    ) -> Option<&super::fluorescence::FluorescenceCorrectionResult> {
        self.fluorescence_correction.as_deref()
    }

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

    /// Create an empty spectrum with no data, selected methods or calculated results.
    /// Use [`Self::from_arrays`] to construct checked input in one call.
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

    /// Copy already normalized or flattened absorption into a processable spectrum
    /// (introduced in 0.2.9). Energy is in eV, `mu` is dimensionless, and `e0` must be
    /// finite and strictly inside the measured interval. Only `Norm` and `Flat`
    /// are accepted; use `from_arrays` for raw measurements.
    ///
    /// By default this preserves every supplied value and records a unit edge step. Calling
    /// `normalize` refreshes that identity mapping without fitting pre/post-edge
    /// curves or flattening again, unless `set_normalization_method` explicitly
    /// selects a new normalization fit. AUTOBK, Fourier transforms and EXAFS fitting
    /// can then use the ordinary spectrum API. On flattened input AUTOBK operates
    /// on those flattened values; the original unflattened signal is not recovered.
    /// `set_e0` and data edits preserve the input type; `set_spectrum` replaces it
    /// with raw input. Arrays must be finite, paired and strictly increasing.
    pub fn from_prepared(
        energy: &[f64],
        mu: &[f64],
        space: super::analysis::AnalysisSpace,
        e0: f64,
    ) -> Result<Self, XAFSError> {
        use super::analysis::AnalysisSpace;
        if !matches!(space, AnalysisSpace::Norm | AnalysisSpace::Flat) {
            return Err(NormalizationError::Other {
                message: "Prepared absorption requires AnalysisSpace::Norm or Flat".into(),
            }
            .into());
        }
        let mut spectrum = Self::from_arrays(energy, mu)?;
        spectrum.prepared_space = Some(space);
        spectrum.e0 = Some(e0);
        spectrum.normalize()?;
        Ok(spectrum)
    }

    /// Input representation established by `from_prepared`, or `None` for raw
    /// measurements. This records input units, not the last displayed plot.
    pub fn prepared_space(&self) -> Option<super::analysis::AnalysisSpace> {
        self.prepared_space
    }

    /// Whether normalization currently preserves prepared values with a unit
    /// edge step. Explicit `set_normalization_method` opts into refitting and
    /// returns false here; the original prepared input type remains recorded.
    pub fn preserves_prepared_values(&self) -> bool {
        self.prepared_space.is_some() && !self.refit_prepared
    }

    fn normalize_prepared(&mut self) -> Result<&mut Self, XAFSError> {
        self.invalidate_derived();
        if !matches!(
            self.prepared_space,
            Some(super::analysis::AnalysisSpace::Norm | super::analysis::AnalysisSpace::Flat)
        ) {
            return Err(NormalizationError::Other {
                message: "Prepared absorption requires AnalysisSpace::Norm or Flat".into(),
            }
            .into());
        }
        let energy = self.energy.as_ref().ok_or_else(|| DataError::MissingData {
            field: "energy".into(),
        })?;
        let mu = self
            .mu
            .as_ref()
            .ok_or_else(|| DataError::MissingData { field: "mu".into() })?;
        Self::validate_energy_mu_inputs(energy, mu)?;
        let e0 = self.e0.ok_or_else(|| DataError::MissingData {
            field: "e0 for prepared absorption".into(),
        })?;
        if !e0.is_finite() || e0 <= energy[0] || e0 >= energy[energy.len() - 1] {
            return Err(NormalizationError::E0OutOfRange {
                e0,
                data_min: energy[0],
                data_max: energy[energy.len() - 1],
            }
            .into());
        }
        let mut output = normalization::PrePostEdge::new();
        output.e0 = Some(e0);
        output.edge_step = Some(1.0);
        #[cfg(not(feature = "ndarray-compat"))]
        let values = mu.clone();
        #[cfg(feature = "ndarray-compat")]
        let values = ndarray::Array1::from_vec(mu.as_slice().to_vec());
        output.norm = Some(values.clone());
        if self.prepared_space == Some(super::analysis::AnalysisSpace::Flat) {
            output.flat = Some(values);
        }
        self.normalization = Some(normalization::NormalizationMethod::PrePostEdge(output));
        self.normalization_edge_step_override = Some(1.0);
        self.normalization_edge_step_last_result = Some(1.0);
        Ok(self)
    }

    /// Borrow the background k grid without cloning its buffer (Å⁻¹).
    /// Returns `None` before a valid AUTOBK result; no calculation is triggered.
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

    /// Borrow dimensionless, unweighted χ(k) without cloning its buffer.
    /// Returns `None` before a valid AUTOBK result; no calculation is triggered.
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

    /// Set the display name without invalidating numerical results.
    pub fn set_name<S: Into<String>>(&mut self, name: S) -> &mut Self {
        self.name = Some(name.into());
        self
    }

    /// Replace the baseline and working arrays, sorting energy and absorption together.
    /// Energy is in eV. This legacy setter takes ownership after conversion and clones
    /// the baseline into working arrays. It does not check lengths or finite values;
    /// prefer [`Self::from_arrays`] for checked input. Clears E0 and derived results
    /// while retaining other stage settings. Replacing the arrays resets the
    /// recorded energy offset to zero; the supplied axis becomes the new baseline.
    ///
    /// # Panics
    /// May panic when sorting mismatched arrays. Supply paired finite arrays.
    pub fn set_spectrum<T: Into<DVector<f64>>, M: Into<DVector<f64>>>(
        &mut self,
        energy: T,
        mu: M,
    ) -> &mut Self {
        if self.prepared_space.take().is_some() {
            self.normalization = None;
            self.normalization_edge_step_override = None;
            self.normalization_edge_step_last_result = None;
        }
        self.refit_prepared = false;
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
        self.energy_shift = 0.0;
        self.e0 = None;
        if let Some(method) = self.normalization.as_mut() {
            method.set_e0(None);
        }
        if let Some(background::BackgroundMethod::AUTOBK(method)) = self.background.as_mut() {
            method.ek0 = None;
        }
        self.invalidate_derived()
    }

    /// Linearly interpolate baseline absorption onto an owned energy grid in eV.
    /// The baseline arrays remain unchanged; successful interpolation invalidates
    /// derived results. The interpolation helper holds endpoint values outside the
    /// baseline range. Missing baseline data or interpolation failures return an error.
    /// The working energy grid is assigned before interpolation, even if it fails.
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

        Ok(self.invalidate_derived())
    }

    /// Set the edge energy in eV and propagate it into existing stage configurations.
    /// Invalidates normalization, background and Fourier results. The value is stored
    /// as supplied; normalization subsequently rejects a non-finite E0 or one outside
    /// the measured range. Use [`Self::find_e0`] for an automatic estimate.
    pub fn set_e0<S: Into<f64>>(&mut self, e0: S) -> &mut Self {
        self.invalidate_derived();
        self.e0 = Some(e0.into());
        if let Some(method) = self.normalization.as_mut() {
            method.set_e0(self.e0);
        }
        if let Some(background::BackgroundMethod::AUTOBK(method)) = self.background.as_mut() {
            method.ek0 = self.e0;
        }
        self
    }

    /// Estimate E0 in eV from the current spectrum using the core edge detector.
    /// Propagates the result through [`Self::set_e0`] and invalidates derived results.
    /// Missing, non-finite, mismatched or decreasing data return a typed error; a usable
    /// absorption edge and enough points are also required by the detector.
    pub fn find_e0(&mut self) -> Result<&mut Self, XAFSError> {
        let energy = self.energy.as_ref().ok_or_else(|| DataError::MissingData {
            field: "energy".to_string(),
        })?;
        let mu = self.mu.as_ref().ok_or_else(|| DataError::MissingData {
            field: "mu".to_string(),
        })?;
        Self::validate_energy_mu_inputs(energy, mu)?;
        let e0 = xafsutils::find_e0(energy, mu)?;
        Ok(self.set_e0(e0))
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

    /// Take ownership of normalization settings and clear dependent results.
    /// Pass [`crate::PrePostEdge`] or [`crate::MBack`] directly, a method enum,
    /// or an optional enum.
    /// `None` selects default pre/post-edge normalization. A configured edge energy
    /// takes precedence over the existing E0; a polynomial edge step remains an override.
    /// MBACK instead determines its step from the atomic match.
    /// No normalization is performed by this setter.
    /// For prepared input this explicitly enables fitting new pre/post-edge
    /// curves, replacing the default unit-step identity mapping. The original
    /// input arrays and their declared representation remain intact; subsequent
    /// `normalize()` calls use these settings. The edge step is fitted unless
    /// the supplied method overrides it. This choice survives serialization.
    pub fn set_normalization_method(
        &mut self,
        method: impl Into<Option<normalization::NormalizationMethod>>,
    ) -> Result<&mut Self, XAFSError> {
        self.refit_prepared = self.prepared_space.is_some();
        let method = method.into();
        self.invalidate_derived();
        if let Some(method) = method {
            self.normalization = Some(method);
        } else {
            let normalization_method = normalization::PrePostEdge::new();
            self.normalization = Some(normalization::NormalizationMethod::PrePostEdge(
                normalization_method,
            ));
        }

        self.normalization_edge_step_override =
            self.normalization.as_ref().and_then(|m| m.get_edge_step());
        let e0 = self
            .normalization
            .as_ref()
            .and_then(|m| m.get_e0())
            .or(self.e0);
        if e0 != self.e0 {
            if let Some(background::BackgroundMethod::AUTOBK(method)) = self.background.as_mut() {
                method.ek0 = e0;
            }
        }
        self.e0 = e0;
        if let Some(normalization_method) = self.normalization.as_mut() {
            if e0.is_some() {
                normalization_method.set_e0(e0);
            }
        } else {
            return Err(DataError::MissingData {
                field: "normalization method".to_string(),
            }
            .into());
        }

        Ok(self)
    }

    /// Normalize using the selected method, resolving E0 and defaults as needed.
    /// The default fits pre/post-edge curves and expresses absorption in edge-step
    /// units. Call [`Self::norm`] or [`Self::flat`] for owned outputs after success.
    /// Recomputing invalidates background and Fourier results. Missing/invalid data,
    /// an unusable fitting range, or an unsupported method returns a typed error.
    pub fn normalize(&mut self) -> Result<&mut Self, XAFSError> {
        if self.preserves_prepared_values() {
            return self.normalize_prepared();
        }
        // Capture explicitly configured edge_step before the algorithm fills it.
        if let Some(method) = &self.normalization {
            if method.get_norm().is_none() {
                self.normalization_edge_step_override = method.get_edge_step();
            }
        }
        self.invalidate_derived();
        if self.normalization.is_none() {
            self.set_normalization_method(None)?;
        }

        let configured_e0 = self.normalization.as_ref().and_then(|m| m.get_e0());
        // Let the selected normalization method estimate an unset E0. In
        // ndarray compatibility mode this preserves its legacy edge detector.

        let energy = self.energy.as_ref().ok_or_else(|| DataError::MissingData {
            field: "energy".to_string(),
        })?;
        let mu = self.mu.as_ref().ok_or_else(|| DataError::MissingData {
            field: "mu".to_string(),
        })?;
        Self::validate_energy_mu_inputs(energy, mu)?;

        if let Some(e0) = self.e0.or(configured_e0) {
            let data_min = energy[0];
            let data_max = energy[energy.len() - 1];
            if !e0.is_finite() || e0 <= data_min || e0 >= data_max {
                return Err(NormalizationError::E0OutOfRange {
                    e0,
                    data_min,
                    data_max,
                }
                .into());
            }
        }

        let mut method = self
            .normalization
            .clone()
            .ok_or_else(|| DataError::MissingData {
                field: "normalization method".to_string(),
            })?;
        if let Err(error) = method.normalize(energy, mu) {
            self.invalidate_derived();
            return Err(error.into());
        }
        self.normalization = Some(method);
        self.e0 = self.normalization.as_ref().and_then(|m| m.get_e0());
        self.normalization_edge_step_last_result =
            self.normalization.as_ref().and_then(|m| m.get_edge_step());
        Ok(self)
    }

    /// Take ownership of background settings and clear background and Fourier results.
    /// Pass [`crate::AUTOBK`] directly, a method enum, or an optional enum.
    /// `None` selects default AUTOBK; existing normalization is retained. No background
    /// calculation is performed by this setter.
    pub fn set_background_method(
        &mut self,
        method: impl Into<Option<background::BackgroundMethod>>,
    ) -> Result<&mut Self, XAFSError> {
        let method = method.into();
        self.invalidate_background();
        if let Some(method) = method {
            self.background = Some(method);
        } else {
            let backgound_method = background::AUTOBK::new();
            self.background = Some(background::BackgroundMethod::AUTOBK(backgound_method));
        }

        Ok(self)
    }

    /// Restrict this spectrum and its clones to XANES processing (since 0.2.10).
    /// Use for calculated descendants of fluorescence-corrected spectra. This
    /// irreversible marker preserves arrays and normalization, clears EXAFS caches,
    /// and survives serialization and data edits. It does not invent a correction
    /// record: retain the actual ancestor records separately. Normalization, LCF,
    /// PCA and MCR in energy space remain available; background, Fourier, wavelet
    /// and RMC processing are rejected. No calculation is performed.
    pub fn restrict_to_xanes(&mut self) -> &mut Self {
        self.xanes_only = true;
        self.invalidate_background();
        self
    }

    /// Whether a direct correction or an inherited restriction limits processing
    /// to XANES. This does not imply a direct correction record exists.
    pub fn is_xanes_only(&self) -> bool {
        self.xanes_only || self.fluorescence_correction.is_some()
    }

    pub(crate) fn ensure_exafs_allowed(&self) -> Result<(), XAFSError> {
        if self.is_xanes_only() {
            return Err(super::errors::BackgroundError::Other { message: "The corrected branch is qualified for XANES only; use the uncorrected spectrum for EXAFS".into() }.into());
        }
        Ok(())
    }

    /// Remove the smooth background using the selected method; normalize if needed.
    /// Default nalgebra AUTOBK minimizes low-R content with a fixed endpoint penalty;
    /// the optional `ndarray-compat` backend retains its historical clamp model.
    /// Successful results are dimensionless unweighted [`Self::chi`] on [`Self::k`].
    /// Recomputes this stage and clears Fourier results on every call. Missing data,
    /// insufficient coverage, invalid settings or solver failure return a typed error.
    /// The native fluorescence-corrected XANES branch is rejected; retain the
    /// original spectrum for EXAFS because this correction is not qualified there.
    pub fn calc_background(&mut self) -> Result<&mut Self, XAFSError> {
        self.ensure_exafs_allowed()?;
        self.invalidate_background();
        if self
            .normalization
            .as_ref()
            .and_then(|m| m.get_norm())
            .is_none()
        {
            self.normalize()?;
        }
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

    /// Take ownership of forward settings and clear forward/inverse results.
    /// Existing normalization and background results are retained. Values already
    /// inferred inside `parameters`, such as `kstep`, stay explicit; use a fresh
    /// configuration or reset such fields to `None` to request new inference.
    pub fn set_fft(&mut self, parameters: xrayfft::XrayFFTF) -> &mut Self {
        self.xftf = Some(parameters);
        self.invalidate_fft();
        self
    }

    /// Weight and window χ(k), then transform it to complex Fourier distance R.
    /// Computes missing normalization/background stages first, using their defaults.
    /// The unnormalized negative-exponent FFT is multiplied by `kstep / sqrt(pi)`;
    /// no extra division by the FFT length or window area is applied.
    /// Default settings use k-weight 2, a Kaiser–Bessel window and 2048 samples.
    /// Invalid input/settings or a failed prerequisite returns a typed error.
    /// Every call recomputes the forward transform and clears inverse results.
    /// Successful calls retain resolved settings: an inferred `kstep` is reused
    /// after later background-grid changes unless reset through [`Self::set_fft`].
    pub fn fft(&mut self) -> Result<&mut Self, XAFSError> {
        self.ensure_exafs_allowed()?;
        self.invalidate_fft();
        if self.k().is_none() || self.chi().is_none() {
            self.calc_background()?;
        }
        // Work on a copy so errors retain the caller's transform parameters.
        let mut xftf = self.xftf.clone().unwrap_or_default();

        #[cfg(feature = "ndarray-compat")]
        {
            let k = self.k_view().ok_or_else(|| DataError::MissingData {
                field: "k (need to calculate background first)".to_string(),
            })?;
            let chi = self.chi_view().ok_or_else(|| DataError::MissingData {
                field: "chi (need to calculate background first)".to_string(),
            })?;
            xftf.xftf(k, chi)?;
        }
        #[cfg(not(feature = "ndarray-compat"))]
        {
            let k = self.k().ok_or_else(|| DataError::MissingData {
                field: "k (need to calculate background first)".to_string(),
            })?;
            let chi = self.chi().ok_or_else(|| DataError::MissingData {
                field: "chi (need to calculate background first)".to_string(),
            })?;
            xftf.xftf(
                &DVector::from_column_slice(k),
                &DVector::from_column_slice(chi),
            )?;
        }

        self.xftf = Some(xftf);
        Ok(self)
    }

    /// Take ownership of inverse settings, preserving forward results and clearing q/chi(q).
    /// Previously inferred `kstep`/`nfft` values in the supplied settings remain
    /// explicit. Pass a fresh configuration after changing forward-grid geometry
    /// when you want the inverse grid to be inferred again.
    pub fn set_ifft(&mut self, mut parameters: xrayfft::XrayFFTR) -> &mut Self {
        parameters.q = None;
        parameters.chiq = None;
        parameters.rwin = None;
        self.q = None;
        self.xftr = Some(parameters);
        self
    }

    /// Filter χ(R) and return a real inverse transform, computing missing stages first.
    /// Uses [`xrayfft::XrayFFTR`] defaults unless inverse settings are configured.
    /// This preserves the forward weighting/window and is not an unweighted χ(k)
    /// reconstruction. Invalid inverse settings or prerequisites return an error.
    /// Resolved inverse grid settings persist across calls; after changing forward
    /// `nfft` or spacing, reset them with [`Self::set_ifft`] to request inference.
    pub fn ifft(&mut self) -> Result<&mut Self, XAFSError> {
        self.ensure_exafs_allowed()?;
        if self.chir().is_none() {
            self.fft()?;
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
    /// the pipeline recomputes from the modified data. Resolved automatic settings
    /// are retained along with explicit settings: clearing a result does not set
    /// its inferred `kstep`, `nfft` or fitting ranges back to `None`. This also leaves
    /// `mu_stddev`, `rebinned` and the selected E0 unchanged.
    pub fn invalidate_derived(&mut self) -> &mut Self {
        if let Some(last_result) = self.normalization_edge_step_last_result.take() {
            let current = self.normalization.as_ref().and_then(|m| m.get_edge_step());
            if current != Some(last_result) {
                // The caller changed (or cleared) the public parameter after
                // normalization. Preserve that choice instead of the old scale.
                self.normalization_edge_step_override = current;
            }
        }
        match self.normalization.as_mut() {
            Some(normalization::NormalizationMethod::PrePostEdge(p)) => {
                if p.norm.is_some() {
                    p.edge_step = self.normalization_edge_step_override;
                }
                p.pre_edge = None;
                p.post_edge = None;
                p.norm = None;
                p.flat = None;
                p.pre_coefficients = None;
                p.norm_coefficients = None;
            }
            Some(normalization::NormalizationMethod::MBack(m)) => {
                if m.norm.is_some() {
                    m.edge_step = self.normalization_edge_step_override;
                }
                m.norm = None;
                m.flat = None;
                m.result = None;
            }
            None => {}
        }
        self.invalidate_background();
        self
    }

    fn invalidate_background(&mut self) {
        self.k = None;
        self.chi = None;
        self.chi_kweighted = None;
        if let Some(background::BackgroundMethod::AUTOBK(a)) = self.background.as_mut() {
            a.bkg = None;
            a.chie = None;
            a.k = None;
            a.chi = None;
        }
        self.invalidate_fft();
    }

    fn invalidate_fft(&mut self) {
        self.chi_r = None;
        self.chi_r_mag = None;
        self.chi_r_re = None;
        self.chi_r_im = None;
        self.q = None;
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

    /// Read the total constant energy correction in eV. Starts at zero and
    /// includes corrections from [`Self::align_to`], [`Self::calibrate`] and
    /// [`Self::shift_energy`]. Reading it does not run processing.
    pub fn energy_offset(&self) -> f64 {
        self.energy_shift
    }

    /// Set the total energy offset in eV (since 0.2.10).
    ///
    /// Positive values move features to higher energy. Repeated assignments do
    /// not accumulate: setting 3.5 twice keeps +3.5 eV; zero removes the recorded
    /// correction. Both working and baseline grids and existing E₀ settings move
    /// by the difference from the previous offset. Absorption values stay unchanged.
    /// Normalization, background and transform results are invalidated only when
    /// the offset changes; the next processing call recomputes them normally.
    ///
    /// This reverses constant shifts to floating-point precision, not other data
    /// edits such as truncation or rebinning. It does not retain an immutable
    /// copy of the imported file. Replacing arrays with [`Self::set_spectrum`]
    /// establishes a new baseline and resets the offset. Do not directly edit
    /// the legacy `energy_shift` field: it describes shifts already in the arrays.
    /// Existing repeated energy points are retained. Nonfinite offsets/settings,
    /// decreasing grids, overflow or newly collapsed spacing between distinct
    /// points return an error before any mutation. Empty spectra may store an offset, but loading
    /// new arrays resets it; normally call this after constructing the spectrum.
    ///
    /// ```
    /// use rexafs::Spectrum;
    /// let mut spectrum = Spectrum::from_arrays(&[8970., 8980., 8990.], &[0., 0.5, 1.])?;
    /// spectrum.set_energy_offset(3.5)?;
    /// spectrum.set_energy_offset(3.5)?;
    /// assert_eq!(spectrum.energy_offset(), 3.5);
    /// spectrum.set_energy_offset(0.0)?;
    /// assert_eq!(spectrum.energy.as_ref().unwrap()[0], 8970.);
    /// # Ok::<(), rexafs::Error>(())
    /// ```
    pub fn set_energy_offset(&mut self, offset_ev: f64) -> Result<&mut Self, XAFSError> {
        let delta = offset_ev - self.energy_shift;
        let invalid = |reason: &str| DataError::InvalidEnergyOffset {
            offset_ev,
            reason: reason.into(),
        };
        if !offset_ev.is_finite() || !delta.is_finite() {
            return Err(invalid("offset and previous correction must be finite").into());
        }
        let norm_e0 = self.normalization.as_ref().and_then(|m| m.get_e0());
        let bkg_e0 = match &self.background {
            Some(background::BackgroundMethod::AUTOBK(a)) => a.ek0,
            _ => None,
        };
        if [self.e0, norm_e0, bkg_e0]
            .into_iter()
            .flatten()
            .any(|e| !(e + delta).is_finite())
        {
            return Err(invalid("shifted E₀ must be finite").into());
        }
        for axis in [self.energy.as_ref(), self.raw_energy.as_ref()]
            .into_iter()
            .flatten()
        {
            let mut previous = None;
            for &energy in axis.iter() {
                let shifted = energy + delta;
                if !shifted.is_finite() {
                    return Err(invalid("shifted energy must be finite").into());
                }
                if previous.is_some_and(|(original, moved)| {
                    energy < original || shifted < moved || (energy > original && shifted == moved)
                }) {
                    return Err(invalid(
                        "shift must preserve energy order and distinct sample positions",
                    )
                    .into());
                }
                previous = Some((energy, shifted));
            }
        }
        if delta != 0.0 {
            self.shift_energy(delta);
        }
        self.energy_shift = offset_ev;
        Ok(self)
    }

    /// Shift the energy axis (working and raw) by `delta_ev`, moving `e0` and
    /// the e0-like stage parameters along with it. The shift accumulates in
    /// `energy_shift`. Derived results are invalidated. The shift is in eV and
    /// must be finite; this setter stores it without immediate validation.
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
                let flat = match self.flat() {
                    Some(flat) if flat.len() == energy.len() => flat,
                    _ => {
                        let mut tmp = self.clone();
                        tmp.invalidate_derived();
                        tmp.normalize()?;
                        tmp.flat().ok_or_else(|| DataError::MissingData {
                            field: "flat".to_string(),
                        })?
                    }
                };
                tools::half_step_energy(energy, &flat, e0)
            }
        }
    }

    /// Calibrate: shift the spectrum so that `feature` lands on `target_ev`
    /// and set the spectrum/normalization E0 to `target_ev`, in eV. Returns the
    /// shift applied in eV and invalidates derived results. A feature-detection
    /// failure is returned before shifting. The target must be finite; no immediate
    /// target validation is performed. Existing AUTOBK `ek0` is shifted with the
    /// axes and can differ from the target when its old reference differed from
    /// the selected feature; use [`Self::set_e0`] to synchronize it explicitly.
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
    /// on a coarse ±20 eV interval with 0.1 eV steps) is applied with
    /// [`Self::shift_energy`] and returned in eV. Refinement can move slightly
    /// outside that interval. The reference is not mutated; insufficient coverage
    /// or an invalid prerequisite returns an error. See [`tools::find_energy_shift`]
    /// for the sign convention and free amplitude scaling.
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

    /// Remove working samples nearest to target energies in eV.
    /// Returns the number of distinct working samples removed; repeated targets
    /// remove a sample once and out-of-range targets select an endpoint. When
    /// usable raw arrays exist, removes their nearest samples too. Invalidates
    /// derived results and errors if fewer than two working samples would remain.
    /// Stored `mu_stddev` is not resized or recalculated.
    pub fn deglitch_points(&mut self, energies_to_remove: &[f64]) -> Result<usize, XAFSError> {
        self.remove_points_at(energies_to_remove)
    }

    /// Remove every working point in an inclusive energy interval, in eV.
    /// Reversed bounds are swapped. Delegates the selected energies to
    /// [`Self::deglitch_points`], including its raw-array and uncertainty behavior.
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
    /// `lower_margin` below it. Bounds are in eV and margins in absorption units;
    /// negative margins use their absolute values. Returns removed energies in eV.
    /// At least two selected points must support the fitted line and two working
    /// samples must remain. Data mutation follows [`Self::deglitch_points`].
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
    /// may be `None` for no bound). Bounds are in eV and are not automatically
    /// swapped. Errors before mutation if fewer than two working samples remain.
    /// Raw arrays are truncated too when at least two raw points remain; otherwise
    /// they stay unchanged. Clears derived results but does not resize `mu_stddev`.
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
    /// per-bin standard deviation and `rebinned` is set. E0 comes from `cfg.e0`,
    /// then the current spectrum E0, then an automatic estimate used for the grid.
    /// The automatically estimated grid E0 is not copied into `self.e0` by this
    /// method. Errors from [`tools::rebin`] leave the arrays unchanged. Subsequent
    /// stages are invalidated, while their resolved parameters remain stored.
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

    /// Clone the spectrum and apply [`Self::rebin`] to the clone.
    /// A named result receives the suffix ` (rebinned)`; the original arrays,
    /// settings and results remain unchanged. Returns the same rebin errors.
    pub fn rebinned(&self, cfg: &tools::RebinConfig) -> Result<XASSpectrum, XAFSError> {
        let mut out = self.clone();
        out.rebin(cfg)?;
        if let Some(name) = self.name.as_deref() {
            out.set_name(format!("{name} (rebinned)"));
        }
        Ok(out)
    }

    /// Replace working μ(E) with a Lorentzian/Gaussian/Voigt smoothed copy.
    /// Width definitions and defaults (`sigma = 1` eV, `gamma = sigma`) follow
    /// [`tools::smooth_mu`]. Matching raw arrays receive the same result; a
    /// different usable raw grid is smoothed independently, otherwise raw data is
    /// retained. Invalidates derived results. A smoothing failure returns before
    /// arrays are replaced. Stored `mu_stddev` is not propagated through the filter.
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

    /// Return the selected or estimated edge energy in eV, or `None` before resolution.
    pub fn e0(&self) -> Option<f64> {
        self.e0
    }

    /// Fit-range endpoints that the calculated stages limited to this scan's
    /// measured data (since 0.2.15).
    ///
    /// Pre/post-edge normalization limits its energy windows (offsets from E0 in
    /// eV) and AUTOBK caps its `kmax` (Å⁻¹) instead of failing on a short scan;
    /// the stored settings keep the requested values. Each entry names the
    /// setting with its requested and used value, recomputed from the current
    /// settings and energy grid for stages that have results. Empty when nothing was limited,
    /// before calculation, for MBACK (which checks its own regions) and for the
    /// legacy `ndarray-compat` backend. Calculates nothing.
    pub fn range_adjustments(&self) -> Vec<super::ranges::RangeAdjustment> {
        let mut adjustments = Vec::new();
        #[cfg(not(feature = "ndarray-compat"))]
        if let Some(energy) = &self.energy {
            if let Some(normalization::NormalizationMethod::PrePostEdge(method)) =
                &self.normalization
            {
                if method.get_norm().is_some() {
                    adjustments.extend(method.range_adjustments(energy));
                }
            }
            if let Some(background::BackgroundMethod::AUTOBK(method)) = &self.background {
                if method.chi.is_some() {
                    adjustments.extend(method.range_adjustments(energy));
                }
            }
        }
        adjustments
    }

    /// Copy normalized absorption in edge-step units on the current energy grid.
    /// Returns `None` until normalization succeeds or after it is invalidated.
    /// When `preserves_prepared_values()` is true this copies the supplied
    /// unit-step values, including flattened values for declared `Flat` input.
    /// After explicitly setting a normalization method it returns the new fit's
    /// normalized output instead, without replacing the original input arrays.
    pub fn norm(&self) -> Option<DVector<f64>> {
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

    /// Copy flattened normalized absorption on the current energy grid.
    /// Flattening removes the fitted post-edge trend; it is a display/analysis result,
    /// not the raw-measurement input used by AUTOBK. While prepared values are
    /// preserved, `Flat` copies the supplied input and `Norm` has no flat output.
    /// Explicit pre/post-edge refitting creates a new flattened result in either
    /// case, without replacing the original input arrays.
    /// Returns `None` without a valid flattened representation.
    pub fn flat(&self) -> Option<DVector<f64>> {
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

    /// Copy the fitted pre-edge baseline on the current energy grid, in input mu units.
    /// Returns `None` without valid pre/post-edge normalization.
    pub fn pre_edge(&self) -> Option<DVector<f64>> {
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

    /// Copy the fitted post-edge normalization curve on the current energy grid,
    /// in input mu units. Returns `None` without valid pre/post-edge normalization.
    pub fn post_edge(&self) -> Option<DVector<f64>> {
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
    /// Borrow the background wave-number grid in Å⁻¹ as an ndarray view.
    /// Available with `ndarray-compat`; returns `None` before a valid background result.
    pub fn k_view(&self) -> Option<ArrayBase<ViewRepr<&f64>, Ix1>> {
        self.background.as_ref()?.get_k_view()
    }

    #[cfg(feature = "ndarray-compat")]
    /// Borrow dimensionless, unweighted EXAFS as an ndarray view.
    /// Available with `ndarray-compat`; returns `None` before a valid background result.
    pub fn chi_view(&self) -> Option<ArrayBase<ViewRepr<&f64>, Ix1>> {
        self.background.as_ref()?.get_chi_view()
    }

    /// Borrow the currently stored forward-transform k-weight, if configured.
    /// Before `fft()` resolves its settings this can be an unresolved user value.
    pub fn kweight(&self) -> Option<&f64> {
        self.xftf.as_ref()?.get_kweight()
    }

    /// Calculate an owned `chi(k) * k^w` vector on the background grid.
    /// Here `w` is the currently stored forward-transform k-weight; units are Å⁻ʷ.
    /// This getter applies no window and does not resample onto the Larch FFT grid.
    /// Returns `None` if background results or the forward k-weight are unavailable.
    pub fn chi_kweighted(&self) -> Option<DVector<f64>> {
        let k = DVector::from_column_slice(self.k()?);
        let chi = DVector::from_column_slice(self.chi()?);
        let kweight = self.kweight()?;

        Some(chi.component_mul(&k.map(|x| x.powf(kweight.to_owned()))))
    }

    /// Borrow the complete stored one-sided complex real-FFT representation.
    /// Unlike the component getters, this includes frequencies beyond `rmax_out`.
    /// Returns `None` without a valid forward transform; no buffer is cloned.
    pub fn chir(&self) -> Option<&DynRealDft<f64>> {
        self.xftf.as_ref()?.get_chir()
    }

    /// Copy the forward-transform magnitude on [`Self::r`], limited by `rmax_out`.
    /// For dimensionless chi and k-weight w, units are Å⁻⁽ʷ⁺¹⁾. Returns `None` without
    /// a valid forward transform. These magnitudes are not normalized to a peak height.
    pub fn chir_mag(&self) -> Option<DVector<f64>> {
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

    /// Wavenumbers for `kwin()`. In Larch mode these follow the resampled
    /// FFT grid, which can differ from the background's `k()` spacing.
    pub fn kwin_k(&self) -> Option<DVector<f64>> {
        let ft = self.xftf.as_ref()?;
        let len = ft.get_kwin()?.len();
        if ft.grid == super::xrayfft::FFTGrid::Larch {
            let step = *ft.get_kstep()?;
            Some(DVector::from_iterator(
                len,
                (0..len).map(|i| i as f64 * step),
            ))
        } else {
            Some(DVector::from_column_slice(self.k()?))
        }
    }

    /// Copy the dimensionless forward-transform window. Pair it with [`Self::kwin_k`],
    /// which can differ from the background grid in Larch mode. Returns `None` without
    /// a valid forward transform.
    pub fn kwin(&self) -> Option<DVector<f64>> {
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

    /// Copy the real Fourier component on [`Self::r`], limited by `rmax_out`.
    /// Units are Å⁻⁽ʷ⁺¹⁾ for dimensionless chi and k-weight w; the forward exponent
    /// is negative. Returns `None` without a valid forward transform.
    pub fn chir_real(&self) -> Option<DVector<f64>> {
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

    /// Copy the imaginary Fourier component on [`Self::r`], limited by `rmax_out`.
    /// Units are Å⁻⁽ʷ⁺¹⁾ for dimensionless chi and k-weight w; the forward exponent
    /// is negative. Returns `None` without a valid forward transform.
    pub fn chir_imag(&self) -> Option<DVector<f64>> {
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

    /// Copy the reported Fourier distance grid in Å, limited by `rmax_out`.
    /// Scattering phase shifts are not corrected: a peak position is not directly a
    /// bond length. Returns `None` without a valid forward transform.
    pub fn r(&self) -> Option<DVector<f64>> {
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

    /// Copy the inverse-transform wave-number grid in Å⁻¹, limited by `qmax_out`.
    /// Returns `None` without a valid inverse transform.
    pub fn q(&self) -> Option<DVector<f64>> {
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

    /// Copy the real inverse-transform signal on [`Self::q`].
    /// Forward k-weighting and windowing remain in the signal; with inverse r-weight
    /// zero its units are Å⁻ʷ for forward weight w. Returns `None` without a valid
    /// inverse transform. This does not reconstruct removed background or lost data.
    pub fn chiq(&self) -> Option<DVector<f64>> {
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
    fn test_k_chi_slices_match_ndarray_views() -> Result<(), Box<dyn std::error::Error>> {
        let path = String::from(TOP_DIR) + "/tests/testfiles/Ru_QAS.dat";
        let mut spectrum = io::load_spectrum_QAS_trans(&path)?;
        spectrum.calc_background()?;

        let k_slice = spectrum.k().unwrap();
        let chi_slice = spectrum.chi().unwrap();
        let k_view = spectrum.k_view().unwrap();
        let chi_view = spectrum.chi_view().unwrap();

        assert_eq!(k_slice.len(), k_view.len());
        assert_eq!(chi_slice.len(), chi_view.len());

        for (slice, view) in k_slice.iter().zip(k_view.iter()) {
            assert_abs_diff_eq!(slice, view, epsilon = TEST_TOL);
        }
        for (slice, view) in chi_slice.iter().zip(chi_view.iter()) {
            assert_abs_diff_eq!(slice, view, epsilon = TEST_TOL_LESS_ACC);
        }

        spectrum.fft()?;
        assert!(spectrum.chir_mag().is_some());
        Ok(())
    }
}
