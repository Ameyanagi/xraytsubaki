use std::path::PathBuf;

use tauri::State;
use uuid::Uuid;
use xraytsubaki::prelude::{
    feffpath, run_feff, FeffExecutionMode, FeffFit, FeffFitDataset, FeffFitResult, FeffFlavor,
    FeffRunRequest, PathParamSpec,
};

use crate::dto::{
    FeffFitConfig, FeffFitResultDto, FeffPathConfig, FeffResolvedModuleDto, FeffRunConfig,
    FeffRunResultDto, FitVariableResult, PathContributionDto, PlotResult, PlotTrace,
};
use crate::state::AppState;

fn dvec_to_vec(v: &nalgebra::DVector<f64>) -> Vec<f64> {
    v.iter().copied().collect()
}

fn path_param(spec: &str, fallback: f64) -> PathParamSpec {
    let trimmed = spec.trim();
    if trimmed.is_empty() {
        return PathParamSpec::Value(fallback);
    }
    if let Ok(value) = trimmed.parse::<f64>() {
        return PathParamSpec::Value(value);
    }
    PathParamSpec::Expression(trimmed.to_string())
}

fn resolve_feff_run_mode(
    explicit_executable: Option<PathBuf>,
    use_sfconv: bool,
) -> Result<(FeffExecutionMode, PathBuf), String> {
    if explicit_executable.is_some() && use_sfconv {
        return Err(
            "SFCONV is only available in FEFF10 pipeline mode. Clear the explicit executable path or disable SFCONV."
                .to_string(),
        );
    }

    Ok(match explicit_executable {
        Some(path) => (FeffExecutionMode::Feff85LModules, path),
        None => (FeffExecutionMode::Feff10Pipeline, PathBuf::new()),
    })
}

fn feff_path_from_config(
    config: &FeffPathConfig,
) -> Result<xraytsubaki::prelude::FeffPathModel, String> {
    let mut path =
        feffpath(&config.feff_dat_path, FeffFlavor::Feff85L).map_err(|e| e.to_string())?;
    if !config.label.trim().is_empty() {
        path = path.set_label(config.label.trim().to_string());
    }
    path = path
        .set_use_path(config.use_path)
        .set_s02(path_param(&config.s02, 1.0))
        .set_e0(path_param(&config.e0, 0.0))
        .set_deltar(path_param(&config.deltar, 0.0))
        .set_sigma2(path_param(&config.sigma2, 0.0));
    Ok(path)
}

fn fit_result_to_dto(id: String, result: &FeffFitResult) -> FeffFitResultDto {
    let mut normalized = result.clone();
    normalized.sync_primary_dataset_fields();

    let variables = normalized
        .variables
        .vars
        .iter()
        .map(|(name, value)| FitVariableResult {
            name: name.clone(),
            value: value.value,
            stderr: value.stderr,
            vary: value.vary,
            init_value: value.init_value,
        })
        .collect();

    let path_contributions = normalized
        .path_contributions
        .iter()
        .map(|path| PathContributionDto {
            label: path.label.clone(),
            chi: dvec_to_vec(&path.chi),
            chir_re: dvec_to_vec(&path.chir_re),
            chir_im: dvec_to_vec(&path.chir_im),
            chir_mag: dvec_to_vec(&path.chir_mag),
        })
        .collect();

    FeffFitResultDto {
        id,
        chi_square: normalized.chi_square,
        reduced_chi_square: normalized.reduced_chi_square,
        r_factor: normalized.r_factor,
        n_vary: normalized.n_vary,
        n_data: normalized.n_data,
        n_idp: normalized.n_idp,
        variables,
        correlation: normalized.correlation.clone(),
        k: dvec_to_vec(&normalized.k),
        data_chi: dvec_to_vec(&normalized.data_chi),
        model_chi: dvec_to_vec(&normalized.model_chi),
        r: dvec_to_vec(&normalized.r),
        data_chir_re: dvec_to_vec(&normalized.data_chir_re),
        data_chir_im: dvec_to_vec(&normalized.data_chir_im),
        model_chir_re: dvec_to_vec(&normalized.model_chir_re),
        model_chir_im: dvec_to_vec(&normalized.model_chir_im),
        model_chir_mag: dvec_to_vec(&normalized.model_chir_mag),
        path_contributions,
        warnings: normalized
            .warnings
            .iter()
            .map(|w| {
                if w.symbol.is_empty() {
                    w.message.clone()
                } else {
                    format!("{}: {}", w.symbol, w.message)
                }
            })
            .collect(),
    }
}

fn magnitude(re: &[f64], im: &[f64]) -> Vec<f64> {
    re.iter()
        .zip(im.iter())
        .map(|(rv, iv)| (rv * rv + iv * iv).sqrt())
        .collect()
}

fn main_trace(x: Vec<f64>, y: Vec<f64>, label: impl Into<String>, panel: &str) -> PlotTrace {
    PlotTrace {
        x,
        y,
        label: label.into(),
        panel: panel.to_string(),
        overlay: None,
        dash: None,
        color: None,
    }
}

fn overlay_trace(
    x: Vec<f64>,
    y: Vec<f64>,
    label: impl Into<String>,
    panel: &str,
    overlay: &str,
    color: &str,
) -> PlotTrace {
    PlotTrace {
        x,
        y,
        label: label.into(),
        panel: panel.to_string(),
        overlay: Some(overlay.to_string()),
        dash: Some("dot".to_string()),
        color: Some(color.to_string()),
    }
}

#[tauri::command]
pub fn run_feff_paths(config: FeffRunConfig) -> Result<FeffRunResultDto, String> {
    let explicit_executable = config
        .executable_path
        .as_deref()
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(PathBuf::from);

    let workspace_dir = config.workspace_dir.trim();
    if workspace_dir.is_empty() {
        return Err("Workspace directory is required".to_string());
    }

    let (mode, executable_path) = resolve_feff_run_mode(explicit_executable, config.use_sfconv)?;

    let request = FeffRunRequest {
        executable_path,
        workspace_dir: PathBuf::from(workspace_dir),
        feffinp: config
            .feffinp
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from),
        mode,
        timeout_sec: config.timeout_sec,
        use_sfconv: config.use_sfconv,
    };

    let run_result = run_feff(&request).map_err(|e| e.to_string())?;

    Ok(FeffRunResultDto {
        mode: format!("{:?}", run_result.mode),
        workspace_dir: run_result.workspace_dir.to_string_lossy().to_string(),
        feffinp_path: run_result.feffinp_path.to_string_lossy().to_string(),
        modules: run_result
            .resolved
            .modules
            .iter()
            .map(|module| FeffResolvedModuleDto {
                module: module.module.clone(),
                executable: module.executable.to_string_lossy().to_string(),
            })
            .collect(),
        logs: run_result
            .logs
            .iter()
            .map(|path| path.to_string_lossy().to_string())
            .collect(),
        path_files: run_result
            .path_files
            .iter()
            .map(|path| path.to_string_lossy().to_string())
            .collect(),
    })
}

#[tauri::command]
pub fn run_feff_fit(
    state: State<'_, AppState>,
    config: FeffFitConfig,
) -> Result<FeffFitResultDto, String> {
    if config.transform.kmin >= config.transform.kmax {
        return Err("Invalid transform: kmin must be less than kmax".to_string());
    }
    if config.transform.rmin >= config.transform.rmax {
        return Err("Invalid transform: rmin must be less than rmax".to_string());
    }

    let (k, chi) = {
        let group = state.group.lock().map_err(|e| e.to_string())?;
        let spectrum = group
            .get_spectrum(config.data_index)
            .map_err(|e| format!("Failed to read spectrum #{}: {}", config.data_index, e))?;

        let k = spectrum.k.as_ref().cloned().ok_or_else(|| {
            "Spectrum does not have k-space data. Run processing first.".to_string()
        })?;
        let chi = spectrum
            .chi
            .as_ref()
            .cloned()
            .or_else(|| spectrum.chi_kweighted.as_ref().cloned())
            .ok_or_else(|| {
                "Spectrum does not have chi(k) data. Run processing first.".to_string()
            })?;
        (k, chi)
    };

    if k.len() != chi.len() {
        return Err(format!(
            "Spectrum k/chi length mismatch (k={}, chi={})",
            k.len(),
            chi.len()
        ));
    }

    let mut dataset = FeffFitDataset::new()
        .data(&k, &chi)
        .krange(config.transform.kmin, config.transform.kmax)
        .rrange(config.transform.rmin, config.transform.rmax)
        .kweight(config.transform.kweight)
        .dk(config.transform.dk);

    let enabled_paths: Vec<_> = config.paths.iter().filter(|path| path.use_path).collect();
    if enabled_paths.is_empty() {
        return Err("No active FEFF paths. Add or enable at least one path.".to_string());
    }

    for path_config in enabled_paths {
        let path = feff_path_from_config(path_config)?;
        dataset = dataset.add_path(path);
    }

    let mut fit = FeffFit::new().add_dataset(dataset);

    for variable in &config.variables {
        let name = variable.name.trim();
        if name.is_empty() {
            continue;
        }

        if let Some(expr) = variable
            .expr
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            fit = fit.var_expr(name, expr);
        } else if variable.vary {
            fit = fit.set_init(name, variable.value);
        } else {
            fit = fit.fix(name, variable.value);
        }

        if let (Some(min), Some(max)) = (variable.min, variable.max) {
            if min <= max {
                fit = fit.set_bounds(name, min, max);
            }
        }
    }

    let fit_result = fit.fit().map_err(|e| e.to_string())?;
    let fit_id = format!("fit-{}", Uuid::new_v4());
    let dto = fit_result_to_dto(fit_id.clone(), &fit_result);

    let mut fits = state.fit_results.lock().map_err(|e| e.to_string())?;
    fits.insert(fit_id, dto.clone());

    Ok(dto)
}

#[tauri::command]
pub fn get_fit_result(
    state: State<'_, AppState>,
    fit_id: String,
) -> Result<FeffFitResultDto, String> {
    let fits = state.fit_results.lock().map_err(|e| e.to_string())?;
    fits.get(&fit_id)
        .cloned()
        .ok_or_else(|| format!("Fit result '{}' not found", fit_id))
}

#[tauri::command]
pub fn list_fit_results(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let fits = state.fit_results.lock().map_err(|e| e.to_string())?;
    let mut ids: Vec<_> = fits.keys().cloned().collect();
    ids.sort();
    Ok(ids)
}

#[tauri::command]
pub fn plot_fit(
    state: State<'_, AppState>,
    fit_id: String,
    panel: String,
    include_paths: Option<bool>,
) -> Result<PlotResult, String> {
    let include_paths = include_paths.unwrap_or(true);
    let fit = {
        let fits = state.fit_results.lock().map_err(|e| e.to_string())?;
        fits.get(&fit_id)
            .cloned()
            .ok_or_else(|| format!("Fit result '{}' not found", fit_id))?
    };

    match panel.as_str() {
        "k" => {
            let mut traces = vec![
                main_trace(fit.k.clone(), fit.data_chi.clone(), "Data χ(k)", "k"),
                main_trace(fit.k.clone(), fit.model_chi.clone(), "Model χ(k)", "k"),
            ];
            if include_paths {
                for path in &fit.path_contributions {
                    traces.push(overlay_trace(
                        fit.k.clone(),
                        path.chi.clone(),
                        format!("Path {}", path.label),
                        "k",
                        "path",
                        "#22c55e",
                    ));
                }
            }

            Ok(PlotResult {
                traces,
                pngs: Vec::new(),
                svgs: Vec::new(),
                x_label: "k (Å⁻¹)".to_string(),
                y_label: "χ(k)".to_string(),
            })
        }
        "r" => {
            let mut traces = vec![
                main_trace(
                    fit.r.clone(),
                    magnitude(&fit.data_chir_re, &fit.data_chir_im),
                    "Data |χ(R)|",
                    "r",
                ),
                main_trace(
                    fit.r.clone(),
                    fit.model_chir_mag.clone(),
                    "Model |χ(R)|",
                    "r",
                ),
            ];
            if include_paths {
                for path in &fit.path_contributions {
                    traces.push(overlay_trace(
                        fit.r.clone(),
                        path.chir_mag.clone(),
                        format!("Path {}", path.label),
                        "r",
                        "path",
                        "#f59e0b",
                    ));
                }
            }

            Ok(PlotResult {
                traces,
                pngs: Vec::new(),
                svgs: Vec::new(),
                x_label: "R (Å)".to_string(),
                y_label: "|χ(R)|".to_string(),
            })
        }
        other => Err(format!(
            "Unsupported fit plot panel '{}'. Expected 'k' or 'r'.",
            other
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_feff_run_mode;
    use std::path::PathBuf;
    use xraytsubaki::prelude::FeffExecutionMode;

    #[test]
    fn resolve_feff_run_mode_uses_feff10_without_explicit_executable() {
        let (mode, executable) = resolve_feff_run_mode(None, true).expect("should resolve");
        assert_eq!(mode, FeffExecutionMode::Feff10Pipeline);
        assert!(executable.as_os_str().is_empty());
    }

    #[test]
    fn resolve_feff_run_mode_uses_feff85l_with_explicit_executable() {
        let executable = PathBuf::from("/tmp/feff85l");
        let (mode, resolved) =
            resolve_feff_run_mode(Some(executable.clone()), false).expect("should resolve");
        assert_eq!(mode, FeffExecutionMode::Feff85LModules);
        assert_eq!(resolved, executable);
    }

    #[test]
    fn resolve_feff_run_mode_rejects_sfconv_with_explicit_executable() {
        let error = resolve_feff_run_mode(Some(PathBuf::from("/tmp/feff85l")), true).unwrap_err();
        assert!(
            error.contains("SFCONV is only available in FEFF10 pipeline mode"),
            "unexpected error: {error}"
        );
    }
}
