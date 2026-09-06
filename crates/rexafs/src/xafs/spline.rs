//! Interpolating B-splines used by AUTOBK and FEFF path resampling.
//!
//! Coefficients are solved directly from the B-spline collocation matrix using
//! QR factorization. Evaluation and derivatives share the same basis. AUTOBK
//! clamps to the boundary; FEFF interpolation extends the end polynomial pieces.
//! See <https://docs.scipy.org/doc/scipy/reference/generated/scipy.interpolate.BSpline.html>.

use nalgebra::{DMatrix, DVector};

/// Fit an interpolating spline and retain the historical zero-padded coefficient
/// layout used by the AUTOBK solvers. Cubic interpolation is not-a-knot.
pub(crate) fn interpolate(
    x: &[f64],
    y: &[f64],
    degree: usize,
) -> Result<(Vec<f64>, Vec<f64>), String> {
    if !matches!(degree, 1 | 3) || x.len() != y.len() || x.len() <= degree {
        return Err(format!(
            "spline degree {degree} requires matching arrays with more than {degree} points"
        ));
    }
    if x.iter().chain(y).any(|value| !value.is_finite())
        || x.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err("spline inputs must be finite with strictly increasing coordinates".into());
    }

    let (knots, mut coefficients) = if degree == 1 {
        // A linear interpolant has the measured values as its B-spline
        // coefficients. This also handles two- and three-point FEFF grids.
        let mut knots = Vec::with_capacity(x.len() + 2);
        knots.push(x[0]);
        knots.extend_from_slice(x);
        knots.push(x[x.len() - 1]);
        (knots, y.to_vec())
    } else {
        // Repeated endpoints and omission of the second/penultimate data
        // points give the cubic not-a-knot interpolant used by the pipeline.
        let mut knots = Vec::with_capacity(x.len() + degree + 1);
        knots.extend(std::iter::repeat_n(x[0], degree + 1));
        knots.extend_from_slice(&x[2..x.len() - 2]);
        knots.extend(std::iter::repeat_n(x[x.len() - 1], degree + 1));
        let matrix = coefficient_jacobian(&knots, x.len(), degree, x, false);
        let coefficients = matrix
            .qr()
            .solve(&DVector::from_column_slice(y))
            .ok_or("spline interpolation matrix is singular")?;
        (knots, coefficients.as_slice().to_vec())
    };
    coefficients.resize(knots.len(), 0.0);
    if coefficients.iter().any(|value| !value.is_finite()) {
        return Err("spline interpolation produced non-finite coefficients".into());
    }
    Ok((knots, coefficients))
}

/// Indices and values of the degree+1 nonzero basis functions at one point.
/// The span is kept inside the base interval for polynomial extrapolation.
fn basis(knots: &[f64], degree: usize, x: f64, clamp: bool) -> (usize, [f64; 6]) {
    assert!(degree <= 5 && knots.len() >= 2 * (degree + 1));
    let count = knots.len() - degree - 1;
    let x = if clamp {
        x.clamp(knots[degree], knots[count])
    } else {
        x
    };
    let span = knots
        .partition_point(|&knot| knot <= x)
        .saturating_sub(1)
        .clamp(degree, count - 1);
    let mut weights = [0.0; 6];
    let mut left = [0.0; 6];
    let mut right = [0.0; 6];
    weights[0] = 1.0;
    for order in 1..=degree {
        left[order] = x - knots[span + 1 - order];
        right[order] = knots[span + order] - x;
        let mut carried = 0.0;
        for index in 0..order {
            let width = right[index + 1] + left[order - index];
            let term = if width == 0.0 {
                0.0
            } else {
                weights[index] / width
            };
            weights[index] = carried + right[index + 1] * term;
            carried = left[order - index] * term;
        }
        weights[order] = carried;
    }
    (span - degree, weights)
}

pub(crate) fn evaluate(
    knots: &[f64],
    coefficients: &[f64],
    degree: usize,
    x: &[f64],
    clamp: bool,
) -> Vec<f64> {
    x.iter()
        .map(|&point| {
            let (start, weights) = basis(knots, degree, point, clamp);
            weights[..=degree]
                .iter()
                .enumerate()
                .map(|(index, weight)| {
                    weight * coefficients.get(start + index).copied().unwrap_or(0.0)
                })
                .sum()
        })
        .collect()
}

pub(crate) fn coefficient_jacobian(
    knots: &[f64],
    coefficient_count: usize,
    degree: usize,
    x: &[f64],
    clamp: bool,
) -> DMatrix<f64> {
    let mut matrix = DMatrix::zeros(x.len(), coefficient_count);
    for (row, &point) in x.iter().enumerate() {
        let (start, weights) = basis(knots, degree, point, clamp);
        for (offset, &weight) in weights[..=degree].iter().enumerate() {
            if start + offset < coefficient_count {
                matrix[(row, start + offset)] = weight;
            }
        }
    }
    matrix
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;

    #[test]
    fn cubic_polynomial_is_reproduced_including_extrapolation() {
        let x: Vec<f64> = vec![-2.0, -1.2, 0.0, 0.4, 2.1, 3.0, 7.0];
        let polynomial = |x: f64| 0.2 * x.powi(3) - 2.0 * x + 1.0;
        let y: Vec<_> = x.iter().map(|&x| polynomial(x)).collect();
        let (knots, coefficients) = interpolate(&x, &y, 3).unwrap();
        let query = [8.0, -3.0, 0.5, 7.0, -2.0, 2.1];
        for (&x, actual) in query
            .iter()
            .zip(evaluate(&knots, &coefficients, 3, &query, false))
        {
            assert_abs_diff_eq!(actual, polynomial(x), epsilon = 1e-11);
        }
        let clamped = evaluate(&knots, &coefficients, 3, &query[..2], true);
        assert_abs_diff_eq!(clamped[0], polynomial(7.0), epsilon = 1e-11);
        assert_abs_diff_eq!(clamped[1], polynomial(-2.0), epsilon = 1e-11);
    }

    #[test]
    fn linear_short_grids_and_empty_queries() {
        for x in [vec![1.0, 4.0], vec![1.0, 2.0, 4.0]] {
            let y: Vec<_> = x.iter().map(|x| 3.0 * x - 2.0).collect();
            let (knots, coefficients) = interpolate(&x, &y, 1).unwrap();
            let query = [0.0, 1.0, 2.5, 4.0, 5.0];
            for (actual, expected) in evaluate(&knots, &coefficients, 1, &query, false)
                .into_iter()
                .zip([-2.0, 1.0, 5.5, 10.0, 13.0])
            {
                assert_abs_diff_eq!(actual, expected, epsilon = 4e-15);
            }
            assert!(evaluate(&knots, &coefficients, 1, &[], true).is_empty());
            assert_eq!(
                coefficient_jacobian(&knots, coefficients.len(), 1, &[], true).nrows(),
                0
            );
        }
    }

    #[test]
    fn invalid_interpolation_data_returns_errors() {
        for (x, y, degree) in [
            (vec![], vec![], 3),
            (vec![0.0, 1.0], vec![1.0], 1),
            (vec![1.0, 1.0], vec![0.0, 1.0], 1),
            (vec![1.0, 0.0], vec![0.0, 1.0], 1),
            (vec![0.0, f64::NAN], vec![0.0, 1.0], 1),
            (vec![0.0, 1.0], vec![0.0, f64::INFINITY], 1),
            (vec![0.0, 1.0, 2.0], vec![0.0, 1.0, 2.0], 3),
        ] {
            assert!(interpolate(&x, &y, degree).is_err());
        }
    }

    #[test]
    fn interpolant_and_coefficient_derivatives_match_scipy() {
        #[derive(serde::Deserialize)]
        struct Reference {
            x: Vec<f64>,
            y: Vec<f64>,
            query: Vec<f64>,
            knots: Vec<f64>,
            coefficients: Vec<f64>,
            extrapolated: Vec<f64>,
            clamped: Vec<f64>,
            extrapolated_basis: Vec<Vec<f64>>,
            clamped_basis: Vec<Vec<f64>>,
        }
        let data: Reference = serde_json::from_str(include_str!(
            "../../tests/testfiles/spline_scipy_reference.json"
        ))
        .unwrap();
        let (knots, coefficients) = interpolate(&data.x, &data.y, 3).unwrap();
        assert_eq!(knots, data.knots);
        for (&actual, &expected) in coefficients.iter().zip(&data.coefficients) {
            assert_abs_diff_eq!(actual, expected, epsilon = 2e-12);
        }
        for (clamp, expected, expected_basis) in [
            (false, &data.extrapolated, &data.extrapolated_basis),
            (true, &data.clamped, &data.clamped_basis),
        ] {
            for (actual, &expected) in evaluate(&knots, &coefficients, 3, &data.query, clamp)
                .iter()
                .zip(expected)
            {
                assert_abs_diff_eq!(*actual, expected, epsilon = 2e-11);
            }
            let jacobian = coefficient_jacobian(&knots, coefficients.len(), 3, &data.query, clamp);
            for row in 0..jacobian.nrows() {
                for col in 0..jacobian.ncols() {
                    assert_abs_diff_eq!(
                        jacobian[(row, col)],
                        expected_basis[row][col],
                        epsilon = 2e-12
                    );
                }
                assert_abs_diff_eq!(jacobian.row(row).sum(), 1.0, epsilon = 2e-12);
            }
        }
    }
}
