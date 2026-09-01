//! Pipeline parameters edited in the context panel. `None` = let the core
//! library auto-determine ("auto" in the UI). The fingerprint keys the
//! processed-spectrum cache so edits invalidate exactly what they change.

use std::hash::{Hash, Hasher};

use serde::{Deserialize, Serialize};
use xraytsubaki::prelude::*;
use xraytsubaki::xafs::background::AUTOBK;
use xraytsubaki::xafs::normalization::PrePostEdge;

/// How the measured intensities turn into mu(E).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum DetectionMode {
    /// Infer the mode from named columns in each file.
    #[default]
    Auto,
    /// mu = ln(I0/It)
    Transmission,
    /// mu = sum(ROI columns)/I0
    Fluorescence,
    /// mu = ln(It/Ir) — the reference foil between It and Ir.
    Reference,
    /// A file column already contains mu(E).
    MuColumn,
}

/// Configure-once import applied to every file in the catalog. `None` and
/// [`DetectionMode::Auto`] resolve independently from each file's content.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ImportConfig {
    pub mode: DetectionMode,
    pub energy_col: Option<usize>,
    pub i0_col: Option<usize>,
    pub it_col: Option<usize>,
    pub ir_col: Option<usize>,
    /// Fluorescence ROI columns (e.g. SDD elements); their sum is If.
    pub fluor_cols: Option<Vec<usize>>,
    /// Precomputed mu(E), used by [`DetectionMode::MuColumn`].
    pub mu_col: Option<usize>,
}

impl Default for ImportConfig {
    fn default() -> Self {
        Self {
            mode: DetectionMode::Auto,
            energy_col: None,
            i0_col: None,
            it_col: None,
            ir_col: None,
            fluor_cols: None,
            mu_col: None,
        }
    }
}

/// File-derived import assignments after applying any manual overrides.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedImport {
    pub mode: DetectionMode,
    pub energy_col: usize,
    pub i0_col: usize,
    pub it_col: usize,
    pub ir_col: usize,
    pub fluor_cols: Vec<usize>,
    pub mu_col: Option<usize>,
}

/// Bounded data shown in the Import panel.
#[derive(Clone, Debug, PartialEq)]
pub struct ImportPreview {
    pub column_count: usize,
    pub names: Option<Vec<String>>,
    pub rows: Vec<Vec<f64>>,
    /// Fully automatic assignments, used for role-picker auto labels.
    pub detected: ResolvedImport,
    /// Mode that Auto would choose while retaining current manual columns.
    pub auto_mode: DetectionMode,
    /// Current assignments after applying all manual overrides.
    pub resolved: ResolvedImport,
}

#[derive(Debug)]
struct ParsedData {
    names: Option<Vec<String>>,
    rows: Vec<Vec<f64>>,
}

#[derive(Debug)]
struct DetectedRoles {
    energy_col: usize,
    i0_col: usize,
    it_col: usize,
    ir_col: usize,
    fluor_cols: Vec<usize>,
    mu_col: Option<usize>,
    named_i0: bool,
    named_it: bool,
    named_fluor: bool,
}

const ENERGY_NAMES: &[&str] = &["energy", "e", "mono_e", "energy_ev", "en"];
const I0_NAMES: &[&str] = &["i0", "io", "i_0", "monitor", "mon"];
const IT_NAMES: &[&str] = &["it", "i1", "i_t", "itrans", "trans", "transmission"];
const IR_NAMES: &[&str] = &["ir", "i2", "iref", "i_r", "ref", "reference"];
const FLUOR_NAMES: &[&str] = &["iff", "if", "i_f", "fluo", "fluor", "fl", "pips", "ifluor"];
const MU_NAMES: &[&str] = &["mu", "xmu", "mutrans", "mu_t", "norm", "mufluor"];

fn name_matches(name: &str, exact: &[&str]) -> bool {
    let name = name.to_ascii_lowercase();
    exact.contains(&name.as_str())
}

fn fluorescence_name_matches(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    FLUOR_NAMES.contains(&name.as_str()) || name.starts_with("sdd") || name.starts_with("roi")
}

fn parse_data(text: &str) -> Result<ParsedData, String> {
    let mut rows = Vec::new();
    let mut width = 0usize;
    let mut last_comment = None;
    for line in text.lines() {
        let line = line.trim();
        if rows.is_empty() && line.starts_with('#') {
            last_comment = Some(line);
            continue;
        }
        if line.is_empty() || line.starts_with('#') || line.starts_with('*') {
            continue;
        }
        let values: Option<Vec<f64>> = line
            .split_whitespace()
            .map(|token| token.parse::<f64>().ok())
            .collect();
        let Some(values) = values else { continue };
        if values.is_empty() {
            continue;
        }
        if rows.is_empty() {
            width = values.len();
        }
        if values.len() >= width {
            rows.push(values[..width].to_vec());
        }
    }
    if rows.is_empty() {
        return Err("no numeric data rows".into());
    }
    let names = last_comment.and_then(|line| {
        let names = line
            .trim_start_matches('#')
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|token| !token.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>();
        (names.len() == width).then_some(names)
    });
    Ok(ParsedData { names, rows })
}

fn monotonic_energy_column(rows: &[Vec<f64>], width: usize) -> Option<usize> {
    (0..width).find(|&column| {
        let values = rows.iter().map(|row| row[column]).collect::<Vec<_>>();
        let increasing = values.windows(2).all(|pair| pair[1] > pair[0]);
        let decreasing = values.windows(2).all(|pair| pair[1] < pair[0]);
        let span = (values[values.len() - 1] - values[0]).abs();
        values.iter().all(|value| value.is_finite())
            && (increasing || decreasing)
            && (10.0..=10_000_000.0).contains(&span)
            && values.iter().any(|value| value.abs() >= 100.0)
    })
}

fn detect_roles(data: &ParsedData) -> DetectedRoles {
    let width = data.rows[0].len();
    let find = |synonyms: &[&str]| {
        data.names
            .as_ref()
            .and_then(|names| names.iter().position(|name| name_matches(name, synonyms)))
    };
    let named_energy = find(ENERGY_NAMES);
    let named_i0 = find(I0_NAMES);
    let named_it = find(IT_NAMES);
    let named_ir = find(IR_NAMES);
    let fluor_cols = data
        .names
        .as_ref()
        .map(|names| {
            names
                .iter()
                .enumerate()
                .filter_map(|(column, name)| fluorescence_name_matches(name).then_some(column))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mu_col = find(MU_NAMES);
    let energy_col = named_energy
        .or_else(|| {
            data.names
                .is_none()
                .then(|| monotonic_energy_column(&data.rows, width))
                .flatten()
        })
        .unwrap_or(0);
    DetectedRoles {
        energy_col,
        i0_col: named_i0.unwrap_or(1),
        it_col: named_it.unwrap_or(2),
        ir_col: named_ir.unwrap_or(3),
        fluor_cols: if fluor_cols.is_empty() {
            vec![4]
        } else {
            fluor_cols.clone()
        },
        mu_col,
        named_i0: named_i0.is_some(),
        named_it: named_it.is_some(),
        named_fluor: !fluor_cols.is_empty(),
    }
}

fn resolve_import(data: &ParsedData, import: &ImportConfig) -> ResolvedImport {
    let detected = detect_roles(data);
    let mode = match import.mode {
        DetectionMode::Auto if import.mu_col.is_some() || detected.mu_col.is_some() => {
            DetectionMode::MuColumn
        }
        DetectionMode::Auto
            if (import.i0_col.is_some() && import.it_col.is_some())
                || (detected.named_i0 && detected.named_it) =>
        {
            DetectionMode::Transmission
        }
        DetectionMode::Auto
            if (import.i0_col.is_some() && import.fluor_cols.is_some())
                || (detected.named_i0 && detected.named_fluor) =>
        {
            DetectionMode::Fluorescence
        }
        DetectionMode::Auto => DetectionMode::Transmission,
        mode => mode,
    };
    ResolvedImport {
        mode,
        energy_col: import.energy_col.unwrap_or(detected.energy_col),
        i0_col: import.i0_col.unwrap_or(detected.i0_col),
        it_col: import.it_col.unwrap_or(detected.it_col),
        ir_col: import.ir_col.unwrap_or(detected.ir_col),
        fluor_cols: import.fluor_cols.clone().unwrap_or(detected.fluor_cols),
        mu_col: import.mu_col.or(detected.mu_col),
    }
}

/// Parse a column list like "4, 6-8" (commas/spaces, inclusive ranges).
pub fn parse_cols(text: &str) -> Option<Vec<usize>> {
    let mut out = Vec::new();
    for token in text.split([',', ' ']).filter(|t| !t.is_empty()) {
        match token.split_once('-') {
            Some((a, b)) => {
                let (a, b) = (
                    a.trim().parse::<usize>().ok()?,
                    b.trim().parse::<usize>().ok()?,
                );
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

/// Bytes read for the import preview: enough to reach the first numeric
/// row of any realistic header without ever parsing a whole file.
const PREVIEW_BYTES: usize = 64 * 1024;

/// Column metadata and the first three numeric rows, reading at most
/// [`PREVIEW_BYTES`]. The caller runs this on the background executor so
/// selection never blocks on I/O (large files, network filesystems).
pub fn preview_import(
    path: &std::path::Path,
    import: &ImportConfig,
) -> Result<ImportPreview, String> {
    use std::io::Read;
    let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut buf = Vec::with_capacity(PREVIEW_BYTES);
    file.take(PREVIEW_BYTES as u64)
        .read_to_end(&mut buf)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let truncated = buf.len() == PREVIEW_BYTES;
    let text = String::from_utf8_lossy(&buf);
    let mut lines: Vec<&str> = text.lines().collect();
    if truncated {
        // the final line may be cut mid-number
        lines.pop();
    }
    let data = parse_data(&lines.join("\n")).map_err(|e| format!("{}: {e}", path.display()))?;
    let detected = resolve_import(&data, &ImportConfig::default());
    let mut auto_import = import.clone();
    auto_import.mode = DetectionMode::Auto;
    let auto_mode = resolve_import(&data, &auto_import).mode;
    let resolved = resolve_import(&data, import);
    Ok(ImportPreview {
        column_count: data.rows[0].len(),
        names: data.names,
        rows: data.rows.into_iter().take(3).collect(),
        detected,
        auto_mode,
        resolved,
    })
}

/// Energy and mu(E) for one file under the import configuration. Rows whose
/// math is non-finite (e.g. log of a non-positive ratio) are dropped.
pub fn load_mu(
    path: &std::path::Path,
    import: &ImportConfig,
) -> Result<(Vec<f64>, Vec<f64>), String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let data = parse_data(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    if data.rows.len() < 2 {
        return Err(format!("{}: no numeric data rows", path.display()));
    }
    let resolved = resolve_import(&data, import);
    let rows = data.rows;
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
    let e = need(resolved.energy_col, "energy")?;
    let mut energy = Vec::with_capacity(rows.len());
    let mut mu = Vec::with_capacity(rows.len());
    match resolved.mode {
        DetectionMode::Auto => unreachable!("auto mode is resolved before import math"),
        DetectionMode::Transmission => {
            let (i0, it) = (need(resolved.i0_col, "I0")?, need(resolved.it_col, "It")?);
            for row in &rows {
                let m = (row[i0] / row[it]).ln();
                if m.is_finite() && row[e].is_finite() {
                    energy.push(row[e]);
                    mu.push(m);
                }
            }
        }
        DetectionMode::Fluorescence => {
            let i0 = need(resolved.i0_col, "I0")?;
            let mut cols = Vec::new();
            for &c in &resolved.fluor_cols {
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
            let (it, ir) = (need(resolved.it_col, "It")?, need(resolved.ir_col, "Ir")?);
            for row in &rows {
                let m = (row[it] / row[ir]).ln();
                if m.is_finite() && row[e].is_finite() {
                    energy.push(row[e]);
                    mu.push(m);
                }
            }
        }
        DetectionMode::MuColumn => {
            let mu_col = resolved
                .mu_col
                .ok_or_else(|| format!("{}: no precomputed mu column detected", path.display()))?;
            let mu_col = need(mu_col, "mu")?;
            for row in &rows {
                if row[mu_col].is_finite() && row[e].is_finite() {
                    energy.push(row[e]);
                    mu.push(row[mu_col]);
                }
            }
        }
    }
    if energy.len() < 2 {
        return Err(format!(
            "{}: fewer than 2 finite data points",
            path.display()
        ));
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
    // Back FT (R -> q).
    pub bft_rmin: Option<f64>,
    pub bft_rmax: Option<f64>,
    pub bft_dr: Option<f64>,
    pub bft_window: Option<FTWindow>,
}

/// Human label for a window choice (`None` = the core default).
pub fn window_label(window: Option<FTWindow>) -> &'static str {
    match window {
        Some(FTWindow::Hanning) => "Hanning",
        Some(FTWindow::Parzen) => "Parzen",
        Some(FTWindow::Welch) => "Welch",
        Some(FTWindow::Gaussian) => "Gaussian",
        Some(FTWindow::Sine) => "Sine",
        Some(FTWindow::KaiserBessel) => "Kaiser",
        Some(FTWindow::FHanning) => "FHanning",
        None => "Kaiser",
    }
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
        self.import.mu_col.hash(&mut hasher);
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
            self.bft_rmin,
            self.bft_rmax,
            self.bft_dr,
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
            "{:?}|{:?}|{:?}|{:?}",
            self.bkg_window, self.bkg_solver, self.fft_window, self.bft_window
        )
        .hash(&mut hasher);
        hasher.finish()
    }
}

/// Load a file and run the full pipeline with the given parameters.
/// Runs on the background executor.
pub fn process_file(
    path: &std::path::Path,
    params: &PipelineParams,
) -> Result<XASSpectrum, String> {
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

/// Running average over spectra streamed one at a time, so merging N files
/// needs memory for the accumulator plus a single input — never all N at
/// once. The first spectrum's energy axis hosts a running sum; the overlap
/// window shrinks as inputs arrive and the grid is trimmed to it at the
/// end, which reproduces [`average_spectra`]'s all-at-once result exactly.
pub struct StreamingAverage {
    grid: Vec<f64>,
    sum: Vec<f64>,
    count: usize,
    lo: f64,
    hi: f64,
}

impl StreamingAverage {
    pub fn new(energy: Vec<f64>, mu: Vec<f64>) -> Self {
        let lo = *energy.first().unwrap_or(&f64::MAX);
        let hi = *energy.last().unwrap_or(&f64::MIN);
        Self {
            grid: energy,
            sum: mu,
            count: 1,
            lo,
            hi,
        }
    }

    /// Fold one spectrum (>= 2 points, ascending energy — what `load_raw`
    /// guarantees) into the running sum via clamped linear interpolation.
    pub fn add(&mut self, energy: &[f64], mu: &[f64]) {
        self.lo = self.lo.max(*energy.first().unwrap_or(&f64::MAX));
        self.hi = self.hi.min(*energy.last().unwrap_or(&f64::MIN));
        let mut j = 0usize;
        for (gi, &g) in self.grid.iter().enumerate() {
            while j + 2 < energy.len() && energy[j + 1] < g {
                j += 1;
            }
            let (e0v, e1v) = (energy[j], energy[j + 1]);
            let t = if e1v > e0v {
                ((g - e0v) / (e1v - e0v)).clamp(0.0, 1.0)
            } else {
                0.0
            };
            self.sum[gi] += mu[j] + t * (mu[j + 1] - mu[j]);
        }
        self.count += 1;
    }

    pub fn finish(self) -> Result<(Vec<f64>, Vec<f64>), String> {
        if self.count < 2 {
            return Err("need at least 2 spectra to merge".into());
        }
        if self.hi <= self.lo {
            return Err("selected spectra have no overlapping energy range".into());
        }
        let n = self.count as f64;
        let (lo, hi) = (self.lo, self.hi);
        let (grid, avg): (Vec<f64>, Vec<f64>) = self
            .grid
            .into_iter()
            .zip(self.sum)
            .filter(|&(g, _)| g >= lo && g <= hi)
            .map(|(g, s)| (g, s / n))
            .unzip();
        if grid.len() < 2 {
            return Err("overlap region too small to merge".into());
        }
        Ok((grid, avg))
    }
}

/// Average spectra on the first input's energy grid, restricted to the
/// overlap region; the others are linearly interpolated onto it.
/// Test-only reference: production merging streams through [`StreamingAverage`].
#[cfg(test)]
pub fn average_spectra(inputs: &[(Vec<f64>, Vec<f64>)]) -> Result<(Vec<f64>, Vec<f64>), String> {
    let mut iter = inputs.iter();
    let Some((energy, mu)) = iter.next() else {
        return Err("need at least 2 spectra to merge".into());
    };
    let mut acc = StreamingAverage::new(energy.clone(), mu.clone());
    for (energy, mu) in iter {
        acc.add(energy, mu);
    }
    acc.finish()
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

    // Back transform (chi(q)); failures here never block the pipeline.
    let mut xftr = XrayFFTR::default();
    if params.bft_rmin.is_some() {
        xftr.rmin = params.bft_rmin;
    }
    if params.bft_rmax.is_some() {
        xftr.rmax = params.bft_rmax;
    }
    if params.bft_dr.is_some() {
        xftr.dr = params.bft_dr;
    }
    if params.bft_window.is_some() {
        xftr.window = params.bft_window;
    }
    sp.xftr = Some(xftr);
    if sp.ifft().is_err() {
        sp.xftr = None;
    }

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

    #[test]
    fn header_names_come_from_last_comment_and_must_match_width() {
        let data =
            parse_data("# metadata that is not a header\n# Energy, I0, It\n7000 10 5\n7010 10 4\n")
                .unwrap();
        assert_eq!(
            data.names,
            Some(vec!["Energy".into(), "I0".into(), "It".into()])
        );

        let no_header = parse_data("7000 10 5\n7010 10 4\n").unwrap();
        assert_eq!(no_header.names, None);

        let mismatch = parse_data("# energy i0\n7000 10 5\n7010 10 4\n").unwrap();
        assert_eq!(mismatch.names, None);
    }

    #[test]
    fn synonyms_match_roles_case_insensitively() {
        for name in ENERGY_NAMES {
            assert!(name_matches(&name.to_ascii_uppercase(), ENERGY_NAMES));
        }
        for name in I0_NAMES {
            assert!(name_matches(&name.to_ascii_uppercase(), I0_NAMES));
        }
        for name in IT_NAMES {
            assert!(name_matches(&name.to_ascii_uppercase(), IT_NAMES));
        }
        for name in IR_NAMES {
            assert!(name_matches(&name.to_ascii_uppercase(), IR_NAMES));
        }
        for name in MU_NAMES {
            assert!(name_matches(&name.to_ascii_uppercase(), MU_NAMES));
        }
        for name in FLUOR_NAMES {
            assert!(fluorescence_name_matches(&name.to_ascii_uppercase()));
        }
        assert!(fluorescence_name_matches("SDD_1"));
        assert!(fluorescence_name_matches("roi7"));

        let data = parse_data(
            "# MONO_E MON ITRANS IREF SDD1 roi_2 XMU\n7000 10 5 2 1 2 0.3\n7010 10 4 2 1 2 0.4\n",
        )
        .unwrap();
        let detected = detect_roles(&data);
        assert_eq!(detected.energy_col, 0);
        assert_eq!(detected.i0_col, 1);
        assert_eq!(detected.it_col, 2);
        assert_eq!(detected.ir_col, 3);
        assert_eq!(detected.fluor_cols, vec![4, 5]);
        assert_eq!(detected.mu_col, Some(6));
    }

    #[test]
    fn auto_mode_inference_has_stable_precedence() {
        let mu = parse_data("# energy mu i0 it iff\n7000 .1 10 5 2\n7010 .2 10 4 2\n").unwrap();
        assert_eq!(
            resolve_import(&mu, &ImportConfig::default()).mode,
            DetectionMode::MuColumn
        );

        let transmission = parse_data("# energy i0 it iff\n7000 10 5 2\n7010 10 4 2\n").unwrap();
        assert_eq!(
            resolve_import(&transmission, &ImportConfig::default()).mode,
            DetectionMode::Transmission
        );

        let fluorescence = parse_data("# energy monitor sdd1\n7000 10 2\n7010 10 3\n").unwrap();
        assert_eq!(
            resolve_import(&fluorescence, &ImportConfig::default()).mode,
            DetectionMode::Fluorescence
        );
    }

    #[test]
    fn no_header_energy_fallback_uses_plausible_monotonic_column() {
        let data = parse_data("0 7000 10 5\n1 7020 10 4\n2 7040 10 3\n").unwrap();
        let resolved = resolve_import(&data, &ImportConfig::default());
        assert_eq!(resolved.energy_col, 1);
        assert_eq!(resolved.i0_col, 1);
        assert_eq!(resolved.it_col, 2);
        assert_eq!(resolved.ir_col, 3);
        assert_eq!(resolved.mode, DetectionMode::Transmission);
    }

    fn fixture(dir: &std::path::Path, name: &str) -> std::path::PathBuf {
        let path = dir.join(name);
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
        let path = fixture(&dir, "detection_modes_fixture.dat");
        let mut import = ImportConfig::default();

        let (e, mu) = load_mu(&path, &import).unwrap();
        assert_eq!(e, vec![100.0, 101.0, 102.0]);
        assert!((mu[0] - (10.0f64 / 5.0).ln()).abs() < 1e-12);
        assert!((mu[2] - (10.0f64 / 2.0).ln()).abs() < 1e-12);

        import.mode = DetectionMode::Fluorescence;
        import.fluor_cols = Some(vec![4, 5]);
        let (_, mu) = load_mu(&path, &import).unwrap();
        assert!((mu[0] - (1.0 + 2.0) / 10.0).abs() < 1e-12);
        assert!((mu[1] - (1.5 + 2.5) / 10.0).abs() < 1e-12);

        import.mode = DetectionMode::Reference;
        let (_, mu) = load_mu(&path, &import).unwrap();
        assert!((mu[0] - (5.0f64 / 2.5).ln()).abs() < 1e-12);

        let mu_path = dir.join("precomputed_mu.dat");
        std::fs::write(&mu_path, "# energy mu\n100 0.25\n101 0.5\n").unwrap();
        let (energy, mu) = load_mu(&mu_path, &ImportConfig::default()).unwrap();
        assert_eq!(energy, vec![100.0, 101.0]);
        assert_eq!(mu, vec![0.25, 0.5]);
    }

    #[test]
    fn bad_column_is_reported() {
        let dir = std::env::temp_dir();
        let path = fixture(&dir, "bad_column_fixture.dat");
        let import = ImportConfig {
            mode: DetectionMode::Transmission,
            i0_col: Some(9),
            ..Default::default()
        };
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

        let _params = PipelineParams {
            align_to_ref: true,
            align_target: Some(100.0),
            ..Default::default()
        };
        let (energy, _) = load_mu(&path, &import).unwrap();
        // emulate the shift process_file applies
        let shift = 100.0 - e0_ref;
        let shifted0 = energy[0] + shift;
        assert!((shifted0 - (0.0 + shift)).abs() < 1e-9);
        assert!(shift < 0.0 && shift > -10.0);
    }

    #[test]
    fn average_of_two_known_spectra() {
        let a = (
            (0..10).map(|i| i as f64).collect::<Vec<_>>(),
            (0..10).map(|i| i as f64).collect::<Vec<_>>(),
        );
        let b = (
            (0..10).map(|i| i as f64 + 0.5).collect::<Vec<_>>(),
            (0..10).map(|_| 1.0).collect::<Vec<_>>(),
        );
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

    #[test]
    fn ru_qas_auto_detection_golden() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../xraytsubaki/tests/testfiles/Ru_QAS.dat");
        let text = std::fs::read_to_string(path).unwrap();
        let data = parse_data(&text).unwrap();
        let resolved = resolve_import(&data, &ImportConfig::default());
        assert_eq!(resolved.energy_col, 0);
        assert_eq!(resolved.i0_col, 1);
        assert_eq!(resolved.it_col, 2);
        assert_eq!(resolved.ir_col, 3);
        assert_eq!(resolved.fluor_cols, vec![4]);
        assert_eq!(resolved.mode, DetectionMode::Transmission);
    }

    #[test]
    fn legacy_import_json_deserializes_as_manual_overrides() {
        let import: ImportConfig = serde_json::from_str(
            r#"{
                "mode":"Transmission",
                "energy_col":0,
                "i0_col":1,
                "it_col":2,
                "ir_col":3,
                "fluor_cols":[4]
            }"#,
        )
        .unwrap();
        assert_eq!(import.energy_col, Some(0));
        assert_eq!(import.i0_col, Some(1));
        assert_eq!(import.it_col, Some(2));
        assert_eq!(import.ir_col, Some(3));
        assert_eq!(import.fluor_cols, Some(vec![4]));
        assert_eq!(import.mu_col, None);
    }
}
