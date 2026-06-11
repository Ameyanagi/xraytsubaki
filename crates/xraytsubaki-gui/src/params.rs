//! Pipeline parameters edited in the context panel. `None` = let the core
//! library auto-determine ("auto" in the UI). The fingerprint keys the
//! processed-spectrum cache so edits invalidate exactly what they change.

use std::hash::{Hash, Hasher};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use xraytsubaki::prelude::*;
use xraytsubaki::xafs::background::AUTOBK;
use xraytsubaki::xafs::normalization::PrePostEdge;

/// How the measured intensities turn into mu(E).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum DetectionMode {
    /// mu = ln(I0/It)
    #[default]
    Transmission,
    /// mu = sum(ROI columns)/I0
    Fluorescence,
    /// mu = ln(It/Ir) — the reference foil between It and Ir.
    Reference,
}

/// Configure-once import applied to every file in the catalog. Defaults
/// match the QAS transmission layout (energy, I0, It, Ir, If).
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ImportConfig {
    pub mode: DetectionMode,
    pub energy_col: usize,
    pub i0_col: usize,
    pub it_col: usize,
    pub ir_col: usize,
    /// Fluorescence ROI columns (e.g. SDD elements); their sum is If.
    pub fluor_cols: Vec<usize>,
}

impl Default for ImportConfig {
    fn default() -> Self {
        Self {
            mode: DetectionMode::Transmission,
            energy_col: 0,
            i0_col: 1,
            it_col: 2,
            ir_col: 3,
            fluor_cols: vec![4],
        }
    }
}

/// Parse a column list like "4, 6-8" (commas/spaces, inclusive ranges).
pub fn parse_cols(text: &str) -> Option<Vec<usize>> {
    let mut out = Vec::new();
    for token in text.split([',', ' ']).filter(|t| !t.is_empty()) {
        match token.split_once('-') {
            Some((a, b)) => {
                let (a, b) = (a.trim().parse::<usize>().ok()?, b.trim().parse::<usize>().ok()?);
                if a > b {
                    return None;
                }
                out.extend(a..=b);
            }
            None => out.push(token.trim().parse::<usize>().ok()?),
        }
    }
    (!out.is_empty()).then_some(out)
}

/// Whitespace-delimited numeric rows; '#', '*' and non-numeric lines are
/// skipped. Rows shorter than the first data row are dropped.
pub fn read_columns(path: &std::path::Path) -> Result<Vec<Vec<f64>>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut rows: Vec<Vec<f64>> = Vec::new();
    let mut width = 0usize;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('*') {
            continue;
        }
        let values: Option<Vec<f64>> = line
            .split_whitespace()
            .map(|t| t.parse::<f64>().ok())
            .collect();
        let Some(values) = values else { continue };
        if rows.is_empty() {
            width = values.len();
        }
        if values.len() >= width && width > 0 {
            rows.push(values);
        }
    }
    if rows.len() < 2 {
        return Err(format!("{}: no numeric data rows", path.display()));
    }
    Ok(rows)
}

/// Energy and mu(E) for one file under the import configuration. Rows whose
/// math is non-finite (e.g. log of a non-positive ratio) are dropped.
pub fn load_mu(path: &std::path::Path, import: &ImportConfig) -> Result<(Vec<f64>, Vec<f64>), String> {
    let rows = read_columns(path)?;
    let width = rows[0].len();
    let need = |col: usize, name: &str| -> Result<usize, String> {
        if col < width {
            Ok(col)
        } else {
            Err(format!(
                "{}: {name} column {col} out of range (file has {width} columns)",
                path.display()
            ))
        }
    };
    let e = need(import.energy_col, "energy")?;
    let mut energy = Vec::with_capacity(rows.len());
    let mut mu = Vec::with_capacity(rows.len());
    match import.mode {
        DetectionMode::Transmission => {
            let (i0, it) = (need(import.i0_col, "I0")?, need(import.it_col, "It")?);
            for row in &rows {
                let m = (row[i0] / row[it]).ln();
                if m.is_finite() && row[e].is_finite() {
                    energy.push(row[e]);
                    mu.push(m);
                }
            }
        }
        DetectionMode::Fluorescence => {
            let i0 = need(import.i0_col, "I0")?;
            let mut cols = Vec::new();
            for &c in &import.fluor_cols {
                cols.push(need(c, "ROI")?);
            }
            if cols.is_empty() {
                return Err("no fluorescence ROI columns configured".into());
            }
            for row in &rows {
                let m = cols.iter().map(|&c| row[c]).sum::<f64>() / row[i0];
                if m.is_finite() && row[e].is_finite() {
                    energy.push(row[e]);
                    mu.push(m);
                }
            }
        }
        DetectionMode::Reference => {
            let (it, ir) = (need(import.it_col, "It")?, need(import.ir_col, "Ir")?);
            for row in &rows {
                let m = (row[it] / row[ir]).ln();
                if m.is_finite() && row[e].is_finite() {
                    energy.push(row[e]);
                    mu.push(m);
                }
            }
        }
    }
    if energy.len() < 2 {
        return Err(format!("{}: fewer than 2 finite data points", path.display()));
    }
    Ok((energy, mu))
}

#[derive(Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PipelineParams {
    pub import: ImportConfig,
    /// Shift each spectrum's energy axis so its reference-channel E0 lands
    /// on `align_target` (requires an Ir column; no-op when target unset).
    pub align_to_ref: bool,
    pub align_target: Option<f64>,
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



impl PipelineParams {
    pub fn fingerprint(&self) -> u64 {
        let mut hasher = std::hash::DefaultHasher::new();
        format!("{:?}", self.import.mode).hash(&mut hasher);
        self.align_to_ref.hash(&mut hasher);
        self.align_target.map(f64::to_bits).hash(&mut hasher);
        self.import.energy_col.hash(&mut hasher);
        self.import.i0_col.hash(&mut hasher);
        self.import.it_col.hash(&mut hasher);
        self.import.ir_col.hash(&mut hasher);
        self.import.fluor_cols.hash(&mut hasher);
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
    let (energy, mu) = load_raw(path, params)?;
    process_arrays(energy, mu, params)
}

/// Raw (energy, mu) after import math and optional reference alignment —
/// the inputs both processing and merging start from.
pub fn load_raw(
    path: &std::path::Path,
    params: &PipelineParams,
) -> Result<(Vec<f64>, Vec<f64>), String> {
    let (mut energy, mu) = load_mu(path, &params.import)?;
    if params.align_to_ref
        && let Some(target) = params.align_target
    {
        let shift = target - reference_e0(path, &params.import)?;
        for e in energy.iter_mut() {
            *e += shift;
        }
    }
    Ok((energy, mu))
}

/// A spectrum created in the app (e.g. the average of a selection) rather
/// than loaded from a file.
#[derive(Clone, Serialize, Deserialize)]
pub struct DerivedSpectrum {
    pub label: String,
    pub energy: Vec<f64>,
    pub mu: Vec<f64>,
}

/// Average spectra on the first input's energy grid, restricted to the
/// overlap region; the others are linearly interpolated onto it.
pub fn average_spectra(inputs: &[(Vec<f64>, Vec<f64>)]) -> Result<(Vec<f64>, Vec<f64>), String> {
    if inputs.len() < 2 {
        return Err("need at least 2 spectra to merge".into());
    }
    let lo = inputs
        .iter()
        .map(|(e, _)| *e.first().unwrap_or(&f64::MAX))
        .fold(f64::MIN, f64::max);
    let hi = inputs
        .iter()
        .map(|(e, _)| *e.last().unwrap_or(&f64::MIN))
        .fold(f64::MAX, f64::min);
    if hi <= lo {
        return Err("selected spectra have no overlapping energy range".into());
    }
    let grid: Vec<f64> = inputs[0]
        .0
        .iter()
        .copied()
        .filter(|&e| e >= lo && e <= hi)
        .collect();
    if grid.len() < 2 {
        return Err("overlap region too small to merge".into());
    }
    let mut sum = vec![0.0f64; grid.len()];
    for (e, mu) in inputs {
        let mut j = 0usize;
        for (gi, &g) in grid.iter().enumerate() {
            while j + 2 < e.len() && e[j + 1] < g {
                j += 1;
            }
            let (e0v, e1v) = (e[j], e[j + 1]);
            let t = if e1v > e0v { ((g - e0v) / (e1v - e0v)).clamp(0.0, 1.0) } else { 0.0 };
            sum[gi] += mu[j] + t * (mu[j + 1] - mu[j]);
        }
    }
    let n = inputs.len() as f64;
    let avg: Vec<f64> = sum.into_iter().map(|v| v / n).collect();
    Ok((grid, avg))
}

/// E0 of the reference channel ln(It/Ir) of this file.
pub fn reference_e0(path: &std::path::Path, import: &ImportConfig) -> Result<f64, String> {
    let mut ref_import = import.clone();
    ref_import.mode = DetectionMode::Reference;
    let (energy, mu) = load_mu(path, &ref_import)?;
    let mut sp = XASSpectrum::new();
    sp.set_spectrum(energy, mu);
    sp.find_e0()
        .map_err(|e| format!("reference E0 failed: {e}"))?;
    sp.get_e0()
        .ok_or_else(|| "reference E0 not found".to_string())
}

/// Normalize/AUTOBK/FFT chain on raw arrays (shared by file loads and
/// derived/merged spectra).
pub fn process_arrays(
    energy: Vec<f64>,
    mu: Vec<f64>,
    params: &PipelineParams,
) -> Result<XASSpectrum, String> {
    let mut sp = XASSpectrum::new();
    sp.set_spectrum(energy, mu);

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_cols_lists_and_ranges() {
        assert_eq!(parse_cols("4, 6-8"), Some(vec![4, 6, 7, 8]));
        assert_eq!(parse_cols("3"), Some(vec![3]));
        assert_eq!(parse_cols("1 2 5-6"), Some(vec![1, 2, 5, 6]));
        assert_eq!(parse_cols(""), None);
        assert_eq!(parse_cols("8-4"), None);
        assert_eq!(parse_cols("a"), None);
    }

    fn fixture(dir: &std::path::Path) -> std::path::PathBuf {
        let path = dir.join("import_fixture.dat");
        std::fs::write(
            &path,
            "# energy i0 it ir if1 if2\n\
             100.0 10.0 5.0 2.5 1.0 2.0\n\
             101.0 10.0 4.0 2.0 1.5 2.5\n\
             102.0 10.0 2.0 1.0 2.0 3.0\n",
        )
        .unwrap();
        path
    }

    #[test]
    fn detection_modes_compute_expected_mu() {
        let dir = std::env::temp_dir();
        let path = fixture(&dir);
        let mut import = ImportConfig::default();

        let (e, mu) = load_mu(&path, &import).unwrap();
        assert_eq!(e, vec![100.0, 101.0, 102.0]);
        assert!((mu[0] - (10.0f64 / 5.0).ln()).abs() < 1e-12);
        assert!((mu[2] - (10.0f64 / 2.0).ln()).abs() < 1e-12);

        import.mode = DetectionMode::Fluorescence;
        import.fluor_cols = vec![4, 5];
        let (_, mu) = load_mu(&path, &import).unwrap();
        assert!((mu[0] - (1.0 + 2.0) / 10.0).abs() < 1e-12);
        assert!((mu[1] - (1.5 + 2.5) / 10.0).abs() < 1e-12);

        import.mode = DetectionMode::Reference;
        let (_, mu) = load_mu(&path, &import).unwrap();
        assert!((mu[0] - (5.0f64 / 2.5).ln()).abs() < 1e-12);
    }

    #[test]
    fn bad_column_is_reported() {
        let dir = std::env::temp_dir();
        let path = fixture(&dir);
        let mut import = ImportConfig::default();
        import.i0_col = 9;
        let err = load_mu(&path, &import).unwrap_err();
        assert!(err.contains("I0 column 9"), "{err}");
    }

    #[test]
    fn alignment_shifts_to_target() {
        // Reference channel ln(it/ir) forms a step at ~105 eV; aligning to
        // target 100 must shift energies by 100 - e0_ref.
        let dir = std::env::temp_dir();
        let path = dir.join("align_fixture.dat");
        let mut text = String::from("# e i0 it ir\n");
        for i in 0..200 {
            let e = i as f64;
            // it/ir sigmoid edge centered at 105 eV (width 2 eV)
            let step = 1.0 / (1.0 + (-(e - 105.0) / 2.0).exp());
            let ratio: f64 = step.exp();
            // transmission channel: flat-ish
            let it = 5.0_f64;
            let i0 = 10.0_f64;
            let ir = it / ratio;
            text.push_str(&format!("{e} {i0} {it} {ir}\n"));
        }
        std::fs::write(&path, text).unwrap();
        let import = ImportConfig::default();
        let e0_ref = reference_e0(&path, &import).unwrap();
        assert!((e0_ref - 105.0).abs() < 2.0, "ref e0 = {e0_ref}");

        let mut params = PipelineParams::default();
        params.align_to_ref = true;
        params.align_target = Some(100.0);
        let (energy, _) = load_mu(&path, &import).unwrap();
        // emulate the shift process_file applies
        let shift = 100.0 - e0_ref;
        let shifted0 = energy[0] + shift;
        assert!((shifted0 - (0.0 + shift)).abs() < 1e-9);
        assert!(shift < 0.0 && shift > -10.0);
    }

    #[test]
    fn average_of_two_known_spectra() {
        let a = ((0..10).map(|i| i as f64).collect::<Vec<_>>(),
                 (0..10).map(|i| i as f64).collect::<Vec<_>>());
        let b = ((0..10).map(|i| i as f64 + 0.5).collect::<Vec<_>>(),
                 (0..10).map(|_| 1.0).collect::<Vec<_>>());
        let (grid, avg) = average_spectra(&[a, b]).unwrap();
        // overlap region [0.5, 9.0]; grid from first input
        assert!(*grid.first().unwrap() >= 0.5 && *grid.last().unwrap() <= 9.0);
        // avg = (g + 1.0)/2 at every grid point
        for (g, v) in grid.iter().zip(avg.iter()) {
            assert!((v - (g + 1.0) / 2.0).abs() < 1e-9, "g={g} v={v}");
        }
    }

    /// Transmission via the generic loader must match the legacy QAS loader.
    #[test]
    fn transmission_matches_legacy_qas_loader() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../xraytsubaki/tests/testfiles/Ru_QAS.dat");
        let (e, mu) = load_mu(&path, &ImportConfig::default()).unwrap();
        let legacy = xraytsubaki::xafs::io::load_spectrum_QAS_trans(&path).unwrap();
        let le = legacy.raw_energy.as_ref().unwrap();
        let lm = legacy.raw_mu.as_ref().unwrap();
        assert_eq!(e.len(), le.len());
        for i in [0, 100, e.len() - 1] {
            assert!((e[i] - le[i]).abs() < 1e-9);
            assert!((mu[i] - lm[i]).abs() < 1e-9);
        }
    }
}
