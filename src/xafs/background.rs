#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

// Import standard library dependencies
use std::error::Error;

// Import external dependencies
use ndarray::{Array1, ArrayBase, Axis, Ix1, OwnedRepr};
use rusty_fitpack;

// Import internal dependencies
use super::mathutils::{self, MathUtils};
use super::normalization::{self, Normalization};
use super::xafsutils;

use super::xafsutils::FTWindow;

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
    pub window: FTWindow,
    pub dk: Option<f64>,
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
            window: FTWindow::Hanning,
            dk: None,
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

        if self.dk.is_none() {
            self.dk = Some(1.);
        }

        Ok(())
    }

    pub fn calc_background<'a>(
        &mut self,
        energy: &ArrayBase<OwnedRepr<f64>, Ix1>,
        mu: &ArrayBase<OwnedRepr<f64>, Ix1>,
        normalization_param: &mut Option<normalization::NormalizationMethod>,
    ) -> Result<&mut Self, Box<dyn Error>> {
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

        if let Some(ek0) = self.ek0 {
            if ek0 < energy.min() || ek0 > energy.max() {
                self.ek0 = None;
            }
        }

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
        let kraw = &enpe.mapv(|x| x.signum() * (xafsutils::constants::ETOK * x.abs()).sqrt());

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

        let ftwin = &kout.mapv(|x| x.powi(self.k_weight.unwrap()))
            * xafsutils::ftwindow(
                &kout,
                self.kmin.clone(),
                Some(kmax.clone()),
                self.dk.clone(),
                self.dk.clone(),
                Some(self.window.clone()),
            )?;

        // nspl = 1 + int(2*rbkg*(kmax-kmin)/np.pi)
        // irbkg = int(1 + (nspl-1)*np.pi/(2*rgrid*(kmax-kmin)))
        // if nknots is not None:
        //     nspl = nknots
        // nspl = max(5, min(128, nspl))
        // spl_y, spl_k  = np.ones(nspl), np.zeros(nspl)
        // for i in range(nspl):
        //     q  = kmin + i*(kmax-kmin)/(nspl - 1)
        //     ik = index_nearest(kraw, q)
        //     i1 = min(len(kraw)-1, ik + 5)
        //     i2 = max(0, ik - 5)
        //     spl_k[i] = kraw[ik]
        //     spl_y[i] = (2*mu[ik+iek0] + mu[i1+iek0] + mu[i2+iek0] ) / 4.0

        // order = 3
        // qmin, qmax  = spl_k[0], spl_k[nspl-1]
        // knots = [spl_k[0] - 1.e-4*(order-i) for i in range(order)]

        // for i in range(order, nspl):
        //     knots.append((i-order)*(qmax - qmin)/(nspl-order+1))
        // qlast = knots[-1]
        // for i in range(order+1):
        //     knots.append(qlast + 1.e-4*(i+1))

        // # coefs = [mu[index_nearest(energy, ek0 + q**2/ETOK)] for q in knots]
        // knots, coefs, order = splrep(spl_k, spl_y, k=order)
        // coefs[nspl:] = coefs[nspl-1]

        let mut nspl = 1
            + (2.0 * self.rbkg.unwrap() * (kmax - self.kmin.unwrap()) / std::f64::consts::PI)
                .round() as i32;
        let irbkg = (1.0
            + (nspl - 1) as f64 * std::f64::consts::PI
                / (2.0 * self.rbkg.unwrap() * (kmax - self.kmin.unwrap())))
        .round() as i32;

        if self.nknots.is_some() {
            nspl = self.nknots.unwrap();
        }

        nspl = nspl.min(128).max(5);

        // !todo!("Finish implementing this part of the code");
        let mut spl_y: Array1<f64> = Array1::ones(Ix1(nspl as usize));
        let mut spl_k: Array1<f64> = Array1::zeros(nspl as usize);

        let a = spl_y
            .iter_mut()
            .zip(spl_k.iter_mut())
            .enumerate()
            .for_each(|(i, (y, k))| {
                let q = &self.kmin.unwrap()
                    + i as f64 * (&kmax - &self.kmin.unwrap()) / (&nspl - 1) as f64;
                let ik = mathutils::index_nearest(&kraw.to_vec(), &q).unwrap();
                let i1 = (&ik + 5).min(kraw.len() - 1);
                let i2 = (ik.clone() as i32 - 5).max(0) as usize;
                *k = kraw[ik];
                *y = (2.0 * mu[&ik + &iek0] + mu[&i1 + &iek0] + mu[&i2 + &iek0]) / 4.0;
            });

        let order = 3;
        let (qmin, qmax) = (spl_k[0], spl_k[-1]);

        let mut knots = Vec::with_capacity(order);
        for i in 0..order {
            knots.push(spl_k[0] - 1e-4 * (order - i) as f64);
        }

        for i in order..nspl {
            knots.push((i - order) as f64 * (qmax - qmin) / (nspl - order + 1) as f64);
        }

        let qlast = knots[order - 1];

        for i in 0..order {
            knots.push(qlast + 1e-4 * (i + 1) as f64);
        }

        let mut coefs = Vec::with_capacity(nspl as usize);

        (knots, coefs, order) = rusty_fitpack::splrep(
            spl_k.to_vec(),
            spl_y.to_vec(),
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

        coefs[nspl as usize..]
            .iter_mut()
            .for_each(|x| *x = coefs[nspl as usize - 1]);

        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ILPBkg {}

impl ILPBkg {
    pub fn new() -> ILPBkg {
        ILPBkg::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xafs::io;
    use data_reader::reader::{load_txt_f64, Delimiter, ReaderParams};
    const TOP_DIR: &'static str = env!("CARGO_MANIFEST_DIR");
    const PARAM_LOADTXT: ReaderParams = ReaderParams {
        comments: Some(b'#'),
        delimiter: Delimiter::WhiteSpace,
        skip_footer: None,
        skip_header: None,
        usecols: None,
        max_rows: None,
    };
    use crate::xafs::normalization::PrePostEdge;

    #[test]
    fn test_autobk() -> Result<(), Box<dyn Error>> {
        let acceptable_e0_diff = 1.5;

        let path = String::from(TOP_DIR) + "/tests/testfiles/Ru_QAS.dat";
        let mut xafs_test_group = io::load_spectrum(&path).unwrap();

        // let mut pre_post_edge = PrePostEdge::new();
        // let _ = pre_post_edge.fill_parameter(
        //     &xafs_test_group.energy.clone().unwrap(),
        //     &xafs_test_group.mu.clone().unwrap(),
        // );

        xafs_test_group
            .set_normalization_method(Some(normalization::NormalizationMethod::PrePostEdge(
                PrePostEdge::new(),
            )))?
            .normalize();

        let mut autobk = AUTOBK::new();

        autobk.calc_background(
            &xafs_test_group.energy.clone().unwrap(),
            &xafs_test_group.mu.clone().unwrap(),
            &mut xafs_test_group.normalization,
        )?;

        println!("ek0: {:?}", autobk);

        Ok(())
    }
}
