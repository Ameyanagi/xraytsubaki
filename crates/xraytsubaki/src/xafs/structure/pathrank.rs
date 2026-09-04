//! Ranking and default selection of FEFF scattering paths.
//!
//! FEFF writes every path below its `RPATH` cutoff, which for a typical
//! 6–8 Å cluster is dozens of files, most of which contribute nothing a fit
//! can resolve. [`rank_paths`] turns the parsed path files into
//! [`PathInfo`] records with a human label, an importance estimate, a shell
//! assignment and the constituent shells of multiple-scattering paths, and
//! [`select_default`] picks the single-scattering shells inside the fit
//! window that carry real amplitude.

use serde::{Deserialize, Serialize};

use super::element::Element;
use crate::xafs::fitting::types::FeffDat;

/// Shell grouping tolerance for single-scattering distances (Å): paths
/// closer than this share a shell (hcp first-shell splits of ~0.05 Å are
/// fitted with one ΔR / σ², as in common practice).
pub const SHELL_TOL: f64 = 0.1;
/// Tolerance when matching a scatterer's distance to a shell (Å).
const LEG_SHELL_TOL: f64 = 0.10;
/// k range used for the importance estimate (Å⁻¹).
const IMPORTANCE_KMIN: f64 = 3.0;
const IMPORTANCE_KMAX: f64 = 12.0;

/// Summary of one path as the picker and the parameter templates see it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PathInfo {
    /// Index into the slice handed to [`rank_paths`].
    pub index: usize,
    /// Path file name (`feff0001.dat`).
    pub filename: String,
    /// Absorber–scatterer chain, e.g. `Ru–Ru` or `Ru–O–Ru`.
    pub label: String,
    pub reff: f64,
    pub degen: f64,
    pub nleg: usize,
    /// Relative amplitude, 0–100 (100 = strongest path of the set).
    pub importance: f64,
    /// 1-based single-scattering shell this path belongs to (an SS path's
    /// own shell; for MS paths the outermost constituent shell), 0 when no
    /// shell could be assigned.
    pub shell: usize,
    /// For every scatterer leg, the 1-based shell it sits in (0 = none).
    pub leg_shells: Vec<usize>,
    pub is_single_scattering: bool,
}

impl PathInfo {
    /// Distinct constituent shells (sorted, zeros removed).
    pub fn shells(&self) -> Vec<usize> {
        let mut s: Vec<usize> = self.leg_shells.iter().copied().filter(|&s| s > 0).collect();
        s.sort_unstable();
        s.dedup();
        s
    }
}

/// One single-scattering shell: paths whose `reff` agree within
/// [`SHELL_TOL`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShellInfo {
    /// 1-based shell number, by increasing distance.
    pub number: usize,
    pub reff: f64,
    /// Scatterer element (of the first path in the shell).
    pub symbol: String,
    /// Absorber element.
    pub absorber: String,
    /// Total degeneracy of the shell's SS paths.
    pub degeneracy: f64,
    /// Indices (into the ranked slice) of the SS paths in this shell.
    pub paths: Vec<usize>,
}

fn symbol_of(z: u8, label: &str) -> String {
    Element::from_z(z)
        .map(|e| e.symbol.to_string())
        .unwrap_or_else(|| {
            let s: String = label.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
            if s.is_empty() {
                format!("Z{z}")
            } else {
                s
            }
        })
}

/// Absorber–scatterer chain label from the leg atoms.
pub fn path_label(dat: &FeffDat) -> String {
    if dat.geometry_atoms.is_empty() {
        return dat.filename.clone();
    }
    dat.geometry_atoms
        .iter()
        .map(|a| symbol_of(a.atomic_number, &a.label))
        .collect::<Vec<_>>()
        .join("–")
}

/// Raw amplitude estimate: the EXAFS prefactor
/// `N |f(k)| R(k) exp(-2 reff/λ) / (k reff²)` averaged over 3–12 Å⁻¹.
fn raw_importance(dat: &FeffDat) -> f64 {
    let n = dat.k.len().min(dat.mag_feff.len());
    if n == 0 || dat.reff <= 0.0 {
        return 0.0;
    }
    let mut sum = 0.0;
    let mut count = 0usize;
    for i in 0..n {
        let k = dat.k[i];
        if !(IMPORTANCE_KMIN..=IMPORTANCE_KMAX).contains(&k) || k <= 0.0 {
            continue;
        }
        let red = dat.red_fact.get(i).copied().unwrap_or(1.0);
        let lam = dat.lam.get(i).copied().unwrap_or(0.0);
        let damp = if lam > 0.0 {
            (-2.0 * dat.reff / lam).exp()
        } else {
            1.0
        };
        sum += dat.degen * dat.mag_feff[i].abs() * red * damp / (k * dat.reff * dat.reff);
        count += 1;
    }
    if count == 0 {
        0.0
    } else {
        sum / count as f64
    }
}

/// Rank paths: labels, importance (0–100), shells and constituent shells.
/// The returned vector is in the input order (`index` = position).
pub fn rank_paths(paths: &[FeffDat]) -> Vec<PathInfo> {
    let raw: Vec<f64> = paths.iter().map(raw_importance).collect();
    let max = raw.iter().copied().fold(0.0_f64, f64::max);
    let mut infos: Vec<PathInfo> = paths
        .iter()
        .enumerate()
        .map(|(i, dat)| PathInfo {
            index: i,
            filename: dat.filename.clone(),
            label: path_label(dat),
            reff: dat.reff,
            degen: dat.degen,
            nleg: dat.nleg,
            importance: if max > 0.0 { 100.0 * raw[i] / max } else { 0.0 },
            shell: 0,
            leg_shells: Vec::new(),
            is_single_scattering: dat.nleg <= 2,
        })
        .collect();
    let shells = shells_of(&infos);
    for (i, info) in infos.iter_mut().enumerate() {
        let dat = &paths[i];
        // Scatterer distances from the absorber (legs after the first).
        let leg_shells: Vec<usize> = if dat.geometry_atoms.len() > 1 {
            dat.geometry_atoms
                .iter()
                .skip(1)
                .map(|a| {
                    let d = (a.x * a.x + a.y * a.y + a.z * a.z).sqrt();
                    nearest_shell(&shells, d)
                })
                .collect()
        } else if info.is_single_scattering {
            vec![nearest_shell(&shells, info.reff)]
        } else {
            Vec::new()
        };
        info.shell = if info.is_single_scattering {
            shells
                .iter()
                .find(|s| s.paths.contains(&i))
                .map(|s| s.number)
                .unwrap_or(0)
        } else {
            leg_shells.iter().copied().max().unwrap_or(0)
        };
        info.leg_shells = leg_shells;
    }
    infos
}

fn nearest_shell(shells: &[ShellInfo], d: f64) -> usize {
    shells
        .iter()
        .map(|s| (s.number, (s.reff - d).abs()))
        .filter(|(_, dd)| *dd <= LEG_SHELL_TOL)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(n, _)| n)
        .unwrap_or(0)
}

/// Single-scattering shells of a ranked set, by increasing distance.
pub fn shells_of(paths: &[PathInfo]) -> Vec<ShellInfo> {
    let mut ss: Vec<&PathInfo> = paths.iter().filter(|p| p.is_single_scattering).collect();
    ss.sort_by(|a, b| a.reff.total_cmp(&b.reff));
    let mut shells: Vec<ShellInfo> = Vec::new();
    for p in ss {
        let (absorber, symbol) = split_label(&p.label);
        match shells.last_mut() {
            Some(last) if (p.reff - last.reff).abs() <= SHELL_TOL + 1e-9 && last.symbol == symbol => {
                last.paths.push(p.index);
                last.degeneracy += p.degen;
            }
            _ => shells.push(ShellInfo {
                number: shells.len() + 1,
                reff: p.reff,
                symbol,
                absorber,
                degeneracy: p.degen,
                paths: vec![p.index],
            }),
        }
    }
    shells
}

fn split_label(label: &str) -> (String, String) {
    let mut parts = label.split('–');
    let absorber = parts.next().unwrap_or("").to_string();
    let scatterer = parts.next().unwrap_or("").to_string();
    (absorber, scatterer)
}

/// Default selection: single-scattering paths with `reff ≤ r_max_fit + 0.3 Å`
/// and importance ≥ 10 %. Returns indices into the ranked slice.
pub fn select_default(paths: &[PathInfo], r_max_fit: f64) -> Vec<usize> {
    select_by(paths, r_max_fit + 0.3, 10.0, true)
}

/// Generic preset: paths with `reff ≤ r_max`, importance ≥ `min_importance`,
/// optionally single scattering only.
pub fn select_by(paths: &[PathInfo], r_max: f64, min_importance: f64, ss_only: bool) -> Vec<usize> {
    paths
        .iter()
        .filter(|p| p.reff <= r_max && p.importance >= min_importance)
        .filter(|p| !ss_only || p.is_single_scattering)
        .map(|p| p.index)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xafs::fitting::types::PathAtom;
    use nalgebra::DVector;

    fn dat(name: &str, reff: f64, degen: f64, atoms: &[(f64, u8)], amp: f64) -> FeffDat {
        let k: Vec<f64> = (0..40).map(|i| i as f64 * 0.5).collect();
        let n = k.len();
        let mut geometry_atoms = vec![PathAtom {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            ipot: 0,
            atomic_number: 44,
            label: "Ru".into(),
        }];
        for (d, z) in atoms {
            geometry_atoms.push(PathAtom {
                x: *d,
                y: 0.0,
                z: 0.0,
                ipot: 1,
                atomic_number: *z,
                label: String::new(),
            });
        }
        FeffDat {
            filename: name.into(),
            reff,
            degen,
            nleg: geometry_atoms.len(),
            k: DVector::from_vec(k),
            mag_feff: DVector::from_element(n, amp),
            red_fact: DVector::from_element(n, 1.0),
            lam: DVector::from_element(n, 10.0),
            geometry_atoms,
            ..Default::default()
        }
    }

    #[test]
    fn labels_shells_and_default_selection() {
        let paths = vec![
            dat("feff0001.dat", 2.65, 6.0, &[(2.65, 44)], 1.0),
            dat("feff0002.dat", 2.70, 6.0, &[(2.70, 44)], 1.0),
            dat("feff0003.dat", 3.79, 6.0, &[(3.79, 44)], 0.6),
            dat("feff0004.dat", 4.00, 12.0, &[(2.65, 44), (2.70, 44)], 0.05),
            dat("feff0005.dat", 4.64, 12.0, &[(4.64, 8)], 0.02),
        ];
        let ranked = rank_paths(&paths);
        assert_eq!(ranked[0].label, "Ru–Ru");
        assert_eq!(ranked[3].label, "Ru–Ru–Ru");
        assert_eq!(ranked[4].label, "Ru–O");
        assert!(!ranked[3].is_single_scattering);
        let shells = shells_of(&ranked);
        assert_eq!(shells.len(), 3, "{shells:?}");
        assert_eq!(shells[0].paths, vec![0, 1]);
        assert_eq!(shells[0].degeneracy, 12.0);
        assert_eq!(ranked[0].shell, 1);
        assert_eq!(ranked[2].shell, 2);
        assert_eq!(ranked[3].leg_shells, vec![1, 1]);
        assert_eq!(ranked[3].shell, 1);
        assert_eq!(ranked[4].shell, 3);
        assert_eq!(ranked[0].importance, 100.0);
        assert!(ranked[3].importance < 10.0);
        assert_eq!(select_default(&ranked, 3.0), vec![0, 1]);
        assert_eq!(select_default(&ranked, 3.6), vec![0, 1, 2]);
        assert_eq!(select_by(&ranked, 6.0, 0.0, false).len(), 5);
    }
}
