use std::env;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use super::errors::FittingError;
use super::path_model::feffpath;
use super::types::{
    FeffExecutionMode, FeffFlavor, FeffModuleCommand, FeffPathModel, FeffResolvedCommands,
    FeffRunRequest, FeffRunResult,
};

const FEFF85L_MODULES: [(&str, &str); 6] = [
    ("rdinp", "feff8l_rdinp"),
    ("pot", "feff8l_pot"),
    ("xsph", "feff8l_xsph"),
    ("pathfinder", "feff8l_pathfinder"),
    ("genfmt", "feff8l_genfmt"),
    ("ff2x", "feff8l_ff2x"),
];

pub fn resolve_feff_commands(
    request: &FeffRunRequest,
) -> Result<FeffResolvedCommands, FittingError> {
    let executable_path = validate_executable_path(&request.executable_path)?;
    let base_dir = executable_path
        .parent()
        .ok_or_else(|| FittingError::InvalidExecutablePath {
            path: request.executable_path.display().to_string(),
            reason: "executable has no parent directory".to_string(),
        })?;

    let mut resolved_modules = Vec::with_capacity(FEFF85L_MODULES.len());
    for &(module_label, module_bin) in required_modules(request.mode) {
        let module_file = platform_executable_name(module_bin);
        let sibling_candidate = base_dir.join(&module_file);
        if is_executable_file(&sibling_candidate) {
            resolved_modules.push(FeffModuleCommand {
                module: module_label.to_string(),
                executable: sibling_candidate,
            });
            continue;
        }

        if let Some(path_candidate) = lookup_in_path(&module_file) {
            resolved_modules.push(FeffModuleCommand {
                module: module_label.to_string(),
                executable: path_candidate,
            });
            continue;
        }

        return Err(FittingError::ExecutableNotFound {
            module: module_label.to_string(),
        });
    }

    Ok(FeffResolvedCommands {
        mode: request.mode,
        modules: resolved_modules,
    })
}

pub fn run_feff(request: &FeffRunRequest) -> Result<FeffRunResult, FittingError> {
    let workspace_dir = validate_workspace_dir(&request.workspace_dir)?;
    let feffinp_path = resolve_feffinp_path(request, &workspace_dir)?;
    let resolved = resolve_feff_commands(request)?;

    let staged_input = stage_feff_input(&workspace_dir, &feffinp_path)?;

    let run_result = (|| {
        let mut logs = Vec::with_capacity(resolved.modules.len());
        for module in &resolved.modules {
            let log_path = run_single_module(module, &workspace_dir, request.timeout_sec)?;
            logs.push(log_path);
        }

        let path_files = discover_path_files(&workspace_dir)?;
        if path_files.is_empty() {
            return Err(FittingError::NoPathOutputs {
                workspace: workspace_dir.display().to_string(),
            });
        }

        Ok(FeffRunResult {
            mode: request.mode,
            workspace_dir: workspace_dir.clone(),
            feffinp_path: feffinp_path.clone(),
            resolved: resolved.clone(),
            logs,
            path_files,
        })
    })();

    let restore_result = staged_input.restore();
    match (run_result, restore_result) {
        (Ok(result), Ok(())) => Ok(result),
        (Err(run_err), Ok(())) => Err(run_err),
        (Ok(_), Err(restore_err)) => Err(restore_err),
        (Err(run_err), Err(_restore_err)) => Err(run_err),
    }
}

pub fn run_feff_and_load_paths(
    request: &FeffRunRequest,
    flavor: FeffFlavor,
) -> Result<Vec<FeffPathModel>, FittingError> {
    let result = run_feff(request)?;
    load_paths_from_run_result(&result, flavor)
}

pub fn load_paths_from_run_result(
    result: &FeffRunResult,
    flavor: FeffFlavor,
) -> Result<Vec<FeffPathModel>, FittingError> {
    result
        .path_files
        .iter()
        .map(|path| feffpath(path, flavor))
        .collect()
}

fn required_modules(mode: FeffExecutionMode) -> &'static [(&'static str, &'static str)] {
    match mode {
        FeffExecutionMode::Feff85LModules => &FEFF85L_MODULES,
    }
}

fn validate_workspace_dir(workspace_dir: &Path) -> Result<PathBuf, FittingError> {
    if !workspace_dir.exists() || !workspace_dir.is_dir() {
        return Err(FittingError::WorkspaceNotFound {
            path: workspace_dir.display().to_string(),
        });
    }

    fs::canonicalize(workspace_dir).map_err(|error| FittingError::IOFailed {
        action: "canonicalize workspace".to_string(),
        path: workspace_dir.display().to_string(),
        reason: error.to_string(),
    })
}

fn validate_executable_path(path: &Path) -> Result<PathBuf, FittingError> {
    if path.as_os_str().is_empty() {
        return Err(FittingError::InvalidExecutablePath {
            path: path.display().to_string(),
            reason: "path is empty".to_string(),
        });
    }

    if !is_executable_file(path) {
        return Err(FittingError::InvalidExecutablePath {
            path: path.display().to_string(),
            reason: "path does not point to an executable file".to_string(),
        });
    }

    fs::canonicalize(path).map_err(|error| FittingError::InvalidExecutablePath {
        path: path.display().to_string(),
        reason: error.to_string(),
    })
}

fn resolve_feffinp_path(
    request: &FeffRunRequest,
    workspace_dir: &Path,
) -> Result<PathBuf, FittingError> {
    let feffinp = request
        .feffinp
        .clone()
        .unwrap_or_else(|| PathBuf::from("feff.inp"));
    let resolved = if feffinp.is_absolute() {
        feffinp
    } else {
        workspace_dir.join(feffinp)
    };

    if !resolved.exists() || !resolved.is_file() {
        return Err(FittingError::FeffInputNotFound {
            path: resolved.display().to_string(),
        });
    }

    Ok(resolved)
}

fn run_single_module(
    module: &FeffModuleCommand,
    workspace_dir: &Path,
    timeout_sec: Option<u64>,
) -> Result<PathBuf, FittingError> {
    let mut child = Command::new(&module.executable)
        .current_dir(workspace_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| FittingError::ProcessSpawnFailed {
            module: module.module.clone(),
            executable: module.executable.display().to_string(),
            reason: error.to_string(),
        })?;

    let timeout = timeout_sec.map(Duration::from_secs);
    let start = Instant::now();
    let status = wait_for_exit(&mut child, start, timeout, &module.module)?;

    let stdout = read_pipe(child.stdout.take(), &module.module, "stdout")?;
    let stderr = read_pipe(child.stderr.take(), &module.module, "stderr")?;

    let log_name = format!("feffrun_{}.log", module.module);
    let log_path = workspace_dir.join(log_name);
    write_module_log(&log_path, module, status, &stdout, &stderr)?;

    if !status.success() {
        return Err(FittingError::ProcessFailed {
            module: module.module.clone(),
            code: status.code().unwrap_or(-1),
        });
    }

    Ok(log_path)
}

fn wait_for_exit(
    child: &mut std::process::Child,
    start: Instant,
    timeout: Option<Duration>,
    module: &str,
) -> Result<ExitStatus, FittingError> {
    loop {
        match child
            .try_wait()
            .map_err(|error| FittingError::ProcessSpawnFailed {
                module: module.to_string(),
                executable: "<running process>".to_string(),
                reason: error.to_string(),
            })? {
            Some(status) => return Ok(status),
            None => {
                if let Some(limit) = timeout {
                    if start.elapsed() > limit {
                        let _ = child.kill();
                        let _ = child.wait();
                        return Err(FittingError::ProcessTimedOut {
                            module: module.to_string(),
                            timeout_sec: limit.as_secs(),
                        });
                    }
                }
                thread::sleep(Duration::from_millis(10));
            }
        }
    }
}

fn write_module_log(
    log_path: &Path,
    module: &FeffModuleCommand,
    status: ExitStatus,
    stdout: &[u8],
    stderr: &[u8],
) -> Result<(), FittingError> {
    let mut log = String::new();
    log.push_str(&format!(
        "# module={} executable={} status={}\n",
        module.module,
        module.executable.display(),
        status.code().unwrap_or(-1)
    ));
    log.push_str("\n# stdout\n");
    log.push_str(&String::from_utf8_lossy(stdout));
    log.push_str("\n# stderr\n");
    log.push_str(&String::from_utf8_lossy(stderr));

    fs::write(log_path, log).map_err(|error| FittingError::IOFailed {
        action: "write module log".to_string(),
        path: log_path.display().to_string(),
        reason: error.to_string(),
    })
}

fn read_pipe(
    mut pipe: Option<impl Read>,
    module: &str,
    stream_name: &str,
) -> Result<Vec<u8>, FittingError> {
    let Some(mut stream) = pipe.take() else {
        return Ok(Vec::new());
    };

    let mut buf = Vec::new();
    stream
        .read_to_end(&mut buf)
        .map_err(|error| FittingError::OutputReadFailed {
            module: module.to_string(),
            reason: format!("{stream_name}: {error}"),
        })?;
    Ok(buf)
}

fn discover_path_files(workspace_dir: &Path) -> Result<Vec<PathBuf>, FittingError> {
    let mut output_paths = Vec::new();

    let entries = fs::read_dir(workspace_dir).map_err(|error| FittingError::IOFailed {
        action: "read workspace directory".to_string(),
        path: workspace_dir.display().to_string(),
        reason: error.to_string(),
    })?;

    for entry_result in entries {
        let entry = entry_result.map_err(|error| FittingError::IOFailed {
            action: "iterate workspace entries".to_string(),
            path: workspace_dir.display().to_string(),
            reason: error.to_string(),
        })?;

        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };

        if is_feff_path_file_name(file_name) {
            output_paths.push(path);
        }
    }

    output_paths.sort_by(|lhs, rhs| {
        lhs.file_name()
            .unwrap_or_default()
            .cmp(rhs.file_name().unwrap_or_default())
    });

    Ok(output_paths)
}

fn is_feff_path_file_name(file_name: &str) -> bool {
    if !file_name.starts_with("feff") || !file_name.ends_with(".dat") {
        return false;
    }

    let digits = &file_name[4..file_name.len() - 4];
    !digits.is_empty() && digits.chars().all(|ch| ch.is_ascii_digit())
}

fn platform_executable_name(base_name: &str) -> String {
    #[cfg(windows)]
    {
        if base_name.to_ascii_lowercase().ends_with(".exe") {
            base_name.to_string()
        } else {
            format!("{base_name}.exe")
        }
    }
    #[cfg(not(windows))]
    {
        base_name.to_string()
    }
}

fn lookup_in_path(executable_name: &str) -> Option<PathBuf> {
    let path_env = env::var_os("PATH")?;
    for directory in env::split_paths(&path_env) {
        let candidate = directory.join(executable_name);
        if is_executable_file(&candidate) {
            return Some(candidate);
        }

        #[cfg(windows)]
        {
            if candidate.extension().is_some() {
                continue;
            }
            let pathext = env::var_os("PATHEXT")
                .unwrap_or_else(|| ".EXE;.BAT;.CMD;.COM".into())
                .to_string_lossy()
                .to_string();
            for ext in pathext.split(';') {
                let trimmed = ext.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let ext_clean = trimmed.trim_start_matches('.').to_ascii_lowercase();
                let candidate_with_ext = directory.join(format!("{executable_name}.{ext_clean}"));
                if is_executable_file(&candidate_with_ext) {
                    return Some(candidate_with_ext);
                }
            }
        }
    }
    None
}

fn is_executable_file(path: &Path) -> bool {
    if !path.exists() || !path.is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::metadata(path)
            .map(|meta| meta.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }

    #[cfg(not(unix))]
    {
        true
    }
}

struct StagedFeffInput {
    staged_path: Option<PathBuf>,
    backup_path: Option<PathBuf>,
    target_path: PathBuf,
}

impl StagedFeffInput {
    fn restore(self) -> Result<(), FittingError> {
        if self.staged_path.is_some() && self.target_path.exists() {
            fs::remove_file(&self.target_path).map_err(|error| FittingError::IOFailed {
                action: "remove staged feff.inp".to_string(),
                path: self.target_path.display().to_string(),
                reason: error.to_string(),
            })?;
        }

        if let Some(backup_path) = self.backup_path {
            fs::rename(&backup_path, &self.target_path).map_err(|error| {
                FittingError::IOFailed {
                    action: "restore original feff.inp".to_string(),
                    path: self.target_path.display().to_string(),
                    reason: error.to_string(),
                }
            })?;
        }

        Ok(())
    }
}

fn stage_feff_input(
    workspace_dir: &Path,
    feffinp_path: &Path,
) -> Result<StagedFeffInput, FittingError> {
    let target_path = workspace_dir.join("feff.inp");
    if feffinp_path == target_path {
        if !target_path.exists() || !target_path.is_file() {
            return Err(FittingError::FeffInputNotFound {
                path: target_path.display().to_string(),
            });
        }
        return Ok(StagedFeffInput {
            staged_path: None,
            backup_path: None,
            target_path,
        });
    }

    let backup_path = if target_path.exists() {
        let backup = workspace_dir.join(".xraytsubaki_feff_inp_backup");
        if backup.exists() {
            fs::remove_file(&backup).map_err(|error| FittingError::IOFailed {
                action: "clear existing feff.inp backup".to_string(),
                path: backup.display().to_string(),
                reason: error.to_string(),
            })?;
        }
        fs::rename(&target_path, &backup).map_err(|error| FittingError::IOFailed {
            action: "backup existing feff.inp".to_string(),
            path: target_path.display().to_string(),
            reason: error.to_string(),
        })?;
        Some(backup)
    } else {
        None
    };

    if let Err(error) = fs::copy(feffinp_path, &target_path) {
        if let Some(backup) = backup_path.as_ref() {
            let _ = fs::rename(backup, &target_path);
        }
        return Err(FittingError::IOFailed {
            action: "stage feff input file".to_string(),
            path: target_path.display().to_string(),
            reason: error.to_string(),
        });
    }

    Ok(StagedFeffInput {
        staged_path: Some(feffinp_path.to_path_buf()),
        backup_path,
        target_path,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xafs::fitting::ff2chi;
    use crate::xafs::fitting::{FeffFlavor, FitVariables};
    use nalgebra::DVector;
    use std::sync::{Mutex, OnceLock};

    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    const TOP_DIR: &str = env!("CARGO_MANIFEST_DIR");

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new(prefix: &str) -> Self {
            let unique = format!(
                "{}-{}-{}",
                prefix,
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            );
            let path = std::env::temp_dir().join(unique);
            fs::create_dir_all(&path).unwrap();
            Self { path }
        }

        fn join(&self, file_name: &str) -> PathBuf {
            self.path.join(file_name)
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[cfg(unix)]
    fn write_exec(path: &Path, body: &str) {
        use std::os::unix::fs::PermissionsExt;

        let script = format!("#!/bin/sh\nset -eu\n{body}\n");
        fs::write(path, script).unwrap();

        let mut perms = fs::metadata(path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).unwrap();
    }

    #[cfg(unix)]
    fn install_mock_modules(bin_dir: &Path, ff2x_payload: &str, fail_module: Option<&str>) {
        fs::create_dir_all(bin_dir).unwrap();

        for &(label, executable_name) in FEFF85L_MODULES.iter() {
            let executable_path = bin_dir.join(executable_name);
            let payload = if Some(label) == fail_module {
                "echo intentional failure >&2\nexit 9".to_string()
            } else if label == "ff2x" {
                format!(
                    "if [ ! -f feff.inp ]; then echo missing feff.inp >&2; exit 8; fi\n{}\necho module ff2x done",
                    ff2x_payload
                )
            } else {
                format!(
                    "if [ ! -f feff.inp ]; then echo missing feff.inp >&2; exit 8; fi\necho module {label} done"
                )
            };
            write_exec(&executable_path, &payload);
        }
    }

    fn build_request(executable_path: PathBuf, workspace_dir: PathBuf) -> FeffRunRequest {
        FeffRunRequest {
            executable_path,
            workspace_dir,
            feffinp: None,
            mode: FeffExecutionMode::Feff85LModules,
            timeout_sec: Some(5),
        }
    }

    #[test]
    fn test_platform_executable_name_has_expected_shape() {
        let executable = platform_executable_name("feff8l_rdinp");

        #[cfg(windows)]
        assert!(executable.ends_with(".exe"));

        #[cfg(not(windows))]
        assert_eq!(executable, "feff8l_rdinp");
    }

    #[test]
    fn test_is_feff_path_file_name() {
        assert!(is_feff_path_file_name("feff0001.dat"));
        assert!(is_feff_path_file_name("feff1234.dat"));
        assert!(!is_feff_path_file_name("feff.dat"));
        assert!(!is_feff_path_file_name("feffABCD.dat"));
        assert!(!is_feff_path_file_name("notfeff0001.dat"));
    }

    #[cfg(unix)]
    #[test]
    fn test_resolve_commands_from_sibling_modules() {
        let bin_dir = TestDir::new("xfeff-bin");
        install_mock_modules(&bin_dir.path, "", None);

        let request = build_request(bin_dir.join("feff8l_rdinp"), bin_dir.path.clone());
        let resolved = resolve_feff_commands(&request).unwrap();
        let canonical_bin_dir = fs::canonicalize(&bin_dir.path).unwrap();

        assert_eq!(resolved.modules.len(), FEFF85L_MODULES.len());
        assert!(resolved
            .modules
            .iter()
            .all(|module| module.executable.starts_with(&canonical_bin_dir)));
    }

    #[test]
    fn test_resolve_missing_executable_fails() {
        let workspace = TestDir::new("xfeff-work");
        let request = build_request(workspace.join("missing-feff"), workspace.path.clone());

        let err = resolve_feff_commands(&request).unwrap_err();
        assert!(matches!(err, FittingError::InvalidExecutablePath { .. }));
    }

    #[cfg(unix)]
    #[test]
    fn test_resolve_falls_back_to_path() {
        let _lock = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        let previous_path = env::var_os("PATH");

        let primary_dir = TestDir::new("xfeff-primary");
        let fallback_dir = TestDir::new("xfeff-fallback");

        fs::create_dir_all(&primary_dir.path).unwrap();
        write_exec(
            &primary_dir.join("feff8l_rdinp"),
            "if [ ! -f feff.inp ]; then :; fi\necho only primary",
        );
        install_mock_modules(&fallback_dir.path, "", None);

        env::set_var("PATH", fallback_dir.path.display().to_string());
        let request = build_request(primary_dir.join("feff8l_rdinp"), primary_dir.path.clone());
        let resolved = resolve_feff_commands(&request).unwrap();

        if let Some(path) = previous_path {
            env::set_var("PATH", path);
        } else {
            env::remove_var("PATH");
        }

        assert_eq!(resolved.modules.len(), FEFF85L_MODULES.len());
        assert_eq!(resolved.modules[0].module, "rdinp");
        assert!(resolved
            .modules
            .iter()
            .skip(1)
            .all(|module| module.executable.starts_with(&fallback_dir.path)));
    }

    #[cfg(unix)]
    #[test]
    fn test_resolve_errors_for_missing_module() {
        let _lock = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        let previous_path = env::var_os("PATH");

        let primary_dir = TestDir::new("xfeff-primary-missing");
        fs::create_dir_all(&primary_dir.path).unwrap();
        write_exec(
            &primary_dir.join("feff8l_rdinp"),
            "if [ ! -f feff.inp ]; then :; fi\necho only rdinp",
        );

        env::set_var("PATH", "");

        let request = build_request(primary_dir.join("feff8l_rdinp"), primary_dir.path.clone());
        let err = resolve_feff_commands(&request).unwrap_err();

        if let Some(path) = previous_path {
            env::set_var("PATH", path);
        } else {
            env::remove_var("PATH");
        }

        assert!(matches!(err, FittingError::ExecutableNotFound { .. }));
    }

    #[cfg(unix)]
    #[test]
    fn test_runner_success_discovers_sorted_outputs() {
        let workspace = TestDir::new("xfeff-work-success");
        fs::write(workspace.join("feff.inp"), "TITLE\n").unwrap();

        let bin_dir = TestDir::new("xfeff-bin-success");
        install_mock_modules(
            &bin_dir.path,
            "echo 'dummy' > feff0010.dat\necho 'dummy' > feff0002.dat",
            None,
        );

        let request = build_request(bin_dir.join("feff8l_rdinp"), workspace.path.clone());
        let result = run_feff(&request).unwrap();

        assert_eq!(result.logs.len(), FEFF85L_MODULES.len());
        assert_eq!(result.path_files.len(), 2);

        let names = result
            .path_files
            .iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().to_string())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["feff0002.dat", "feff0010.dat"]);
    }

    #[cfg(unix)]
    #[test]
    fn test_runner_missing_feffinp_fails() {
        let workspace = TestDir::new("xfeff-work-missing-inp");
        let bin_dir = TestDir::new("xfeff-bin-missing-inp");
        install_mock_modules(&bin_dir.path, "", None);

        let request = build_request(bin_dir.join("feff8l_rdinp"), workspace.path.clone());
        let err = run_feff(&request).unwrap_err();

        assert!(matches!(err, FittingError::FeffInputNotFound { .. }));
    }

    #[cfg(unix)]
    #[test]
    fn test_runner_nonzero_exit_fails_fast() {
        let workspace = TestDir::new("xfeff-work-fail");
        fs::write(workspace.join("feff.inp"), "TITLE\n").unwrap();

        let bin_dir = TestDir::new("xfeff-bin-fail");
        install_mock_modules(&bin_dir.path, "", Some("pot"));

        let request = build_request(bin_dir.join("feff8l_rdinp"), workspace.path.clone());
        let err = run_feff(&request).unwrap_err();

        assert!(matches!(
            err,
            FittingError::ProcessFailed {
                module,
                code: 9
            } if module == "pot"
        ));
    }

    #[cfg(unix)]
    #[test]
    fn test_runner_empty_output_set_errors() {
        let workspace = TestDir::new("xfeff-work-empty");
        fs::write(workspace.join("feff.inp"), "TITLE\n").unwrap();

        let bin_dir = TestDir::new("xfeff-bin-empty");
        install_mock_modules(&bin_dir.path, "", None);

        let request = build_request(bin_dir.join("feff8l_rdinp"), workspace.path.clone());
        let err = run_feff(&request).unwrap_err();

        assert!(matches!(err, FittingError::NoPathOutputs { .. }));
    }

    #[cfg(unix)]
    #[test]
    fn test_run_and_load_paths_interoperates_with_modeling() {
        let workspace = TestDir::new("xfeff-work-load");
        fs::write(workspace.join("feff.inp"), "TITLE\n").unwrap();

        let fixture1 = format!("{TOP_DIR}/tests/testfiles/feffcu01.dat");
        let fixture2 = format!("{TOP_DIR}/tests/testfiles/feff0002.dat");

        let bin_dir = TestDir::new("xfeff-bin-load");
        let ff2x_payload = format!(
            "cat '{}' > feff0010.dat\ncat '{}' > feff0002.dat",
            fixture1, fixture2
        );
        install_mock_modules(&bin_dir.path, &ff2x_payload, None);

        let request = build_request(bin_dir.join("feff8l_rdinp"), workspace.path.clone());
        let paths = run_feff_and_load_paths(&request, FeffFlavor::Feff85L).unwrap();

        assert_eq!(paths.len(), 2);

        let k = DVector::from_iterator(120, (0..120).map(|i| 0.05 * (i as f64 + 1.0)));
        let modeled = ff2chi(&paths, &FitVariables::new(), &k).unwrap();

        assert_eq!(modeled.chi.len(), k.len());
        assert_eq!(modeled.path_chi.len(), 2);
    }
}
