//! GUI-side FEFF fitting: plain-data specs assembled in the Fit panel are
//! turned into a core `FeffFit` and run on the background executor.

use std::collections::BTreeMap;
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

    fn transform(&self) -> FeffFitTransform {
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
            group,
            r_factor: result.r_factor,
            reduced_chi_square: result.reduced_chi_square,
            chi_square: result.chi_square,
            n_idp: result.n_idp,
            n_vary: result.n_vary,
            paths,
            vars,
            ranges,
            values,
        }
    }

    /// One-line description of what distinguishes this fit.
    pub fn summary(&self) -> String {
        let enabled = self.paths.iter().filter(|p| p.enabled).count();
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
        let noise =
            xraytsubaki::xafs::fitting::transform::estimate_noise(&k, &chi, &ranges.transform())
                .map_err(|e| format!("noise estimate: {e}"))?;
        fit = fit.epsilon_ks(&noise.epsilon_k);
    }
    let mut any = false;
    for p in paths.iter().filter(|p| p.enabled) {
        let mut model = feffpath(&p.file.to_string_lossy(), FeffFlavor::Feff85L)
            .map_err(|e| format!("{}: {e}", p.label))?
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
    out.push('\n');
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
    use crate::feffgen::{CrystalSpec, new_workspace_from_spec, run_feff10};
    use crate::params::{PipelineParams, process_file};

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
        }];
        let labels = BTreeMap::from([(17, "scan_a_0017.dat".to_string())]);
        let csv = batch_csv(&rows, &["amp".into()], &labels);
        assert!(csv.contains("frame,file,r_factor,reduced_chi_square,amp,amp_stderr"));
        assert!(csv.contains("17,scan_a_0017.dat,0.012300,4.500000e-6,0.910000,0.020000"));
    }

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
            FitVarSpec {
                name: "amp".into(),
                value: 0.9,
                vary: false,
                min: Some(0.0),
                max: Some(1.5),
                expr: None,
            },
            FitVarSpec {
                name: "de0".into(),
                value: 0.0,
                vary: true,
                min: Some(-20.0),
                max: Some(20.0),
                expr: None,
            },
        ];
        for (i, file) in files.iter().enumerate() {
            let n = i + 1;
            paths.push(FitPathSpec {
                label: format!("path{n}"),
                ..FitPathSpec::standard(file.clone(), n)
            });
            vars.push(FitVarSpec {
                name: format!("sig2_{n}"),
                value: 0.003,
                vary: true,
                // wide enough to never bind at the physical optimum; the
                // bounds only exercise the set_bounds plumbing
                min: Some(0.0),
                max: Some(0.05),
                expr: None,
            });
            vars.push(FitVarSpec {
                name: format!("dr_{n}"),
                value: 0.0,
                vary: true,
                min: Some(-0.5),
                max: Some(0.5),
                expr: None,
            });
        }

        let ranges = FitRanges {
            kmin: 3.0,
            kmax: 12.0,
            rmin: 1.8,
            rmax: 3.0,
            kweight: 2.0,
            kweights: vec![2.0],
            ..FitRanges::default()
        };
        let result = run_fit(k, chi, &paths, &vars, &ranges).expect("fit");
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
