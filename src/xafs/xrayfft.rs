use easyfft::prelude::{DynRealFft, DynRealIfft};
use easyfft::{dyn_size::realfft::DynRealDft, num_complex::Complex};
use nalgebra::{DVector, Owned};
use ndarray::{Array, Array1, ArrayBase, Axis, Ix, Ix1, OwnedRepr};

use crate::xafs::xafsutils::FTWindow;

use super::mathutils::MathUtils;
use super::xafsutils::ftwindow;

#[derive(Debug, Clone)]
pub struct XrayFFTF {
    pub rmax_out: Option<f64>,
    pub window: Option<FTWindow>,
    pub dk: Option<f64>,
    pub dk2: Option<f64>,
    pub kmin: Option<f64>,
    pub kmax: Option<f64>,
    pub kweight: Option<f64>,
    pub nfft: Option<usize>,
    pub kstep: Option<f64>,
    pub r: Option<ArrayBase<OwnedRepr<f64>, Ix1>>,
    pub chir: Option<DynRealDft<f64>>,
    pub chir_mag: Option<ArrayBase<OwnedRepr<f64>, Ix1>>,
    pub kwin: Option<ArrayBase<OwnedRepr<f64>, Ix1>>,
}

impl Default for XrayFFTF {
    fn default() -> Self {
        XrayFFTF {
            rmax_out: Some(10.0),
            window: Some(FTWindow::KaiserBessel),
            dk: Some(1.),
            dk2: None,
            kmin: Some(2.0),
            kmax: Some(15.0),
            kweight: Some(2.0),
            nfft: Some(2048),
            kstep: None,
            r: None,
            chir: None,
            chir_mag: None,
            kwin: None,
        }
    }
}

impl XrayFFTF {
    pub fn new() -> XrayFFTF {
        XrayFFTF::default()
    }

    pub fn fill_parameter(&mut self, k: &ArrayBase<OwnedRepr<f64>, Ix1>) -> &mut Self {
        if self.kweight.is_none() {
            self.kweight = Some(2.0);
        }

        self.kweight = Some(self.kweight.unwrap().max(0.0).floor());

        if self.kstep.is_none() {
            self.kstep = Some(k[1] - k[0]);
        }

        if self.kmin.is_none() {
            self.kmin = Some(k[0]);
        }

        if self.kmax.is_none() {
            self.kmax = Some(k[k.len() - 1]);
        }

        if self.dk.is_none() {
            self.dk = Some(1.0);
        }

        if self.dk2.is_none() {
            self.dk2 = self.dk;
        }

        if self.nfft.is_none() {
            self.nfft = Some(2048);
        }

        if self.rmax_out.is_none() {
            self.rmax_out = Some(10.0);
        }

        self
    }

    pub fn xftf_prep(
        &mut self,
        k: &ArrayBase<OwnedRepr<f64>, Ix1>,
        chi: &ArrayBase<OwnedRepr<f64>, Ix1>,
    ) -> Result<
        (
            ArrayBase<OwnedRepr<f64>, Ix1>,
            ArrayBase<OwnedRepr<f64>, Ix1>,
        ),
        Box<dyn std::error::Error>,
    > {
        self.fill_parameter(k);
        let kweight = self.kweight.unwrap() as i32;
        let mut k_max = k.max();
        let npts = (1.01 + &k_max / self.kstep.unwrap()) as usize;
        k_max = k_max.max(self.kmax.unwrap() + self.dk2.unwrap());
        let k_ = Array1::range(0.0, k_max + self.kstep.unwrap(), self.kstep.unwrap());

        let chi_ = k_.interpolate(&k.to_vec(), &chi.to_vec())?;
        let win = self
            .window
            .unwrap()
            .window(&k_, self.kmin, self.kmax, self.dk, self.dk2)?;
        let win = (&win).slice_axis(Axis(0), (0..npts).into()).to_owned();
        let chi_ = &chi_.slice_axis(Axis(0), (0..npts).into())
            * &k_
                .slice_axis(Axis(0), (0..npts).into())
                .map(|x| x.powi(kweight));

        Ok((chi_, win))
    }

    pub fn xftf(
        &mut self,
        k: &ArrayBase<OwnedRepr<f64>, Ix1>,
        chi: &ArrayBase<OwnedRepr<f64>, Ix1>,
    ) -> &mut Self {
        let (cchi, win) = self.xftf_prep(k, chi).unwrap();

        let cchi_fft = xftf_fast(&cchi, self.nfft.unwrap(), self.kstep.unwrap());

        let rstep = std::f64::consts::PI / self.kstep.unwrap() / self.nfft.unwrap() as f64;

        // The length of r is different by 1 between xraylarch and xraytsubaki. This is due to the implementation of FFT.
        let irmax =
            (self.nfft.unwrap() / 2 + 1).min((1.01 + self.rmax_out.unwrap() / rstep) as usize);

        self.r = Some(Array1::range(0.0, irmax as f64 * rstep, rstep));

        self.chir = Some(cchi_fft.clone());
        self.chir_mag = Some(cchi_fft[0..irmax].norm());
        self.kwin = Some(win);

        self
    }

    pub fn get_r(&self) -> Option<ArrayBase<OwnedRepr<f64>, Ix1>> {
        self.r.clone()
    }

    pub fn get_chir(&self) -> Option<DynRealDft<f64>> {
        self.chir.clone()
    }

    pub fn get_chir_real(&self) -> Option<ArrayBase<OwnedRepr<f64>, Ix1>> {
        let len_r = self.r.as_ref().unwrap().len();

        let chir: Array1<f64> = self.chir.clone().unwrap().re();

        Some(chir.slice_axis(Axis(0), (0..len_r).into()).to_owned())
    }

    pub fn get_chir_imag(&self) -> Option<ArrayBase<OwnedRepr<f64>, Ix1>> {
        let len_r = self.r.as_ref().unwrap().len();

        let chir: Array1<f64> = self.chir.clone().unwrap().re();

        Some(chir.slice_axis(Axis(0), (0..len_r).into()).to_owned())
    }

    pub fn get_chir_mag(&self) -> Option<ArrayBase<OwnedRepr<f64>, Ix1>> {
        self.chir_mag.clone()
    }

    pub fn get_kwin(&self) -> Option<ArrayBase<OwnedRepr<f64>, Ix1>> {
        self.kwin.clone()
    }

    pub fn get_kstep(&self) -> Option<f64> {
        self.kstep.clone()
    }

    pub fn get_kweight(&self) -> Option<f64> {
        self.kweight.clone()
    }
}

#[derive(Debug, Clone)]
pub struct XrayFFTR {
    pub qmax_out: Option<f64>,
    pub window: Option<FTWindow>,
    pub dr: Option<f64>,
    pub dr2: Option<f64>,
    pub rmin: Option<f64>,
    pub rmax: Option<f64>,
    pub rweight: Option<f64>,
    pub nfft: Option<usize>,
    pub kstep: Option<f64>,
    pub q: Option<ArrayBase<OwnedRepr<f64>, Ix1>>,
    pub chiq: Option<ArrayBase<OwnedRepr<f64>, Ix1>>,
    pub rwin: Option<ArrayBase<OwnedRepr<f64>, Ix1>>,
}

impl Default for XrayFFTR {
    fn default() -> Self {
        XrayFFTR {
            qmax_out: Some(10.0),
            window: Some(FTWindow::KaiserBessel),
            dr: Some(1.),
            dr2: None,
            rmin: Some(0.),
            rmax: Some(20.0),
            rweight: Some(0.0),
            nfft: Some(2048),
            kstep: None,
            q: None,
            chiq: None,
            rwin: None,
        }
    }
}

impl XrayFFTR {
    pub fn new() -> XrayFFTR {
        XrayFFTR::default()
    }

    pub fn fill_parameter(&mut self, r: &ArrayBase<OwnedRepr<f64>, Ix1>) -> &mut Self {
        if self.rweight.is_none() {
            self.rweight = Some(0.0);
        }

        self.rweight = Some(self.rweight.unwrap().max(0.0).floor());

        if self.rmin.is_none() {
            self.rmin = Some(r[0]);
        }

        if self.rmax.is_none() {
            self.rmax = Some(r[r.len() - 1]);
        }

        if self.dr.is_none() {
            self.dr = Some(1.0);
        }

        if self.nfft.is_none() {
            self.nfft = Some(2048);
        }

        if self.qmax_out.is_none() {
            self.qmax_out = Some(10.0);
        }

        if self.kstep.is_none() {
            self.kstep = Some(std::f64::consts::PI / (r[1] - r[0]) / self.nfft.unwrap() as f64);
        }
        self
    }

    pub fn xftr_prep(
        &mut self,
        r: &ArrayBase<OwnedRepr<f64>, Ix1>,
        chir: &DynRealDft<f64>,
    ) -> Result<(DynRealDft<f64>, ArrayBase<OwnedRepr<f64>, Ix1>), Box<dyn std::error::Error>> {
        self.fill_parameter(r);
        let rweight = self.rweight.unwrap() as i32;
        // let r_max = r.max();
        // let npts = (1.01 + &r_max / self.kstep.unwrap()) as usize;
        let nfft = self.nfft.unwrap();
        let r_len = chir.len();
        let rstep = std::f64::consts::PI / self.kstep.unwrap() / nfft as f64;

        let r_ = Array1::range(0.0, r_len as f64 * rstep, rstep);

        let win = if rweight == 0 {
            ftwindow(&r_, self.rmin, self.rmax, self.dr, self.dr2, self.window)?
        } else {
            ftwindow(&r_, self.rmin, self.rmax, self.dr, self.dr2, self.window)?
                * &r_.map(|x| x.powi(rweight))
        };

        let chir_win = chir
            .iter()
            .zip(win.iter())
            .map(|(x, y)| x * y)
            .collect::<Vec<Complex<f64>>>();

        let chir_win = DynRealDft::new(chir.get_offset().clone(), &chir_win[1..], nfft);

        Ok((chir_win, win))
    }

    pub fn xftr(
        &mut self,
        r: &ArrayBase<OwnedRepr<f64>, Ix1>,
        chir: &DynRealDft<f64>,
    ) -> &mut Self {
        let (chir_win, win) = self.xftr_prep(r, chir).unwrap();
        let nfft = self.nfft.unwrap();
        let out = xftr_fast(&chir_win, nfft, self.kstep.unwrap());

        let q = Array1::linspace(
            0.0,
            self.qmax_out.unwrap(),
            (1.05 + self.qmax_out.unwrap() / self.kstep.unwrap()) as usize,
        );

        self.q = Some(q);
        self.rwin = Some(win);
        self.chiq = Some(out);

        self
    }

    pub fn get_q(&self) -> Option<ArrayBase<OwnedRepr<f64>, Ix1>> {
        self.q.clone()
    }

    pub fn get_chiq(&self) -> Option<ArrayBase<OwnedRepr<f64>, Ix1>> {
        if self.q.is_none() || self.chiq.is_none() {
            return None;
        }

        let len_q = self.q.as_ref().unwrap().len();

        Some(
            self.chiq
                .as_ref()
                .unwrap()
                .slice_axis(Axis(0), (0..len_q).into())
                .to_owned(),
        )
    }

    pub fn get_rwin(&self) -> Option<ArrayBase<OwnedRepr<f64>, Ix1>> {
        self.rwin.clone()
    }

    pub fn get_kstep(&self) -> Option<f64> {
        self.kstep.clone()
    }

    pub fn get_rweight(&self) -> Option<f64> {
        self.rweight.clone()
    }

    pub fn get_nfft(&self) -> Option<usize> {
        self.nfft.clone()
    }

    pub fn get_window(&self) -> Option<FTWindow> {
        self.window.clone()
    }
}

pub fn xftf_fast(chi: &ArrayBase<OwnedRepr<f64>, Ix1>, nfft: usize, kstep: f64) -> DynRealDft<f64> {
    let mut cchi = vec![0.0 as f64; nfft];
    cchi[..chi.len()].copy_from_slice(&chi.to_vec()[..]);

    let mut freq = cchi.real_fft();

    freq *= kstep / (std::f64::consts::PI).sqrt();

    freq
}

pub fn xftr_fast(
    chir: &DynRealDft<f64>,
    nfft: usize,
    kstep: f64,
) -> ArrayBase<OwnedRepr<f64>, Ix1> {
    let cchi = if chir.len() < nfft / 2 + 1 {
        let mut freq_bin = vec![Complex::new(0.0, 0.0); nfft - 1];
        freq_bin[..chir.len() - 1].copy_from_slice(chir.get_frequency_bins());
        DynRealDft::new(chir.get_offset().clone(), &freq_bin, nfft)
    } else {
        chir.clone()
    };

    let mut chi = Array1::from(cchi.real_ifft());

    chi *= std::f64::consts::PI.sqrt() / kstep / nfft as f64;

    chi
}

pub fn xftf_fast_nalgebra(chi: &DVector<f64>, nfft: usize, kstep: f64) -> DynRealDft<f64> {
    let mut cchi = vec![0.0 as f64; nfft];
    cchi[..chi.len()].copy_from_slice(&chi.data.as_vec()[..]);

    let mut freq = cchi.real_fft();

    freq *= kstep / std::f64::consts::PI.sqrt();

    freq
}

pub fn xftr_fast_nalgebra(chir: &DynRealDft<f64>, nfft: usize, kstep: f64) -> DVector<f64> {
    let cchi = if chir.len() < nfft / 2 + 1 {
        let mut freq_bin = vec![Complex::new(0.0, 0.0); nfft - 1];
        freq_bin[..chir.len() - 1].copy_from_slice(chir.get_frequency_bins());
        DynRealDft::new(chir.get_offset().clone(), &freq_bin, nfft)
    } else {
        chir.clone()
    };

    let mut chi = DVector::from(cchi.real_ifft().to_vec());

    chi *= (std::f64::consts::PI).sqrt() / kstep / nfft as f64;

    chi
}

pub trait XFFT {
    fn xftf_fast(&self, nfft: usize, kstep: f64) -> DynRealDft<f64>;
}

impl XFFT for ArrayBase<OwnedRepr<f64>, Ix1> {
    fn xftf_fast(&self, nfft: usize, kstep: f64) -> DynRealDft<f64> {
        xftf_fast(self, nfft, kstep)
    }
}

impl XFFT for DVector<f64> {
    fn xftf_fast(&self, nfft: usize, kstep: f64) -> DynRealDft<f64> {
        xftf_fast_nalgebra(self, nfft, kstep)
    }
}

pub trait XFFTReverse<T> {
    fn xftr_fast(&self, nfft: usize, kstep: f64) -> T;
}

impl XFFTReverse<ArrayBase<OwnedRepr<f64>, Ix1>> for DynRealDft<f64> {
    fn xftr_fast(&self, nfft: usize, kstep: f64) -> ArrayBase<OwnedRepr<f64>, Ix1> {
        xftr_fast(self, nfft, kstep)
    }
}

impl XFFTReverse<DVector<f64>> for DynRealDft<f64> {
    fn xftr_fast(&self, nfft: usize, kstep: f64) -> DVector<f64> {
        xftr_fast_nalgebra(self, nfft, kstep)
    }
}

pub trait FFTUtils<T> {
    fn realimg(&self) -> T;
    fn re(&self) -> T;
    fn im(&self) -> T;
    fn norm(&self) -> T;
    fn norm_sqr(&self) -> T;
}

impl FFTUtils<ArrayBase<OwnedRepr<f64>, Ix1>> for DynRealDft<f64> {
    fn realimg(&self) -> Array1<f64> {
        Array1::from_iter(self.iter().flat_map(|x| vec![x.re, x.im]))
    }

    fn re(&self) -> Array1<f64> {
        Array1::from_iter(self.iter().map(|x| x.re))
    }

    fn im(&self) -> Array1<f64> {
        Array1::from_iter(self.iter().map(|x| x.im))
    }

    fn norm(&self) -> Array1<f64> {
        Array1::from_iter(self.iter().map(|x| x.norm()))
    }

    fn norm_sqr(&self) -> Array1<f64> {
        Array1::from_iter(self.iter().map(|x| x.norm_sqr()))
    }
}

impl FFTUtils<DVector<f64>> for DynRealDft<f64> {
    fn realimg(&self) -> DVector<f64> {
        DVector::from_iterator(self.len() * 2, self.iter().flat_map(|x| vec![x.re, x.im]))
    }

    fn re(&self) -> DVector<f64> {
        DVector::from_iterator(self.len(), self.iter().map(|x| x.re))
    }

    fn im(&self) -> DVector<f64> {
        DVector::from_iterator(self.len(), self.iter().map(|x| x.im))
    }

    fn norm(&self) -> DVector<f64> {
        DVector::from_iterator(self.len(), self.iter().map(|x| x.norm()))
    }

    fn norm_sqr(&self) -> DVector<f64> {
        DVector::from_iterator(self.len(), self.iter().map(|x| x.norm_sqr()))
    }
}

impl FFTUtils<ArrayBase<OwnedRepr<f64>, Ix1>> for [Complex<f64>] {
    fn realimg(&self) -> Array1<f64> {
        Array1::from_iter(self.iter().flat_map(|x| vec![x.re, x.im]))
    }

    fn re(&self) -> Array1<f64> {
        Array1::from_iter(self.iter().map(|x| x.re))
    }

    fn im(&self) -> Array1<f64> {
        Array1::from_iter(self.iter().map(|x| x.im))
    }

    fn norm(&self) -> Array1<f64> {
        Array1::from_iter(self.iter().map(|x| x.norm()))
    }

    fn norm_sqr(&self) -> Array1<f64> {
        Array1::from_iter(self.iter().map(|x| x.norm_sqr()))
    }
}

impl FFTUtils<DVector<f64>> for [Complex<f64>] {
    fn realimg(&self) -> DVector<f64> {
        DVector::from_iterator(self.len() * 2, self.iter().flat_map(|x| vec![x.re, x.im]))
    }

    fn re(&self) -> DVector<f64> {
        DVector::from_iterator(self.len(), self.iter().map(|x| x.re))
    }

    fn im(&self) -> DVector<f64> {
        DVector::from_iterator(self.len(), self.iter().map(|x| x.im))
    }

    fn norm(&self) -> DVector<f64> {
        DVector::from_iterator(self.len(), self.iter().map(|x| x.norm()))
    }

    fn norm_sqr(&self) -> DVector<f64> {
        DVector::from_iterator(self.len(), self.iter().map(|x| x.norm_sqr()))
    }
}

// impl PartialEq for DynRealDft<f64> {
//     fn eq(&self, other: &Self) -> bool {
//         self.len() == other.len()
//             && self.get_offset() == other.get_offset()
//             && self.get_frequency_bins() == other.get_frequency_bins()
//     }
// }

// #[derive(Debug, Clone, PartialEq, Default)]
// pub struct XrayFFTR {}

#[cfg(test)]
mod test {
    use easyfft::prelude::*;
    use ndarray::Array1;

    use super::*;
    use crate::xafs::io;
    use crate::xafs::nshare::ToNalgebra;
    use approx::{assert_abs_diff_eq, assert_relative_eq};

    use data_reader::reader::{load_txt_f64, Delimiter, ReaderParams};

    use crate::xafs::tests::PARAM_LOADTXT;
    use crate::xafs::tests::TEST_TOL;
    use crate::xafs::tests::TOP_DIR;

    use crate::xafs::background::BackgroundMethod;
    use crate::xafs::background::AUTOBK;

    const ACCEPTABLE_MU_DIFF: f64 = 1e-6;
    const CHI_MSE_TOL: f64 = 1e-2;
    const CHI_Q_TOL: f64 = 1e-1;

    #[test]
    fn test_xftf_fast() {
        let x: Array1<f64> = Array1::linspace(0., 10., 10);
        let sin_x = x.map(|x| x.sin());
        let nfft = 10;
        let kstep = 1.;
        let fft = xftf_fast(&sin_x, nfft, kstep);

        let norm: DVector<f64> = fft.norm();

        let expected_norm = DVector::from(vec![
            0.6822515304148188,
            0.999816632055004,
            2.4133321684349966,
            0.35447122637608214,
            0.16620199767343982,
            0.1252841340812192,
        ]);

        assert_abs_diff_eq!(norm, expected_norm, epsilon = 1e-16);
    }

    #[test]
    fn test_xftr_fast() {
        let x: Array1<f64> = Array1::linspace(0., 10., 1024);
        let sin_x = x.map(|x| x.sin());
        let nfft = 1024;
        let kstep = 0.1;

        let fft = xftf_fast(&sin_x, nfft, kstep);
        let ifft = xftr_fast(&fft, nfft, kstep);

        sin_x.iter().zip(ifft.iter()).for_each(|(x, y)| {
            assert_abs_diff_eq!(x, y, epsilon = 1e-12);
        });
    }

    #[test]
    fn test_xftf_fast_nalgebra() {
        let x: DVector<f64> = Array1::linspace(0., 10., 10).into_nalgebra();
        let sin_x = x.map(|x| x.sin());
        let nfft = 10;
        let kstep = 1.;
        let fft = xftf_fast_nalgebra(&sin_x, nfft, kstep);

        let norm: DVector<f64> = fft.norm();

        let expected_norm = DVector::from(vec![
            0.6822515304148188,
            0.999816632055004,
            2.4133321684349966,
            0.35447122637608214,
            0.16620199767343982,
            0.1252841340812192,
        ]);

        assert_abs_diff_eq!(norm, expected_norm, epsilon = 1e-16);
    }

    #[test]
    fn test_xftr_fast_nalgebra() {
        let x: DVector<f64> = Array1::linspace(0., 10., 1024).into_nalgebra();
        let sin_x = x.map(|x| x.sin());
        let nfft = 1024;
        let kstep = 0.1;

        let fft = xftf_fast_nalgebra(&sin_x, nfft, kstep);
        let ifft = xftr_fast_nalgebra(&fft, nfft, kstep);

        sin_x.iter().zip(ifft.iter()).for_each(|(x, y)| {
            assert_abs_diff_eq!(x, y, epsilon = 1e-12);
        });
    }

    #[test]
    #[allow(non_snake_case)]
    fn test_Xray_FFTF() -> Result<(), Box<dyn std::error::Error>> {
        let path = String::from(TOP_DIR) + "/tests/testfiles/Ru_QAS.dat";
        let mut xafs_test_group = io::load_spectrum(&path).unwrap();

        xafs_test_group.set_background_method(Some(BackgroundMethod::AUTOBK(AUTOBK {
            rbkg: Some(1.4),
            k_weight: Some(2),
            ..Default::default()
        })));
        xafs_test_group.calc_background()?;

        xafs_test_group.xftf = Some(XrayFFTF {
            window: Some(FTWindow::Hanning),
            dk: Some(1.0),
            kmin: Some(2.0),
            kmax: Some(15.0),
            kweight: Some(2.0),
            ..Default::default()
        });
        xafs_test_group.fft()?;

        let larch_r_path = String::from(TOP_DIR) + "/tests/testfiles/Ru_QAS_xftf_larch.txt";
        let larch_r = load_txt_f64(&larch_r_path, &PARAM_LOADTXT).unwrap();

        let r_expected = larch_r.get_col(0);
        let chir_expected = larch_r.get_col(1);

        let r = xafs_test_group.get_r().unwrap();
        let chir = xafs_test_group.get_chir_mag().unwrap();

        r.iter().zip(r_expected.iter()).for_each(|(x, y)| {
            assert_abs_diff_eq!(x, y, epsilon = TEST_TOL);
        });

        let mse = chir
            .iter()
            .zip(chir_expected.iter())
            .map(|(x, y)| (x - y).powi(2))
            .sum::<f64>()
            / r.len() as f64;

        // println!("mse: {}", mse);
        assert!(mse < CHI_MSE_TOL);

        // chir.iter().zip(chir_expected.iter()).for_each(|(x, y)| {
        //     println!("[{}, {}],", x, y);
        // });

        // mse = r
        // .iter()
        // .zip(r_expected.iter())
        // .map(|(x, y)| (x - y).powi(2))
        // .sum::<f64>()
        // / r.len() as f64;

        // println!("mse: {}", mse);

        // assert!(mse < CHI_MSE_TOL);

        Ok(())
    }

    #[test]
    fn test_XrayFFTR() -> Result<(), Box<dyn std::error::Error>> {
        let path = String::from(TOP_DIR) + "/tests/testfiles/Ru_QAS.dat";
        let mut xafs_test_group = io::load_spectrum(&path).unwrap();

        xafs_test_group.set_background_method(Some(BackgroundMethod::AUTOBK(AUTOBK {
            rbkg: Some(1.4),
            ..Default::default()
        })));
        xafs_test_group.calc_background()?;

        xafs_test_group.xftf = Some(XrayFFTF {
            window: Some(FTWindow::Hanning),
            dk: Some(std::f64::EPSILON),
            kmin: Some(0.0),
            kmax: Some(15.0),
            kweight: Some(2.0),
            ..Default::default()
        });
        xafs_test_group.fft()?;

        xafs_test_group.xftr = Some(XrayFFTR {
            window: Some(FTWindow::Hanning),
            rweight: Some(0.0),
            dr: Some(std::f64::EPSILON),
            rmin: Some(0.0),
            rmax: Some(10.0),
            ..Default::default()
        });
        xafs_test_group.ifft()?;

        let q = xafs_test_group.get_q().unwrap();
        let chiq = xafs_test_group.get_chiq().unwrap();

        // println!("q: {:?}", q.len());
        // println!("chiq: {:?}", chiq.len());

        assert!(q.len() == chiq.len());

        let chi = xafs_test_group.get_chi_kweighted().unwrap()
            * xafs_test_group.xftf.unwrap().get_kwin().unwrap();

        let chi = chi.slice_axis(Axis(0), (0..chi.len()).into()).to_owned();

        chi.iter().zip(chiq.iter()).for_each(|(x, y)| {
            assert_relative_eq!(x, y, epsilon = CHI_Q_TOL);
        });

        Ok(())
    }
}
