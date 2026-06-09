//! Pipeline parameters edited in the context panel. `None` = let the core
//! library auto-determine ("auto" in the UI). The fingerprint keys the
//! processed-spectrum cache so edits invalidate exactly what they change.

use std::hash::{Hash, Hasher};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use xraytsubaki::prelude::*;
use xraytsubaki::xafs::background::AUTOBK;
use xraytsubaki::xafs::io;
use xraytsubaki::xafs::normalization::PrePostEdge;

#[derive(Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PipelineParams {
    // Normalization (pre/post-edge); energies relative to E0.
    pub e0: Option<f64>,
    pub pre_edge_start: Option<f64>,
    pub pre_edge_end: Option<f64>,
    pub norm_start: Option<f64>,
    pub norm_end: Option<f64>,
    // AUTOBK background.
    pub rbkg: Option<f64>,
    pub bkg_kmin: Option<f64>,
    pub bkg_kmax: Option<f64>,
    // Forward FFT.
    pub fft_kmin: Option<f64>,
    pub fft_kmax: Option<f64>,
    pub fft_dk: Option<f64>,
    pub fft_kweight: Option<f64>,
}

impl PipelineParams {
    pub fn fingerprint(&self) -> u64 {
        let mut hasher = std::hash::DefaultHasher::new();
        for v in [
            self.e0,
            self.pre_edge_start,
            self.pre_edge_end,
            self.norm_start,
            self.norm_end,
            self.rbkg,
            self.bkg_kmin,
            self.bkg_kmax,
            self.fft_kmin,
            self.fft_kmax,
            self.fft_dk,
            self.fft_kweight,
        ] {
            v.map(f64::to_bits).hash(&mut hasher);
        }
        hasher.finish()
    }
}

/// Load a file and run the full pipeline with the given parameters.
/// Runs on the background executor.
pub fn process_file(path: &PathBuf, params: &PipelineParams) -> Result<XASSpectrum, String> {
    let mut sp = io::load_spectrum_QAS_trans(path).map_err(|e| e.to_string())?;

    match params.e0 {
        Some(e0) => {
            sp.set_e0(e0);
        }
        None => {
            sp.find_e0().map_err(|e| e.to_string())?;
        }
    }

    let mut ppe = PrePostEdge::new();
    let defaults = PrePostEdge::default();
    ppe.pre_edge_start = params.pre_edge_start.or(defaults.pre_edge_start);
    ppe.pre_edge_end = params.pre_edge_end.or(defaults.pre_edge_end);
    ppe.norm_start = params.norm_start.or(defaults.norm_start);
    ppe.norm_end = params.norm_end.or(defaults.norm_end);
    ppe.norm_polyorder = defaults.norm_polyorder;
    ppe.n_victoreen = defaults.n_victoreen;
    sp.set_normalization_method(Some(NormalizationMethod::PrePostEdge(ppe)))
        .map_err(|e| e.to_string())?;
    sp.normalize().map_err(|e| e.to_string())?;

    let mut autobk = AUTOBK::new();
    if params.rbkg.is_some() {
        autobk.rbkg = params.rbkg;
    }
    if params.bkg_kmin.is_some() {
        autobk.kmin = params.bkg_kmin;
    }
    if params.bkg_kmax.is_some() {
        autobk.kmax = params.bkg_kmax;
    }
    sp.set_background_method(Some(BackgroundMethod::AUTOBK(autobk)))
        .map_err(|e| e.to_string())?;
    sp.calc_background().map_err(|e| e.to_string())?;

    let mut xftf = XrayFFTF::default();
    if params.fft_kmin.is_some() {
        xftf.kmin = params.fft_kmin;
    }
    if params.fft_kmax.is_some() {
        xftf.kmax = params.fft_kmax;
    }
    if params.fft_dk.is_some() {
        xftf.dk = params.fft_dk;
    }
    if params.fft_kweight.is_some() {
        xftf.kweight = params.fft_kweight;
    }
    sp.xftf = Some(xftf);
    sp.fft().map_err(|e| e.to_string())?;

    Ok(sp)
}
