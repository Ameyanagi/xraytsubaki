//! GUI-side FEFF fitting: plain-data specs assembled in the Fit panel are
//! turned into a core `FeffFit` and run on the background executor.

use std::collections::BTreeMap;
use std::path::PathBuf;

use nalgebra::DVector;
use rexafs::prelude::*;
use rexafs::xafs::fitting::FitSolverReport;

/// Parameter namespaces follow the calculation directories. Different phases
/// must not share distance/disorder (or amplitudes) just because shell numbers
/// coincide. The first source retains legacy names; later ones use p2_, p3_…
pub(crate) fn source_template(
    template: rexafs::xafs::fitting::template::ParameterTemplate,
    selected: &[rexafs::xafs::structure::PathInfo],
    paths: &[FitPathSpec],
) -> rexafs::xafs::fitting::template::TemplateResult {
    use rexafs::xafs::fitting::template::{TemplateResult, apply_template};
    let mut sources = Vec::new();
    for p in paths {
        let s = p.file.parent();
        if !sources.contains(&s) {
            sources.push(s);
        }
    }
    let mut output = TemplateResult::default();
    for (i, source) in sources.iter().enumerate() {
        let selected: Vec<_> = selected
            .iter()
            .filter(|p| paths[p.index].file.parent() == *source)
            .cloned()
            .collect();
        let mut result = apply_template(template, &selected);
        if i > 0 {
            let names: Vec<_> = result.variables.iter().map(|v| v.name.clone()).collect();
            let rename = |expr: &str| -> String {
                // Whole identifier replacement, never substrings (dr_1/dr_10).
                let mut out = String::new();
                let mut token = String::new();
                for c in expr.chars().chain(std::iter::once(' ')) {
                    if c.is_ascii_alphanumeric() || c == '_' {
                        token.push(c);
                    } else {
                        if names.contains(&token) {
                            out.push_str(&format!("p{}_", i + 1));
                        }
                        out.push_str(&token);
                        token.clear();
                        out.push(c);
                    }
                }
                out.trim_end().to_string()
            };
            for v in &mut result.variables {
                v.name = format!("p{}_{}", i + 1, v.name);
                if let Some(e) = &v.expr {
                    v.expr = Some(rename(e));
                }
            }
            for a in &mut result.assignments {
                a.s02 = rename(&a.s02);
                a.e0 = rename(&a.e0);
                a.deltar = rename(&a.deltar);
                a.sigma2 = rename(&a.sigma2);
            }
        }
        output.variables.extend(result.variables);
        output.assignments.extend(result.assignments);
        output.notes.extend(result.notes);
    }
    if sources.len() > 1 {
        output.notes.push("Each structure has independent parameters (p2_, p3_…). Share a variable explicitly in Path expressions when physically appropriate; phase fractions are not inferred.".into());
    }
    output
}

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
    /// Optional cumulant / lifetime terms (empty = the core default of 0).
    #[serde(default)]
    pub ei: String,
    #[serde(default)]
    pub third: String,
    #[serde(default)]
    pub fourth: String,
}

impl FitPathSpec {
    /// A path with empty parameter cells, not yet selected: a parameter
    /// template fills the cells once the path is chosen for the fit.
    pub fn blank(file: PathBuf) -> Self {
        let label = file
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| file.display().to_string());
        Self {
            file,
            label,
            s02: String::new(),
            e0: String::new(),
            sigma2: String::new(),
            deltar: String::new(),
            enabled: false,
            ei: String::new(),
            third: String::new(),
            fourth: String::new(),
        }
    }

    /// Standard parameterization for path number `i` (Artemis: shared amp
    /// and enot, per-path sigma2 and delr).
    pub fn standard(file: PathBuf, i: usize) -> Self {
        let label = file
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| file.display().to_string());
        Self {
            file,
            label,
            s02: "amp".into(),
            e0: "de0".into(),
            sigma2: format!("sig2_{i}"),
            deltar: format!("dr_{i}"),
            enabled: true,
            ei: String::new(),
            third: String::new(),
            fourth: String::new(),
        }
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct FitVarSpec {
    pub name: String,
    pub value: f64,
    pub vary: bool,
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
    /// When set, the variable is defined by this expression (derived, not
    /// varied), e.g. `sig2_1 * 2`.
    pub expr: Option<String>,
}

/// Identifiers referenced by an expression, excluding builtins/functions, in
/// order of appearance. Used to auto-create variables.
pub fn expr_identifiers(expr: &str) -> Vec<String> {
    const RESERVED: &[&str] = &[
        "reff", "degen", "pi", "e", "k", "sin", "cos", "tan", "asin", "acos", "atan", "exp", "ln",
        "log", "log10", "sqrt", "abs", "min", "max", "pow",
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

/// Space the residual is evaluated in.
#[derive(
    Clone, Copy, PartialEq, Eq, Hash, Debug, Default, serde::Serialize, serde::Deserialize,
)]
pub enum FitSpaceSpec {
    #[default]
    R,
    K,
    Q,
}

impl FitSpaceSpec {
    pub const ALL: [FitSpaceSpec; 3] = [FitSpaceSpec::R, FitSpaceSpec::K, FitSpaceSpec::Q];

    pub fn label(self) -> &'static str {
        match self {
            FitSpaceSpec::R => "R",
            FitSpaceSpec::K => "k",
            FitSpaceSpec::Q => "q",
        }
    }

    fn core(self) -> FitSpace {
        match self {
            FitSpaceSpec::R => FitSpace::R,
            FitSpaceSpec::K => FitSpace::K,
            FitSpaceSpec::Q => FitSpace::Q,
        }
    }
}

#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct FitRanges {
    /// New models follow the preprocessing transform; old projects retain
    /// their explicitly saved weights when this field is absent.
    #[serde(default)]
    pub follow_transform: bool,
    pub kmin: f64,
    pub kmax: f64,
    pub rmin: f64,
    pub rmax: f64,
    /// Plot / primary k-weight.
    pub kweight: f64,
    /// k-weights fit simultaneously (Artemis default 1, 2, 3). Empty means
    /// `kweight` alone.
    pub kweights: Vec<f64>,
    pub fitspace: FitSpaceSpec,
    /// Scale each residual block by the noise estimated from the high-R
    /// part of χ(R) (Larch `estimate_noise`), so χ² is meaningful.
    pub noise: bool,
}

impl Default for FitRanges {
    fn default() -> Self {
        Self {
            follow_transform: true,
            kmin: 2.0,
            kmax: 12.0,
            rmin: 1.0,
            rmax: 3.0,
            kweight: 2.0,
            kweights: vec![1.0, 2.0, 3.0],
            fitspace: FitSpaceSpec::R,
            noise: false,
        }
    }
}

impl FitRanges {
    pub fn resolved(&self, transform_kweight: Option<f64>) -> Self {
        let mut result = self.clone();
        if self.follow_transform {
            result.kweight = transform_kweight.unwrap_or(2.0);
            result.kweights = vec![result.kweight];
        }
        result
    }

    pub fn valid(&self) -> bool {
        [self.kmin, self.kmax, self.rmin, self.rmax]
            .iter()
            .all(|v| v.is_finite())
            && self.kmin >= 0.
            && self.rmin >= 0.
            && self.kmin < self.kmax
            && self.rmin < self.rmax
            && self.effective_kweights().iter().all(|v| v.is_finite())
    }

    /// The k-weights actually fit (falls back to the primary weight).
    pub fn effective_kweights(&self) -> Vec<f64> {
        if self.kweights.is_empty() {
            return vec![self.kweight];
        }
        // The plot k-weight leads: the result's primary arrays (the ones the
        // plots show) come from the first k-weight of the list.
        let mut ks = self.kweights.clone();
        ks.sort_by(|a, b| a.total_cmp(b));
        if let Some(pos) = ks.iter().position(|k| (*k - self.kweight).abs() < 1e-9) {
            let lead = ks.remove(pos);
            ks.insert(0, lead);
        }
        ks
    }

    pub fn toggle_kweight(&mut self, kw: f64) {
        self.follow_transform = false;
        if let Some(pos) = self.kweights.iter().position(|k| (*k - kw).abs() < 1e-9) {
            if self.kweights.len() > 1 {
                self.kweights.remove(pos);
            }
        } else {
            self.kweights.push(kw);
        }
        self.kweights.sort_by(|a, b| a.total_cmp(b));
    }

    /// Nidp = 2ΔkΔR/π + 1 (Larch), the information content of the fit.
    pub fn n_idp(&self) -> f64 {
        2.0 * (self.kmax - self.kmin) * (self.rmax - self.rmin) / std::f64::consts::PI + 1.0
    }

    pub(crate) fn transform(&self) -> FeffFitTransform {
        FeffFitTransform {
            kmin: self.kmin,
            kmax: self.kmax,
            kweight: self.kweight,
            kweights: self.effective_kweights(),
            rmin: self.rmin,
            rmax: self.rmax,
            fitspace: self.fitspace.core(),
            ..FeffFitTransform::default()
        }
    }
}

/// One completed fit, kept so the model can be compared with and restored
/// from earlier runs (Artemis' fit history).
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct FitHistoryEntry {
    #[serde(default)]
    pub joint: Option<crate::joint_fitting::JointConfig>,
    pub id: usize,
    /// Group the fit was run on.
    pub group: String,
    pub r_factor: f64,
    pub reduced_chi_square: f64,
    pub chi_square: f64,
    pub n_idp: f64,
    pub n_vary: usize,
    pub paths: Vec<FitPathSpec>,
    pub vars: Vec<FitVarSpec>,
    pub ranges: FitRanges,
    /// Fitted values (name, value, stderr) for the varying parameters.
    pub values: Vec<(String, f64, Option<f64>)>,
    /// Physical path values captured at fit time, portable with the project.
    #[serde(default)]
    pub path_details: Vec<crate::fit_details::PathFitDetails>,
    #[serde(default)]
    pub solver_report: Option<FitSolverReport>,
}

impl FitHistoryEntry {
    pub fn from_result(
        id: usize,
        group: String,
        paths: Vec<FitPathSpec>,
        vars: Vec<FitVarSpec>,
        ranges: FitRanges,
        result: &FeffFitResult,
    ) -> Self {
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
            id,
            joint: None,
            group,
            r_factor: result.r_factor,
            reduced_chi_square: result.reduced_chi_square,
            chi_square: result.chi_square,
            n_idp: result.n_idp,
            n_vary: result.n_vary,
            path_details: crate::fit_details::snapshot(&paths, result),
            paths,
            vars,
            ranges,
            values,
            solver_report: result.solver_report.clone(),
        }
    }

    /// One-line description of what distinguishes this fit.
    pub fn summary(&self) -> String {
        let enabled = self
            .joint
            .as_ref()
            .map(|c| c.datasets.iter().map(|d| d.paths.len()).sum())
            .unwrap_or_else(|| self.paths.iter().filter(|p| p.enabled).count());
        let kws = self
            .ranges
            .effective_kweights()
            .iter()
            .map(|k| format!("{k:.0}"))
            .collect::<Vec<_>>()
            .join("");
        format!(
            "{enabled} paths · {} vars · {} kw{kws} · k {:.1}–{:.1} · R {:.1}–{:.1}",
            self.n_vary,
            self.ranges.fitspace.label(),
            self.ranges.kmin,
            self.ranges.kmax,
            self.ranges.rmin,
            self.ranges.rmax
        )
    }
}

/// Pairs of varying parameters whose correlation exceeds `threshold`.
pub fn high_correlations(result: &FeffFitResult, threshold: f64) -> Vec<(String, String, f64)> {
    let Some(corr) = result.correlation.as_ref() else {
        return Vec::new();
    };
    let names = &result.varying_names;
    let mut out = Vec::new();
    for i in 0..names.len().min(corr.len()) {
        for j in (i + 1)..names.len().min(corr[i].len()) {
            let c = corr[i][j];
            if c.abs() >= threshold {
                out.push((names[i].clone(), names[j].clone(), c));
            }
        }
    }
    out.sort_by(|a, b| b.2.abs().total_cmp(&a.2.abs()));
    out
}

/// Keep numerical termination separate from how well the model describes data.
pub fn fit_result_notices(result: &FeffFitResult) -> Vec<String> {
    let mut notes = Vec::new();
    if let Some(report) = &result.solver_report {
        if !report.converged {
            notes.push(format!("Stopped before convergence: {}. Review the parameters and fit ranges before continuing.", report.termination));
        }
        if report.final_cost > report.initial_cost * (1.0 + 1e-8) + 1e-15 {
            notes.push("The returned model has a larger residual than the starting model. Restore the earlier starting values from History before trying again.".into());
        }
    }
    if result.r_factor >= 1.0 {
        notes.push("R-factor ≥ 1: the residual is at least as large as the data signal in the selected fit space. Check the structure, paths, fit ranges, and starting values.".into());
    } else if result.r_factor > 0.1 {
        notes.push("Large residual: the model does not closely describe the data. Inspect the structure, selected paths, starting values, and fit ranges.".into());
    }
    let at_bounds: Vec<_> = result
        .varying_names
        .iter()
        .filter_map(|name| {
            let v = result.variables.get(name)?;
            let at = |bound: f64| {
                bound.is_finite()
                    && (v.value - bound).abs() <= 1e-6 * v.value.abs().max(bound.abs()).max(1.0)
            };
            let side = if v.min.is_some_and(at) {
                "lower"
            } else if v.max.is_some_and(at) {
                "upper"
            } else {
                return None;
            };
            Some(format!("{name} ({side})"))
        })
        .collect();
    if !at_bounds.is_empty() {
        notes.push(format!("At parameter bounds: {}. Review the physical constraints and model; symmetric uncertainties near a bound need caution.", at_bounds.join(", ")));
    }
    if result.n_idp <= result.n_vary as f64 {
        notes.push("The number of free variables reaches or exceeds the independent information in the fit range. Reduce the model before interpreting uncertainties.".into());
    }
    notes
}

fn spec(text: &str) -> PathParamSpec {
    let text = text.trim();
    match text.parse::<f64>() {
        Ok(v) => PathParamSpec::Value(v),
        Err(_) => PathParamSpec::Expression(text.to_string()),
    }
}

pub(crate) fn path_model(p: &FitPathSpec) -> Result<FeffPathModel, String> {
    let mut model = feffpath(&p.file.to_string_lossy(), FeffFlavor::Feff85L)
        .map_err(|e| format!("{}: {e}", p.label))?
        .set_label(&p.label)
        .set_s02(spec(&p.s02))
        .set_e0(spec(&p.e0))
        .set_sigma2(spec(&p.sigma2))
        .set_deltar(spec(&p.deltar));
    if !p.ei.trim().is_empty() {
        model = model.set_ei(spec(&p.ei));
    }
    if !p.third.trim().is_empty() {
        model = model.set_third(spec(&p.third));
    }
    if !p.fourth.trim().is_empty() {
        model = model.set_fourth(spec(&p.fourth));
    }
    Ok(model)
}

pub fn run_fit(
    k: DVector<f64>,
    chi: DVector<f64>,
    paths: &[FitPathSpec],
    vars: &[FitVarSpec],
    ranges: &FitRanges,
) -> Result<FeffFitResult, String> {
    let kweights = ranges.effective_kweights();
    let mut fit = FeffFit::new()
        .data(&k, &chi)
        .krange(ranges.kmin, ranges.kmax)
        .rrange(ranges.rmin, ranges.rmax)
        .kweight(kweights[0])
        .kweights(&kweights)
        .fitspace(ranges.fitspace.core());
    if ranges.noise {
        let noise = rexafs::xafs::fitting::transform::estimate_noise(&k, &chi, &ranges.transform())
            .map_err(|e| format!("noise estimate: {e}"))?;
        fit = fit.epsilon_ks(&noise.epsilon_k);
    }
    let mut any = false;
    for p in paths.iter().filter(|p| p.enabled) {
        fit = fit.add_path(path_model(p)?);
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
        if v.expr.is_none() && (v.min.is_some() || v.max.is_some()) {
            let min = v.min.unwrap_or(f64::NEG_INFINITY);
            let max = v.max.unwrap_or(f64::INFINITY);
            if min > max {
                return Err(format!(
                    "invalid bounds for '{}': min {min} exceeds max {max}",
                    v.name
                ));
            }
            fit = fit.set_bounds(&v.name, min, max);
        }
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
    pub reduced_chi_square: f64,
    /// (name, value, stderr) for each varying parameter.
    pub values: Vec<(String, f64, Option<f64>)>,
    pub solver_report: Option<FitSolverReport>,
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
            reduced_chi_square: result.reduced_chi_square,
            values,
            solver_report: result.solver_report.clone(),
        }
    }

    pub fn value_of(&self, name: &str) -> Option<f64> {
        self.values
            .iter()
            .find(|(n, _, _)| n == name)
            .map(|(_, v, _)| *v)
    }
}

/// Escape a text field for CSV (RFC 4180) and neutralize spreadsheet formula
/// injection.
///
/// Two distinct hazards reach these cells: file labels come from the
/// filesystem and parameter names are user-defined, so both can contain a
/// comma or quote — which would shift every later column — and both can begin
/// with a character that Excel, LibreOffice and Sheets treat as the start of a
/// formula rather than text.
fn csv_field(value: &str) -> String {
    let mut field = String::with_capacity(value.len() + 2);
    if value.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        field.push('\'');
    }
    field.push_str(value);
    if field.contains([',', '"', '\n', '\r']) {
        return format!("\"{}\"", field.replace('"', "\"\""));
    }
    field
}

/// CSV for a batch-fit result set: frame, captured file label, fit statistics, then one
/// value/stderr column pair per varying parameter.
pub fn batch_csv(
    rows: &[BatchFitRow],
    names: &[String],
    frame_labels: &BTreeMap<usize, String>,
) -> String {
    let mut out = String::from("frame,file,r_factor,reduced_chi_square");
    for name in names {
        out.push_str(&format!(
            ",{},{}",
            csv_field(name),
            csv_field(&format!("{name}_stderr"))
        ));
    }
    out.push_str(",solver_converged,solver_termination\n");
    for row in rows {
        let file = frame_labels
            .get(&row.frame)
            .map(String::as_str)
            .unwrap_or("");
        out.push_str(&format!(
            "{},{},{:.6},{:.6e}",
            row.frame,
            csv_field(file),
            row.r_factor,
            row.reduced_chi_square
        ));
        for name in names {
            match row.values.iter().find(|(n, _, _)| n == name) {
                Some((_, v, Some(err))) => out.push_str(&format!(",{v:.6},{err:.6}")),
                Some((_, v, None)) => out.push_str(&format!(",{v:.6},")),
                None => out.push_str(",,"),
            }
        }
        if let Some(report) = &row.solver_report {
            out.push_str(&format!(
                ",{},{}",
                report.converged,
                csv_field(&report.termination)
            ));
        } else {
            out.push_str(",,");
        }
        out.push('\n');
    }
    out
}

/// "value ± stderr" lines for the varying parameters, plus fit statistics.
#[cfg(test)]
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
    use crate::params::{PipelineParams, process_file};

    #[test]
    fn energy_shift_name_resolves_as_variable() {
        let value = rexafs::xafs::fitting::variables::eval_expression_with("e0", |name| {
            assert_eq!(name, "e0");
            Ok(7.25)
        })
        .unwrap();
        assert_eq!(value, 7.25);
    }

    #[test]
    fn default_template_recovers_known_energy_shift() {
        use rexafs::xafs::fitting::template::ParameterTemplate;
        use rexafs::xafs::structure::rank_paths;
        let file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../rexafs/tests/testfiles/feffcu01.dat");
        let path = feffpath(&file.to_string_lossy(), FeffFlavor::Feff85L).unwrap();
        let ranked = rank_paths(std::slice::from_ref(&path.feff));
        let mut specs = vec![FitPathSpec::blank(file)];
        let template = source_template(ParameterTemplate::PerShell, &ranked, &specs);
        let a = &template.assignments[0];
        specs[0].s02 = a.s02.clone();
        specs[0].e0 = a.e0.clone();
        specs[0].deltar = a.deltar.clone();
        specs[0].sigma2 = a.sigma2.clone();
        specs[0].enabled = true;
        let vars: Vec<_> = template
            .variables
            .iter()
            .map(|v| FitVarSpec {
                name: v.name.clone(),
                value: v.value,
                vary: v.vary,
                min: v.min,
                max: v.max,
                expr: None,
            })
            .collect();
        let truth = path
            .set_s02(PathParamSpec::Value(0.92))
            .set_e0(PathParamSpec::Value(3.2))
            .set_deltar(PathParamSpec::Value(0.025))
            .set_sigma2(PathParamSpec::Value(0.004));
        let k = DVector::from_iterator(301, (0..301).map(|i| i as f64 * 0.05));
        let chi = path2chi(&truth, &FitVariables::new(), &k).unwrap();
        let result = run_fit(k, chi, &specs, &vars, &FitRanges::default()).unwrap();
        let report = result.solver_report.as_ref().unwrap();
        assert!(report.converged, "{}", report.termination);
        assert!(report.final_cost < report.initial_cost);
        assert!(result.r_factor < 1e-6, "R = {}", result.r_factor);
        assert!((result.variables.get("e0").unwrap().value - 3.2).abs() < 0.01);
    }

    #[test]
    fn structures_have_independent_shell_parameters() {
        use rexafs::xafs::fitting::template::ParameterTemplate;
        use rexafs::xafs::structure::PathInfo;
        let paths = vec![
            FitPathSpec::blank("metal/feff0001.dat".into()),
            FitPathSpec::blank("oxide/feff0001.dat".into()),
        ];
        let infos: Vec<_> = (0..2)
            .map(|index| PathInfo {
                index,
                filename: String::new(),
                label: "Cu–Cu".into(),
                reff: 2.5,
                degen: 6.,
                nleg: 2,
                importance: 100.,
                shell: 1,
                leg_shells: vec![1],
                is_single_scattering: true,
            })
            .collect();
        let result = source_template(ParameterTemplate::PerShell, &infos, &paths);
        assert_eq!(result.variables.len(), 8);
        assert_eq!(result.assignments[0].deltar, "dr_1");
        assert_eq!(result.assignments[1].deltar, "p2_dr_1");
        assert_eq!(result.assignments[1].e0, "p2_e0");
    }

    /// Inspect convergence without recalculating FEFF or changing the saved model.
    #[test]
    #[ignore = "requires REXAFS_COMPARE_PROJECT with local paths and a standalone spectrum"]
    fn diagnose_saved_fit() {
        let file = crate::settings::env_var("COMPARE_PROJECT").expect("REXAFS_COMPARE_PROJECT");
        let project = crate::project::load(std::path::Path::new(&file)).unwrap();
        let spectrum =
            process_file(project.spectrum_file.as_ref().unwrap(), &project.params).unwrap();
        let mut vars = project.fit_vars.clone();
        for attempt in 0..3 {
            let result = run_fit(
                spectrum
                    .k()
                    .map(nalgebra::DVector::from_column_slice)
                    .unwrap(),
                spectrum
                    .chi()
                    .map(nalgebra::DVector::from_column_slice)
                    .unwrap(),
                &project.fit_paths,
                &vars,
                &project.fit_ranges,
            )
            .unwrap();
            println!(
                "{}",
                serde_json::json!({"attempt": attempt + 1,
                "r_factor": result.r_factor, "solver": result.solver_report,
                "parameters": result_summary(&result)})
            );
            for var in &mut vars {
                if var.expr.is_none() {
                    var.value = result.variables.get(&var.name).unwrap().value;
                }
            }
        }
    }

    /// Reproduce a saved GUI model with both engines using the exact same
    /// feff.inp and processing/fit ranges. Run explicitly with both features.
    #[test]
    #[ignore = "requires REXAFS_COMPARE_PROJECT and both calculation backends"]
    #[cfg(all(feature = "refeff-runner", feature = "feff10-runner"))]
    fn compare_saved_project_backends() {
        let _guard = crate::feffgen::feff_test_lock();
        let file = crate::settings::env_var("COMPARE_PROJECT").expect("REXAFS_COMPARE_PROJECT");
        let project = crate::project::load(std::path::Path::new(&file)).unwrap();
        let input =
            std::fs::read(project.feff_workspace.as_ref().unwrap().join("feff.inp")).unwrap();
        let spectrum =
            process_file(project.spectrum_file.as_ref().unwrap(), &project.params).unwrap();
        let root = std::env::temp_dir().join(format!("xts-backend-compare-{}", std::process::id()));
        let mut report = Vec::new();
        for (name, mode) in [
            ("refeff", FeffExecutionMode::RefeffPipeline),
            ("feffrs", FeffExecutionMode::Feff10Pipeline),
        ] {
            let workspace = root.join(name);
            std::fs::create_dir_all(&workspace).unwrap();
            std::fs::write(workspace.join("feff.inp"), &input).unwrap();
            let generated = run_feff(&FeffRunRequest {
                executable_path: PathBuf::new(),
                workspace_dir: workspace.clone(),
                feffinp: Some(workspace.join("feff.inp")),
                mode,
                timeout_sec: Some(600),
                use_sfconv: false,
                keep_all_outputs: true,
            })
            .unwrap();
            let mut paths = project.fit_paths.clone();
            for p in &mut paths {
                p.file = workspace.join(p.file.file_name().unwrap());
            }
            let result = run_fit(
                spectrum
                    .k()
                    .map(nalgebra::DVector::from_column_slice)
                    .unwrap(),
                spectrum
                    .chi()
                    .map(nalgebra::DVector::from_column_slice)
                    .unwrap(),
                &paths,
                &project.fit_vars,
                &project.fit_ranges,
            )
            .unwrap();
            let row = serde_json::json!({"backend": name, "paths": generated.path_files.len(), "r_factor": result.r_factor,
                "chi_square": result.chi_square, "parameters": result_summary(&result)});
            println!("{row}");
            report.push(row);
        }
        std::fs::write(
            root.join("comparison.json"),
            serde_json::to_vec_pretty(&report).unwrap(),
        )
        .unwrap();
        println!("Comparison artifacts: {}", root.display());
    }

    #[test]
    fn csv_fields_are_escaped_and_formula_neutralized() {
        // Plain text passes through untouched.
        assert_eq!(csv_field("Ru_QAS.dat"), "Ru_QAS.dat");
        // A comma in a filename would otherwise shift every later column.
        assert_eq!(csv_field("scan,01.dat"), "\"scan,01.dat\"");
        // Quotes are doubled per RFC 4180.
        assert_eq!(csv_field("a\"b"), "\"a\"\"b\"");
        // Newlines force quoting too.
        assert_eq!(csv_field("a\nb"), "\"a\nb\"");
        // Formula-leading text is forced to a literal string.
        assert_eq!(csv_field("=1+1"), "'=1+1");
        assert_eq!(csv_field("@SUM(A1)"), "'@SUM(A1)");
        assert_eq!(csv_field("-2+3"), "'-2+3");
        // Both hazards at once: neutralized *and* quoted.
        assert_eq!(csv_field("=cmd|'/c calc'!A1,x"), "\"'=cmd|'/c calc'!A1,x\"");
    }

    #[test]
    fn batch_csv_escapes_hostile_labels_and_names() {
        let rows = vec![BatchFitRow {
            frame: 0,
            entry_ix: 0,
            r_factor: 0.01,
            reduced_chi_square: 1.0e-6,
            values: vec![("=evil".into(), 1.0, Some(0.1))],
            solver_report: None,
        }];
        let names = vec!["=evil".to_string()];
        let labels = BTreeMap::from([(0usize, "scan,01.dat".to_string())]);
        let csv = batch_csv(&rows, &names, &labels);
        let header = csv.lines().next().unwrap();
        assert!(
            header.contains("'=evil"),
            "header not neutralized: {header}"
        );
        assert!(
            header.contains("'=evil_stderr"),
            "stderr header not neutralized: {header}"
        );
        let row = csv.lines().nth(1).unwrap();
        assert!(
            row.contains("\"scan,01.dat\""),
            "comma in label not quoted: {row}"
        );
    }

    #[test]
    fn batch_csv_uses_labels_captured_with_the_batch() {
        let rows = vec![BatchFitRow {
            frame: 17,
            entry_ix: 42,
            r_factor: 0.0123,
            reduced_chi_square: 4.5e-6,
            values: vec![("amp".into(), 0.91, Some(0.02))],
            solver_report: None,
        }];
        let labels = BTreeMap::from([(17, "scan_a_0017.dat".to_string())]);
        let csv = batch_csv(&rows, &["amp".into()], &labels);
        assert!(csv.contains("frame,file,r_factor,reduced_chi_square,amp,amp_stderr"));
        assert!(csv.contains("17,scan_a_0017.dat,0.012300,4.500000e-6,0.910000,0.020000"));
    }

    /// Measured, explicitly identified metal foils. The curated crystal, 8 Å
    /// input, default import pipeline, path preset and four-variable template
    /// all match the GUI. Keep the two engines on identical inputs/ranges.
    #[test]
    #[cfg(all(feature = "refeff-runner", feature = "feff10-runner"))]
    fn metal_foils_fit_with_both_backends() {
        let _guard = crate::feffgen::feff_test_lock();
        let core = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../rexafs");
        let root = std::env::temp_dir().join(format!("xts-metal-foils-{}", std::process::id()));
        let mut report = Vec::new();
        let mut failures = Vec::new();
        for (element, key, filename) in
            [("Cu", "cu", "cu_150k.xmu"), ("Ni", "ni", "ni_metal_rt.xdi")]
        {
            let data = core
                .join("tests/testfiles/xraylarch_d867/xafsdata")
                .join(filename)
                .canonicalize()
                .unwrap();
            let structure =
                read_cif(core.join("data/builtin_cifs").join(format!("{key}.cif"))).unwrap();
            let cluster = build_cluster(
                &structure,
                &AbsorberSelection::Element(element.into()),
                &ClusterOptions::default(),
            )
            .unwrap();
            let inp = write_feff_inp(
                &cluster,
                &FeffInputOptions {
                    edge: Edge::K,
                    rmax: Some(8.0),
                    rpath: Some(8.0),
                    titles: vec![format!(
                        "{element} foil validation, curated fcc crystal, 8 angstrom cluster"
                    )],
                    ..Default::default()
                },
            );
            let spectrum = process_file(&data, &PipelineParams::default()).unwrap();
            let mut backend_results: Vec<FeffFitResult> = Vec::new();
            for (backend, mode) in [
                ("refeff", FeffExecutionMode::RefeffPipeline),
                ("feffrs", FeffExecutionMode::Feff10Pipeline),
            ] {
                let ws = root.join(format!("{key}-{backend}"));
                std::fs::create_dir_all(&ws).unwrap();
                std::fs::write(ws.join("feff.inp"), &inp).unwrap();
                std::fs::write(
                    ws.join("crystal.json"),
                    serde_json::to_vec(&(&structure, &cluster)).unwrap(),
                )
                .unwrap();
                let files = crate::feffgen::run_backend(&ws, mode).unwrap();
                let parsed = files
                    .iter()
                    .map(|file| {
                        feffpath(&file.to_string_lossy(), FeffFlavor::Feff85L)
                            .unwrap()
                            .feff
                    })
                    .collect::<Vec<_>>();
                let ranked = rank_paths(&parsed);
                let ranges = FitRanges::default();
                let selected_indices = select_default(&ranked, ranges.rmax);
                let selected = ranked
                    .iter()
                    .filter(|p| selected_indices.contains(&p.index))
                    .cloned()
                    .collect::<Vec<_>>();
                let mut paths = files
                    .into_iter()
                    .map(FitPathSpec::blank)
                    .collect::<Vec<_>>();
                let template = source_template(ParameterTemplate::PerShell, &selected, &paths);
                for a in &template.assignments {
                    let p = &mut paths[a.index];
                    p.enabled = true;
                    p.s02 = a.s02.clone();
                    p.e0 = a.e0.clone();
                    p.deltar = a.deltar.clone();
                    p.sigma2 = a.sigma2.clone();
                }
                let vars = template
                    .variables
                    .iter()
                    .map(|v| FitVarSpec {
                        name: v.name.clone(),
                        value: v.value,
                        vary: v.vary,
                        min: v.min,
                        max: v.max,
                        expr: v.expr.clone(),
                    })
                    .collect::<Vec<_>>();
                assert_eq!(vars.len(), 4, "fcc first-shell template");
                let fit = run_fit(
                    spectrum
                        .k()
                        .map(nalgebra::DVector::from_column_slice)
                        .unwrap(),
                    spectrum
                        .chi()
                        .map(nalgebra::DVector::from_column_slice)
                        .unwrap(),
                    &paths,
                    &vars,
                    &ranges,
                )
                .unwrap();
                let row = serde_json::json!({"element": element, "data": filename, "backend": backend,
                    "cluster_radius_angstrom": 8.0, "cluster_atoms": cluster.atoms.len(),
                    "generated_paths": paths.len(), "selected_paths": selected.len(), "ranges": ranges,
                    "r_factor": fit.r_factor, "solver": fit.solver_report,
                    "parameters": fit.varying_names.iter().map(|name| { let v = fit.variables.get(name).unwrap();
                        serde_json::json!({"name": name, "value": v.value, "stderr": v.stderr}) }).collect::<Vec<_>>()});
                println!("{row}");
                report.push(row);
                let project = crate::project::ProjectFile {
                    version: crate::project::PROJECT_VERSION,
                    spectrum_file: Some(data.clone()),
                    feff_workspace: Some(ws.clone()),
                    fit_paths: paths.clone(),
                    fit_vars: vars.clone(),
                    fit_ranges: ranges.clone(),
                    fit_history: vec![FitHistoryEntry::from_result(
                        1,
                        filename.into(),
                        paths,
                        vars,
                        ranges,
                        &fit,
                    )],
                    ..Default::default()
                };
                crate::project::save(&ws.join("foil.rxs"), &project).unwrap();
                if !fit.solver_report.as_ref().unwrap().converged || fit.r_factor >= 0.02 {
                    failures.push(format!(
                        "{element}/{backend}: R={}, {:?}",
                        fit.r_factor, fit.solver_report
                    ));
                }
                for (name, lo, hi) in [
                    ("s02", 0.5, 1.2),
                    ("e0", -15.0, 15.0),
                    ("dr_1", -0.08, 0.08),
                    ("ss_1", 0.0001, 0.02),
                ] {
                    let v = fit
                        .variables
                        .get(name)
                        .expect("all four template variables are fitted");
                    assert!(
                        v.value > lo && v.value < hi,
                        "{element}/{backend} {name}={}",
                        v.value
                    );
                    assert!(
                        v.stderr.is_some_and(|e| e.is_finite() && e > 0.),
                        "missing uncertainty: {name}"
                    );
                }
                backend_results.push(fit);
            }
            let (a, b) = (&backend_results[0], &backend_results[1]);
            if (a.r_factor - b.r_factor).abs() >= 5e-5 {
                failures.push(format!(
                    "{element} backend R disagreement: {} vs {}",
                    a.r_factor, b.r_factor
                ));
            }
            // Distances, energies and disorder have different units. Require
            // engine differences below 5% of the smaller fitted uncertainty,
            // instead of applying an arbitrary common absolute tolerance.
            for name in &a.varying_names {
                let (a, b) = (
                    a.variables.get(name).unwrap(),
                    b.variables.get(name).unwrap(),
                );
                let tolerance = 0.05 * a.stderr.unwrap().min(b.stderr.unwrap());
                if (a.value - b.value).abs() >= tolerance {
                    failures.push(format!(
                        "{element}/{name} backend disagreement: {} vs {} (tolerance {tolerance})",
                        a.value, b.value
                    ));
                }
            }
        }
        std::fs::write(
            root.join("comparison.json"),
            serde_json::to_vec_pretty(&report).unwrap(),
        )
        .unwrap();
        println!("Metal foil validation artifacts: {}", root.display());
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
}
