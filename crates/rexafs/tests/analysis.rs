//! Linear combination fitting and PCA tests on synthetic mixtures built from
//! the Ru K-edge QAS test spectrum.

use std::path::PathBuf;

use nalgebra::DVector;
use rexafs::prelude::*;
use rexafs::xafs::tools::interp_linear;

const TOP_DIR: &str = env!("CARGO_MANIFEST_DIR");
const TMP_DIR: &str = env!("CARGO_TARGET_TMPDIR");

fn testfile(name: &str) -> PathBuf {
    PathBuf::from(TOP_DIR).join("tests/testfiles").join(name)
}

/// Deterministic pseudo-noise (LCG) so tests do not need a rand dependency.
fn noise(n: usize, seed: u64, amplitude: f64) -> DVector<f64> {
    let mut state = seed;
    DVector::from_fn(n, |_, _| {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let u = ((state >> 11) as f64) / ((1u64 << 53) as f64);
        amplitude * (2.0 * u - 1.0)
    })
}

fn e0_of(s: &XASSpectrum) -> f64 {
    s.e0.or_else(|| s.normalization.as_ref().and_then(|n| n.get_e0()))
        .unwrap()
}

/// Standard A: the measured Ru spectrum, normalized.
fn standard_a() -> XASSpectrum {
    let mut s = io::load_spectrum_QAS_trans(testfile("Ru_QAS.dat")).unwrap();
    s.set_name("Ru_A");
    s.normalize().unwrap();
    s
}

/// Synthetic standard: A with its white line scaled by `(1 + bump·gauss)` and
/// its energy axis shifted by `shift` eV, then normalized.
fn synthetic_standard(a: &XASSpectrum, name: &str, shift: f64, bump: f64) -> XASSpectrum {
    let e0 = e0_of(a);
    let energy = a.energy.clone().unwrap();
    let mu = a.mu.clone().unwrap();
    let scaled = DVector::from_fn(mu.len(), |i, _| {
        let z = (energy[i] - e0 - 6.0) / 6.0;
        mu[i] * (1.0 + bump * (-z * z).exp())
    });
    let mut s = XASSpectrum::new();
    s.set_name(name);
    s.set_spectrum(energy, scaled);
    s.shift_energy(shift);
    s.normalize().unwrap();
    s
}

fn standard_b(a: &XASSpectrum) -> XASSpectrum {
    synthetic_standard(a, "Ru_B", 4.0, 0.25)
}

/// Mixture `wa·μA + wb·μB` (B interpolated onto A's grid) plus noise, normalized.
fn mixture(a: &XASSpectrum, b: &XASSpectrum, wa: f64, wb: f64, seed: u64) -> XASSpectrum {
    noisy_mixture(a, b, wa, wb, seed, 0.003)
}

fn noisy_mixture(
    a: &XASSpectrum,
    b: &XASSpectrum,
    wa: f64,
    wb: f64,
    seed: u64,
    amplitude: f64,
) -> XASSpectrum {
    let ea = a.energy.as_ref().unwrap();
    let mu_a = a.mu.as_ref().unwrap();
    let mu_b = interp_linear(ea, b.energy.as_ref().unwrap(), b.mu.as_ref().unwrap()).unwrap();
    let mu = mu_a * wa + mu_b * wb + noise(ea.len(), seed, amplitude);
    let mut s = XASSpectrum::new();
    s.set_name(format!("mix_{wa:.2}_{wb:.2}"));
    s.set_spectrum(ea.clone(), mu);
    s.normalize().unwrap();
    s
}

fn assert_weights(result: &LcfResult, expected: &[(&str, f64)], tol: f64) {
    for (name, w) in expected {
        let got = result.weight_of(name).unwrap();
        assert!(
            (got - w).abs() < tol,
            "{name}: expected {w}, got {got} (result: {:?})",
            result
                .weights
                .iter()
                .map(|c| (c.name.clone(), c.weight))
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn analysis_lcf_norm_recovers_mixture() {
    let a = standard_a();
    let b = standard_b(&a);
    let unknown = mixture(&a, &b, 0.7, 0.3, 1);

    let cfg = LcfConfig::default();
    let result = lcf(&unknown, &[&a, &b], &cfg).unwrap();

    assert_weights(&result, &[("Ru_A", 0.7), ("Ru_B", 0.3)], 0.02);
    assert!((result.sum_of_weights - 1.0).abs() < 1e-9);
    assert!(result.r_factor < 1e-3, "r_factor = {}", result.r_factor);
    assert_eq!(result.n_vary, 1);
    assert_eq!(result.x.len(), result.fit.len());
    assert_eq!(result.components.len(), 2);
    let sum_components: DVector<f64> = &result.components[0] + &result.components[1];
    assert!((sum_components - &result.fit).amax() < 1e-12);
    assert!((&result.data - &result.fit - &result.residual).amax() < 1e-12);
    // Both weights are inside (0, 1) so they carry an uncertainty.
    assert!(result.weights.iter().all(|c| c.stderr.is_some()));
    assert!(result.weights.iter().all(|c| c.e0_shift == 0.0));
    // Grid is Athena's default −20…+30 eV around E₀.
    let e0 = e0_of(&unknown);
    assert!(result.x[0] >= e0 - 20.0 && result.x[result.x.len() - 1] <= e0 + 30.0);
}

#[test]
fn analysis_lcf_deriv_recovers_mixture() {
    let a = standard_a();
    let b = standard_b(&a);
    let unknown = mixture(&a, &b, 0.7, 0.3, 2);

    let cfg = LcfConfig {
        space: LcfSpace::Deriv,
        ..LcfConfig::default()
    };
    let result = lcf(&unknown, &[&a, &b], &cfg).unwrap();
    assert_weights(&result, &[("Ru_A", 0.7), ("Ru_B", 0.3)], 0.02);
    assert!((result.sum_of_weights - 1.0).abs() < 1e-9);
}

#[test]
fn analysis_lcf_flat_and_free_sum() {
    let a = standard_a();
    let b = standard_b(&a);
    let unknown = mixture(&a, &b, 0.7, 0.3, 3);

    let cfg = LcfConfig {
        space: LcfSpace::Flat,
        sum_to_one: false,
        ..LcfConfig::default()
    };
    let result = lcf(&unknown, &[&a, &b], &cfg).unwrap();
    assert_weights(&result, &[("Ru_A", 0.7), ("Ru_B", 0.3)], 0.03);
    assert!((result.sum_of_weights - 1.0).abs() < 0.03);
    assert_eq!(result.n_vary, 2);
}

#[test]
fn analysis_lcf_bounds_clip_pure_standard() {
    // A pure standard fitted with itself and another one: the second weight
    // lands on its lower bound (0) and the first on the upper bound (1).
    let a = standard_a();
    let b = standard_b(&a);
    let result = lcf(&a, &[&a, &b], &LcfConfig::default()).unwrap();
    assert_weights(&result, &[("Ru_A", 1.0), ("Ru_B", 0.0)], 1e-6);
    assert!(result.r_factor < 1e-12);
    assert!(result.weights.iter().all(|c| c.stderr.is_none()));
}

#[test]
fn analysis_lcf_combinatorial_ranks_true_pair_first() {
    let a = standard_a();
    let b = standard_b(&a);
    let c = synthetic_standard(&a, "Ru_C", -3.0, -0.15);
    let unknown = mixture(&a, &b, 0.7, 0.3, 4);

    let cfg = LcfConfig::default();
    let results = lcf_combinatorial(&unknown, &[&a, &b, &c], &cfg, 2).unwrap();
    assert_eq!(results.len(), 3 + 3);
    assert!(results.windows(2).all(|w| w[0].r_factor <= w[1].r_factor));

    let best = &results[0];
    let mut names = best.names();
    names.sort_unstable();
    assert_eq!(names, vec!["Ru_A", "Ru_B"]);
    assert_weights(best, &[("Ru_A", 0.7), ("Ru_B", 0.3)], 0.02);

    // Combinatorial explosion is capped.
    let capped = LcfConfig {
        max_combinations: 2,
        ..LcfConfig::default()
    };
    assert!(matches!(
        lcf_combinatorial(&unknown, &[&a, &b, &c], &capped, 3),
        Err(AnalysisError::TooManyCombinations { count: 7, max: 2 })
    ));
}

#[test]
fn analysis_lcf_e0_shift_recovers_shift() {
    let a = standard_a();
    // The unknown is A shifted by +2 eV: fitting A with a free shift must find δ ≈ +2.
    let shifted = synthetic_standard(&a, "Ru_A_shifted", 2.0, 0.0);
    let cfg = LcfConfig {
        fit_e0_shift: true,
        max_e0_shift: 5.0,
        ..LcfConfig::default()
    };
    let result = lcf(&shifted, &[&a], &cfg).unwrap();
    let comp = &result.weights[0];
    assert!(
        (comp.e0_shift - 2.0).abs() < 0.2,
        "e0_shift = {}",
        comp.e0_shift
    );
    assert!((comp.weight - 1.0).abs() < 1e-6);
    assert!(comp.e0_shift_stderr.is_some());
    assert_eq!(result.n_vary, 1);
    assert!(result.r_factor < 1e-4, "r_factor = {}", result.r_factor);

    // Without the shift the same fit is much worse.
    let fixed = lcf(&shifted, &[&a], &LcfConfig::default()).unwrap();
    assert!(fixed.r_factor > 10.0 * result.r_factor);
}

#[test]
fn analysis_lcf_reports_missing_arrays() {
    let a = standard_a();
    let mut raw = io::load_spectrum_QAS_trans(testfile("Ru_QAS.dat")).unwrap();
    raw.find_e0().unwrap();
    assert!(matches!(
        lcf(&raw, &[&a], &LcfConfig::default()),
        Err(AnalysisError::MissingArray { .. })
    ));
    let no_std: Vec<XASSpectrum> = Vec::new();
    assert!(matches!(
        lcf(&a, &no_std, &LcfConfig::default()),
        Err(AnalysisError::NoSpectra)
    ));
    let empty_range = LcfConfig {
        range: Some((1000.0, 1010.0)),
        ..LcfConfig::default()
    };
    assert!(matches!(
        lcf(&a, &[&a], &empty_range),
        Err(AnalysisError::EmptyRange { .. })
    ));
}

fn mixtures(a: &XASSpectrum, b: &XASSpectrum) -> Vec<XASSpectrum> {
    noisy_mixtures(a, b, 0.003)
}

fn noisy_mixtures(a: &XASSpectrum, b: &XASSpectrum, amplitude: f64) -> Vec<XASSpectrum> {
    [0.9, 0.7, 0.5, 0.3, 0.1]
        .iter()
        .enumerate()
        .map(|(i, &wa)| noisy_mixture(a, b, wa, 1.0 - wa, 10 + i as u64, amplitude))
        .collect()
}

#[test]
fn analysis_pca_two_component_mixtures() {
    let a = standard_a();
    let b = standard_b(&a);
    let spectra = mixtures(&a, &b);

    let model = pca_train(&spectra, &PcaConfig::default()).unwrap();
    assert_eq!(model.n_spectra(), 5);
    assert_eq!(model.n_components(), 5);
    assert_eq!(model.labels.len(), 5);
    assert_eq!(model.ind.len(), 5);
    assert!(!model.centered);

    assert!(
        model.cumulative_variance[1] >= 0.999,
        "cumulative variance = {:?}",
        model.cumulative_variance
    );
    assert!((model.cumulative_variance[4] - 1.0).abs() < 1e-9);
    assert_eq!(model.suggested_components_ind(), 2, "ind = {:?}", model.ind);
    assert_eq!(model.suggested_components_variance(0.999), 2);
    assert!(model.eigenvalues.windows(2).all(|w| w[0] >= w[1]));

    // Components are orthonormal and reproduce the training data.
    let gram = &model.components * model.components.transpose();
    assert!((gram - nalgebra::DMatrix::identity(5, 5)).amax() < 1e-8);
    // Reconstruction error scales with the data magnitude and the platform's
    // SVD kernel (x86 CI differs from Apple Silicon), so compare relatively.
    let recon = &model.scores * &model.components;
    let scale = model.data.amax().max(1.0);
    assert!((recon - &model.data).amax() < 1e-7 * scale);

    // Target transform of a pure standard with 2 components is excellent, with 1 it is not.
    let two = model.target_transform(&a, 2).unwrap();
    assert!(two.r_factor < 1e-3, "r_factor = {}", two.r_factor);
    assert_eq!(two.weights.len(), 2);
    let one = model.target_transform(&a, 1).unwrap();
    assert!(one.r_factor > two.r_factor);
    let bb = model.target_transform(&b, 2).unwrap();
    assert!(bb.r_factor < 1e-3, "r_factor = {}", bb.r_factor);

    // Training spectra reconstruct from 2 components.
    let training = model.reconstruct_training(2, 2).unwrap();
    assert!(training.r_factor < 1e-4);
    assert!(matches!(
        model.reconstruct_training(0, 6),
        Err(AnalysisError::TooManyComponents { .. })
    ));
}

#[test]
fn analysis_pca_centered_mixtures_have_one_direction() {
    let a = standard_a();
    let b = standard_b(&a);
    let spectra = mixtures(&a, &b);
    let cfg = PcaConfig {
        center: true,
        ..PcaConfig::default()
    };
    let model = pca_train(&spectra, &cfg).unwrap();
    assert!(model.centered);
    // Mixtures of two species lie on a line through the mean.
    assert!(model.variance_explained[0] > 0.99);
    assert!(model.mean.amax() > 0.0);
    let fit = model.target_transform(&a, 1).unwrap();
    assert!(fit.r_factor < 1e-3);

    assert!(matches!(
        pca_train(&spectra[..1], &cfg),
        Err(AnalysisError::InsufficientSpectra { min: 2, actual: 1 })
    ));
}

#[test]
fn analysis_pca_deriv_space() {
    let a = standard_a();
    let b = standard_b(&a);
    let cfg = PcaConfig {
        space: AnalysisSpace::Deriv,
        ..PcaConfig::default()
    };
    // The derivative amplifies point noise (0.2 eV grid), so use quieter mixtures.
    let quiet = pca_train(&noisy_mixtures(&a, &b, 3e-4), &cfg).unwrap();
    assert!(
        quiet.cumulative_variance[1] >= 0.99,
        "variance = {:?}",
        quiet.variance_explained
    );
    assert_eq!(quiet.suggested_components_ind(), 2, "ind = {:?}", quiet.ind);

    // With the noisier mixtures the derivative is noise dominated beyond the
    // first component and IND says so.
    let noisy = pca_train(&mixtures(&a, &b), &cfg).unwrap();
    assert!(noisy.cumulative_variance[1] < 0.99);
    assert!(noisy.suggested_components_ind() <= 2);
}

// ---------------------------------------------------------------------------
// Larch parity (slow: `import larch` takes minutes) — run with `--ignored`.
// ---------------------------------------------------------------------------

const LARCH_SCRIPT: &str = r#"
import json, sys
import numpy as np
from larch import Group
from larch.math.lincombo_fitting import lincombo_fit

def load(path, name):
    d = np.loadtxt(path)
    return Group(energy=d[:, 0], norm=d[:, 1], filename=name)

base = sys.argv[1]
unknown = load(f"{base}/unknown.dat", "unknown")
stds = [load(f"{base}/A.dat", "Ru_A"), load(f"{base}/B.dat", "Ru_B")]
xmin, xmax = float(sys.argv[2]), float(sys.argv[3])
out = {}
for tag, kws in (
    ("unbounded", dict()),
    ("bounded", dict(minvals=[0.0, 0.0], maxvals=[1.0, 1.0])),
):
    r = lincombo_fit(unknown, stds, arrayname="norm", xmin=xmin, xmax=xmax,
                     sum_to_one=True, **kws)
    out[tag] = dict(weights=r.weights, rfactor=float(r.rfactor),
                    chisqr=float(r.chisqr), npts=int(len(r.xdata)))
print(json.dumps(out))
"#;

fn write_xy(path: &PathBuf, x: &DVector<f64>, y: &DVector<f64>) {
    let text: String = x
        .iter()
        .zip(y.iter())
        .map(|(x, y)| format!("{x:.6} {y:.10}\n"))
        .collect();
    std::fs::write(path, text).unwrap();
}

#[test]
#[ignore = "runs Larch through uv (import takes minutes)"]
fn analysis_lcf_larch_parity() {
    let a = standard_a();
    let b = standard_b(&a);
    let unknown = mixture(&a, &b, 0.7, 0.3, 1);

    let dir = PathBuf::from(TMP_DIR).join("lcf_larch_parity");
    std::fs::create_dir_all(&dir).unwrap();
    for (s, name) in [(&unknown, "unknown"), (&a, "A"), (&b, "B")] {
        write_xy(
            &dir.join(format!("{name}.dat")),
            s.energy.as_ref().unwrap(),
            &s.get_norm().unwrap(),
        );
    }
    let script = dir.join("lcf_parity.py");
    std::fs::write(&script, LARCH_SCRIPT).unwrap();

    let cfg = LcfConfig::default();
    let ours = lcf(&unknown, &[&a, &b], &cfg).unwrap();
    // Hand Larch the exact grid end points so both fits use the same points.
    let xmin = ours.x[0];
    let xmax = ours.x[ours.x.len() - 1];

    // `LARCH_PROJECT_DIR` overrides the uv project (e.g. when running from a worktree).
    let project = std::env::var("LARCH_PROJECT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(TOP_DIR).join("tests/pythonscript"));
    let output = std::process::Command::new("uv")
        .arg("run")
        .arg("--project")
        .arg(&project)
        .arg("python")
        .arg(&script)
        .arg(&dir)
        .arg(format!("{xmin}"))
        .arg(format!("{xmax}"))
        .output()
        .expect("failed to launch uv");
    assert!(
        output.status.success(),
        "larch script failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(stdout.lines().last().unwrap()).unwrap();

    let bounded = &json["bounded"];
    assert_eq!(bounded["npts"].as_u64().unwrap() as usize, ours.n_data);
    for name in ["Ru_A", "Ru_B"] {
        let larch = bounded["weights"][name].as_f64().unwrap();
        let rust = ours.weight_of(name).unwrap();
        assert!(
            (larch - rust).abs() < 0.01,
            "{name}: larch {larch} vs rust {rust}"
        );
    }
    let larch_r = bounded["rfactor"].as_f64().unwrap();
    assert!(
        (larch_r - ours.r_factor).abs() < 0.2 * larch_r.max(ours.r_factor) + 1e-6,
        "rfactor: larch {larch_r} vs rust {}",
        ours.r_factor
    );

    // Unbounded, sum-to-one: an equality-constrained least squares in both codes.
    let free = LcfConfig {
        weight_bounds: (f64::NEG_INFINITY, f64::INFINITY),
        ..LcfConfig::default()
    };
    let ours_free = lcf(&unknown, &[&a, &b], &free).unwrap();
    for name in ["Ru_A", "Ru_B"] {
        let larch = json["unbounded"]["weights"][name].as_f64().unwrap();
        let rust = ours_free.weight_of(name).unwrap();
        assert!(
            (larch - rust).abs() < 0.01,
            "{name}: larch {larch} vs rust {rust}"
        );
    }
    eprintln!("larch: {json}\nrust bounded: {:?}", ours.weights);
}
