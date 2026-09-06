//! Explicit dataset/path assignment and global/local parameter expansion.
use crate::fitting::{FitPathSpec, FitRanges, FitVarSpec};
use nalgebra::DVector;
use rexafs::xafs::fitting::{
    self, FeffFitDataset, FeffFitResult, FitVariable, FitVariables, expression::try_extract_symbols,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub(crate) struct JointConfig {
    pub enabled: bool,
    pub datasets: Vec<JointDataset>,
    /// Names marked Per spectrum. Others are shared by assigned datasets.
    pub local: BTreeSet<String>,
    pub scopes: BTreeMap<usize, BTreeMap<String, bool>>,
    pub values: BTreeMap<usize, BTreeMap<String, f64>>,
    pub varying: BTreeMap<usize, BTreeMap<String, bool>>,
}
#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
pub(crate) struct JointDataset {
    pub id: usize,
    pub file: PathBuf,
    pub label: String,
    /// Stable file identity, never an index into the path table.
    pub paths: Vec<PathBuf>,
    #[serde(default)]
    pub ranges: Option<FitRanges>,
    #[serde(default)]
    pub expressions: BTreeMap<PathBuf, FitPathSpec>,
}
impl JointConfig {
    pub fn is_local(&self, id: usize, name: &str) -> bool {
        self.scopes
            .get(&id)
            .and_then(|s| s.get(name))
            .copied()
            .unwrap_or_else(|| self.local.contains(name))
    }
    pub fn local_for(&self, id: usize) -> BTreeSet<String> {
        let mut names = self.local.clone();
        if let Some(scopes) = self.scopes.get(&id) {
            for (name, local) in scopes {
                if *local {
                    names.insert(name.clone());
                } else {
                    names.remove(name);
                }
            }
        }
        names
    }
    pub fn initial_value(&self, id: usize, var: &FitVarSpec) -> f64 {
        if self.is_local(id, &var.name) {
            self.values
                .get(&id)
                .and_then(|v| v.get(&var.name))
                .copied()
                .unwrap_or(var.value)
        } else {
            var.value
        }
    }
    pub fn varies(&self, id: usize, var: &FitVarSpec) -> bool {
        if self.is_local(id, &var.name) {
            self.varying
                .get(&id)
                .and_then(|v| v.get(&var.name))
                .copied()
                .unwrap_or(var.vary)
        } else {
            var.vary
        }
    }
}
pub(crate) fn local_name(id: usize, name: &str) -> String {
    format!("d{id}__{name}")
}
pub(crate) fn display_name(name: &str, config: Option<&JointConfig>) -> String {
    if let Some(config) = config {
        for d in &config.datasets {
            if let Some(name) = name.strip_prefix(&format!("d{}__", d.id)) {
                return format!("{name} · spectrum {}", d.id);
            }
        }
        return format!("{name} · shared");
    }
    name.into()
}
fn rename(expr: &str, local: &BTreeSet<String>, id: usize) -> String {
    let mut out = String::new();
    let mut token = String::new();
    for c in expr.chars().chain(std::iter::once(' ')) {
        if c.is_ascii_alphanumeric() || c == '_' {
            token.push(c);
        } else {
            out.push_str(&if local.contains(&token) {
                local_name(id, &token)
            } else {
                token.clone()
            });
            token.clear();
            out.push(c);
        }
    }
    out.trim_end().into()
}
pub(crate) struct PreparedJoint {
    pub paths: Vec<Vec<FitPathSpec>>,
    pub vars: FitVariables,
}
pub(crate) fn prepare(
    config: &JointConfig,
    paths: &[FitPathSpec],
    vars: &[FitVarSpec],
) -> Result<PreparedJoint, String> {
    if config.datasets.len() < 2 {
        return Err("Add at least two spectra for a joint fit.".into());
    }
    let mut ids = BTreeSet::new();
    let mut files = BTreeSet::new();
    for d in &config.datasets {
        if !ids.insert(d.id) || !files.insert(&d.file) {
            return Err("Each joint-fit spectrum must have a unique ID and file.".into());
        }
        if d.paths.is_empty() {
            return Err(format!("{} has no assigned paths.", d.label));
        }
    }
    let mut output = PreparedJoint {
        paths: vec![],
        vars: FitVariables::new(),
    };
    let by_name: BTreeMap<_, _> = vars.iter().map(|v| (v.name.as_str(), v)).collect();
    if by_name.len() != vars.len() {
        return Err("Parameter names must be unique.".into());
    }
    for d in &config.datasets {
        let mut active = BTreeSet::new();
        let local_names = config.local_for(d.id);
        let mut assigned = vec![];
        for file in &d.paths {
            if assigned.iter().any(|p: &FitPathSpec| &p.file == file) {
                return Err(format!("{} assigns the same path more than once.", d.label));
            }
            let base = paths
                .iter()
                .find(|p| &p.file == file)
                .ok_or_else(|| {
                    format!(
                        "{}: assigned path {} is no longer in the model.",
                        d.label,
                        file.display()
                    )
                })?
                .clone();
            let mut p = d.expressions.get(file).unwrap_or(&base).clone();
            p.file = file.clone();
            p.enabled = true;
            p.label = format!("Spectrum {} · {} · {}", d.id, d.label, p.label);
            for expr in [
                &mut p.s02,
                &mut p.e0,
                &mut p.sigma2,
                &mut p.deltar,
                &mut p.ei,
                &mut p.third,
                &mut p.fourth,
            ] {
                if !expr.trim().is_empty() {
                    active.extend(try_extract_symbols(expr).map_err(|e| e.to_string())?);
                    *expr = rename(expr, &local_names, d.id);
                }
            }
            assigned.push(p);
        }
        // Include constraints recursively; omit unused variables to avoid
        // singular fits when datasets have different path assignments.
        let mut pending: Vec<_> = active.iter().cloned().collect();
        while let Some(name) = pending.pop() {
            let var = by_name
                .get(name.as_str())
                .ok_or_else(|| format!("{}: undefined parameter '{name}'.", d.label))?;
            if let Some(expr) = &var.expr {
                for dep in try_extract_symbols(expr).map_err(|e| e.to_string())? {
                    if !local_names.contains(&name) && local_names.contains(&dep) {
                        return Err(format!(
                            "Shared parameter '{name}' depends on Per spectrum parameter '{dep}'. Make '{name}' Per spectrum too, or change its expression."
                        ));
                    }
                    if active.insert(dep.clone()) {
                        pending.push(dep);
                    }
                }
            }
        }
        for name in active {
            let v = by_name[name.as_str()];
            let local = local_names.contains(&name);
            let expanded = if local {
                local_name(d.id, &name)
            } else {
                name.clone()
            };
            if local && by_name.contains_key(expanded.as_str()) {
                return Err(format!(
                    "Parameter '{expanded}' collides with an internal per-spectrum name; rename it."
                ));
            }
            if v.min.zip(v.max).is_some_and(|(lo, hi)| lo > hi) {
                return Err(format!("Invalid bounds for '{name}'."));
            }
            if output.vars.get(&expanded).is_none() {
                let value = config.initial_value(d.id, v);
                if !value.is_finite() {
                    return Err(format!("{}: invalid value for '{name}'.", d.label));
                }
                let mut var =
                    FitVariable::new(value, config.varies(d.id, v)).with_bounds(v.min, v.max);
                if let Some(expr) = &v.expr {
                    var = var.with_expr(rename(expr, &local_names, d.id));
                }
                output.vars.insert(expanded, var);
            }
        }
        output.paths.push(assigned);
    }
    output.vars.resolve_values().map_err(|e| e.to_string())?;
    Ok(output)
}
pub(crate) fn run(
    config: &JointConfig,
    data: &[(DVector<f64>, DVector<f64>)],
    paths: &[FitPathSpec],
    vars: &[FitVarSpec],
    ranges: &FitRanges,
) -> Result<(FeffFitResult, Vec<FitPathSpec>), String> {
    if data.len() != config.datasets.len() {
        return Err("Joint dataset count changed while preparing data.".into());
    }
    let prepared = prepare(config, paths, vars)?;
    let mut datasets = vec![];
    for (((k, chi), paths), dataset) in data.iter().zip(&prepared.paths).zip(&config.datasets) {
        let ranges = dataset.ranges.as_ref().unwrap_or(ranges);
        if !ranges.valid() {
            return Err(format!("{}: invalid fit range.", dataset.label));
        }
        let mut ds = FeffFitDataset::new().data(k, chi);
        ds.transform = ranges.transform();
        if ranges.noise {
            ds.epsilon_ks = fitting::transform::estimate_noise(k, chi, &ds.transform)
                .map_err(|e| e.to_string())?
                .epsilon_k;
        }
        for p in paths {
            ds.paths.push(crate::fitting::path_model(p)?);
        }
        datasets.push(ds);
    }
    let result = fitting::feffit_joint(&datasets, &prepared.vars).map_err(|e| e.to_string())?;
    Ok((result, prepared.paths.into_iter().flatten().collect()))
}
/// Select plotted arrays while retaining the joint fit's global statistics.
pub(crate) fn result_view(result: &FeffFitResult, index: usize) -> FeffFitResult {
    let mut view = result.clone();
    if index < view.datasets.len() {
        view.datasets.swap(0, index);
        view.sync_primary_dataset_fields();
        for p in &mut view.path_contributions {
            if let Some(label) = p.label.splitn(3, " · ").nth(2) {
                p.label = label.to_string();
            }
        }
    }
    view
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (JointConfig, Vec<FitPathSpec>, Vec<FitVarSpec>) {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../rexafs/tests/testfiles/xraylarch_d867/feffit/Feff_Cu");
        let paths = (1..=2)
            .map(|i| FitPathSpec {
                s02: "amp".into(),
                e0: "0".into(),
                deltar: "dr".into(),
                sigma2: "0.004".into(),
                ..FitPathSpec::standard(dir.join(format!("feff{i:04}.dat")), i)
            })
            .collect::<Vec<_>>();
        let config = JointConfig {
            enabled: true,
            local: BTreeSet::from(["dr".into()]),
            datasets: vec![
                JointDataset {
                    id: 1,
                    file: "spectrum_a".into(),
                    label: "A".into(),
                    paths: vec![paths[0].file.clone()],
                    ..Default::default()
                },
                JointDataset {
                    id: 2,
                    file: "spectrum_b".into(),
                    label: "B".into(),
                    paths: paths.iter().map(|p| p.file.clone()).collect(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let vars = vec![
            FitVarSpec {
                name: "amp".into(),
                value: 0.9,
                vary: true,
                min: Some(0.5),
                max: Some(1.5),
                expr: None,
            },
            FitVarSpec {
                name: "dr".into(),
                value: 0.,
                vary: true,
                min: Some(-0.2),
                max: Some(0.2),
                expr: None,
            },
        ];
        (config, paths, vars)
    }
    #[test]
    fn joint_fit_recovers_shared_and_local_parameters_with_different_paths() {
        let (mut config, paths, vars) = fixture();
        config.datasets[0].ranges = Some(
            FitRanges {
                kmin: 2.,
                kmax: 11.,
                rmin: 1.,
                rmax: 3.5,
                ..Default::default()
            }
            .resolved(Some(1.)),
        );
        config.datasets[1].ranges = Some(
            FitRanges {
                kmin: 3.,
                kmax: 12.,
                rmin: 1.2,
                rmax: 4.5,
                ..Default::default()
            }
            .resolved(Some(3.)),
        );
        let mut override_path = paths[1].clone();
        override_path.s02 = "amp * 0.5".into();
        override_path.deltar = "dr + 0.01".into();
        config.datasets[1]
            .expressions
            .insert(override_path.file.clone(), override_path);
        config
            .values
            .entry(1)
            .or_default()
            .insert("dr".into(), -0.02);
        config
            .values
            .entry(2)
            .or_default()
            .insert("dr".into(), 0.03);
        let prepared = prepare(&config, &paths, &vars).unwrap();
        assert_eq!(
            prepared.paths.iter().map(|p| p.len()).collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert_eq!(prepared.vars.varying_names().len(), 3);
        assert_eq!(prepared.vars.get("d1__dr").unwrap().value, -0.02);
        assert_eq!(prepared.vars.get("d2__dr").unwrap().value, 0.03);
        assert_eq!(prepared.paths[1][1].deltar, "d2__dr + 0.01");
        assert_eq!(prepared.paths[0][0].deltar, "d1__dr");
        let k = DVector::from_iterator(281, (0..281).map(|i| i as f64 * 0.05));
        let mut truth = prepared.vars.clone();
        truth.get_mut("amp").unwrap().value = 0.82;
        truth.get_mut("d1__dr").unwrap().value = -0.013;
        truth.get_mut("d2__dr").unwrap().value = 0.024;
        let data = prepared
            .paths
            .iter()
            .enumerate()
            .map(|(i, paths)| {
                let models = paths
                    .iter()
                    .map(|p| crate::fitting::path_model(p).unwrap())
                    .collect::<Vec<_>>();
                let mut chi = fitting::ff2chi(&models, &truth, &k).unwrap().chi;
                for (j, y) in chi.iter_mut().enumerate() {
                    *y += 1e-5 * (j as f64 * 1.337 + i as f64).sin();
                }
                (k.clone(), chi)
            })
            .collect::<Vec<_>>();
        let ranges = FitRanges {
            kmin: 2.,
            kmax: 12.,
            rmin: 1.,
            rmax: 4.5,
            noise: false,
            ..Default::default()
        };
        let (result, expanded) = run(&config, &data, &paths, &vars, &ranges).unwrap();
        assert!(
            result.solver_report.as_ref().unwrap().converged,
            "{:?}",
            result.solver_report
        );
        assert_eq!(
            result
                .datasets
                .iter()
                .map(|d| d.path_contributions.len())
                .collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert_eq!(result.n_vary, 3);
        assert_eq!(result.datasets[0].kweights, vec![1.]);
        assert_eq!(result.datasets[1].kweights, vec![3.]);
        assert_eq!(result.datasets[0].kmax, Some(11.));
        assert_eq!(result.datasets[1].kmin, Some(3.));
        assert_eq!(result.datasets[0].rmax, Some(3.5));
        assert_eq!(result.datasets[1].rmin, Some(1.2));
        for (name, value) in [("amp", 0.82), ("d1__dr", -0.013), ("d2__dr", 0.024)] {
            let var = result.variables.get(name).unwrap();
            assert!((var.value - value).abs() < 0.001, "{name}: {}", var.value);
            assert!(var.stderr.is_some_and(|e| e.is_finite() && e > 0.));
        }
        let details = crate::fit_details::snapshot(&expanded, &result);
        assert_eq!(details.len(), 3);
        assert!(
            details
                .iter()
                .all(|p| p.distance.as_ref().unwrap().stderr.is_some())
        );
        let second = result_view(&result, 1);
        assert_eq!(second.path_contributions.len(), 2);
        assert_eq!(second.r_factor, result.r_factor);
    }
    #[test]
    fn scopes_reject_ambiguous_constraints_missing_paths_and_duplicates() {
        let (mut config, paths, mut vars) = fixture();
        vars[0].expr = Some("0.8+dr".into());
        assert!(
            prepare(&config, &paths, &vars)
                .err()
                .unwrap()
                .contains("Shared parameter")
        );
        config.local.insert("amp".into());
        assert!(prepare(&config, &paths, &vars).is_ok());
        config.datasets[0].paths.push("missing.dat".into());
        assert!(prepare(&config, &paths, &vars).is_err());
        config.datasets[0].paths.pop();
        config.datasets.push(config.datasets[0].clone());
        assert!(prepare(&config, &paths, &vars).is_err());
    }
    #[test]
    fn assignments_and_scopes_roundtrip_without_catalog_indices() {
        let (config, paths, vars) = fixture();
        let project = crate::project::ProjectFile {
            joint: config,
            fit_paths: paths,
            fit_vars: vars,
            ..Default::default()
        };
        let json = serde_json::to_string(&project).unwrap();
        let restored: crate::project::ProjectFile = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.joint.datasets[1].paths.len(), 2);
        assert!(restored.joint.local.contains("dr"));
        let old: crate::project::ProjectFile = serde_json::from_str("{}").unwrap();
        assert!(!old.joint.enabled);
    }
    #[test]
    fn spectrum_scopes_values_and_expressions_are_independent_and_persist() {
        let (mut config, paths, vars) = fixture();
        config
            .scopes
            .entry(1)
            .or_default()
            .insert("dr".into(), false);
        config
            .values
            .entry(2)
            .or_default()
            .insert("dr".into(), 0.031);
        config
            .varying
            .entry(2)
            .or_default()
            .insert("dr".into(), false);
        config.datasets[1].ranges = Some(
            FitRanges {
                kmin: 4.,
                kmax: 10.,
                ..Default::default()
            }
            .resolved(Some(3.)),
        );
        let mut p = paths[1].clone();
        p.deltar = "dr + 0.02".into();
        config.datasets[1].expressions.insert(p.file.clone(), p);
        let config: JointConfig =
            serde_json::from_str(&serde_json::to_string(&config).unwrap()).unwrap();
        let prepared = prepare(&config, &paths, &vars).unwrap();
        assert!(prepared.vars.get("d1__dr").is_none());
        assert!(prepared.vars.get("dr").unwrap().vary);
        assert!(!prepared.vars.get("d2__dr").unwrap().vary);
        assert_eq!(prepared.vars.get("d2__dr").unwrap().value, 0.031);
        assert_eq!(prepared.paths[1][0].deltar, "d2__dr");
        assert_eq!(prepared.paths[1][1].deltar, "d2__dr + 0.02");
        assert_eq!(config.datasets[1].ranges.as_ref().unwrap().kmin, 4.);
        assert_eq!(
            config.datasets[1]
                .ranges
                .as_ref()
                .unwrap()
                .effective_kweights(),
            vec![3.]
        );
    }
    #[test]
    fn fit_weights_follow_each_transform_and_preserve_manual_projects() {
        let auto = FitRanges::default();
        assert!(auto.follow_transform);
        assert_eq!(auto.resolved(Some(1.)).effective_kweights(), vec![1.]);
        assert_eq!(auto.resolved(Some(3.)).effective_kweights(), vec![3.]);
        let mut manual = auto.resolved(Some(2.));
        manual.toggle_kweight(1.);
        assert!(!manual.follow_transform);
        assert_eq!(manual.resolved(Some(3.)).effective_kweights(), vec![2., 1.]);
        let old: FitRanges = serde_json::from_str(r#"{"kweight":2,"kweights":[1,2,3]}"#).unwrap();
        assert!(!old.follow_transform);
        assert_eq!(
            old.resolved(Some(1.)).effective_kweights(),
            vec![2., 1., 3.]
        );
    }
}
