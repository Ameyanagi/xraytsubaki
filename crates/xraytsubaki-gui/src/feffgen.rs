//! FEFF10 helper: create a feff.inp workspace and run the selected embedded
//! FEFFRS or pure-Rust ReFEFF backend to generate fitting path files.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use xraytsubaki::prelude::*;

/// Atoms-lite: build a feff.inp from a simple crystal description
/// (element(s), common structure type, lattice constants, edge, cluster
/// radius) — covers the common metal/oxide cases without full space-group
/// machinery; arbitrary structures can still be pasted into feff.inp.
#[derive(Clone, Debug, PartialEq)]
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

/// `feff.inp` text for a simple crystal, generated through the core
/// structure module (`builtin_structure` → `build_cluster` → `write_feff_inp`).
#[cfg_attr(not(test), allow(dead_code))]
pub fn generate_inp(spec: &CrystalSpec) -> Result<String, String> {
    use xraytsubaki::xafs::structure::{
        AbsorberSelection, ClusterOptions, Edge, FeffInputOptions, build_cluster, write_feff_inp,
    };
    let structure = crate::structure::builtin_structure(spec)?;
    let el1 = spec.element.trim();
    let edge = Edge::parse(&spec.edge)
        .ok_or_else(|| format!("unknown edge '{}' (K, L1, L2, L3)", spec.edge))?;
    let radius = spec.rmax.clamp(2.0, 12.0);
    let cluster = build_cluster(
        &structure,
        &AbsorberSelection::Element(el1.to_string()),
        &ClusterOptions {
            radius,
            ..Default::default()
        },
    )
    .map_err(|e| e.to_string())?;
    let opts = FeffInputOptions {
        edge,
        rmax: Some(radius),
        rpath: Some(radius),
        ..Default::default()
    };
    Ok(write_feff_inp(&cluster, &opts))
}

/// Create a workspace containing a feff.inp generated from `spec`.
#[cfg_attr(not(test), allow(dead_code))]
pub fn new_workspace_from_spec(spec: &CrystalSpec) -> Result<PathBuf, String> {
    new_workspace_with(&generate_inp(spec)?)
}

/// Create a workspace containing the given feff.inp text.
pub fn new_workspace_with(inp: &str) -> Result<PathBuf, String> {
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
        assert!(inp.contains("  8   O"), "{inp}");
        assert!(inp.contains("ATOMS"));
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
