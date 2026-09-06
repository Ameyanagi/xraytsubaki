use std::collections::{BTreeMap, HashMap, HashSet};

use nalgebra::DVector;

use super::errors::FittingError;
use super::expression;
use super::types::{FitVariable, FitVariables, PathParamSpec};

impl FitVariables {
    pub fn varying_names(&self) -> Vec<String> {
        self.vars
            .iter()
            .filter(|(_, var)| var.vary && var.expr.is_none())
            .map(|(name, _)| name.clone())
            .collect()
    }

    pub fn parameter_vector(&self, names: &[String]) -> DVector<f64> {
        DVector::from_iterator(
            names.len(),
            names
                .iter()
                .map(|name| self.vars.get(name).map(|var| var.value).unwrap_or(0.0)),
        )
    }

    pub fn apply_parameter_vector(
        &mut self,
        names: &[String],
        values: &DVector<f64>,
    ) -> Result<(), FittingError> {
        if names.len() != values.len() {
            return Err(FittingError::InvalidDataset {
                reason: format!(
                    "parameter vector length mismatch: names={}, values={}",
                    names.len(),
                    values.len()
                ),
            });
        }
        for (index, name) in names.iter().enumerate() {
            if let Some(var) = self.vars.get_mut(name) {
                var.value = var.clamp(values[index]);
            }
        }
        Ok(())
    }

    pub fn resolve_values(&self) -> Result<BTreeMap<String, f64>, FittingError> {
        let mut resolved: HashMap<String, f64> = HashMap::new();
        let mut visiting: HashSet<String> = HashSet::new();

        fn resolve_one(
            name: &str,
            vars: &BTreeMap<String, FitVariable>,
            resolved: &mut HashMap<String, f64>,
            visiting: &mut HashSet<String>,
        ) -> Result<f64, FittingError> {
            if let Some(value) = resolved.get(name) {
                return Ok(*value);
            }
            if !vars.contains_key(name) {
                return Err(FittingError::UndefinedSymbol {
                    symbol: name.to_string(),
                });
            }
            if !visiting.insert(name.to_string()) {
                return Err(FittingError::CyclicExpression {
                    symbol: name.to_string(),
                });
            }

            let variable = vars.get(name).expect("checked contains_key");
            let value = if let Some(expr) = variable.expr.as_ref() {
                eval_expression_with(expr, |symbol| {
                    if symbol == name {
                        return Err(FittingError::CyclicExpression {
                            symbol: symbol.to_string(),
                        });
                    }
                    resolve_one(symbol, vars, resolved, visiting)
                })?
            } else {
                variable.value
            };

            visiting.remove(name);
            resolved.insert(name.to_string(), value);
            Ok(value)
        }

        for name in self.vars.keys() {
            resolve_one(name, &self.vars, &mut resolved, &mut visiting)?;
        }

        Ok(BTreeMap::from_iter(resolved))
    }
}

pub fn resolve_path_param(
    spec: &PathParamSpec,
    _default: f64,
    globals: &BTreeMap<String, f64>,
    locals: &BTreeMap<String, f64>,
) -> Result<f64, FittingError> {
    let value = match spec {
        PathParamSpec::Value(value) => Ok(*value),
        PathParamSpec::Expression(expr) => eval_expression_with(expr, |symbol| {
            locals
                .get(symbol)
                .copied()
                .or_else(|| globals.get(symbol).copied())
                .ok_or_else(|| FittingError::UndefinedSymbol {
                    symbol: symbol.to_string(),
                })
        }),
    }?;

    if value.is_finite() {
        return Ok(value);
    }

    Err(FittingError::InvalidDataset {
        reason: match spec {
            PathParamSpec::Value(_) => "path parameter literal value is non-finite".to_string(),
            PathParamSpec::Expression(expr) => {
                format!("path parameter expression '{expr}' resolved to non-finite value")
            }
        },
    })
}

pub fn eval_expression_with<F>(expr: &str, resolver: F) -> Result<f64, FittingError>
where
    F: FnMut(&str) -> Result<f64, FittingError>,
{
    expression::eval_expression_with(expr, resolver)
}

pub fn extract_symbols(expr: &str) -> Vec<String> {
    expression::extract_symbols(expr)
}

pub fn try_extract_symbols(expr: &str) -> Result<Vec<String>, FittingError> {
    expression::try_extract_symbols(expr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eval_expression_math_and_symbols() {
        let mut symbols = BTreeMap::new();
        symbols.insert("a".to_string(), 2.0);
        symbols.insert("b".to_string(), 3.0);

        let value = eval_expression_with("a + b*2 - (4/2)", |name| {
            symbols
                .get(name)
                .copied()
                .ok_or_else(|| FittingError::UndefinedSymbol {
                    symbol: name.to_string(),
                })
        })
        .unwrap();

        assert!((value - 6.0).abs() < 1.0e-12);
    }

    #[test]
    fn test_eval_expression_rejects_unknown_symbol() {
        let err = eval_expression_with("a + z", |name| {
            if name == "a" {
                Ok(1.0)
            } else {
                Err(FittingError::UndefinedSymbol {
                    symbol: name.to_string(),
                })
            }
        })
        .unwrap_err();

        assert!(matches!(err, FittingError::UndefinedSymbol { .. }));
    }

    #[test]
    fn test_fit_variables_resolve_expression_graph() {
        let mut vars = FitVariables::new();
        vars.insert("amp", FitVariable::new(0.9, true));
        vars.insert("sig2", FitVariable::new(0.003, true));
        vars.insert(
            "sig2_scale",
            FitVariable::new(0.0, false).with_expr("sig2 * 2"),
        );

        let resolved = vars.resolve_values().unwrap();
        assert!((resolved["sig2_scale"] - 0.006).abs() < 1.0e-12);
    }

    #[test]
    fn test_fit_variables_detect_cycle() {
        let mut vars = FitVariables::new();
        vars.insert("a", FitVariable::new(0.0, false).with_expr("b"));
        vars.insert("b", FitVariable::new(0.0, false).with_expr("a"));

        let err = vars.resolve_values().unwrap_err();
        assert!(matches!(err, FittingError::CyclicExpression { .. }));
    }

    #[test]
    fn test_extract_symbols_excludes_reff() {
        let symbols = extract_symbols("amp * sqrt(reff) + sig2");
        assert_eq!(symbols, vec!["amp".to_string(), "sig2".to_string()]);
    }

    #[test]
    fn test_resolve_path_param_rejects_non_finite_literal() {
        let globals = BTreeMap::new();
        let locals = BTreeMap::new();
        let err = resolve_path_param(&PathParamSpec::Value(f64::NAN), 0.0, &globals, &locals)
            .unwrap_err();
        assert!(matches!(err, FittingError::InvalidDataset { .. }));
    }
}
