//! GUI-side FEFF fitting: plain-data specs assembled in the Fit panel are
//! turned into a core `FeffFit` and run on the background executor.

use std::path::PathBuf;

use nalgebra::DVector;
use xraytsubaki::prelude::*;

/// One imported FEFF path. Param fields hold either a number ("0.9") or a
/// variable/expression name ("amp"), mirroring `PathParamSpec`.
#[derive(Clone)]
pub struct FitPathSpec {
    pub file: PathBuf,
    pub label: String,
    pub s02: String,
    pub e0: String,
    pub sigma2: String,
    pub deltar: String,
    pub enabled: bool,
}

#[derive(Clone)]
pub struct FitVarSpec {
    pub name: String,
    pub value: f64,
    pub vary: bool,
    /// When set, the variable is defined by this expression (derived, not
    /// varied), e.g. `sig2_1 * 2`.
    pub expr: Option<String>,
}

/// Identifiers referenced by an expression, excluding builtins/functions, in
/// order of appearance. Used to auto-create variables.
pub fn expr_identifiers(expr: &str) -> Vec<String> {
    const RESERVED: &[&str] = &[
        "reff", "degen", "pi", "e", "k", "sin", "cos", "tan", "asin", "acos", "atan", "exp",
        "ln", "log", "log10", "sqrt", "abs", "min", "max", "pow",
    ];
    let mut out: Vec<String> = Vec::new();
    let mut current = String::new();
    for c in expr.chars().chain(std::iter::once(' ')) {
        if c.is_ascii_alphanumeric() || c == '_' {
            current.push(c);
        } else if !current.is_empty() {
            let ident = std::mem::take(&mut current);
            if !ident.chars().next().unwrap().is_ascii_digit()
                && !RESERVED.contains(&ident.as_str())
                && !out.contains(&ident)
            {
                out.push(ident);
            }
        }
    }
    out
}

#[derive(Clone, Copy)]
pub struct FitRanges {
    pub kmin: f64,
    pub kmax: f64,
    pub rmin: f64,
    pub rmax: f64,
    pub kweight: f64,
}

impl Default for FitRanges {
    fn default() -> Self {
        Self {
            kmin: 2.0,
            kmax: 12.0,
            rmin: 1.0,
            rmax: 3.0,
            kweight: 2.0,
        }
    }
}

fn spec(text: &str) -> PathParamSpec {
    let text = text.trim();
    match text.parse::<f64>() {
        Ok(v) => PathParamSpec::Value(v),
        Err(_) => PathParamSpec::Expression(text.to_string()),
    }
}

pub fn run_fit(
    k: DVector<f64>,
    chi: DVector<f64>,
    paths: &[FitPathSpec],
    vars: &[FitVarSpec],
    ranges: FitRanges,
) -> Result<FeffFitResult, String> {
    let mut fit = FeffFit::new()
        .data(&k, &chi)
        .krange(ranges.kmin, ranges.kmax)
        .rrange(ranges.rmin, ranges.rmax)
        .kweight(ranges.kweight);
    let mut any = false;
    for p in paths.iter().filter(|p| p.enabled) {
        let model = feffpath(&p.file.to_string_lossy(), FeffFlavor::Feff85L)
            .map_err(|e| format!("{}: {e}", p.label))?
            .set_s02(spec(&p.s02))
            .set_e0(spec(&p.e0))
            .set_sigma2(spec(&p.sigma2))
            .set_deltar(spec(&p.deltar));
        fit = fit.add_path(model);
        any = true;
    }
    if !any {
        return Err("no enabled paths".into());
    }
    for v in vars {
        fit = match &v.expr {
            Some(expr) => fit.var_expr(&v.name, expr),
            None if v.vary => fit.set_init(&v.name, v.value),
            None => fit.fix(&v.name, v.value),
        };
    }
    fit.fit().map_err(|e| e.to_string())
}

/// "value ± stderr" lines for the varying parameters, plus fit statistics.
pub fn result_summary(result: &FeffFitResult) -> Vec<String> {
    let mut lines = vec![
        format!("R-factor  {:.5}", result.r_factor),
        format!("red. chi²  {:.4e}", result.reduced_chi_square),
        format!("n_idp {:.1} · n_vary {}", result.n_idp, result.n_vary),
    ];
    for name in &result.varying_names {
        if let Some(var) = result.variables.get(name) {
            match var.stderr {
                Some(err) => lines.push(format!("{name} = {:.5} ± {err:.5}", var.value)),
                None => lines.push(format!("{name} = {:.5}", var.value)),
            }
        }
    }
    for warning in &result.warnings {
        lines.push(format!("⚠ {warning:?}"));
    }
    lines
}
