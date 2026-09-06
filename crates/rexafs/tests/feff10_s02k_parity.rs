#![cfg(feature = "feff10-runner")]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use nalgebra::DVector;
use rexafs::prelude::*;
use serde::Deserialize;

const REL_TOL_CORE: f64 = 0.05;
const ABS_TOL_VALUE: f64 = 1.0e-4;
const ABS_TOL_STDERR: f64 = 1.0e-6;
const MAX_RFACTOR_ABS_DELTA: f64 = 1.5e-3;
const MAX_RESIDUAL_RMS_RATIO: f64 = 1.50;

#[derive(Debug, Deserialize)]
struct LarchFitMetadata {
    references: LarchReferences,
}

#[derive(Debug, Deserialize)]
struct LarchReferences {
    znse: LarchDatasetRef,
}

#[derive(Debug, Deserialize)]
struct LarchDatasetRef {
    epsilon_k: f64,
    params: BTreeMap<String, LarchParamRef>,
}

#[derive(Debug, Deserialize)]
struct LarchParamRef {
    value: f64,
}

#[derive(Debug)]
struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(prefix: &str) -> Self {
        let unique = format!(
            "{prefix}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock before unix epoch")
                .as_nanos()
        );
        let path = std::env::temp_dir().join(unique);
        fs::create_dir_all(&path).expect("failed to create temp workspace");
        Self { path }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn fixture_root() -> PathBuf {
    crate_dir().join("tests/testfiles/xraylarch_d867")
}

fn refs_dir() -> PathBuf {
    crate_dir().join("tests/testfiles/larch_fit_refs")
}

fn load_columns(path: &Path, ncols: usize) -> Vec<DVector<f64>> {
    let content = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("failed to read fixture {}: {e}", path.display()));
    let mut cols = vec![Vec::<f64>::new(); ncols];

    for (line_no, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let values = line
            .split_whitespace()
            .map(str::parse::<f64>)
            .collect::<Result<Vec<_>, _>>()
            .unwrap_or_else(|e| {
                panic!(
                    "failed parsing numeric row at {}:{}: {e}",
                    path.display(),
                    line_no + 1
                )
            });
        assert!(
            values.len() >= ncols,
            "{}:{} has {} columns, expected at least {ncols}",
            path.display(),
            line_no + 1,
            values.len()
        );

        for idx in 0..ncols {
            cols[idx].push(values[idx]);
        }
    }

    assert!(
        cols.iter().all(|col| !col.is_empty()),
        "no data rows found in {}",
        path.display()
    );

    cols.into_iter().map(DVector::from_vec).collect()
}

fn load_metadata() -> LarchFitMetadata {
    let meta_path = refs_dir().join("metadata.json");
    let raw = std::fs::read_to_string(&meta_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", meta_path.display()));
    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("failed to parse metadata.json: {e}"))
}

fn ref_value(reference: &LarchDatasetRef, name: &str) -> f64 {
    reference
        .params
        .get(name)
        .unwrap_or_else(|| panic!("missing reference parameter {name}"))
        .value
}

fn assert_within(label: &str, actual: f64, expected: f64, rel: f64, abs_floor: f64) {
    assert!(actual.is_finite(), "{label} actual is non-finite: {actual}");
    assert!(
        expected.is_finite(),
        "{label} expected is non-finite: {expected}"
    );

    let tol = abs_floor.max(rel * actual.abs().max(expected.abs()));
    let diff = (actual - expected).abs();
    assert!(
        diff <= tol,
        "{label} mismatch: actual={actual:.15e}, expected={expected:.15e}, diff={diff:.15e}, tol={tol:.15e}"
    );
}

fn percent_delta(lhs: f64, rhs: f64) -> f64 {
    let denom = lhs.abs().max(rhs.abs());
    if denom < 1.0e-12 {
        0.0
    } else {
        ((lhs - rhs).abs() / denom) * 100.0
    }
}

fn rms(v: &DVector<f64>) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    let mse = v.iter().map(|x| x * x).sum::<f64>() / v.len() as f64;
    mse.sqrt()
}

fn residual_rms(data: &DVector<f64>, model: &DVector<f64>) -> f64 {
    let n = data.len().min(model.len());
    if n == 0 {
        return 0.0;
    }

    let mut err = DVector::zeros(n);
    for i in 0..n {
        err[i] = data[i] - model[i];
    }
    rms(&err)
}

fn znse_data() -> (DVector<f64>, DVector<f64>) {
    let k_cols = load_columns(&refs_dir().join("znse_fit_kspace.txt"), 4);
    (k_cols[0].clone(), k_cols[1].clone())
}

fn configure_znse_path(path: FeffPathModel) -> FeffPathModel {
    path.set_degen(4.0)
        .set_s02("amp")
        .set_e0("de0")
        .set_sigma2("sig2")
        .set_deltar("dr")
}

fn fit_znse_path(
    path: FeffPathModel,
    k: &DVector<f64>,
    data_chi: &DVector<f64>,
    reference: &LarchDatasetRef,
) -> FeffFitResult {
    FeffFit::new()
        .data(k, data_chi)
        .epsilon_k(reference.epsilon_k)
        .add_path(path)
        .set_inits([
            ("amp", ref_value(reference, "amp")),
            ("de0", ref_value(reference, "de0")),
            ("sig2", ref_value(reference, "sig2")),
            ("dr", ref_value(reference, "dr")),
        ])
        .set_bounds("sig2", 0.0, 0.02)
        .kweight(2.0)
        .window(FTWindow::KaiserBessel)
        .dk(4.0)
        .krange(3.0, 13.0)
        .rrange(1.5, 3.0)
        .fit()
        .unwrap()
}

fn select_matching_feff10_path() -> FeffPathModel {
    let fixture = fixture_root();
    let feffinp = fixture.join("feffit/Feff_ZnSe/feff.inp");

    let feff85_reference = feffpath(
        fixture
            .join("feffit/Feff_ZnSe/feff_znse.dat")
            .display()
            .to_string()
            .as_str(),
        FeffFlavor::Feff85L,
    )
    .expect("failed to load FEFF85 ZnSe reference path");

    let workspace = TempDir::new("xfeff10-znse");
    let request = FeffRunRequest {
        executable_path: PathBuf::new(),
        workspace_dir: workspace.path.clone(),
        feffinp: Some(feffinp),
        mode: FeffExecutionMode::Feff10Pipeline,
        timeout_sec: Some(180),
        use_sfconv: false,
        keep_all_outputs: false,
    };

    let run_result = run_feff(&request).expect("FEFF10 pipeline run failed");
    assert!(
        !run_result.path_files.is_empty(),
        "FEFF10 pipeline produced no path files"
    );

    let mut best: Option<(usize, f64, f64, FeffPathModel)> = None;

    for path_file in &run_result.path_files {
        let candidate = feffpath(path_file.to_string_lossy().as_ref(), FeffFlavor::Feff85L)
            .unwrap_or_else(|e| panic!("failed parsing FEFF10 path {}: {e}", path_file.display()));
        let score = (
            candidate.feff.nleg.abs_diff(feff85_reference.feff.nleg),
            (candidate.feff.reff - feff85_reference.feff.reff).abs(),
            (candidate.feff.degen - feff85_reference.feff.degen).abs(),
        );

        let is_better = match &best {
            None => true,
            Some(current) => {
                score.0 < current.0
                    || (score.0 == current.0 && score.1 < current.1)
                    || (score.0 == current.0
                        && (score.1 - current.1).abs() < 1.0e-9
                        && score.2 < current.2)
            }
        };

        if is_better {
            best = Some((score.0, score.1, score.2, candidate));
        }
    }

    let (_, reff_diff, degen_diff, mut selected) = best.expect("no parseable FEFF10 path found");
    assert!(
        reff_diff < 0.2,
        "selected FEFF10 path has large reff diff: {reff_diff}"
    );
    assert!(
        degen_diff < 2.5,
        "selected FEFF10 path has large degeneracy diff: {degen_diff}"
    );

    selected.label = "feff10_znse_selected".to_string();
    selected
}

fn get_var<'a>(result: &'a FeffFitResult, name: &str) -> &'a FitVariable {
    result
        .variables
        .vars
        .get(name)
        .unwrap_or_else(|| panic!("missing variable {name}"))
}

#[test]
fn feff10_znse_fit_has_finite_error_bars_and_similar_model_quality() {
    let metadata = load_metadata();
    let reference = &metadata.references.znse;
    let (k, data_chi) = znse_data();

    let feff85_path = configure_znse_path(
        feffpath(
            fixture_root()
                .join("feffit/Feff_ZnSe/feff_znse.dat")
                .display()
                .to_string()
                .as_str(),
            FeffFlavor::Feff85L,
        )
        .expect("failed to load FEFF85 path"),
    );
    let feff10_path = configure_znse_path(select_matching_feff10_path());

    let feff85_fit = fit_znse_path(feff85_path, &k, &data_chi, reference);
    let feff10_fit = fit_znse_path(feff10_path, &k, &data_chi, reference);

    let mut report = String::from(
        "param,feff10_value,feff85_value,value_delta_pct,feff10_stderr,feff85_stderr,stderr_delta_pct\n",
    );

    for name in ["amp", "de0", "sig2", "dr"] {
        let lhs = get_var(&feff10_fit, name);
        let rhs = get_var(&feff85_fit, name);
        let lhs_stderr = lhs
            .stderr
            .unwrap_or_else(|| panic!("missing FEFF10 stderr for {name}"));
        let rhs_stderr = rhs
            .stderr
            .unwrap_or_else(|| panic!("missing FEFF85 stderr for {name}"));
        assert!(
            lhs_stderr.is_finite() && rhs_stderr.is_finite(),
            "{name} stderr must be finite"
        );
        let value_delta_pct = percent_delta(lhs.value, rhs.value);
        let stderr_delta_pct = percent_delta(lhs_stderr, rhs_stderr);
        if std::env::var("REXAFS_PARITY_DEBUG").is_ok() {
            eprintln!(
                "{name}: value feff10={:.15e} feff85={:.15e} (delta={:.3}%) stderr feff10={:.15e} feff85={:.15e} (delta={:.3}%)",
                lhs.value,
                rhs.value,
                value_delta_pct,
                lhs_stderr,
                rhs_stderr,
                stderr_delta_pct
            );
        }

        report.push_str(&format!(
            "{name},{:.15e},{:.15e},{:.6},{:.15e},{:.15e},{:.6}\n",
            lhs.value, rhs.value, value_delta_pct, lhs_stderr, rhs_stderr, stderr_delta_pct
        ));
    }

    let rfactor_delta_pct = percent_delta(feff10_fit.r_factor, feff85_fit.r_factor);
    let rfactor_abs_delta = (feff10_fit.r_factor - feff85_fit.r_factor).abs();
    let feff10_residual_rms = residual_rms(&data_chi, &feff10_fit.model_chi);
    let feff85_residual_rms = residual_rms(&data_chi, &feff85_fit.model_chi);
    let residual_rms_ratio = if feff85_residual_rms < 1.0e-12 {
        0.0
    } else {
        feff10_residual_rms / feff85_residual_rms
    };
    assert!(
        rfactor_abs_delta <= MAX_RFACTOR_ABS_DELTA,
        "R-factor absolute drift too large: FEFF10={} FEFF85={} abs_delta={:.6e}",
        feff10_fit.r_factor,
        feff85_fit.r_factor,
        rfactor_abs_delta
    );
    assert!(
        residual_rms_ratio <= MAX_RESIDUAL_RMS_RATIO,
        "FEFF10 residual RMS is too large vs FEFF85: feff10={:.6e} feff85={:.6e} ratio={:.6}",
        feff10_residual_rms,
        feff85_residual_rms,
        residual_rms_ratio
    );

    report.push_str(&format!(
        "summary_r_factor,{:.15e},{:.15e},{:.6},{:.15e},,\n",
        feff10_fit.r_factor, feff85_fit.r_factor, rfactor_delta_pct, rfactor_abs_delta
    ));
    report.push_str(&format!(
        "summary_residual_rms,{:.15e},{:.15e},{:.6},,,\n",
        feff10_residual_rms, feff85_residual_rms, residual_rms_ratio
    ));

    let report_path = crate_dir().join("target/feff10_vs_feff85_znse_fit_report.csv");
    if let Some(parent) = report_path.parent() {
        fs::create_dir_all(parent).expect("failed to create target dir for parity report");
    }
    fs::write(&report_path, report)
        .unwrap_or_else(|e| panic!("failed writing report {}: {e}", report_path.display()));
}

#[test]
fn feff10_a_times_s02k_amp_is_stable_across_k_ranges() {
    let metadata = load_metadata();
    let reference = &metadata.references.znse;
    let (k, data_chi) = znse_data();
    let path = configure_znse_path(select_matching_feff10_path());

    let baseline_full_fit = fit_znse_path(path.clone(), &k, &data_chi, reference);
    let de0_fixed = get_var(&baseline_full_fit, "de0").value;
    let sig2_fixed = get_var(&baseline_full_fit, "sig2").value;
    let dr_fixed = get_var(&baseline_full_fit, "dr").value;
    let amp_seed = get_var(&baseline_full_fit, "amp").value;

    let ranges = [(3.0, 13.0), (3.0, 12.5), (3.5, 13.0), (3.5, 12.5)];
    let mut rows = Vec::new();

    for (kmin, kmax) in ranges {
        let result = FeffFit::new()
            .data(&k, &data_chi)
            .epsilon_k(reference.epsilon_k)
            .add_path(path.clone())
            .set_init("amp", amp_seed)
            .fix("de0", de0_fixed)
            .fix("sig2", sig2_fixed)
            .fix("dr", dr_fixed)
            .kweight(2.0)
            .window(FTWindow::KaiserBessel)
            .dk(4.0)
            .krange(kmin, kmax)
            .rrange(1.5, 3.0)
            .fit()
            .unwrap();

        let amp = get_var(&result, "amp");
        let amp_stderr = amp
            .stderr
            .unwrap_or_else(|| panic!("missing stderr for amp at k-range [{kmin}, {kmax}]"));

        rows.push((
            kmin,
            kmax,
            amp.value,
            amp_stderr,
            result.chi_square,
            result.r_factor,
        ));
    }

    debug_assert!(
        !rows.is_empty(),
        "ranges is a compile-time constant; this should never be empty"
    );
    let (_, _, baseline_amp, baseline_stderr, _, _) = rows[0];

    for (kmin, kmax, amp, amp_stderr, _, _) in rows.iter().skip(1) {
        assert_within(
            &format!("amp.value[{kmin:.1},{kmax:.1}]"),
            *amp,
            baseline_amp,
            REL_TOL_CORE,
            ABS_TOL_VALUE,
        );
        assert_within(
            &format!("amp.stderr[{kmin:.1},{kmax:.1}]"),
            *amp_stderr,
            baseline_stderr,
            REL_TOL_CORE,
            ABS_TOL_STDERR,
        );
    }

    let report_path = crate_dir().join("target/feff10_s02k_report.csv");
    if let Some(parent) = report_path.parent() {
        fs::create_dir_all(parent).expect("failed to create target dir for report");
    }
    let mut report = String::from("kmin,kmax,amp,amp_stderr,chi_square,r_factor\n");
    for (kmin, kmax, amp, amp_stderr, chi_square, r_factor) in rows {
        report.push_str(&format!(
            "{kmin:.3},{kmax:.3},{amp:.15e},{amp_stderr:.15e},{chi_square:.15e},{r_factor:.15e}\n"
        ));
    }
    fs::write(&report_path, report)
        .unwrap_or_else(|e| panic!("failed writing report {}: {e}", report_path.display()));
}
