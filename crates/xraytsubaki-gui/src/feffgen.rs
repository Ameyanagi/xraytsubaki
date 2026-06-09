//! FEFF10 helper: create a feff.inp workspace from a template and run the
//! in-crate FEFF10 pipeline (core `feff10-runner` feature) to generate
//! feffNNNN.dat path files for fitting.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use xraytsubaki::prelude::*;

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
    std::fs::write(dir.join("feff.inp"), template()).map_err(|e| e.to_string())?;
    Ok(dir)
}

/// Worker-process flag: `xraytsubaki-gui --feff10-worker <workspace>`.
///
/// feff10 0.2 executes each Fortran stage in a `fork()`ed child without
/// `exec`; inside a Cocoa/Metal GUI process that child dies with SIGILL.
/// The GUI therefore re-invokes its own binary (pre-GUI, fork-safe) to run
/// the pipeline, and parses the generated path list from stdout.
pub const FEFF10_WORKER_FLAG: &str = "--feff10-worker";

/// Entry point for the worker process; prints one path file per line.
pub fn worker_main(workspace: &Path) -> i32 {
    match run_feff10(workspace) {
        Ok(paths) => {
            for p in paths {
                println!("{}", p.display());
            }
            0
        }
        Err(e) => {
            eprintln!("{e}");
            1
        }
    }
}

/// Run FEFF10 in a helper subprocess. Blocking — call on the background
/// executor.
pub fn run_feff10_subprocess(workspace: &Path) -> Result<Vec<PathBuf>, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let output = std::process::Command::new(exe)
        .arg(FEFF10_WORKER_FLAG)
        .arg(workspace)
        .output()
        .map_err(|e| format!("failed to launch FEFF10 worker: {e}"))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        let err = err.trim();
        return Err(if err.is_empty() {
            format!("FEFF10 worker exited with {}", output.status)
        } else {
            err.to_string()
        });
    }
    let paths: Vec<PathBuf> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|l| PathBuf::from(l.trim()))
        .filter(|p| p.exists())
        .collect();
    if paths.is_empty() {
        return Err("FEFF10 produced no path files".into());
    }
    Ok(paths)
}

/// Run the FEFF10 pipeline on `workspace/feff.inp`; returns generated
/// feffNNNN.dat files. Only safe in a non-GUI process (see
/// [`FEFF10_WORKER_FLAG`]).
pub fn run_feff10(workspace: &Path) -> Result<Vec<PathBuf>, String> {
    let request = FeffRunRequest {
        executable_path: PathBuf::new(),
        workspace_dir: workspace.to_path_buf(),
        feffinp: Some(workspace.join("feff.inp")),
        mode: FeffExecutionMode::Feff10Pipeline,
        timeout_sec: Some(600),
        use_sfconv: false,
    };
    run_feff(&request)
        .map(|result| result.path_files)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Template must parse and produce path files via the FEFF10 pipeline.
    #[test]
    fn template_runs_feff10() {
        let ws = new_workspace().expect("workspace");
        let paths = run_feff10(&ws).expect("feff10 run");
        assert!(!paths.is_empty());
        println!("generated {} path files in {}", paths.len(), ws.display());
    }
}
