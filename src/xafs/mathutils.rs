use enterpolation::{
    linear::{Linear, LinearError},
    Generator,
};
use errorfunctions::ComplexErrorFunctions;
use ndarray::{Array1, ArrayBase, Ix1, OwnedRepr};
use num_complex::Complex64;
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
            .into()
    }

    fn lorentzian(self, center: f64, sigma: f64) -> Array1<f64>
    where
        Self: Into<Array1<f64>>,
    {
        let x: Array1<f64> = self.into() - center;
        let sigma = sigma.max(f64::EPSILON);
        let coefficient = sigma / std::f64::consts::PI;
        x.map(|x| coefficient / (x.powi(2) + sigma.powi(2))).into()
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
    /// ```
    fn gradient(&self) -> Self {
        let mut result = Vec::with_capacity(self.len());
        result.push(self[1] - self[0]);
        for i in 1..self.len() - 1 {
            result.push((self[i + 1] - self[i - 1]) / 2.0);
        }
        result.push(self[self.len() - 1] - self[self.len() - 2]);
        result
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
    /// ```
    fn gradient(&self) -> Self {
        let mut result = Array1::zeros(self.len());
        result
            .slice_mut(ndarray::s![0])
            .assign(&(&self.slice(ndarray::s![1]) - &self.slice(ndarray::s![0])));
        result
            .slice_mut(ndarray::s![1..-1])
            .assign(&((&self.slice(ndarray::s![2..]) - &self.slice(ndarray::s![..-2])) / 2.0));
        result
            .slice_mut(ndarray::s![-1])
            .assign(&(&self.slice(ndarray::s![-1]) - &self.slice(ndarray::s![-2])));
        result
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

// impl MathUtils for Array1<f64> {
//     fn interpolate(&self, x: &Vec<f64>, y: &Vec<f64>) -> Result<Self, LinearError> {
//         let x_left = x.min();
//         let x_right = x.max();
//         let lin = Linear::builder().elements(y).knots(x).build()?;
//         let result: Vec<f64> = lin
//             .sample(self.map(|a| match a {
//                 a if a > &x_right => x_right,
//                 a if a < &x_left => x_left,
//                 _ => *a,
//             }))
//             .collect();

//         Ok(result.into())
//     }

//     fn is_sorted(&self) -> bool {
//         is_sorted(self.to_vec())
//     }

//     fn argsort(&self) -> Vec<usize> {
//         argsort(&self.to_vec())
//     }

//     fn min(&self) -> f64 {
//         self.iter()
//             .min_by(|a, b| a.partial_cmp(b).unwrap())
//             .unwrap()
//             .clone()
//     }

//     fn max(&self) -> f64 {
//         self.iter()
//             .max_by(|a, b| a.partial_cmp(b).unwrap())
//             .unwrap()
//             .clone()
//     }

//     fn diff(&self) -> Self {
//         &self.slice(ndarray::s![1..]) - &self.slice(ndarray::s![..-1])
//     }

//     fn gaussian(self, center: f64, sigma: f64) -> Array1<f64> {
//         let x: Array1<f64> = self - center;
//         let sigma = sigma.max(f64::EPSILON);
//         let inverse_of_coefficient = sigma * (2.0 * std::f64::consts::PI).sqrt();
//         x.map(|x| (-x.powi(2) / (2.0 * sigma.powi(2))).exp() / inverse_of_coefficient)
//     }

//     fn lorentzian(self, center: f64, sigma: f64) -> Array1<f64> {
//         let x: Array1<f64> = self - center;
//         let sigma = sigma.max(f64::EPSILON);
//         let coefficient = sigma / std::f64::consts::PI;
//         x.map(|x| coefficient / (x.powi(2) + sigma.powi(2)))
//     }

//     fn voigt(self, center: f64, sigma: f64, gamma: f64) -> Array1<f64> {
//         let x: Array1<f64> = self - center;
//         let sigma = sigma.max(f64::EPSILON);
//         let gamma = gamma.max(f64::EPSILON);
//         let inverse_of_coefficient = sigma * (2.0 * std::f64::consts::PI).sqrt();

//         x.iter()
//             .map(|x| {
//                 let z = Complex64::new(*x, gamma) / sigma / (2.0 as f64).sqrt();
//                 z.w().re / inverse_of_coefficient
//             })
//             .collect()
//     }

//     /// Calculate the central difference gradient of the Array
//     ///
//     /// # Example
//     /// ```
//     /// use xraytsubaki::xafs::mathutils::MathUtils;
//     /// use ndarray::Array1;
//     ///
//     /// let v = Array1::from_vec(vec![1., 2., 4., 7., 11., 16.]);
//     /// assert_eq!(v.gradient(), Array1::from_vec(vec![1. , 1.5, 2.5, 3.5, 4.5, 5. ]));
//     /// ```
//     fn gradient(&self) -> Self {
//         let mut result = Array1::zeros(self.len());
//         result
//             .slice_mut(ndarray::s![0])
//             .assign(&(&self.slice(ndarray::s![1]) - &self.slice(ndarray::s![0])));
//         result
//             .slice_mut(ndarray::s![1..-1])
//             .assign(&((&self.slice(ndarray::s![2..]) - &self.slice(ndarray::s![..-2])) / 2.0));
//         result
//             .slice_mut(ndarray::s![-1])
//             .assign(&(&self.slice(ndarray::s![-1]) - &self.slice(ndarray::s![-2])));
//         result
//     }
// }

#[cfg(test)]
mod tests {
    use std::vec;

    use super::*;

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
        assert_eq!(
            x.interpolate(&y, &z).unwrap(),
            vec![15.0, 20.0, 33.333333333333336, 40.0, 50.0]
        );
    }

    #[test]
    fn test_gaussian() {
        let x = Array1::from_vec(vec![0., 1.0, 2.0, 3.0, 4.0, 5.0, 6., 7., 8., 9.]);
        let y = gaussian(x.clone(), 5.0, 1.0);
        assert_eq!(
            y,
            Array1::from(vec![
                1.4867195147342979e-6,
                0.00013383022576488537,
                0.0044318484119380075,
                0.05399096651318806,
                0.24197072451914337,
                0.3989422804014327,
                0.24197072451914337,
                0.05399096651318806,
                0.0044318484119380075,
                0.00013383022576488537
            ])
        );

        let y = x.clone().gaussian(5.0, 1.0).to_vec();
        assert_eq!(
            y,
            vec![
                1.4867195147342979e-6,
                0.00013383022576488537,
                0.0044318484119380075,
                0.05399096651318806,
                0.24197072451914337,
                0.3989422804014327,
                0.24197072451914337,
                0.05399096651318806,
                0.0044318484119380075,
                0.00013383022576488537
            ]
        );
    }

    #[test]
    fn test_lorentzian() {
        let x = Array1::from_vec(vec![0., 1.0, 2.0, 3.0, 4.0, 5.0, 6., 7., 8., 9.]);
        let y = lorentzian(x.clone(), 5.0, 1.0);
        assert_eq!(
            y,
            Array1::from_vec(vec![
                0.012242687930145796,
                0.01872411095198769,
                0.03183098861837907,
                0.06366197723675814,
                0.15915494309189535,
                0.3183098861837907,
                0.15915494309189535,
                0.06366197723675814,
                0.03183098861837907,
                0.01872411095198769
            ])
        );

        let y = x.clone().lorentzian(5.0, 1.0).to_vec();

        assert_eq!(
            y,
            vec![
                0.012242687930145796,
                0.01872411095198769,
                0.03183098861837907,
                0.06366197723675814,
                0.15915494309189535,
                0.3183098861837907,
                0.15915494309189535,
                0.06366197723675814,
                0.03183098861837907,
                0.01872411095198769
            ]
        );
    }

    #[test]
    fn test_voigt() {
        let x = Array1::from_vec(vec![0., 1.0, 2.0, 3.0, 4.0, 5.0, 6., 7., 8., 9.]);
        let y = voigt(x.clone(), 5.0, 1.0, 1.0);
        assert_eq!(
            y,
            Array1::from_vec(vec![
                0.013884921288571273,
                0.022813635258707103,
                0.04338582232367969,
                0.09071519942627546,
                0.16579566268916654,
                0.20870928052036772,
                0.16579566268916654,
                0.09071519942627546,
                0.04338582232367969,
                0.022813635258707103
            ])
        );

        let y = x.clone().voigt(5.0, 1.0, 1.0).to_vec();

        assert_eq!(
            y,
            vec![
                0.013884921288571273,
                0.022813635258707103,
                0.04338582232367969,
                0.09071519942627546,
                0.16579566268916654,
                0.20870928052036772,
                0.16579566268916654,
                0.09071519942627546,
                0.04338582232367969,
                0.022813635258707103
            ]
        )
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
}
