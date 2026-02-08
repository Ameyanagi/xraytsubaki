#![allow(dead_code)]
#![allow(unused_imports)]

use nalgebra::DVector;
use serde::{Deserialize, Serialize};

use super::errors::{DataError, NormalizationError};
use super::mathutils;
use super::xafsutils;

pub trait Normalization {
    fn normalize(
        &mut self,
        energy: &DVector<f64>,
        mu: &DVector<f64>,
    ) -> Result<&mut Self, NormalizationError>;

    fn get_norm(&self) -> Option<&DVector<f64>>;
    fn get_flat(&self) -> Option<&DVector<f64>>;
    fn get_edge_step(&self) -> Option<f64>;
    fn get_e0(&self) -> Option<f64>;
    fn set_e0(&mut self, e0: Option<f64>) -> &mut Self;
    fn set_edge_step(&mut self, edge_step: Option<f64>) -> &mut Self;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NormalizationMethod {
    PrePostEdge(PrePostEdge),
    MBack(MBack),
}

impl Default for NormalizationMethod {
    fn default() -> Self {
        NormalizationMethod::PrePostEdge(PrePostEdge::default())
    }
}

impl NormalizationMethod {
    pub fn new() -> NormalizationMethod {
        NormalizationMethod::PrePostEdge(PrePostEdge::new())
    }

    pub fn new_prepostedge() -> NormalizationMethod {
        NormalizationMethod::PrePostEdge(PrePostEdge::new())
    }

    pub fn new_mback() -> NormalizationMethod {
        NormalizationMethod::MBack(MBack::new())
    }

    pub fn fill_parameter(
        &mut self,
        energy: &DVector<f64>,
        mu: &DVector<f64>,
    ) -> Result<&mut Self, NormalizationError> {
        match self {
            NormalizationMethod::PrePostEdge(pre_post_edge) => {
                pre_post_edge.fill_parameter(energy, mu)?;
            }
            NormalizationMethod::MBack(mback) => {
                mback.fill_parameter();
            }
        }

        Ok(self)
    }

    pub fn normalize(
        &mut self,
        energy: &DVector<f64>,
        mu: &DVector<f64>,
    ) -> Result<&mut Self, NormalizationError> {
        match self {
            NormalizationMethod::PrePostEdge(pre_post_edge) => {
                pre_post_edge.normalize(energy, mu)?;
            }
            NormalizationMethod::MBack(mback) => {
                mback.normalize(energy, mu)?;
            }
        }

        Ok(self)
    }

    pub fn get_e0(&self) -> Option<f64> {
        match self {
            NormalizationMethod::PrePostEdge(pre_post_edge) => pre_post_edge.get_e0(),
            NormalizationMethod::MBack(mback) => mback.get_e0(),
        }
    }

    pub fn get_edge_step(&self) -> Option<f64> {
        match self {
            NormalizationMethod::PrePostEdge(pre_post_edge) => pre_post_edge.get_edge_step(),
            NormalizationMethod::MBack(mback) => mback.get_edge_step(),
        }
    }

    pub fn get_flat(&self) -> Option<&DVector<f64>> {
        match self {
            NormalizationMethod::PrePostEdge(pre_post_edge) => pre_post_edge.get_flat(),
            NormalizationMethod::MBack(mback) => mback.get_flat(),
        }
    }

    pub fn get_norm(&self) -> Option<&DVector<f64>> {
        match self {
            NormalizationMethod::PrePostEdge(pre_post_edge) => pre_post_edge.get_norm(),
            NormalizationMethod::MBack(mback) => mback.get_norm(),
        }
    }

    pub fn set_e0(&mut self, e0: Option<f64>) -> &mut Self {
        match self {
            NormalizationMethod::PrePostEdge(pre_post_edge) => {
                pre_post_edge.set_e0(e0);
            }
            NormalizationMethod::MBack(mback) => {
                mback.set_e0(e0);
            }
        }

        self
    }

    pub fn set_edge_step(&mut self, edge_step: Option<f64>) -> &mut Self {
        match self {
            NormalizationMethod::PrePostEdge(pre_post_edge) => {
                pre_post_edge.set_edge_step(edge_step);
            }
            NormalizationMethod::MBack(mback) => {
                mback.set_edge_step(edge_step);
            }
        }

        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PrePostEdge {
    pub pre_edge_start: Option<f64>,
    pub pre_edge_end: Option<f64>,
    pub norm_start: Option<f64>,
    pub norm_end: Option<f64>,
    pub norm_polyorder: Option<i32>,
    pub n_victoreen: Option<i32>,
    pub e0: Option<f64>,
    pub edge_step: Option<f64>,
    pub pre_edge: Option<DVector<f64>>,
    pub post_edge: Option<DVector<f64>>,
    pub norm: Option<DVector<f64>>,
    pub flat: Option<DVector<f64>>,
    pub pre_coefficients: Option<Vec<f64>>,
    pub norm_coefficients: Option<Vec<f64>>,
}

impl Default for PrePostEdge {
    fn default() -> Self {
        PrePostEdge {
            pre_edge_start: Some(-200.0),
            pre_edge_end: Some(-30.0),
            norm_start: Some(150.0),
            norm_end: Some(2000.0),
            norm_polyorder: Some(2),
            n_victoreen: Some(0),
            e0: None,
            edge_step: None,
            pre_edge: None,
            post_edge: None,
            norm: None,
            flat: None,
            norm_coefficients: None,
            pre_coefficients: None,
        }
    }
}

impl PrePostEdge {
    pub fn new() -> PrePostEdge {
        PrePostEdge {
            pre_edge_start: None,
            pre_edge_end: None,
            norm_start: None,
            norm_end: None,
            norm_polyorder: None,
            n_victoreen: None,
            e0: None,
            edge_step: None,
            pre_edge: None,
            post_edge: None,
            norm: None,
            flat: None,
            norm_coefficients: None,
            pre_coefficients: None,
        }
    }

    pub fn fill_parameter(
        &mut self,
        energy: &DVector<f64>,
        mu: &DVector<f64>,
    ) -> Result<&mut Self, NormalizationError> {
        if energy.len() != mu.len() {
            return Err(DataError::LengthMismatch {
                energy_len: energy.len(),
                mu_len: mu.len(),
            }
            .into());
        }

        let mut e0 = self.e0.unwrap_or(f64::NAN);
        if !e0.is_finite() || e0 > energy[energy.len() - 1] || e0 < energy[0] {
            e0 = xafsutils::find_e0(energy, mu)?;
        }

        let ie0 = mathutils::index_nearest_sorted(energy.as_slice(), &e0)?;
        self.e0 = Some(energy[ie0]);

        if self.edge_step.is_none() {
            let step = (mu[mu.len() - 1] - mu[0]).abs().max(1e-12);
            self.edge_step = Some(step);
        }

        Ok(self)
    }

    pub fn get_pre_edge(&self) -> Option<&DVector<f64>> {
        self.pre_edge.as_ref()
    }

    pub fn get_post_edge(&self) -> Option<&DVector<f64>> {
        self.post_edge.as_ref()
    }
}

impl Normalization for PrePostEdge {
    fn normalize(
        &mut self,
        energy: &DVector<f64>,
        mu: &DVector<f64>,
    ) -> Result<&mut Self, NormalizationError> {
        self.fill_parameter(energy, mu)?;

        let edge_step = self.edge_step.unwrap_or(1.0).max(1e-12);
        let pre_level = mu[0];
        let post_level = mu[mu.len() - 1];

        let pre_edge = DVector::from_element(mu.len(), pre_level);
        let post_edge = DVector::from_element(mu.len(), post_level);
        let norm = (mu - &pre_edge) / edge_step;

        self.pre_edge = Some(pre_edge);
        self.post_edge = Some(post_edge);
        self.flat = Some(norm.clone());
        self.norm = Some(norm);
        self.pre_coefficients = Some(vec![pre_level]);
        self.norm_coefficients = Some(vec![post_level]);

        Ok(self)
    }

    fn get_flat(&self) -> Option<&DVector<f64>> {
        self.flat.as_ref()
    }

    fn get_norm(&self) -> Option<&DVector<f64>> {
        self.norm.as_ref()
    }

    fn get_edge_step(&self) -> Option<f64> {
        self.edge_step
    }

    fn get_e0(&self) -> Option<f64> {
        self.e0
    }

    fn set_e0(&mut self, e0: Option<f64>) -> &mut Self {
        self.e0 = e0;
        self
    }

    fn set_edge_step(&mut self, edge_step: Option<f64>) -> &mut Self {
        self.edge_step = edge_step;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MBack {
    pub e0: Option<f64>,
    pub edge_step: Option<f64>,
    pub norm: Option<DVector<f64>>,
    pub flat: Option<DVector<f64>>,
}

impl Default for MBack {
    fn default() -> Self {
        MBack {
            e0: None,
            edge_step: None,
            norm: None,
            flat: None,
        }
    }
}

impl MBack {
    pub fn new() -> MBack {
        MBack::default()
    }

    pub fn fill_parameter(&mut self) {}
}

impl Normalization for MBack {
    fn normalize(
        &mut self,
        _energy: &DVector<f64>,
        _mu: &DVector<f64>,
    ) -> Result<&mut Self, NormalizationError> {
        Err(NormalizationError::NotImplemented {
            method: "MBack normalization".to_string(),
        })
    }

    fn get_flat(&self) -> Option<&DVector<f64>> {
        self.flat.as_ref()
    }

    fn get_norm(&self) -> Option<&DVector<f64>> {
        self.norm.as_ref()
    }

    fn get_edge_step(&self) -> Option<f64> {
        self.edge_step
    }

    fn get_e0(&self) -> Option<f64> {
        self.e0
    }

    fn set_e0(&mut self, e0: Option<f64>) -> &mut Self {
        self.e0 = e0;
        self
    }

    fn set_edge_step(&mut self, edge_step: Option<f64>) -> &mut Self {
        self.edge_step = edge_step;
        self
    }
}
