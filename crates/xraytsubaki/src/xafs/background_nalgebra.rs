#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

// Import standard library dependencies
use std::collections::hash_map::DefaultHasher;
use std::error::Error;
use std::hash::Hasher;
use std::ops::Deref;
use std::sync::{Arc, Mutex, OnceLock};

// Import external dependencies
use levenberg_marquardt::{LeastSquaresProblem, LevenbergMarquardt};
use nalgebra::{DMatrix, DVector, Dyn, Owned};
use rusty_fitpack;
use serde::{Deserialize, Serialize};

// Import internal dependencies
use super::errors::{BackgroundError, DataError};
use super::lmutils::LMParameters;
use super::mathutils::{self, splev_jacobian, MathUtils};
use super::normalization::{self, Normalization};
use super::xafsutils::FTWindow;
use super::xrayfft::{FFTUtils, XFFTReverse, XFFT};
use super::{xafsutils, xrayfft};

const DEFAULT_LINEAR_REGULARIZATION: f64 = 1.0e-4;
const DEFAULT_LINEAR_CONDITION_LIMIT: f64 = 1.0e8;
const DEFAULT_LINEAR_RESIDUAL_RATIO_LIMIT: f64 = 1.05;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AUTOBKSolver {
    LegacyLm,
    LinearDirect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AUTOBKClampScalePolicy {
    Fixed,
    TwoPass,
}

#[derive(Debug, Clone)]
struct AutobkLinearWorkspace {
    signature: u64,
    design_matrix: Arc<DMatrix<f64>>,
}

fn autobk_workspace_cache() -> &'static Mutex<Option<AutobkLinearWorkspace>> {
    static CACHE: OnceLock<Mutex<Option<AutobkLinearWorkspace>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

/// Enum for background subtraction methods
/// AUTOBK: M. Newville, P. Livins, Y. Yacoby, J. J. Rehr, and E. A. Stern. Near-edge x-ray-absorption fine structure of Pb: A comparison of theory and experiment. Phys. Rev. B, 47:14126–14131, Jun 1993. doi:10.1103/PhysRevB.47.14126.
/// ILPBkg: To be implemented
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BackgroundMethod {
    AUTOBK(AUTOBK),
    ILPBkg(ILPBkg),
    None,
}

impl Default for BackgroundMethod {
    fn default() -> Self {
        BackgroundMethod::AUTOBK(AUTOBK::default())
    }
}

impl BackgroundMethod {
    pub fn new() -> BackgroundMethod {
        BackgroundMethod::AUTOBK(AUTOBK::new())
    }

    pub fn new_autobk() -> BackgroundMethod {
        BackgroundMethod::AUTOBK(AUTOBK::new())
    }

    pub fn new_ilpbkg() -> BackgroundMethod {
        BackgroundMethod::ILPBkg(ILPBkg::new())
    }

    pub fn calc_background(
        &mut self,
        energy: &DVector<f64>,
        mu: &DVector<f64>,
        normalization_param: &mut Option<normalization::NormalizationMethod>,
    ) -> Result<&mut Self, BackgroundError> {
        match self {
            BackgroundMethod::AUTOBK(autobk) => {
                autobk.calc_background(energy, mu, normalization_param)?;
                Ok(self)
            }
            BackgroundMethod::ILPBkg(_ilpbkg) => Err(BackgroundError::NotImplemented {
                feature: "ILPBkg background removal".to_string(),
            }),
            BackgroundMethod::None => Ok(self),
        }
    }

    pub fn get_k(&self) -> Option<DVector<f64>> {
        match self {
            BackgroundMethod::AUTOBK(autobk) => autobk.get_k().cloned(),
            _ => None,
        }
    }

    pub fn get_chi(&self) -> Option<DVector<f64>> {
        match self {
            BackgroundMethod::AUTOBK(autobk) => autobk.get_chi().cloned(),
            _ => None,
        }
    }
}

/// Struct for AUTOBK
///
/// Parameters and the output are stored in this struct
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AUTOBK {
    /// Edge energy in eV (this is used for starting point of k). If None, it will be determined.
    pub ek0: Option<f64>,
    /// Rbkg parameter: distance (in Ang) for chi(R) above which the signal is ignored. Default = 1.
    pub rbkg: Option<f64>,
    /// Number of knots in spline. If None, it will be determined.
    pub nknots: Option<i32>,
    /// Minimum k value. Default = 0.
    pub kmin: Option<f64>,
    /// Maximum k value. Default = full data range.
    pub kmax: Option<f64>,
    /// k step size to use for FFT. Default = 0.05.
    pub kstep: Option<f64>,
    /// Number of energy end-points for clamp. Default = 3.
    pub nclamp: Option<i32>,
    /// Weight of low-energy clamp. Default = 0.
    pub clamp_lo: Option<i32>,
    /// Weight of high-energy clamp. Default = 1.
    pub clamp_hi: Option<i32>,
    /// Array size to use for FFT. Default = 2048.
    pub nfft: Option<i32>,
    /// Optional chi array for standard chi(k).
    pub chi_std: Option<DVector<f64>>,
    /// Optional k array for standard chi(k).
    pub k_std: Option<DVector<f64>>,
    /// k weight for FFT. Default = 1.
    pub kweight: Option<i32>,
    /// FFT window function name. Default = Hanning.
    pub window: FTWindow,
    /// FFT window window parameter. Default = 0.1.
    pub dk: Option<f64>,
    /// Solver backend for AUTOBK spline optimization.
    pub solver: Option<AUTOBKSolver>,
    /// Clamp scaling policy used by direct solver.
    pub clamp_scale_policy: Option<AUTOBKClampScalePolicy>,
    /// Ridge (Tikhonov) regularization magnitude for the direct solver's augmented least-squares system.
    pub linear_regularization: Option<f64>,
    /// Condition proxy threshold used to reject unstable direct solves.
    pub linear_condition_limit: Option<f64>,
    /// Maximum accepted solved/base residual norm ratio for direct solver.
    pub linear_residual_ratio_limit: Option<f64>,
    /// If true, direct-solver failures fall back to legacy LM automatically.
    pub linear_fallback_to_lm: Option<bool>,
    /// If true, cache direct-solver design matrices for compatible workloads.
    pub linear_workspace_cache: Option<bool>,
    /// Background of mu(E)
    pub bkg: Option<DVector<f64>>,
    /// Edge normalized mu(E) - bkg
    pub chie: Option<DVector<f64>>,
    /// k grid
    pub k: Option<DVector<f64>>,
    /// chi(k)
    pub chi: Option<DVector<f64>>,
}

impl Default for AUTOBK {
    fn default() -> Self {
        AUTOBK {
            ek0: None,
            rbkg: Some(1.0),
            nknots: None,
            kmin: Some(0.0),
            kmax: None,
            kstep: Some(0.05),
            nclamp: Some(3),
            clamp_lo: Some(0),
            clamp_hi: Some(1),
            nfft: Some(2048),
            chi_std: None,
            k_std: None,
            kweight: Some(1),
            window: FTWindow::Hanning,
            dk: Some(0.1),
            solver: Some(AUTOBKSolver::LinearDirect),
            clamp_scale_policy: Some(AUTOBKClampScalePolicy::Fixed),
            linear_regularization: Some(DEFAULT_LINEAR_REGULARIZATION),
            linear_condition_limit: Some(DEFAULT_LINEAR_CONDITION_LIMIT),
            linear_residual_ratio_limit: Some(DEFAULT_LINEAR_RESIDUAL_RATIO_LIMIT),
            linear_fallback_to_lm: Some(true),
            linear_workspace_cache: Some(true),
            bkg: None,
            chie: None,
            k: None,
            chi: None,
        }
    }
}

/// Implementation of AUTOBK
impl AUTOBK {
    pub fn new() -> AUTOBK {
        AUTOBK::default()
    }

    /// Fill in default values for parameters that are not set
    pub fn fill_parameter(&mut self) -> Result<(), BackgroundError> {
        if self.rbkg.is_none() {
            self.rbkg = Some(1.0);
        }

        if self.kmin.is_none() {
            self.kmin = Some(0.0);
        }

        if self.kstep.is_none() {
            self.kstep = Some(0.05);
        }

        if self.nclamp.is_none() {
            self.nclamp = Some(3);
        }

        if self.clamp_lo.is_none() {
            self.clamp_lo = Some(0);
        }

        if self.clamp_hi.is_none() {
            self.clamp_hi = Some(1);
        }

        if self.nfft.is_none() {
            self.nfft = Some(2048);
        }

        if self.kweight.is_none() {
            self.kweight = Some(1);
        }

        if self.dk.is_none() {
            self.dk = Some(0.1);
        }

        if self.solver.is_none() {
            self.solver = Some(AUTOBKSolver::LinearDirect);
        }

        if self.clamp_scale_policy.is_none() {
            self.clamp_scale_policy = Some(AUTOBKClampScalePolicy::Fixed);
        }

        if self.linear_regularization.is_none() {
            self.linear_regularization = Some(DEFAULT_LINEAR_REGULARIZATION);
        }

        if self.linear_condition_limit.is_none() {
            self.linear_condition_limit = Some(DEFAULT_LINEAR_CONDITION_LIMIT);
        }

        if self.linear_residual_ratio_limit.is_none() {
            self.linear_residual_ratio_limit = Some(DEFAULT_LINEAR_RESIDUAL_RATIO_LIMIT);
        }

        if self.linear_fallback_to_lm.is_none() {
            self.linear_fallback_to_lm = Some(true);
        }

        if self.linear_workspace_cache.is_none() {
            self.linear_workspace_cache = Some(true);
        }

        Ok(())
    }

    fn validate_input_vectors(
        energy: &DVector<f64>,
        mu: &DVector<f64>,
    ) -> Result<(), BackgroundError> {
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

    fn build_knot_domain(spl_k: &DVector<f64>, order: usize) -> Result<Vec<f64>, BackgroundError> {
        if spl_k.is_empty() {
            return Err(BackgroundError::SplineKnotsFailed {
                kmin: 0.0,
                kmax: 0.0,
            });
        }

        let nspl = spl_k.len();
        if nspl <= order + 1 {
            return Err(BackgroundError::Other {
                message: format!(
                    "insufficient spline control points for order {}: {}",
                    order, nspl
                ),
            });
        }

        let qmin = spl_k[0];
        let qmax = spl_k[nspl - 1];
        if !qmin.is_finite() || !qmax.is_finite() || qmax <= qmin {
            return Err(BackgroundError::SplineKnotsFailed {
                kmin: qmin,
                kmax: qmax,
            });
        }

        let mut knots = Vec::with_capacity(nspl + order);
        for i in 0..order {
            knots.push(qmin - 1e-4 * (order - i) as f64);
        }
        let interior_count = nspl - order;
        if interior_count < 2 {
            return Err(BackgroundError::Other {
                message: format!(
                    "insufficient interior knots for order {} and control points {}",
                    order, nspl
                ),
            });
        }
        for j in 0..interior_count {
            knots.push(qmin + j as f64 * (qmax - qmin) / (interior_count - 1) as f64);
        }
        let qlast = *knots.last().ok_or_else(|| BackgroundError::Other {
            message: "failed to build interior knot domain".to_string(),
        })?;
        for i in 0..order {
            knots.push(qlast + 1e-4 * (i + 1) as f64);
        }

        Ok(knots)
    }

    fn solve_lm_problem(problem: AUTOBKSpline) -> Result<AUTOBKSpline, BackgroundError> {
        let (fit_result, _report) = LevenbergMarquardt::new()
            .with_gtol(1.0e-5)
            .with_ftol(1.0e-5)
            .with_xtol(1.0e-5)
            .with_stepbound(1.0e-5)
            .minimize(problem);

        Ok(fit_result)
    }

    fn solve_spline_problem(
        &self,
        spline_opt: AUTOBKSpline,
    ) -> Result<AUTOBKSpline, BackgroundError> {
        match self.solver.unwrap_or(AUTOBKSolver::LinearDirect) {
            AUTOBKSolver::LegacyLm => Self::solve_lm_problem(spline_opt),
            AUTOBKSolver::LinearDirect => {
                let clamp_policy = self
                    .clamp_scale_policy
                    .unwrap_or(AUTOBKClampScalePolicy::Fixed);
                let regularization = self
                    .linear_regularization
                    .unwrap_or(DEFAULT_LINEAR_REGULARIZATION)
                    .max(0.0);
                let condition_limit = self
                    .linear_condition_limit
                    .unwrap_or(DEFAULT_LINEAR_CONDITION_LIMIT)
                    .max(1.0);
                let residual_ratio_limit = self
                    .linear_residual_ratio_limit
                    .unwrap_or(DEFAULT_LINEAR_RESIDUAL_RATIO_LIMIT)
                    .max(1.0);
                let use_workspace_cache = self.linear_workspace_cache.unwrap_or(true);
                let fallback_to_lm = self.linear_fallback_to_lm.unwrap_or(true);

                match spline_opt.solve_linear_direct(
                    clamp_policy,
                    regularization,
                    condition_limit,
                    residual_ratio_limit,
                    use_workspace_cache,
                ) {
                    Ok(coefs) => {
                        let mut fit_result = spline_opt;
                        fit_result.coefs = coefs;
                        Ok(fit_result)
                    }
                    Err(linear_err) => {
                        if fallback_to_lm {
                            Self::solve_lm_problem(spline_opt)
                        } else {
                            Err(linear_err)
                        }
                    }
                }
            }
        }
    }

    /// Calculate background
    ///
    /// # Arguments
    ///
    /// * `energy` - 1-d array of x-ray energies, in eV, or group
    /// * `mu` - 1-d array of mu(E)
    /// * `normalization_param` - xraytsubaki::normalization::NormalizationMethod struct which contains parameters for normalization
    ///
    /// # Example
    ///
    /// TODO: Add example
    ///
    pub fn calc_background(
        &mut self,
        energy: &DVector<f64>,
        mu: &DVector<f64>,
        normalization_param: &mut Option<normalization::NormalizationMethod>,
    ) -> Result<&mut Self, BackgroundError> {
        // Fill in default values for parameters that are not set
        self.fill_parameter()?;
        Self::validate_input_vectors(energy, mu)?;

        let rbkg = self.rbkg.ok_or_else(|| DataError::MissingData {
            field: "rbkg".to_string(),
        })?;
        if !rbkg.is_finite() || rbkg <= 0.0 {
            return Err(BackgroundError::InvalidRbkg { rbkg });
        }

        let kmin = self.kmin.ok_or_else(|| DataError::MissingData {
            field: "kmin".to_string(),
        })?;
        let kstep = self.kstep.ok_or_else(|| DataError::MissingData {
            field: "kstep".to_string(),
        })?;
        if !kstep.is_finite() || kstep <= 0.0 {
            return Err(BackgroundError::Other {
                message: format!("invalid kstep: {}", kstep),
            });
        }
        let nfft = self.nfft.ok_or_else(|| DataError::MissingData {
            field: "nfft".to_string(),
        })?;
        if nfft <= 0 {
            return Err(BackgroundError::Other {
                message: format!("invalid nfft: {}", nfft),
            });
        }
        let kweight = self.kweight.ok_or_else(|| DataError::MissingData {
            field: "kweight".to_string(),
        })?;
        let nclamp = self.nclamp.ok_or_else(|| DataError::MissingData {
            field: "nclamp".to_string(),
        })?;
        let clamp_lo = self.clamp_lo.ok_or_else(|| DataError::MissingData {
            field: "clamp_lo".to_string(),
        })?;
        let clamp_hi = self.clamp_hi.ok_or_else(|| DataError::MissingData {
            field: "clamp_hi".to_string(),
        })?;
        let energy_dv = energy;
        let mu_dv = mu;

        let energy = energy_dv;
        let mu = mu_dv;

        // Perform normalization if necessary

        if normalization_param.is_none() {
            let mut normalization_method = normalization::PrePostEdge::new();
            normalization_method.set_e0(self.ek0);
            *normalization_param =
                Some(normalization::NormalizationMethod::PrePostEdge(normalization_method));
        }
        let normalization_method =
            normalization_param
                .as_mut()
                .ok_or_else(|| DataError::MissingData {
                    field: "normalization method".to_string(),
                })?;

        if let Some(ek0) = self.ek0 {
            if ek0 < energy_dv.min() || ek0 > energy_dv.max() {
                self.ek0 = None;
                normalization_method.set_e0(None);
            }
        }

        let e0 = normalization_method.get_e0();
        let mut edge_step = normalization_method.get_edge_step();
        let ek0 = self.ek0;

        if (ek0.is_none() && e0.is_none()) || edge_step.is_none() {
            normalization_method.normalize(energy_dv, mu_dv)?;
            edge_step = normalization_method.get_edge_step();
        }

        let ek0 = if self.ek0.is_none() {
            normalization_method.get_e0()
        } else {
            self.ek0
        }
        .ok_or_else(|| DataError::MissingData {
            field: "ek0/e0".to_string(),
        })?;
        self.ek0 = Some(ek0);

        let edge_step = edge_step
            .or_else(|| normalization_method.get_edge_step())
            .ok_or_else(|| DataError::MissingData {
                field: "edge_step".to_string(),
            })?;
        if !edge_step.is_finite() {
            return Err(BackgroundError::Other {
                message: format!("edge_step is not finite: {}", edge_step),
            });
        }
        let edge_step = edge_step.max(1.0e-12);
        // Rbkg Algorithm
        let energy_slice = energy.as_slice();
        let iek0 = mathutils::index_of_sorted(energy_slice, &ek0)?;
        let mut rgrid = std::f64::consts::PI / (kstep * nfft as f64);

        if rbkg < (2.0 * rgrid) {
            rgrid *= 2.0;
        }

        let enpe = DVector::from_iterator(
            energy.len() - iek0,
            energy.iter().skip(iek0).map(|x| x - ek0),
        );
        let kraw = enpe.map(|x| x.signum() * (xafsutils::constants::ETOK * x.abs()).sqrt());

        let kmax = self
            .kmax
            .map(|max_value| max_value.min(kraw.max()).max(0.0))
            .unwrap_or_else(|| kraw.max());
        if kmax <= kmin {
            return Err(BackgroundError::SplineKnotsFailed { kmin, kmax });
        }

        let kout = dvector_arange(0.0, (1.01 + kmax / kstep).floor(), 1.0) * kstep;
        if kout.len() < 2 {
            return Err(DataError::InsufficientData {
                min: 2,
                actual: kout.len(),
            }
            .into());
        }

        let iemax = energy.len().min(
            2 + mathutils::index_of_sorted(
                energy_slice,
                &(ek0 + kmax.powi(2) / xafsutils::constants::ETOK),
            )?,
        ) - 1;
        if iemax <= iek0 {
            return Err(DataError::InsufficientData {
                min: 3,
                actual: iemax.saturating_sub(iek0) + 1,
            }
            .into());
        }

        let chi_std = match (&self.k_std, &self.chi_std) {
            (Some(k_std), Some(chi_std)) => Some(
                kout.interpolate(k_std.as_slice(), chi_std.as_slice())?,
            ),
            (None, None) => None,
            _ => {
                return Err(DataError::MissingData {
                    field: "k_std and chi_std must both be set".to_string(),
                }
                .into())
            }
        };

        let ftwin = kout
            .map(|x| x.powi(kweight))
            .component_mul(&xafsutils::ftwindow(
                &kout,
                Some(kmin),
                Some(kmax),
                self.dk,
                self.dk,
                Some(self.window),
            )?);

        let mut nspl = 1 + (2.0 * rbkg * (kmax - kmin) / std::f64::consts::PI).round() as i32;
        let irbkg = (1.0 + (nspl - 1) as f64 * std::f64::consts::PI / (2.0 * rgrid * (kmax - kmin)))
            .round() as i32;

        if let Some(nknots) = self.nknots {
            nspl = nknots;
        }

        nspl = nspl.clamp(5, 128);

        // !todo!("Finish implementing this part of the code");
        let mut spl_y = DVector::from_element(nspl as usize, 1.0);
        let mut spl_k = DVector::zeros(nspl as usize);
        let kraw_slice = kraw.as_slice();
        for i in 0..nspl as usize {
            let q = kmin + i as f64 * (kmax - kmin) / (nspl - 1) as f64;
            let ik = mathutils::index_nearest_sorted(kraw_slice, &q)?;
            let i1 = (ik + 5).min(kraw.len() - 1);
            let i2 = (ik as i32 - 5).max(0) as usize;
            spl_k[i] = kraw[ik];
            spl_y[i] = (2.0 * mu[ik + iek0] + mu[i1 + iek0] + mu[i2 + iek0]) / 4.0;
        }

        let order = 3;
        // Validate knot-domain construction used for AUTOBK bounds reasoning.
        let _knot_domain = Self::build_knot_domain(&spl_k, order)?;
        let (knots, coefs, _) = rusty_fitpack::splrep(
            spl_k.as_slice().to_vec(),
            spl_y.as_slice().to_vec(),
            None,
            None,
            None,
            Some(order),
            None,
            None,
            None,
            None,
            None,
            None,
        );

        // Calculate the mu interpolated to the k grid
        let kraw_fit = kraw.rows(0, iemax - iek0 + 1).into_owned();
        let mu_fit = mu.rows(iek0, iemax - iek0 + 1).into_owned();
        let mu_out = kout.interpolate(kraw_fit.as_slice(), mu_fit.as_slice())?;

        let spline_opt = AUTOBKSpline {
            coefs: DVector::from_vec(coefs),
            knots: DVector::from_vec(knots),
            order,
            irbkg: irbkg.max(1) as usize,
            nfft: nfft as usize,
            kraw: kraw_fit,
            mu: mu_out,
            kout: kout.clone(),
            ftwin,
            kweight,
            chi_std,
            nclamp,
            clamp_lo,
            clamp_hi,
            kstep,
            ..Default::default()
        };

        let fit_result = self.solve_spline_problem(spline_opt)?;

        let (bkg, chi) = spline_eval_nalgebra(
            &fit_result.kraw,
            &fit_result.mu,
            &fit_result.knots,
            &fit_result.coefs,
            fit_result.order,
            &fit_result.kout,
        );

        let mut obkg = mu.clone();
        for i in 0..bkg.len() {
            obkg[iek0 + i] = bkg[i];
        }

        self.chie = Some((mu - &obkg) / edge_step);
        self.bkg = Some(obkg);
        self.k = Some(kout);
        self.chi = Some(chi / edge_step);

        Ok(self)
    }

    pub fn get_ek0(&self) -> Option<&f64> {
        self.ek0.as_ref()
    }

    pub fn get_rbkg(&self) -> Option<&f64> {
        self.rbkg.as_ref()
    }

    pub fn get_nknots(&self) -> Option<&i32> {
        self.nknots.as_ref()
    }

    pub fn get_kmin(&self) -> Option<&f64> {
        self.kmin.as_ref()
    }

    pub fn get_kmax(&self) -> Option<&f64> {
        self.kmax.as_ref()
    }

    pub fn get_kstep(&self) -> Option<&f64> {
        self.kstep.as_ref()
    }

    pub fn get_nclamp(&self) -> Option<&i32> {
        self.nclamp.as_ref()
    }

    pub fn get_clamp_lo(&self) -> Option<&i32> {
        self.clamp_lo.as_ref()
    }

    pub fn get_clamp_hi(&self) -> Option<&i32> {
        self.clamp_hi.as_ref()
    }

    pub fn get_nfft(&self) -> Option<&i32> {
        self.nfft.as_ref()
    }

    pub fn get_chi_std(&self) -> Option<&DVector<f64>> {
        self.chi_std.as_ref()
    }

    pub fn get_k_std(&self) -> Option<&DVector<f64>> {
        self.k_std.as_ref()
    }

    pub fn get_kweight(&self) -> Option<&i32> {
        self.kweight.as_ref()
    }

    pub fn get_window(&self) -> FTWindow {
        self.window
    }

    pub fn get_dk(&self) -> Option<&f64> {
        self.dk.as_ref()
    }

    pub fn get_bkg(&self) -> Option<&DVector<f64>> {
        self.bkg.as_ref()
    }

    pub fn get_chie(&self) -> Option<&DVector<f64>> {
        self.chie.as_ref()
    }

    pub fn get_k(&self) -> Option<&DVector<f64>> {
        self.k.as_ref()
    }

    pub fn get_chi(&self) -> Option<&DVector<f64>> {
        self.chi.as_ref()
    }

    pub fn get_chi_kweighted(&self) -> Option<DVector<f64>> {
        let kweight = self.kweight?;
        let k = self.k.clone()?;
        let chi = self.chi.clone()?;

        if kweight == 0 {
            Some(chi)
        } else {
            Some(chi.component_mul(&k.map(|x| x.powi(kweight))))
        }
    }

    pub fn get_ftwin(&self) -> Option<DVector<f64>> {
        let k = self.k.as_ref()?;

        let ftwin =
            xafsutils::ftwindow(k, self.kmin, self.kmax, self.dk, self.dk, Some(self.window))
                .ok()?;

        Some(ftwin)
    }
}

/// Evaluation of the spline used in AUTOBK
///
/// In puts and outputs are in DVector struct from nalgebra crate
///
/// # Arguments
///
/// * `kraw` - kraw, the k grid converted from energy
/// * `mu` - mu(E)
/// * `knots` - knots of the spline
/// * `coefs` - coefficients of the spline
/// * `order` - order of the spline
/// * `kout` - k grid ready for FFT
fn spline_eval_nalgebra(
    kraw: &DVector<f64>,
    mu: &DVector<f64>,
    knots: &DVector<f64>,
    coefs: &DVector<f64>,
    order: usize,
    kout: &DVector<f64>,
) -> (DVector<f64>, DVector<f64>) {
    let knots_vec = knots.as_slice().to_vec();
    let coefs_vec = coefs.as_slice().to_vec();

    let bkg = DVector::from_vec(rusty_fitpack::splev(
        knots_vec.clone(),
        coefs_vec.clone(),
        order,
        kraw.as_slice().to_vec(),
        3,
    ));

    // experimental
    let bkg_out = DVector::from_vec(rusty_fitpack::splev(
        knots_vec,
        coefs_vec,
        order,
        kout.as_slice().to_vec(),
        3,
    ));

    let chi = mu - &bkg_out;

    (bkg, chi.clone())
}

/// Struct for solving Levenberg-Marquardt optimization for AUTOBK
#[derive(Debug, Clone, PartialEq)]
struct AUTOBKSpline {
    pub coefs: DVector<f64>,
    pub knots: DVector<f64>,
    pub order: usize,
    pub irbkg: usize,
    pub nfft: usize,
    pub kraw: DVector<f64>,
    pub mu: DVector<f64>,
    pub kout: DVector<f64>,
    pub ftwin: DVector<f64>,
    pub kweight: i32,
    pub chi_std: Option<DVector<f64>>,
    pub nclamp: i32,
    pub clamp_lo: i32,
    pub clamp_hi: i32,
    pub kstep: f64,
    pub scale: f64,
}

impl Default for AUTOBKSpline {
    fn default() -> Self {
        AUTOBKSpline {
            coefs: DVector::zeros(0),
            knots: DVector::zeros(0),
            order: 3,
            irbkg: 1,
            nfft: 2048,
            kraw: DVector::zeros(0),
            mu: DVector::zeros(0),
            kout: DVector::zeros(0),
            ftwin: DVector::zeros(0),
            kweight: 1,
            chi_std: None,
            nclamp: 0,
            clamp_lo: 1,
            clamp_hi: 1,
            kstep: 0.05,
            scale: 1.0,
        }
    }
}

impl AUTOBKSpline {
    fn chi_for_coefs(&self, coefs: &DVector<f64>) -> DVector<f64> {
        let (_, chi) = spline_eval_nalgebra(
            &self.kraw,
            &self.mu,
            &self.knots,
            coefs,
            self.order,
            &self.kout,
        );

        if let Some(chi_std) = self.chi_std.as_ref() {
            chi - chi_std
        } else {
            chi
        }
    }

    fn fft_residual_head(&self, chi: &DVector<f64>) -> DVector<f64> {
        let fft = chi
            .component_mul(&self.ftwin)
            .xftf_fast(self.nfft, self.kstep);
        let upper = self.irbkg.min(fft.len());
        fft[..upper].realimg()
    }

    fn clamp_len(&self, vec_len: usize) -> usize {
        if self.nclamp <= 0 {
            return 0;
        }
        (self.nclamp as usize).min(vec_len.saturating_sub(1))
    }

    fn clamp_scale(&self, coefs: &DVector<f64>) -> f64 {
        if self.nclamp == 0 {
            return 1.0;
        }
        let chi = self.chi_for_coefs(coefs);
        let out = self.fft_residual_head(&chi);
        if out.is_empty() {
            return 1.0;
        }

        1.0 + 100.0 * out.dot(&out) / out.len() as f64
    }

    /// The Loss function in 1-d array for the Levenberg-Marquardt optimization
    pub fn residual_vec_with_scale(
        &self,
        coefs: &DVector<f64>,
        clamp_scale_override: Option<f64>,
    ) -> DVector<f64> {
        let chi = self.chi_for_coefs(coefs);
        let mut out = self.fft_residual_head(&chi);

        if self.nclamp == 0 {
            return out;
        }

        let nclamp = self.clamp_len(chi.len());
        if nclamp == 0 {
            return out;
        }

        let scale = clamp_scale_override.unwrap_or_else(|| self.clamp_scale(coefs));
        let low_clamp = self.clamp_lo as f64 * scale * chi.view((0, 0), (nclamp, 1));
        let high_start = chi.len() - nclamp - 1;
        let high_clamp = self.clamp_hi as f64 * scale * chi.view((high_start, 0), (nclamp, 1));

        out.extend(low_clamp.data.as_vec().to_owned());
        out.extend(high_clamp.data.as_vec().to_owned());
        out
    }

    pub fn residual_vec(&self, coefs: &DVector<f64>) -> DVector<f64> {
        self.residual_vec_with_scale(coefs, None)
    }

    pub fn residual_jacobian_with_scale(
        &self,
        coefs: &DVector<f64>,
        clamp_scale_override: Option<f64>,
    ) -> DMatrix<f64> {
        let scale = if self.nclamp != 0 {
            clamp_scale_override.unwrap_or_else(|| self.clamp_scale(coefs))
        } else {
            1.0
        };

        let spline_jacobian = -splev_jacobian(
            self.knots.data.as_vec().clone(),
            self.coefs.data.as_vec().clone(),
            self.order,
            self.kout.data.as_vec().clone(),
            3,
        );

        let jacobian_columns = spline_jacobian
            .column_iter()
            .map(|chi_der| {
                let fft = chi_der
                    .component_mul(&self.ftwin)
                    .xftf_fast(self.nfft, self.kstep);
                let upper = self.irbkg.min(fft.len());
                let mut out: DVector<f64> = fft[..upper].realimg();

                if self.nclamp == 0 {
                    return out;
                }

                let nclamp = self.clamp_len(chi_der.len());
                if nclamp == 0 {
                    return out;
                }

                let low_clamp = self.clamp_lo as f64 * scale * chi_der.view((0, 0), (nclamp, 1));
                let high_start = chi_der.len() - nclamp - 1;
                let high_clamp =
                    self.clamp_hi as f64 * scale * chi_der.view((high_start, 0), (nclamp, 1));

                out.extend(low_clamp.data.as_vec().to_owned());
                out.extend(high_clamp.data.as_vec().to_owned());
                out
            })
            .collect::<Vec<DVector<f64>>>();

        DMatrix::from_columns(&jacobian_columns)
    }

    pub fn residual_jacobian(&self, coefs: &DVector<f64>) -> DMatrix<f64> {
        self.residual_jacobian_with_scale(coefs, None)
    }

    fn workspace_signature(&self, clamp_scale: f64) -> u64 {
        let mut hasher = DefaultHasher::new();
        hasher.write_u64(self.order as u64);
        hasher.write_u64(self.irbkg as u64);
        hasher.write_u64(self.nfft as u64);
        hasher.write_u64(self.nclamp.max(0) as u64);
        hasher.write_u64(self.clamp_lo.max(0) as u64);
        hasher.write_u64(self.clamp_hi.max(0) as u64);
        hasher.write_u64(self.kweight.max(0) as u64);
        hasher.write_u64(clamp_scale.to_bits());
        hasher.write_u64(self.kstep.to_bits());
        hasher.write_u64(self.knots.len() as u64);
        hasher.write_u64(self.coefs.len() as u64);
        hasher.write_u64(self.kout.len() as u64);
        hasher.write_u64(self.ftwin.len() as u64);

        for value in self.knots.iter() {
            hasher.write_u64(value.to_bits());
        }
        for value in self.kout.iter() {
            hasher.write_u64(value.to_bits());
        }
        for value in self.ftwin.iter() {
            hasher.write_u64(value.to_bits());
        }

        hasher.finish()
    }

    fn linear_design_matrix(
        &self,
        clamp_scale: f64,
        use_workspace_cache: bool,
    ) -> Arc<DMatrix<f64>> {
        let signature = self.workspace_signature(clamp_scale);

        if use_workspace_cache {
            if let Ok(cache) = autobk_workspace_cache().lock() {
                if let Some(entry) = cache.as_ref() {
                    if entry.signature == signature {
                        return entry.design_matrix.clone();
                    }
                }
            }
        }

        let matrix = Arc::new(self.residual_jacobian_with_scale(&self.coefs, Some(clamp_scale)));
        if use_workspace_cache {
            if let Ok(mut cache) = autobk_workspace_cache().lock() {
                *cache = Some(AutobkLinearWorkspace {
                    signature,
                    design_matrix: matrix.clone(),
                });
            }
        }

        matrix
    }

    fn solve_linear_direct_for_scale(
        &self,
        clamp_scale: f64,
        regularization: f64,
        condition_limit: f64,
        residual_ratio_limit: f64,
        use_workspace_cache: bool,
    ) -> Result<DVector<f64>, BackgroundError> {
        if self.coefs.is_empty() {
            return Err(BackgroundError::DirectSolverFailed {
                reason: "empty coefficient vector".to_string(),
            });
        }

        let base_residual =
            self.residual_vec_with_scale(&DVector::zeros(self.coefs.len()), Some(clamp_scale));
        if base_residual.iter().any(|v| !v.is_finite()) {
            return Err(BackgroundError::DirectSolverFailed {
                reason: "non-finite base residual".to_string(),
            });
        }

        let design = self.linear_design_matrix(clamp_scale, use_workspace_cache);
        if design.iter().any(|v| !v.is_finite()) {
            return Err(BackgroundError::DirectSolverFailed {
                reason: "non-finite design matrix".to_string(),
            });
        }

        let mut regularization_candidates = vec![regularization.max(0.0)];
        for candidate in [1.0e-8, 1.0e-6, 1.0e-4, 1.0e-2, 1.0] {
            if candidate > regularization.max(0.0) {
                regularization_candidates.push(candidate);
            }
        }

        let mut last_error = None;

        for reg in regularization_candidates {
            let n_rows = design.nrows();
            let n_cols = design.ncols();
            let reg_sqrt = reg.sqrt();

            let mut augmented_design = DMatrix::zeros(n_rows + n_cols, n_cols);
            for i in 0..n_rows {
                for j in 0..n_cols {
                    augmented_design[(i, j)] = design[(i, j)];
                }
            }
            for i in 0..n_cols {
                augmented_design[(n_rows + i, i)] = reg_sqrt;
            }

            let mut augmented_rhs = DVector::zeros(n_rows + n_cols);
            for i in 0..n_rows {
                augmented_rhs[i] = -base_residual[i];
            }

            let mut column_scales = DVector::from_element(n_cols, 1.0);
            let mut scaled_design = augmented_design;
            for j in 0..n_cols {
                let norm = scaled_design.column(j).norm();
                let scale = if norm.is_finite() && norm > f64::EPSILON {
                    norm
                } else {
                    1.0
                };
                column_scales[j] = scale;
                for i in 0..scaled_design.nrows() {
                    scaled_design[(i, j)] /= scale;
                }
            }

            let svd = scaled_design.svd(true, true);
            let mut sigma_max = 0.0_f64;
            let mut sigma_min = f64::INFINITY;
            for sigma in svd.singular_values.iter() {
                let value = sigma.abs();
                sigma_max = sigma_max.max(value);
                if value > f64::EPSILON {
                    sigma_min = sigma_min.min(value);
                }
            }

            let condition_proxy = if sigma_min.is_finite() && sigma_min > f64::EPSILON {
                sigma_max / sigma_min
            } else {
                f64::INFINITY
            };

            if !condition_proxy.is_finite() || condition_proxy > condition_limit {
                last_error = Some(BackgroundError::DirectSolverIllConditioned {
                    condition_proxy,
                    limit: condition_limit,
                });
                continue;
            }

            let solved_scaled = match svd.solve(&augmented_rhs, f64::EPSILON.sqrt()) {
                Ok(solution) => solution,
                Err(reason) => {
                    last_error = Some(BackgroundError::DirectSolverFailed {
                        reason: format!("svd solve failed at regularization {}: {}", reg, reason),
                    });
                    continue;
                }
            };

            let mut coefs = solved_scaled;
            for j in 0..coefs.len() {
                coefs[j] /= column_scales[j];
            }

            if coefs.iter().any(|v| !v.is_finite()) {
                last_error = Some(BackgroundError::DirectSolverFailed {
                    reason: "non-finite coefficient solution".to_string(),
                });
                continue;
            }

            let solved_residual = &base_residual + design.as_ref() * &coefs;
            let base_norm = base_residual.norm();
            let solved_norm = solved_residual.norm();
            if !base_norm.is_finite() || !solved_norm.is_finite() {
                last_error = Some(BackgroundError::DirectSolverFailed {
                    reason: "non-finite residual norms".to_string(),
                });
                continue;
            }

            if base_norm > f64::EPSILON {
                let ratio = solved_norm / base_norm;
                if !ratio.is_finite() || ratio > residual_ratio_limit {
                    last_error = Some(BackgroundError::DirectSolverFailed {
                        reason: format!(
                            "residual quality ratio {} exceeded limit {} at regularization {}",
                            ratio, residual_ratio_limit, reg
                        ),
                    });
                    continue;
                }
            }

            return Ok(coefs);
        }

        Err(
            last_error.unwrap_or_else(|| BackgroundError::DirectSolverFailed {
                reason: "direct solve failed for all regularization attempts".to_string(),
            }),
        )
    }

    pub fn solve_linear_direct(
        &self,
        clamp_policy: AUTOBKClampScalePolicy,
        regularization: f64,
        condition_limit: f64,
        residual_ratio_limit: f64,
        use_workspace_cache: bool,
    ) -> Result<DVector<f64>, BackgroundError> {
        let first_scale = if self.nclamp == 0 {
            1.0
        } else {
            self.clamp_scale(&self.coefs)
        };

        let mut coefs = self.solve_linear_direct_for_scale(
            first_scale,
            regularization,
            condition_limit,
            residual_ratio_limit,
            use_workspace_cache,
        )?;

        if self.nclamp != 0 && clamp_policy == AUTOBKClampScalePolicy::TwoPass {
            let second_scale = self.clamp_scale(&coefs);
            if second_scale.is_finite() && (second_scale - first_scale).abs() > 1.0e-12 {
                coefs = self.solve_linear_direct_for_scale(
                    second_scale,
                    regularization,
                    condition_limit,
                    residual_ratio_limit,
                    use_workspace_cache,
                )?;
            }
        }

        Ok(coefs)
    }
}

/// Implementation of LeastSquaresProblem trait for AUTOBK algorithm
impl LeastSquaresProblem<f64, Dyn, Dyn> for AUTOBKSpline {
    type ParameterStorage = Owned<f64, Dyn>;
    type ResidualStorage = Owned<f64, Dyn>;
    type JacobianStorage = Owned<f64, Dyn, Dyn>;

    fn set_params(&mut self, coefs: &DVector<f64>) {
        self.coefs.copy_from(coefs);
    }

    fn params(&self) -> DVector<f64> {
        self.coefs.clone()
    }

    fn residuals(&self) -> Option<DVector<f64>> {
        Some(self.residual_vec(&self.coefs))
    }

    /// Jacobian matrix for the Levenberg-Marquardt optimization
    /// Jacobian matrix is calculated by numerical differentiation using foward difference
    fn jacobian(&self) -> Option<DMatrix<f64>> {
        // let residual_vec = |coefs: &DVector<f64>| AUTOBKSpline::residual_vec(&self, &coefs);
        // Some(self.coefs.jacobian(&residual_vec))

        // let start = Instant::now();

        // let jac1 = self.coefs.jacobian(&residual_vec);
        // let duration = start.elapsed();

        // println!("jac1: {}", duration.as_secs_f64());

        // let start = Instant::now();
        // let jac2 = self.residual_jacobian(&self.coefs);

        // let duration = start.elapsed();
        // println!("jac2: {}", duration.as_secs_f64());

        // println!("jac1: {:?}", jac1.shape());
        // println!("jac2: {:?}", jac2.shape());

        // jac1.iter().zip(jac2.iter()).for_each(|(x, y)| {
        //     println!("x: {}, y: {}", x, y);
        //     assert_abs_diff_eq!(x, y, epsilon = 1.0e-1);
        // });

        // Some(jac2)
        Some(self.residual_jacobian(&self.coefs))
    }
}

fn dvector_arange(start: f64, stop: f64, step: f64) -> DVector<f64> {
    if !step.is_finite() || step <= 0.0 {
        return DVector::zeros(0);
    }

    let mut values = Vec::new();
    let mut value = start;
    while value < stop {
        values.push(value);
        value += step;
    }
    DVector::from_vec(values)
}

/// TODO: Implement ILPBkg
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ILPBkg {}

/// TODO: Implement ILPBkg
impl ILPBkg {
    pub fn new() -> ILPBkg {
        ILPBkg::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xafs::io;
    use crate::xafs::normalization::PrePostEdge;
    use data_reader::reader::{load_txt_f64, Delimiter, ReaderParams};

    use crate::xafs::tests::PARAM_LOADTXT;
    use crate::xafs::tests::TEST_TOL;
    use crate::xafs::tests::TOP_DIR;
    use approx::assert_abs_diff_eq;

    const CHI_MSE_TOL: f64 = 1.0e-4;

    #[test]
    fn test_autobk() -> Result<(), Box<dyn Error>> {
        let acceptable_e0_diff = 1.5;

        let path = String::from(TOP_DIR) + "/tests/testfiles/Ru_QAS.dat";
        let mut xafs_test_group = io::load_spectrum_QAS_trans(&path).unwrap();

        // let mut pre_post_edge = PrePostEdge::new();
        // let _ = pre_post_edge.fill_parameter(
        //     &xafs_test_group.energy.clone().unwrap(),
        //     &xafs_test_group.mu.clone().unwrap(),
        // );

        xafs_test_group
            .set_normalization_method(Some(normalization::NormalizationMethod::PrePostEdge(
                PrePostEdge::new(),
            )))?
            .normalize()?;

        let mut autobk = AUTOBK::new();

        autobk.calc_background(
            &xafs_test_group.energy.clone().unwrap(),
            &xafs_test_group.mu.clone().unwrap(),
            &mut xafs_test_group.normalization,
        )?;

        // Test for chi with larch
        // The chi is not exactly the same as the one calculated by larch, but it is comparable in k**kweight*chi*ftwin
        // The MSE is below 1.0e-4

        let larch_k_path = String::from(TOP_DIR) + "/tests/testfiles/Ru_QAS_autobk_k_larch.txt";
        let larch_k = load_txt_f64(&larch_k_path, &PARAM_LOADTXT).unwrap();

        let k_expected = larch_k.get_col(0);
        let chi_expected = larch_k.get_col(1);

        let k = autobk.get_k().unwrap();
        let chi = autobk.get_chi_kweighted().unwrap();
        let ftwin = autobk.get_ftwin().unwrap();
        let kweight = autobk.get_kweight().unwrap();

        let chi_weighted = chi.component_mul(&ftwin);

        let chi_k2_weighted_expected = chi_expected
            .iter()
            .zip(k_expected.iter())
            .zip(ftwin.clone().iter())
            .map(|((x, y), z)| x * y.powi(kweight.clone()) * z)
            .collect::<Vec<f64>>();

        let mse = chi_weighted
            .iter()
            .zip(chi_k2_weighted_expected.iter())
            .map(|(x, y)| (x - y).powi(2))
            .sum::<f64>()
            / chi_weighted.len() as f64;

        assert!(mse < CHI_MSE_TOL);
        Ok(())
    }

    #[test]
    fn test_autobk_knot_domain_bounds_and_ordering() {
        let spl_k = DVector::from_iterator(9, (0..9).map(|i| 1.5 + i as f64 * (11.0 / 8.0)));
        let order = 3usize;
        let knots = AUTOBK::build_knot_domain(&spl_k, order).unwrap();

        // Domain extension points are lower than qmin and upper than qmax.
        let qmin = spl_k[0];
        let qmax = spl_k[spl_k.len() - 1];
        assert!(knots[0] < qmin);
        assert!(knots[order - 1] < qmin);
        assert!(knots[order] >= qmin);
        assert!(*knots.last().unwrap() > qmax);

        // Knot vector must remain non-decreasing for spline construction.
        for i in 1..knots.len() {
            assert!(knots[i] >= knots[i - 1]);
        }

        // Interior knots stay inside [qmin, qmax].
        for knot in knots.iter().skip(order).take(spl_k.len() - order) {
            assert!(*knot >= qmin);
            assert!(*knot <= qmax);
        }
    }

    #[test]
    fn test_autobk_knot_domain_rejects_invalid_range() {
        let spl_k = DVector::from_vec(vec![2.0, 2.0, 2.0, 2.0, 2.0]);
        let err = AUTOBK::build_knot_domain(&spl_k, 3).unwrap_err();
        assert!(matches!(err, BackgroundError::SplineKnotsFailed { .. }));
    }

    #[test]
    fn test_autobk_direct_solver_parity_with_legacy_lm() -> Result<(), Box<dyn Error>> {
        let path = String::from(TOP_DIR) + "/tests/testfiles/Ru_QAS.dat";
        let mut spectrum = io::load_spectrum_QAS_trans(&path)?;

        spectrum
            .set_normalization_method(Some(normalization::NormalizationMethod::PrePostEdge(
                PrePostEdge::new(),
            )))?
            .normalize()?;

        let energy = spectrum.energy.clone().unwrap();
        let mu = spectrum.mu.clone().unwrap();

        let mut legacy_norm = spectrum.normalization.clone();
        let mut direct_norm = spectrum.normalization.clone();

        let mut legacy = AUTOBK::new();
        legacy.solver = Some(AUTOBKSolver::LegacyLm);
        legacy.calc_background(&energy, &mu, &mut legacy_norm)?;

        let mut direct = AUTOBK::new();
        direct.solver = Some(AUTOBKSolver::LinearDirect);
        direct.clamp_scale_policy = Some(AUTOBKClampScalePolicy::TwoPass);
        direct.calc_background(&energy, &mu, &mut direct_norm)?;

        let legacy_chi = legacy.get_chi().unwrap();
        let direct_chi = direct.get_chi().unwrap();
        assert_eq!(legacy_chi.len(), direct_chi.len());

        let mse = legacy_chi
            .iter()
            .zip(direct_chi.iter())
            .map(|(x, y)| (x - y).powi(2))
            .sum::<f64>()
            / legacy_chi.len() as f64;

        assert!(mse < CHI_MSE_TOL);
        Ok(())
    }

    #[test]
    fn test_autobk_direct_solver_fallback_to_lm() -> Result<(), Box<dyn Error>> {
        let path = String::from(TOP_DIR) + "/tests/testfiles/Ru_QAS.dat";
        let mut spectrum = io::load_spectrum_QAS_trans(&path)?;
        spectrum
            .set_normalization_method(Some(normalization::NormalizationMethod::PrePostEdge(
                PrePostEdge::new(),
            )))?
            .normalize()?;

        let energy = spectrum.energy.clone().unwrap();
        let mu = spectrum.mu.clone().unwrap();

        let mut normalization = spectrum.normalization.clone();

        let mut autobk = AUTOBK::new();
        autobk.solver = Some(AUTOBKSolver::LinearDirect);
        autobk.linear_condition_limit = Some(1.0);
        autobk.linear_fallback_to_lm = Some(true);

        let result = autobk.calc_background(&energy, &mu, &mut normalization)?;
        assert!(result.get_chi().is_some());
        Ok(())
    }

    #[test]
    fn test_autobk_direct_solver_default_succeeds_without_fallback() -> Result<(), Box<dyn Error>> {
        let path = String::from(TOP_DIR) + "/tests/testfiles/Ru_QAS.dat";
        let mut spectrum = io::load_spectrum_QAS_trans(&path)?;
        spectrum
            .set_normalization_method(Some(normalization::NormalizationMethod::PrePostEdge(
                PrePostEdge::new(),
            )))?
            .normalize()?;

        let energy = spectrum.energy.clone().unwrap();
        let mu = spectrum.mu.clone().unwrap();
        let mut normalization = spectrum.normalization.clone();

        let mut autobk = AUTOBK::new();
        autobk.solver = Some(AUTOBKSolver::LinearDirect);
        autobk.linear_fallback_to_lm = Some(false);

        let result = autobk.calc_background(&energy, &mu, &mut normalization)?;
        assert!(result.get_chi().is_some());
        Ok(())
    }

    #[test]
    fn test_autobk_direct_solver_failure_without_fallback() -> Result<(), Box<dyn Error>> {
        let path = String::from(TOP_DIR) + "/tests/testfiles/Ru_QAS.dat";
        let mut spectrum = io::load_spectrum_QAS_trans(&path)?;
        spectrum
            .set_normalization_method(Some(normalization::NormalizationMethod::PrePostEdge(
                PrePostEdge::new(),
            )))?
            .normalize()?;

        let energy = spectrum.energy.clone().unwrap();
        let mu = spectrum.mu.clone().unwrap();
        let mut normalization = spectrum.normalization.clone();

        let mut autobk = AUTOBK::new();
        autobk.solver = Some(AUTOBKSolver::LinearDirect);
        autobk.linear_condition_limit = Some(1.0);
        autobk.linear_fallback_to_lm = Some(false);

        let err = autobk
            .calc_background(&energy, &mu, &mut normalization)
            .unwrap_err();
        assert!(matches!(
            err,
            BackgroundError::DirectSolverIllConditioned { .. }
                | BackgroundError::DirectSolverFailed { .. }
        ));
        Ok(())
    }

    #[test]
    fn test_autobk_direct_solver_deterministic_two_pass() -> Result<(), Box<dyn Error>> {
        let path = String::from(TOP_DIR) + "/tests/testfiles/Ru_QAS.dat";
        let mut spectrum = io::load_spectrum_QAS_trans(&path)?;
        spectrum
            .set_normalization_method(Some(normalization::NormalizationMethod::PrePostEdge(
                PrePostEdge::new(),
            )))?
            .normalize()?;

        let energy = spectrum.energy.clone().unwrap();
        let mu = spectrum.mu.clone().unwrap();

        let mut norm_a = spectrum.normalization.clone();
        let mut norm_b = spectrum.normalization.clone();

        let mut autobk_a = AUTOBK::new();
        autobk_a.solver = Some(AUTOBKSolver::LinearDirect);
        autobk_a.clamp_scale_policy = Some(AUTOBKClampScalePolicy::TwoPass);
        autobk_a.calc_background(&energy, &mu, &mut norm_a)?;

        let mut autobk_b = AUTOBK::new();
        autobk_b.solver = Some(AUTOBKSolver::LinearDirect);
        autobk_b.clamp_scale_policy = Some(AUTOBKClampScalePolicy::TwoPass);
        autobk_b.calc_background(&energy, &mu, &mut norm_b)?;

        let chi_a = autobk_a.get_chi().unwrap();
        let chi_b = autobk_b.get_chi().unwrap();
        assert_eq!(chi_a.len(), chi_b.len());

        chi_a.iter().zip(chi_b.iter()).for_each(|(a, b)| {
            assert_abs_diff_eq!(a, b, epsilon = 1.0e-12);
        });
        Ok(())
    }
}
