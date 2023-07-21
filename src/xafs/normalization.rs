use crate::xafs;
use ndarray::{Array1, ArrayBase, Ix1, OwnedRepr};

use std::error::Error;

use super::mathutils::{self, MathUtils};
use super::xafsutils;

trait Normalization {
    fn normalize(
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
    pub e0: Option<f64>,
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
        }
    }
}

impl PrePostEdge {
    const MAX_NORM_POLYORDER: i32 = 5;

    pub fn new() -> PrePostEdge {
        PrePostEdge {
            pre_edge_start: None,
            pre_edge_end: None,
            norm_start: None,
            norm_end: None,
            norm_polyorder: None,
            n_victoreen: None,
            e0: None,
        }
    }

    pub fn fill_parameter(
        &mut self,
        energy: &Array1<f64>,
        mu: &Array1<f64>,
    ) -> Result<(), Box<dyn Error>> {
        if self.e0.is_none()
            || self.e0.unwrap().is_nan()
            || self.e0.unwrap() > energy[&energy.len() - 2]
        {
            let e0 = xafsutils::find_e0(energy.clone(), mu.clone())?;
            self.e0 = Some(e0);
        }

        let ie0 = mathutils::index_nearest(&energy.to_vec(), &self.e0.unwrap());
        let e0 = energy[ie0];

        if self.pre_edge_start.is_none() {
            let pre_edge_start = if ie0 > 20 {
                5.0 * ((&energy[1] - &e0) / 5.0).round()
            } else {
                2.0 * ((&energy[1] - &e0) / 2.0).round()
            }
            .max(&energy.min() - &e0);

            self.pre_edge_start = Some(pre_edge_start);
        }

        if self.pre_edge_end.is_none() {
            let pre_edge_end = 5.0 * (&self.pre_edge_start.unwrap() / 15.0).round();
            self.pre_edge_end = Some(pre_edge_end);
        }

        if self.pre_edge_start.unwrap() > self.pre_edge_end.unwrap() {
            (self.pre_edge_start, self.pre_edge_end) = (self.pre_edge_end, self.pre_edge_start);
        }

        if self.norm_end.is_none() {
            let norm_end = 5.0 * ((&energy.max() - &e0) / 5.0).round();
            let norm_end = if &norm_end < &0.0 {
                &energy.max() - &e0 - norm_end
            } else {
                norm_end
            }
            .min(&energy.max() - &e0);

            self.norm_end = Some(norm_end);
        }

        if self.norm_start.is_none() {
            let norm_start = 5.0 * (self.norm_end.unwrap() / 15.0).round();
            self.norm_start = Some(norm_start.min(25.0));
        }

        if self.norm_start.unwrap() > self.norm_end.unwrap() + 5.0 {
            (self.norm_start, self.norm_end) = (self.norm_end, self.norm_start);
        }

        self.norm_start = Some(
            self.norm_start
                .unwrap()
                .min(self.pre_edge_end.unwrap() - 10.0),
        );

        if self.norm_polyorder.is_none() {
            let diff = self.norm_end.unwrap() - self.norm_start.unwrap();
            if diff < 50.0 {
                self.norm_polyorder = Some(0);
            } else if diff < 350.0 {
                self.norm_polyorder = Some(1);
            } else {
                self.norm_polyorder = Some(2);
            }
        }

        self.norm_polyorder = Some(
            self.norm_polyorder
                .unwrap()
                .min(PrePostEdge::MAX_NORM_POLYORDER)
                .max(0),
        );

        // let mut p1 = mathutils::index_of(&energy.to_vec(), &self.pre_edge_start.unwrap() + &e0)?;
        // let mut p2 = mathutils::index_nearest(&energy.to_vec(), &self.pre_edge_end.unwrap() + &e0)?;

        // if &p2 - &p1 < &2 {
        //     p2 = (&energy.len().min(&p1 + &2)).clone();
        // }
        // if &p2 - &p1 < &2 {
        //     p1 = p1 - 2;
        // }

        Ok(())
    }
}

impl Normalization for PrePostEdge {
    fn normalize(
        &self,
        energy: &ArrayBase<OwnedRepr<f64>, Ix1>,
        mu: &ArrayBase<OwnedRepr<f64>, Ix1>,
    ) -> Result<(Array1<f64>, Array1<f64>), Box<dyn Error>> {
        // let (energy, mu): (Vec<_>, Vec<_>) = energy
        //     .iter()
        //     .zip(mu.iter())
        //     .filter(|(e, m)| e.is_finite() && m.is_finite())
        //     .unzip();

        // Ok((energy, mu))

        todo!("Implement this method");
        // let energy = xafsutils::remove_dups(energy.clone(), xafsutils::TINY_ENERGY, None, None);

        // if energy.len() < 2 {
        //     return Err(Box::new(xafs::XAFSError::NotEnoughData));
        // }

        // if self.e0.is_none() || self.e0.unwrap().is_nan() || self.e0.unwap() > &energy[-2] {
        //     let e0 = xafsutils::find_e0(energy.clone(), mu.clone())?;
        //     self.e0 = Some(e0);
        // }

        // let ie0 = (&energy - self.e0.unwrap()).abs_argmin();
        // e0 = energy[ie0];

        // Ok((energy, mu.clone()))
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct MBack {}
