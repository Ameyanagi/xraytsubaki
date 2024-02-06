use levenberg_marquardt::{LeastSquaresProblem, LevenbergMarquardt};
use nalgebra::{DMatrix, DVector, Dyn, Owned};

const EPS_F64: f64 = std::f64::EPSILON;

/// Update the function value at x[idx] and return the value.
pub fn mod_and_calc_nalgebra_f64<T>(
    x: &mut DVector<f64>,
    f: &dyn Fn(&DVector<f64>) -> T,
    idx: usize,
    y: f64,
) -> T {
    let xtmp = x[idx];
    x[idx] = xtmp + y;
    let fx1 = (f)(&x);
    x[idx] = xtmp;
    fx1
}

/// Calculation of the Jacobian matrix by forward difference.
pub fn forward_jacobian_nalgebra_f64(
    x: &DVector<f64>,
    fs: &dyn Fn(&DVector<f64>) -> DVector<f64>,
) -> DMatrix<f64> {
    let fx = (fs)(&x);
    let mut xt = x.clone();
    let mut jac = DMatrix::zeros(fx.len(), x.len());
    for i in 0..x.len() {
        let fx1 = mod_and_calc_nalgebra_f64(&mut xt, fs, i, EPS_F64.sqrt());
        jac.set_column(i, &((fx1 - &fx) / EPS_F64.sqrt()));
    }
    jac
}

/// Calculation of the Jacobian matrix by central difference.
pub fn center_jacobian_nalgebra_f64(
    x: &DVector<f64>,
    fs: &dyn Fn(&DVector<f64>) -> DVector<f64>,
) -> DMatrix<f64> {
    let fx = (fs)(&x);
    let mut xt = x.clone();
    let mut jac = DMatrix::zeros(fx.len(), x.len());
    for i in 0..x.len() {
        let fx1 = mod_and_calc_nalgebra_f64(&mut xt, fs, i, EPS_F64.sqrt());
        let fx2 = mod_and_calc_nalgebra_f64(&mut xt, fs, i, -EPS_F64.sqrt());
        jac.set_column(i, &((fx1 - fx2) / (2.0 * EPS_F64.sqrt())));
    }
    jac
}

/// Calculation of the approximate Hessian matrix.
/// The Hessian matrix is calculated as the product of the Jacobian matrix and its transpose.
/// This approximation is valid only when the residual function near the minimum.
pub fn approx_hessian_nalgebra_f64(
    x: &DVector<f64>,
    fs: &dyn Fn(&DVector<f64>) -> DVector<f64>,
) -> DMatrix<f64> {
    let jac = forward_jacobian_nalgebra_f64(x, fs);
    let hess = jac.transpose() * &jac;
    hess
}

/// Calculation of the approximate covariance matrix.
/// The covariance matrix is calculated as the inverse of the approximate Hessian matrix.
/// This approximation is valid only when the residual function near the minimum.
pub fn approx_covariance_matrix_nalgebra_f64(
    x: &DVector<f64>,
    fs: &dyn Fn(&DVector<f64>) -> DVector<f64>,
) -> Option<DMatrix<f64>> {
    let hess = approx_hessian_nalgebra_f64(x, fs);
    let cov = hess.try_inverse();
    cov
}

/// Trait for Levenberg-Marquardt parameters.
/// It implements the Jacobian matrix, the Hessian matrix, and the covariance matrix.
pub trait LMParameters<T> {
    fn jacobian(&self, f: T) -> DMatrix<f64>;
    fn hessian(&self, f: T) -> DMatrix<f64>;
    fn covariance(&self, f: T) -> Option<DMatrix<f64>>;
}

impl LMParameters<&dyn Fn(&DVector<f64>) -> DVector<f64>> for DVector<f64> {
    fn jacobian(&self, f: &dyn Fn(&DVector<f64>) -> DVector<f64>) -> DMatrix<f64> {
        forward_jacobian_nalgebra_f64(self, f)
    }

    fn hessian(&self, f: &dyn Fn(&DVector<f64>) -> DVector<f64>) -> DMatrix<f64> {
        approx_hessian_nalgebra_f64(self, f)
    }

    fn covariance(&self, f: &dyn Fn(&DVector<f64>) -> DVector<f64>) -> Option<DMatrix<f64>> {
        approx_covariance_matrix_nalgebra_f64(self, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xafs::tests::PARAM_LOADTXT;
    use crate::xafs::tests::TEST_TOL;
    use crate::xafs::tests::TOP_DIR;
    use approx::assert_abs_diff_eq;

    const NUM_DIFF_TOL: f64 = 1e-6;

    fn residuals(p: &DVector<f64>) -> DVector<f64> {
        let mut res = DVector::zeros(p.len());

        res.iter_mut()
            .enumerate()
            .for_each(|(i, x)| *x = p[i] - (i as f64 + 1.0));

        res
    }

    #[test]
    fn test_forward_jacobian_nalgebra_f64() {
        let x = DVector::from_vec(vec![1.0, 2.0, 3.0]);
        let fs = |x: &DVector<f64>| {
            let mut y = DVector::zeros(x.len());
            y[0] = 2.0 * x[0] + 3.0 * x[1] + 4.0 * x[2];
            y[1] = 3.0 * x[1] + 4.0 * x[2];
            y[2] = 4.0 * x[2] + 5.0 + x[1].exp();
            y
        };
        let jac = forward_jacobian_nalgebra_f64(&x, &fs);
        let jac_ref = DMatrix::from_vec(
            3,
            3,
            vec![2.0, 0.0, 0.0, 3.0, 3.0, (2.0 as f64).exp(), 4.0, 4.0, 4.0],
        );

        assert_abs_diff_eq!(jac, jac_ref, epsilon = NUM_DIFF_TOL);
    }

    #[test]
    fn test_center_jacobian_nalgebra_f64() {
        let x = DVector::from_vec(vec![1.0, 2.0, 3.0]);
        let fs = |x: &DVector<f64>| {
            let mut y = DVector::zeros(x.len());
            y[0] = 2.0 * x[0] + 3.0 * x[1] + 4.0 * x[2];
            y[1] = 3.0 * x[1] + 4.0 * x[2];
            y[2] = 4.0 * x[2] + 5.0 + x[1].exp();
            y
        };
        let jac = center_jacobian_nalgebra_f64(&x, &fs);
        let jac_ref = DMatrix::from_vec(
            3,
            3,
            vec![2.0, 0.0, 0.0, 3.0, 3.0, (2.0 as f64).exp(), 4.0, 4.0, 4.0],
        );

        assert_abs_diff_eq!(jac, jac_ref, epsilon = NUM_DIFF_TOL);
    }
}
