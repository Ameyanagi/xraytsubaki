use std::collections::BTreeMap;
use std::path::PathBuf;

use nalgebra::DVector;
use serde::{Deserialize, Serialize};

use crate::xafs::xafsutils::FTWindow;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FeffFlavor {
    Feff85L,
    Feff10,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FitSpace {
    R,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FeffExecutionMode {
    Feff85LModules,
}

impl Default for FeffExecutionMode {
    fn default() -> Self {
        Self::Feff85LModules
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FeffRunRequest {
    pub executable_path: PathBuf,
    pub workspace_dir: PathBuf,
    pub feffinp: Option<PathBuf>,
    pub mode: FeffExecutionMode,
    pub timeout_sec: Option<u64>,
}

impl Default for FeffRunRequest {
    fn default() -> Self {
        Self {
            executable_path: PathBuf::new(),
            workspace_dir: PathBuf::new(),
            feffinp: None,
            mode: FeffExecutionMode::Feff85LModules,
            timeout_sec: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FeffModuleCommand {
    pub module: String,
    pub executable: PathBuf,
}

impl Default for FeffModuleCommand {
    fn default() -> Self {
        Self {
            module: String::new(),
            executable: PathBuf::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FeffResolvedCommands {
    pub mode: FeffExecutionMode,
    pub modules: Vec<FeffModuleCommand>,
}

impl Default for FeffResolvedCommands {
    fn default() -> Self {
        Self {
            mode: FeffExecutionMode::Feff85LModules,
            modules: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FeffRunResult {
    pub mode: FeffExecutionMode,
    pub workspace_dir: PathBuf,
    pub feffinp_path: PathBuf,
    pub resolved: FeffResolvedCommands,
    pub logs: Vec<PathBuf>,
    pub path_files: Vec<PathBuf>,
}

impl Default for FeffRunResult {
    fn default() -> Self {
        Self {
            mode: FeffExecutionMode::Feff85LModules,
            workspace_dir: PathBuf::new(),
            feffinp_path: PathBuf::new(),
            resolved: FeffResolvedCommands::default(),
            logs: Vec::new(),
            path_files: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PathParamSpec {
    Value(f64),
    Expression(String),
}

impl Default for PathParamSpec {
    fn default() -> Self {
        Self::Value(0.0)
    }
}

impl From<f64> for PathParamSpec {
    fn from(value: f64) -> Self {
        Self::Value(value)
    }
}

impl From<&str> for PathParamSpec {
    fn from(value: &str) -> Self {
        Self::Expression(value.to_string())
    }
}

impl From<String> for PathParamSpec {
    fn from(value: String) -> Self {
        Self::Expression(value)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FitVariable {
    pub value: f64,
    pub vary: bool,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub expr: Option<String>,
    pub stderr: Option<f64>,
    pub init_value: f64,
}

impl Default for FitVariable {
    fn default() -> Self {
        Self {
            value: 0.0,
            vary: false,
            min: None,
            max: None,
            expr: None,
            stderr: None,
            init_value: 0.0,
        }
    }
}

impl FitVariable {
    pub fn new(value: f64, vary: bool) -> Self {
        Self {
            value,
            vary,
            init_value: value,
            ..Self::default()
        }
    }

    pub fn with_bounds(mut self, min: Option<f64>, max: Option<f64>) -> Self {
        self.min = min;
        self.max = max;
        self
    }

    pub fn with_expr<S: Into<String>>(mut self, expr: S) -> Self {
        self.expr = Some(expr.into());
        self.vary = false;
        self
    }

    pub fn clamp(&self, value: f64) -> f64 {
        let mut out = value;
        if let Some(min) = self.min {
            out = out.max(min);
        }
        if let Some(max) = self.max {
            out = out.min(max);
        }
        out
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct FitVariables {
    pub vars: BTreeMap<String, FitVariable>,
}

impl FitVariables {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert<S: Into<String>>(&mut self, name: S, variable: FitVariable) -> &mut Self {
        self.vars.insert(name.into(), variable);
        self
    }

    pub fn get(&self, name: &str) -> Option<&FitVariable> {
        self.vars.get(name)
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut FitVariable> {
        self.vars.get_mut(name)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FeffDat {
    pub filename: String,
    pub title: String,
    pub version: String,
    pub absorber: Option<String>,
    pub shell: Option<String>,
    pub reff: f64,
    pub degen: f64,
    pub nleg: usize,
    pub k: DVector<f64>,
    pub real_phc: DVector<f64>,
    pub mag_feff: DVector<f64>,
    pub pha_feff: DVector<f64>,
    pub red_fact: DVector<f64>,
    pub lam: DVector<f64>,
    pub rep: DVector<f64>,
    pub pha: DVector<f64>,
    pub amp: DVector<f64>,
    pub geometry: Vec<String>,
}

impl Default for FeffDat {
    fn default() -> Self {
        Self {
            filename: String::new(),
            title: String::new(),
            version: String::new(),
            absorber: None,
            shell: None,
            reff: 0.0,
            degen: 1.0,
            nleg: 0,
            k: DVector::zeros(0),
            real_phc: DVector::zeros(0),
            mag_feff: DVector::zeros(0),
            pha_feff: DVector::zeros(0),
            red_fact: DVector::zeros(0),
            lam: DVector::zeros(0),
            rep: DVector::zeros(0),
            pha: DVector::zeros(0),
            amp: DVector::zeros(0),
            geometry: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FeffPathModel {
    pub label: String,
    pub feff: FeffDat,
    pub use_path: bool,
    pub degen: PathParamSpec,
    pub s02: PathParamSpec,
    pub e0: PathParamSpec,
    pub ei: PathParamSpec,
    pub deltar: PathParamSpec,
    pub sigma2: PathParamSpec,
    pub third: PathParamSpec,
    pub fourth: PathParamSpec,
}

impl Default for FeffPathModel {
    fn default() -> Self {
        Self {
            label: String::new(),
            feff: FeffDat::default(),
            use_path: true,
            degen: PathParamSpec::Value(1.0),
            s02: PathParamSpec::Value(1.0),
            e0: PathParamSpec::Value(0.0),
            ei: PathParamSpec::Value(0.0),
            deltar: PathParamSpec::Value(0.0),
            sigma2: PathParamSpec::Value(0.0),
            third: PathParamSpec::Value(0.0),
            fourth: PathParamSpec::Value(0.0),
        }
    }
}

impl FeffPathModel {
    pub fn from_feffdat<S: Into<String>>(label: S, feff: FeffDat) -> Self {
        let degen = feff.degen;
        Self {
            label: label.into(),
            degen: PathParamSpec::Value(degen),
            feff,
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FeffFitTransform {
    pub kmin: f64,
    pub kmax: f64,
    pub kweight: f64,
    pub dk: f64,
    pub dk2: Option<f64>,
    pub window: FTWindow,
    pub nfft: usize,
    pub kstep: Option<f64>,
    pub rmin: f64,
    pub rmax: f64,
    pub dr: f64,
    pub dr2: Option<f64>,
    pub rwindow: FTWindow,
    pub fitspace: FitSpace,
}

impl Default for FeffFitTransform {
    fn default() -> Self {
        Self {
            kmin: 0.0,
            kmax: 20.0,
            kweight: 2.0,
            dk: 4.0,
            dk2: None,
            window: FTWindow::KaiserBessel,
            nfft: 2048,
            kstep: Some(0.05),
            rmin: 1.0,
            rmax: 3.0,
            dr: 0.0,
            dr2: None,
            rwindow: FTWindow::Hanning,
            fitspace: FitSpace::R,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FeffFitDataset {
    pub k: DVector<f64>,
    pub chi: DVector<f64>,
    pub epsilon_k: Option<f64>,
    pub transform: FeffFitTransform,
    pub paths: Vec<FeffPathModel>,
}

impl Default for FeffFitDataset {
    fn default() -> Self {
        Self {
            k: DVector::zeros(0),
            chi: DVector::zeros(0),
            epsilon_k: None,
            transform: FeffFitTransform::default(),
            paths: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PathContribution {
    pub label: String,
    pub chi: DVector<f64>,
    pub chir_re: DVector<f64>,
    pub chir_im: DVector<f64>,
    pub chir_mag: DVector<f64>,
}

impl Default for PathContribution {
    fn default() -> Self {
        Self {
            label: String::new(),
            chi: DVector::zeros(0),
            chir_re: DVector::zeros(0),
            chir_im: DVector::zeros(0),
            chir_mag: DVector::zeros(0),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FeffFitResult {
    pub variables: FitVariables,
    pub n_vary: usize,
    pub n_data: usize,
    pub chi_square: f64,
    pub reduced_chi_square: f64,
    pub r_factor: f64,
    pub k: DVector<f64>,
    pub data_chi: DVector<f64>,
    pub model_chi: DVector<f64>,
    pub r: DVector<f64>,
    pub data_chir_re: DVector<f64>,
    pub data_chir_im: DVector<f64>,
    pub model_chir_re: DVector<f64>,
    pub model_chir_im: DVector<f64>,
    pub model_chir_mag: DVector<f64>,
    pub path_contributions: Vec<PathContribution>,
}

impl Default for FeffFitResult {
    fn default() -> Self {
        Self {
            variables: FitVariables::default(),
            n_vary: 0,
            n_data: 0,
            chi_square: 0.0,
            reduced_chi_square: 0.0,
            r_factor: 0.0,
            k: DVector::zeros(0),
            data_chi: DVector::zeros(0),
            model_chi: DVector::zeros(0),
            r: DVector::zeros(0),
            data_chir_re: DVector::zeros(0),
            data_chir_im: DVector::zeros(0),
            model_chir_re: DVector::zeros(0),
            model_chir_im: DVector::zeros(0),
            model_chir_mag: DVector::zeros(0),
            path_contributions: Vec::new(),
        }
    }
}
