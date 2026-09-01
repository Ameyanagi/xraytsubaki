//! FEFF10 helper: create a feff.inp workspace and run the selected embedded
//! FEFFRS or pure-Rust ReFEFF backend to generate fitting path files.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use xraytsubaki::prelude::*;

/// Atoms-lite: build a feff.inp from a simple crystal description
/// (element(s), common structure type, lattice constants, edge, cluster
/// radius) — covers the common metal/oxide cases without full space-group
/// machinery; arbitrary structures can still be pasted into feff.inp.
pub struct CrystalSpec {
    pub element: String,
    /// Second element for binary structures (rocksalt, zincblende, cscl).
    pub element2: Option<String>,
    /// fcc | bcc | hcp | diamond | rocksalt | zincblende | cscl
    pub structure: String,
    pub a: f64,
    /// c axis for hcp (defaults to ideal c/a).
    pub c: Option<f64>,
    /// K | L1 | L2 | L3
    pub edge: String,
    pub rmax: f64,
}

const SYMBOLS: &[&str] = &[
    "H", "He", "Li", "Be", "B", "C", "N", "O", "F", "Ne", "Na", "Mg", "Al", "Si", "P", "S", "Cl",
    "Ar", "K", "Ca", "Sc", "Ti", "V", "Cr", "Mn", "Fe", "Co", "Ni", "Cu", "Zn", "Ga", "Ge", "As",
    "Se", "Br", "Kr", "Rb", "Sr", "Y", "Zr", "Nb", "Mo", "Tc", "Ru", "Rh", "Pd", "Ag", "Cd", "In",
    "Sn", "Sb", "Te", "I", "Xe", "Cs", "Ba", "La", "Ce", "Pr", "Nd", "Pm", "Sm", "Eu", "Gd", "Tb",
    "Dy", "Ho", "Er", "Tm", "Yb", "Lu", "Hf", "Ta", "W", "Re", "Os", "Ir", "Pt", "Au", "Hg", "Tl",
    "Pb", "Bi", "Po", "At", "Rn", "Fr", "Ra", "Ac", "Th", "Pa", "U",
];

fn atomic_number(symbol: &str) -> Option<usize> {
    SYMBOLS
        .iter()
        .position(|s| s.eq_ignore_ascii_case(symbol))
        .map(|i| i + 1)
}

fn hole_index(edge: &str) -> Option<u32> {
    match edge.trim().to_ascii_uppercase().as_str() {
        "K" => Some(1),
        "L1" => Some(2),
        "L2" => Some(3),
        "L3" => Some(4),
        _ => None,
    }
}

type Atom = (f64, f64, f64, usize); // x, y, z, potential index
type Basis = Vec<([f64; 3], usize)>; // (frac coords, potential index)
type Cell = [[f64; 3]; 3];

/// Atoms within rmax of the absorber at the origin for the given structure.
fn build_cluster(spec: &CrystalSpec) -> Result<Vec<Atom>, String> {
    let a = spec.a;
    if !(a > 0.5 && a < 50.0) {
        return Err(format!("implausible lattice constant a = {a}"));
    }
    let rmax = spec.rmax.clamp(2.0, 12.0);
    // (lattice vectors, basis as (frac coords, potential))
    let cubic = |basis: Basis| ([[a, 0.0, 0.0], [0.0, a, 0.0], [0.0, 0.0, a]], basis);
    let structure = spec.structure.trim().to_ascii_lowercase();
    let (cell, basis): (Cell, Basis) = match structure.as_str() {
        // conventional cubic cells keep the math obvious
        "fcc" => cubic(vec![
            ([0.0, 0.0, 0.0], 1),
            ([0.5, 0.5, 0.0], 1),
            ([0.5, 0.0, 0.5], 1),
            ([0.0, 0.5, 0.5], 1),
        ]),
        "bcc" => cubic(vec![([0.0, 0.0, 0.0], 1), ([0.5, 0.5, 0.5], 1)]),
        "cscl" => cubic(vec![([0.0, 0.0, 0.0], 1), ([0.5, 0.5, 0.5], 2)]),
        "rocksalt" => cubic(vec![
            ([0.0, 0.0, 0.0], 1),
            ([0.5, 0.5, 0.0], 1),
            ([0.5, 0.0, 0.5], 1),
            ([0.0, 0.5, 0.5], 1),
            ([0.5, 0.0, 0.0], 2),
            ([0.0, 0.5, 0.0], 2),
            ([0.0, 0.0, 0.5], 2),
            ([0.5, 0.5, 0.5], 2),
        ]),
        "zincblende" | "diamond" => {
            let p2 = if structure == "diamond" { 1 } else { 2 };
            cubic(vec![
                ([0.0, 0.0, 0.0], 1),
                ([0.5, 0.5, 0.0], 1),
                ([0.5, 0.0, 0.5], 1),
                ([0.0, 0.5, 0.5], 1),
                ([0.25, 0.25, 0.25], p2),
                ([0.75, 0.75, 0.25], p2),
                ([0.75, 0.25, 0.75], p2),
                ([0.25, 0.75, 0.75], p2),
            ])
        }
        "hcp" => {
            let c = spec.c.unwrap_or(a * (8.0f64 / 3.0).sqrt());
            (
                [
                    [a, 0.0, 0.0],
                    [-a / 2.0, a * 3.0f64.sqrt() / 2.0, 0.0],
                    [0.0, 0.0, c],
                ],
                vec![([0.0, 0.0, 0.0], 1), ([1.0 / 3.0, 2.0 / 3.0, 0.5], 1)],
            )
        }
        other => {
            return Err(format!(
                "unknown structure '{other}' (fcc, bcc, hcp, diamond, rocksalt, zincblende, cscl)"
            ));
        }
    };

    let mut atoms: Vec<Atom> = Vec::new();
    let n = (rmax / a).ceil() as i32 + 2;
    for i in -n..=n {
        for j in -n..=n {
            for k in -n..=n {
                for (frac, pot) in &basis {
                    let fx = i as f64 + frac[0];
                    let fy = j as f64 + frac[1];
                    let fz = k as f64 + frac[2];
                    let x = fx * cell[0][0] + fy * cell[1][0] + fz * cell[2][0];
                    let y = fx * cell[0][1] + fy * cell[1][1] + fz * cell[2][1];
                    let z = fx * cell[0][2] + fy * cell[1][2] + fz * cell[2][2];
                    let d = (x * x + y * y + z * z).sqrt();
                    if d <= rmax {
                        atoms.push((x, y, z, *pot));
                    }
                }
            }
        }
    }
    atoms.sort_by(|p, q| {
        (p.0 * p.0 + p.1 * p.1 + p.2 * p.2).total_cmp(&(q.0 * q.0 + q.1 * q.1 + q.2 * q.2))
    });
    if atoms.is_empty() || (atoms[0].0.abs() + atoms[0].1.abs() + atoms[0].2.abs()) > 1e-9 {
        return Err("no absorber atom at the origin".into());
    }
    Ok(atoms)
}

/// Render a feff.inp for the crystal spec (FEFF10-strict card set).
pub fn generate_inp(spec: &CrystalSpec) -> Result<String, String> {
    let el1 = spec.element.trim();
    let z1 = atomic_number(el1).ok_or_else(|| format!("unknown element '{el1}'"))?;
    let needs_el2 = matches!(
        spec.structure.trim().to_ascii_lowercase().as_str(),
        "rocksalt" | "zincblende" | "cscl"
    );
    let el2 = spec
        .element2
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let z2 = match (needs_el2, el2) {
        (true, Some(sym)) => Some((
            sym,
            atomic_number(sym).ok_or_else(|| format!("unknown element '{sym}'"))?,
        )),
        (true, None) => return Err("this structure needs a second element".into()),
        (false, _) => None,
    };
    let hole = hole_index(&spec.edge)
        .ok_or_else(|| format!("unknown edge '{}' (K, L1, L2, L3)", spec.edge))?;
    let mut atoms = build_cluster(spec)?;
    // absorber takes potential 0
    atoms[0].3 = 0;

    let mut s = format!(
        "TITLE {el1} {} (generated by xraytsubaki)\n\
         HOLE {hole}   1.0\n\n\
         CONTROL   1      1     1     1     1     1\n\
         PRINT     1      0     0     0     0     3\n\
         RMAX      {:.1}\n\
         NLEG      4\n\
         EXAFS     20\n\n\
         POTENTIALS\n\
         \x20       0   {z1}   {el1}\n\
         \x20       1   {z1}   {el1}\n",
        spec.structure.trim(),
        spec.rmax.clamp(2.0, 12.0),
    );
    if let Some((sym, z)) = z2 {
        s.push_str(&format!("        2   {z}   {sym}\n"));
    }
    s.push_str("\nATOMS\n");
    for (x, y, z, pot) in &atoms {
        let tag = match pot {
            0 => format!("{el1}0"),
            1 => format!("{el1}1"),
            _ => format!("{}2", z2.map(|(sym, _)| sym).unwrap_or(el1)),
        };
        let d = (x * x + y * y + z * z).sqrt();
        s.push_str(&format!(
            "  {x:9.5}  {y:9.5}  {z:9.5}  {pot} {tag:<12} {d:9.5}\n"
        ));
    }
    s.push_str("END\n");
    Ok(s)
}

/// Create a workspace containing a feff.inp generated from `spec`.
pub fn new_workspace_from_spec(spec: &CrystalSpec) -> Result<PathBuf, String> {
    let inp = generate_inp(spec)?;
    let dir = workspace_dir()?;
    std::fs::write(dir.join("feff.inp"), inp).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn workspace_dir() -> Result<PathBuf, String> {
    let home = std::env::var("HOME").map_err(|_| "HOME not set".to_string())?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs();
    let dir = PathBuf::from(home)
        .join(".xraytsubaki")
        .join("feff")
        .join(format!("ws-{stamp}"));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

/// Cu fcc EXAFS template (a = 3.615 Å, K edge). Card set mirrors the
/// FEFF10-verified ZnSe fixture; users edit POTENTIALS/ATOMS for their system.
fn template() -> String {
    let a = 3.615_f64;
    let half = a / 2.0;
    let rmax = 5.2_f64;
    let mut atoms: Vec<(f64, f64, f64, f64)> = Vec::new();
    for i in -3i32..=3 {
        for j in -3i32..=3 {
            for k in -3i32..=3 {
                if (i + j + k).rem_euclid(2) != 0 {
                    continue;
                }
                let (x, y, z) = (i as f64 * half, j as f64 * half, k as f64 * half);
                let d = (x * x + y * y + z * z).sqrt();
                if d < 1e-6 || d > rmax {
                    continue;
                }
                atoms.push((x, y, z, d));
            }
        }
    }
    atoms.sort_by(|p, q| p.3.total_cmp(&q.3));

    let mut s = String::new();
    s.push_str(
        "TITLE Cu fcc template - replace POTENTIALS/ATOMS with your structure\n\
         HOLE 1   1.0   * Cu K edge, second number is S0^2\n\n\
         CONTROL   1      1     1     1     1     1\n\
         PRINT     1      0     0     0     0     3\n\
         RMAX      5.0\n\
         NLEG      4\n\
         EXAFS     20\n\n\
         POTENTIALS\n\
         \x20       0   29   Cu\n\
         \x20       1   29   Cu\n\n\
         ATOMS\n",
    );
    s.push_str("    0.00000    0.00000    0.00000  0 Cu0             0.00000\n");
    for (x, y, z, d) in atoms {
        s.push_str(&format!(
            "  {x:9.5}  {y:9.5}  {z:9.5}  1 Cu1          {d:9.5}\n"
        ));
    }
    s.push_str("END\n");
    s
}

/// Create `~/.xraytsubaki/feff/ws-<stamp>/feff.inp` from the template.
pub fn new_workspace() -> Result<PathBuf, String> {
    let dir = workspace_dir()?;
    std::fs::write(dir.join("feff.inp"), template()).map_err(|e| e.to_string())?;
    Ok(dir)
}

/// Compatibility entry point used by the GUI background executor.
///
/// ReFEFF runs in-process; FEFFRS uses its embedded worker pipeline. Neither
/// route requires a separately installed command-line executable.
pub fn run_feff10_subprocess(workspace: &Path) -> Result<Vec<PathBuf>, String> {
    run_feff10(workspace)
}

/// Run the selected embedded FEFF10-compatible backend on
/// `workspace/feff.inp`; returns the generated feffNNNN.dat files. A build
/// containing both backends defaults to ReFEFF and accepts
/// `XTS_FEFF_BACKEND=feffrs` as a runtime override.
pub fn run_feff10(workspace: &Path) -> Result<Vec<PathBuf>, String> {
    let mode = selected_feff_mode()?;
    let request = FeffRunRequest {
        executable_path: PathBuf::new(),
        workspace_dir: workspace.to_path_buf(),
        feffinp: Some(workspace.join("feff.inp")),
        mode,
        timeout_sec: Some(600),
        use_sfconv: false,
        keep_all_outputs: false,
    };
    run_feff(&request)
        .map(|result| result.path_files)
        .map_err(|e| e.to_string())
}

fn selected_feff_mode() -> Result<FeffExecutionMode, String> {
    #[cfg(all(feature = "refeff-runner", feature = "feff10-runner"))]
    {
        return match std::env::var("XTS_FEFF_BACKEND")
            .unwrap_or_else(|_| "refeff".to_string())
            .to_ascii_lowercase()
            .as_str()
        {
            "refeff" => Ok(FeffExecutionMode::RefeffPipeline),
            "feffrs" | "feff10" => Ok(FeffExecutionMode::Feff10Pipeline),
            value => Err(format!(
                "unknown XTS_FEFF_BACKEND '{value}' (expected 'refeff' or 'feffrs')"
            )),
        };
    }
    #[cfg(all(feature = "refeff-runner", not(feature = "feff10-runner")))]
    {
        Ok(FeffExecutionMode::RefeffPipeline)
    }
    #[cfg(all(feature = "feff10-runner", not(feature = "refeff-runner")))]
    {
        return Ok(FeffExecutionMode::Feff10Pipeline);
    }
    #[cfg(not(any(feature = "refeff-runner", feature = "feff10-runner")))]
    {
        Err("GUI was built without a FEFF backend feature".to_string())
    }
}

/// Serialize expensive FEFF calculations in tests. The GUI itself runs one
/// calculation at a time.
#[cfg(test)]
pub(crate) static FEFF_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
pub(crate) fn feff_test_lock() -> std::sync::MutexGuard<'static, ()> {
    FEFF_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    use super::feff_test_lock as feff_lock;

    /// Generated hcp Ru input must run through FEFF10 (matches the user's
    /// Ru K-edge test data).
    #[test]
    fn generated_ru_hcp_runs_feff10() {
        let _guard = feff_lock();
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
        let paths = run_feff10(&ws).expect("feff10 run");
        assert!(!paths.is_empty());
        println!("Ru hcp: {} path files", paths.len());
    }

    /// Binary structure (NiO rocksalt) generates a parseable input.
    #[test]
    fn generated_nio_parses() {
        let spec = CrystalSpec {
            element: "Ni".into(),
            element2: Some("O".into()),
            structure: "rocksalt".into(),
            a: 4.177,
            c: None,
            edge: "K".into(),
            rmax: 4.5,
        };
        let inp = generate_inp(&spec).expect("inp");
        assert!(inp.contains("POTENTIALS"));
        assert!(inp.contains(" 2   8   O"));
    }

    /// The legacy-named background route must produce path files through
    /// embedded refeff.
    #[test]
    fn subprocess_route_runs() {
        let _guard = feff_lock();
        let ws = new_workspace().expect("workspace");
        let paths = run_feff10_subprocess(&ws).expect("subprocess run");
        assert!(!paths.is_empty());
        println!("embedded refeff route: {} path files", paths.len());
    }

    /// Template must parse and produce path files via the FEFF10 pipeline.
    #[test]
    fn template_runs_feff10() {
        let _guard = feff_lock();
        let ws = new_workspace().expect("workspace");
        let paths = run_feff10(&ws).expect("feff10 run");
        assert!(!paths.is_empty());
        println!("generated {} path files in {}", paths.len(), ws.display());
    }
}
