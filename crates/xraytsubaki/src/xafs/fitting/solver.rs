use levenberg_marquardt::{LeastSquaresProblem, LevenbergMarquardt};
use nalgebra::{DMatrix, DVector, Dyn, Owned};

use super::errors::FittingError;
use super::path_model::ff2chi;
use super::transform::{
    apply_r_transform, residual_in_r_space, validate_transform, TransformOutput,
};
use super::types::{FeffFitDataset, FeffFitResult, FitVariables, PathContribution};

pub fn feffit(
    dataset: &FeffFitDataset,
    variables: &FitVariables,
) -> Result<FeffFitResult, FittingError> {
    validate_dataset(dataset)?;
    let varying_names = variables.varying_names();
    if varying_names.is_empty() {
        return Err(FittingError::NoVaryingVariables);
    }

    let data_transform = apply_r_transform(&dataset.k, &dataset.chi, &dataset.transform)?;

    let problem = FeffFitProblem::new(
        dataset.clone(),
        variables.clone(),
        varying_names,
        data_transform,
    )?;

    let (solved, _report) = LevenbergMarquardt::new().minimize(problem);

    let model_eval = solved.evaluate_model(&solved.variables)?;
    let residual = residual_in_r_space(
        &solved.data_transform,
        &model_eval.model_transform,
        solved.dataset.epsilon_k,
    )?;

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

    let path_contributions = model_eval
        .path_chi
        .iter()
        .filter_map(|(label, chi)| {
            let transformed =
                apply_r_transform(&solved.dataset.k, chi, &solved.dataset.transform).ok()?;
            Some(PathContribution {
                label: label.clone(),
                chi: chi.clone(),
                chir_re: transformed.chir_re,
                chir_im: transformed.chir_im,
                chir_mag: transformed.chir_mag,
            })
        })
        .collect::<Vec<_>>();

    let data_norm = solved
        .dataset
        .chi
        .iter()
        .map(|value| value.abs())
        .sum::<f64>();
    let model_diff_norm = solved
        .dataset
        .chi
        .iter()
        .zip(model_eval.model_chi.iter())
        .map(|(d, m)| (d - m).abs())
        .sum::<f64>();
    let r_factor = if data_norm.abs() < f64::EPSILON {
        0.0
    } else {
        model_diff_norm / data_norm
    };

    Ok(FeffFitResult {
        variables: solved_variables,
        n_vary,
        n_data,
        chi_square,
        reduced_chi_square,
        r_factor,
        k: solved.dataset.k.clone(),
        data_chi: solved.dataset.chi.clone(),
        model_chi: model_eval.model_chi,
        r: model_eval.model_transform.r.clone(),
        data_chir_re: solved.data_transform.chir_re.clone(),
        data_chir_im: solved.data_transform.chir_im.clone(),
        model_chir_re: model_eval.model_transform.chir_re,
        model_chir_im: model_eval.model_transform.chir_im,
        model_chir_mag: model_eval.model_transform.chir_mag,
        path_contributions,
    })
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
struct FeffFitProblem {
    dataset: FeffFitDataset,
    variables: FitVariables,
    variable_names: Vec<String>,
    data_transform: TransformOutput,
    residual_len: usize,
}

impl FeffFitProblem {
    fn new(
        dataset: FeffFitDataset,
        variables: FitVariables,
        variable_names: Vec<String>,
        data_transform: TransformOutput,
    ) -> Result<Self, FittingError> {
        let mut problem = Self {
            dataset,
            variables,
            variable_names,
            data_transform,
            residual_len: 0,
        };

        let initial_residual = problem.current_residual()?;
        problem.residual_len = initial_residual.len();
        Ok(problem)
    }

    fn evaluate_model(&self, vars: &FitVariables) -> Result<ModelEvaluation, FittingError> {
        let model = ff2chi(&self.dataset.paths, vars, &self.dataset.k)?;
        let model_transform =
            apply_r_transform(&self.dataset.k, &model.chi, &self.dataset.transform)?;

        Ok(ModelEvaluation {
            model_chi: model.chi,
            path_chi: model.path_chi,
            model_transform,
        })
    }

    fn current_residual(&self) -> Result<DVector<f64>, FittingError> {
        let model = self.evaluate_model(&self.variables)?;
        residual_in_r_space(
            &self.data_transform,
            &model.model_transform,
            self.dataset.epsilon_k,
        )
    }

    fn residual_for_parameter_vector(&self, params: &DVector<f64>) -> DVector<f64> {
        let mut vars = self.variables.clone();
        if vars
            .apply_parameter_vector(&self.variable_names, params)
            .is_err()
        {
            return DVector::from_element(self.residual_len.max(2), 1.0e12);
        }

        match self.evaluate_model(&vars).and_then(|model| {
            residual_in_r_space(
                &self.data_transform,
                &model.model_transform,
                self.dataset.epsilon_k,
            )
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

impl LeastSquaresProblem<f64, Dyn, Dyn> for FeffFitProblem {
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
    use crate::xafs::fitting::path_model::feffpath;
    use crate::xafs::fitting::types::{FeffFlavor, FitVariable, PathParamSpec};
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
        assert!(!result.path_contributions.is_empty());
    }
}
