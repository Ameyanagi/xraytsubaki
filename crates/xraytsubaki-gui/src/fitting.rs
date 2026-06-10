//! GUI-side FEFF fitting: plain-data specs assembled in the Fit panel are
//! turned into a core `FeffFit` and run on the background executor.

use std::path::PathBuf;

use nalgebra::DVector;
use xraytsubaki::prelude::*;

/// One imported FEFF path. Param fields hold either a number ("0.9") or a
/// variable/expression name ("amp"), mirroring `PathParamSpec`.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct FitPathSpec {
    pub file: PathBuf,
    pub label: String,
    pub s02: String,
    pub e0: String,
    pub sigma2: String,
    pub deltar: String,
    pub enabled: bool,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
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

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(default)]
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

/// Athena-style path metadata for the selector list.
#[derive(Clone, Copy)]
pub struct PathMeta {
    pub reff: f64,
    pub degen: f64,
    pub nleg: usize,
}

pub fn path_meta(file: &std::path::Path) -> Option<PathMeta> {
    let model = feffpath(file.to_string_lossy().as_ref(), FeffFlavor::Feff85L).ok()?;
    Some(PathMeta {
        reff: model.feff.reff,
        degen: model.feff.degen,
        nleg: model.feff.nleg,
    })
}

/// One frame's result in a batch fit over a scan.
#[derive(Clone)]
pub struct BatchFitRow {
    /// Position within the sampled frame sequence.
    pub frame: usize,
    /// Catalog entry index (for file names).
    pub entry_ix: usize,
    pub r_factor: f64,
    /// (name, value, stderr) for each varying parameter.
    pub values: Vec<(String, f64, Option<f64>)>,
}

impl BatchFitRow {
    pub fn from_result(frame: usize, entry_ix: usize, result: &FeffFitResult) -> Self {
        let values = result
            .varying_names
            .iter()
            .filter_map(|name| {
                result
                    .variables
                    .get(name)
                    .map(|v| (name.clone(), v.value, v.stderr))
            })
            .collect();
        Self {
            frame,
            entry_ix,
            r_factor: result.r_factor,
            values,
        }
    }

    pub fn value_of(&self, name: &str) -> Option<f64> {
        self.values
            .iter()
            .find(|(n, _, _)| n == name)
            .map(|(_, v, _)| *v)
    }
}

/// CSV for a batch-fit result set: frame, file, r_factor, then one
/// value/stderr column pair per varying parameter.
pub fn batch_csv(rows: &[BatchFitRow], names: &[String], files: &[String]) -> String {
    let mut out = String::from("frame,file,r_factor");
    for name in names {
        out.push_str(&format!(",{name},{name}_stderr"));
    }
    out.push('\n');
    for row in rows {
        let file = files.get(row.frame).map(String::as_str).unwrap_or("");
        out.push_str(&format!("{},{file},{:.6}", row.frame, row.r_factor));
        for name in names {
            match row.values.iter().find(|(n, _, _)| n == name) {
                Some((_, v, Some(err))) => out.push_str(&format!(",{v:.6},{err:.6}")),
                Some((_, v, None)) => out.push_str(&format!(",{v:.6},")),
                None => out.push_str(",,"),
            }
        }
        out.push('\n');
    }
    out
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feffgen::{CrystalSpec, new_workspace_from_spec, run_feff10};
    use crate::params::{PipelineParams, process_file};

    /// End-to-end check of the GUI fit flow (Generate -> Run FEFF10 -> Run
    /// Fit) on real data: first Ru-Ru shell vs the Ru K-edge test spectrum.
    /// Note: Ru_QAS.dat is not clean bulk Ru metal (large de0/dr when forced
    /// onto hcp Ru paths), so amp is held fixed and the R-factor bound is
    /// loose — this validates plumbing and convergence, not a publication
    /// fit.
    #[test]
    fn ru_hcp_paths_fit_ru_spectrum() {
        let _guard = crate::feffgen::feff_test_lock();
        let spec = CrystalSpec {
            element: "Ru".into(),
            element2: None,
            structure: "hcp".into(),
            a: 2.706,
            c: Some(4.282),
            edge: "K".into(),
            rmax: 5.0,
        };
        let ws = new_workspace_from_spec(&spec).expect("workspace");
        let mut files = run_feff10(&ws).expect("feff10");
        files.truncate(1); // first Ru-Ru shell only

        let data = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../xraytsubaki/tests/testfiles/Ru_QAS.dat");
        let sp = process_file(&data, &PipelineParams::default()).expect("pipeline");
        let (k, chi) = (sp.get_k().unwrap(), sp.get_chi().unwrap());

        let mut paths = Vec::new();
        let mut vars = vec![
            FitVarSpec { name: "amp".into(), value: 0.9, vary: false, expr: None },
            FitVarSpec { name: "de0".into(), value: 0.0, vary: true, expr: None },
        ];
        for (i, file) in files.iter().enumerate() {
            let n = i + 1;
            paths.push(FitPathSpec {
                file: file.clone(),
                label: format!("path{n}"),
                s02: "amp".into(),
                e0: "de0".into(),
                sigma2: format!("sig2_{n}"),
                deltar: format!("dr_{n}"),
                enabled: true,
            });
            vars.push(FitVarSpec { name: format!("sig2_{n}"), value: 0.003, vary: true, expr: None });
            vars.push(FitVarSpec { name: format!("dr_{n}"), value: 0.0, vary: true, expr: None });
        }

        let ranges = FitRanges {
            kmin: 3.0,
            kmax: 12.0,
            rmin: 1.8,
            rmax: 3.0,
            kweight: 2.0,
        };
        let result = run_fit(k, chi, &paths, &vars, ranges).expect("fit");
        println!("Ru hcp fit: R-factor {:.4}", result.r_factor);
        for line in result_summary(&result) {
            println!("  {line}");
        }
        assert!(
            result.r_factor < 0.35,
            "first-shell fit should describe the metal shell (R-factor {})",
            result.r_factor
        );
    }
}
