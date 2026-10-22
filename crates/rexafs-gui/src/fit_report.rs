//! Markdown export of one fit (Larch `feffit_report` layout): statistics,
//! fit ranges, variables, correlations and one block per path with the
//! physical values, their uncertainties and the expressions behind them.
use crate::fit_details::Estimate;
use crate::fitting::{FitHistoryEntry, FitPathSpec, fit_result_notices, high_correlations};
use rexafs::xafs::fitting::FeffFitResult;

/// Correlations weaker than this are left out, as in Larch.
const CORRELATION_MIN: f64 = 0.1;

fn est(e: &Option<Estimate>, digits: usize, unit: &str) -> String {
    match e {
        Some(e) => {
            let unit = if unit.is_empty() {
                String::new()
            } else {
                format!(" {unit}")
            };
            if e.fixed {
                format!("{:.*}{unit} (fixed)", digits, e.value)
            } else if let Some(err) = e.stderr {
                format!("{:.*} ± {:.*}{unit}", digits, e.value, digits, err)
            } else {
                format!("{:.*}{unit} (± unavailable)", digits, e.value)
            }
        }
        None => "unavailable".into(),
    }
}

fn opt(v: Option<f64>, digits: usize) -> String {
    v.map(|v| format!("{v:.*}", digits))
        .unwrap_or_else(|| "?".into())
}

fn cell(text: &str) -> String {
    text.replace('|', "\\|")
}

fn expr_cell(expr: &str) -> String {
    let expr = expr.trim();
    if expr.is_empty() {
        String::new()
    } else {
        format!("`{}`", cell(expr))
    }
}

/// Builds the report. `result` adds the noise estimate, correlations and
/// warnings that the archived history entry does not carry; the report is
/// still complete without it (older projects).
pub(crate) fn markdown(entry: &FitHistoryEntry, result: Option<&FeffFitResult>) -> String {
    let joint = entry.joint.as_ref();
    let name = |n: &str| crate::joint_fitting::display_name(n, joint);
    let mut out = String::new();
    out.push_str(&format!("# Fit {} · {}\n\n", entry.id, cell(&entry.group)));

    // ---- statistics ---------------------------------------------------
    out.push_str("## Statistics\n\n| quantity | value |\n|---|---:|\n");
    out.push_str(&format!("| R-factor | {:.5} |\n", entry.r_factor));
    out.push_str(&format!("| χ² | {:.4e} |\n", entry.chi_square));
    out.push_str(&format!(
        "| reduced χ² | {} |\n",
        entry.reduced_chi_square_text()
    ));
    out.push_str(&format!(
        "| N idp / N var | {:.1} / {} |\n",
        entry.n_idp, entry.n_vary
    ));
    if let Some(r) = result {
        out.push_str(&format!("| N data | {} |\n", r.n_data));
        if !entry.noise_estimated() {
            out.push_str("| ε k (noise) | not estimated (unit scale) |\n");
        } else if let Some(k) = r.kweight_results.first() {
            out.push_str(&format!("| ε k (noise) | {:.3e} |\n", k.epsilon_k));
            out.push_str(&format!("| ε R (noise) | {:.3e} |\n", k.epsilon_r));
        }
    }
    if let Some(report) = &entry.solver_report {
        out.push_str(&format!(
            "| optimizer | {:?} · {} |\n",
            report.method,
            if report.converged {
                "converged"
            } else {
                "stopped"
            }
        ));
        if let Some(n) = report.iterations {
            out.push_str(&format!("| iterations | {n} |\n"));
        }
        if let Some(n) = report.evaluations {
            out.push_str(&format!("| evaluations | {n} |\n"));
        }
        out.push_str(&format!(
            "| objective | {:.3e} → {:.3e} |\n",
            report.initial_cost, report.final_cost
        ));
        out.push_str(&format!(
            "| termination | {} |\n",
            cell(&report.termination)
        ));
    }

    // ---- ranges ---------------------------------------------------------
    let r = &entry.ranges;
    let kweights = r
        .effective_kweights()
        .iter()
        .map(|k| format!("{k}"))
        .collect::<Vec<_>>()
        .join(", ");
    out.push_str("\n## Fit ranges\n\n| space | k-range (Å⁻¹) | k-weights | R-range (Å) | noise weighting |\n|---|---|---|---|---|\n");
    out.push_str(&format!(
        "| {} | {:.3} – {:.3} | {} | {:.3} – {:.3} | {} |\n",
        r.fitspace.label(),
        r.kmin,
        r.kmax,
        kweights,
        r.rmin,
        r.rmax,
        if r.noise { "on" } else { "off" }
    ));

    // ---- variables ------------------------------------------------------
    out.push_str("\n## Variables\n\n| name | kind | value | ± | initial | bounds | expression |\n|---|---|---:|---:|---:|---|---|\n");
    for v in &entry.vars {
        let fitted = entry.values.iter().find(|(n, _, _)| n == &v.name);
        let live = result.and_then(|r| r.variables.get(&v.name));
        let (kind, value, err) = if let Some(expr) = &v.expr {
            let _ = expr;
            ("expr", live.map(|x| x.value), None)
        } else if v.vary {
            (
                "vary",
                fitted.map(|(_, x, _)| *x).or(live.map(|x| x.value)),
                fitted
                    .and_then(|(_, _, e)| *e)
                    .or(live.and_then(|x| x.stderr)),
            )
        } else {
            ("fixed", Some(v.value), None)
        };
        let bounds = match (v.min, v.max) {
            (None, None) => String::new(),
            (lo, hi) => format!(
                "{} … {}",
                lo.map(|x| format!("{x}")).unwrap_or_else(|| "−∞".into()),
                hi.map(|x| format!("{x}")).unwrap_or_else(|| "∞".into())
            ),
        };
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} |\n",
            cell(&name(&v.name)),
            kind,
            value
                .map(|x| format!("{x:.5}"))
                .unwrap_or_else(|| "—".into()),
            err.map(|e| format!("{e:.5}")).unwrap_or_default(),
            v.value,
            bounds,
            v.expr.as_deref().map(expr_cell).unwrap_or_default()
        ));
    }

    // ---- correlations ---------------------------------------------------
    if let Some(r) = result {
        let corr = high_correlations(r, CORRELATION_MIN);
        if !corr.is_empty() {
            out.push_str(&format!(
                "\n## Correlations (|r| ≥ {CORRELATION_MIN:.2})\n\n| a | b | r |\n|---|---|---:|\n"
            ));
            for (a, b, c) in corr {
                out.push_str(&format!(
                    "| {} | {} | {c:+.3} |\n",
                    cell(&name(&a)),
                    cell(&name(&b))
                ));
            }
        }
    }

    // ---- paths ----------------------------------------------------------
    out.push_str("\n## Paths\n");
    let specs: Vec<&FitPathSpec> = entry.paths.iter().filter(|p| p.enabled).collect();
    if entry.path_details.is_empty() {
        out.push_str("\nPath values were not recorded for this fit (older history entry).\n");
    }
    for (i, p) in entry.path_details.iter().enumerate() {
        let spec = specs.get(i).copied();
        let e =
            |f: fn(&FitPathSpec) -> &String| spec.map(f).map(|s| expr_cell(s)).unwrap_or_default();
        let source = p
            .file
            .parent()
            .and_then(|d| d.file_name())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let file = p
            .file
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| p.file.display().to_string());
        out.push_str(&format!(
            "\n### {} · {} · {}\n\n",
            i + 1,
            cell(&p.label),
            cell(&file)
        ));
        if !source.is_empty() {
            out.push_str(&format!("FEFF run `{}`", cell(&source)));
            if let Some(n) = p.nleg {
                out.push_str(&format!(" · {n} legs"));
            }
            out.push_str("\n\n");
        }
        out.push_str("| quantity | value | expression |\n|---|---:|---|\n");
        out.push_str(&format!("| R_eff | {} Å | |\n", opt(p.reff, 4)));
        out.push_str(&format!(
            "| FEFF degeneracy | {} | |\n",
            opt(p.degeneracy, 0)
        ));
        let n_expr = spec
            .map(|s| {
                if s.degen.trim().is_empty() {
                    "`degen`".to_string()
                } else {
                    expr_cell(&s.degen)
                }
            })
            .unwrap_or_default();
        out.push_str(&format!(
            "| N | {} | {} |\n",
            est(&p.effective_degen, 2, ""),
            n_expr
        ));
        out.push_str(&format!("| N·S₀² | {} | |\n", est(&p.n_s02, 3, "")));
        out.push_str(&format!(
            "| S₀² | {} | {} |\n",
            est(&p.s02, 3, ""),
            e(|s| &s.s02)
        ));
        out.push_str(&format!(
            "| ΔE₀ | {} | {} |\n",
            est(&p.e0, 3, "eV"),
            e(|s| &s.e0)
        ));
        let r_expr = spec
            .map(|s| expr_cell(&format!("reff + {}", s.deltar.trim())))
            .unwrap_or_default();
        out.push_str(&format!(
            "| R | {} | {} |\n",
            est(&p.distance, 4, "Å"),
            r_expr
        ));
        out.push_str(&format!(
            "| ΔR | {} | {} |\n",
            est(&p.deltar, 4, "Å"),
            e(|s| &s.deltar)
        ));
        out.push_str(&format!(
            "| σ² | {} | {} |\n",
            est(&p.sigma2, 5, "Å²"),
            e(|s| &s.sigma2)
        ));
        if p.ei.is_some() || spec.is_some_and(|s| !s.ei.trim().is_empty()) {
            out.push_str(&format!(
                "| Ei | {} | {} |\n",
                est(&p.ei, 3, "eV"),
                e(|s| &s.ei)
            ));
        }
        if p.third.is_some() || spec.is_some_and(|s| !s.third.trim().is_empty()) {
            out.push_str(&format!(
                "| C₃ | {} | {} |\n",
                est(&p.third, 6, "Å³"),
                e(|s| &s.third)
            ));
        }
        if p.fourth.is_some() || spec.is_some_and(|s| !s.fourth.trim().is_empty()) {
            out.push_str(&format!(
                "| C₄ | {} | {} |\n",
                est(&p.fourth, 6, "Å⁴"),
                e(|s| &s.fourth)
            ));
        }
    }

    // ---- notices --------------------------------------------------------
    if let Some(r) = result {
        let notices = fit_result_notices(r);
        if !notices.is_empty() || !r.warnings.is_empty() {
            out.push_str("\n## Notes\n\n");
            for n in notices {
                out.push_str(&format!("- {n}\n"));
            }
            for w in &r.warnings {
                out.push_str(&format!("- {}\n", cell(&w.message)));
            }
        }
    }
    out.push_str("\nUncertainties are one standard error propagated from the fit covariance; the FEFF reference geometry is treated as exact.\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fitting::{FitPathSpec, FitRanges, FitVarSpec};
    use rexafs::xafs::fitting::FitVariable;
    use std::path::PathBuf;

    fn entry() -> (FitHistoryEntry, FeffFitResult) {
        let file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../rexafs/tests/testfiles/feffcu01.dat");
        let mut spec = FitPathSpec::standard(file, 1);
        spec.degen = "n1".into();
        let vars = vec![
            FitVarSpec {
                name: "amp".into(),
                value: 0.9,
                vary: false,
                min: None,
                max: None,
                expr: None,
            },
            FitVarSpec {
                name: "n1".into(),
                value: 12.0,
                vary: true,
                min: Some(0.0),
                max: None,
                expr: None,
            },
            FitVarSpec {
                name: "de0".into(),
                value: 0.0,
                vary: true,
                min: None,
                max: None,
                expr: None,
            },
            FitVarSpec {
                name: "dr_1".into(),
                value: 0.0,
                vary: true,
                min: None,
                max: None,
                expr: None,
            },
            FitVarSpec {
                name: "sig2_1".into(),
                value: 0.003,
                vary: true,
                min: None,
                max: None,
                expr: None,
            },
        ];
        let mut result = FeffFitResult::default();
        for (n, v, e) in [
            ("amp", 0.9, None),
            ("n1", 11.4, Some(0.8)),
            ("de0", 4.2, Some(0.5)),
            ("dr_1", -0.006, Some(0.004)),
            ("sig2_1", 0.0087, Some(0.0004)),
        ] {
            result.variables.insert(
                n,
                FitVariable {
                    value: v,
                    vary: e.is_some(),
                    stderr: e,
                    ..Default::default()
                },
            );
        }
        result.varying_names = vec!["n1".into(), "de0".into(), "dr_1".into(), "sig2_1".into()];
        result.n_vary = 4;
        result.r_factor = 0.0123;
        result.n_idp = 11.2;
        let entry = FitHistoryEntry::from_result(
            3,
            "cu_150k.xmu".into(),
            vec![spec],
            vars,
            FitRanges::default(),
            &result,
        );
        (entry, result)
    }

    #[test]
    fn report_lists_statistics_variables_and_path_values() {
        let (entry, result) = entry();
        let md = markdown(&entry, Some(&result));
        assert!(md.starts_with("# Fit 3 · cu_150k.xmu\n"));
        assert!(md.contains("| R-factor | 0.01230 |"));
        assert!(md.contains("| n1 | vary | 11.40000 | 0.80000 | 12 | 0 … ∞ |  |"));
        assert!(md.contains("| amp | fixed | 0.90000 |  | 0.9 |  |  |"));
        assert!(md.contains("### 1 · feffcu01.dat · feffcu01.dat"));
        // FeffFitResult::default() carries no covariance, so the archived
        // estimates have values but no propagated uncertainty.
        assert!(
            md.contains("| N | 11.40") && md.contains("| `n1` |"),
            "{md}"
        );
        assert!(md.contains("| N·S₀² | 10.260"), "{md}");
        assert!(
            md.contains("| S₀² | 0.900") && md.contains("| `amp` |"),
            "{md}"
        );
        assert!(
            md.contains("| ΔR | -0.0060") && md.contains("| `dr_1` |"),
            "{md}"
        );
        assert!(md.contains("| R | ") && md.contains("`reff + dr_1`"));
        assert!(
            !md.contains("| Ei |"),
            "unused cumulants stay out of the report"
        );
    }

    #[test]
    fn reduced_chi_square_needs_a_noise_estimate() {
        let (mut entry, mut result) = entry();
        entry.reduced_chi_square = 4.5e-5;
        result.reduced_chi_square = 4.5e-5;
        let md = markdown(&entry, Some(&result));
        assert!(
            md.contains("| reduced χ² | n/a (noise not estimated) |"),
            "{md}"
        );
        assert!(
            md.contains("| ε k (noise) | not estimated (unit scale) |"),
            "{md}"
        );
        assert!(!md.contains("4.5000e-5"), "{md}");

        entry.ranges.noise = true;
        result.kweight_results = vec![Default::default()];
        result.kweight_results[0].epsilon_k = 2.5e-4;
        let md = markdown(&entry, Some(&result));
        assert!(md.contains("| reduced χ² | 4.5000e-5 |"), "{md}");
        assert!(md.contains("| ε k (noise) | 2.500e-4 |"), "{md}");
    }

    #[test]
    fn report_without_live_result_still_renders() {
        let (entry, _) = entry();
        let md = markdown(&entry, None);
        assert!(md.contains("## Paths"));
        assert!(!md.contains("## Correlations"));
        assert!(md.contains("| de0 | vary | 4.20000 | 0.50000 |"));
    }
}
