use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};

use pest::iterators::Pair;
use pest::Parser;
use pest_derive::Parser;

use super::errors::FittingError;

#[derive(Parser)]
#[grammar = "xafs/fitting/expression.pest"]
struct ExprGrammar;

#[derive(Debug, Clone)]
enum UnaryOp {
    Neg,
}

#[derive(Debug, Clone)]
enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
}

#[derive(Debug, Clone)]
enum Expr {
    Number(f64),
    Symbol(String),
    Unary {
        op: UnaryOp,
        value: Box<Expr>,
    },
    Binary {
        op: BinaryOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    Call {
        name: String,
        args: Vec<Expr>,
    },
}

static CACHE: OnceLock<Mutex<HashMap<String, Expr>>> = OnceLock::new();

fn parse_error(expr: &str, reason: impl Into<String>) -> FittingError {
    FittingError::ExpressionFailed {
        expr: expr.to_string(),
        reason: reason.into(),
    }
}

fn parse_expr(expr: &str) -> Result<Expr, FittingError> {
    let mut pairs = ExprGrammar::parse(Rule::complete_expression, expr)
        .map_err(|err| parse_error(expr, format!("parse error: {err}")))?;
    let pair = pairs
        .next()
        .ok_or_else(|| parse_error(expr, "empty expression"))?;
    build_expr(expr, pair)
}

fn cache() -> &'static Mutex<HashMap<String, Expr>> {
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn parse_cached(expr: &str) -> Result<Expr, FittingError> {
    let cached = cache()
        .lock()
        .map_err(|_| parse_error(expr, "expression cache lock poisoned"))?
        .get(expr)
        .cloned();
    if let Some(ast) = cached {
        return Ok(ast);
    }

    let parsed = parse_expr(expr)?;
    cache()
        .lock()
        .map_err(|_| parse_error(expr, "expression cache lock poisoned"))?
        .insert(expr.to_string(), parsed.clone());
    Ok(parsed)
}

fn build_expr(source: &str, pair: Pair<Rule>) -> Result<Expr, FittingError> {
    match pair.as_rule() {
        Rule::complete_expression | Rule::expression => {
            let inner = pair
                .into_inner()
                .next()
                .ok_or_else(|| parse_error(source, "missing expression node"))?;
            build_expr(source, inner)
        }
        Rule::sum => build_left_assoc(source, pair, BinaryOp::Add, BinaryOp::Sub),
        Rule::product => build_left_assoc(source, pair, BinaryOp::Mul, BinaryOp::Div),
        Rule::power => {
            let mut inner = pair.into_inner();
            let lhs = inner
                .next()
                .ok_or_else(|| parse_error(source, "missing power lhs"))?;
            let lhs_expr = build_expr(source, lhs)?;
            if let Some(op) = inner.next() {
                if op.as_rule() != Rule::pow_op {
                    return Err(parse_error(source, "invalid power operator"));
                }
                let rhs = inner
                    .next()
                    .ok_or_else(|| parse_error(source, "missing power rhs"))?;
                let rhs_expr = build_expr(source, rhs)?;
                Ok(Expr::Binary {
                    op: BinaryOp::Pow,
                    lhs: Box::new(lhs_expr),
                    rhs: Box::new(rhs_expr),
                })
            } else {
                Ok(lhs_expr)
            }
        }
        Rule::unary => {
            let mut sign_flip = false;
            let mut primary = None;
            for item in pair.into_inner() {
                if item.as_rule() == Rule::sign {
                    if item.as_str() == "-" {
                        sign_flip = !sign_flip;
                    }
                } else {
                    primary = Some(item);
                }
            }
            let primary =
                primary.ok_or_else(|| parse_error(source, "missing primary after unary sign"))?;
            let mut expr = build_expr(source, primary)?;
            if sign_flip {
                expr = Expr::Unary {
                    op: UnaryOp::Neg,
                    value: Box::new(expr),
                };
            }
            Ok(expr)
        }
        Rule::primary => {
            let inner = pair
                .into_inner()
                .next()
                .ok_or_else(|| parse_error(source, "missing primary expression"))?;
            build_expr(source, inner)
        }
        Rule::function_call => {
            let mut inner = pair.into_inner();
            let name = inner
                .next()
                .ok_or_else(|| parse_error(source, "missing function name"))?
                .as_str()
                .to_string();
            let args = if let Some(arglist) = inner.next() {
                if arglist.as_rule() != Rule::expr_list {
                    return Err(parse_error(source, "invalid function arglist"));
                }
                arglist
                    .into_inner()
                    .filter(|arg| arg.as_rule() == Rule::expression)
                    .map(|arg| build_expr(source, arg))
                    .collect::<Result<Vec<_>, _>>()?
            } else {
                Vec::new()
            };
            Ok(Expr::Call { name, args })
        }
        Rule::constant => match pair.as_str() {
            "pi" => Ok(Expr::Number(std::f64::consts::PI)),
            "e" => Ok(Expr::Number(std::f64::consts::E)),
            other => Err(parse_error(
                source,
                format!("unsupported constant '{other}'"),
            )),
        },
        Rule::identifier => Ok(Expr::Symbol(pair.as_str().to_string())),
        Rule::number => {
            let parsed = pair.as_str().parse::<f64>().map_err(|_| {
                parse_error(
                    source,
                    format!("invalid numeric literal '{}'", pair.as_str()),
                )
            })?;
            Ok(Expr::Number(parsed))
        }
        _ => Err(parse_error(
            source,
            format!("unexpected parse rule {:?}", pair.as_rule()),
        )),
    }
}

fn build_left_assoc(
    source: &str,
    pair: Pair<Rule>,
    add_variant: BinaryOp,
    sub_variant: BinaryOp,
) -> Result<Expr, FittingError> {
    let mut inner = pair.into_inner();
    let first = inner
        .next()
        .ok_or_else(|| parse_error(source, "missing lhs expression"))?;
    let mut expr = build_expr(source, first)?;

    while let Some(op) = inner.next() {
        let rhs = inner
            .next()
            .ok_or_else(|| parse_error(source, "missing rhs expression"))?;
        let op = match op.as_str() {
            "+" | "*" => add_variant.clone(),
            "-" | "/" => sub_variant.clone(),
            other => {
                return Err(parse_error(
                    source,
                    format!("unsupported binary operator '{other}'"),
                ));
            }
        };
        expr = Expr::Binary {
            op,
            lhs: Box::new(expr),
            rhs: Box::new(build_expr(source, rhs)?),
        };
    }

    Ok(expr)
}

fn eval_call(expr: &str, name: &str, args: &[f64]) -> Result<f64, FittingError> {
    let one_arg = |f: fn(f64) -> f64| -> Result<f64, FittingError> {
        if args.len() != 1 {
            return Err(parse_error(
                expr,
                format!("function '{name}' expects 1 argument, got {}", args.len()),
            ));
        }
        Ok(f(args[0]))
    };
    let two_arg = |f: fn(f64, f64) -> f64| -> Result<f64, FittingError> {
        if args.len() != 2 {
            return Err(parse_error(
                expr,
                format!("function '{name}' expects 2 arguments, got {}", args.len()),
            ));
        }
        Ok(f(args[0], args[1]))
    };

    match name {
        "abs" => one_arg(f64::abs),
        "exp" => one_arg(f64::exp),
        "log" => one_arg(f64::ln),
        "log10" => one_arg(f64::log10),
        "sqrt" => one_arg(f64::sqrt),
        "sin" => one_arg(f64::sin),
        "cos" => one_arg(f64::cos),
        "tan" => one_arg(f64::tan),
        "asin" => one_arg(f64::asin),
        "acos" => one_arg(f64::acos),
        "atan" => one_arg(f64::atan),
        "sinh" => one_arg(f64::sinh),
        "cosh" => one_arg(f64::cosh),
        "tanh" => one_arg(f64::tanh),
        "ceil" => one_arg(f64::ceil),
        "floor" => one_arg(f64::floor),
        "round" => one_arg(f64::round),
        "min" => two_arg(f64::min),
        "max" => two_arg(f64::max),
        "atan2" => two_arg(f64::atan2),
        _ => Err(parse_error(expr, format!("unsupported function '{name}'"))),
    }
}

fn eval_ast<F>(expr: &str, ast: &Expr, resolver: &mut F) -> Result<f64, FittingError>
where
    F: FnMut(&str) -> Result<f64, FittingError>,
{
    match ast {
        Expr::Number(v) => Ok(*v),
        Expr::Symbol(name) => resolver(name).map_err(|err| match err {
            FittingError::UndefinedSymbol { .. } | FittingError::CyclicExpression { .. } => err,
            other => parse_error(expr, other.to_string()),
        }),
        Expr::Unary { op, value } => {
            let v = eval_ast(expr, value, resolver)?;
            match op {
                UnaryOp::Neg => Ok(-v),
            }
        }
        Expr::Binary { op, lhs, rhs } => {
            let l = eval_ast(expr, lhs, resolver)?;
            let r = eval_ast(expr, rhs, resolver)?;
            match op {
                BinaryOp::Add => Ok(l + r),
                BinaryOp::Sub => Ok(l - r),
                BinaryOp::Mul => Ok(l * r),
                BinaryOp::Div => {
                    if r.abs() < f64::EPSILON {
                        return Err(parse_error(expr, "division by zero"));
                    }
                    Ok(l / r)
                }
                BinaryOp::Pow => Ok(l.powf(r)),
            }
        }
        Expr::Call { name, args } => {
            let evaluated = args
                .iter()
                .map(|arg| eval_ast(expr, arg, resolver))
                .collect::<Result<Vec<_>, _>>()?;
            eval_call(expr, name, &evaluated)
        }
    }
}

pub fn eval_expression_with<F>(expr: &str, mut resolver: F) -> Result<f64, FittingError>
where
    F: FnMut(&str) -> Result<f64, FittingError>,
{
    let ast = parse_cached(expr)?;
    let value = eval_ast(expr, &ast, &mut resolver)?;
    if !value.is_finite() {
        return Err(parse_error(expr, "expression produced non-finite value"));
    }
    Ok(value)
}

fn collect_symbols(ast: &Expr, out: &mut Vec<String>, seen: &mut HashSet<String>) {
    match ast {
        Expr::Number(_) => {}
        Expr::Symbol(symbol) => {
            if symbol == "reff" {
                return;
            }
            if seen.insert(symbol.clone()) {
                out.push(symbol.clone());
            }
        }
        Expr::Unary { value, .. } => collect_symbols(value, out, seen),
        Expr::Binary { lhs, rhs, .. } => {
            collect_symbols(lhs, out, seen);
            collect_symbols(rhs, out, seen);
        }
        Expr::Call { args, .. } => {
            for arg in args {
                collect_symbols(arg, out, seen);
            }
        }
    }
}

pub fn try_extract_symbols(expr: &str) -> Result<Vec<String>, FittingError> {
    let ast = parse_cached(expr)?;
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    collect_symbols(&ast, &mut out, &mut seen);
    Ok(out)
}

pub fn extract_symbols(expr: &str) -> Vec<String> {
    try_extract_symbols(expr).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn test_eval_expression_functions_and_constants() {
        let symbols = BTreeMap::<String, f64>::new();
        let value = eval_expression_with("max(2, sqrt(4)) + sin(pi/2) + log(e)", |name| {
            symbols
                .get(name)
                .copied()
                .ok_or_else(|| FittingError::UndefinedSymbol {
                    symbol: name.to_string(),
                })
        })
        .unwrap();
        assert!((value - 4.0).abs() < 1.0e-12);
    }

    #[test]
    fn test_eval_expression_right_associative_power() {
        let value = eval_expression_with("2^3^2", |_| Ok(0.0)).unwrap();
        assert!((value - 512.0).abs() < 1.0e-12);
    }

    #[test]
    fn test_eval_expression_unknown_function() {
        let err = eval_expression_with("unknown(2.0)", |_| Ok(0.0)).unwrap_err();
        assert!(matches!(err, FittingError::ExpressionFailed { .. }));
    }

    #[test]
    fn test_extract_symbols_excludes_locals_and_constants() {
        let symbols = try_extract_symbols("amp * sqrt(reff) + log(e) + s02").unwrap();
        assert_eq!(symbols, vec!["amp".to_string(), "s02".to_string()]);
    }

    #[test]
    fn variable_names_may_begin_with_constant_names() {
        for name in ["e0", "ei", "energy", "pi_scale"] {
            assert_eq!(eval_expression_with(name, |_| Ok(7.25)).unwrap(), 7.25);
            assert_eq!(try_extract_symbols(name).unwrap(), vec![name]);
        }
        assert_eq!(
            eval_expression_with("(e0 + 2) * pi_scale", |_| Ok(3.)).unwrap(),
            15.
        );
        assert!(
            (eval_expression_with("e + 1", |_| unreachable!()).unwrap() - std::f64::consts::E - 1.)
                .abs()
                < 1e-12
        );
    }

    #[test]
    fn expressions_reject_unparsed_suffixes() {
        for expr in ["1 garbage", "amp +", "e0 ???", "2**3", "(1+2))"] {
            assert!(eval_expression_with(expr, |_| Ok(1.)).is_err(), "{expr}");
        }
        assert_eq!(
            eval_expression_with(" 1e-3 + 2E+1 ", |_| unreachable!()).unwrap(),
            20.001
        );
    }
}
