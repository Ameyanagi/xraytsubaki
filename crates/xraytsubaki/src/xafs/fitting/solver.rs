use std::collections::BTreeSet;
#[cfg(feature = "trust-region")]
use std::collections::HashMap;

#[cfg(feature = "trust-region")]
use apex_solver::core::problem::{Problem as ApexProblem, VariableEnum};
#[cfg(feature = "trust-region")]
use apex_solver::factors::Factor as ApexFactor;
#[cfg(feature = "trust-region")]
use apex_solver::linalg::{JacobianMode as ApexJacobianMode, LinearSolverType};
#[cfg(feature = "trust-region")]
use apex_solver::manifold::ManifoldType;
#[cfg(feature = "trust-region")]
use apex_solver::optimizer::dog_leg::{DogLeg, DogLegConfig};
use levenberg_marquardt::{LeastSquaresProblem, LevenbergMarquardt};
use nalgebra::{DMatrix, DVector, Dyn, Owned};
#[cfg(feature = "trust-region")]
use nalgebra_apex::{DMatrix as ApexDMatrix, DVector as ApexDVector};
use rayon::prelude::*;

use super::errors::FittingError;
use super::path_model::ff2chi;
use super::transform::{
    apply_r_transform, compute_n_idp, data_residual_in_r_space, residual_in_r_space,
    validate_transform, TransformOutput,
};
#[cfg(feature = "trust-region")]
use super::types::FeffFitJacobianMode;
use super::types::{
    DatasetResult, FeffBatchExecutionStrategy, FeffBatchOptions, FeffFitDataset, FeffFitOptions,
    FeffFitResult, FeffFitSolverMethod, FitVariables, PathContribution, PathParamSpec,
};
use super::variables::try_extract_symbols;

fn feffit(
    dataset: &FeffFitDataset,
    variables: &FitVariables,
    options: &FeffFitOptions,
) -> Result<FeffFitResult, FittingError> {
    feffit_joint_with_options(std::slice::from_ref(dataset), variables, options)
}

fn matrix_to_nested(matrix: &DMatrix<f64>) -> Vec<Vec<f64>> {
    let nrows = matrix.nrows();
    let ncols = matrix.ncols();
    (0..nrows)
        .map(|r| (0..ncols).map(|c| matrix[(r, c)]).collect::<Vec<_>>())
        .collect::<Vec<_>>()
}

pub fn feffit_joint(
    datasets: &[FeffFitDataset],
    variables: &FitVariables,
) -> Result<FeffFitResult, FittingError> {
    feffit_joint_with_options(datasets, variables, &FeffFitOptions::default())
}

pub fn feffit_joint_with_options(
    datasets: &[FeffFitDataset],
    variables: &FitVariables,
    options: &FeffFitOptions,
) -> Result<FeffFitResult, FittingError> {
    if datasets.is_empty() {
        return Err(FittingError::InvalidDataset {
            reason: "fit requires at least one dataset".to_string(),
        });
    }
    for dataset in datasets {
        validate_dataset(dataset)?;
    }

    let varying_names = variables.varying_names();
    if varying_names.is_empty() {
        return Err(FittingError::NoVaryingVariables);
    }

    let problem = FeffFitMultiProblem::new(datasets.to_vec(), variables.clone(), varying_names)?;
    let solved = solve_fit_problem(problem, options)?;
    let residual = solved.current_residual()?;

    let mut solved_variables = solved.variables.clone();
    let raw_chi_square = residual.dot(&residual);
    let n_data = residual.len();
    let n_vary = solved.variable_names.len();
    let global_n_idp = solved
        .datasets
        .iter()
        .map(|dataset| compute_n_idp(&dataset.transform))
        .sum::<f64>();
    let n_idp_dof = (global_n_idp - n_vary as f64).max(1.0e-12);
    let chi_square = raw_chi_square * global_n_idp / n_data.max(1) as f64;
    let reduced_chi_square = chi_square / n_idp_dof;

    let mut covariance: Option<Vec<Vec<f64>>> = None;
    let mut correlation: Option<Vec<Vec<f64>>> = None;
    if let Some(covar) = solved.approx_covariance() {
        // Keep covariance scaling on the raw LM residual convention to match Larch stderr
        // behavior. Reported chi-square statistics apply additional presentation scaling.
        let error_scale = raw_chi_square / n_idp_dof;
        let scaled_covar = covar * error_scale;
        for (idx, name) in solved.variable_names.iter().enumerate() {
            if let Some(var) = solved_variables.get_mut(name) {
                let variance = scaled_covar[(idx, idx)].max(0.0);
                var.stderr = Some(variance.sqrt());
            }
        }

        let mut corr = DMatrix::<f64>::zeros(scaled_covar.nrows(), scaled_covar.ncols());
        for i in 0..scaled_covar.nrows() {
            for j in 0..scaled_covar.ncols() {
                let denom = (scaled_covar[(i, i)].max(0.0) * scaled_covar[(j, j)].max(0.0)).sqrt();
                corr[(i, j)] = if denom > 0.0 {
                    scaled_covar[(i, j)] / denom
                } else if i == j {
                    1.0
                } else {
                    0.0
                };
            }
        }
        covariance = Some(matrix_to_nested(&scaled_covar));
        correlation = Some(matrix_to_nested(&corr));
    }

    let model_evaluations = solved.evaluate_models(&solved.variables)?;
    let mut datasets_out = Vec::with_capacity(solved.datasets.len());

    let mut global_data_norm = 0.0;
    let mut global_model_diff_norm = 0.0;

    for ((dataset, data_transform), model_eval) in solved
        .datasets
        .iter()
        .zip(solved.data_transforms.iter())
        .zip(model_evaluations)
    {
        let ds_residual = residual_in_r_space(
            data_transform,
            &model_eval.model_transform,
            &dataset.transform,
            dataset.epsilon_k,
        )?;
        let ds_data_residual =
            data_residual_in_r_space(data_transform, &dataset.transform, dataset.epsilon_k)?;
        let ds_raw_chi_square = ds_residual.dot(&ds_residual);
        let ds_n_data = ds_residual.len();
        let ds_n_vary = dataset_vary_count(dataset, &solved.variables, &solved.variable_names)?;
        let ds_n_idp = compute_n_idp(&dataset.transform);
        let ds_n_idp_dof = (ds_n_idp - ds_n_vary as f64).max(1.0e-12);
        let ds_chi_square = ds_raw_chi_square * ds_n_idp / ds_n_data.max(1) as f64;
        let ds_reduced_chi_square = ds_chi_square / ds_n_idp_dof;

        let data_norm = ds_data_residual.dot(&ds_data_residual);
        let model_diff_norm = ds_raw_chi_square;
        global_data_norm += data_norm;
        global_model_diff_norm += model_diff_norm;
        let ds_r_factor = if data_norm.abs() < f64::EPSILON {
            0.0
        } else {
            model_diff_norm / data_norm
        };

        let path_contributions = model_eval
            .path_chi
            .iter()
            .filter_map(|(label, chi)| {
                let transformed = apply_r_transform(&dataset.k, chi, &dataset.transform).ok()?;
                Some(PathContribution {
                    label: label.clone(),
                    chi: chi.clone(),
                    chir_re: transformed.chir_re,
                    chir_im: transformed.chir_im,
                    chir_mag: transformed.chir_mag,
                })
            })
            .collect::<Vec<_>>();

        datasets_out.push(DatasetResult {
            n_data: ds_n_data,
            chi_square: ds_chi_square,
            reduced_chi_square: ds_reduced_chi_square,
            r_factor: ds_r_factor,
            n_idp: ds_n_idp,
            k: dataset.k.clone(),
            data_chi: dataset.chi.clone(),
            model_chi: model_eval.model_chi,
            kweight: dataset.transform.kweight,
            kmin: Some(dataset.transform.kmin),
            kmax: Some(dataset.transform.kmax),
            kwin: data_transform.kwin.clone(),
            r: model_eval.model_transform.r.clone(),
            rmin: Some(dataset.transform.rmin),
            rmax: Some(dataset.transform.rmax),
            data_chir_re: data_transform.chir_re.clone(),
            data_chir_im: data_transform.chir_im.clone(),
            model_chir_re: model_eval.model_transform.chir_re,
            model_chir_im: model_eval.model_transform.chir_im,
            model_chir_mag: model_eval.model_transform.chir_mag,
            path_contributions,
        });
    }

    let r_factor = if global_data_norm.abs() < f64::EPSILON {
        0.0
    } else {
        global_model_diff_norm / global_data_norm
    };

    let mut out = FeffFitResult {
        variables: solved_variables,
        varying_names: solved.variable_names.clone(),
        n_vary,
        n_data,
        chi_square,
        reduced_chi_square,
        r_factor,
        covariance,
        correlation,
        datasets: datasets_out,
        n_idp: global_n_idp,
        ..FeffFitResult::default()
    };
    out.sync_primary_dataset_fields();
    Ok(out)
}

fn solve_fit_problem(
    problem: FeffFitMultiProblem,
    options: &FeffFitOptions,
) -> Result<FeffFitMultiProblem, FittingError> {
    match options.solver_method {
        FeffFitSolverMethod::LevenbergMarquardt => {
            let (solved, _report) = LevenbergMarquardt::new().minimize(problem);
            Ok(solved)
        }
        FeffFitSolverMethod::TrustRegionDogLeg => solve_fit_problem_apex(problem, options),
    }
}

fn dataset_vary_count(
    dataset: &FeffFitDataset,
    variables: &FitVariables,
    varying_names: &[String],
) -> Result<usize, FittingError> {
    Ok(dataset_varying_names(dataset, variables, varying_names)?.len())
}

fn dataset_varying_names(
    dataset: &FeffFitDataset,
    variables: &FitVariables,
    varying_names: &[String],
) -> Result<Vec<String>, FittingError> {
    let varying = varying_names
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let mut referenced = BTreeSet::<String>::new();

    for path in dataset.paths.iter().filter(|path| path.use_path) {
        for spec in [
            &path.degen,
            &path.s02,
            &path.e0,
            &path.ei,
            &path.deltar,
            &path.sigma2,
            &path.third,
            &path.fourth,
        ] {
            if let PathParamSpec::Expression(expr) = spec {
                for symbol in try_extract_symbols(expr)? {
                    referenced.insert(symbol);
                }
            }
        }
    }

    let mut active = BTreeSet::<String>::new();
    let mut visiting = BTreeSet::<String>::new();

    fn walk_symbol(
        symbol: &str,
        varying: &BTreeSet<&str>,
        variables: &FitVariables,
        active: &mut BTreeSet<String>,
        visiting: &mut BTreeSet<String>,
    ) -> Result<(), FittingError> {
        if varying.contains(symbol) {
            active.insert(symbol.to_string());
            return Ok(());
        }
        if !visiting.insert(symbol.to_string()) {
            return Ok(());
        }

        if let Some(expr) = variables
            .vars
            .get(symbol)
            .and_then(|variable| variable.expr.as_ref())
        {
            for dependency in try_extract_symbols(expr)? {
                walk_symbol(&dependency, varying, variables, active, visiting)?;
            }
        }

        visiting.remove(symbol);
        Ok(())
    }

    for symbol in referenced {
        walk_symbol(&symbol, &varying, variables, &mut active, &mut visiting)?;
    }

    Ok(varying_names
        .iter()
        .filter(|name| active.contains(*name))
        .cloned()
        .collect())
}

pub fn feffit_independent(
    datasets: &[FeffFitDataset],
    variables: &FitVariables,
    options: &FeffBatchOptions,
) -> Vec<Result<FeffFitResult, FittingError>> {
    if datasets.is_empty() {
        return Vec::new();
    }

    let chunk_size = options.chunk_size.get();
    let solver_options = options.solver_options.clone();
    let run_parallel = || {
        (0..datasets.len())
            .into_par_iter()
            .with_max_len(chunk_size)
            .map(|idx| feffit(&datasets[idx], variables, &solver_options))
            .collect::<Vec<_>>()
    };

    match options.strategy {
        FeffBatchExecutionStrategy::Sequential => datasets
            .iter()
            .map(|dataset| feffit(dataset, variables, &options.solver_options))
            .collect::<Vec<_>>(),
        FeffBatchExecutionStrategy::GlobalPool => run_parallel(),
        FeffBatchExecutionStrategy::DedicatedPool { threads } => {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads.get())
                .build()
                .map_err(|err| FittingError::SolverFailed {
                    reason: format!("failed to build rayon thread pool: {err}"),
                });

            match pool {
                Ok(pool) => pool.install(run_parallel),
                Err(err) => vec![Err(err); datasets.len()],
            }
        }
    }
}

fn validate_dataset(dataset: &FeffFitDataset) -> Result<(), FittingError> {
    if dataset.k.len() != dataset.chi.len() {
        return Err(FittingError::InvalidDataset {
            reason: format!(
                "k/chi length mismatch: k={}, chi={}",
                dataset.k.len(),
                dataset.chi.len()
            ),
        });
    }
    if dataset.k.len() < 3 {
        return Err(FittingError::InvalidDataset {
            reason: "fit dataset requires at least 3 points".to_string(),
        });
    }
    for index in 0..dataset.k.len() {
        if !dataset.k[index].is_finite() || !dataset.chi[index].is_finite() {
            return Err(FittingError::InvalidDataset {
                reason: format!("non-finite value detected at index {index}"),
            });
        }
        if index > 0 && dataset.k[index] < dataset.k[index - 1] {
            return Err(FittingError::InvalidDataset {
                reason: format!("k grid must be monotonic: k[{index}] < k[{}]", index - 1),
            });
        }
    }
    if dataset.paths.is_empty() {
        return Err(FittingError::EmptyPaths);
    }
    validate_transform(&dataset.transform)
}

#[derive(Clone)]
struct ModelEvaluation {
    model_chi: DVector<f64>,
    path_chi: Vec<(String, DVector<f64>)>,
    model_transform: TransformOutput,
}

#[derive(Clone)]
struct FeffFitMultiProblem {
    datasets: Vec<FeffFitDataset>,
    data_transforms: Vec<TransformOutput>,
    variables: FitVariables,
    variable_names: Vec<String>,
    residual_len: usize,
}

impl FeffFitMultiProblem {
    fn new(
        datasets: Vec<FeffFitDataset>,
        variables: FitVariables,
        variable_names: Vec<String>,
    ) -> Result<Self, FittingError> {
        let mut data_transforms = Vec::with_capacity(datasets.len());
        for dataset in &datasets {
            data_transforms.push(apply_r_transform(
                &dataset.k,
                &dataset.chi,
                &dataset.transform,
            )?);
        }

        let mut problem = Self {
            datasets,
            data_transforms,
            variables,
            variable_names,
            residual_len: 0,
        };
        let initial_residual = problem.current_residual()?;
        problem.residual_len = initial_residual.len();
        Ok(problem)
    }

    fn evaluate_dataset_model(
        dataset: &FeffFitDataset,
        vars: &FitVariables,
    ) -> Result<ModelEvaluation, FittingError> {
        let model = ff2chi(&dataset.paths, vars, &dataset.k)?;
        let model_transform = apply_r_transform(&dataset.k, &model.chi, &dataset.transform)?;
        Ok(ModelEvaluation {
            model_chi: model.chi,
            path_chi: model.path_chi,
            model_transform,
        })
    }

    fn evaluate_models(&self, vars: &FitVariables) -> Result<Vec<ModelEvaluation>, FittingError> {
        self.datasets
            .iter()
            .map(|dataset| Self::evaluate_dataset_model(dataset, vars))
            .collect()
    }

    fn current_residual(&self) -> Result<DVector<f64>, FittingError> {
        let models = self.evaluate_models(&self.variables)?;
        let mut residual = Vec::new();
        for ((dataset, data_transform), model) in self
            .datasets
            .iter()
            .zip(self.data_transforms.iter())
            .zip(models)
        {
            let ds_residual = residual_in_r_space(
                data_transform,
                &model.model_transform,
                &dataset.transform,
                dataset.epsilon_k,
            )?;
            residual.extend(ds_residual.iter().copied());
        }
        if residual.is_empty() {
            return Err(FittingError::InvalidDataset {
                reason: "multi-dataset residual is empty".to_string(),
            });
        }
        Ok(DVector::from_vec(residual))
    }

    fn residual_for_parameter_vector(&self, params: &DVector<f64>) -> DVector<f64> {
        let mut vars = self.variables.clone();
        if vars
            .apply_parameter_vector(&self.variable_names, params)
            .is_err()
        {
            return DVector::from_element(self.residual_len.max(2), 1.0e12);
        }

        match self.evaluate_models(&vars).and_then(|models| {
            let mut residual = Vec::new();
            for ((dataset, data_transform), model) in self
                .datasets
                .iter()
                .zip(self.data_transforms.iter())
                .zip(models)
            {
                let ds_residual = residual_in_r_space(
                    data_transform,
                    &model.model_transform,
                    &dataset.transform,
                    dataset.epsilon_k,
                )?;
                residual.extend(ds_residual.iter().copied());
            }
            Ok::<_, FittingError>(DVector::from_vec(residual))
        }) {
            Ok(residual) => residual,
            Err(_) => DVector::from_element(self.residual_len.max(2), 1.0e12),
        }
    }

    fn approx_covariance(&self) -> Option<DMatrix<f64>> {
        let params = self.params();
        if params.is_empty() {
            return None;
        }
        let jac = self.jacobian()?;
        let jtj = jac.transpose() * jac;
        jtj.try_inverse()
    }
}

#[cfg(feature = "trust-region")]
#[derive(Clone)]
struct FeffSpectrumFactor {
    dataset: FeffFitDataset,
    data_transform: TransformOutput,
    variables: FitVariables,
    variable_names: Vec<String>,
    residual_len: usize,
}

#[cfg(feature = "trust-region")]
impl FeffSpectrumFactor {
    fn residual_for_values(&self, values: &[f64]) -> DVector<f64> {
        let mut vars = self.variables.clone();
        for (name, value) in self.variable_names.iter().zip(values.iter().copied()) {
            if let Some(var) = vars.vars.get_mut(name) {
                var.value = var.clamp(value);
            }
        }

        match FeffFitMultiProblem::evaluate_dataset_model(&self.dataset, &vars).and_then(|model| {
            residual_in_r_space(
                &self.data_transform,
                &model.model_transform,
                &self.dataset.transform,
                self.dataset.epsilon_k,
            )
        }) {
            Ok(residual) => residual,
            Err(_) => DVector::from_element(self.residual_len, 1.0e12),
        }
    }

    fn clamped_values(&self, values: &[f64]) -> Vec<f64> {
        self.variable_names
            .iter()
            .zip(values.iter().copied())
            .map(|(name, value)| {
                self.variables
                    .vars
                    .get(name)
                    .map(|var| var.clamp(value))
                    .unwrap_or(value)
            })
            .collect()
    }

    fn bounded_difference_step(&self, column: usize, value: f64, step: f64) -> Option<f64> {
        let name = self.variable_names.get(column)?;
        let variable = self.variables.vars.get(name);
        let lower = variable
            .and_then(|var| var.min)
            .unwrap_or(f64::NEG_INFINITY);
        let upper = variable.and_then(|var| var.max).unwrap_or(f64::INFINITY);
        if lower > upper {
            return None;
        }

        let forward_room = upper - value;
        if forward_room >= step {
            return Some(step);
        }

        let backward_room = value - lower;
        if backward_room >= step {
            return Some(-step);
        }

        if forward_room > 0.0 {
            return Some(forward_room);
        }
        if backward_room > 0.0 {
            return Some(-backward_room);
        }

        None
    }
}

#[cfg(feature = "trust-region")]
impl ApexFactor for FeffSpectrumFactor {
    fn linearize(
        &self,
        params: &[ApexDVector<f64>],
        compute_jacobian: bool,
    ) -> (ApexDVector<f64>, Option<ApexDMatrix<f64>>) {
        let values = params
            .iter()
            .map(|param| param.get(0).copied().unwrap_or(0.0))
            .collect::<Vec<_>>();
        let values = self.clamped_values(&values);
        let base = self.residual_for_values(&values);
        let residual = ApexDVector::from_vec(base.as_slice().to_vec());

        if !compute_jacobian {
            return (residual, None);
        }

        let epsilon = f64::EPSILON.sqrt();
        let mut jacobian = ApexDMatrix::zeros(base.len(), values.len());

        for column in 0..values.len() {
            let step = (epsilon * values[column].abs().max(1.0)).max(epsilon);
            if !step.is_finite() {
                return (residual, None);
            }
            let Some(step) = self.bounded_difference_step(column, values[column], step) else {
                continue;
            };

            let mut shifted = values.clone();
            shifted[column] = values[column] + step;
            let fx1 = self.residual_for_values(&shifted);
            let diff = (fx1 - &base) / step;
            for row in 0..diff.len() {
                jacobian[(row, column)] = diff[row];
            }
        }

        (residual, Some(jacobian))
    }

    fn get_dimension(&self) -> usize {
        self.residual_len
    }
}

#[cfg(feature = "trust-region")]
fn apex_jacobian_mode(options: &FeffFitOptions, dataset_count: usize) -> ApexJacobianMode {
    match options.jacobian_mode {
        FeffFitJacobianMode::Dense => ApexJacobianMode::Dense,
        FeffFitJacobianMode::Sparse => ApexJacobianMode::Sparse,
        FeffFitJacobianMode::Auto => {
            if dataset_count > 1 {
                ApexJacobianMode::Sparse
            } else {
                ApexJacobianMode::Dense
            }
        }
    }
}

#[cfg(feature = "trust-region")]
fn solve_fit_problem_apex(
    mut problem: FeffFitMultiProblem,
    options: &FeffFitOptions,
) -> Result<FeffFitMultiProblem, FittingError> {
    let jacobian_mode = apex_jacobian_mode(options, problem.datasets.len());
    let mut apex_problem = ApexProblem::new(jacobian_mode);
    let mut initial_values = HashMap::<String, (ManifoldType, ApexDVector<f64>)>::new();

    for name in &problem.variable_names {
        let variable =
            problem
                .variables
                .vars
                .get(name)
                .ok_or_else(|| FittingError::UndefinedSymbol {
                    symbol: name.clone(),
                })?;
        initial_values.insert(
            name.clone(),
            (
                ManifoldType::RN,
                ApexDVector::from_vec(vec![variable.clamp(variable.value)]),
            ),
        );
        if variable.min.is_some() || variable.max.is_some() {
            let lower = variable.min.unwrap_or(f64::NEG_INFINITY);
            let upper = variable.max.unwrap_or(f64::INFINITY);
            if lower > upper {
                return Err(FittingError::InvalidDataset {
                    reason: format!(
                        "invalid bounds for variable '{name}': lower {lower} exceeds upper {upper}"
                    ),
                });
            }
            apex_problem.set_variable_bounds(name, 0, lower, upper);
        }
    }

    for (dataset, data_transform) in problem
        .datasets
        .iter()
        .cloned()
        .zip(problem.data_transforms.iter().cloned())
    {
        let factor_names =
            dataset_varying_names(&dataset, &problem.variables, &problem.variable_names)?;
        let residual_len = residual_in_r_space(
            &data_transform,
            &FeffFitMultiProblem::evaluate_dataset_model(&dataset, &problem.variables)?
                .model_transform,
            &dataset.transform,
            dataset.epsilon_k,
        )?
        .len();
        let factor = FeffSpectrumFactor {
            dataset,
            data_transform,
            variables: problem.variables.clone(),
            variable_names: factor_names.clone(),
            residual_len,
        };
        let factor_name_refs = factor_names.iter().map(String::as_str).collect::<Vec<_>>();
        apex_problem.add_residual_block(&factor_name_refs, Box::new(factor), None);
    }

    let linear_solver_type = match jacobian_mode {
        ApexJacobianMode::Dense => LinearSolverType::DenseQR,
        ApexJacobianMode::Sparse => LinearSolverType::SparseQR,
    };
    let config = DogLegConfig::new()
        .with_linear_solver_type(linear_solver_type)
        .with_max_iterations(100)
        .with_cost_tolerance(1.0e-10)
        .with_parameter_tolerance(1.0e-10)
        .with_gradient_tolerance(1.0e-10);
    let mut optimizer = DogLeg::with_config(config);
    let result = optimizer
        .optimize(&apex_problem, &initial_values)
        .map_err(|err| FittingError::SolverFailed {
            reason: format!("trust-region DogLeg solver failed: {err}"),
        })?;

    let mut solved_params = DVector::zeros(problem.variable_names.len());
    for (idx, name) in problem.variable_names.iter().enumerate() {
        let value = result
            .parameters
            .get(name)
            .map(VariableEnum::to_vector)
            .and_then(|vector| vector.get(0).copied())
            .ok_or_else(|| FittingError::SolverFailed {
                reason: format!("trust-region result missing variable '{name}'"),
            })?;
        solved_params[idx] = value;
    }
    problem
        .variables
        .apply_parameter_vector(&problem.variable_names, &solved_params)?;
    Ok(problem)
}

#[cfg(not(feature = "trust-region"))]
fn solve_fit_problem_apex(
    _problem: FeffFitMultiProblem,
    _options: &FeffFitOptions,
) -> Result<FeffFitMultiProblem, FittingError> {
    Err(FittingError::SolverFailed {
        reason: "trust-region DogLeg solver requires the trust-region feature".to_string(),
    })
}

impl LeastSquaresProblem<f64, Dyn, Dyn> for FeffFitMultiProblem {
    type ParameterStorage = Owned<f64, Dyn>;
    type ResidualStorage = Owned<f64, Dyn>;
    type JacobianStorage = Owned<f64, Dyn, Dyn>;

    fn set_params(&mut self, params: &DVector<f64>) {
        let _ = self
            .variables
            .apply_parameter_vector(&self.variable_names, params);
    }

    fn params(&self) -> DVector<f64> {
        self.variables.parameter_vector(&self.variable_names)
    }

    fn residuals(&self) -> Option<DVector<f64>> {
        Some(
            self.current_residual()
                .unwrap_or_else(|_| DVector::from_element(self.residual_len.max(2), 1.0e12)),
        )
    }

    fn jacobian(&self) -> Option<DMatrix<f64>> {
        let x = self.params();
        if x.is_empty() {
            return Some(DMatrix::zeros(self.residual_len, 0));
        }

        let epsilon = f64::EPSILON.sqrt();
        let base = self.residual_for_parameter_vector(&x);
        let mut jac = DMatrix::zeros(base.len(), x.len());

        for i in 0..x.len() {
            let step = (epsilon * x[i].abs().max(1.0)).max(epsilon);
            if !step.is_finite() {
                return None;
            }
            let mut xt = x.clone();
            xt[i] += step;
            let fx1 = self.residual_for_parameter_vector(&xt);
            jac.set_column(i, &((fx1 - &base) / step));
        }

        Some(jac)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::num::NonZeroUsize;

    use crate::xafs::fitting::path_model::feffpath;
    use crate::xafs::fitting::types::{
        FeffBatchExecutionStrategy, FeffBatchOptions, FeffFitJacobianMode, FeffFitOptions,
        FeffFlavor, FitVariable, PathParamSpec,
    };
    use crate::xafs::tests::TOP_DIR;

    #[cfg(feature = "trust-region")]
    #[test]
    fn test_single_dataset_fit_recovers_synthetic_parameters() {
        let pathfile = format!("{TOP_DIR}/tests/testfiles/feffcu01.dat");
        let mut path = feffpath(pathfile, FeffFlavor::Feff85L).unwrap();
        path.s02 = PathParamSpec::Expression("amp".to_string());
        path.e0 = PathParamSpec::Expression("de0".to_string());
        path.sigma2 = PathParamSpec::Expression("sig2".to_string());
        path.deltar = PathParamSpec::Expression("dr".to_string());

        let k = DVector::from_iterator(280, (0..280).map(|i| 0.05 * (i as f64 + 1.0)));

        let mut truth = FitVariables::new();
        truth.insert("amp", FitVariable::new(0.88, false));
        truth.insert("de0", FitVariable::new(1.2, false));
        truth.insert("sig2", FitVariable::new(0.0032, false));
        truth.insert("dr", FitVariable::new(0.012, false));

        let synthetic = ff2chi(&[path.clone()], &truth, &k).unwrap();

        let dataset = FeffFitDataset {
            k: k.clone(),
            chi: synthetic.chi,
            epsilon_k: Some(1.0),
            transform: Default::default(),
            paths: vec![path],
        };

        let mut initial = FitVariables::new();
        initial.insert("amp", FitVariable::new(0.95, true));
        initial.insert("de0", FitVariable::new(0.0, true));
        initial.insert(
            "sig2",
            FitVariable::new(0.0020, true).with_bounds(Some(0.0), Some(0.02)),
        );
        initial.insert("dr", FitVariable::new(0.0, true));

        let result = feffit(&dataset, &initial, &FeffFitOptions::trust_region()).unwrap();

        let amp = result.variables.get("amp").unwrap().value;
        let de0 = result.variables.get("de0").unwrap().value;
        let sig2 = result.variables.get("sig2").unwrap().value;
        let dr = result.variables.get("dr").unwrap().value;

        assert!((amp - 0.88).abs() < 0.15);
        assert!((de0 - 1.2).abs() < 1.2);
        assert!((sig2 - 0.0032).abs() < 0.003);
        assert!((dr - 0.012).abs() < 0.02);
        assert!(result.chi_square.is_finite());
        assert_eq!(result.datasets.len(), 1);
        assert!(!result.path_contributions.is_empty());
    }

    #[test]
    fn test_joint_dataset_fit_runs() {
        let pathfile = format!("{TOP_DIR}/tests/testfiles/feffcu01.dat");
        let mut path1 = feffpath(pathfile.clone(), FeffFlavor::Feff85L).unwrap();
        path1.s02 = PathParamSpec::Expression("amp".to_string());
        path1.e0 = PathParamSpec::Expression("de0".to_string());
        path1.sigma2 = PathParamSpec::Expression("sig2".to_string());
        path1.deltar = PathParamSpec::Expression("dr".to_string());
        let path2 = path1.clone().set_sigma2("sig2_2");

        let k1 = DVector::from_iterator(220, (0..220).map(|i| 0.05 * (i as f64 + 1.0)));
        let k2 = DVector::from_iterator(250, (0..250).map(|i| 0.05 * (i as f64 + 1.0)));

        let mut truth = FitVariables::new();
        truth.insert("amp", FitVariable::new(0.9, false));
        truth.insert("de0", FitVariable::new(1.2, false));
        truth.insert("sig2", FitVariable::new(0.003, false));
        truth.insert("sig2_2", FitVariable::new(0.004, false));
        truth.insert("dr", FitVariable::new(0.01, false));

        let chi1 = ff2chi(std::slice::from_ref(&path1), &truth, &k1)
            .unwrap()
            .chi;
        let chi2 = ff2chi(std::slice::from_ref(&path2), &truth, &k2)
            .unwrap()
            .chi;

        let ds1 = FeffFitDataset {
            k: k1,
            chi: chi1,
            epsilon_k: Some(1.0),
            transform: Default::default(),
            paths: vec![path1],
        };
        let ds2 = FeffFitDataset {
            k: k2,
            chi: chi2,
            epsilon_k: Some(1.0),
            transform: Default::default(),
            paths: vec![path2],
        };

        let mut initial = FitVariables::new();
        initial.insert("amp", FitVariable::new(0.95, true));
        initial.insert("de0", FitVariable::new(0.0, true));
        initial.insert("sig2", FitVariable::new(0.0020, true));
        initial.insert("sig2_2", FitVariable::new(0.0020, true));
        initial.insert("dr", FitVariable::new(0.0, true));

        let result = feffit_joint(&[ds1, ds2], &initial).unwrap();
        assert!(result.chi_square.is_finite());
        assert_eq!(result.datasets.len(), 2);

        // Per-dataset reduced chi-square follows Larch scaling with n_idp.
        let expected0 = result.datasets[0].chi_square / (result.datasets[0].n_idp - 4.0);
        let expected1 = result.datasets[1].chi_square / (result.datasets[1].n_idp - 4.0);
        assert!((result.datasets[0].reduced_chi_square - expected0).abs() < 1.0e-10);
        assert!((result.datasets[1].reduced_chi_square - expected1).abs() < 1.0e-10);
    }

    #[cfg(feature = "trust-region")]
    #[test]
    fn test_trust_region_matches_legacy_lm_for_single_dataset() {
        let pathfile = format!("{TOP_DIR}/tests/testfiles/feffcu01.dat");
        let mut path = feffpath(pathfile, FeffFlavor::Feff85L).unwrap();
        path.s02 = PathParamSpec::Expression("amp".to_string());

        let k = DVector::from_iterator(180, (0..180).map(|i| 0.05 * (i as f64 + 1.0)));
        let mut truth = FitVariables::new();
        truth.insert("amp", FitVariable::new(0.82, false));
        let synthetic = ff2chi(&[path.clone()], &truth, &k).unwrap();

        let dataset = FeffFitDataset::new()
            .data(&k, &synthetic.chi)
            .epsilon_k(1.0)
            .add_path(path);
        let mut initial = FitVariables::new();
        initial.insert("amp", FitVariable::new(0.95, true));

        let trust_region = feffit_joint_with_options(
            std::slice::from_ref(&dataset),
            &initial,
            &FeffFitOptions::trust_region(),
        )
        .unwrap();
        let lm =
            feffit_joint_with_options(&[dataset], &initial, &FeffFitOptions::levenberg_marquardt())
                .unwrap();

        let tr_amp = trust_region.variables.get("amp").unwrap().value;
        let lm_amp = lm.variables.get("amp").unwrap().value;
        assert!((tr_amp - lm_amp).abs() < 1.0e-8);
        assert!((trust_region.chi_square - lm.chi_square).abs() < 1.0e-12);
    }

    #[cfg(feature = "trust-region")]
    #[test]
    fn test_sparse_trust_region_matches_legacy_lm_for_joint_dataset() {
        let pathfile = format!("{TOP_DIR}/tests/testfiles/feffcu01.dat");
        let base_path = feffpath(pathfile, FeffFlavor::Feff85L).unwrap();
        let k = DVector::from_iterator(160, (0..160).map(|i| 0.05 * (i as f64 + 1.0)));

        let mut datasets = [("amp_a", 0.78), ("amp_b", 0.91)]
            .into_iter()
            .map(|(name, value)| {
                let mut path = base_path.clone();
                path.s02 = PathParamSpec::Expression(name.to_string());
                let mut truth = FitVariables::new();
                truth.insert(name, FitVariable::new(value, false));
                let synthetic = ff2chi(&[path.clone()], &truth, &k).unwrap();
                FeffFitDataset::new()
                    .data(&k, &synthetic.chi)
                    .epsilon_k(1.0)
                    .add_path(path)
            })
            .collect::<Vec<_>>();
        let fixed_synthetic = ff2chi(std::slice::from_ref(&base_path), &FitVariables::new(), &k)
            .unwrap()
            .chi;
        datasets.push(
            FeffFitDataset::new()
                .data(&k, &fixed_synthetic)
                .epsilon_k(1.0)
                .add_path(base_path),
        );

        let mut initial = FitVariables::new();
        initial.insert("amp_a", FitVariable::new(0.95, true));
        initial.insert("amp_b", FitVariable::new(0.95, true));

        let trust_region = feffit_joint_with_options(
            &datasets,
            &initial,
            &FeffFitOptions::trust_region().with_jacobian_mode(FeffFitJacobianMode::Sparse),
        )
        .unwrap();
        let lm =
            feffit_joint_with_options(&datasets, &initial, &FeffFitOptions::levenberg_marquardt())
                .unwrap();

        for name in ["amp_a", "amp_b"] {
            let tr_value = trust_region.variables.get(name).unwrap().value;
            let lm_value = lm.variables.get(name).unwrap().value;
            assert!((tr_value - lm_value).abs() < 1.0e-8);
        }
        assert!((trust_region.chi_square - lm.chi_square).abs() < 1.0e-12);
    }

    #[test]
    fn test_dataset_vary_count_tracks_expression_dependencies() {
        let path1 = crate::xafs::fitting::types::FeffPathModel::default()
            .set_s02("amp")
            .set_sigma2("sig2_eff");
        let path2 = crate::xafs::fitting::types::FeffPathModel::default().set_s02("amp2");

        let ds1 = FeffFitDataset::new().add_path(path1);
        let ds2 = FeffFitDataset::new().add_path(path2);

        let mut vars = FitVariables::new();
        vars.insert("amp", FitVariable::new(1.0, true));
        vars.insert("amp2", FitVariable::new(1.0, true));
        vars.insert("sig2", FitVariable::new(0.003, true));
        vars.insert(
            "sig2_eff",
            FitVariable::new(0.0, false).with_expr("sig2 * 2.0"),
        );
        let varying = vec!["amp".to_string(), "amp2".to_string(), "sig2".to_string()];

        assert_eq!(dataset_vary_count(&ds1, &vars, &varying).unwrap(), 2);
        assert_eq!(dataset_vary_count(&ds2, &vars, &varying).unwrap(), 1);
    }

    #[test]
    fn test_batch_parallel_preserves_order() {
        let pathfile = format!("{TOP_DIR}/tests/testfiles/feffcu01.dat");
        let mut path = feffpath(pathfile, FeffFlavor::Feff85L).unwrap();
        path.s02 = PathParamSpec::Expression("amp".to_string());

        let k = DVector::from_iterator(180, (0..180).map(|i| 0.05 * (i as f64 + 1.0)));

        let mut initial = FitVariables::new();
        initial.insert("amp", FitVariable::new(0.95, true));

        let datasets = [0.70, 0.80, 0.90, 1.00]
            .iter()
            .map(|amp| {
                let mut truth = FitVariables::new();
                truth.insert("amp", FitVariable::new(*amp, false));
                let synthetic = ff2chi(&[path.clone()], &truth, &k).unwrap();

                FeffFitDataset::new()
                    .data(&k, &synthetic.chi)
                    .epsilon_k(1.0)
                    .add_path(path.clone())
            })
            .collect::<Vec<_>>();

        let serial = feffit_independent(
            &datasets,
            &initial,
            &FeffBatchOptions {
                strategy: FeffBatchExecutionStrategy::Sequential,
                chunk_size: NonZeroUsize::new(2).expect("nonzero constant"),
                solver_options: FeffFitOptions::default(),
            },
        );
        let parallel = feffit_independent(
            &datasets,
            &initial,
            &FeffBatchOptions::dedicated(NonZeroUsize::new(2).expect("nonzero constant"))
                .with_chunk_size(NonZeroUsize::new(2).expect("nonzero constant")),
        );

        assert_eq!(serial.len(), datasets.len());
        assert_eq!(parallel.len(), datasets.len());

        for idx in 0..datasets.len() {
            let serial_result = serial[idx].as_ref().unwrap();
            let parallel_result = parallel[idx].as_ref().unwrap();
            assert!((parallel_result.data_chi[5] - datasets[idx].chi[5]).abs() < 1.0e-12);
            let amp_serial = serial_result.variables.get("amp").unwrap().value;
            let amp_parallel = parallel_result.variables.get("amp").unwrap().value;
            assert!((amp_serial - amp_parallel).abs() < 1.0e-8);
        }
    }

    #[test]
    fn test_independent_batch_collects_per_item_failures() {
        let pathfile = format!("{TOP_DIR}/tests/testfiles/feffcu01.dat");
        let mut path = feffpath(pathfile, FeffFlavor::Feff85L).unwrap();
        path.s02 = PathParamSpec::Expression("amp".to_string());

        let k = DVector::from_iterator(140, (0..140).map(|i| 0.05 * (i as f64 + 1.0)));
        let mut truth = FitVariables::new();
        truth.insert("amp", FitVariable::new(0.9, false));
        let synthetic = ff2chi(&[path.clone()], &truth, &k).unwrap();

        let valid = FeffFitDataset::new()
            .data(&k, &synthetic.chi)
            .epsilon_k(1.0)
            .add_path(path);
        let invalid = FeffFitDataset::new()
            .data(&k, &DVector::zeros(k.len() - 1))
            .epsilon_k(1.0);
        let datasets = vec![valid.clone(), invalid, valid];

        let mut initial = FitVariables::new();
        initial.insert("amp", FitVariable::new(0.95, true));

        let out = feffit_independent(&datasets, &initial, &FeffBatchOptions::parallel());
        assert_eq!(out.len(), datasets.len());
        assert!(out[0].is_ok());
        assert!(out[1].is_err());
        assert!(out[2].is_ok());
    }

    #[cfg(feature = "trust-region")]
    #[test]
    fn test_fit_result_exposes_covariance_and_correlation() {
        let pathfile = format!("{TOP_DIR}/tests/testfiles/feffcu01.dat");
        let mut path = feffpath(pathfile, FeffFlavor::Feff85L).unwrap();
        path.s02 = PathParamSpec::Expression("amp".to_string());
        path.e0 = PathParamSpec::Expression("de0".to_string());
        path.sigma2 = PathParamSpec::Expression("sig2".to_string());
        path.deltar = PathParamSpec::Expression("dr".to_string());

        let k = DVector::from_iterator(240, (0..240).map(|i| 0.05 * (i as f64 + 1.0)));
        let mut truth = FitVariables::new();
        truth.insert("amp", FitVariable::new(0.92, false));
        truth.insert("de0", FitVariable::new(5.3, false));
        truth.insert("sig2", FitVariable::new(0.0052, false));
        truth.insert("dr", FitVariable::new(0.0018, false));
        let synthetic = ff2chi(&[path.clone()], &truth, &k).unwrap();

        let dataset = FeffFitDataset::new()
            .data(&k, &synthetic.chi)
            .epsilon_k(0.0015)
            .krange(3.0, 16.0)
            .rrange(1.4, 3.0)
            .kweight(2.0)
            .dk(5.0)
            .add_path(path);

        let mut initial = FitVariables::new();
        initial.insert("amp", FitVariable::new(1.0, true));
        initial.insert("de0", FitVariable::new(0.0, true));
        initial.insert(
            "sig2",
            FitVariable::new(0.003, true).with_bounds(Some(0.0), Some(0.02)),
        );
        initial.insert("dr", FitVariable::new(0.0, true));

        let result = feffit(&dataset, &initial, &FeffFitOptions::trust_region()).unwrap();
        assert_eq!(result.varying_names.len(), result.n_vary);
        let cov = result.covariance.as_ref().expect("missing covariance");
        let corr = result.correlation.as_ref().expect("missing correlation");
        assert_eq!(cov.len(), result.n_vary);
        assert_eq!(corr.len(), result.n_vary);
        for i in 0..result.n_vary {
            assert_eq!(cov[i].len(), result.n_vary);
            assert_eq!(corr[i].len(), result.n_vary);
            assert!((corr[i][i] - 1.0).abs() < 1.0e-8);
        }
    }
}
