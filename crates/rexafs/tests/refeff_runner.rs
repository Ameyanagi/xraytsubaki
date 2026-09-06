#![cfg(feature = "refeff-runner")]

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rexafs::prelude::{feffpath, run_feff, FeffExecutionMode, FeffFlavor, FeffRunRequest};

struct TestWorkspace(PathBuf);

impl TestWorkspace {
    fn new() -> Self {
        let unique = format!(
            "rexafs-refeff-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        );
        let path = std::env::temp_dir().join(unique);
        fs::create_dir_all(&path).expect("create test workspace");
        Self(path)
    }
}

impl Drop for TestWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn refeff_in_memory_generates_only_loadable_path_files_by_default() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let input = crate_dir.join("tests/testfiles/xraylarch_d867/feffit/Feff_ZnSe/feff.inp");
    let workspace = TestWorkspace::new();
    let request = FeffRunRequest {
        executable_path: PathBuf::new(),
        workspace_dir: workspace.0.clone(),
        feffinp: Some(input),
        mode: FeffExecutionMode::RefeffPipeline,
        timeout_sec: None,
        use_sfconv: false,
        keep_all_outputs: false,
    };

    let result = run_feff(&request).expect("ReFEFF calculation");

    assert_eq!(result.mode, FeffExecutionMode::RefeffPipeline);
    assert!(!result.path_files.is_empty());
    assert!(result.logs.is_empty());
    assert!(result
        .resolved
        .modules
        .iter()
        .all(|module| { module.executable.to_string_lossy().starts_with("refeff::") }));
    assert!(result.path_files.iter().all(|path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("feff") && name.ends_with(".dat"))
    }));

    let workspace_files: Vec<_> = fs::read_dir(&workspace.0)
        .expect("read output workspace")
        .map(|entry| entry.expect("workspace entry").path())
        .collect();
    assert_eq!(workspace_files.len(), result.path_files.len());

    feffpath(
        result.path_files[0].to_string_lossy().as_ref(),
        FeffFlavor::Feff85L,
    )
    .expect("ReFEFF path output must load in the existing fitting parser");
}

#[test]
fn refeff_verbose_output_mode_materializes_full_feff_artifacts() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let input = crate_dir.join("tests/testfiles/xraylarch_d867/feffit/Feff_ZnSe/feff.inp");
    let workspace = TestWorkspace::new();
    let request = FeffRunRequest {
        executable_path: PathBuf::new(),
        workspace_dir: workspace.0.clone(),
        feffinp: Some(input),
        mode: FeffExecutionMode::RefeffPipeline,
        timeout_sec: None,
        use_sfconv: false,
        keep_all_outputs: true,
    };

    let result = run_feff(&request).expect("verbose ReFEFF calculation");

    assert!(!result.path_files.is_empty());
    assert!(workspace.0.join("pot.bin").is_file());
    assert!(workspace.0.join("phase.bin").is_file());
    assert!(workspace.0.join("chi.dat").is_file());
    assert!(!workspace.0.join("feff.inp").exists());
    assert!(
        fs::read_dir(&workspace.0)
            .expect("read verbose output workspace")
            .count()
            > result.path_files.len()
    );
}
