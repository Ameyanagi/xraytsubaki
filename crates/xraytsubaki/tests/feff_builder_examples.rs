use nalgebra::DVector;
use xraytsubaki::prelude::*;

fn fixture_path(name: &str) -> String {
    format!("{}/tests/testfiles/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn load_two_column(path: &str) -> (DVector<f64>, DVector<f64>) {
    let content = std::fs::read_to_string(path).expect("failed to read fixture");
    let mut x = Vec::new();
    let mut y = Vec::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let xv: f64 = parts.next().expect("missing x").parse().expect("invalid x");
        let yv: f64 = parts.next().expect("missing y").parse().expect("invalid y");
        x.push(xv);
        y.push(yv);
    }

    (DVector::from_vec(x), DVector::from_vec(y))
}

fn assert_chi_matches_larch(
    k: &DVector<f64>,
    actual: &DVector<f64>,
    expected: &DVector<f64>,
    epsilon: f64,
) {
    assert_eq!(actual.len(), expected.len(), "model/expected length mismatch");
    assert_eq!(k.len(), expected.len(), "k/model length mismatch");
    for i in 0..k.len() {
        if k[i] < 2.0 {
            continue;
        }
        let diff = (actual[i] - expected[i]).abs();
        assert!(
            diff <= epsilon,
            "k={} index={} |actual-expected|={} > {}",
            k[i],
            i,
            diff,
            epsilon
        );
    }
}

fn larch_truth_variables() -> FitVariables {
    let mut vars = FitVariables::new();
    vars.insert("amp", FitVariable::new(0.92, false));
    vars.insert("de0", FitVariable::new(1.4, false));
    vars.insert("sig2", FitVariable::new(0.0031, false));
    vars.insert("dr", FitVariable::new(0.011, false));
    vars.insert("amp2", FitVariable::new(0.35, false));
    vars.insert("dr2", FitVariable::new(0.0025, false));
    vars
}

#[test]
fn example_path_builder_matches_xraylarch_path2chi() {
    let base_path = feffpath(&fixture_path("feffcu01.dat"), FeffFlavor::Feff85L)
        .unwrap()
        .set_s02("amp")
        .set_e0("de0")
        .set_deltar("dr");
    let path = base_path.clone().set_sigma2("sig2");

    let (k, chi_expected) = load_two_column(&fixture_path("feff_path_chi_larch_ref.txt"));
    let chi = path2chi(&path, &larch_truth_variables(), &k).unwrap();
    assert_chi_matches_larch(&k, &chi, &chi_expected, 3.0e-3);
}

#[test]
fn example_multi_path_model_matches_xraylarch_ff2chi() {
    let path1 = feffpath(&fixture_path("feffcu01.dat"), FeffFlavor::Feff85L)
        .unwrap()
        .set_s02("amp")
        .set_e0("de0")
        .set_sigma2("sig2")
        .set_deltar("dr");
    let path2 = feffpath(&fixture_path("feff0002.dat"), FeffFlavor::Feff85L)
        .unwrap()
        .set_s02("amp2")
        .set_e0("de0")
        .set_sigma2("sig2")
        .set_deltar("dr2");

    let (k, chi_expected) = load_two_column(&fixture_path("feff_ff2chi_larch_ref.txt"));
    let out = ff2chi(&[path1, path2], &larch_truth_variables(), &k).unwrap();
    assert_chi_matches_larch(&k, &out.chi, &chi_expected, 4.0e-3);
}

#[test]
fn example_single_dataset_fit_matches_xraylarch_target_curve() {
    let path = feffpath(&fixture_path("feffcu01.dat"), FeffFlavor::Feff85L)
        .unwrap()
        .set_s02("amp")
        .set_e0("de0")
        .set_sigma2("sig2")
        .set_deltar("dr");
    let (k, chi_expected) = load_two_column(&fixture_path("feff_fit_target_larch.txt"));

    let result = FeffFit::new()
        .data(&k, &chi_expected)
        .add_path(path)
        .set_inits([("amp", 0.95), ("de0", 0.0), ("sig2", 0.002), ("dr", 0.0)])
        .set_bounds("sig2", 0.0, 0.02)
        .krange(2.0, 14.0)
        .rrange(1.0, 3.0)
        .fit()
        .unwrap();

    assert_eq!(result.datasets.len(), 1);
    assert_chi_matches_larch(&k, &result.model_chi, &chi_expected, 6.0e-3);
}

#[test]
fn example_clone_template_fits_xraylarch_reference_curves() {
    let path1 = feffpath(&fixture_path("feffcu01.dat"), FeffFlavor::Feff85L)
        .unwrap()
        .set_s02("amp")
        .set_e0("de0")
        .set_sigma2("sig2")
        .set_deltar("dr");
    let path2 = feffpath(&fixture_path("feff0002.dat"), FeffFlavor::Feff85L)
        .unwrap()
        .set_s02("amp2")
        .set_e0("de0")
        .set_sigma2("sig2")
        .set_deltar("dr2");

    let base = FeffFit::new()
        .params([
            Param::new("amp", 0.95),
            Param::new("de0", 0.0),
            Param::new("sig2", 0.002).bounds(0.0, 0.02),
            Param::new("dr", 0.0),
            Param::new("amp2", 0.2),
            Param::new("dr2", 0.0),
        ])
        .krange(2.0, 14.0)
        .rrange(1.0, 3.0);

    let (k1, chi1_expected) = load_two_column(&fixture_path("feff_path_chi_larch_ref.txt"));
    let (k2, chi2_expected) = load_two_column(&fixture_path("feff_ff2chi_larch_ref.txt"));

    let r1 = base
        .clone()
        .data(&k1, &chi1_expected)
        .add_path(path1.clone())
        .fit()
        .unwrap();
    let r2 = base
        .clone()
        .data(&k2, &chi2_expected)
        .add_path(path1)
        .add_path(path2)
        .fit()
        .unwrap();

    assert_chi_matches_larch(&k1, &r1.model_chi, &chi1_expected, 7.0e-3);
    assert_chi_matches_larch(&k2, &r2.model_chi, &chi2_expected, 7.0e-3);
}

#[test]
fn example_multi_dataset_global_fit_matches_xraylarch_references() {
    let ds1_path = feffpath(&fixture_path("feffcu01.dat"), FeffFlavor::Feff85L)
        .unwrap()
        .set_s02("amp")
        .set_e0("de0")
        .set_sigma2("sig2")
        .set_deltar("dr");
    let ds2_path1 = feffpath(&fixture_path("feffcu01.dat"), FeffFlavor::Feff85L)
        .unwrap()
        .set_s02("amp")
        .set_e0("de0")
        .set_sigma2("sig2")
        .set_deltar("dr");
    let ds2_path2 = feffpath(&fixture_path("feff0002.dat"), FeffFlavor::Feff85L)
        .unwrap()
        .set_s02("amp2")
        .set_e0("de0")
        .set_sigma2("sig2")
        .set_deltar("dr2");

    let (k1, chi1_expected) = load_two_column(&fixture_path("feff_path_chi_larch_ref.txt"));
    let (k2, chi2_expected) = load_two_column(&fixture_path("feff_ff2chi_larch_ref.txt"));

    let ds1 = FeffFitDataset::new()
        .data(&k1, &chi1_expected)
        .add_path(ds1_path)
        .krange(2.0, 14.0)
        .rrange(1.0, 3.0);
    let ds2 = FeffFitDataset::new()
        .data(&k2, &chi2_expected)
        .add_path(ds2_path1)
        .add_path(ds2_path2)
        .krange(2.0, 14.0)
        .rrange(1.0, 3.0);

    let result = FeffFit::new()
        .add_dataset(ds1)
        .add_dataset(ds2)
        .set_inits([
            ("amp", 0.95),
            ("de0", 0.0),
            ("sig2", 0.002),
            ("dr", 0.0),
            ("amp2", 0.2),
            ("dr2", 0.0),
        ])
        .set_bounds("sig2", 0.0, 0.02)
        .fit()
        .unwrap();

    assert_eq!(result.datasets.len(), 2);
    assert_chi_matches_larch(
        &k1,
        &result.datasets[0].model_chi,
        &chi1_expected,
        8.0e-3,
    );
    assert_chi_matches_larch(
        &k2,
        &result.datasets[1].model_chi,
        &chi2_expected,
        8.0e-3,
    );
}
