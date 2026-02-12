use serde::{Deserialize, Serialize};

// --- Load / Spectrum Metadata ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadResult {
    pub loaded: usize,
    pub errors: Vec<LoadError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadError {
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectrumMeta {
    pub index: usize,
    pub name: String,
    pub has_e0: bool,
    pub has_norm: bool,
    pub has_chi: bool,
    pub has_chir: bool,
}

// --- Spectrum Data ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectrumData {
    pub index: usize,
    pub name: String,
    pub energy: Option<Vec<f64>>,
    pub mu: Option<Vec<f64>>,
    pub e0: Option<f64>,
    pub norm: Option<Vec<f64>>,
    pub flat: Option<Vec<f64>>,
    pub k: Option<Vec<f64>>,
    pub chi: Option<Vec<f64>>,
    pub chi_kweighted: Option<Vec<f64>>,
    pub r: Option<Vec<f64>>,
    pub chir_mag: Option<Vec<f64>>,
    pub chir_re: Option<Vec<f64>>,
    pub chir_im: Option<Vec<f64>>,
    pub q: Option<Vec<f64>>,
    pub chiq: Option<Vec<f64>>,
    pub kwin: Option<Vec<f64>>,
    pub pre_edge: Option<Vec<f64>>,
    pub post_edge: Option<Vec<f64>>,
}

// --- Processing Options ---

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NormOptions {
    pub e0: Option<f64>,
    pub pre_edge_start: Option<f64>,
    pub pre_edge_end: Option<f64>,
    pub norm_start: Option<f64>,
    pub norm_end: Option<f64>,
    pub norm_polyorder: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BgOptions {
    pub rbkg: Option<f64>,
    pub kmin: Option<f64>,
    pub kmax: Option<f64>,
    pub kweight: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FFTOptions {
    pub kmin: Option<f64>,
    pub kmax: Option<f64>,
    pub kweight: Option<f64>,
    pub dk: Option<f64>,
    pub window: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PipelineOptions {
    pub norm: Option<NormOptions>,
    pub bg: Option<BgOptions>,
    pub fft: Option<FFTOptions>,
}

// --- Batch Processing ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchResult {
    pub succeeded: usize,
    pub failed: usize,
    pub errors: Vec<BatchError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchError {
    pub index: usize,
    pub name: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchProgressEvent {
    pub current: usize,
    pub total: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub index: usize,
    pub name: String,
}

// --- Plotting ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlotTrace {
    pub x: Vec<f64>,
    pub y: Vec<f64>,
    pub label: String,
    pub panel: String,
    /// If set, this trace is an overlay identified by this key (e.g. "preedge", "dmude").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overlay: Option<String>,
    /// Dash style hint: "solid", "dash", "dot", "dashdot". Default solid.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dash: Option<String>,
    /// Optional explicit color for overlay traces.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlotResult {
    pub traces: Vec<PlotTrace>,
    pub svgs: Vec<String>,
    pub x_label: String,
    pub y_label: String,
}

// --- Workspace ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceData {
    pub version: String,
    pub layout: Option<serde_json::Value>,
    pub tabs: Vec<serde_json::Value>,
    pub spectra_source: Option<String>,
    pub spectra_count: usize,
    pub processing: std::collections::HashMap<usize, ProcessingState>,
    pub fits: std::collections::HashMap<String, serde_json::Value>,
    pub plot_settings: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProcessingState {
    pub e0: Option<f64>,
    pub norm_options: Option<NormOptions>,
    pub bg_options: Option<BgOptions>,
    pub fft_options: Option<FFTOptions>,
}

// --- FEFF Fitting ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeffFitConfig {
    pub paths: Vec<FeffPathConfig>,
    pub variables: Vec<FitVariableConfig>,
    pub transform: FitTransformConfig,
    pub data_index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeffPathConfig {
    pub label: String,
    pub feff_dat_path: String,
    pub use_path: bool,
    pub s02: String,
    pub e0: String,
    pub deltar: String,
    pub sigma2: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FitVariableConfig {
    pub name: String,
    pub value: f64,
    pub vary: bool,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub expr: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FitTransformConfig {
    pub kmin: f64,
    pub kmax: f64,
    pub kweight: f64,
    pub dk: f64,
    pub rmin: f64,
    pub rmax: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeffFitResultDto {
    pub id: String,
    pub chi_square: f64,
    pub reduced_chi_square: f64,
    pub r_factor: f64,
    pub n_vary: usize,
    pub n_data: usize,
    pub n_idp: f64,
    pub variables: Vec<FitVariableResult>,
    pub correlation: Option<Vec<Vec<f64>>>,
    pub k: Vec<f64>,
    pub data_chi: Vec<f64>,
    pub model_chi: Vec<f64>,
    pub r: Vec<f64>,
    pub data_chir_re: Vec<f64>,
    pub data_chir_im: Vec<f64>,
    pub model_chir_re: Vec<f64>,
    pub model_chir_im: Vec<f64>,
    pub model_chir_mag: Vec<f64>,
    pub path_contributions: Vec<PathContributionDto>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FitVariableResult {
    pub name: String,
    pub value: f64,
    pub stderr: Option<f64>,
    pub vary: bool,
    pub init_value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathContributionDto {
    pub label: String,
    pub chi: Vec<f64>,
    pub chir_re: Vec<f64>,
    pub chir_im: Vec<f64>,
    pub chir_mag: Vec<f64>,
}
