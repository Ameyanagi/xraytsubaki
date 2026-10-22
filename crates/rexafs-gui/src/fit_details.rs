//! Archived physical path values and first-order covariance propagation.
use crate::fitting::FitPathSpec;
use rexafs::xafs::fitting::{
    FeffFitResult, FeffFlavor, FitVariables, FittingError, expression::eval_expression_with,
    feffpath,
};

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct Estimate {
    pub value: f64,
    pub stderr: Option<f64>,
    pub fixed: bool,
}
impl Estimate {
    pub fn label(&self, digits: usize) -> String {
        if self.fixed {
            format!("{:.*} (fixed)", digits, self.value)
        } else if let Some(e) = self.stderr {
            format!("{:.*} ± {:.*}", digits, self.value, digits, e)
        } else {
            format!("{:.*} ± unavailable", digits, self.value)
        }
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct PathFitDetails {
    pub file: std::path::PathBuf,
    pub label: String,
    /// Absorber–scatterer chain of the FEFF path, e.g. "Cu–Cu" or
    /// "Cu–O–Cu", from the atoms in its geometry block. `None` in history
    /// entries recorded before rexafs stored it, or when the file lists no
    /// atoms; the display then falls back to `label`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scatterers: Option<String>,
    pub reff: Option<f64>,
    pub nleg: Option<usize>,
    pub degeneracy: Option<f64>,
    /// Coordination number used by the fit (FEFF degeneracy unless overridden).
    #[serde(default)]
    pub effective_degen: Option<Estimate>,
    pub distance: Option<Estimate>,
    pub deltar: Option<Estimate>,
    pub sigma2: Option<Estimate>,
    pub s02: Option<Estimate>,
    pub e0: Option<Estimate>,
    /// N·S₀², the amplitude the fit actually resolves.
    #[serde(default)]
    pub n_s02: Option<Estimate>,
    #[serde(default)]
    pub ei: Option<Estimate>,
    #[serde(default)]
    pub third: Option<Estimate>,
    #[serde(default)]
    pub fourth: Option<Estimate>,
}

fn evaluate(expr: &str, reff: Option<f64>, degen: Option<f64>, vars: &FitVariables) -> Option<f64> {
    let globals = vars.resolve_values().ok()?;
    eval_expression_with(if expr.trim().is_empty() { "0" } else { expr }, |symbol| {
        (if symbol == "reff" {
            reff
        } else if symbol == "degen" {
            degen
        } else {
            globals.get(symbol).copied()
        })
        .ok_or_else(|| FittingError::UndefinedSymbol {
            symbol: symbol.into(),
        })
    })
    .ok()
}

/// J C Jᵀ, including shared-variable correlations and constrained expressions.
/// The FEFF reference geometry is treated as exact; this is fit uncertainty.
pub(crate) fn estimate(
    expr: &str,
    reff: Option<f64>,
    degen: Option<f64>,
    result: &FeffFitResult,
) -> Option<Estimate> {
    let value = evaluate(expr, reff, degen, &result.variables)?;
    let gradient: Option<Vec<f64>> = result
        .varying_names
        .iter()
        .map(|name| {
            let var = result.variables.get(name)?;
            let step = 1e-5 * var.value.abs().max(0.01);
            let lo = var
                .min
                .map_or(var.value - step, |b| (var.value - step).max(b));
            let hi = var
                .max
                .map_or(var.value + step, |b| (var.value + step).min(b));
            if hi <= lo {
                return None;
            }
            let mut vars = result.variables.clone();
            vars.get_mut(name)?.value = hi;
            let a = evaluate(expr, reff, degen, &vars)?;
            vars.get_mut(name)?.value = lo;
            let b = evaluate(expr, reff, degen, &vars)?;
            Some((a - b) / (hi - lo))
        })
        .collect();
    fn depends(
        expr: &str,
        result: &FeffFitResult,
        seen: &mut std::collections::BTreeSet<String>,
    ) -> bool {
        rexafs::xafs::fitting::expression::extract_symbols(expr)
            .iter()
            .any(|s| {
                result.varying_names.contains(s)
                    || (seen.insert(s.clone())
                        && result
                            .variables
                            .get(s)
                            .and_then(|v| v.expr.as_deref())
                            .is_some_and(|expr| depends(expr, result, seen)))
            })
    }
    let fixed = !depends(expr, result, &mut Default::default());
    let stderr = gradient.as_ref().and_then(|g| {
        if fixed {
            return Some(0.);
        }
        let c = result.covariance.as_ref()?;
        if c.len() != g.len()
            || c.iter()
                .any(|row| row.len() != g.len() || row.iter().any(|v| !v.is_finite()))
        {
            return None;
        }
        let variance: f64 = g
            .iter()
            .enumerate()
            .map(|(i, a)| {
                g.iter()
                    .enumerate()
                    .map(|(j, b)| a * c[i][j] * b)
                    .sum::<f64>()
            })
            .sum();
        (variance.is_finite() && variance >= 0.).then(|| variance.sqrt())
    });
    Some(Estimate {
        value,
        stderr,
        fixed,
    })
}

/// Cumulant / lifetime cells are optional: an empty cell is the core default
/// of zero and is left out of reports rather than shown as a fitted value.
fn optional(
    expr: &str,
    reff: Option<f64>,
    degen: Option<f64>,
    result: &FeffFitResult,
) -> Option<Estimate> {
    if expr.trim().is_empty() {
        None
    } else {
        estimate(expr, reff, degen, result)
    }
}

pub(crate) fn snapshot(paths: &[FitPathSpec], result: &FeffFitResult) -> Vec<PathFitDetails> {
    paths
        .iter()
        .filter(|p| p.enabled)
        .map(|p| {
            let feff = feffpath(&p.file.to_string_lossy(), FeffFlavor::Feff85L)
                .ok()
                .map(|p| p.feff);
            let reff = feff.as_ref().map(|f| f.reff);
            let degen = feff.as_ref().map(|f| f.degen);
            let deltar = estimate(&p.deltar, reff, degen, result);
            let distance = reff.zip(deltar.as_ref()).map(|(r, dr)| Estimate {
                value: r + dr.value,
                ..dr.clone()
            });
            PathFitDetails {
                file: p.file.clone(),
                label: p.label.clone(),
                scatterers: feff
                    .as_ref()
                    .filter(|f| !f.geometry_atoms.is_empty())
                    .map(rexafs::xafs::structure::path_label),
                reff,
                nleg: feff.as_ref().map(|f| f.nleg),
                degeneracy: degen,
                effective_degen: estimate(
                    if p.degen.trim().is_empty() {
                        "degen"
                    } else {
                        &p.degen
                    },
                    reff,
                    degen,
                    result,
                ),
                distance,
                deltar,
                sigma2: estimate(&p.sigma2, reff, degen, result),
                s02: estimate(
                    if p.s02.trim().is_empty() { "1" } else { &p.s02 },
                    reff,
                    degen,
                    result,
                ),
                e0: estimate(&p.e0, reff, degen, result),
                n_s02: estimate(
                    &format!(
                        "({}) * ({})",
                        if p.degen.trim().is_empty() {
                            "degen"
                        } else {
                            p.degen.trim()
                        },
                        if p.s02.trim().is_empty() {
                            "1"
                        } else {
                            p.s02.trim()
                        }
                    ),
                    reff,
                    degen,
                    result,
                ),
                ei: optional(&p.ei, reff, degen, result),
                third: optional(&p.third, reff, degen, result),
                fourth: optional(&p.fourth, reff, degen, result),
            }
        })
        .collect()
}

impl PathFitDetails {
    /// Readable path name: the absorber–scatterer chain and the FEFF half
    /// path length R_eff, e.g. "Cu–Cu 2.553 Å", matching the Paths list of
    /// the fit model. R_eff is the geometric half path length of the FEFF
    /// reference structure, not the fitted distance R = R_eff + ΔR. A joint
    /// fit keeps its "Spectrum n · data ·" prefix. Older history entries
    /// without the chain fall back to the stored path label (the file name).
    pub fn title(&self) -> String {
        let Some(chain) = self.scatterers.as_deref() else {
            return match self.reff {
                Some(r) => format!("{} {r:.3} Å", self.label),
                None => self.label.clone(),
            };
        };
        let name = match self.reff {
            Some(r) => format!("{chain} {r:.3} Å"),
            None => chain.to_string(),
        };
        match self.label.rsplit_once(" · ") {
            Some((prefix, _)) if self.label.starts_with("Spectrum ") => {
                format!("{prefix} · {name}")
            }
            _ => name,
        }
    }
}

/// Path-parameter columns and their readable symbols with units, as in the
/// path editor: amplitude reduction factor S₀², coordination number N,
/// threshold-energy shift ΔE₀ (eV), half-path-length change ΔR (Å),
/// mean-square relative displacement σ² (Å²), imaginary energy Eᵢ (eV) and
/// the third and fourth cumulants C₃ (Å³) and C₄ (Å⁴).
type PathColumn = fn(&FitPathSpec) -> &str;

const PATH_TERMS: [(&str, PathColumn); 8] = [
    ("S₀²", |p| &p.s02),
    ("N", |p| &p.degen),
    ("ΔE₀ (eV)", |p| &p.e0),
    ("ΔR (Å)", |p| &p.deltar),
    ("σ² (Å²)", |p| &p.sigma2),
    ("Eᵢ (eV)", |p| &p.ei),
    ("C₃ (Å³)", |p| &p.third),
    ("C₄ (Å⁴)", |p| &p.fourth),
];

/// Readable label of fit variable `name` from how the archived fit used it,
/// e.g. "ΔR (Å) · Cu–Cu 2.553 Å" for `dr_1` or "S₀²" for a shared `s02`.
///
/// A variable gets a label only when it is the whole expression of one
/// path-parameter column (any name, including the template names `s02`,
/// `e0`, `dr_<n>`, `ss_<n>`). The path is named when the variable belongs
/// to some but not all enabled paths; "+n" counts further paths sharing it.
/// Variables used in expressions, in several columns or not at all return
/// `None` and keep the name the user typed. `paths` are the fit's path
/// specifications and `details` the snapshot of its enabled paths, in the
/// same order as produced by [`snapshot`].
pub(crate) fn variable_label(
    name: &str,
    paths: &[FitPathSpec],
    details: &[PathFitDetails],
) -> Option<String> {
    let enabled: Vec<&FitPathSpec> = paths.iter().filter(|p| p.enabled).collect();
    let mut found = None;
    for (symbol, column) in PATH_TERMS {
        let users: Vec<usize> = enabled
            .iter()
            .enumerate()
            .filter(|(_, p)| column(p).trim() == name)
            .map(|(i, _)| i)
            .collect();
        if users.is_empty() {
            continue;
        }
        if found.is_some() {
            return None;
        }
        found = Some((symbol, users));
    }
    let (symbol, users) = found?;
    if users.len() == enabled.len() {
        return Some(symbol.to_string());
    }
    let Some(path) = details.get(users[0]) else {
        return Some(symbol.to_string());
    };
    Some(match users.len() {
        1 => format!("{symbol} · {}", path.title()),
        n => format!("{symbol} · {} +{}", path.title(), n - 1),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rexafs::xafs::fitting::FitVariable;
    #[test]
    fn effective_coordination_number_uses_the_feff_degeneracy_local() {
        let r = FeffFitResult::default();
        let n = estimate("degen", None, Some(6.0), &r).unwrap();
        assert_eq!(n.value, 6.0);
        assert!(n.fixed);
        let half = estimate("degen / 2", None, Some(6.0), &r).unwrap();
        assert_eq!(half.value, 3.0);
        assert!(estimate("degen", None, None, &r).is_none());
    }

    #[test]
    fn propagates_shared_constraints_and_covariance() {
        let mut r = FeffFitResult::default();
        r.variables.insert(
            "a",
            FitVariable {
                value: 0.01,
                vary: true,
                ..Default::default()
            },
        );
        r.variables.insert(
            "b",
            FitVariable {
                value: 0.02,
                vary: true,
                ..Default::default()
            },
        );
        r.variables.insert(
            "dr",
            FitVariable {
                expr: Some("2*a+b".into()),
                ..Default::default()
            },
        );
        r.varying_names = vec!["a".into(), "b".into()];
        r.covariance = Some(vec![vec![0.0001, 0.00005], vec![0.00005, 0.0004]]);
        let e = estimate("reff+dr", Some(2.5), None, &r).unwrap();
        assert!((e.value - 2.54).abs() < 1e-10);
        assert!((e.stderr.unwrap() - 0.001_f64.sqrt()).abs() < 1e-9);
        assert!(!e.fixed);
        r.covariance = None;
        assert!(estimate("dr", None, None, &r).unwrap().stderr.is_none());
        assert!(estimate("2.5", None, None, &r).unwrap().fixed);
        assert!(estimate("unknown", None, None, &r).is_none());
        r.variables.get_mut("a").unwrap().value = 0.;
        assert!(!estimate("a*a", None, None, &r).unwrap().fixed);
    }
    #[test]
    fn respects_bounds_when_differentiating() {
        let mut r = FeffFitResult::default();
        r.variables.insert(
            "a",
            FitVariable {
                value: 0.,
                vary: true,
                min: Some(0.),
                ..Default::default()
            },
        );
        r.varying_names = vec!["a".into()];
        r.covariance = Some(vec![vec![0.0004]]);
        assert!((estimate("a", None, None, &r).unwrap().stderr.unwrap() - 0.02).abs() < 1e-10);
    }

    fn details(label: &str, scatterers: Option<&str>, reff: Option<f64>) -> PathFitDetails {
        PathFitDetails {
            file: label.into(),
            label: label.into(),
            scatterers: scatterers.map(Into::into),
            reff,
            nleg: Some(2),
            degeneracy: None,
            effective_degen: None,
            distance: None,
            deltar: None,
            sigma2: None,
            s02: None,
            e0: None,
            n_s02: None,
            ei: None,
            third: None,
            fourth: None,
        }
    }

    fn spec(file: &str, deltar: &str, sigma2: &str) -> FitPathSpec {
        FitPathSpec {
            s02: "s02".into(),
            e0: "e0".into(),
            deltar: deltar.into(),
            sigma2: sigma2.into(),
            enabled: true,
            ..FitPathSpec::blank(file.into())
        }
    }

    #[test]
    fn path_title_names_the_scatterers_instead_of_the_file() {
        let p = details("feff0001.dat", Some("Cu–Cu"), Some(2.5527));
        assert_eq!(p.title(), "Cu–Cu 2.553 Å");
        let joint = details(
            "Spectrum 2 · cu_150k · feff0001.dat",
            Some("Cu–Cu"),
            Some(2.5527),
        );
        assert_eq!(joint.title(), "Spectrum 2 · cu_150k · Cu–Cu 2.553 Å");
        // History entries recorded before the chain was stored.
        let old = details("feff0001.dat", None, Some(2.5527));
        assert_eq!(old.title(), "feff0001.dat 2.553 Å");
        assert_eq!(details("feff0001.dat", None, None).title(), "feff0001.dat");
    }

    #[test]
    fn variable_labels_follow_the_path_column_that_uses_them() {
        let mut paths = vec![
            spec("feff0001.dat", "dr_1", "ss_1"),
            spec("feff0002.dat", "dr_2", "ss_2"),
            spec("feff0003.dat", "dr_1", "ss_1"),
        ];
        paths.push(FitPathSpec {
            enabled: false,
            ..spec("feff0004.dat", "dr_9", "ss_9")
        });
        let snap = vec![
            details("feff0001.dat", Some("Cu–Cu"), Some(2.5527)),
            details("feff0002.dat", Some("Cu–Cu"), Some(3.61)),
            details("feff0003.dat", Some("Cu–Cu–Cu"), Some(3.8292)),
        ];
        let label = |paths: &[FitPathSpec], name| variable_label(name, paths, &snap);
        assert_eq!(label(&paths, "s02").as_deref(), Some("S₀²"));
        assert_eq!(label(&paths, "e0").as_deref(), Some("ΔE₀ (eV)"));
        assert_eq!(
            label(&paths, "dr_2").as_deref(),
            Some("ΔR (Å) · Cu–Cu 3.610 Å")
        );
        assert_eq!(
            label(&paths, "ss_1").as_deref(),
            Some("σ² (Å²) · Cu–Cu 2.553 Å +1")
        );
        // Disabled paths and unused names keep the typed name.
        assert_eq!(label(&paths, "dr_9"), None);
        assert_eq!(label(&paths, "amp"), None);
        // A name used in two different columns is ambiguous.
        paths[1].sigma2 = "dr_2".into();
        assert_eq!(label(&paths, "dr_2"), None);
        // Part of an expression is not the parameter itself.
        paths[0].deltar = "dr_1 * 2".into();
        assert_eq!(
            label(&paths, "dr_1").as_deref(),
            Some("ΔR (Å) · Cu–Cu–Cu 3.829 Å")
        );
    }
}
