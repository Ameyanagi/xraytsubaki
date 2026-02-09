use std::collections::{BTreeMap, HashMap, HashSet};

use nalgebra::DVector;

use super::errors::FittingError;
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

        Ok(BTreeMap::from_iter(resolved.into_iter()))
    }
}

pub fn resolve_path_param(
    spec: &PathParamSpec,
    default: f64,
    globals: &BTreeMap<String, f64>,
    locals: &BTreeMap<String, f64>,
) -> Result<f64, FittingError> {
    match spec {
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
    }
    .map(|value| if value.is_finite() { value } else { default })
}

pub fn eval_expression_with<F>(expr: &str, resolver: F) -> Result<f64, FittingError>
where
    F: FnMut(&str) -> Result<f64, FittingError>,
{
    let mut parser = ExprParser::new(expr, resolver);
    let value = parser.parse_expression()?;
    parser.skip_whitespace();
    if parser.pos < parser.input.len() {
        return Err(FittingError::ExpressionFailed {
            expr: expr.to_string(),
            reason: format!("unexpected token at byte {}", parser.pos),
        });
    }
    if !value.is_finite() {
        return Err(FittingError::ExpressionFailed {
            expr: expr.to_string(),
            reason: "expression produced non-finite value".to_string(),
        });
    }
    Ok(value)
}

struct ExprParser<'a, F>
where
    F: FnMut(&str) -> Result<f64, FittingError>,
{
    input: &'a str,
    pos: usize,
    resolver: F,
}

impl<'a, F> ExprParser<'a, F>
where
    F: FnMut(&str) -> Result<f64, FittingError>,
{
    fn new(input: &'a str, resolver: F) -> Self {
        Self {
            input,
            pos: 0,
            resolver,
        }
    }

    fn parse_expression(&mut self) -> Result<f64, FittingError> {
        self.parse_add_sub()
    }

    fn parse_add_sub(&mut self) -> Result<f64, FittingError> {
        let mut value = self.parse_mul_div()?;
        loop {
            self.skip_whitespace();
            if self.consume_char('+') {
                value += self.parse_mul_div()?;
            } else if self.consume_char('-') {
                value -= self.parse_mul_div()?;
            } else {
                break;
            }
        }
        Ok(value)
    }

    fn parse_mul_div(&mut self) -> Result<f64, FittingError> {
        let mut value = self.parse_pow()?;
        loop {
            self.skip_whitespace();
            if self.consume_char('*') {
                value *= self.parse_pow()?;
            } else if self.consume_char('/') {
                let rhs = self.parse_pow()?;
                if rhs.abs() < f64::EPSILON {
                    return Err(FittingError::ExpressionFailed {
                        expr: self.input.to_string(),
                        reason: "division by zero".to_string(),
                    });
                }
                value /= rhs;
            } else {
                break;
            }
        }
        Ok(value)
    }

    fn parse_pow(&mut self) -> Result<f64, FittingError> {
        let lhs = self.parse_unary()?;
        self.skip_whitespace();
        if self.consume_char('^') {
            let rhs = self.parse_pow()?;
            Ok(lhs.powf(rhs))
        } else {
            Ok(lhs)
        }
    }

    fn parse_unary(&mut self) -> Result<f64, FittingError> {
        self.skip_whitespace();
        if self.consume_char('+') {
            return self.parse_unary();
        }
        if self.consume_char('-') {
            return self.parse_unary().map(|value| -value);
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<f64, FittingError> {
        self.skip_whitespace();

        if self.consume_char('(') {
            let value = self.parse_expression()?;
            self.skip_whitespace();
            if !self.consume_char(')') {
                return Err(FittingError::ExpressionFailed {
                    expr: self.input.to_string(),
                    reason: "missing closing ')'".to_string(),
                });
            }
            return Ok(value);
        }

        if let Some(number) = self.try_parse_number()? {
            return Ok(number);
        }

        let ident = self.try_parse_identifier();
        if let Some(name) = ident {
            return (&mut self.resolver)(&name).map_err(|err| match err {
                FittingError::UndefinedSymbol { .. } | FittingError::CyclicExpression { .. } => err,
                other => FittingError::ExpressionFailed {
                    expr: self.input.to_string(),
                    reason: other.to_string(),
                },
            });
        }

        Err(FittingError::ExpressionFailed {
            expr: self.input.to_string(),
            reason: format!("unexpected token at byte {}", self.pos),
        })
    }

    fn try_parse_number(&mut self) -> Result<Option<f64>, FittingError> {
        self.skip_whitespace();
        let bytes = self.input.as_bytes();
        if self.pos >= bytes.len() {
            return Ok(None);
        }

        let starts_number = matches!(bytes[self.pos] as char, '0'..='9' | '.');
        if !starts_number {
            return Ok(None);
        }

        let start = self.pos;
        let mut seen_exp = false;

        while self.pos < bytes.len() {
            let c = bytes[self.pos] as char;
            match c {
                '0'..='9' | '.' => self.pos += 1,
                'e' | 'E' if !seen_exp => {
                    seen_exp = true;
                    self.pos += 1;
                    if self.pos < bytes.len() {
                        let sign = bytes[self.pos] as char;
                        if sign == '+' || sign == '-' {
                            self.pos += 1;
                        }
                    }
                }
                _ => break,
            }
        }

        let raw = &self.input[start..self.pos];
        raw.parse::<f64>()
            .map(Some)
            .map_err(|_| FittingError::ExpressionFailed {
                expr: self.input.to_string(),
                reason: format!("invalid numeric literal '{raw}'"),
            })
    }

    fn try_parse_identifier(&mut self) -> Option<String> {
        self.skip_whitespace();
        let bytes = self.input.as_bytes();
        if self.pos >= bytes.len() {
            return None;
        }

        let first = bytes[self.pos] as char;
        if !(first.is_ascii_alphabetic() || first == '_') {
            return None;
        }

        let start = self.pos;
        self.pos += 1;
        while self.pos < bytes.len() {
            let c = bytes[self.pos] as char;
            if c.is_ascii_alphanumeric() || c == '_' {
                self.pos += 1;
            } else {
                break;
            }
        }

        Some(self.input[start..self.pos].to_string())
    }

    fn skip_whitespace(&mut self) {
        let bytes = self.input.as_bytes();
        while self.pos < bytes.len() && (bytes[self.pos] as char).is_ascii_whitespace() {
            self.pos += 1;
        }
    }

    fn consume_char(&mut self, expected: char) -> bool {
        self.skip_whitespace();
        let bytes = self.input.as_bytes();
        if self.pos < bytes.len() && bytes[self.pos] as char == expected {
            self.pos += 1;
            true
        } else {
            false
        }
    }
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
}
