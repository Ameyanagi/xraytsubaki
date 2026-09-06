//! Parity of k-space, q-space and multi-k-weight FEFF fits against XrayLarch `feffit`.
//!
//! References are produced by `scripts/generate_larch_fitspace_references.py`; see
//! `tests/testfiles/larch_fit_refs/README.md`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use nalgebra::DVector;
use rexafs::prelude::*;
use rexafs::xafs::fitting::estimate_noise;
use serde::Deserialize;

const REL_TOL: f64 = 2.0e-1;
const ABS_TOL_DEFAULT: f64 = 1.0e-8;
const ABS_TOL_DE0_VALUE: f64 = 2.0e-1;

#[derive(Debug, Deserialize)]
struct FitspaceMetadata {
    input_epsilon_k: f64,
    noise_estimate_kw123: NoiseRef,
    references: BTreeMap<String, FitspaceRef>,
}

#[derive(Debug, Deserialize)]
struct NoiseRef {
    epsilon_k: Vec<f64>,
    epsilon_r: Vec<f64>,
}

#[derive(Debug, Deserialize)]
struct FitspaceRef {
    fitspace: String,
    kweights: Vec<f64>,
    chi_square: f64,
    reduced_chi_square: f64,
    r_factor: f64,
    n_idp: f64,
    n_data: usize,
    epsilon_r: Vec<f64>,
    params: BTreeMap<String, ParamRef>,
}

#[derive(Debug, Deserialize)]
struct ParamRef {
    value: f64,
    stderr: f64,
}

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn refs_dir() -> PathBuf {
    crate_dir().join("tests/testfiles/larch_fit_refs")
}

fn load_columns(path: &Path, ncols: usize) -> Vec<DVector<f64>> {
    let content = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("failed to read fixture {}: {e}", path.display()));
    let mut cols = vec![Vec::<f64>::new(); ncols];
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let values = line
            .split_whitespace()
            .map(|v| v.parse::<f64>().unwrap())
            .collect::<Vec<_>>();
        assert!(values.len() >= ncols, "{}: short row", path.display());
        for idx in 0..ncols {
            cols[idx].push(values[idx]);
        }
    }
    cols.into_iter().map(DVector::from_vec).collect()
}

fn load_metadata() -> FitspaceMetadata {
    let meta_path = refs_dir().join("fitspace_metadata.json");
    assert!(
        meta_path.exists(),
        "missing Larch fit-space metadata at {}. regenerate with: uv run --project crates/rexafs/tests/pythonscript python crates/rexafs/scripts/generate_larch_fitspace_references.py",
        meta_path.display()
    );
    let raw = std::fs::read_to_string(&meta_path).unwrap();
    serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("failed to parse {}: {e}", meta_path.display()))
}

fn fitspace_of(reference: &FitspaceRef) -> FitSpace {
    match reference.fitspace.as_str() {
        "k" => FitSpace::K,
        "q" => FitSpace::Q,
        "r" => FitSpace::R,
        other => panic!("unsupported reference fitspace {other}"),
    }
}

fn cu_data() -> (DVector<f64>, DVector<f64>) {
    let k_cols = load_columns(&refs_dir().join("cu_fit_kspace.txt"), 4);
    (k_cols[0].clone(), k_cols[1].clone())
}

fn cu_path() -> FeffPathModel {
    feffpath(
        crate_dir()
            .join("tests/testfiles/xraylarch_d867/feffit/Feff_Cu/feff0001.dat")
            .display()
            .to_string()
            .as_str(),
        FeffFlavor::Feff85L,
    )
    .unwrap()
    .set_s02("amp")
    .set_e0("de0")
    .set_sigma2("sig2")
    .set_deltar("dr")
}

fn build_cu_fit(metadata: &FitspaceMetadata, reference: &FitspaceRef) -> FeffFitResult {
    let (k, chi) = cu_data();
    let value = |name: &str| reference.params[name].value;
    let mut fit = FeffFit::new()
        .data(&k, &chi)
        .epsilon_k(metadata.input_epsilon_k)
        .add_path(cu_path())
        .set_inits([
            ("amp", value("amp")),
            ("de0", value("de0")),
            ("sig2", value("sig2")),
            ("dr", value("dr")),
        ])
        .set_bounds("sig2", 0.0, 0.02)
        .window(FTWindow::KaiserBessel)
        .dk(5.0)
        .krange(3.0, 16.0)
        .rrange(1.4, 3.0)
        .fitspace(fitspace_of(reference));
    fit = if reference.kweights.len() == 1 {
        fit.kweight(reference.kweights[0])
    } else {
        fit.kweights(&reference.kweights)
    };
    fit.fit().unwrap()
}

fn assert_near(actual: f64, expected: f64, abs_tol: f64, label: &str) {
    assert!(actual.is_finite(), "{label} actual is non-finite: {actual}");
    let tol = abs_tol.max(REL_TOL * actual.abs().max(expected.abs()));
    let diff = (actual - expected).abs();
    assert!(
        diff <= tol,
        "{label} mismatch: actual={actual:.15e}, expected={expected:.15e}, diff={diff:.15e}, tol={tol:.15e}"
    );
}

fn assert_fit_matches(result: &FeffFitResult, reference: &FitspaceRef, name: &str) {
    if std::env::var("rexafs_PARITY_DEBUG").is_ok() {
        eprintln!(
            "{name}: actual chi2={} redchi={} rfactor={} n_idp={} n_data={} | expected chi2={} redchi={} rfactor={} n_idp={} n_data={}",
            result.chi_square,
            result.reduced_chi_square,
            result.r_factor,
            result.n_idp,
            result.n_data,
            reference.chi_square,
            reference.reduced_chi_square,
            reference.r_factor,
            reference.n_idp,
            reference.n_data,
        );
        for p in ["amp", "de0", "sig2", "dr"] {
            let actual = result.variables.get(p).unwrap();
            let expected = &reference.params[p];
            eprintln!(
                "{name}.{p}: value actual={} expected={} stderr actual={:?} expected={}",
                actual.value, expected.value, actual.stderr, expected.stderr
            );
        }
    }

    assert_eq!(result.n_data, reference.n_data, "{name}.n_data");
    assert_eq!(result.datasets.len(), 1);
    assert_eq!(result.datasets[0].n_data, reference.n_data);
    assert_eq!(result.kweights, reference.kweights, "{name}.kweights");
    assert_eq!(result.fitspace, fitspace_of(reference), "{name}.fitspace");
    assert_eq!(result.kweight_results.len(), reference.kweights.len());
    let block_total = result
        .kweight_results
        .iter()
        .map(|block| block.n_data)
        .sum::<usize>();
    assert_eq!(block_total, reference.n_data, "{name}: per-kweight n_data");
    for (block, eps_r) in result.kweight_results.iter().zip(&reference.epsilon_r) {
        assert_near(
            block.epsilon_r,
            *eps_r,
            ABS_TOL_DEFAULT,
            &format!("{name}.epsilon_r[kw={}]", block.kweight),
        );
    }

    assert_near(
        result.chi_square,
        reference.chi_square,
        ABS_TOL_DEFAULT,
        &format!("{name}.chi_square"),
    );
    assert_near(
        result.reduced_chi_square,
        reference.reduced_chi_square,
        ABS_TOL_DEFAULT,
        &format!("{name}.reduced_chi_square"),
    );
    assert_near(
        result.r_factor,
        reference.r_factor,
        ABS_TOL_DEFAULT,
        &format!("{name}.r_factor"),
    );
    assert_near(
        result.n_idp,
        reference.n_idp,
        ABS_TOL_DEFAULT,
        &format!("{name}.n_idp"),
    );

    for p in ["amp", "de0", "sig2", "dr"] {
        let actual = result.variables.get(p).unwrap();
        let expected = &reference.params[p];
        // Parameters that Larch itself cannot pin down (stderr comparable to the value) are only
        // required to agree within their Larch uncertainty.
        let abs_tol = if p == "de0" {
            ABS_TOL_DE0_VALUE.max(0.2 * expected.stderr)
        } else {
            (0.2 * expected.stderr).max(ABS_TOL_DEFAULT)
        };
        assert_near(
            actual.value,
            expected.value,
            abs_tol,
            &format!("{name}.params.{p}.value"),
        );
        let stderr = actual
            .stderr
            .unwrap_or_else(|| panic!("missing stderr for {name}.{p}"));
        assert_near(
            stderr,
            expected.stderr,
            ABS_TOL_DEFAULT,
            &format!("{name}.params.{p}.stderr"),
        );
    }
}

#[test]
fn cu_kspace_fit_matches_larch() {
    let metadata = load_metadata();
    let reference = &metadata.references["cu_kspace_kw2"];
    let result = build_cu_fit(&metadata, reference);
    assert_fit_matches(&result, reference, "cu_kspace_kw2");
    // k-space fits carry the k-weighted arrays for plotting.
    let block = &result.kweight_results[0];
    assert_eq!(block.data_chik.len(), result.k.len());
    assert_eq!(block.model_chik.len(), result.k.len());
}

#[test]
fn cu_qspace_fit_matches_larch() {
    let metadata = load_metadata();
    let reference = &metadata.references["cu_qspace_kw2"];
    let result = build_cu_fit(&metadata, reference);
    assert_fit_matches(&result, reference, "cu_qspace_kw2");

    // chi(q) arrays follow Larch's `xftr_fast` convention on Larch's output q grid.
    let cols = load_columns(&refs_dir().join("cu_fit_qspace.txt"), 5);
    let (q_ref, data_ref, model_ref) = (&cols[0], &cols[1], &cols[3]);
    assert_eq!(result.q.len(), q_ref.len());
    assert!((result.q[result.q.len() - 1] - q_ref[q_ref.len() - 1]).abs() < 1.0e-9);
    let scale = data_ref.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    let rmse = |a: &DVector<f64>, b: &DVector<f64>| {
        (a.iter()
            .zip(b.iter())
            .map(|(x, y)| (x - y).powi(2))
            .sum::<f64>()
            / a.len() as f64)
            .sqrt()
    };
    let data_rmse = rmse(&result.data_chiq, data_ref) / scale;
    let model_rmse = rmse(&result.model_chiq, model_ref) / scale;
    if std::env::var("rexafs_PARITY_DEBUG").is_ok() {
        eprintln!("cu chiq parity: data rmse={data_rmse:.3e} model rmse={model_rmse:.3e}");
    }
    assert!(
        data_rmse < 5.0e-3,
        "data chi(q) rmse too large: {data_rmse}"
    );
    assert!(
        model_rmse < 2.0e-2,
        "model chi(q) rmse too large: {model_rmse}"
    );
}

#[test]
fn cu_rspace_multi_kweight_fit_matches_larch() {
    let metadata = load_metadata();
    let reference = &metadata.references["cu_rspace_kw123"];
    let result = build_cu_fit(&metadata, reference);
    assert_fit_matches(&result, reference, "cu_rspace_kw123");

    // Primary arrays correspond to the first k-weight; every block is exposed.
    assert_eq!(result.kweight, 1.0);
    let kws = result
        .kweight_results
        .iter()
        .map(|b| b.kweight)
        .collect::<Vec<_>>();
    assert_eq!(kws, vec![1.0, 2.0, 3.0]);
    assert_eq!(result.data_chir_re, result.kweight_results[0].data_chir_re);
    assert!(result
        .kweight_results
        .iter()
        .all(|b| b.data_chir_mag.len() == result.r.len()));
}

#[test]
fn cu_kspace_multi_kweight_fit_matches_larch() {
    let metadata = load_metadata();
    let reference = &metadata.references["cu_kspace_kw123"];
    let result = build_cu_fit(&metadata, reference);
    assert_fit_matches(&result, reference, "cu_kspace_kw123");
}

#[test]
fn cu_noise_estimate_matches_larch() {
    let metadata = load_metadata();
    let (k, chi) = cu_data();
    let transform = FeffFitTransform {
        kmin: 3.0,
        kmax: 16.0,
        dk: 5.0,
        window: FTWindow::KaiserBessel,
        rmin: 1.4,
        rmax: 3.0,
        kweights: vec![1.0, 2.0, 3.0],
        ..FeffFitTransform::default()
    };
    let noise = estimate_noise(&k, &chi, &transform).unwrap();
    let expected = &metadata.noise_estimate_kw123;
    assert_eq!(noise.epsilon_k.len(), 3);
    for i in 0..3 {
        if std::env::var("rexafs_PARITY_DEBUG").is_ok() {
            eprintln!(
                "noise kw{}: eps_k {} vs {} | eps_r {} vs {}",
                i + 1,
                noise.epsilon_k[i],
                expected.epsilon_k[i],
                noise.epsilon_r[i],
                expected.epsilon_r[i]
            );
        }
        let tol_k = 5.0e-2 * expected.epsilon_k[i];
        let tol_r = 5.0e-2 * expected.epsilon_r[i];
        assert!((noise.epsilon_k[i] - expected.epsilon_k[i]).abs() < tol_k);
        assert!((noise.epsilon_r[i] - expected.epsilon_r[i]).abs() < tol_r);
    }
}
