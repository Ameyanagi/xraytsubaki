#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

// Import standard library dependencies
use std::error::Error;

// Import external dependencies
use ndarray::{Array1, ArrayBase, Ix1, OwnedRepr};

// Import internal dependencies
use super::mathutils::{self, MathUtils};
use super::normalization::{self, Normalization};
use super::xafsutils;

#[derive(Debug, Clone, PartialEq)]
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
}

#[derive(Debug, Clone, PartialEq)]
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
    pub chi_std: Option<Array1<f64>>,
    pub k_std: Option<Array1<f64>>,
    pub k_weight: Option<i32>,
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
            k_weight: Some(1),
        }
    }
}

impl AUTOBK {
    pub fn new() -> AUTOBK {
        AUTOBK::default()
    }

    pub fn fill_parameter(&mut self) -> Result<(), Box<dyn Error>> {
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

        if self.k_weight.is_none() {
            self.k_weight = Some(1);
        }

        Ok(())
    }

    pub fn calc_background<'a>(
        &mut self,
        energy: &ArrayBase<OwnedRepr<f64>, Ix1>,
        mu: &ArrayBase<OwnedRepr<f64>, Ix1>,
        normalization_param: &mut Option<normalization::NormalizationMethod>,
    ) -> Result<(), Box<dyn Error>> {
        // Fill in default values for parameters that are not set
        self.fill_parameter()?;

        let energy = xafsutils::remove_dups(energy.clone(), None, None, None);

        // Perform normalization if necessary

        let mut normalization_method: normalization::NormalizationMethod =
            if normalization_param.is_none() {
                let mut normalization_method = normalization::PrePostEdge::new();
                let ek0 = self.ek0.clone();

                normalization_method.set_e0(ek0);
                normalization::NormalizationMethod::PrePostEdge(normalization_method)
            } else {
                normalization_param.clone().unwrap()
            };

        self.ek0 = if &self.ek0.unwrap() < &energy.min() || &self.ek0.unwrap() > &energy.max() {
            None
        } else {
            self.ek0
        };

        let e0 = normalization_method.get_e0();
        let edge_step = normalization_method.get_edge_step();
        let ek0 = self.ek0;

        if (ek0.is_none() && e0.is_none()) || edge_step.is_none() {
            normalization_method.normalize(&energy, &mu)?;
        }

        self.ek0 = if self.ek0.is_none() {
            normalization_method.get_e0().clone()
        } else {
            self.ek0
        };

        // Rbkg Algorithm
        let iek0 = mathutils::index_of(&energy.to_vec(), &self.ek0.unwrap())?;
        let mut rgrid =
            std::f64::consts::PI / (&self.kstep.unwrap() * self.nfft.unwrap().clone() as f64);

        if &self.rbkg.unwrap() < &(2.0 * &rgrid) {
            rgrid = 2.0 * rgrid;
        }

        let enpe = &energy.slice(ndarray::s![iek0..]).clone() - self.ek0.unwrap().clone();
        let kraw = &enpe.mapv(|x| x.sqrt() * xafsutils::constants::ETOK * x.abs());

        let kmax = if self.kmax.is_none() {
            kraw.max()
        } else {
            self.kmax.unwrap().min(kraw.max()).max(0.0)
        };

        let kout = self.kstep.unwrap().clone()
            * &Array1::range(0.0, 1.01 + &kmax / &self.kstep.unwrap(), 1.0);

        let iemax = &energy.len().min(
            2 + mathutils::index_of(
                &energy.to_vec(),
                &(&self.ek0.unwrap() + &kmax.powi(2) / xafsutils::constants::ETOK),
            )?,
        ) - 1;

        let chi_std = if self.chi_std.is_some() || self.k_std.is_some() {
            Some(kout.interpolate(
                &self.k_std.as_ref().unwrap().to_vec(),
                &self.chi_std.as_ref().unwrap().to_vec(),
            )?)
        } else {
            None
        };

        // let ftwin = kout
        // .mapv(|x| x.powi(self.k_weight.unwrap()) * xafsutils::window::kaiser_bessel(x, rgrid));

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ILPBkg {}

impl ILPBkg {
    pub fn new() -> ILPBkg {
        ILPBkg::default()
    }
}
