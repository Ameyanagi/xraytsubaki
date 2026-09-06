#[cfg(not(feature = "plotting"))]
fn main() {
    eprintln!(
        "Enable plotting support: cargo run -p rexafs --features plotting --example plot_demo"
    );
}

#[cfg(feature = "plotting")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::env;
    use std::fs;
    use std::path::{Path, PathBuf};

    use nalgebra::DVector;
    use rexafs::prelude::*;
    use rexafs::xafs::io::load_spectrum_QAS_trans;
    use rexafs::xafs::{FittingError, XAFSError};

    fn crate_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    fn output_dir() -> PathBuf {
        crate_dir().join("target").join("plot_demo")
    }

    fn ensure_dir(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        fs::create_dir_all(path)?;
        Ok(())
    }

    fn path_to_string(path: &Path) -> String {
        path.to_string_lossy().to_string()
    }

    fn load_columns(
        path: &Path,
        ncols: usize,
    ) -> Result<Vec<DVector<f64>>, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path)?;
        let mut cols = vec![Vec::<f64>::new(); ncols];

        for (line_no, line) in content.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let values = line
                .split_whitespace()
                .map(str::parse::<f64>)
                .collect::<Result<Vec<_>, _>>()?;
            if values.len() < ncols {
                return Err(format!(
                    "{path:?}: line {} has {} columns, expected at least {ncols}",
                    line_no + 1,
                    values.len()
                )
                .into());
            }
            for idx in 0..ncols {
                cols[idx].push(values[idx]);
            }
        }

        if cols.iter().any(Vec::is_empty) {
            return Err(format!("no data rows found in {path:?}").into());
        }

        Ok(cols.into_iter().map(DVector::from_vec).collect())
    }

    fn rmse(lhs: &DVector<f64>, rhs: &DVector<f64>) -> Option<f64> {
        let len = lhs.len().min(rhs.len());
        if len == 0 {
            return None;
        }
        let mse = lhs
            .iter()
            .take(len)
            .zip(rhs.iter().take(len))
            .map(|(a, b)| {
                let d = a - b;
                d * d
            })
            .sum::<f64>()
            / len as f64;
        Some(mse.sqrt())
    }

    fn discover_feff85l_rdinp() -> Option<PathBuf> {
        let env_candidates = ["REXAFS_FEFF8L_RDINP", "FEFF8L_RDINP", "FEFF85L_RDINP"];
        for key in env_candidates {
            if let Ok(value) = env::var(key) {
                let path = PathBuf::from(value);
                if path.is_file() {
                    return Some(path);
                }
            }
        }

        let crate_dir = crate_dir();
        let rel_candidates = [
            "tests/pythonscript/.venv/lib/python3.14/site-packages/larch/bin/darwin64/feff8l_rdinp",
            "tests/pythonscript/.venv/lib/python3.14/site-packages/larch/bin/linux64/feff8l_rdinp",
        ];
        for rel in rel_candidates {
            let candidate = crate_dir.join(rel);
            if candidate.is_file() {
                return Some(candidate);
            }
        }

        let path_env = env::var_os("PATH")?;
        for dir in env::split_paths(&path_env) {
            let candidate = dir.join("feff8l_rdinp");
            if candidate.is_file() {
                return Some(candidate);
            }
        }

        None
    }

    fn run_feff_material(
        feff8l_rdinp: &Path,
        material_name: &str,
        feffinp_path: &Path,
        output_root: &Path,
    ) -> Result<usize, Box<dyn std::error::Error>> {
        let slug = material_name.to_ascii_lowercase().replace([' ', '/'], "_");
        let workspace_dir = output_root.join(format!("feff_calc_{slug}"));
        if workspace_dir.exists() {
            fs::remove_dir_all(&workspace_dir)?;
        }
        fs::create_dir_all(&workspace_dir)?;

        let request = FeffRunRequest {
            executable_path: feff8l_rdinp.to_path_buf(),
            workspace_dir: workspace_dir.clone(),
            feffinp: Some(feffinp_path.to_path_buf()),
            mode: FeffExecutionMode::Feff85LModules,
            timeout_sec: Some(180),
            use_sfconv: false,
            keep_all_outputs: false,
        };

        let run_result = match run_feff(&request) {
            Ok(result) => Some(result),
            Err(XAFSError::Fitting(FittingError::NoPathOutputs { .. })) => None,
            Err(error) => {
                return Err(format!("FEFF run failed for {material_name}: {error}").into());
            }
        };
        let mut summary = String::new();
        summary.push_str(&format!("material={material_name}\n"));
        summary.push_str(&format!("feffinp={}\n", feffinp_path.display()));

        if let Some(run_result) = run_result {
            summary.push_str("modules:\n");
            for module in &run_result.resolved.modules {
                summary.push_str(&format!(
                    "- {} => {}\n",
                    module.module,
                    module.executable.display()
                ));
            }
            summary.push_str("logs:\n");
            for log in &run_result.logs {
                summary.push_str(&format!("- {}\n", log.display()));
            }

            let loaded_paths = run_result
                .path_files
                .iter()
                .map(|path| feffpath(path_to_string(path).as_str(), FeffFlavor::Feff85L))
                .collect::<Result<Vec<_>, _>>()?;

            summary.push_str("path_files:\n");
            for path in &run_result.path_files {
                summary.push_str(&format!("- {}\n", path.display()));
            }
            summary.push_str(&format!("loaded_paths={}\n", loaded_paths.len()));
            fs::write(workspace_dir.join("summary.txt"), summary)?;
            return Ok(loaded_paths.len());
        }

        summary.push_str(
            "note=no feffNNNN.dat outputs discovered for this input; FEFF modules still completed\n",
        );
        let mut log_files = fs::read_dir(&workspace_dir)?
            .filter_map(|entry| entry.ok().map(|v| v.path()))
            .filter(|path| {
                path.file_name()
                    .and_then(|v| v.to_str())
                    .is_some_and(|name| name.starts_with("feffrun_") && name.ends_with(".log"))
            })
            .collect::<Vec<_>>();
        log_files.sort();
        summary.push_str("logs:\n");
        for path in log_files {
            summary.push_str(&format!("- {}\n", path.display()));
        }
        summary.push_str("loaded_paths=0\n");
        fs::write(workspace_dir.join("summary.txt"), summary)?;
        Ok(0)
    }

    fn build_cu_fit(
        fixture_root: &Path,
        refs_root: &Path,
    ) -> Result<(FeffFitResult, Option<f64>, Option<f64>), Box<dyn std::error::Error>> {
        let k_cols = load_columns(&refs_root.join("cu_fit_kspace.txt"), 4)?;
        let r_cols = load_columns(&refs_root.join("cu_fit_rspace.txt"), 7)?;
        let k = k_cols[0].clone();
        let data_chi = k_cols[1].clone();
        let larch_model_k = k_cols[2].clone();
        let larch_model_r_mag = r_cols[6].clone();

        let path = feffpath(
            path_to_string(&fixture_root.join("feffit/Feff_Cu/feff0001.dat")).as_str(),
            FeffFlavor::Feff85L,
        )?
        .set_s02("amp")
        .set_e0("de0")
        .set_sigma2("sig2")
        .set_deltar("dr");

        let fit = FeffFit::new()
            .data(&k, &data_chi)
            .add_path(path)
            .set_inits([("amp", 1.0), ("de0", 0.0), ("sig2", 0.003), ("dr", 0.0)])
            .set_bounds("sig2", 0.0, 0.02)
            .kweight(2.0)
            .window(FTWindow::KaiserBessel)
            .dk(5.0)
            .krange(3.0, 16.0)
            .rrange(1.4, 3.0)
            .fit()?;

        let k_rmse = rmse(&fit.model_chi, &larch_model_k);
        let r_rmse = rmse(&fit.model_chir_mag, &larch_model_r_mag);
        Ok((fit, k_rmse, r_rmse))
    }

    fn build_znse_fit(
        fixture_root: &Path,
        refs_root: &Path,
    ) -> Result<(FeffFitResult, Option<f64>, Option<f64>), Box<dyn std::error::Error>> {
        let k_cols = load_columns(&refs_root.join("znse_fit_kspace.txt"), 4)?;
        let r_cols = load_columns(&refs_root.join("znse_fit_rspace.txt"), 7)?;
        let k = k_cols[0].clone();
        let data_chi = k_cols[1].clone();
        let larch_model_k = k_cols[2].clone();
        let larch_model_r_mag = r_cols[6].clone();

        let path = feffpath(
            path_to_string(&fixture_root.join("feffit/Feff_ZnSe/feff_znse.dat")).as_str(),
            FeffFlavor::Feff85L,
        )?
        .set_degen(4.0)
        .set_s02("amp")
        .set_e0("de0")
        .set_sigma2("sig2")
        .set_deltar("dr");

        let fit = FeffFit::new()
            .data(&k, &data_chi)
            .add_path(path)
            .set_inits([("amp", 1.0), ("de0", 0.1), ("sig2", 0.006), ("dr", 0.0)])
            .set_bounds("sig2", 0.0, 0.02)
            .kweight(2.0)
            .window(FTWindow::KaiserBessel)
            .dk(4.0)
            .krange(3.0, 13.0)
            .rrange(1.5, 3.0)
            .fit()?;

        let k_rmse = rmse(&fit.model_chi, &larch_model_k);
        let r_rmse = rmse(&fit.model_chir_mag, &larch_model_r_mag);
        Ok((fit, k_rmse, r_rmse))
    }

    fn save_fit_plots(
        fit: &mut FeffFitResult,
        out: &Path,
        stem: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        fit.plot()
            .k()
            .dataset(0)
            .paths(true)
            .save_png(out.join(format!("fit_{stem}_k.png")))?;

        fit.plot()
            .k()
            .dataset(0)
            .paths(true)
            .window(true)
            .save_png(out.join(format!("fit_{stem}_k_window.png")))?;

        fit.plot()
            .r()
            .dataset(0)
            .save_png(out.join(format!("fit_{stem}_r.png")))?;

        fit.plot()
            .r()
            .dataset(0)
            .window_box(true)
            .save_png(out.join(format!("fit_{stem}_r_window.png")))?;

        Ok(())
    }

    fn save_cu_alias_plots(
        fit: &mut FeffFitResult,
        out: &Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        fit.plot()
            .k()
            .dataset(0)
            .paths(true)
            .save_png(out.join("fit_k.png"))?;
        fit.plot()
            .k()
            .dataset(0)
            .paths(true)
            .window(true)
            .save_png(out.join("fit_k_window.png"))?;
        fit.plot().r().dataset(0).save_png(out.join("fit_r.png"))?;
        fit.plot()
            .r()
            .dataset(0)
            .window_box(true)
            .save_png(out.join("fit_r_window.png"))?;
        Ok(())
    }

    let out = output_dir();
    ensure_dir(&out)?;

    let fixture_root = crate_dir().join("tests/testfiles/xraylarch_d867");
    let refs_root = crate_dir().join("tests/testfiles/larch_fit_refs");

    if !refs_root.exists() {
        return Err(format!(
            "missing larch references at {}\nrun: uv run --with xraylarch python crates/rexafs/scripts/generate_larch_fit_references.py",
            refs_root.display()
        )
        .into());
    }

    // XASSpectrum examples
    let input_path = crate_dir().join("tests/testfiles/Ru_QAS.dat");
    let mut spectrum = load_spectrum_QAS_trans(&input_path)?;
    spectrum.plot().mu().save_png(out.join("spectrum_mu.png"))?;
    spectrum
        .plot()
        .mu()
        .norm()
        .k()
        .r()
        .title("XAS overview")
        .save_png(out.join("spectrum_overview.png"))?;

    // XASGroup examples
    let mut group = XASGroup::new();
    group.add_spectrum(spectrum.clone());

    let mut spectrum_scaled = spectrum.clone();
    if let Some(mu) = spectrum_scaled.mu.take() {
        spectrum_scaled.mu = Some(mu.map(|v| v * 1.03));
    }
    if let Some(raw_mu) = spectrum_scaled.raw_mu.take() {
        spectrum_scaled.raw_mu = Some(raw_mu.map(|v| v * 1.03));
    }
    spectrum_scaled.set_name("scaled-1.03");

    let mut spectrum_shifted = spectrum.clone();
    if let Some(mu) = spectrum_shifted.mu.take() {
        spectrum_shifted.mu = Some(mu.map(|v| v + 0.03));
    }
    if let Some(raw_mu) = spectrum_shifted.raw_mu.take() {
        spectrum_shifted.raw_mu = Some(raw_mu.map(|v| v + 0.03));
    }
    spectrum_shifted.set_name("shifted+0.03");

    group.add_spectrum(spectrum_scaled);
    group.add_spectrum(spectrum_shifted);

    group.plot().mu().save_png(out.join("group_overlay.png"))?;
    group
        .plot()
        .mu()
        .stacked(0.2)
        .save_png(out.join("group_stacked.png"))?;
    group
        .plot()
        .mu()
        .select(&[0, 2])
        .save_png(out.join("group_selected.png"))?;

    let feff8l_rdinp = discover_feff85l_rdinp().ok_or_else(|| {
        "failed to locate feff8l_rdinp executable. set REXAFS_FEFF8L_RDINP or install xraylarch FEFF binaries".to_string()
    })?;

    let feff_materials = [
        ("Co", fixture_root.join("feff8l/Co/feff.inp")),
        (
            "FeO_withPb",
            fixture_root.join("feff8l/FeO_withPb/feff.inp"),
        ),
        ("MnO2", fixture_root.join("feff8l/MnO2/feff.inp")),
        ("ZnSe", fixture_root.join("feffit/Feff_ZnSe/feff.inp")),
    ];

    for (name, feffinp) in feff_materials {
        let npaths = run_feff_material(&feff8l_rdinp, name, &feffinp, &out)?;
        println!("feff material {name}: discovered {npaths} paths");
    }

    let (mut cu_fit, cu_k_rmse, cu_r_rmse) = build_cu_fit(&fixture_root, &refs_root)?;
    let (mut znse_fit, znse_k_rmse, znse_r_rmse) = build_znse_fit(&fixture_root, &refs_root)?;

    save_fit_plots(&mut cu_fit, &out, "cu")?;
    save_fit_plots(&mut znse_fit, &out, "znse")?;
    save_cu_alias_plots(&mut cu_fit, &out)?;

    println!(
        "fit comparison cu: k_rmse={:?}, r_rmse={:?}",
        cu_k_rmse, cu_r_rmse
    );
    println!(
        "fit comparison znse: k_rmse={:?}, r_rmse={:?}",
        znse_k_rmse, znse_r_rmse
    );
    println!("saved demo plots to {}", out.display());
    Ok(())
}
