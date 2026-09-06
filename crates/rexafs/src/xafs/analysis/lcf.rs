//! Linear combination fitting (LCF) of an unknown spectrum with a set of
//! standards, following Athena / Larch (`lincombo_fit`, `lincombo_fitall`).
//!
//! # Algorithm
//!
//! The unknown and the standards are taken in the chosen [`AnalysisSpace`]
//! and the standards are interpolated linearly onto the unknown's grid
//! restricted to the fit range. The weights are then the solution of the
//! bounded least-squares problem
//!
//! ```text
//! min ‖Σᵢ wᵢ Sᵢ(x) − U(x)‖²   s.t.   lo ≤ wᵢ ≤ hi,   (optionally) Σᵢ wᵢ = 1
//! ```
//!
//! solved exactly with a primal active-set method for convex quadratic
//! programs (Nocedal & Wright, Alg. 16.3): fixed variables sit on their bound,
//! the equality-constrained step on the free variables is a small KKT system
//! solved by SVD, and bounds are added / released using their Lagrange
//! multipliers.
//!
//! With `fit_e0_shift` each standard additionally gets an energy shift δᵢ
//! (its x axis becomes `x + δᵢ` before interpolation, as `shift_energy(δᵢ)`
//! would do). The shifts are the outer, nonlinear parameters of a variable
//! projection: for a given δ the weights are the bounded linear solution
//! above, and δ is optimised by Levenberg–Marquardt (the `levenberg_marquardt`
//! crate, forward-difference Jacobian) on the projected residual. δᵢ is kept
//! within ±`max_e0_shift` through a `tanh` reparametrisation.
//!
//! Standard errors are the usual (JᵀJ)⁻¹·χ²ᵣ estimates over the *free*
//! parameters; a weight sitting on a bound reports `None`.

use std::borrow::Borrow;

use levenberg_marquardt::{LeastSquaresProblem, LevenbergMarquardt};
use nalgebra::{DMatrix, DVector, Dyn, Owned, SVD};
use serde::{Deserialize, Serialize};

use super::{as_refs, on_grid, r_factor, reference_grid, spectrum_label, AnalysisSpace};
use crate::xafs::errors::AnalysisError;
use crate::xafs::lmutils::forward_jacobian_nalgebra_f64;
use crate::xafs::tools::interp_linear;
use crate::xafs::xasspectrum::XASSpectrum;

/// Configuration of a linear combination fit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LcfConfig {
    /// Array to fit.
    pub space: AnalysisSpace,
    /// Fit range: relative to the unknown's E₀ for energy spaces, absolute
    /// k for `Chi`. `None` → Athena's defaults (−20…+30 eV, 3…12 Å⁻¹).
    pub range: Option<(f64, f64)>,
    /// Force Σ wᵢ = 1 (Athena default: on).
    pub sum_to_one: bool,
    /// Lower / upper bound on every weight (Athena default: 0…1). Use
    /// `f64::NEG_INFINITY` / `f64::INFINITY` for unbounded weights.
    pub weight_bounds: (f64, f64),
    /// Fit an energy shift per standard.
    pub fit_e0_shift: bool,
    /// Maximum |shift| (in x units of the space) when `fit_e0_shift` is on.
    pub max_e0_shift: f64,
    /// Cap on the number of combinations `lcf_combinatorial` will fit.
    pub max_combinations: usize,
}

impl Default for LcfConfig {
    fn default() -> Self {
        Self {
            space: AnalysisSpace::Norm,
            range: None,
            sum_to_one: true,
            weight_bounds: (0.0, 1.0),
            fit_e0_shift: false,
            max_e0_shift: 5.0,
            max_combinations: 1000,
        }
    }
}

/// One fitted standard.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LcfComponent {
    /// Index of the standard in the slice passed to [`lcf`] / [`lcf_combinatorial`].
    pub index: usize,
    pub name: String,
    pub weight: f64,
    /// Standard error of the weight; `None` if it sits on a bound.
    pub stderr: Option<f64>,
    /// Energy (or k) shift applied to the standard.
    pub e0_shift: f64,
    /// Standard error of the shift; `None` if the shift was not fitted.
    pub e0_shift_stderr: Option<f64>,
}

/// Result of a linear combination fit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LcfResult {
    pub space: AnalysisSpace,
    /// Fitted grid (energy or k).
    pub x: DVector<f64>,
    /// Unknown on the grid.
    pub data: DVector<f64>,
    /// Best fit Σ wᵢ Sᵢ.
    pub fit: DVector<f64>,
    /// `data − fit`.
    pub residual: DVector<f64>,
    /// Weights, shifts and uncertainties per standard.
    pub weights: Vec<LcfComponent>,
    /// Scaled contributions wᵢ Sᵢ on the grid, in the order of `weights`.
    pub components: Vec<DVector<f64>>,
    /// Σ(data − fit)² / Σ data².
    pub r_factor: f64,
    /// Σ(data − fit)².
    pub chi_square: f64,
    /// χ² / (n_data − n_vary).
    pub reduced_chi_square: f64,
    pub n_data: usize,
    pub n_vary: usize,
    pub sum_of_weights: f64,
}

impl LcfResult {
    /// Weight of the standard called `name`, if it took part in the fit.
    pub fn weight_of(&self, name: &str) -> Option<f64> {
        self.weights
            .iter()
            .find(|c| c.name == name)
            .map(|c| c.weight)
    }

    /// Names of the standards in this fit.
    pub fn names(&self) -> Vec<&str> {
        self.weights.iter().map(|c| c.name.as_str()).collect()
    }
}

/// Fit `unknown` as a linear combination of `standards`.
pub fn lcf<S: Borrow<XASSpectrum>>(
    unknown: &XASSpectrum,
    standards: &[S],
    cfg: &LcfConfig,
) -> Result<LcfResult, AnalysisError> {
    let refs = as_refs(standards);
    let indices: Vec<usize> = (0..refs.len()).collect();
    lcf_subset(unknown, &refs, &indices, cfg)
}

/// Fit every combination of 1…`max_standards` standards and return the
/// results sorted by R-factor (best first), as Athena's "fit all
/// combinations". Fails with [`AnalysisError::TooManyCombinations`] if the
/// number of combinations exceeds `cfg.max_combinations`.
pub fn lcf_combinatorial<S: Borrow<XASSpectrum>>(
    unknown: &XASSpectrum,
    standards: &[S],
    cfg: &LcfConfig,
    max_standards: usize,
) -> Result<Vec<LcfResult>, AnalysisError> {
    let refs = as_refs(standards);
    let n = refs.len();
    if n == 0 {
        return Err(AnalysisError::NoSpectra);
    }
    let max_k = max_standards.clamp(1, n);
    let count: usize = (1..=max_k).map(|k| binomial(n, k)).sum();
    if count > cfg.max_combinations {
        return Err(AnalysisError::TooManyCombinations {
            count,
            max: cfg.max_combinations,
        });
    }

    let mut results = Vec::with_capacity(count);
    for k in 1..=max_k {
        for combo in combinations(n, k) {
            results.push(lcf_subset(unknown, &refs, &combo, cfg)?);
        }
    }
    results.sort_by(|a, b| {
        a.r_factor
            .partial_cmp(&b.r_factor)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok(results)
}

fn binomial(n: usize, k: usize) -> usize {
    if k > n {
        return 0;
    }
    let k = k.min(n - k);
    (0..k).fold(1usize, |acc, i| acc.saturating_mul(n - i) / (i + 1))
}

/// All k-subsets of `0..n` in lexicographic order.
fn combinations(n: usize, k: usize) -> Vec<Vec<usize>> {
    let mut out = Vec::new();
    let mut current = Vec::with_capacity(k);
    fn rec(start: usize, n: usize, k: usize, current: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
        if current.len() == k {
            out.push(current.clone());
            return;
        }
        for i in start..n {
            current.push(i);
            rec(i + 1, n, k, current, out);
            current.pop();
        }
    }
    rec(0, n, k, &mut current, &mut out);
    out
}

/// Fit with the standards at `indices`.
fn lcf_subset(
    unknown: &XASSpectrum,
    standards: &[&XASSpectrum],
    indices: &[usize],
    cfg: &LcfConfig,
) -> Result<LcfResult, AnalysisError> {
    if indices.is_empty() {
        return Err(AnalysisError::NoSpectra);
    }
    let n_std = indices.len();
    let (x, data) = reference_grid(unknown, cfg.space, cfg.range, n_std + 1)?;

    // Standards in the fit space on their own grids (for shifting) and on the unknown's grid.
    let mut std_arrays = Vec::with_capacity(n_std);
    for &i in indices {
        std_arrays.push(cfg.space.arrays(standards[i])?);
    }
    let (lo, hi) = cfg.weight_bounds;
    let lo_v = vec![lo; n_std];
    let hi_v = vec![hi; n_std];
    let sum_to = cfg.sum_to_one.then_some(1.0);

    let (shifts, shift_stderr) = if cfg.fit_e0_shift {
        fit_shifts(
            &x,
            &data,
            &std_arrays,
            &lo_v,
            &hi_v,
            sum_to,
            cfg.max_e0_shift,
        )?
    } else {
        (vec![0.0; n_std], vec![None; n_std])
    };

    let a = design_matrix(&x, &std_arrays, &shifts)?;
    let w = bounded_lstsq(&a, &data, &lo_v, &hi_v, sum_to)?;

    let fit = &a * &w;
    let residual = &data - &fit;
    let chi_square: f64 = residual.iter().map(|r| r * r).sum();
    let n_data = x.len();
    let n_free_weights = if cfg.sum_to_one {
        n_std.saturating_sub(1)
    } else {
        n_std
    };
    let n_vary = n_free_weights + if cfg.fit_e0_shift { n_std } else { 0 };
    let dof = n_data.saturating_sub(n_vary).max(1) as f64;
    let reduced_chi_square = chi_square / dof;

    let stderr = weight_stderr(&a, &w, &lo_v, &hi_v, cfg.sum_to_one, reduced_chi_square);

    let mut weights = Vec::with_capacity(n_std);
    let mut components = Vec::with_capacity(n_std);
    for (j, &i) in indices.iter().enumerate() {
        weights.push(LcfComponent {
            index: i,
            name: spectrum_label(standards[i], format!("standard{i}")),
            weight: w[j],
            stderr: stderr[j],
            e0_shift: shifts[j],
            e0_shift_stderr: shift_stderr[j],
        });
        components.push(a.column(j) * w[j]);
    }

    Ok(LcfResult {
        space: cfg.space,
        r_factor: r_factor(&data, &fit),
        x,
        data,
        fit,
        residual,
        weights,
        components,
        chi_square,
        reduced_chi_square,
        n_data,
        n_vary,
        sum_of_weights: w.iter().sum(),
    })
}

/// Columns = standards interpolated onto `x` after shifting their x axis by `shifts`.
fn design_matrix(
    x: &DVector<f64>,
    standards: &[(DVector<f64>, DVector<f64>)],
    shifts: &[f64],
) -> Result<DMatrix<f64>, AnalysisError> {
    let mut a = DMatrix::zeros(x.len(), standards.len());
    for (j, ((sx, sy), &delta)) in standards.iter().zip(shifts).enumerate() {
        let col = if delta == 0.0 {
            interp_linear(x, sx, sy)?
        } else {
            interp_linear(x, &sx.add_scalar(delta), sy)?
        };
        a.set_column(j, &col);
    }
    Ok(a)
}

/// Solve `min ‖A w − y‖²` subject to `lo ≤ w ≤ hi` and optionally `Σ w = sum_to`
/// with a primal active-set method.
pub fn bounded_lstsq(
    a: &DMatrix<f64>,
    y: &DVector<f64>,
    lo: &[f64],
    hi: &[f64],
    sum_to: Option<f64>,
) -> Result<DVector<f64>, AnalysisError> {
    let n = a.ncols();
    if n == 0 {
        return Ok(DVector::zeros(0));
    }
    if lo.len() != n || hi.len() != n || lo.iter().zip(hi).any(|(l, h)| l > h) {
        return Err(AnalysisError::LinearAlgebra {
            reason: "invalid weight bounds".to_string(),
        });
    }
    let h = a.transpose() * a;
    let b = a.transpose() * y;
    let scale = 1.0 + h.amax();

    // Feasible starting point.
    let mut w = DVector::zeros(n);
    match sum_to {
        Some(target) => {
            let slo: f64 = lo.iter().sum();
            let shi: f64 = hi.iter().sum();
            if target < slo - 1e-12 || target > shi + 1e-12 {
                return Err(AnalysisError::InfeasibleConstraints {
                    target,
                    lo: slo,
                    hi: shi,
                });
            }
            if slo.is_finite() && shi.is_finite() {
                let span = shi - slo;
                for i in 0..n {
                    w[i] = if span > 0.0 {
                        lo[i] + (target - slo) * (hi[i] - lo[i]) / span
                    } else {
                        lo[i]
                    };
                }
            } else {
                // Some bound is infinite: start from an equal split, clamped
                // into finite bounds; the remainder goes to the first
                // variable that is unbounded in the required direction.
                for i in 0..n {
                    w[i] = (target / n as f64).clamp(lo[i], hi[i]);
                }
                let excess = target - w.iter().sum::<f64>();
                if excess.abs() > 0.0 {
                    if let Some(i) = (0..n).find(|&i| {
                        if excess > 0.0 {
                            hi[i].is_infinite()
                        } else {
                            lo[i].is_infinite()
                        }
                    }) {
                        w[i] += excess;
                    }
                }
            }
        }
        None => {
            for i in 0..n {
                w[i] = 0.0f64.clamp(lo[i], hi[i]);
            }
        }
    }

    // Working set: 0 = free, -1 = on lower bound, +1 = on upper bound.
    let mut active = vec![0i8; n];
    for i in 0..n {
        if lo[i] == hi[i] || w[i] <= lo[i] {
            active[i] = -1;
        } else if w[i] >= hi[i] {
            active[i] = 1;
        }
    }

    let step_tol = 1e-13 * (1.0 + w.amax());
    let mult_tol = 1e-10 * scale;
    let has_eq = sum_to.is_some();
    let max_iter = 30 * n + 50;

    for _ in 0..max_iter {
        let free: Vec<usize> = (0..n).filter(|&i| active[i] == 0).collect();
        let nf = free.len();
        let g = &h * &w - &b;

        // Equality-constrained Newton step on the free variables.
        let m = nf + usize::from(has_eq);
        let mut p = DVector::zeros(n);
        let mut lambda = 0.0;
        if m > 0 {
            let mut kkt = DMatrix::zeros(m, m);
            let mut rhs = DVector::zeros(m);
            for (r, &i) in free.iter().enumerate() {
                for (c, &j) in free.iter().enumerate() {
                    kkt[(r, c)] = h[(i, j)];
                }
                rhs[r] = -g[i];
            }
            if let Some(target) = sum_to {
                for r in 0..nf {
                    kkt[(r, nf)] = 1.0;
                    kkt[(nf, r)] = 1.0;
                }
                rhs[nf] = target - w.iter().sum::<f64>();
            }
            let sol = SVD::new(kkt, true, true)
                .solve(&rhs, 1e-14 * scale)
                .map_err(|e| AnalysisError::LinearAlgebra {
                    reason: e.to_string(),
                })?;
            for (r, &i) in free.iter().enumerate() {
                p[i] = sol[r];
            }
            if has_eq {
                lambda = sol[nf];
            }
        }

        if p.amax() <= step_tol {
            // Converged on the current working set: check the multipliers of
            // the active bounds and release the most negative one.
            let mut worst: Option<(usize, f64)> = None;
            for i in 0..n {
                if active[i] == 0 || lo[i] == hi[i] {
                    continue;
                }
                let mu = if active[i] < 0 {
                    g[i] + lambda
                } else {
                    -(g[i] + lambda)
                };
                if mu < -mult_tol && worst.is_none_or(|(_, m)| mu < m) {
                    worst = Some((i, mu));
                }
            }
            match worst {
                None => break,
                Some((i, _)) => {
                    active[i] = 0;
                    continue;
                }
            }
        }

        // Longest step that keeps the free variables inside their bounds.
        let mut alpha: f64 = 1.0;
        let mut blocking: Option<(usize, i8)> = None;
        for &i in &free {
            if p[i] < 0.0 && lo[i].is_finite() {
                let a_i = (lo[i] - w[i]) / p[i];
                if a_i < alpha {
                    alpha = a_i;
                    blocking = Some((i, -1));
                }
            } else if p[i] > 0.0 && hi[i].is_finite() {
                let a_i = (hi[i] - w[i]) / p[i];
                if a_i < alpha {
                    alpha = a_i;
                    blocking = Some((i, 1));
                }
            }
        }
        let alpha = alpha.max(0.0);
        w += &p * alpha;
        if let Some((i, side)) = blocking {
            w[i] = if side < 0 { lo[i] } else { hi[i] };
            active[i] = side;
        }
    }

    for i in 0..n {
        w[i] = w[i].clamp(lo[i], hi[i]);
    }
    Ok(w)
}

/// Standard errors of the free weights from (JᵀJ)⁻¹·χ²ᵣ, with the sum-to-one
/// constraint eliminated through a null-space basis. Bound weights → `None`.
fn weight_stderr(
    a: &DMatrix<f64>,
    w: &DVector<f64>,
    lo: &[f64],
    hi: &[f64],
    sum_to_one: bool,
    reduced_chi_square: f64,
) -> Vec<Option<f64>> {
    let n = a.ncols();
    let free: Vec<usize> = (0..n)
        .filter(|&i| w[i] > lo[i] + 1e-12 && w[i] < hi[i] - 1e-12)
        .collect();
    let mut out = vec![None; n];
    let nf = free.len();
    if nf == 0 || (sum_to_one && nf < 2) {
        return out;
    }
    let a_f = a.select_columns(free.iter());
    let (jac, basis) = if sum_to_one {
        // Null space of 1ᵀ on the free variables: columns e_k − e_last.
        let mut basis = DMatrix::zeros(nf, nf - 1);
        for k in 0..nf - 1 {
            basis[(k, k)] = 1.0;
            basis[(nf - 1, k)] = -1.0;
        }
        (&a_f * &basis, basis)
    } else {
        (a_f, DMatrix::identity(nf, nf))
    };
    let Some(cov_z) = (jac.transpose() * &jac).try_inverse() else {
        return out;
    };
    let cov = &basis * cov_z * basis.transpose() * reduced_chi_square;
    for (r, &i) in free.iter().enumerate() {
        let v = cov[(r, r)];
        out[i] = (v.is_finite() && v >= 0.0).then(|| v.sqrt());
    }
    out
}

// ---------------------------------------------------------------------------
// Energy-shift fitting (variable projection + Levenberg–Marquardt)
// ---------------------------------------------------------------------------

struct ShiftProblem<'a> {
    u: DVector<f64>,
    x: &'a DVector<f64>,
    y: &'a DVector<f64>,
    standards: &'a [(DVector<f64>, DVector<f64>)],
    lo: &'a [f64],
    hi: &'a [f64],
    sum_to: Option<f64>,
    max_shift: f64,
}

impl ShiftProblem<'_> {
    fn shifts(&self, u: &DVector<f64>) -> Vec<f64> {
        u.iter().map(|v| self.max_shift * v.tanh()).collect()
    }

    fn residual_vec(&self, u: &DVector<f64>) -> DVector<f64> {
        let shifts = self.shifts(u);
        let Ok(a) = design_matrix(self.x, self.standards, &shifts) else {
            return DVector::from_element(self.y.len(), f64::NAN);
        };
        match bounded_lstsq(&a, self.y, self.lo, self.hi, self.sum_to) {
            Ok(w) => &a * w - self.y,
            Err(_) => DVector::from_element(self.y.len(), f64::NAN),
        }
    }
}

impl LeastSquaresProblem<f64, Dyn, Dyn> for ShiftProblem<'_> {
    type ParameterStorage = Owned<f64, Dyn>;
    type ResidualStorage = Owned<f64, Dyn>;
    type JacobianStorage = Owned<f64, Dyn, Dyn>;

    fn set_params(&mut self, u: &DVector<f64>) {
        self.u.copy_from(u);
    }

    fn params(&self) -> DVector<f64> {
        self.u.clone()
    }

    fn residuals(&self) -> Option<DVector<f64>> {
        Some(self.residual_vec(&self.u))
    }

    fn jacobian(&self) -> Option<DMatrix<f64>> {
        let f = |u: &DVector<f64>| self.residual_vec(u);
        Some(forward_jacobian_nalgebra_f64(&self.u, &f))
    }
}

type ShiftFit = (Vec<f64>, Vec<Option<f64>>);

fn fit_shifts(
    x: &DVector<f64>,
    y: &DVector<f64>,
    standards: &[(DVector<f64>, DVector<f64>)],
    lo: &[f64],
    hi: &[f64],
    sum_to: Option<f64>,
    max_shift: f64,
) -> Result<ShiftFit, AnalysisError> {
    let n = standards.len();
    if max_shift <= 0.0 {
        return Ok((vec![0.0; n], vec![None; n]));
    }
    let problem = ShiftProblem {
        u: DVector::zeros(n),
        x,
        y,
        standards,
        lo,
        hi,
        sum_to,
        max_shift,
    };
    let (problem, _report) = LevenbergMarquardt::new()
        .with_ftol(1e-8)
        .with_xtol(1e-8)
        .with_gtol(1e-8)
        .minimize(problem);

    let shifts = problem.shifts(&problem.u);
    let n_data = y.len();
    let n_weights = n - usize::from(sum_to.is_some());
    let dof = n_data.saturating_sub(n + n_weights).max(1) as f64;
    let res = problem.residual_vec(&problem.u);
    let redchi = res.iter().map(|r| r * r).sum::<f64>() / dof;
    let jac = forward_jacobian_nalgebra_f64(&problem.u, &|u| problem.residual_vec(u));
    let stderr = match (jac.transpose() * &jac).try_inverse() {
        Some(cov) => (0..n)
            .map(|i| {
                let du = max_shift * (1.0 - problem.u[i].tanh().powi(2));
                let v = cov[(i, i)] * redchi;
                (v.is_finite() && v >= 0.0).then(|| v.sqrt() * du)
            })
            .collect(),
        None => vec![None; n],
    };
    Ok((shifts, stderr))
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;

    #[test]
    fn bounded_lstsq_matches_unconstrained_when_inside_bounds() {
        let a = DMatrix::from_row_slice(4, 2, &[1.0, 0.0, 0.0, 1.0, 1.0, 1.0, 2.0, 1.0]);
        let w_true = DVector::from_vec(vec![0.3, 0.6]);
        let y = &a * &w_true;
        let w = bounded_lstsq(&a, &y, &[0.0, 0.0], &[1.0, 1.0], None).unwrap();
        assert_abs_diff_eq!(w[0], 0.3, epsilon = 1e-10);
        assert_abs_diff_eq!(w[1], 0.6, epsilon = 1e-10);
    }

    #[test]
    fn bounded_lstsq_clips_to_bounds_and_sum() {
        let a = DMatrix::from_row_slice(3, 2, &[1.0, 0.0, 0.0, 1.0, 1.0, 1.0]);
        // Unconstrained solution would be (1.5, -0.5).
        let y = DVector::from_vec(vec![1.5, -0.5, 1.0]);
        let w = bounded_lstsq(&a, &y, &[0.0, 0.0], &[1.0, 1.0], Some(1.0)).unwrap();
        assert_abs_diff_eq!(w[0], 1.0, epsilon = 1e-10);
        assert_abs_diff_eq!(w[1], 0.0, epsilon = 1e-10);
        let w = bounded_lstsq(&a, &y, &[0.0, 0.0], &[1.0, 1.0], None).unwrap();
        assert_abs_diff_eq!(w[0], 1.0, epsilon = 1e-10);
        assert_abs_diff_eq!(w[1], 0.0, epsilon = 1e-10);
        let w = bounded_lstsq(&a, &y, &[-10.0, -10.0], &[10.0, 10.0], None).unwrap();
        assert_abs_diff_eq!(w[0], 1.5, epsilon = 1e-10);
        assert_abs_diff_eq!(w[1], -0.5, epsilon = 1e-10);
    }

    #[test]
    fn bounded_lstsq_sum_to_one_unbounded_is_equality_lstsq() {
        let a = DMatrix::from_row_slice(3, 2, &[1.0, 0.0, 0.0, 1.0, 1.0, 1.0]);
        let y = DVector::from_vec(vec![1.5, -0.5, 1.0]);
        let inf = f64::INFINITY;
        let w = bounded_lstsq(&a, &y, &[-inf, -inf], &[inf, inf], Some(1.0)).unwrap();
        assert_abs_diff_eq!(w[0] + w[1], 1.0, epsilon = 1e-12);
        assert_abs_diff_eq!(w[0], 1.5, epsilon = 1e-10);
    }

    #[test]
    fn combinations_count() {
        assert_eq!(combinations(4, 2).len(), 6);
        assert_eq!(binomial(5, 3), 10);
        assert_eq!(combinations(3, 3), vec![vec![0, 1, 2]]);
    }
}
