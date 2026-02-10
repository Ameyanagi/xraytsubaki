use levenberg_marquardt::{LeastSquaresProblem, LevenbergMarquardt};
use nalgebra::{DMatrix, DVector, Dyn, Owned};
use rayon::prelude::*;

use super::errors::FittingError;
use super::path_model::ff2chi;
use super::transform::{
    apply_r_transform, compute_n_idp, residual_in_r_space, validate_transform, TransformOutput,
};
use super::types::{
    DatasetResult, FeffBatchExecutionStrategy, FeffBatchOptions, FeffFitDataset, FeffFitResult,
    FitVariables, PathContribution,
};

fn feffit(
    dataset: &FeffFitDataset,
    variables: &FitVariables,
) -> Result<FeffFitResult, FittingError> {
    feffit_joint(std::slice::from_ref(dataset), variables)
}

pub fn feffit_joint(
    datasets: &[FeffFitDataset],
    variables: &FitVariables,
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
    let (solved, _report) = LevenbergMarquardt::new().minimize(problem);
    let residual = solved.current_residual()?;

    let mut solved_variables = solved.variables.clone();
    let chi_square = residual.dot(&residual);
    let n_data = residual.len();
    let n_vary = solved.variable_names.len();
    let dof = n_data.saturating_sub(n_vary).max(1);
    let reduced_chi_square = chi_square / dof as f64;

    if let Some(covar) = solved.approx_covariance() {
        for (idx, name) in solved.variable_names.iter().enumerate() {
            if let Some(var) = solved_variables.get_mut(name) {
                let variance = covar[(idx, idx)].max(0.0) * reduced_chi_square;
                var.stderr = Some(variance.sqrt());
            }
        }
    }

    let model_evaluations = solved.evaluate_models(&solved.variables)?;
    let mut datasets_out = Vec::with_capacity(solved.datasets.len());

    let mut global_data_norm = 0.0;
    let mut global_model_diff_norm = 0.0;
    let mut global_n_idp = 0.0;

    for ((dataset, data_transform), model_eval) in solved
        .datasets
        .iter()
        .zip(solved.data_transforms.iter())
        .zip(model_evaluations.into_iter())
    {
        let ds_residual = residual_in_r_space(
            data_transform,
            &model_eval.model_transform,
            dataset.epsilon_k,
        )?;
        let ds_chi_square = ds_residual.dot(&ds_residual);
        let ds_n_data = ds_residual.len();
        let ds_dof = ds_n_data.saturating_sub(n_vary).max(1);
        let ds_reduced_chi_square = ds_chi_square / ds_dof as f64;

        let data_norm = dataset.chi.iter().map(|value| value.abs()).sum::<f64>();
        let model_diff_norm = dataset
            .chi
            .iter()
            .zip(model_eval.model_chi.iter())
            .map(|(d, m)| (d - m).abs())
            .sum::<f64>();
        global_data_norm += data_norm;
        global_model_diff_norm += model_diff_norm;
        let ds_r_factor = if data_norm.abs() < f64::EPSILON {
            0.0
        } else {
            model_diff_norm / data_norm
        };

        let ds_n_idp = compute_n_idp(&dataset.transform);
        global_n_idp += ds_n_idp;

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
            r: model_eval.model_transform.r.clone(),
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
        n_vary,
        n_data,
        chi_square,
        reduced_chi_square,
        r_factor,
        datasets: datasets_out,
        n_idp: global_n_idp,
        ..FeffFitResult::default()
    };
    out.sync_primary_dataset_fields();
    Ok(out)
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
    let run_parallel = || {
        (0..datasets.len())
            .into_par_iter()
            .with_max_len(chunk_size)
            .map(|idx| feffit(&datasets[idx], variables))
            .collect::<Vec<_>>()
    };

    match options.strategy {
        FeffBatchExecutionStrategy::Sequential => datasets
            .iter()
            .map(|dataset| feffit(dataset, variables))
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
            .zip(models.into_iter())
        {
            let ds_residual =
                residual_in_r_space(data_transform, &model.model_transform, dataset.epsilon_k)?;
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
                .zip(models.into_iter())
            {
                let ds_residual =
                    residual_in_r_space(data_transform, &model.model_transform, dataset.epsilon_k)?;
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
            let mut xt = x.clone();
            xt[i] += epsilon;
            let fx1 = self.residual_for_parameter_vector(&xt);
            jac.set_column(i, &((fx1 - &base) / epsilon));
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
        FeffBatchExecutionStrategy, FeffBatchOptions, FeffFlavor, FitVariable, PathParamSpec,
    };
    use crate::xafs::tests::TOP_DIR;

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

        let result = feffit(&dataset, &initial).unwrap();

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

        let chi1 = ff2chi(&[path1.clone()], &truth, &k1).unwrap().chi;
        let chi2 = ff2chi(&[path2.clone()], &truth, &k2).unwrap().chi;

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
}
