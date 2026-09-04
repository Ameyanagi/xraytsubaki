//! Parameter templates: turn a set of selected scattering paths into fit
//! variables and per-path parameter expressions in one step, instead of
//! creating a guess for every path.
//!
//! Naming follows Artemis/Larch conventions: `s02` and `e0` are shared by
//! every path; `dr_<n>` / `ss_<n>` are the ΔR / σ² of single-scattering
//! shell `n` (shell numbers come from [`crate::xafs::structure::pathrank`]).
//!
//! Multiple-scattering rule (documented here because there is no single
//! right answer): a MS path's ΔR is the mean of its scatterer legs' shell
//! `dr_<n>` (each leg counts once, so a Ru–Ru–Ru triangle through shell 1
//! twice is `dr_1`, and a shell-1/shell-2 path is `(dr_1 + dr_2) / 2`), and
//! its σ² is the *largest* constituent shell's `ss_<n>` — a MS path is
//! never sharper than its softest leg, and the sum would over-damp short
//! triangle paths. Legs that fall in no shell are ignored; a MS path with no
//! shell at all gets the first selected shell's parameters.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::xafs::structure::pathrank::PathInfo;

/// How the selected paths are parameterised.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ParameterTemplate {
    /// Shared `s02`, `e0`; one `dr_<n>` / `ss_<n>` per selected shell; MS
    /// paths derived from their constituent shells.
    #[default]
    PerShell,
    /// Shared `s02`, `e0`; `dr_<i>` / `ss_<i>` for every selected path.
    PerPath,
    /// Shared `s02`, `e0`; only shell 1's ΔR / σ² vary, every other path
    /// keeps ΔR = 0 and a fixed σ² = 0.003.
    FirstShellOnly,
    /// No variables: every cell is left empty for the user to fill in.
    Manual,
}

impl ParameterTemplate {
    pub const ALL: [ParameterTemplate; 4] = [
        ParameterTemplate::PerShell,
        ParameterTemplate::PerPath,
        ParameterTemplate::FirstShellOnly,
        ParameterTemplate::Manual,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ParameterTemplate::PerShell => "Per shell",
            ParameterTemplate::PerPath => "Per path",
            ParameterTemplate::FirstShellOnly => "First shell only",
            ParameterTemplate::Manual => "Manual",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            ParameterTemplate::PerShell => {
                "shared S₀² and E₀; ΔR and σ² per single-scattering shell; multiple-scattering paths follow their shells"
            }
            ParameterTemplate::PerPath => "shared S₀² and E₀; ΔR and σ² for every selected path",
            ParameterTemplate::FirstShellOnly => {
                "shared S₀² and E₀; only the first shell's ΔR and σ² vary, other paths fixed"
            }
            ParameterTemplate::Manual => "no variables created; fill the cells yourself",
        }
    }
}

/// A fit variable produced by a template.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateVariable {
    pub name: String,
    pub value: f64,
    pub vary: bool,
    pub min: Option<f64>,
    pub max: Option<f64>,
    /// A defined (derived) variable, when `Some`.
    pub expr: Option<String>,
}

/// Parameter expressions for one selected path.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PathAssignment {
    /// `PathInfo::index`.
    pub index: usize,
    pub s02: String,
    pub e0: String,
    pub deltar: String,
    pub sigma2: String,
}

/// Result of [`apply_template`].
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct TemplateResult {
    pub variables: Vec<TemplateVariable>,
    pub assignments: Vec<PathAssignment>,
    /// Human notes (e.g. which MS rule applied).
    pub notes: Vec<String>,
}

const S02_GUESS: f64 = 0.9;
const S02_MIN: f64 = 0.5;
const S02_MAX: f64 = 1.5;
const DR_LIMIT: f64 = 0.3;
const SS_GUESS: f64 = 0.003;

fn guess(name: &str, value: f64, min: Option<f64>, max: Option<f64>) -> TemplateVariable {
    TemplateVariable {
        name: name.to_string(),
        value,
        vary: true,
        min,
        max,
        expr: None,
    }
}

fn shared(vars: &mut Vec<TemplateVariable>) {
    vars.push(guess("s02", S02_GUESS, Some(S02_MIN), Some(S02_MAX)));
    vars.push(guess("e0", 0.0, None, None));
}

/// Build the variables and per-path expressions for `selected` paths.
pub fn apply_template(template: ParameterTemplate, selected: &[PathInfo]) -> TemplateResult {
    let mut out = TemplateResult::default();
    if selected.is_empty() {
        return out;
    }
    match template {
        ParameterTemplate::Manual => {
            for p in selected {
                out.assignments.push(PathAssignment {
                    index: p.index,
                    s02: String::new(),
                    e0: String::new(),
                    deltar: String::new(),
                    sigma2: String::new(),
                });
            }
        }
        ParameterTemplate::PerPath => {
            shared(&mut out.variables);
            for (i, p) in selected.iter().enumerate() {
                let n = i + 1;
                out.variables
                    .push(guess(&format!("dr_{n}"), 0.0, Some(-DR_LIMIT), Some(DR_LIMIT)));
                out.variables
                    .push(guess(&format!("ss_{n}"), SS_GUESS, Some(0.0), None));
                out.assignments.push(PathAssignment {
                    index: p.index,
                    s02: "s02".into(),
                    e0: "e0".into(),
                    deltar: format!("dr_{n}"),
                    sigma2: format!("ss_{n}"),
                });
            }
        }
        ParameterTemplate::PerShell | ParameterTemplate::FirstShellOnly => {
            shared(&mut out.variables);
            // Shells touched by the selection (SS shells, plus MS legs).
            let mut shells: BTreeSet<usize> = BTreeSet::new();
            for p in selected {
                if p.is_single_scattering && p.shell > 0 {
                    shells.insert(p.shell);
                }
                for &s in &p.leg_shells {
                    if s > 0 {
                        shells.insert(s);
                    }
                }
            }
            let first_shell = shells.iter().next().copied().unwrap_or(1);
            let vary_shells: Vec<usize> = match template {
                ParameterTemplate::FirstShellOnly => vec![first_shell],
                _ => shells.iter().copied().collect(),
            };
            for &n in &vary_shells {
                out.variables
                    .push(guess(&format!("dr_{n}"), 0.0, Some(-DR_LIMIT), Some(DR_LIMIT)));
                out.variables
                    .push(guess(&format!("ss_{n}"), SS_GUESS, Some(0.0), None));
            }
            let mut ms_noted = false;
            for p in selected {
                let (deltar, sigma2) = if p.is_single_scattering {
                    let n = if p.shell > 0 { p.shell } else { first_shell };
                    shell_params(n, &vary_shells)
                } else {
                    let legs: Vec<usize> = p.leg_shells.iter().copied().filter(|&s| s > 0).collect();
                    if legs.is_empty() {
                        shell_params(first_shell, &vary_shells)
                    } else {
                        if !ms_noted {
                            out.notes.push(
                                "multiple-scattering paths: ΔR = mean of the legs' shell ΔR, σ² = the largest constituent shell's σ²"
                                    .into(),
                            );
                            ms_noted = true;
                        }
                        ms_params(&legs, &vary_shells)
                    }
                };
                out.assignments.push(PathAssignment {
                    index: p.index,
                    s02: "s02".into(),
                    e0: "e0".into(),
                    deltar,
                    sigma2,
                });
            }
        }
    }
    out
}

fn shell_params(n: usize, vary_shells: &[usize]) -> (String, String) {
    if vary_shells.contains(&n) {
        (format!("dr_{n}"), format!("ss_{n}"))
    } else {
        ("0".into(), format!("{SS_GUESS}"))
    }
}

fn ms_params(legs: &[usize], vary_shells: &[usize]) -> (String, String) {
    let terms: Vec<String> = legs
        .iter()
        .map(|&n| if vary_shells.contains(&n) { format!("dr_{n}") } else { "0".to_string() })
        .collect();
    let deltar = if terms.iter().all(|t| t == "0") {
        "0".to_string()
    } else if terms.len() == 1 {
        terms[0].clone()
    } else if terms.iter().all(|t| t == &terms[0]) {
        terms[0].clone()
    } else {
        format!("({}) / {}", terms.join(" + "), terms.len())
    };
    let largest = legs.iter().copied().max().unwrap_or(1);
    let sigma2 = if vary_shells.contains(&largest) {
        format!("ss_{largest}")
    } else {
        format!("{SS_GUESS}")
    };
    (deltar, sigma2)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(index: usize, label: &str, reff: f64, nleg: usize, shell: usize, legs: &[usize]) -> PathInfo {
        PathInfo {
            index,
            filename: format!("feff{index:04}.dat"),
            label: label.into(),
            reff,
            degen: 6.0,
            nleg,
            importance: 50.0,
            shell,
            leg_shells: legs.to_vec(),
            is_single_scattering: nleg <= 2,
        }
    }

    #[test]
    fn per_shell_shares_variables_and_derives_ms_paths() {
        let sel = vec![
            info(0, "Ru–Ru", 2.65, 2, 1, &[1]),
            info(1, "Ru–Ru", 2.70, 2, 1, &[1]),
            info(2, "Ru–Ru", 3.79, 2, 2, &[2]),
            info(3, "Ru–Ru–Ru", 4.0, 3, 1, &[1, 1]),
            info(4, "Ru–Ru–Ru", 4.7, 3, 2, &[1, 2]),
        ];
        let r = apply_template(ParameterTemplate::PerShell, &sel);
        let names: Vec<&str> = r.variables.iter().map(|v| v.name.as_str()).collect();
        assert_eq!(names, vec!["s02", "e0", "dr_1", "ss_1", "dr_2", "ss_2"]);
        assert_eq!(r.assignments[0].deltar, "dr_1");
        assert_eq!(r.assignments[1].sigma2, "ss_1");
        assert_eq!(r.assignments[2].deltar, "dr_2");
        assert_eq!(r.assignments[3].deltar, "dr_1");
        assert_eq!(r.assignments[3].sigma2, "ss_1");
        assert_eq!(r.assignments[4].deltar, "(dr_1 + dr_2) / 2");
        assert_eq!(r.assignments[4].sigma2, "ss_2");
        assert_eq!(r.notes.len(), 1);
        let s02 = &r.variables[0];
        assert_eq!((s02.value, s02.min, s02.max), (0.9, Some(0.5), Some(1.5)));
    }

    #[test]
    fn first_shell_only_and_per_path_and_manual() {
        let sel = vec![
            info(0, "Ru–Ru", 2.65, 2, 1, &[1]),
            info(2, "Ru–Ru", 3.79, 2, 2, &[2]),
        ];
        let r = apply_template(ParameterTemplate::FirstShellOnly, &sel);
        assert_eq!(r.variables.len(), 4);
        assert_eq!(r.assignments[1].deltar, "0");
        assert_eq!(r.assignments[1].sigma2, "0.003");
        let r = apply_template(ParameterTemplate::PerPath, &sel);
        assert_eq!(r.variables.len(), 6);
        assert_eq!(r.assignments[1].deltar, "dr_2");
        let r = apply_template(ParameterTemplate::Manual, &sel);
        assert!(r.variables.is_empty());
        assert!(r.assignments.iter().all(|a| a.s02.is_empty()));
        assert!(apply_template(ParameterTemplate::PerShell, &[]).assignments.is_empty());
    }
}
