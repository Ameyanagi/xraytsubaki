use enterpolation::{linear::Linear, Generator};
use errorfunctions::ComplexErrorFunctions;
use ndarray::Array1;
use num_complex::Complex64;

pub enum ConvolveForm {
    Lorentzian,
    Gaussian,
    Voigt,
}

pub trait MathUtils {
    fn interpolation<T: Into<Vec<f64>>, U: Into<Vec<f64>>>(&self, x: &T, y: &U) -> Self;
    fn gaussian(&self, center: f64, sigma: f64) -> Self
    where
        Self: Into<Array1<f64>>,
    {
        let x: Array1<f64> = self.into() - center;
        let sigma = sigma.max(f64::EPSILON);
        let inverse_of_coefficient = sigma * (2.0 * std::f64::consts::PI).sqrt();
        x.map(|x| (-x.powi(2) / (2.0 * sigma.powi(2))).exp() / inverse_of_coefficient)
            .into()
    }
    fn lorentzian(&self, center: f64, sigma: f64) -> Self
    where
        Self: Into<Array1<f64>>,
    {
        let x: Array1<f64> = self.into() - center;
        let sigma = sigma.max(f64::EPSILON);
        let coefficient = sigma / std::f64::consts::PI;
        x.map(|x| coefficient / (x.powi(2) + sigma.powi(2))).into()
    }
    fn voigt(&self, center: f64, sigma: f64, gamma: f64) -> Self
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
            .into()
    }
    // fn smooth(
    //     &self,
    //     sigma: f64,
    //     gamma: Option<f64>,
    //     xstep: Option<f64>,
    //     npad: Option<usize>,
    //     form: ConvolveForm,
    // ) -> Self;

    fn is_sorted(&self) -> bool
    where
        Self: IntoIterator,
        Self::Item: PartialOrd,
    {
        let mut it = self.into_iter();
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

    fn argsort(&self) -> Vec<usize>
    where
        Self: PartialOrd,
    {
        let mut idx = (0..self.len()).collect::<Vec<_>>();
        idx.sort_by(|a, b| self[*a].partial_cmp(&self[*b]).unwrap());
        idx
    }
}

impl MathUtils for Vec<f64> {
    fn interpolation<T: Into<Vec<f64>>, U: Into<Vec<f64>>>(&self, x: &T, y: &U) -> Self {
        let lin = Linear::builder()
            .elements(y.into())
            .knots(x.into())
            .build()?;
        let result: Vec<f64> = lin.sample(self).collect();

        result.unwrap()
    }
}

impl MathUtils for Array1<f64> {
    fn interpolation<T: Into<Vec<f64>>, U: Into<Vec<f64>>>(&self, x: &T, y: &U) -> Self {
        let lin = Linear::builder()
            .elements(y.into())
            .knots(x.into())
            .build()?;
        let result: Vec<f64> = lin.sample(self).collect();

        result.unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_sorted() {
        let v = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        assert!(v.is_sorted());
        let v = vec![1.0, 2.0, 3.0, 4.0, 5.0, 3.0];
        assert!(!v.is_sorted());
    }

    #[test]
    fn test_argsort() {
        let v = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        assert_eq!(v.argsort(), vec![0, 1, 2, 3, 4]);
        let v = vec![1.0, 2.0, 3.0, 4.0, 5.0, 3.0];
        assert_eq!(v.argsort(), vec![0, 1, 2, 5, 3, 4]);
    }

    #[test]
    fn test_interpolation() {
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let y = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let z = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        assert_eq!(x.interpolation(&y, &z), vec![1.0, 2.0, 3.0, 4.0, 5.0]);
    }

    #[test]
    fn test_gaussian() {
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        assert_eq!(
            x.gaussian(3.0, 1.0),
            vec![
                0.24197072451914337,
                0.3989422804014327,
                0.24197072451914337,
                0.05399096651318806,
                0.0044318484119380075
            ]
        );
    }
}
