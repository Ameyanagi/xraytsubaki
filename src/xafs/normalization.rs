use crate::xafs;
use ndarray::{Array1, ArrayBase, Ix1, OwnedRepr};

use std::error::Error;

trait Normalization {
    fn calc_normalize(
        &self,
        energy: &ArrayBase<OwnedRepr<f64>, Ix1>,
        mu: &ArrayBase<OwnedRepr<f64>, Ix1>,
    ) -> Result<(Array1<f64>, Array1<f64>), Box<dyn Error>>;
}

#[derive(Debug, Clone, PartialEq)]
pub enum NormalizationMethod {
    PrePostEdge(PrePostEdge),
    MBack(MBack),
    None,
}

impl Default for NormalizationMethod {
    fn default() -> Self {
        NormalizationMethod::PrePostEdge(PrePostEdge::default())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PrePostEdge {
    pub pre_edge_start: Option<f64>,
    pub pre_edge_end: Option<f64>,
    pub norm_start: Option<f64>,
    pub norm_end: Option<f64>,
    pub norm_polyorder: Option<i32>,
    pub n_victoreen: Option<i32>,
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
        }
    }
}

impl PrePostEdge {
    pub fn new() -> PrePostEdge {
        PrePostEdge::default()
    }
}

impl Normalization for PrePostEdge {
    fn calc_normalize(
        &self,
        energy: &ArrayBase<OwnedRepr<f64>, Ix1>,
        mu: &ArrayBase<OwnedRepr<f64>, Ix1>,
    ) -> Result<(Array1<f64>, Array1<f64>), Box<dyn Error>> {
        todo!()
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct MBack {}
