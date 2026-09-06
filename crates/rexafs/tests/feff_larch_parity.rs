use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use nalgebra::DVector;
use rexafs::prelude::*;
use serde::Deserialize;

const REL_TOL: f64 = 2.0e-1;
const ABS_TOL_DEFAULT: f64 = 1.0e-8;
const ABS_TOL_DE0_VALUE: f64 = 2.0e-1;

#[derive(Debug, Deserialize)]
struct LarchFitMetadata {
    references: LarchReferences,
}

#[derive(Debug, Deserialize)]
struct LarchReferences {
    cu: LarchDatasetRef,
    znse: LarchDatasetRef,
}

#[derive(Debug, Deserialize)]
struct LarchDatasetRef {
    chi_square: f64,
    reduced_chi_square: f64,
    r_factor: f64,
    n_idp: f64,
    epsilon_k: f64,
    params: BTreeMap<String, LarchParamRef>,
}

#[derive(Debug, Deserialize)]
struct LarchParamRef {
    value: f64,
    stderr: f64,
}

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn refs_dir() -> PathBuf {
    crate_dir().join("tests/testfiles/larch_fit_refs")
}

fn fixture_root() -> PathBuf {
    crate_dir().join("tests/testfiles/xraylarch_d867")
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
    assert!(
        meta_path.exists(),
        "missing Larch metadata at {}. regenerate with: uv run --with xraylarch python crates/rexafs/scripts/generate_larch_fit_references.py",
        meta_path.display()
    );
    let raw = std::fs::read_to_string(&meta_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", meta_path.display()));
    serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("failed to parse {}: {e}", meta_path.display()))
}

fn ref_value(reference: &LarchDatasetRef, name: &str) -> f64 {
    reference
        .params
        .get(name)
        .unwrap_or_else(|| panic!("missing reference parameter {name}"))
        .value
}

fn assert_near(actual: f64, expected: f64, label: &str) {
    assert!(actual.is_finite(), "{label} actual is non-finite: {actual}");
    assert!(
        expected.is_finite(),
        "{label} expected is non-finite: {expected}"
    );
    let abs_tol = if label.ends_with(".params.de0.value") {
        ABS_TOL_DE0_VALUE
    } else {
        ABS_TOL_DEFAULT
    };
    let tol = abs_tol.max(REL_TOL * actual.abs().max(expected.abs()));
    let diff = (actual - expected).abs();
    assert!(
        diff <= tol,
        "{label} mismatch: actual={actual:.15e}, expected={expected:.15e}, diff={diff:.15e}, tol={tol:.15e}"
    );
}

fn assert_reference_finite(reference: &LarchDatasetRef, dataset: &str) {
    for (field, value) in [
        ("chi_square", reference.chi_square),
        ("reduced_chi_square", reference.reduced_chi_square),
        ("r_factor", reference.r_factor),
        ("n_idp", reference.n_idp),
        ("epsilon_k", reference.epsilon_k),
    ] {
        assert!(
            value.is_finite(),
            "reference {dataset}.{field} is non-finite: {value}"
        );
    }

    for name in ["amp", "de0", "sig2", "dr"] {
        let param = reference.params.get(name).unwrap_or_else(|| {
            panic!("missing reference parameter {dataset}.params.{name}; regenerate fixtures")
        });
        assert!(
            param.value.is_finite(),
            "reference {dataset}.params.{name}.value is non-finite"
        );
        assert!(
            param.stderr.is_finite(),
            "reference {dataset}.params.{name}.stderr is non-finite"
        );
    }
}

fn build_cu_fit(reference: &LarchDatasetRef) -> FeffFitResult {
    let k_cols = load_columns(&refs_dir().join("cu_fit_kspace.txt"), 4);
    let k = k_cols[0].clone();
    let data_chi = k_cols[1].clone();

    let path = feffpath(
        fixture_root()
            .join("feffit/Feff_Cu/feff0001.dat")
            .display()
            .to_string()
            .as_str(),
        FeffFlavor::Feff85L,
    )
    .unwrap()
    .set_s02("amp")
    .set_e0("de0")
    .set_sigma2("sig2")
    .set_deltar("dr");

    FeffFit::new()
        .data(&k, &data_chi)
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
        .dk(5.0)
        .krange(3.0, 16.0)
        .rrange(1.4, 3.0)
        .fit()
        .unwrap()
}

fn build_znse_fit(reference: &LarchDatasetRef) -> FeffFitResult {
    let k_cols = load_columns(&refs_dir().join("znse_fit_kspace.txt"), 4);
    let k = k_cols[0].clone();
    let data_chi = k_cols[1].clone();

    let path = feffpath(
        fixture_root()
            .join("feffit/Feff_ZnSe/feff_znse.dat")
            .display()
            .to_string()
            .as_str(),
        FeffFlavor::Feff85L,
    )
    .unwrap()
    .set_degen(4.0)
    .set_s02("amp")
    .set_e0("de0")
    .set_sigma2("sig2")
    .set_deltar("dr");

    FeffFit::new()
        .data(&k, &data_chi)
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

fn assert_fit_matches_reference(
    result: &FeffFitResult,
    reference: &LarchDatasetRef,
    dataset: &str,
) {
    if std::env::var("REXAFS_PARITY_DEBUG").is_ok() {
        eprintln!(
            "{dataset}: stats actual chi2={} redchi={} rfactor={} n_idp={} n_data={} | expected chi2={} redchi={} rfactor={} n_idp={}",
            result.chi_square,
            result.reduced_chi_square,
            result.r_factor,
            result.n_idp,
            result.n_data,
            reference.chi_square,
            reference.reduced_chi_square,
            reference.r_factor,
            reference.n_idp,
        );
        if let Some(ds0) = result.datasets.first() {
            eprintln!(
                "{dataset}: dataset0 n_data={} n_idp={} chi2={} redchi={} rfactor={}",
                ds0.n_data, ds0.n_idp, ds0.chi_square, ds0.reduced_chi_square, ds0.r_factor
            );
        }
        for name in ["amp", "de0", "sig2", "dr"] {
            let actual = result.variables.get(name).unwrap();
            let stderr = actual.stderr.unwrap_or(f64::NAN);
            let expected = reference.params.get(name).unwrap();
            eprintln!(
                "{dataset}.{name}: value actual={} expected={} stderr actual={} expected={}",
                actual.value, expected.value, stderr, expected.stderr
            );
        }
    }

    assert_near(
        result.chi_square,
        reference.chi_square,
        &format!("{dataset}.chi_square"),
    );
    assert_near(
        result.reduced_chi_square,
        reference.reduced_chi_square,
        &format!("{dataset}.reduced_chi_square"),
    );
    assert_near(
        result.r_factor,
        reference.r_factor,
        &format!("{dataset}.r_factor"),
    );
    assert_near(result.n_idp, reference.n_idp, &format!("{dataset}.n_idp"));

    for name in ["amp", "de0", "sig2", "dr"] {
        let actual = result
            .variables
            .get(name)
            .unwrap_or_else(|| panic!("missing fitted variable {dataset}.{name}"));
        let expected = reference
            .params
            .get(name)
            .unwrap_or_else(|| panic!("missing reference parameter {dataset}.params.{name}"));

        assert_near(
            actual.value,
            expected.value,
            &format!("{dataset}.params.{name}.value"),
        );

        let stderr = actual.stderr.unwrap_or_else(|| {
            panic!("missing fitted stderr for {dataset}.{name}; covariance scaling failed")
        });
        assert_near(
            stderr,
            expected.stderr,
            &format!("{dataset}.params.{name}.stderr"),
        );
    }
}

#[test]
fn znse_path2chi_curve_matches_larch_reference_curve() {
    let metadata = load_metadata();
    let reference = &metadata.references.znse;
    let k_cols = load_columns(&refs_dir().join("znse_fit_kspace.txt"), 4);
    let k = k_cols[0].clone();
    let larch_model = &k_cols[2];

    let path = feffpath(
        fixture_root()
            .join("feffit/Feff_ZnSe/feff_znse.dat")
            .display()
            .to_string()
            .as_str(),
        FeffFlavor::Feff85L,
    )
    .unwrap()
    .set_degen(4.0)
    .set_s02("amp")
    .set_e0("de0")
    .set_sigma2("sig2")
    .set_deltar("dr");

    let mut vars = FitVariables::new();
    vars.insert("amp", FitVariable::new(ref_value(reference, "amp"), false));
    vars.insert("de0", FitVariable::new(ref_value(reference, "de0"), false));
    vars.insert(
        "sig2",
        FitVariable::new(ref_value(reference, "sig2"), false),
    );
    vars.insert("dr", FitVariable::new(ref_value(reference, "dr"), false));
    let rust_model = path2chi(&path, &vars, &k).unwrap();

    let mut max_abs = 0.0_f64;
    let mut mse = 0.0_f64;
    for i in 0..k.len() {
        let diff = (rust_model[i] - larch_model[i]).abs();
        max_abs = max_abs.max(diff);
        mse += diff * diff;
    }
    let rmse = (mse / k.len() as f64).sqrt();
    if std::env::var("REXAFS_PARITY_DEBUG").is_ok() {
        eprintln!("znse path2chi parity: rmse={rmse:.15e}, max_abs={max_abs:.15e}");
    }
    assert!(rmse <= 2.0e-3, "znse path2chi rmse too large: {rmse}");
}

#[test]
fn larch_metadata_is_complete_and_finite() {
    let metadata = load_metadata();
    assert_reference_finite(&metadata.references.cu, "cu");
    assert_reference_finite(&metadata.references.znse, "znse");
}

#[test]
fn cu_fit_matches_larch_values_and_error_bars() {
    let metadata = load_metadata();
    let result = build_cu_fit(&metadata.references.cu);
    assert_fit_matches_reference(&result, &metadata.references.cu, "cu");
}

#[test]
fn znse_fit_matches_larch_values_and_error_bars() {
    let metadata = load_metadata();
    let result = build_znse_fit(&metadata.references.znse);
    assert_fit_matches_reference(&result, &metadata.references.znse, "znse");
}
