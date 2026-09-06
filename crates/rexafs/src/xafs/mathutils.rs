use enterpolation::{
    linear::{Linear, LinearError},
    Signal,
};
use errorfunctions::ComplexErrorFunctions;
use nalgebra::{DMatrix, DVector};
use num_complex::Complex64;

use super::errors::MathError;

pub trait MathUtils {
    fn interpolate(&self, x: &[f64], y: &[f64]) -> Result<Self, LinearError>
    where
        Self: Sized;

    fn is_sorted(&self) -> bool;

    fn argsort(&self) -> Vec<usize>;

    fn gaussian(self, center: f64, sigma: f64) -> DVector<f64>
    where
        Self: Into<DVector<f64>>,
    {
        let x: DVector<f64> = self.into().map(|value| value - center);
        let sigma = sigma.max(f64::EPSILON);
        let inverse_of_coefficient = sigma * (2.0 * std::f64::consts::PI).sqrt();
        x.map(|value| (-value.powi(2) / (2.0 * sigma.powi(2))).exp() / inverse_of_coefficient)
    }

    fn lorentzian(self, center: f64, sigma: f64) -> DVector<f64>
    where
        Self: Into<DVector<f64>>,
    {
        let x: DVector<f64> = self.into().map(|value| value - center);
        let sigma = sigma.max(f64::EPSILON);
        let coefficient = sigma / std::f64::consts::PI;
        x.map(|value| coefficient / (value.powi(2) + sigma.powi(2)))
    }

    fn voigt(self, center: f64, sigma: f64, gamma: f64) -> DVector<f64>
    where
        Self: Into<DVector<f64>>,
    {
        let x: DVector<f64> = self.into().map(|value| value - center);
        let sigma = sigma.max(f64::EPSILON);
        let gamma = gamma.max(f64::EPSILON);
        let inverse_of_coefficient = sigma * (2.0 * std::f64::consts::PI).sqrt();

        DVector::from_iterator(
            x.len(),
            x.iter().map(|value| {
                let z = Complex64::new(*value, gamma) / sigma / 2.0_f64.sqrt();
                z.w().re / inverse_of_coefficient
            }),
        )
    }

    fn min(&self) -> f64;
    fn max(&self) -> f64;
    fn diff(&self) -> Self;
    fn gradient(&self) -> Self;
}

impl MathUtils for Vec<f64> {
    fn interpolate(&self, x: &[f64], y: &[f64]) -> Result<Self, LinearError> {
        let x_left = *x.iter().min_by(|a, b| a.partial_cmp(b).unwrap()).unwrap();
        let x_right = *x.iter().max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap();
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
        is_sorted(self.as_slice())
    }

    fn argsort(&self) -> Vec<usize> {
        argsort(self.as_slice())
    }

    fn min(&self) -> f64 {
        *self
            .iter()
            .min_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap()
    }

    fn max(&self) -> f64 {
        *self
            .iter()
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap()
    }

    fn diff(&self) -> Self {
        let mut result = Vec::with_capacity(self.len().saturating_sub(1));
        for i in 0..self.len().saturating_sub(1) {
            result.push(self[i + 1] - self[i]);
        }
        result
    }

    fn gradient(&self) -> Self {
        match self.len() {
            0..=1 => vec![0.0; self.len()],
            2 => vec![self[1] - self[0], self[1] - self[0]],
            _ => {
                let mut result = Vec::with_capacity(self.len());
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

impl MathUtils for DVector<f64> {
    fn interpolate(&self, x: &[f64], y: &[f64]) -> Result<Self, LinearError> {
        let x_left = *x.iter().min_by(|a, b| a.partial_cmp(b).unwrap()).unwrap();
        let x_right = *x.iter().max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap();
        let lin = Linear::builder().elements(y).knots(x).build()?;
        let result: Vec<f64> = lin
            .sample(self.iter().map(|a| match a {
                a if a > &x_right => x_right,
                a if a < &x_left => x_left,
                _ => *a,
            }))
            .collect();
        Ok(DVector::from_vec(result))
    }

    fn is_sorted(&self) -> bool {
        is_sorted(self.as_slice())
    }

    fn argsort(&self) -> Vec<usize> {
        argsort(self.as_slice())
    }

    fn min(&self) -> f64 {
        *self
            .iter()
            .min_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap()
    }

    fn max(&self) -> f64 {
        *self
            .iter()
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap()
    }

    fn diff(&self) -> Self {
        let mut result = Vec::with_capacity(self.len().saturating_sub(1));
        for i in 0..self.len().saturating_sub(1) {
            result.push(self[i + 1] - self[i]);
        }
        DVector::from_vec(result)
    }

    fn gradient(&self) -> Self {
        match self.len() {
            0..=1 => DVector::zeros(self.len()),
            2 => DVector::from_vec(vec![self[1] - self[0], self[1] - self[0]]),
            _ => {
                let mut result = DVector::zeros(self.len());
                result[0] = self[1] - self[0];
                for i in 1..self.len() - 1 {
                    result[i] = (self[i + 1] - self[i - 1]) / 2.0;
                }
                result[self.len() - 1] = self[self.len() - 1] - self[self.len() - 2];
                result
            }
        }
    }
}

fn is_sorted(data: &[f64]) -> bool {
    data.windows(2).all(|pair| pair[0] <= pair[1])
}

fn argsort(v: &[f64]) -> Vec<usize> {
    let mut idx = (0..v.len()).collect::<Vec<_>>();
    idx.sort_by(|a, b| v[*a].partial_cmp(&v[*b]).unwrap());
    idx
}

pub fn index_of(array: &[f64], value: &f64) -> Result<usize, MathError> {
    if array.is_empty() {
        return Err(MathError::IndexOutOfBounds { index: 0, len: 0 });
    }

    let min_value = array
        .iter()
        .min_by(|a, b| a.partial_cmp(b).unwrap())
        .copied()
        .ok_or(MathError::IndexOutOfBounds { index: 0, len: 0 })?;
    if &min_value > value {
        return Ok(0);
    }

    Ok(array
        .iter()
        .enumerate()
        .find_map(|(i, x)| if x > value { Some(i - 1) } else { None })
        .unwrap_or(array.len() - 1))
}

pub fn index_of_sorted(array: &[f64], value: &f64) -> Result<usize, MathError> {
    if array.is_empty() {
        return Err(MathError::IndexOutOfBounds { index: 0, len: 0 });
    }
    let idx = array.partition_point(|x| x <= value);
    Ok(idx.saturating_sub(1))
}

pub fn index_nearest(array: &[f64], value: &f64) -> Result<usize, MathError> {
    Ok(array
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| (*a - value).abs().partial_cmp(&(*b - value).abs()).unwrap())
        .unwrap()
        .0)
}

pub fn index_nearest_sorted(array: &[f64], value: &f64) -> Result<usize, MathError> {
    if array.is_empty() {
        return Err(MathError::IndexOutOfBounds { index: 0, len: 0 });
    }

    let idx = array.partition_point(|x| x < value);
    if idx == 0 {
        return Ok(0);
    }
    if idx >= array.len() {
        return Ok(array.len() - 1);
    }

    let prev = array[idx - 1];
    let next = array[idx];
    if (prev - value).abs() <= (next - value).abs() {
        Ok(idx - 1)
    } else {
        Ok(idx)
    }
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

pub fn splev_jacobian(t: Vec<f64>, c: Vec<f64>, k: usize, x: Vec<f64>, e: usize) -> DMatrix<f64> {
    super::spline::coefficient_jacobian(&t, c.len(), k, &x, e == 3)
}
