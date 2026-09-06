#[cfg(test)]
mod clamp_chain_rule_diagnostic {
    use super::*;
    #[test]
    fn compare_clamp_jacobian_to_finite_differences() {
        let x: Vec<f64> = (0..9).map(|i| i as f64 * 1.5).collect();
        let y: Vec<f64> = x.iter().map(|v| 1.0 + 0.03 * v.cos()).collect();
        let (knots, coefs) = spline::interpolate(&x, &y, 3).unwrap();
        let k = DVector::from_iterator(241, (0..241).map(|i| i as f64 * 0.05));
        let problem = AUTOBKSpline {
            coefs: DVector::from_vec(coefs),
            knots: DVector::from_vec(knots),
            kraw: k.clone(),
            kout: k.clone(),
            mu: k.map(|v| 1.0 + 0.1 * (2.0 * v).sin()),
            ftwin: k.clone(),
            nclamp: 3,
            clamp_lo: 0,
            clamp_hi: 1,
            irbkg: 35,
            ..Default::default()
        };
        for fixed in [true, false] {
            let scale = fixed.then(|| problem.clamp_scale(&problem.coefs));
            let analytic = problem.residual_jacobian_with_scale(&problem.coefs, scale);
            let numeric = DMatrix::from_columns(
                &(0..problem.coefs.len())
                    .map(|j| {
                        let mut plus = problem.coefs.clone();
                        let mut minus = problem.coefs.clone();
                        plus[j] += 1.0e-6;
                        minus[j] -= 1.0e-6;
                        (problem.residual_vec_with_scale(&plus, scale)
                            - problem.residual_vec_with_scale(&minus, scale))
                            / 2.0e-6
                    })
                    .collect::<Vec<_>>(),
            );
            let head_error =
                (analytic.rows(0, 70) - numeric.rows(0, 70)).norm() / numeric.rows(0, 70).norm();
            let clamp_error =
                (analytic.rows(70, 6) - numeric.rows(70, 6)).norm() / numeric.rows(70, 6).norm();
            println!("fixed={fixed} fft_head_relative_error={head_error:.12e} clamp_relative_error={clamp_error:.12e}");
            assert!(head_error < 1.0e-7);
            if fixed {
                assert!(clamp_error < 1.0e-7);
            } else {
                assert!(clamp_error > 1.0e-3);
            }
        }
    }
}
