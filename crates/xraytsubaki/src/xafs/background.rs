#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

// Import standard library dependencies
use std::error::Error;
use std::ops::Deref;

// Import external dependencies
use levenberg_marquardt::{LeastSquaresProblem, LevenbergMarquardt};
use nalgebra::{DMatrix, DVector, Dyn, Owned};
use ndarray::{Array1, ArrayBase, Axis, Ix1, OwnedRepr};
use rusty_fitpack;
use serde::{Deserialize, Serialize};

// Import internal dependencies
use super::lmutils::LMParameters;
use super::mathutils::{self, splev_jacobian, MathUtils};
use super::normalization::{self, Normalization};
use super::nshare::{ToNalgebra, ToNdarray1};
use super::xafsutils::FTWindow;
use super::xrayfft::{FFTUtils, XFFTReverse, XFFT};
use super::{xafsutils, xrayfft};

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
        energy: &ArrayBase<OwnedRepr<f64>, Ix1>,
        mu: &ArrayBase<OwnedRepr<f64>, Ix1>,
        normalization_param: &mut Option<normalization::NormalizationMethod>,
    ) -> Result<&mut Self, Box<dyn Error>> {
        match self {
            BackgroundMethod::AUTOBK(autobk) => {
                autobk.calc_background(energy, mu, normalization_param)?;
                Ok(self)
            }
            BackgroundMethod::ILPBkg(ilpbkg) => {
                todo!("Implement ILPBkg");
                // ilpbkg.calc_background(energy, mu, normalization_param);
                Ok(self)
            }
            BackgroundMethod::None => Ok(self),
        }
    }

    pub fn get_k(&self) -> Option<ArrayBase<OwnedRepr<f64>, Ix1>> {
        match self {
            BackgroundMethod::AUTOBK(autobk) => autobk.k.clone(),
            BackgroundMethod::ILPBkg(ilpbkg) => None,
            BackgroundMethod::None => None,
        }
    }

    pub fn get_chi(&self) -> Option<ArrayBase<OwnedRepr<f64>, Ix1>> {
        match self {
            BackgroundMethod::AUTOBK(autobk) => autobk.chi.clone(),
            BackgroundMethod::ILPBkg(ilpbkg) => None,
            BackgroundMethod::None => None,
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
    pub chi_std: Option<Array1<f64>>,
    /// Optional k array for standard chi(k).
    pub k_std: Option<Array1<f64>>,
    /// k weight for FFT. Default = 1.
    pub kweight: Option<i32>,
    /// FFT window function name. Default = Hanning.
    pub window: FTWindow,
    /// FFT window window parameter. Default = 0.1.
    pub dk: Option<f64>,
    /// Background of mu(E)
    pub bkg: Option<Array1<f64>>,
    /// Edge normalized mu(E) - bkg
    pub chie: Option<Array1<f64>>,
    /// k grid
    pub k: Option<Array1<f64>>,
    /// chi(k)
    pub chi: Option<Array1<f64>>,
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

        if self.kweight.is_none() {
            self.kweight = Some(1);
        }

        if self.dk.is_none() {
            self.dk = Some(0.1);
        }

        Ok(())
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
                let ek0 = self.ek0;

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
        let mut edge_step = normalization_method.get_edge_step();
        let ek0 = self.ek0;

        if (ek0.is_none() && e0.is_none()) || edge_step.is_none() {
            normalization_method.normalize(&energy, mu)?;
            edge_step = normalization_method.get_edge_step();
        }

        self.ek0 = if self.ek0.is_none() {
            normalization_method.get_e0()
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
            * &Array1::range(0.0, (1.01 + &kmax / &self.kstep.unwrap()).floor(), 1.0);

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

        let ftwin = &kout.mapv(|x| x.powi(self.kweight.unwrap()))
            * xafsutils::ftwindow(
                &kout,
                self.kmin.clone(),
                Some(kmax.clone()),
                self.dk.clone(),
                self.dk.clone(),
                Some(self.window.clone()),
            )?;

        let mut nspl = 1
            + (2.0 * self.rbkg.unwrap() * (kmax - self.kmin.unwrap()) / std::f64::consts::PI)
                .round() as i32;
        let irbkg = (1.0
            + (nspl - 1) as f64 * std::f64::consts::PI
                / (2.0 * rgrid * (kmax - self.kmin.unwrap())))
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
        let (qmin, qmax) = (spl_k[0], spl_k[nspl as usize - 1]);

        let mut knots = Vec::with_capacity(order);
        for i in 0..order {
            knots.push(spl_k[0] - 1e-4 * (order - i) as f64);
        }

        for i in order..nspl as usize {
            knots.push((i - order) as f64 * (qmax - qmin) / (nspl - order as i32 + 1) as f64);
        }

        let qlast = knots[order - 1];

        for i in 0..order {
            knots.push(qlast + 1e-4 * (i + 1) as f64);
        }
        let coefs;

        (knots, coefs, _) = rusty_fitpack::splrep(
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

        // Calculate the mu interpolated to the k grid
        let mu_out = kout.to_vec().interpolate(
            &kraw
                .slice_axis(Axis(0), ndarray::Slice::from(0..iemax - iek0 + 1))
                .to_vec(),
            &mu.slice_axis(Axis(0), ndarray::Slice::from(iek0..iemax + 1))
                .to_vec(),
        )?;

        let spline_opt = AUTOBKSpline {
            coefs: DVector::from_vec(coefs),
            knots: DVector::from_vec(knots),
            order: order,
            irbkg: irbkg as usize,
            nfft: self.nfft.unwrap() as usize,
            kraw: kraw
                .slice_axis(Axis(0), ndarray::Slice::from(0..iemax - iek0 + 1))
                .clone()
                .to_owned()
                .into_nalgebra(),
            mu: DVector::from_vec(mu_out),
            kout: kout.clone().into_nalgebra(),
            ftwin: ftwin.into_nalgebra(),
            kweight: self.kweight.unwrap(),
            chi_std: if let Some(chi_std) = chi_std {
                Some(chi_std.into_nalgebra())
            } else {
                None
            },
            nclamp: self.nclamp.unwrap(),
            clamp_lo: self.clamp_lo.unwrap(),
            clamp_hi: self.clamp_hi.unwrap(),
            kstep: self.kstep.unwrap(),
            ..Default::default()
        };

        let (fit_result, report) = LevenbergMarquardt::new()
            .with_gtol(1.0e-6)
            .with_ftol(1.0e-6)
            .with_xtol(1.0e-6)
            .with_stepbound(1.0e-6)
            .minimize(spline_opt);

        let (bkg, chi) = spline_eval_nalgebra(
            &fit_result.kraw,
            &fit_result.mu,
            &fit_result.knots,
            &fit_result.coefs,
            fit_result.order,
            &fit_result.kout,
        );

        let bkg = bkg.into_ndarray1();
        let chi = chi.into_ndarray1();

        let mut obkg = mu.clone();
        obkg.slice_mut(ndarray::s![iek0..iek0 + bkg.len()])
            .assign(&bkg);

        self.bkg = Some(obkg.clone());
        self.chie = Some((mu - &obkg) / edge_step.unwrap());
        self.k = Some(kout);
        self.chi = Some(chi / edge_step.unwrap());

        Ok(self)
    }

    pub fn get_ek0(&self) -> Option<f64> {
        self.ek0.clone()
    }

    pub fn get_rbkg(&self) -> Option<f64> {
        self.rbkg.clone()
    }

    pub fn get_nknots(&self) -> Option<i32> {
        self.nknots.clone()
    }

    pub fn get_kmin(&self) -> Option<f64> {
        self.kmin.clone()
    }

    pub fn get_kmax(&self) -> Option<f64> {
        self.kmax.clone()
    }

    pub fn get_kstep(&self) -> Option<f64> {
        self.kstep.clone()
    }

    pub fn get_nclamp(&self) -> Option<i32> {
        self.nclamp.clone()
    }

    pub fn get_clamp_lo(&self) -> Option<i32> {
        self.clamp_lo.clone()
    }

    pub fn get_clamp_hi(&self) -> Option<i32> {
        self.clamp_hi.clone()
    }

    pub fn get_nfft(&self) -> Option<i32> {
        self.nfft.clone()
    }

    pub fn get_chi_std(&self) -> Option<Array1<f64>> {
        self.chi_std.clone()
    }

    pub fn get_k_std(&self) -> Option<Array1<f64>> {
        self.k_std.clone()
    }

    pub fn get_kweight(&self) -> Option<i32> {
        self.kweight.clone()
    }

    pub fn get_window(&self) -> FTWindow {
        self.window.clone()
    }

    pub fn get_dk(&self) -> Option<f64> {
        self.dk.clone()
    }

    pub fn get_bkg(&self) -> Option<Array1<f64>> {
        self.bkg.clone()
    }

    pub fn get_chie(&self) -> Option<Array1<f64>> {
        self.chie.clone()
    }

    pub fn get_k(&self) -> Option<Array1<f64>> {
        self.k.clone()
    }

    pub fn get_chi(&self) -> Option<Array1<f64>> {
        self.chi.clone()
    }

    pub fn get_chi_kweighted(&self) -> Option<Array1<f64>> {
        if self.kweight.is_none() || self.k.is_none() || self.chi.is_none() {
            return None;
        }

        let kweight = self.kweight.unwrap();
        let k = self.k.clone().unwrap();
        let chi = self.chi.clone().unwrap();

        if kweight == 0 {
            return Some(chi);
        } else {
            return Some(chi * &k.mapv(|x| x.powi(kweight)));
        }
    }

    pub fn get_ftwin(&self) -> Option<ArrayBase<OwnedRepr<f64>, Ix1>> {
        if self.kweight.is_none() || self.k.is_none() {
            return None;
        }

        let k = self.k.as_ref().unwrap();

        let ftwin = xafsutils::ftwindow(
            k,
            self.kmin.clone(),
            self.kmax.clone(),
            self.dk.clone(),
            self.dk.clone(),
            Some(self.window.clone()),
        )
        .unwrap();

        Some(ftwin.clone())
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
    let bkg = DVector::from_vec(rusty_fitpack::splev(
        knots.data.as_vec().clone(),
        coefs.data.as_vec().clone(),
        order,
        kraw.data.as_vec().clone(),
        3,
    ));

    // experimental
    let bkg_out = DVector::from_vec(rusty_fitpack::splev(
        knots.data.as_vec().clone(),
        coefs.data.as_vec().clone(),
        order,
        kout.data.as_vec().clone(),
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
    /// The Loss function in 1-d array for the Levenberg-Marquardt optimization
    pub fn residual_vec(&self, coefs: &DVector<f64>) -> DVector<f64> {
        let (bkg, chi) = spline_eval_nalgebra(
            &self.kraw,
            &self.mu,
            &self.knots,
            coefs,
            self.order,
            &self.kout,
        );

        let chi: DVector<f64> = if self.chi_std.is_some() {
            chi - self.chi_std.as_ref().unwrap()
        } else {
            chi
        };

        let mut out: DVector<f64> = chi
            .component_mul(&self.ftwin)
            .xftf_fast(self.nfft, self.kstep)[..self.irbkg]
            .realimg();

        if self.nclamp == 0 {
            return out;
        }

        let scale = 1.0 + 100.0 * out.dot(&out) / out.len() as f64;

        let low_clamp = self.clamp_lo as f64 * scale * chi.view((0, 0), (self.nclamp as usize, 1));

        let high_clamp = self.clamp_hi as f64
            * scale
            * chi.view(
                (chi.len() - self.nclamp as usize - 1, 0),
                (self.nclamp as usize, 1),
            );

        out.extend(low_clamp.data.as_vec().to_owned());

        out.extend(high_clamp.data.as_vec().to_owned());

        out
    }

    pub fn residual_jacobian(&self, coefs: &DVector<f64>) -> DMatrix<f64> {
        // just for calculating the scale

        let scale = if self.nclamp != 0 {
            let (_, chi) = spline_eval_nalgebra(
                &self.kraw,
                &self.mu,
                &self.knots,
                coefs,
                self.order,
                &self.kout,
            );

            let chi: DVector<f64> = if self.chi_std.is_some() {
                chi - self.chi_std.as_ref().unwrap()
            } else {
                chi
            };

            let mut out: DVector<f64> = chi
                .component_mul(&self.ftwin)
                .xftf_fast(self.nfft, self.kstep)[..self.irbkg]
                .realimg();

            let scale = 1.0 + 100.0 * out.dot(&out) / out.len() as f64;

            scale
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
        let num_cols = self.coefs.len();

        let jacobian_columns = spline_jacobian
            .column_iter()
            .map(|chi_der| {
                let mut out: DVector<f64> = chi_der
                    .component_mul(&self.ftwin)
                    .xftf_fast(self.nfft, self.kstep)[..self.irbkg]
                    .realimg();

                if self.nclamp == 0 {
                    return out;
                }

                // let scale = 1.0 + 100.0 * out.dot(&out) / out.len() as f64;

                let low_clamp =
                    self.clamp_lo as f64 * &scale * chi_der.view((0, 0), (self.nclamp as usize, 1));
                let high_clamp = self.clamp_hi as f64
                    * &scale
                    * chi_der.view(
                        (chi_der.len() - self.nclamp as usize - 1, 0),
                        (self.nclamp as usize, 1),
                    );

                out.extend(low_clamp.data.as_vec().to_owned());
                out.extend(high_clamp.data.as_vec().to_owned());
                out
            })
            .collect::<Vec<DVector<f64>>>();

        DMatrix::from_columns(&jacobian_columns)
    }
}

use approx::assert_abs_diff_eq;
use std::time::{Duration, Instant};

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

        let chi_weighted = chi * &ftwin;

        let chi_k2_weighted_expected = chi_expected
            .iter()
            .zip(k_expected.iter())
            .zip(ftwin.clone().iter())
            .map(|((x, y), z)| x * y.powi(kweight) * z)
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
}
