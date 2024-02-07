use std::mem;

use numpy::{IntoPyArray, PyArray1, PyReadonlyArray, PyReadonlyArray1};
use pyo3::prelude::*;
use xraytsubaki::{prelude::*, xafs::xasspectrum};

#[pyclass]
#[repr(transparent)]
#[derive(Clone)]
pub struct PyXASSpectrum {
    pub xasspectrum: XASSpectrum,
}

#[pymethods]
#[allow(clippy::should_implement_trait)]
impl PyXASSpectrum {
    #[new]
    pub fn new(
        energy: Option<PyReadonlyArray1<f64>>,
        mu: Option<PyReadonlyArray1<f64>>,
    ) -> PyResult<Self> {
        let mut xas_spectrum = XASSpectrum::new();

        xas_spectrum.raw_energy = energy.map(|x| x.as_array().to_owned());
        xas_spectrum.raw_mu = mu.map(|x| x.as_array().to_owned());
        Ok(PyXASSpectrum {
            xasspectrum: xas_spectrum,
        })
    }

    #[pyo3(signature = (e0 = None, normalization_method="prepostedge", pre_edge_start = None, pre_edge_end = None, norm_start = None, norm_end = None, norm_polyorder = None, n_victoreen = None, edge_step = None, background_method="autobk", ek0 = None, rbkg = None, bkg_nknots = None, bkg_kmin = None, bkg_kmax = None, bkg_kstep = None, bkg_nclamp = None, bkg_clamp_lo = None, bkg_clamp_hi = None, bkg_nfft = None, bkg_window = None, bkg_dk = None, fft_rmax_out = None, fft_window = None, fft_dk = None, fft_dk2 = None, fft_kmin = None, fft_kmax = None, fft_kweight = None, fft_nfft = None, fft_kstep = None, ifft_qmax_out = None, ifft_window = None, ifft_dr = None, ifft_dr2 = None, ifft_rmin = None, ifft_rmax = None, ifft_rweight = None, ifft_nfft = None, ifft_kstep = None))]
    pub fn set_parameters(
        &self,
        e0: Option<f64>,
        normalization_method: Option<&str>,
        pre_edge_start: Option<f64>,
        pre_edge_end: Option<f64>,
        norm_start: Option<f64>,
        norm_end: Option<f64>,
        norm_polyorder: Option<i32>,
        n_victoreen: Option<i32>,
        edge_step: Option<f64>,
        background_method: Option<&str>,
        ek0: Option<f64>,
        rbkg: Option<f64>,
        bkg_nknots: Option<i32>,
        bkg_kmin: Option<f64>,
        bkg_kmax: Option<f64>,
        bkg_kstep: Option<f64>,
        bkg_nclamp: Option<i32>,
        bkg_clamp_lo: Option<i32>,
        bkg_clamp_hi: Option<i32>,
        bkg_nfft: Option<i32>,
        bkg_window: Option<&str>,
        bkg_dk: Option<f64>,
        fft_rmax_out: Option<f64>,
        fft_window: Option<&str>,
        fft_dk: Option<f64>,
        fft_dk2: Option<f64>,
        fft_kmin: Option<f64>,
        fft_kmax: Option<f64>,
        fft_kweight: Option<f64>,
        fft_nfft: Option<i32>,
        fft_kstep: Option<f64>,
        ifft_qmax_out: Option<f64>,
        ifft_window: Option<&str>,
        ifft_dr: Option<f64>,
        ifft_dr2: Option<f64>,
        ifft_rmin: Option<f64>,
        ifft_rmax: Option<f64>,
        ifft_rweight: Option<f64>,
        ifft_nfft: Option<i32>,
        ifft_kstep: Option<f64>,
    ) -> PyResult<&Self> {
        Ok(self)
    }
}
