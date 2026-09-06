use derivative::Derivative;
use easyfft::prelude::DynRealFft;
use easyfft::{dyn_size::realfft::DynRealDft, num_complex::Complex};
use nalgebra::DVector;
use serde::{Deserialize, Serialize};

use super::errors::FFTError;
use super::xafsutils::{ftwindow, FTWindow};

#[derive(Derivative, Debug, Clone, Serialize, Deserialize)]
#[derivative(PartialEq)]
#[serde(default)]
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
    pub r: Option<DVector<f64>>,
    #[derivative(PartialEq = "ignore")]
    pub chir: Option<DynRealDft<f64>>,
    pub chir_mag: Option<DVector<f64>>,
    pub kwin: Option<DVector<f64>>,
}

impl Default for XrayFFTF {
    fn default() -> Self {
        Self {
            rmax_out: Some(10.0),
            window: Some(FTWindow::KaiserBessel),
            dk: Some(1.0),
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
        Self::default()
    }

    pub fn fill_parameter(&mut self, k: &DVector<f64>) -> &mut Self {
        if self.kweight.is_none() {
            self.kweight = Some(2.0);
        }

        self.kweight = Some(self.kweight.unwrap().max(0.0).floor());

        if self.kstep.is_none() {
            self.kstep = Some(if k.len() > 1 { k[1] - k[0] } else { 0.05 });
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

    pub fn xftf(&mut self, k: &DVector<f64>, chi: &DVector<f64>) -> Result<&mut Self, FFTError> {
        if self.nfft.is_some_and(|n| n < 2) {
            return Err(FFTError::InvalidParameter {
                parameter: "nfft".into(),
                reason: "must be at least 2".into(),
            });
        }
        for (name, value, strictly_positive) in [
            ("kstep", self.kstep, true),
            ("kweight", self.kweight, false),
            ("dk", self.dk, false),
            ("dk2", self.dk2, false),
            ("rmax_out", self.rmax_out, false),
        ] {
            if value.is_some_and(|v| !v.is_finite() || v < 0.0 || (strictly_positive && v == 0.0)) {
                return Err(FFTError::InvalidParameter {
                    parameter: name.into(),
                    reason: "must be finite and nonnegative (kstep must be positive)".into(),
                });
            }
        }
        if self.kmin.is_some_and(|v| !v.is_finite())
            || self.kmax.is_some_and(|v| !v.is_finite())
            || self.kmin.zip(self.kmax).is_some_and(|(lo, hi)| lo >= hi)
        {
            return Err(FFTError::InvalidParameter {
                parameter: "kmin/kmax".into(),
                reason: "must be finite with kmin < kmax".into(),
            });
        }
        if k.len() != chi.len() {
            return Err(FFTError::InterpolationFailed {
                reason: "k/chi length mismatch".to_string(),
            });
        }
        if k.len() < 2 {
            return Err(FFTError::InsufficientPoints {
                min: 2,
                actual: k.len(),
                kmin: 0.0,
                kmax: 0.0,
            });
        }

        self.fill_parameter(k);
        let nfft = self.nfft.unwrap();
        let kweight = self.kweight.unwrap();

        let mut chi_weighted = DVector::zeros(chi.len());
        for i in 0..chi.len() {
            chi_weighted[i] = chi[i] * k[i].powf(kweight);
        }

        let win =
            ftwindow(k, self.kmin, self.kmax, self.dk, self.dk2, self.window).map_err(|e| {
                FFTError::WindowCalculationFailed {
                    reason: e.to_string(),
                }
            })?;

        for i in 0..chi_weighted.len() {
            chi_weighted[i] *= win[i];
        }

        let cchi_fft = xftf_fast_nalgebra(&chi_weighted, nfft, self.kstep.unwrap());
        let rstep = std::f64::consts::PI / self.kstep.unwrap() / nfft as f64;
        let irmax = (nfft / 2 + 1)
            .min((1.01 + self.rmax_out.unwrap() / rstep) as usize)
            .max(1);

        self.r = Some(linspace(0.0, (irmax - 1) as f64 * rstep, irmax));
        self.chir_mag = Some(DVector::from_iterator(
            irmax,
            cchi_fft.iter().take(irmax).map(|x| x.norm()),
        ));
        self.kwin = Some(win);
        self.chir = Some(cchi_fft);

        Ok(self)
    }

    pub fn get_rmax_out(&self) -> Option<&f64> {
        self.rmax_out.as_ref()
    }

    pub fn get_window(&self) -> Option<&FTWindow> {
        self.window.as_ref()
    }

    pub fn get_dk(&self) -> Option<&f64> {
        self.dk.as_ref()
    }

    pub fn get_dk2(&self) -> Option<&f64> {
        self.dk2.as_ref()
    }

    pub fn get_kmin(&self) -> Option<&f64> {
        self.kmin.as_ref()
    }

    pub fn get_kmax(&self) -> Option<&f64> {
        self.kmax.as_ref()
    }

    pub fn get_kweight(&self) -> Option<&f64> {
        self.kweight.as_ref()
    }

    pub fn get_r(&self) -> Option<&DVector<f64>> {
        self.r.as_ref()
    }

    pub fn get_chir(&self) -> Option<&DynRealDft<f64>> {
        self.chir.as_ref()
    }

    pub fn get_chir_real(&self) -> Option<DVector<f64>> {
        let len_r = self.r.as_ref()?.len();
        let chir = self.chir.as_ref()?;
        Some(DVector::from_iterator(
            len_r,
            chir.iter().take(len_r).map(|x| x.re),
        ))
    }

    pub fn get_chir_imag(&self) -> Option<DVector<f64>> {
        let len_r = self.r.as_ref()?.len();
        let chir = self.chir.as_ref()?;
        Some(DVector::from_iterator(
            len_r,
            chir.iter().take(len_r).map(|x| x.im),
        ))
    }

    pub fn get_chir_mag(&self) -> Option<&DVector<f64>> {
        self.chir_mag.as_ref()
    }

    pub fn get_kwin(&self) -> Option<&DVector<f64>> {
        self.kwin.as_ref()
    }

    pub fn get_kstep(&self) -> Option<&f64> {
        self.kstep.as_ref()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
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
    pub q: Option<DVector<f64>>,
    pub chiq: Option<DVector<f64>>,
    pub rwin: Option<DVector<f64>>,
}

impl Default for XrayFFTR {
    fn default() -> Self {
        Self {
            qmax_out: Some(10.0),
            window: Some(FTWindow::KaiserBessel),
            dr: Some(1.0),
            dr2: None,
            rmin: Some(0.0),
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
        Self::default()
    }

    pub fn fill_parameter(&mut self, r: &DVector<f64>) -> &mut Self {
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
            self.kstep = Some(if r.len() > 1 {
                std::f64::consts::PI / (r[1] - r[0]) / self.nfft.unwrap() as f64
            } else {
                0.05
            });
        }
        self
    }

    pub fn xftr(
        &mut self,
        r: &DVector<f64>,
        chir: &DynRealDft<f64>,
    ) -> Result<&mut Self, FFTError> {
        super::inverse_fft::validate(r.as_slice(), self)?;
        self.fill_parameter(r);
        let rstep = std::f64::consts::PI / self.kstep.unwrap() / self.nfft.unwrap() as f64;
        let full_r = DVector::from_iterator(chir.len(), (0..chir.len()).map(|i| i as f64 * rstep));
        let mut win = ftwindow(
            &full_r,
            self.rmin,
            self.rmax,
            self.dr,
            self.dr2,
            self.window,
        )
        .map_err(|e| FFTError::WindowCalculationFailed {
            reason: e.to_string(),
        })?;
        let weight = self.rweight.unwrap();
        if weight > 0.0 {
            for (window, radius) in win.iter_mut().zip(full_r.iter()) {
                *window *= radius.powf(weight);
            }
        }
        // The multiplication covers DC, all positive bins and Nyquist, at
        // their actual R positions. Display rmax_out never truncates the filter.
        let chir_scaled = chir * win.as_slice();
        let out = xftr_fast_nalgebra(&chir_scaled, self.nfft.unwrap(), self.kstep.unwrap());
        self.q = Some(DVector::from_vec(super::inverse_fft::q_grid(
            self.qmax_out.unwrap(),
            self.kstep.unwrap(),
            out.len(),
        )));
        self.rwin = Some(win);
        self.chiq = Some(out);

        Ok(self)
    }

    pub fn get_q(&self) -> Option<&DVector<f64>> {
        self.q.as_ref()
    }

    pub fn get_chiq(&self) -> Option<DVector<f64>> {
        let len_q = self.q.as_ref()?.len();
        let chiq = self.chiq.as_ref()?;
        Some(DVector::from_iterator(
            len_q.min(chiq.len()),
            chiq.iter().take(len_q).copied(),
        ))
    }

    pub fn get_rwin(&self) -> Option<&DVector<f64>> {
        self.rwin.as_ref()
    }

    pub fn get_kstep(&self) -> Option<&f64> {
        self.kstep.as_ref()
    }

    pub fn get_rweight(&self) -> Option<&f64> {
        self.rweight.as_ref()
    }

    pub fn get_nfft(&self) -> Option<&usize> {
        self.nfft.as_ref()
    }

    pub fn get_window(&self) -> Option<&FTWindow> {
        self.window.as_ref()
    }
}

pub fn xftf_fast_nalgebra(chi: &DVector<f64>, nfft: usize, kstep: f64) -> DynRealDft<f64> {
    let mut cchi = vec![0.0_f64; nfft];
    cchi[..chi.len().min(nfft)].copy_from_slice(&chi.as_slice()[..chi.len().min(nfft)]);

    let mut freq = cchi.real_fft();
    freq *= kstep / std::f64::consts::PI.sqrt();
    freq
}

pub fn xftr_fast_nalgebra(chir: &DynRealDft<f64>, nfft: usize, kstep: f64) -> DVector<f64> {
    DVector::from_vec(super::inverse_fft::inverse(chir, nfft, kstep))
}

pub trait XFFT {
    fn xftf_fast(&self, nfft: usize, kstep: f64) -> DynRealDft<f64>;
}

impl XFFT for DVector<f64> {
    fn xftf_fast(&self, nfft: usize, kstep: f64) -> DynRealDft<f64> {
        xftf_fast_nalgebra(self, nfft, kstep)
    }
}

pub trait XFFTReverse<T> {
    fn xftr_fast(&self, nfft: usize, kstep: f64) -> T;
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

impl FFTUtils<DVector<f64>> for DynRealDft<f64> {
    fn realimg(&self) -> DVector<f64> {
        DVector::from_iterator(self.len() * 2, self.iter().flat_map(|x| [x.re, x.im]))
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

impl FFTUtils<DVector<f64>> for [Complex<f64>] {
    fn realimg(&self) -> DVector<f64> {
        DVector::from_iterator(self.len() * 2, self.iter().flat_map(|x| [x.re, x.im]))
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

fn linspace(start: f64, end: f64, n: usize) -> DVector<f64> {
    if n <= 1 {
        return DVector::from_vec(vec![start]);
    }

    let step = (end - start) / (n as f64 - 1.0);
    DVector::from_iterator(n, (0..n).map(|i| start + step * i as f64))
}
