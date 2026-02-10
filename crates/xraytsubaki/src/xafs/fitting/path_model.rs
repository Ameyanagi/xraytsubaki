use std::collections::BTreeMap;
use std::path::Path;

use nalgebra::DVector;
use num_complex::Complex64;

use crate::xafs::xafsutils::constants::ETOK;

use super::errors::FittingError;
use super::feffdat::parse_feff_path_file;
use super::types::{FeffFlavor, FeffPathModel, FitVariables};
use super::variables::resolve_path_param;

const SMALL_Q: f64 = 1.0e-7;

#[derive(Debug, Clone)]
pub struct FF2ChiOutput {
    pub chi: DVector<f64>,
    pub path_chi: Vec<(String, DVector<f64>)>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PathParams {
    degen: f64,
    s02: f64,
    e0: f64,
    ei: f64,
    deltar: f64,
    sigma2: f64,
    third: f64,
    fourth: f64,
}

pub fn feffpath<P: AsRef<Path>>(
    path: P,
    flavor: FeffFlavor,
) -> Result<FeffPathModel, FittingError> {
    let parsed = parse_feff_path_file(path.as_ref(), flavor)?;
    let label = path
        .as_ref()
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "path".to_string());
    Ok(FeffPathModel::from_feffdat(label, parsed))
}

pub fn path2chi(
    path: &FeffPathModel,
    vars: &FitVariables,
    k: &DVector<f64>,
) -> Result<DVector<f64>, FittingError> {
    let globals = vars.resolve_values()?;
    let params = resolve_params(path, &globals)?;
    let (_cchi, chi) = calc_path_chi(path, &params, k)?;
    Ok(chi)
}

pub fn ff2chi(
    paths: &[FeffPathModel],
    vars: &FitVariables,
    k: &DVector<f64>,
) -> Result<FF2ChiOutput, FittingError> {
    if paths.is_empty() {
        return Err(FittingError::EmptyPaths);
    }

    let globals = vars.resolve_values()?;
    let mut total = DVector::zeros(k.len());
    let mut path_chi = Vec::with_capacity(paths.len());

    for path in paths.iter().filter(|path| path.use_path) {
        let params = resolve_params(path, &globals)?;
        let (_cchi, chi) = calc_path_chi(path, &params, k)?;
        total += &chi;
        path_chi.push((path.label.clone(), chi));
    }

    if path_chi.is_empty() {
        return Err(FittingError::EmptyPaths);
    }

    Ok(FF2ChiOutput {
        chi: total,
        path_chi,
    })
}

pub(crate) fn calc_path_chi(
    path: &FeffPathModel,
    params: &PathParams,
    k: &DVector<f64>,
) -> Result<(Vec<Complex64>, DVector<f64>), FittingError> {
    if path.feff.reff < 0.05 {
        return Err(FittingError::InvalidFeffData {
            reason: format!("path '{}' has invalid reff {}", path.label, path.feff.reff),
        });
    }
    if k.len() < 3 {
        return Err(FittingError::InvalidDataset {
            reason: "k grid must contain at least 3 points".to_string(),
        });
    }
    if let Some(index) = k.iter().position(|kv| !kv.is_finite()) {
        return Err(FittingError::InvalidDataset {
            reason: format!("k grid contains non-finite value at index {index}"),
        });
    }

    let reff = path.feff.reff;
    let q = k.map(|kv| {
        let energy = kv * kv - params.e0 * ETOK;
        let signed_q = energy.signum() * energy.abs().sqrt();
        if signed_q.abs() < SMALL_Q {
            if signed_q.is_sign_negative() {
                -SMALL_Q
            } else {
                SMALL_Q
            }
        } else {
            signed_q
        }
    });

    let pha = interp_linear_clamped(&path.feff.k, &path.feff.pha, &q)?;
    let amp = interp_linear_clamped(&path.feff.k, &path.feff.amp, &q)?;
    let rep = interp_linear_clamped(&path.feff.k, &path.feff.rep, &q)?;
    let lam = interp_linear_clamped(&path.feff.k, &path.feff.lam, &q)?;

    let mut cchi: Vec<Complex64> = Vec::with_capacity(k.len());
    for i in 0..k.len() {
        let inv_lam = if lam[i].abs() < SMALL_Q {
            1.0 / SMALL_Q
        } else {
            1.0 / lam[i]
        };

        let pp = Complex64::new(rep[i], inv_lam).powi(2) + Complex64::new(0.0, params.ei * ETOK);
        let p = pp.sqrt();

        let sigma_term = params.sigma2 - pp * (params.fourth / 3.0);
        let delta_term = params.deltar - 2.0 * params.sigma2 / reff - 2.0 * pp * params.third / 3.0;
        let phase_inner = Complex64::new(2.0 * q[i] * reff + pha[i], 0.0) + 2.0 * p * delta_term;

        let exponent = Complex64::new(-2.0 * reff * p.im, 0.0) - 2.0 * pp * sigma_term
            + Complex64::new(0.0, 1.0) * phase_inner;

        let denom_q = if q[i].abs() < SMALL_Q { SMALL_Q } else { q[i] };
        let denom_r = (reff + params.deltar).powi(2).max(SMALL_Q);

        let scale = params.degen * params.s02 * amp[i] / (denom_q * denom_r);
        cchi.push(scale * exponent.exp());
    }

    if cchi.len() >= 3 {
        cchi[0] = 2.0 * cchi[1] - cchi[2];
    }

    let chi = DVector::from_iterator(cchi.len(), cchi.iter().map(|value| value.im));
    Ok((cchi, chi))
}

pub(crate) fn resolve_params(
    path: &FeffPathModel,
    globals: &BTreeMap<String, f64>,
) -> Result<PathParams, FittingError> {
    let mut locals = BTreeMap::new();
    locals.insert("reff".to_string(), path.feff.reff);

    Ok(PathParams {
        degen: resolve_path_param(&path.degen, path.feff.degen, globals, &locals)?,
        s02: resolve_path_param(&path.s02, 1.0, globals, &locals)?,
        e0: resolve_path_param(&path.e0, 0.0, globals, &locals)?,
        ei: resolve_path_param(&path.ei, 0.0, globals, &locals)?,
        deltar: resolve_path_param(&path.deltar, 0.0, globals, &locals)?,
        sigma2: resolve_path_param(&path.sigma2, 0.0, globals, &locals)?,
        third: resolve_path_param(&path.third, 0.0, globals, &locals)?,
        fourth: resolve_path_param(&path.fourth, 0.0, globals, &locals)?,
    })
}

fn interp_linear_clamped(
    xin: &DVector<f64>,
    yin: &DVector<f64>,
    xout: &DVector<f64>,
) -> Result<DVector<f64>, FittingError> {
    if xin.len() != yin.len() || xin.len() < 2 {
        return Err(FittingError::InvalidFeffData {
            reason: "interpolation requires at least 2 FEFF grid points".to_string(),
        });
    }

    for i in 1..xin.len() {
        if xin[i] < xin[i - 1] {
            return Err(FittingError::InvalidFeffData {
                reason: "FEFF k grid must be monotonic for interpolation".to_string(),
            });
        }
    }

    let mut out = DVector::zeros(xout.len());
    let xs = xin.as_slice();
    let ys = yin.as_slice();

    for (i, &x) in xout.iter().enumerate() {
        if x <= xs[0] {
            out[i] = ys[0];
            continue;
        }
        if x >= xs[xs.len() - 1] {
            out[i] = ys[ys.len() - 1];
            continue;
        }

        let idx = match xs.binary_search_by(|probe| probe.partial_cmp(&x).unwrap()) {
            Ok(found) => found,
            Err(insert) => insert,
        };
        let hi = idx;
        let lo = hi.saturating_sub(1);

        let x0 = xs[lo];
        let x1 = xs[hi];
        let y0 = ys[lo];
        let y1 = ys[hi];

        let t = if (x1 - x0).abs() < f64::EPSILON {
            0.0
        } else {
            (x - x0) / (x1 - x0)
        };
        out[i] = y0 + t * (y1 - y0);
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xafs::fitting::types::{FeffFlavor, PathParamSpec};
    use crate::xafs::tests::{PARAM_LOADTXT, TOP_DIR};
    use approx::assert_abs_diff_eq;
    use data_reader::reader::load_txt_f64;

    #[test]
    fn test_path2chi_returns_finite_values() {
        let pathfile = format!("{TOP_DIR}/tests/testfiles/feffcu01.dat");
        let mut path = feffpath(pathfile, FeffFlavor::Feff85L).unwrap();
        path.s02 = PathParamSpec::Expression("amp".to_string());
        path.e0 = PathParamSpec::Expression("de0".to_string());
        path.sigma2 = PathParamSpec::Expression("sig2".to_string());

        let mut vars = FitVariables::new();
        vars.insert("amp", super::super::types::FitVariable::new(0.95, true));
        vars.insert("de0", super::super::types::FitVariable::new(1.5, true));
        vars.insert("sig2", super::super::types::FitVariable::new(0.003, true));

        let k = DVector::from_iterator(240, (0..240).map(|i| 0.05 * (i as f64 + 1.0)));
        let chi = path2chi(&path, &vars, &k).unwrap();

        assert_eq!(chi.len(), k.len());
        assert!(chi.iter().all(|value| value.is_finite()));
    }

    #[test]
    fn test_path2chi_rejects_non_finite_k_grid() {
        let pathfile = format!("{TOP_DIR}/tests/testfiles/feffcu01.dat");
        let path = feffpath(pathfile, FeffFlavor::Feff85L).unwrap();
        let vars = FitVariables::new();

        let mut k = DVector::from_iterator(240, (0..240).map(|i| 0.05 * (i as f64 + 1.0)));
        k[10] = f64::NAN;

        let err = path2chi(&path, &vars, &k).unwrap_err();
        assert!(matches!(err, FittingError::InvalidDataset { .. }));
    }

    #[test]
    fn test_ff2chi_combines_multiple_paths() {
        let pathfile1 = format!("{TOP_DIR}/tests/testfiles/feffcu01.dat");
        let pathfile2 = format!("{TOP_DIR}/tests/testfiles/feff0002.dat");

        let path1 = feffpath(pathfile1, FeffFlavor::Feff85L).unwrap();
        let path2 = feffpath(pathfile2, FeffFlavor::Feff85L).unwrap();

        let vars = FitVariables::new();
        let k = DVector::from_iterator(200, (0..200).map(|i| 0.05 * (i as f64 + 1.0)));

        let out = ff2chi(&[path1, path2], &vars, &k).unwrap();
        assert_eq!(out.chi.len(), k.len());
        assert_eq!(out.path_chi.len(), 2);
    }

    #[test]
    fn test_path2chi_matches_larch_reference_within_tolerance() {
        let pathfile = format!("{TOP_DIR}/tests/testfiles/feffcu01.dat");
        let reference_path = format!("{TOP_DIR}/tests/testfiles/feff_path_chi_larch_ref.txt");

        let mut path = feffpath(pathfile, FeffFlavor::Feff85L).unwrap();
        path.s02 = PathParamSpec::Expression("amp".to_string());
        path.e0 = PathParamSpec::Expression("de0".to_string());
        path.sigma2 = PathParamSpec::Expression("sig2".to_string());
        path.deltar = PathParamSpec::Expression("dr".to_string());

        let mut vars = FitVariables::new();
        vars.insert("amp", super::super::types::FitVariable::new(0.92, false));
        vars.insert("de0", super::super::types::FitVariable::new(1.4, false));
        vars.insert("sig2", super::super::types::FitVariable::new(0.0031, false));
        vars.insert("dr", super::super::types::FitVariable::new(0.011, false));

        let reference = load_txt_f64(&reference_path, &PARAM_LOADTXT).unwrap();
        let k = DVector::from_vec(reference.get_col(0));
        let chi_expected = reference.get_col(1);
        let chi = path2chi(&path, &vars, &k).unwrap();

        for ((kv, actual), expected) in k.iter().zip(chi.iter()).zip(chi_expected.iter()) {
            if *kv < 2.0 {
                continue;
            }
            assert_abs_diff_eq!(actual, expected, epsilon = 3.0e-3);
        }
    }

    #[test]
    fn test_ff2chi_matches_larch_reference_within_tolerance() {
        let pathfile1 = format!("{TOP_DIR}/tests/testfiles/feffcu01.dat");
        let pathfile2 = format!("{TOP_DIR}/tests/testfiles/feff0002.dat");
        let reference_path = format!("{TOP_DIR}/tests/testfiles/feff_ff2chi_larch_ref.txt");

        let mut path1 = feffpath(pathfile1, FeffFlavor::Feff85L).unwrap();
        path1.s02 = PathParamSpec::Expression("amp".to_string());
        path1.e0 = PathParamSpec::Expression("de0".to_string());
        path1.sigma2 = PathParamSpec::Expression("sig2".to_string());
        path1.deltar = PathParamSpec::Expression("dr".to_string());

        let mut path2 = feffpath(pathfile2, FeffFlavor::Feff85L).unwrap();
        path2.s02 = PathParamSpec::Expression("amp2".to_string());
        path2.e0 = PathParamSpec::Expression("de0".to_string());
        path2.sigma2 = PathParamSpec::Expression("sig2".to_string());
        path2.deltar = PathParamSpec::Expression("dr2".to_string());

        let mut vars = FitVariables::new();
        vars.insert("amp", super::super::types::FitVariable::new(0.92, false));
        vars.insert("de0", super::super::types::FitVariable::new(1.4, false));
        vars.insert("sig2", super::super::types::FitVariable::new(0.0031, false));
        vars.insert("dr", super::super::types::FitVariable::new(0.011, false));
        vars.insert("amp2", super::super::types::FitVariable::new(0.35, false));
        vars.insert("dr2", super::super::types::FitVariable::new(0.0025, false));

        let reference = load_txt_f64(&reference_path, &PARAM_LOADTXT).unwrap();
        let k = DVector::from_vec(reference.get_col(0));
        let chi_expected = reference.get_col(1);

        let out = ff2chi(&[path1, path2], &vars, &k).unwrap();
        for ((kv, actual), expected) in k.iter().zip(out.chi.iter()).zip(chi_expected.iter()) {
            if *kv < 2.0 {
                continue;
            }
            assert_abs_diff_eq!(actual, expected, epsilon = 4.0e-3);
        }
    }
}
