use easyfft::prelude::{DynRealFft, DynRealIfft};
use easyfft::{dyn_size::realfft::DynRealDft, num_complex::Complex};
use nalgebra::{DVector, Owned};
use ndarray::{Array, Array1, ArrayBase, Ix1, OwnedRepr};

#[derive(Debug, Clone, PartialEq, Default)]
pub enum FFTWindow {
    #[default]
    Hanning,
    KaiserBessel,
    Parzen,
    Sine,
    Gaussian,
    Welch,
    None,
}

#[derive(Debug, Clone, PartialEq)]
pub struct XrayForwardFFT {
    pub rmax_out: Option<f64>,
    pub window: Option<FFTWindow>,
    pub dk: Option<f64>,
    pub dk2: Option<f64>,
    pub kmin: Option<f64>,
    pub kmax: Option<f64>,
    pub kweight: Option<f64>,
    pub nfft: Option<i32>,
}

impl Default for XrayForwardFFT {
    fn default() -> Self {
        XrayForwardFFT {
            rmax_out: Some(10.0),
            window: Some(FFTWindow::default()),
            dk: Some(0.05),
            dk2: None,
            kmin: Some(2.0),
            kmax: Some(15.0),
            kweight: Some(2.0),
            nfft: Some(2048),
        }
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

// pub fn xftf_fast_real

#[derive(Debug, Clone, PartialEq, Default)]
pub struct XrayReverseFFT {}

#[cfg(test)]
mod test {
    use easyfft::prelude::*;
    use ndarray::Array1;

    use super::*;
    use crate::xafs::nshare::ToNalgebra;
    use approx::assert_abs_diff_eq;

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

}
