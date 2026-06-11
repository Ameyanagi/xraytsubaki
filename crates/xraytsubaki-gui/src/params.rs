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
    /// Advanced: polynomial order of the post-edge fit.
    pub norm_polyorder: Option<i32>,
    /// Advanced: Victoreen exponent for the pre-edge fit.
    pub n_victoreen: Option<i32>,
    // AUTOBK background.
    pub rbkg: Option<f64>,
    pub bkg_kmin: Option<f64>,
    pub bkg_kmax: Option<f64>,
    // Advanced AUTOBK.
    pub bkg_kstep: Option<f64>,
    pub bkg_nknots: Option<i32>,
    pub bkg_kweight: Option<i32>,
    pub bkg_clamp_lo: Option<i32>,
    pub bkg_clamp_hi: Option<i32>,
    pub bkg_window: Option<FTWindow>,
    pub bkg_dk: Option<f64>,
    pub bkg_solver: Option<AUTOBKSolver>,
    pub bkg_nfft: Option<i32>,
    // Forward FFT.
    pub fft_kmin: Option<f64>,
    pub fft_kmax: Option<f64>,
    pub fft_dk: Option<f64>,
    pub fft_kweight: Option<f64>,
    // Advanced FFT.
    pub fft_dk2: Option<f64>,
    pub fft_rmax: Option<f64>,
    pub fft_window: Option<FTWindow>,
    pub fft_kstep: Option<f64>,
    pub fft_nfft: Option<i32>,
}

/// Cycle order for the window selector chips.
pub const FT_WINDOWS: [FTWindow; 7] = [
    FTWindow::Hanning,
    FTWindow::Parzen,
    FTWindow::Welch,
    FTWindow::Gaussian,
    FTWindow::Sine,
    FTWindow::KaiserBessel,
    FTWindow::FHanning,
];

pub const AUTOBK_SOLVERS: [AUTOBKSolver; 3] = [
    AUTOBKSolver::LinearDirect,
    AUTOBKSolver::TrustRegionDogLeg,
    AUTOBKSolver::LegacyLm,
];

/// None -> first -> ... -> last -> None ("auto").
pub fn cycle_option<T: Copy + PartialEq>(current: Option<T>, all: &[T]) -> Option<T> {
    match current {
        None => all.first().copied(),
        Some(c) => all
            .iter()
            .position(|x| *x == c)
            .and_then(|i| all.get(i + 1).copied()),
    }
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
            self.bkg_kstep,
            self.bkg_dk,
            self.fft_kmin,
            self.fft_kmax,
            self.fft_dk,
            self.fft_kweight,
            self.fft_dk2,
            self.fft_rmax,
            self.fft_kstep,
        ] {
            v.map(f64::to_bits).hash(&mut hasher);
        }
        for v in [
            self.norm_polyorder,
            self.n_victoreen,
            self.bkg_nknots,
            self.bkg_kweight,
            self.bkg_clamp_lo,
            self.bkg_clamp_hi,
            self.bkg_nfft,
            self.fft_nfft,
        ] {
            v.hash(&mut hasher);
        }
        format!(
            "{:?}|{:?}|{:?}",
            self.bkg_window, self.bkg_solver, self.fft_window
        )
        .hash(&mut hasher);
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
    ppe.norm_polyorder = params.norm_polyorder.or(defaults.norm_polyorder);
    ppe.n_victoreen = params.n_victoreen.or(defaults.n_victoreen);
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
    if params.bkg_kstep.is_some() {
        autobk.kstep = params.bkg_kstep;
    }
    if params.bkg_nknots.is_some() {
        autobk.nknots = params.bkg_nknots;
    }
    if params.bkg_kweight.is_some() {
        autobk.kweight = params.bkg_kweight;
    }
    if params.bkg_clamp_lo.is_some() {
        autobk.clamp_lo = params.bkg_clamp_lo;
    }
    if params.bkg_clamp_hi.is_some() {
        autobk.clamp_hi = params.bkg_clamp_hi;
    }
    if let Some(window) = params.bkg_window {
        autobk.window = window;
    }
    if params.bkg_dk.is_some() {
        autobk.dk = params.bkg_dk;
    }
    if params.bkg_solver.is_some() {
        autobk.solver = params.bkg_solver;
    }
    if let Some(nfft) = params.bkg_nfft {
        autobk.nfft = Some(nfft);
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
    if params.fft_dk2.is_some() {
        xftf.dk2 = params.fft_dk2;
    }
    if params.fft_rmax.is_some() {
        xftf.rmax_out = params.fft_rmax;
    }
    if params.fft_window.is_some() {
        xftf.window = params.fft_window;
    }
    if params.fft_kstep.is_some() {
        xftf.kstep = params.fft_kstep;
    }
    if let Some(nfft) = params.fft_nfft {
        xftf.nfft = Some(nfft.max(64) as usize);
    }
    sp.xftf = Some(xftf);
    sp.fft().map_err(|e| e.to_string())?;

    Ok(sp)
}

/// Linearly resample the k-weighted chi(k) onto a fixed grid (0 outside the
/// data range), so operando frames share one heatmap axis.
pub fn resample_chik(sp: &XASSpectrum, grid: &[f64]) -> Option<Vec<f64>> {
    let k = sp.get_k()?;
    let chi = sp.get_chi_kweighted()?;
    if k.len() < 2 || k.len() != chi.len() {
        return None;
    }
    let mut out = Vec::with_capacity(grid.len());
    let mut j = 0usize;
    for &g in grid {
        if g < k[0] || g > k[k.len() - 1] {
            out.push(0.0);
            continue;
        }
        while j + 2 < k.len() && k[j + 1] < g {
            j += 1;
        }
        let (k0, k1) = (k[j], k[j + 1]);
        let t = if k1 > k0 { (g - k0) / (k1 - k0) } else { 0.0 };
        out.push(chi[j] + t * (chi[j + 1] - chi[j]));
    }
    Some(out)
}
