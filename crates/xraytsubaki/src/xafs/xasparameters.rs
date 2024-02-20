#![allow(dead_code)]
#![allow(unused_imports)]

#[cfg_attr(debug_assertions, allow(dead_code, unused_imports))]
// Standard library dependencies
use std::error::Error;

// External dependencies
use serde::{Deserialize, Serialize};

// load dependencies
use super::background;
use super::io;
use super::lmutils;
use super::mathutils;
use super::normalization;
use super::nshare;
use super::xafsutils;
use super::xrayfft;

// Load local traits
use background::{BackgroundMethod, ILPBkg, AUTOBK};
use mathutils::MathUtils;
use normalization::Normalization;
use normalization::{MBack, NormalizationMethod, PrePostEdge};
use xafsutils::FTWindow;

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Default)]
#[serde(default)]
pub struct XASParameters<'a> {
    e0: Option<f64>,
    normalization_method: Option<NormalizationMethod>,
    pre_edge_start: Option<f64>,
    pre_edge_end: Option<f64>,
    norm_start: Option<f64>,
    norm_end: Option<f64>,
    norm_polyorder: Option<i32>,
    n_victoreen: Option<i32>,
    edge_step: Option<f64>,
    background_method: Option<BackgroundMethod>,
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
    bkg_window: Option<&'a str>,
    bkg_dk: Option<f64>,
    fft_rmax_out: Option<f64>,
    fft_window: Option<FTWindow>,
    fft_dk: Option<f64>,
    fft_dk2: Option<f64>,
    fft_kmin: Option<f64>,
    fft_kmax: Option<f64>,
    fft_kweight: Option<f64>,
    fft_nfft: Option<i32>,
    fft_kstep: Option<f64>,
    ifft_qmax_out: Option<f64>,
    ifft_window: Option<FTWindow>,
    ifft_dr: Option<f64>,
    ifft_dr2: Option<f64>,
    ifft_rmin: Option<f64>,
    ifft_rmax: Option<f64>,
    ifft_rweight: Option<f64>,
    ifft_nfft: Option<i32>,
    ifft_kstep: Option<f64>,
}

impl<'a> XASParameters<'a> {
    pub fn new() -> Self {
        XASParameters {
            ..XASParameters::default()
        }
    }

    pub fn set_normalization_method(
        &mut self,
        normalization_method: &str,
    ) -> Result<&mut Self, Box<dyn Error>> {
        match normalization_method {
            x if x.to_lowercase().starts_with('p') => {
                self.normalization_method =
                    Some(NormalizationMethod::PrePostEdge(PrePostEdge::new()));
            }
            x if x.to_lowercase().starts_with('m') => {
                self.normalization_method = Some(NormalizationMethod::MBack(MBack::new()))
            }
            _ => {
                self.normalization_method =
                    Some(NormalizationMethod::PrePostEdge(PrePostEdge::new()));
            }
        };

        Ok(self)
    }

    pub fn set_background_method(
        &mut self,
        background_method: &str,
    ) -> Result<&mut Self, Box<dyn Error>> {
        match background_method {
            x if x.to_lowercase().starts_with('a') => {
                self.background_method = Some(BackgroundMethod::AUTOBK(AUTOBK::new()));
            }
            x if x.to_lowercase().starts_with('i') => {
                self.background_method = Some(BackgroundMethod::ILPBkg(ILPBkg::new()));
            }
            _ => {
                self.background_method = Some(BackgroundMethod::AUTOBK(AUTOBK::new()));
            }
        };
        Ok(self)
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_initialize_xas_parameters() {
        let xas_params = XASParameters::new();
    }
}
