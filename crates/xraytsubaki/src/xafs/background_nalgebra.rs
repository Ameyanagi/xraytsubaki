#![allow(dead_code)]
#![allow(unused_imports)]

use nalgebra::DVector;
use serde::{Deserialize, Serialize};

use super::errors::{BackgroundError, DataError};
use super::normalization::{self, Normalization};
use super::xafsutils::{self, FTWindow, XAFSUtils};

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
            BackgroundMethod::ILPBkg(_) => Err(BackgroundError::NotImplemented {
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AUTOBK {
    pub ek0: Option<f64>,
    pub rbkg: Option<f64>,
    pub nknots: Option<i32>,
    pub kmin: Option<f64>,
    pub kmax: Option<f64>,
    pub kstep: Option<f64>,
    pub nclamp: Option<i32>,
    pub clamp_lo: Option<i32>,
    pub clamp_hi: Option<i32>,
    pub nfft: Option<i32>,
    pub chi_std: Option<DVector<f64>>,
    pub k_std: Option<DVector<f64>>,
    pub kweight: Option<i32>,
    pub window: FTWindow,
    pub dk: Option<f64>,
    pub solver: Option<AUTOBKSolver>,
    pub clamp_scale_policy: Option<AUTOBKClampScalePolicy>,
    pub linear_regularization: Option<f64>,
    pub linear_condition_limit: Option<f64>,
    pub linear_residual_ratio_limit: Option<f64>,
    pub linear_fallback_to_lm: Option<bool>,
    pub linear_workspace_cache: Option<bool>,
    pub bkg: Option<DVector<f64>>,
    pub chie: Option<DVector<f64>>,
    pub k: Option<DVector<f64>>,
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

impl AUTOBK {
    pub fn new() -> AUTOBK {
        AUTOBK::default()
    }

    fn validate_input_vectors(energy: &DVector<f64>, mu: &DVector<f64>) -> Result<(), BackgroundError> {
        if energy.len() != mu.len() {
            return Err(DataError::LengthMismatch {
                energy_len: energy.len(),
                mu_len: mu.len(),
            }
            .into());
        }
        if energy.len() < 3 {
            return Err(DataError::InsufficientData {
                min: 3,
                actual: energy.len(),
            }
            .into());
        }
        Ok(())
    }

    pub fn calc_background(
        &mut self,
        energy: &DVector<f64>,
        mu: &DVector<f64>,
        normalization_param: &mut Option<normalization::NormalizationMethod>,
    ) -> Result<&mut Self, BackgroundError> {
        Self::validate_input_vectors(energy, mu)?;

        if normalization_param.is_none() {
            *normalization_param = Some(normalization::NormalizationMethod::new_prepostedge());
        }

        let norm = normalization_param.as_mut().ok_or_else(|| DataError::MissingData {
            field: "normalization method".to_string(),
        })?;
        norm.normalize(energy, mu)?;

        let e0 = self
            .ek0
            .or(norm.get_e0())
            .ok_or_else(|| DataError::MissingData {
                field: "e0".to_string(),
            })?;
        let edge_step = norm.get_edge_step().unwrap_or(1.0).max(1e-12);

        let mut k = DVector::zeros(energy.len());
        for (i, value) in energy.iter().enumerate() {
            let de = *value - e0;
            k[i] = if de <= 0.0 {
                0.0
            } else {
                (de * xafsutils::constants::ETOK).sqrt()
            };
        }

        let baseline = DVector::from_element(mu.len(), mu[0]);
        let chi = (mu - &baseline) / edge_step;

        self.ek0 = Some(e0);
        self.bkg = Some(baseline);
        self.chie = Some(chi.clone());
        self.k = Some(k);
        self.chi = Some(chi);

        Ok(self)
    }

    pub fn get_kweight(&self) -> Option<&i32> {
        self.kweight.as_ref()
    }

    pub fn get_k(&self) -> Option<&DVector<f64>> {
        self.k.as_ref()
    }

    pub fn get_chi(&self) -> Option<&DVector<f64>> {
        self.chi.as_ref()
    }

    pub fn get_bkg(&self) -> Option<&DVector<f64>> {
        self.bkg.as_ref()
    }

    pub fn get_chie(&self) -> Option<&DVector<f64>> {
        self.chie.as_ref()
    }

    pub fn get_chi_kweighted(&self) -> Option<DVector<f64>> {
        let kweight = self.kweight?;
        let k = self.k.as_ref()?;
        let chi = self.chi.as_ref()?;

        if kweight == 0 {
            Some(chi.clone())
        } else {
            Some(chi.component_mul(&k.map(|x| x.powi(kweight))))
        }
    }

    pub fn get_ftwin(&self) -> Option<DVector<f64>> {
        let k = self.k.as_ref()?;
        xafsutils::ftwindow(k, self.kmin, self.kmax, self.dk, self.dk, Some(self.window)).ok()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ILPBkg {}

impl Default for ILPBkg {
    fn default() -> Self {
        ILPBkg {}
    }
}

impl ILPBkg {
    pub fn new() -> ILPBkg {
        ILPBkg {}
    }
}
