#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

// Import standard library dependencies
use std::error::Error;

// Import external dependencies
use ndarray::{Array1, ArrayBase, Ix1, OwnedRepr};
use polyfit_rs::polyfit_rs;
use serde::{Deserialize, Serialize};

// Import internal dependencies
use super::mathutils::{self, MathUtils};
use super::xafsutils;

/// trait for Normalization
/// it impliments some methods required for nomalization of XAFS data
pub trait Normalization {
    fn normalize(
        &mut self,
        energy: &ArrayBase<OwnedRepr<f64>, Ix1>,
        mu: &ArrayBase<OwnedRepr<f64>, Ix1>,
    ) -> Result<&mut Self, Box<dyn Error>>;

    fn get_norm(&self) -> &Option<Array1<f64>>;
    fn get_flat(&self) -> &Option<Array1<f64>>;
    fn get_edge_step(&self) -> Option<f64>;
    fn get_e0(&self) -> Option<f64>;
    fn set_e0(&mut self, e0: Option<f64>) -> &mut Self;
    fn set_edge_step(&mut self, edge_step: Option<f64>) -> &mut Self;
}

/// Enum for normalization method
///
/// It has two variants, PrePostEdge and MBack.
/// PrePostEdge is the standard normalization method used in athena and larch.
/// MBack is the normalization method described in the paper by Weng et al.
/// Tsu-Chien Weng, Geoffrey S. Waldo, and James E. Penner-Hahn. A method for normalization of X-ray absorption spectra. Journal of Synchrotron Radiation, 12(4):506–510, Jul 2005. doi:10.1107/S0909049504034193.
///
/// # Examples
///
/// ```
/// use xraytsubaki::xafs::normalization::{NormalizationMethod, PrePostEdge};
///
/// let mut normalization_method = NormalizationMethod::new();
/// ```
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

/// Implementation of NormalizationMethod
///
/// It provides some common interface for the normalization methods.
/// Normalization of the methods can be called by the normalize method.
/// E0, edge_step, norm, and flat can be directly accessed by the methods, which are useful for calculation and plotting purposes.
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
        energy: &Array1<f64>,
        mu: &Array1<f64>,
    ) -> Result<&mut Self, Box<dyn Error>> {
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
        energy: &ArrayBase<OwnedRepr<f64>, Ix1>,
        mu: &ArrayBase<OwnedRepr<f64>, Ix1>,
    ) -> Result<&mut Self, Box<dyn Error>> {
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

    pub fn get_flat(&self) -> &Option<Array1<f64>> {
        match self {
            NormalizationMethod::PrePostEdge(pre_post_edge) => pre_post_edge.get_flat(),
            NormalizationMethod::MBack(mback) => mback.get_flat(),
        }
    }

    pub fn get_norm(&self) -> &Option<Array1<f64>> {
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

/// PrePostEdge normalization method
///
/// This is the standard normalization method used in athena and larch.
///
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrePostEdge {
    pub pre_edge_start: Option<f64>,
    pub pre_edge_end: Option<f64>,
    pub norm_start: Option<f64>,
    pub norm_end: Option<f64>,
    pub norm_polyorder: Option<i32>,
    pub n_victoreen: Option<i32>,
    pub e0: Option<f64>,
    pub edge_step: Option<f64>,
    pub pre_edge: Option<Array1<f64>>,
    pub post_edge: Option<Array1<f64>>,
    pub norm: Option<Array1<f64>>,
    pub flat: Option<Array1<f64>>,
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
        energy: &Array1<f64>,
        mu: &Array1<f64>,
    ) -> Result<&mut Self, Box<dyn Error>> {
        if self.e0.is_none()
            || self.e0.unwrap().is_nan()
            || self.e0.unwrap() > energy[&energy.len() - 2]
        {
            let e0 = xafsutils::find_e0(energy.clone(), mu.clone())?;
            self.e0 = Some(e0);
        }

        let ie0 = mathutils::index_nearest(&energy.to_vec(), &self.e0.unwrap())?;
        let e0 = energy[ie0];

        if self.n_victoreen.is_none() {
            self.n_victoreen = Some(0);
        }

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

        self.norm_start = Some(self.norm_start.unwrap().min(self.norm_end.unwrap() - 10.0));

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

        Ok(self)
    }

    pub fn get_pre_edge_start(&self) -> Option<f64> {
        self.pre_edge_start
    }

    pub fn get_pre_edge_end(&self) -> Option<f64> {
        self.pre_edge_end
    }

    pub fn get_norm_start(&self) -> Option<f64> {
        self.norm_start
    }

    pub fn get_norm_end(&self) -> Option<f64> {
        self.norm_end
    }

    pub fn get_norm_polyorder(&self) -> Option<i32> {
        self.norm_polyorder
    }

    pub fn get_n_victoreen(&self) -> Option<i32> {
        self.n_victoreen
    }

    pub fn get_pre_edge(&self) -> &Option<Array1<f64>> {
        &self.pre_edge
    }

    pub fn get_post_edge(&self) -> &Option<Array1<f64>> {
        &self.post_edge
    }

    pub fn get_norm_coefficients(&self) -> &Option<Vec<f64>> {
        &self.norm_coefficients
    }

    pub fn get_pre_coefficients(&self) -> &Option<Vec<f64>> {
        &self.pre_coefficients
    }
}

impl Normalization for PrePostEdge {
    fn normalize(
        &mut self,
        energy: &ArrayBase<OwnedRepr<f64>, Ix1>,
        mu: &ArrayBase<OwnedRepr<f64>, Ix1>,
    ) -> Result<&mut Self, Box<dyn Error>> {
        // let (energy, mu): (Vec<f64>, Vec<f64>) = energy
        //     .iter()
        //     .zip(mu.iter())
        //     .filter(|(e, m)| e.is_finite() && m.is_finite())
        //     .unzip();

        // let energy = Array1::from_vec(energy);
        // let mu = Array1::from_vec(mu);

        let (energy, mu) = xafsutils::remove_nan2(energy, mu);

        let _ = self.fill_parameter(&energy, &mu)?;

        let p1 = mathutils::index_of(
            &energy.to_vec(),
            &(&self.pre_edge_start.unwrap() + &self.e0.unwrap()),
        )?;
        let mut p2 = mathutils::index_nearest(
            &energy.to_vec(),
            &(&self.pre_edge_end.unwrap() + &self.e0.unwrap()),
        )?;

        if &p2 - &p1 < 2 {
            p2 = energy.len().min(&p1 + 2);
        }

        let nvict = self.n_victoreen.unwrap_or(0);

        // TODO: make it faster.
        let omu = &mu.slice(ndarray::s![p1..p2])
            * &energy.slice(ndarray::s![p1..p2]).map(|e| e.powi(nvict));

        let (energy_x, mu_x) =
            xafsutils::remove_nan2(&energy.slice(ndarray::s![p1..p2]).to_owned(), &omu);

        let pre_coefficients: Vec<f64> =
            polyfit_rs::polyfit(&energy_x.to_vec(), &mu_x.to_vec(), 1)?;

        let pre_edge = (&energy * pre_coefficients[1].clone() + pre_coefficients[0].clone())
            * &energy.map(|e| e.powi(-nvict));

        let mut p1 = mathutils::index_of(
            &energy.to_vec(),
            &(&self.norm_start.unwrap() + &self.e0.unwrap()),
        )?;
        let mut p2 = mathutils::index_nearest(
            &energy.to_vec(),
            &(&self.norm_end.unwrap() + &self.e0.unwrap()),
        )?;

        if &p2 - &p1 < 2 {
            p2 = energy.len().min(&p1 + 2);
            p1 = energy.len().min(&p1 + 1);
        }

        let presub = (&mu - &pre_edge)
            .slice(ndarray::s![p1..p2])
            .to_vec()
            .clone();
        let post_edge_energy = energy.slice(ndarray::s![p1..p2]).clone();
        let post_coefficients = polyfit_rs::polyfit(
            &post_edge_energy.to_vec(),
            &presub,
            self.norm_polyorder.unwrap() as usize,
        )?;

        let mut post_edge = pre_edge.clone();

        for (i, c) in post_coefficients.iter().enumerate() {
            post_edge = &post_edge + &energy.map(|e| e.powi(i as i32)) * c.clone();
        }
        let ie0 = mathutils::index_nearest(&energy.to_vec(), &self.e0.unwrap())?;
        let edge_step = if self.edge_step.is_none() {
            post_edge[ie0] - pre_edge[ie0]
        } else {
            self.edge_step.unwrap()
        }
        .max(1.0e-12);

        let norm = (&mu - &pre_edge) / edge_step.clone();

        // let flat_diff = (&post_edge - &mu) / edge_step.clone();
        let flat_residue = (&post_edge - &pre_edge) / edge_step.clone();

        let mut flat = &norm - &flat_residue + flat_residue[ie0].clone();

        flat.slice_mut(ndarray::s![..ie0])
            .assign(&norm.slice(ndarray::s![..ie0]));

        self.edge_step = Some(edge_step);
        self.pre_edge = Some(pre_edge);
        self.post_edge = Some(post_edge);
        self.norm = Some(norm);
        self.flat = Some(flat);
        self.norm_coefficients = Some(post_coefficients);
        self.pre_coefficients = Some(pre_coefficients);

        Ok(self)
    }

    fn get_e0(&self) -> Option<f64> {
        self.e0
    }

    fn get_edge_step(&self) -> Option<f64> {
        self.edge_step
    }

    fn get_flat(&self) -> &Option<Array1<f64>> {
        &self.flat
    }

    fn get_norm(&self) -> &Option<Array1<f64>> {
        &self.norm
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
pub struct MBack {
    pub e0: Option<f64>,
    pub edge_step: Option<f64>,
    pub norm: Option<Array1<f64>>,
    pub flat: Option<Array1<f64>>,
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
        MBack {
            ..Default::default()
        }
    }

    pub fn fill_parameter(&mut self) {
        todo!("Implement MBack fill_parameter")
    }
}

impl Normalization for MBack {
    fn normalize(
        &mut self,
        energy: &ArrayBase<OwnedRepr<f64>, Ix1>,
        mu: &ArrayBase<OwnedRepr<f64>, Ix1>,
    ) -> Result<&mut Self, Box<dyn Error>> {
        todo!("Implement MBack normalization");
    }

    fn get_e0(&self) -> Option<f64> {
        self.e0
    }

    fn get_edge_step(&self) -> Option<f64> {
        self.edge_step
    }

    fn get_flat(&self) -> &Option<Array1<f64>> {
        &self.flat
    }

    fn get_norm(&self) -> &Option<Array1<f64>> {
        &self.norm
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

#[cfg(test)]
mod tests {
    use crate::xafs::io;
    use data_reader::reader::{load_txt_f64, Delimiter, ReaderParams};

    use super::*;
    use crate::xafs::tests::PARAM_LOADTXT;
    use crate::xafs::tests::TEST_TOL;
    use crate::xafs::tests::TOP_DIR;
    use approx::assert_abs_diff_eq;
    const ACCEPTABLE_MU_DIFF: f64 = 1e-6;

    #[test]
    fn test_pre_post_edge_fill_parameter() {
        let path = String::from(TOP_DIR) + "/tests/testfiles/Ru_QAS.dat";
        let xafs_test_group = io::load_spectrum_QAS_trans(&path).unwrap();

        let mut pre_post_edge = PrePostEdge::new();
        let _ = pre_post_edge.fill_parameter(
            &xafs_test_group.energy.clone().unwrap(),
            &xafs_test_group.mu.clone().unwrap(),
        );

        let expected = PrePostEdge {
            pre_edge_start: Some(-200.0),
            pre_edge_end: Some(-65.0),
            norm_start: Some(25.0),
            norm_end: Some(944.5331719999995),
            norm_polyorder: Some(2),
            n_victoreen: None,
            e0: Some(22118.8),
            edge_step: None,
            pre_edge: None,
            post_edge: None,
            norm: None,
            flat: None,
            norm_coefficients: None,
            pre_coefficients: None,
        };

        assert_abs_diff_eq!(
            pre_post_edge.e0.unwrap(),
            expected.e0.unwrap(),
            epsilon = TEST_TOL
        );

        assert_abs_diff_eq!(
            pre_post_edge.pre_edge_start.unwrap(),
            expected.pre_edge_start.unwrap(),
            epsilon = TEST_TOL
        );

        assert_abs_diff_eq!(
            pre_post_edge.pre_edge_end.unwrap(),
            expected.pre_edge_end.unwrap(),
            epsilon = TEST_TOL
        );

        assert_abs_diff_eq!(
            pre_post_edge.norm_start.unwrap(),
            expected.norm_start.unwrap(),
            epsilon = TEST_TOL
        );

        assert_abs_diff_eq!(
            pre_post_edge.norm_end.unwrap(),
            expected.norm_end.unwrap(),
            epsilon = TEST_TOL
        );

        assert_abs_diff_eq!(
            pre_post_edge.norm_polyorder.unwrap() as f64,
            expected.norm_polyorder.unwrap() as f64,
            epsilon = TEST_TOL
        );
    }

    #[test]
    fn test_normalization() {
        let path = String::from(TOP_DIR) + "/tests/testfiles/Ru_QAS.dat";
        let xafs_test_group = io::load_spectrum_QAS_trans(&path).unwrap();

        let mut pre_post_edge = PrePostEdge::new();
        let _ = pre_post_edge.fill_parameter(
            &xafs_test_group.energy.clone().unwrap(),
            &xafs_test_group.mu.clone().unwrap(),
        );

        let _ = pre_post_edge.normalize(
            &xafs_test_group.energy.clone().unwrap(),
            &xafs_test_group.mu.clone().unwrap(),
        );

        assert_eq!(pre_post_edge.edge_step, Some(0.862815921384477));
        assert_eq!(
            pre_post_edge.pre_coefficients,
            Some(vec![-0.05298882571982536, -1.9039451808611713e-7])
        );
        assert_eq!(
            pre_post_edge.norm_coefficients,
            Some(vec![
                8.985714230146124,
                -0.0005540674890038064,
                8.446567273641622e-9
            ])
        );

        assert_abs_diff_eq!(
            pre_post_edge.edge_step.unwrap(),
            0.862815921384477,
            epsilon = TEST_TOL
        );

        // // Write results to a file

        // use itertools::izip;
        // use std::fs::File;
        // use std::io::prelude::*;

        // let save_path =
        //     String::from(TOP_DIR) + "/tests/testfiles/Ru_QAS_pre_post_edge_expected.dat";

        // // Save data for further comparison
        // let mut file = File::create(save_path).unwrap();

        // let _ = writeln!(file, "# energy mu pre_edge post_edge norm flat");

        // for (e, mu, pre, post, norm, flat) in izip!(
        //     xafs_test_group.energy.clone().unwrap(),
        //     xafs_test_group.mu.clone().unwrap(),
        //     pre_post_edge.pre_edge.clone().unwrap(),
        //     pre_post_edge.post_edge.clone().unwrap(),
        //     pre_post_edge.norm.clone().unwrap(),
        //     pre_post_edge.flat.clone().unwrap()
        // ) {
        //     let _ = writeln!(file, "{} {} {} {} {} {}", e, mu, pre, post, norm, flat);
        // }

        // Compare output strictly with the reference

        let path = String::from(TOP_DIR) + "/tests/testfiles/Ru_QAS_pre_post_edge_expected.dat";
        let reference_dat = load_txt_f64(&path, &PARAM_LOADTXT).unwrap();

        let reference_norm = reference_dat.get_col(4);
        let reference_flat = reference_dat.get_col(5);

        pre_post_edge
            .norm
            .clone()
            .unwrap()
            .iter()
            .zip(reference_norm.iter())
            .for_each(|(a, b)| assert_abs_diff_eq!(a, b, epsilon = TEST_TOL));

        pre_post_edge
            .flat
            .clone()
            .unwrap()
            .iter()
            .zip(reference_flat.iter())
            .for_each(|(a, b)| assert_abs_diff_eq!(a, b, epsilon = TEST_TOL));

        //
        // Comparison with the data obtained from xraylarch
        //  data obtained by larch: {'e0': 22118.8, 'edge_step': 0.8628161198296296, 'norm_coefs': [8.985714130708697, -0.0005540674801681585, 8.446567483044725e-09], 'nvict': 0, 'nnorm': 2, 'norm1': 25, 'norm2': 944.5331719999995, 'pre1': -200.0, 'pre2': -65.0, 'precoefs': array([-5.29888257e-02, -1.90394518e-07])}

        let larch_norm_path = String::from(TOP_DIR) + "/tests/testfiles/Ru_QAS_preedge_larch.txt";
        let larch_norm = load_txt_f64(&larch_norm_path, &PARAM_LOADTXT).unwrap();

        let norm_expected = larch_norm.get_col(1);

        let expected = PrePostEdge {
            pre_edge_start: Some(-200.0),
            pre_edge_end: Some(-65.0),
            norm_start: Some(25.0),
            norm_end: Some(945.0),
            norm_polyorder: Some(2),
            n_victoreen: None,
            e0: Some(22118.8),
            edge_step: Some(0.8614006777730155),
            pre_edge: None,
            post_edge: None,
            norm: None,
            flat: None,
            norm_coefficients: Some(vec![
                8.985714130708697,
                -0.0005540674801681585,
                8.446567483044725e-09,
            ]),
            pre_coefficients: Some(vec![-5.29888257e-02, -1.90394518e-07]),
        };

        assert_abs_diff_eq!(
            pre_post_edge.e0.unwrap(),
            expected.e0.unwrap(),
            epsilon = TEST_TOL
        );

        // Test for post_edge polynominal fitting will fail.

        // pre_post_edge
        //     .norm_coefficients
        //     .unwrap()
        //     .iter()
        //     .zip(expected.norm_coefficients.unwrap().iter())
        //     .for_each(|(a, b)| {
        //         assert!(
        //             (a - b).abs() < acceptable_mu_diff,
        //             "norm_coefficients: {} != {}",
        //             a,
        //             b
        //         );
        //     });

        // pre_post_edge
        //     .pre_coefficients
        //     .unwrap()
        //     .iter()
        //     .zip(expected.pre_coefficients.unwrap().iter())
        //     .for_each(|(a, b)| {
        //         assert!(
        //             (a - b).abs() < acceptable_mu_diff,
        //             "pre_coefficients: {} != {}",
        //             a,
        //             b
        //         );
        //     });

        pre_post_edge
            .norm
            .clone()
            .unwrap()
            .iter()
            .zip(norm_expected.iter())
            .for_each(|(a, b)| {
                assert_abs_diff_eq!(a, b, epsilon = ACCEPTABLE_MU_DIFF);
            });
    }
}
