use enterpolation::{
    linear::{Linear, LinearError},
    Generator,
};
use errorfunctions::ComplexErrorFunctions;
use nalgebra::DMatrix;
use ndarray::{Array1, ArrayBase, Ix1, OwnedRepr};
use num_complex::Complex64;
use std::error::Error;

#[deny(clippy::reversed_empty_ranges)]

pub trait MathUtils {
    fn interpolate(&self, x: &Vec<f64>, y: &Vec<f64>) -> Result<Self, LinearError>
    where
        Self: Sized;

    fn is_sorted(&self) -> bool;

    fn argsort(&self) -> Vec<usize>;

    fn gaussian(self, center: f64, sigma: f64) -> Array1<f64>
    where
        Self: Into<Array1<f64>>,
    {
        let x: Array1<f64> = self.into() - center;
        let sigma = sigma.max(f64::EPSILON);
        let inverse_of_coefficient = sigma * (2.0 * std::f64::consts::PI).sqrt();
        x.map(|x| (-x.powi(2) / (2.0 * sigma.powi(2))).exp() / inverse_of_coefficient)
    }

    fn lorentzian(self, center: f64, sigma: f64) -> Array1<f64>
    where
        Self: Into<Array1<f64>>,
    {
        let x: Array1<f64> = self.into() - center;
        let sigma = sigma.max(f64::EPSILON);
        let coefficient = sigma / std::f64::consts::PI;
        x.map(|x| coefficient / (x.powi(2) + sigma.powi(2)))
    }

    fn voigt(self, center: f64, sigma: f64, gamma: f64) -> Array1<f64>
    where
        Self: Into<Array1<f64>>,
    {
        let x: Array1<f64> = self.into() - center;
        let sigma = sigma.max(f64::EPSILON);
        let gamma = gamma.max(f64::EPSILON);
        let inverse_of_coefficient = sigma * (2.0 * std::f64::consts::PI).sqrt();

        x.iter()
            .map(|x| {
                let z = Complex64::new(*x, gamma) / sigma / (2.0 as f64).sqrt();
                z.w().re / inverse_of_coefficient
            })
            .collect()
    }

    fn min(&self) -> f64;
    fn max(&self) -> f64;
    fn diff(&self) -> Self;
    fn gradient(&self) -> Self;
    fn ptp(&self) -> f64
    where
        Self: IntoIterator<Item = f64> + Sized,
    {
        self.max() - self.min()
    }

    fn argmin(&self) -> usize
    where
        Self: IntoIterator<Item = f64> + Sized + Clone,
    {
        self.clone()
            .into_iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .unwrap()
            .0
    }

    fn argmax(&self) -> usize
    where
        Self: IntoIterator<Item = f64> + Sized + Clone,
    {
        self.clone()
            .into_iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .unwrap()
            .0
    }

    fn abs_argmin(&self) -> usize
    where
        Self: IntoIterator<Item = f64> + Sized + Clone,
    {
        self.clone()
            .into_iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| a.abs().partial_cmp(&b.abs()).unwrap())
            .unwrap()
            .0
    }
}

impl MathUtils for Vec<f64> {
    fn interpolate(&self, x: &Vec<f64>, y: &Vec<f64>) -> Result<Self, LinearError> {
        let x_left = x.min();
        let x_right = x.max();
        let lin = Linear::builder().elements(y).knots(x).build()?;
        let result: Vec<f64> = lin
            .sample(self.iter().map(|a| match a {
                a if a > &x_right => x_right,
                a if a < &x_left => x_left,
                _ => *a,
            }))
            .collect();
        Ok(result)
    }

    fn is_sorted(&self) -> bool {
        is_sorted(self)
    }

    fn argsort(&self) -> Vec<usize> {
        argsort(self)
    }

    fn min(&self) -> f64 {
        self.iter()
            .min_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap()
            .clone()
    }

    fn max(&self) -> f64 {
        self.iter()
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap()
            .clone()
    }

    fn diff(&self) -> Self {
        let mut result = Vec::with_capacity(self.len() - 1);
        for i in 0..self.len() - 1 {
            result.push(self[i + 1] - self[i]);
        }
        result
    }

    /// Calculate the central difference gradient of the vector
    ///
    /// # Example
    /// ```
    /// use xraytsubaki::xafs::mathutils::MathUtils;
    /// let v = vec![1., 2., 4., 7., 11., 16.];
    /// assert_eq!(v.gradient(), vec![1. , 1.5, 2.5, 3.5, 4.5, 5. ]);
    ///
    /// let v = vec![0.];
    /// assert_eq!(v.gradient(), vec![0.]);
    ///
    /// let v = vec![1., 2.];
    /// assert_eq!(v.gradient(), vec![1., 1.]);
    ///
    /// ```
    fn gradient(&self) -> Self {
        let mut result = Vec::with_capacity(self.len());

        match self.len() {
            0..=1 => vec![0.; self.len()],
            2 => vec![self[1] - self[0], self[1] - self[0]],
            _ => {
                result.push(self[1] - self[0]);
                for i in 1..self.len() - 1 {
                    result.push((self[i + 1] - self[i - 1]) / 2.0);
                }
                result.push(self[self.len() - 1] - self[self.len() - 2]);
                result
            }
        }
    }
}

impl MathUtils for ArrayBase<OwnedRepr<f64>, Ix1> {
    fn interpolate(&self, x: &Vec<f64>, y: &Vec<f64>) -> Result<Self, LinearError> {
        let x_left = x.min();
        let x_right = x.max();
        let lin = Linear::builder().elements(y).knots(x).build()?;
        let result: Vec<f64> = lin
            .sample(self.map(|a| match a {
                a if a > &x_right => x_right,
                a if a < &x_left => x_left,
                _ => *a,
            }))
            .collect();

        Ok(result.into())
    }

    fn is_sorted(&self) -> bool {
        is_sorted(self.to_vec())
    }

    fn argsort(&self) -> Vec<usize> {
        argsort(&self.to_vec())
    }

    fn min(&self) -> f64 {
        self.iter()
            .min_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap()
            .clone()
    }

    fn max(&self) -> f64 {
        self.iter()
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap()
            .clone()
    }

    fn diff(&self) -> Self {
        &self.slice(ndarray::s![1..]) - &self.slice(ndarray::s![..-1])
    }

    fn gaussian(self, center: f64, sigma: f64) -> Array1<f64> {
        let x: Array1<f64> = self - center;
        let sigma = sigma.max(f64::EPSILON);
        let inverse_of_coefficient = sigma * (2.0 * std::f64::consts::PI).sqrt();
        x.map(|x| (-x.powi(2) / (2.0 * sigma.powi(2))).exp() / inverse_of_coefficient)
    }

    fn lorentzian(self, center: f64, sigma: f64) -> Array1<f64> {
        let x: Array1<f64> = self - center;
        let sigma = sigma.max(f64::EPSILON);
        let coefficient = sigma / std::f64::consts::PI;
        x.map(|x| coefficient / (x.powi(2) + sigma.powi(2)))
    }

    fn voigt(self, center: f64, sigma: f64, gamma: f64) -> Array1<f64> {
        let x: Array1<f64> = self - center;
        let sigma = sigma.max(f64::EPSILON);
        let gamma = gamma.max(f64::EPSILON);
        let inverse_of_coefficient = sigma * (2.0 * std::f64::consts::PI).sqrt();

        x.iter()
            .map(|x| {
                let z = Complex64::new(*x, gamma) / sigma / (2.0 as f64).sqrt();
                z.w().re / inverse_of_coefficient
            })
            .collect()
    }
    /// Calculate the central difference gradient of the Array
    ///
    /// # Example
    /// ```
    /// use xraytsubaki::xafs::mathutils::MathUtils;
    /// use ndarray::Array1;
    ///
    /// let v = Array1::from_vec(vec![1., 2., 4., 7., 11., 16.]);
    /// assert_eq!(v.gradient(), Array1::from_vec(vec![1. , 1.5, 2.5, 3.5, 4.5, 5. ]));
    ///
    /// let v = Array1::from_vec(vec![0.]);
    /// assert_eq!(v.gradient(), Array1::from_vec(vec![0.]));
    ///
    /// let v = Array1::from_vec(vec![1., 2.]);
    /// assert_eq!(v.gradient(), Array1::from_vec(vec![1., 1.]));
    ///
    /// ```
    #[allow(clippy::reversed_empty_ranges)]
    fn gradient(&self) -> Self {
        match self.len() {
            0..=1 => Array1::zeros(self.len()),
            2 => Array1::from_vec(vec![self[1] - self[0], self[1] - self[0]]),
            _ => {
                let mut result = Array1::zeros(self.len());

                result
                    .slice_mut(ndarray::s![0])
                    .assign(&(&self.slice(ndarray::s![1]) - &self.slice(ndarray::s![0])));
                result.slice_mut(ndarray::s![1..-1]).assign(
                    &((&self.slice(ndarray::s![2..]) - &self.slice(ndarray::s![..-2])) / 2.0),
                );
                result
                    .slice_mut(ndarray::s![-1])
                    .assign(&(&self.slice(ndarray::s![-1]) - &self.slice(ndarray::s![-2])));
                result
            }
        }
    }
}

fn is_sorted<I>(data: I) -> bool
where
    I: IntoIterator,
    I::Item: PartialOrd,
{
    let mut it = data.into_iter();
    match it.next() {
        None => true,
        Some(first) => it
            .scan(first, |state, next| {
                let cmp = *state <= next;
                *state = next;
                Some(cmp)
            })
            .all(|b| b),
    }
}

fn argsort<T: PartialOrd>(v: &[T]) -> Vec<usize> {
    let mut idx = (0..v.len()).collect::<Vec<_>>();
    idx.sort_by(|a, b| v[*a].partial_cmp(&v[*b]).unwrap());
    idx
}

fn gaussian<T: Into<Array1<f64>>>(x: T, center: f64, sigma: f64) -> Array1<f64> {
    let x: Array1<f64> = x.into() - center;
    let sigma = sigma.max(f64::EPSILON);
    let inverse_of_coefficient = sigma * (2.0 * std::f64::consts::PI).sqrt();
    x.map(|x| (-x.powi(2) / (2.0 * sigma.powi(2))).exp() / inverse_of_coefficient)
}

fn lorentzian<T: Into<Array1<f64>>>(x: T, center: f64, sigma: f64) -> Array1<f64> {
    let x: Array1<f64> = x.into() - center;
    let sigma = sigma.max(f64::EPSILON);
    let coefficient = sigma / std::f64::consts::PI;
    x.map(|x| coefficient / (x.powi(2) + sigma.powi(2)))
}

fn voigt<T: Into<Array1<f64>>>(x: T, center: f64, sigma: f64, gamma: f64) -> Array1<f64> {
    let x: Array1<f64> = x.into() - center;
    let sigma = sigma.max(f64::EPSILON);
    let gamma = gamma.max(f64::EPSILON);
    let inverse_of_coefficient = sigma * (2.0 * std::f64::consts::PI).sqrt();

    x.iter()
        .map(|x| {
            let z = Complex64::new(*x, gamma) / sigma / (2.0 as f64).sqrt();
            z.w().re / inverse_of_coefficient
        })
        .collect()
}
/// Find the index of array *at or below* the value
/// returns 0 if the value is below the minimum of the array
///
/// # Arguments
/// * `array` - The array to search
/// * `value` - The value to search for
///
/// # Returns
/// Result<usize, Box<dyn Error>>
///
/// # Example
/// ```
/// use xraytsubaki::xafs::mathutils::index_of;
/// let array = vec![1.0, 2.0, 3.0, 4.0, 5.0];
/// let value = 3.4;
/// assert_eq!(index_of(&array, &value).unwrap(), 2);
/// ```

pub fn index_of(array: &Vec<f64>, value: &f64) -> Result<usize, Box<dyn Error>> {
    if &array.min() > value {
        return Ok(0);
    }

    Ok(array
        .iter()
        .enumerate()
        .find_map(|(i, x)| if x > value { Some(i - 1) } else { None })
        .unwrap_or(array.len() - 1))
}

/// Find the index of the nearest value in the array
///
/// # Arguments
/// * `array` - The array to search
/// * `value` - The value to search for
///
/// # Example
/// ```
/// use xraytsubaki::xafs::mathutils::index_nearest;
/// let array = vec![1.0, 2.0, 3.0, 4.0, 5.0];
/// let value = 3.4;
/// assert_eq!(index_nearest(&array, &value).unwrap(), 2);
/// ```
pub fn index_nearest(array: &[f64], value: &f64) -> Result<usize, Box<dyn Error>> {
    Ok(array
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| (*a - value).abs().partial_cmp(&(*b - value).abs()).unwrap())
        .unwrap()
        .0)
}

#[allow(non_snake_case)]
pub fn bessel_I0(x: f64) -> f64 {
    let base = x * x / 4.0;
    let mut addend = 1.0;
    let mut sum = 1.0;
    for j in 1.. {
        addend = addend * base / (j * j) as f64;
        let old = sum;
        sum += addend;
        if sum == old || !sum.is_finite() {
            break;
        }
    }
    sum
}

/// Calculation jacobian of splev respect to c_i
///
///
pub fn splev_jacobian(t: Vec<f64>, c: Vec<f64>, k: usize, x: Vec<f64>, e: usize) -> DMatrix<f64> {
    let k1: usize = k + 1;
    let k2: usize = k1 + 1;
    let nk1: usize = t.len() - k1;
    let tb: f64 = t[k1 - 1];
    let te: f64 = t[nk1];

    let mut derivatives: Vec<Vec<f64>> = vec![vec![0.0; c.len()]; x.len()];

    for (i, &arg) in x.iter().enumerate() {
        let mut arg = arg;
        if arg < tb && e == 3 {
            arg = tb;
        } else if arg > te && e == 3 {
            arg = te;
        }

        let mut l = k1;
        let mut l1 = l + 1;
        while arg < t[l - 1] && l1 != k2 {
            l1 = l;
            l -= 1;
        }
        while arg >= t[l1 - 1] && l != nk1 {
            l = l1;
            l1 += 1;
        }

        let h = rusty_fitpack::fpbspl::fpbspl(arg, &t, k, l);

        let mut ll = l - k1;
        for j in 1..=k1 {
            ll += 1;
            if ll - 1 < c.len() {
                derivatives[i][ll - 1] = h[j - 1];
            }
        }
    }

    DMatrix::from_vec(
        derivatives[0].len(),
        derivatives.len(),
        derivatives.concat(),
    )
    .transpose()
}

#[cfg(test)]
mod tests {
    use std::vec;

    use super::*;
    use crate::xafs::tests::PARAM_LOADTXT;
    use crate::xafs::tests::TEST_TOL;
    use crate::xafs::tests::TOP_DIR;
    use approx::assert_abs_diff_eq;
    use nalgebra::DVector;

    const NUMERICAL_TEST_TOL: f64 = 1e-6;

    #[test]
    fn test_argsort_float() {
        let v = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        assert_eq!(v.argsort(), vec![0, 1, 2, 3, 4]);
        let v = vec![1.0, 2.0, 3.0, 4.0, 5.0, 3.0];
        assert_eq!(v.argsort(), vec![0, 1, 2, 5, 3, 4]);

        let v = Array1::from(vec![1.0, 2.0, 3.0, 4.0, 5.0]);
        assert_eq!(v.argsort(), vec![0, 1, 2, 3, 4]);
        let v = Array1::from(vec![1.0, 2.0, 3.0, 4.0, 5.0, 3.0]);
        assert_eq!(v.argsort(), vec![0, 1, 2, 5, 3, 4]);
    }

    #[test]
    fn test_interpolation() {
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let y = vec![0.0, 2.0, 2.5, 4.0, 5.0];
        let z = vec![10.0, 20.0, 30.0, 40.0, 50.0];

        let expected = [15.0, 20.0, 33.333333333333336, 40.0, 50.0];
        x.interpolate(&y, &z)
            .unwrap()
            .iter()
            .zip(expected.iter())
            .for_each(|(a, b)| {
                assert_abs_diff_eq!(a, b, epsilon = TEST_TOL);
            });
    }

    #[test]
    fn test_gaussian() {
        let x = Array1::from_vec(vec![0., 1.0, 2.0, 3.0, 4.0, 5.0, 6., 7., 8., 9.]);
        let y = gaussian(x.clone(), 5.0, 1.0);
        let expected = Array1::from_vec(vec![
            1.4867195147342979e-6,
            0.00013383022576488537,
            0.0044318484119380075,
            0.05399096651318806,
            0.24197072451914337,
            0.3989422804014327,
            0.24197072451914337,
            0.05399096651318806,
            0.0044318484119380075,
            0.00013383022576488537,
        ]);

        let _ = y.iter().zip(expected.iter()).map(|(a, b)| {
            assert_abs_diff_eq!(a, b, epsilon = TEST_TOL);
        });

        // assert_eq!(
        //     y,
        //     Array1::from(vec![
        //         1.4867195147342979e-6,
        //         0.00013383022576488537,
        //         0.0044318484119380075,
        //         0.05399096651318806,
        //         0.24197072451914337,
        //         0.3989422804014327,
        //         0.24197072451914337,
        //         0.05399096651318806,
        //         0.0044318484119380075,
        //         0.00013383022576488537
        //     ])
        // );

        let y = x.clone().gaussian(5.0, 1.0).to_vec();
        let expected = [
            1.4867195147342979e-6,
            0.00013383022576488537,
            0.0044318484119380075,
            0.05399096651318806,
            0.24197072451914337,
            0.3989422804014327,
            0.24197072451914337,
            0.05399096651318806,
            0.0044318484119380075,
            0.00013383022576488537,
        ];

        let _ = y.iter().zip(expected.iter()).map(|(a, b)| {
            assert_abs_diff_eq!(a, b, epsilon = TEST_TOL);
        });
    }

    #[test]
    #[allow(clippy::approx_constant)]
    fn test_lorentzian() {
        let x = Array1::from_vec(vec![0., 1.0, 2.0, 3.0, 4.0, 5.0, 6., 7., 8., 9.]);
        let y = lorentzian(x.clone(), 5.0, 1.0);

        let expected = Array1::from_vec(vec![
            0.012242687930145796,
            0.01872411095198769,
            0.03183098861837907,
            0.06366197723675814,
            0.15915494309189535,
            0.3183098861837907,
            0.15915494309189535,
            0.06366197723675814,
            0.03183098861837907,
            0.01872411095198769,
        ]);

        let _ = y.iter().zip(expected.iter()).map(|(a, b)| {
            assert_abs_diff_eq!(a, b, epsilon = TEST_TOL);
        });

        let y = x.clone().lorentzian(5.0, 1.0).to_vec();

        let expected = [
            0.012242687930145796,
            0.01872411095198769,
            0.03183098861837907,
            0.06366197723675814,
            0.15915494309189535,
            0.3183098861837907,
            0.15915494309189535,
            0.06366197723675814,
            0.03183098861837907,
            0.01872411095198769,
        ];

        y.iter().zip(expected.iter()).for_each(|(a, b)| {
            assert_abs_diff_eq!(a, b, epsilon = TEST_TOL);
        });
    }

    #[test]
    fn test_voigt() {
        let x = Array1::from_vec(vec![0., 1.0, 2.0, 3.0, 4.0, 5.0, 6., 7., 8., 9.]);
        let y = voigt(x.clone(), 5.0, 1.0, 1.0);

        let expected = Array1::from_vec(vec![
            0.013884921288571273,
            0.022813635258707103,
            0.04338582232367969,
            0.09071519942627546,
            0.16579566268916654,
            0.20870928052036772,
            0.16579566268916654,
            0.09071519942627546,
            0.04338582232367969,
            0.022813635258707103,
        ]);

        y.iter().zip(expected.iter()).for_each(|(a, b)| {
            assert_abs_diff_eq!(a, b, epsilon = TEST_TOL);
        });

        let y = x.clone().voigt(5.0, 1.0, 1.0).to_vec();

        let expected = [
            0.013884921288571273,
            0.022813635258707103,
            0.04338582232367969,
            0.09071519942627546,
            0.16579566268916654,
            0.20870928052036772,
            0.16579566268916654,
            0.09071519942627546,
            0.04338582232367969,
            0.022813635258707103,
        ];

        y.iter().zip(expected.iter()).for_each(|(a, b)| {
            assert_abs_diff_eq!(a, b, epsilon = TEST_TOL);
        });
    }

    #[test]
    fn test_min_vec() {
        let v = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        assert_eq!(v.min(), 1.0);
        let v = vec![1.0, 2.0, 3.0, 4.0, 5.0, 3.0, 0.0];
        assert_eq!(v.min(), 0.0);
    }

    #[test]
    fn test_min_array() {
        let v = Array1::from_vec(vec![1.0, 2.0, 3.0, 4.0, 5.0]);
        assert_eq!(v.min(), 1.0);
        let v = Array1::from_vec(vec![1.0, 2.0, 3.0, 4.0, 5.0, 3.0, 0.0]);
        assert_eq!(v.min(), 0.0);
    }

    #[test]
    fn test_max_vec() {
        let v = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        assert_eq!(v.max(), 5.0);
        let v = vec![1.0, 2.0, 3.0, 4.0, 5.0, 3.0, 0.0];
        assert_eq!(v.max(), 5.0);
    }
    #[test]
    fn test_max_array() {
        let v = Array1::from_vec(vec![1.0, 2.0, 3.0, 4.0, 5.0]);
        assert_eq!(v.max(), 5.0);
        let v = Array1::from_vec(vec![1.0, 2.0, 3.0, 4.0, 5.0, 3.0, 0.0]);
        assert_eq!(v.max(), 5.0);
    }

    #[test]
    fn test_diff_vec() {
        let v = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        assert_eq!(v.diff(), vec![1.0, 1.0, 1.0, 1.0]);
    }

    #[test]
    fn test_diff_array() {
        let v = Array1::from_vec(vec![1.0, 2.0, 3.0, 4.0, 5.0]);
        assert_eq!(v.diff(), Array1::from_vec(vec![1.0, 1.0, 1.0, 1.0]));
    }

    #[test]
    fn test_splev_jacobian() {
        let x = vec![
            0.0, 0.555, 1.111, 1.666, 2.222, 2.777, 3.333, 3.888, 4.444, 5.0,
        ];
        let y = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, -1.0, -2.0, -3.0, -4.0];
        let order = 3;

        let (t, c, k) = rusty_fitpack::splrep(
            x.clone(),
            y.clone(),
            None,
            None,
            None,
            Some(order),
            None,
            None,
            None,
            None,
            None,
            None,
        );

        let e = 3; // Example end condition

        let analytical_jacobian = splev_jacobian(t.clone(), c.clone(), k, x.clone(), e);

        use crate::xafs::lmutils::forward_jacobian_nalgebra_f64;

        // let spline = rusty_fitpack::splev(t.clone(), c.clone(), k, x.clone(), e);
        //
        // let spline = DVector::from(spline);

        let spline_function = |coef: &DVector<f64>| {
            DVector::from(rusty_fitpack::splev(
                t.clone(),
                coef.data.as_vec().clone(),
                k,
                x.clone(),
                e,
            ))
        };

        let numerical_jacobian = forward_jacobian_nalgebra_f64(&DVector::from(c), &spline_function);

        analytical_jacobian
            .iter()
            .zip(numerical_jacobian.iter())
            .for_each(|(a, b)| {
                assert_abs_diff_eq!(a, b, epsilon = NUMERICAL_TEST_TOL);
            });
    }
}
