use std::collections::BTreeMap;

use nalgebra::DVector;

use super::errors::FittingError;
use super::solver;
use super::types::{
    FeffFitDataset, FeffFitResult, FeffFlavor, FeffPathModel, FitVariable, FitVariables,
    FitWarning, Param, PathParamSpec,
};
use super::variables::try_extract_symbols;
use crate::xafs::xafsutils::FTWindow;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParamRole {
    S02,
    Degen,
    Sigma2,
    E0,
    Ei,
    Deltar,
    Third,
    Fourth,
    UntypedExpr,
}

impl ParamRole {
    fn default_value(self) -> f64 {
        match self {
            Self::S02 | Self::Degen => 1.0,
            Self::Sigma2 => 0.003,
            Self::E0 | Self::Ei | Self::Deltar | Self::Third | Self::Fourth | Self::UntypedExpr => {
                0.0
            }
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::S02 => "s02",
            Self::Degen => "degen",
            Self::Sigma2 => "sigma2",
            Self::E0 => "e0",
            Self::Ei => "ei",
            Self::Deltar => "deltar",
            Self::Third => "third",
            Self::Fourth => "fourth",
            Self::UntypedExpr => "expr",
        }
    }
}

fn path_param_expr(spec: &PathParamSpec) -> Option<&str> {
    match spec {
        PathParamSpec::Expression(expr) => Some(expr.as_str()),
        PathParamSpec::Value(_) => None,
    }
}

#[derive(Debug, Clone)]
pub struct FeffFit {
    datasets: Vec<FeffFitDataset>,
    variables: FitVariables,
    flavor: FeffFlavor,
    default_dataset: FeffFitDataset,
    has_default: bool,
}

impl Default for FeffFit {
    fn default() -> Self {
        Self {
            datasets: Vec::new(),
            variables: FitVariables::new(),
            flavor: FeffFlavor::Feff85L,
            default_dataset: FeffFitDataset::default(),
            has_default: false,
        }
    }
}

impl FeffFit {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn data(mut self, k: &DVector<f64>, chi: &DVector<f64>) -> Self {
        self.default_dataset = self.default_dataset.data(k, chi);
        self.has_default = true;
        self
    }

    pub fn epsilon_k(mut self, value: f64) -> Self {
        self.default_dataset = self.default_dataset.epsilon_k(value);
        self.has_default = true;
        self
    }

    pub fn add_path(mut self, path: FeffPathModel) -> Self {
        self.default_dataset = self.default_dataset.add_path(path);
        self.has_default = true;
        self
    }

    pub fn krange(mut self, kmin: f64, kmax: f64) -> Self {
        self.default_dataset = self.default_dataset.krange(kmin, kmax);
        self.has_default = true;
        self
    }

    pub fn rrange(mut self, rmin: f64, rmax: f64) -> Self {
        self.default_dataset = self.default_dataset.rrange(rmin, rmax);
        self.has_default = true;
        self
    }

    pub fn kweight(mut self, value: f64) -> Self {
        self.default_dataset = self.default_dataset.kweight(value);
        self.has_default = true;
        self
    }

    pub fn dk(mut self, value: f64) -> Self {
        self.default_dataset = self.default_dataset.dk(value);
        self.has_default = true;
        self
    }

    pub fn window(mut self, value: FTWindow) -> Self {
        self.default_dataset = self.default_dataset.window(value);
        self.has_default = true;
        self
    }

    pub fn rwindow(mut self, value: FTWindow) -> Self {
        self.default_dataset = self.default_dataset.rwindow(value);
        self.has_default = true;
        self
    }

    pub fn dr(mut self, value: f64) -> Self {
        self.default_dataset = self.default_dataset.dr(value);
        self.has_default = true;
        self
    }

    pub fn add_dataset(mut self, dataset: FeffFitDataset) -> Self {
        self.datasets.push(dataset);
        self
    }

    pub fn set_init(mut self, name: impl Into<String>, value: f64) -> Self {
        let name = name.into();
        let entry = self
            .variables
            .vars
            .entry(name)
            .or_insert_with(|| FitVariable::new(value, true));
        entry.value = value;
        entry.init_value = value;
        entry.vary = true;
        if entry.expr.is_some() {
            entry.expr = None;
        }
        self
    }

    pub fn set_inits<I, S>(mut self, inits: I) -> Self
    where
        I: IntoIterator<Item = (S, f64)>,
        S: Into<String>,
    {
        for (name, value) in inits {
            self = self.set_init(name, value);
        }
        self
    }

    pub fn set_bounds(mut self, name: impl Into<String>, min: f64, max: f64) -> Self {
        let name = name.into();
        let entry = self
            .variables
            .vars
            .entry(name)
            .or_insert_with(|| FitVariable::new(0.0, true));
        entry.min = Some(min);
        entry.max = Some(max);
        self
    }

    pub fn fix(mut self, name: impl Into<String>, value: f64) -> Self {
        let name = name.into();
        let entry = self
            .variables
            .vars
            .entry(name)
            .or_insert_with(|| FitVariable::new(value, false));
        entry.value = value;
        entry.init_value = value;
        entry.vary = false;
        entry.expr = None;
        self
    }

    pub fn var_expr(mut self, name: impl Into<String>, expr: impl Into<String>) -> Self {
        let name = name.into();
        let expr = expr.into();
        let entry = self
            .variables
            .vars
            .entry(name)
            .or_insert_with(|| FitVariable::new(0.0, false));
        entry.expr = Some(expr);
        entry.vary = false;
        self
    }

    pub fn params<I>(mut self, params: I) -> Self
    where
        I: IntoIterator<Item = Param>,
    {
        for param in params {
            self.variables
                .vars
                .insert(param.name.clone(), param.to_fit_variable());
        }
        self
    }

    pub fn set_flavor(mut self, flavor: FeffFlavor) -> Self {
        self.flavor = flavor;
        self
    }

    fn collect_default_dataset(&self, datasets: &mut Vec<FeffFitDataset>) {
        let has_data = !self.default_dataset.k.is_empty() || !self.default_dataset.chi.is_empty();
        let has_paths = !self.default_dataset.paths.is_empty();
        if self.has_default && (has_data || has_paths) {
            datasets.push(self.default_dataset.clone());
        }
    }

    fn maybe_infer_symbols_for_expr(
        expr: &str,
        role: ParamRole,
        inferred: &mut BTreeMap<String, (ParamRole, f64)>,
        warnings: &mut Vec<FitWarning>,
    ) -> Result<(), FittingError> {
        for symbol in try_extract_symbols(expr)? {
            let default_value = role.default_value();
            if let Some((existing_role, existing_default)) = inferred.get(&symbol) {
                if (*existing_default - default_value).abs() > f64::EPSILON {
                    warnings.push(FitWarning {
                        symbol: symbol.clone(),
                        inferred_from: existing_role.as_str().to_string(),
                        default_value: *existing_default,
                        message: format!(
                            "conflicting inferred defaults: '{}'={} vs '{}'={}, kept first",
                            existing_role.as_str(),
                            existing_default,
                            role.as_str(),
                            default_value
                        ),
                    });
                }
                continue;
            }
            inferred.insert(symbol, (role, default_value));
        }
        Ok(())
    }

    fn auto_discover_variables(
        &self,
        datasets: &[FeffFitDataset],
        vars: &mut FitVariables,
        warnings: &mut Vec<FitWarning>,
    ) -> Result<(), FittingError> {
        let mut inferred = BTreeMap::new();

        for dataset in datasets {
            for path in &dataset.paths {
                let path_roles = [
                    (path_param_expr(&path.s02), ParamRole::S02),
                    (path_param_expr(&path.degen), ParamRole::Degen),
                    (path_param_expr(&path.sigma2), ParamRole::Sigma2),
                    (path_param_expr(&path.e0), ParamRole::E0),
                    (path_param_expr(&path.ei), ParamRole::Ei),
                    (path_param_expr(&path.deltar), ParamRole::Deltar),
                    (path_param_expr(&path.third), ParamRole::Third),
                    (path_param_expr(&path.fourth), ParamRole::Fourth),
                ];

                for (expr, role) in path_roles {
                    if let Some(expr) = expr {
                        Self::maybe_infer_symbols_for_expr(expr, role, &mut inferred, warnings)?;
                    }
                }
            }
        }

        for (symbol, (role, default_value)) in inferred.iter() {
            if vars.vars.contains_key(symbol) {
                continue;
            }
            vars.insert(symbol.clone(), FitVariable::new(*default_value, true));
            warnings.push(FitWarning {
                symbol: symbol.clone(),
                inferred_from: role.as_str().to_string(),
                default_value: *default_value,
                message: "auto-created symbol with typed default".to_string(),
            });
        }

        let var_exprs = vars
            .vars
            .iter()
            .filter_map(|(name, var)| var.expr.as_ref().map(|expr| (name.clone(), expr.clone())))
            .collect::<Vec<_>>();
        for (_name, expr) in var_exprs {
            for symbol in try_extract_symbols(&expr)? {
                if vars.vars.contains_key(&symbol) {
                    continue;
                }
                let default_value = ParamRole::UntypedExpr.default_value();
                vars.insert(symbol.clone(), FitVariable::new(default_value, true));
                warnings.push(FitWarning {
                    symbol,
                    inferred_from: ParamRole::UntypedExpr.as_str().to_string(),
                    default_value,
                    message: "auto-created symbol with untyped expression default".to_string(),
                });
            }
        }

        Ok(())
    }

    pub fn fit(&self) -> Result<FeffFitResult, FittingError> {
        let _ = self.flavor;
        let mut datasets = self.datasets.clone();
        self.collect_default_dataset(&mut datasets);
        if datasets.is_empty() {
            return Err(FittingError::InvalidDataset {
                reason: "fit requires at least one dataset".to_string(),
            });
        }

        let mut vars = self.variables.clone();
        let mut warnings = Vec::new();
        self.auto_discover_variables(&datasets, &mut vars, &mut warnings)?;

        let mut result = solver::feffit_multi(&datasets, &vars)?;
        result.warnings.extend(warnings);
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xafs::fitting::ff2chi;
    use crate::xafs::fitting::path_model::feffpath;
    use crate::xafs::fitting::types::{FeffFlavor, PathParamSpec};
    use crate::xafs::tests::TOP_DIR;

    #[test]
    fn test_auto_discovery_uses_typed_defaults() {
        let path = FeffPathModel::default()
            .set_s02("amp")
            .set_sigma2("sig2")
            .set_e0("de0")
            .set_deltar("dr");
        let dataset = FeffFitDataset::new()
            .data(&DVector::zeros(3), &DVector::zeros(3))
            .add_path(path);
        let fit = FeffFit::new().add_dataset(dataset);

        let mut vars = FitVariables::new();
        let mut warnings = Vec::new();
        fit.auto_discover_variables(&fit.datasets, &mut vars, &mut warnings)
            .unwrap();

        assert_eq!(vars.get("amp").unwrap().value, 1.0);
        assert_eq!(vars.get("sig2").unwrap().value, 0.003);
        assert_eq!(vars.get("de0").unwrap().value, 0.0);
        assert_eq!(vars.get("dr").unwrap().value, 0.0);
        assert!(!warnings.is_empty());
    }

    #[test]
    fn test_auto_discovery_warns_on_conflicting_defaults() {
        let path = FeffPathModel::default()
            .set_s02("shared")
            .set_sigma2("shared");
        let dataset = FeffFitDataset::new()
            .data(&DVector::zeros(3), &DVector::zeros(3))
            .add_path(path);
        let fit = FeffFit::new().add_dataset(dataset);

        let mut vars = FitVariables::new();
        let mut warnings = Vec::new();
        fit.auto_discover_variables(&fit.datasets, &mut vars, &mut warnings)
            .unwrap();

        assert_eq!(vars.get("shared").unwrap().value, 1.0);
        assert!(warnings.iter().any(|w| w.message.contains("conflicting")));
    }

    #[test]
    fn test_auto_discovery_untyped_expr_defaults_to_zero() {
        let fit = FeffFit::new().var_expr("scale", "missing * 2");
        let mut vars = fit.variables.clone();
        let mut warnings = Vec::new();
        fit.auto_discover_variables(&[], &mut vars, &mut warnings)
            .unwrap();

        assert_eq!(vars.get("missing").unwrap().value, 0.0);
        assert!(warnings
            .iter()
            .any(|w| w.inferred_from == ParamRole::UntypedExpr.as_str()));
    }

    #[test]
    fn test_builder_single_dataset_fit() {
        let pathfile = format!("{TOP_DIR}/tests/testfiles/feffcu01.dat");
        let path = feffpath(pathfile, FeffFlavor::Feff85L)
            .unwrap()
            .set_s02("amp")
            .set_e0("de0")
            .set_sigma2("sig2")
            .set_deltar("dr");

        let k = DVector::from_iterator(260, (0..260).map(|i| 0.05 * (i as f64 + 1.0)));
        let mut truth = FitVariables::new();
        truth.insert("amp", FitVariable::new(0.9, false));
        truth.insert("de0", FitVariable::new(1.1, false));
        truth.insert("sig2", FitVariable::new(0.0030, false));
        truth.insert("dr", FitVariable::new(0.01, false));

        let synthetic = ff2chi(&[path.clone()], &truth, &k).unwrap();
        let result = FeffFit::new()
            .data(&k, &synthetic.chi)
            .add_path(path)
            .set_inits([("amp", 0.95), ("de0", 0.0), ("sig2", 0.002), ("dr", 0.0)])
            .set_bounds("sig2", 0.0, 0.02)
            .krange(2.0, 14.0)
            .rrange(1.0, 3.0)
            .fit()
            .unwrap();

        assert!(result.chi_square.is_finite());
        assert_eq!(result.datasets.len(), 1);
        assert!(!result.path_contributions.is_empty());
    }

    #[test]
    fn test_builder_multi_dataset_fit() {
        let pathfile = format!("{TOP_DIR}/tests/testfiles/feffcu01.dat");
        let mut path1 = feffpath(pathfile.clone(), FeffFlavor::Feff85L).unwrap();
        path1.s02 = PathParamSpec::Expression("amp".to_string());
        path1.e0 = PathParamSpec::Expression("de0".to_string());
        path1.sigma2 = PathParamSpec::Expression("sig2".to_string());
        path1.deltar = PathParamSpec::Expression("dr".to_string());
        let path2 = path1.clone().set_sigma2("sig2_2");

        let k1 = DVector::from_iterator(220, (0..220).map(|i| 0.05 * (i as f64 + 1.0)));
        let k2 = DVector::from_iterator(240, (0..240).map(|i| 0.05 * (i as f64 + 1.0)));

        let mut truth = FitVariables::new();
        truth.insert("amp", FitVariable::new(0.9, false));
        truth.insert("de0", FitVariable::new(1.2, false));
        truth.insert("sig2", FitVariable::new(0.003, false));
        truth.insert("sig2_2", FitVariable::new(0.004, false));
        truth.insert("dr", FitVariable::new(0.01, false));

        let chi1 = ff2chi(&[path1.clone()], &truth, &k1).unwrap().chi;
        let chi2 = ff2chi(&[path2.clone()], &truth, &k2).unwrap().chi;
        let ds1 = FeffFitDataset::new()
            .data(&k1, &chi1)
            .add_path(path1)
            .krange(2.0, 14.0)
            .rrange(1.0, 3.0);
        let ds2 = FeffFitDataset::new()
            .data(&k2, &chi2)
            .add_path(path2)
            .krange(2.0, 14.0)
            .rrange(1.0, 3.0);

        let result = FeffFit::new()
            .add_dataset(ds1)
            .add_dataset(ds2)
            .set_inits([
                ("amp", 0.95),
                ("de0", 0.0),
                ("sig2", 0.002),
                ("sig2_2", 0.002),
                ("dr", 0.0),
            ])
            .fit()
            .unwrap();

        assert!(result.chi_square.is_finite());
        assert_eq!(result.datasets.len(), 2);
    }
}
