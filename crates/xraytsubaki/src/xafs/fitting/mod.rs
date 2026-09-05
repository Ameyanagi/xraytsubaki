pub mod builder;
pub mod errors;
pub mod expression;
pub mod feffdat;
pub mod path_model;
pub mod runner;
pub mod solver;
pub mod template;
pub mod transform;
pub mod types;
pub mod variables;

use nalgebra::DVector;

pub use builder::FeffFit;
pub use errors::FittingError;
pub use path_model::FF2ChiOutput;
pub use template::{
    apply_template, ParameterTemplate, PathAssignment, TemplateResult, TemplateVariable,
};
pub use transform::{estimate_noise, NoiseEstimate};
pub use types::{
    DatasetResult, FeffBatchExecutionStrategy, FeffBatchOptions, FeffDat, FeffExecutionMode,
    FeffFitDataset, FeffFitJacobianMode, FeffFitOptions, FeffFitResult, FeffFitSolverMethod,
    FeffFitTransform, FeffFlavor, FeffModuleCommand, FeffPathModel, FeffResolvedCommands,
    FeffRunRequest, FeffRunResult, FitSolverReport, FitSpace, FitVariable, FitVariables,
    FitWarning, KweightResult, Param, PathContribution, PathParamSpec,
};

use crate::xafs::{Result, XAFSError};

pub fn parse_feff_path_file(path: &str, flavor: FeffFlavor) -> Result<FeffDat> {
    feffdat::parse_feff_path_file(path, flavor).map_err(XAFSError::from)
}

pub fn feffpath(path: &str, flavor: FeffFlavor) -> Result<FeffPathModel> {
    path_model::feffpath(path, flavor).map_err(XAFSError::from)
}

pub fn path2chi(
    path: &FeffPathModel,
    vars: &FitVariables,
    k: &DVector<f64>,
) -> Result<DVector<f64>> {
    path_model::path2chi(path, vars, k).map_err(XAFSError::from)
}

pub fn ff2chi(
    paths: &[FeffPathModel],
    vars: &FitVariables,
    k: &DVector<f64>,
) -> Result<FF2ChiOutput> {
    path_model::ff2chi(paths, vars, k).map_err(XAFSError::from)
}

pub fn feffit_joint(datasets: &[FeffFitDataset], vars: &FitVariables) -> Result<FeffFitResult> {
    solver::feffit_joint(datasets, vars).map_err(XAFSError::from)
}

pub fn feffit_joint_with_options(
    datasets: &[FeffFitDataset],
    vars: &FitVariables,
    options: &FeffFitOptions,
) -> Result<FeffFitResult> {
    solver::feffit_joint_with_options(datasets, vars, options).map_err(XAFSError::from)
}

pub fn feffit_independent(
    datasets: &[FeffFitDataset],
    vars: &FitVariables,
    options: &FeffBatchOptions,
) -> Vec<std::result::Result<FeffFitResult, FittingError>> {
    solver::feffit_independent(datasets, vars, options)
}

pub fn resolve_feff_commands(request: &FeffRunRequest) -> Result<FeffResolvedCommands> {
    runner::resolve_feff_commands(request).map_err(XAFSError::from)
}

pub fn run_feff(request: &FeffRunRequest) -> Result<FeffRunResult> {
    runner::run_feff(request).map_err(XAFSError::from)
}

pub fn run_feff_and_load_paths(
    request: &FeffRunRequest,
    flavor: FeffFlavor,
) -> Result<Vec<FeffPathModel>> {
    runner::run_feff_and_load_paths(request, flavor).map_err(XAFSError::from)
}
