use errorfunctions::ComplexErrorFunctions;
use ndarray::{Array, Array1};
use num_complex::Complex64;

const TINY_ENERGY: f64 = 0.005;

pub fn is_sorted<I>(data: I) -> bool
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

pub fn argsort<T: PartialOrd>(v: &[T]) -> Vec<usize> {
    let mut idx = (0..v.len()).collect::<Vec<_>>();
    idx.sort_by(|a, b| v[*a].partial_cmp(&v[*b]).unwrap());
    idx
}

#[derive(Debug, Clone, Copy, Default)]
pub enum ConvolveForm {
    #[default]
    Lorentzian,
    Gaussian,
    Voigt,
}

pub fn smooth<T: Into<Array1<f64>>>(
    x: T,
    y: T,
    sigma: Option<f64>,
    gamma: Option<f64>,
    xstep: Option<f64>,
    npad: Option<i32>,
    conv_form: ConvolveForm,
) {
    const TINY: f64 = 1e-12;

    let x = x.into();
    let y = y.into();
    // let sigma = sigma.unwrap_or(1.0);
    // let gamma = gamma.unwrap_or(0.0);
    let npad = npad.unwrap_or(5);

    let x_diff = &x.slice(ndarray::s![1..]) - &x.slice(ndarray::s![..-1]);
    let xstep = xstep.unwrap_or(
        x_diff
            .iter()
            .min_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap()
            .clone(),
    );

    if xstep < TINY {
        panic!("Cannot smooth data: must be strictly increasing");
    }

    let sigma = sigma.unwrap_or(1.0) / xstep;
    let gamma = gamma.unwrap_or(sigma);

    let xmin = xstep
        * ((x.iter().min_by(|a, b| a.partial_cmp(b).unwrap()).unwrap() - npad as f64 * xstep)
            / xstep);
    let xmax = xstep
        * ((x.iter().max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap() + npad as f64 * xstep)
            / xstep);
    let npts1 = 1 + ((xmax - xmin + xstep * 0.1) / xstep).abs() as usize;
    let npts = npts1.min(50 * x.len());

    let x0 = Array1::linspace(xmin, xmax, npts);
    // let y0 =

    // let wx = Array1::range(0.0, 2.0 * npts as f64, 1.0);

    // let win = match conv_form {
    //     ConvolveForm::Gaussian => gaussian(wx, npts as f64, sigma),
    //     ConvolveForm::Voigt => voigt(wx, npts as f64, sigma, gamma),
    //     ConvolveForm::Lorentzian => lorentzian(wx, npts as f64, sigma),
    // };

    // let x0 = Array1::linspace(xmin, xmax, npts);
    // let y0 = interp(&x0, &x, &y);

    // let sigma = sigma / xstep;
    // let gamma = gamma / xstep;

    // let wx = Array1::range(0.0, 2.0 * npts as f64, 1.0);

    // let win = match conv_form {
    //     ConvolveForm::Gaussian => gaussian(&wx, npts, sigma),
    //     ConvolveForm::Voigt => voigt(&wx, npts, sigma, gamma),
    //     _ => lorentzian(&wx, npts, sigma),
    // };
}

pub fn gaussian<T: Into<Array1<f64>>>(x: T, center: f64, sigma: f64) -> Array1<f64> {
    let x: Array1<f64> = x.into() - center;
    let sigma = sigma.max(f64::EPSILON);
    let inverse_of_coefficient = sigma * (2.0 * std::f64::consts::PI).sqrt();
    x.map(|x| (-x.powi(2) / (2.0 * sigma.powi(2))).exp() / inverse_of_coefficient)
}

pub fn lorentzian<T: Into<Array1<f64>>>(x: T, center: f64, sigma: f64) -> Array1<f64> {
    let x: Array1<f64> = x.into() - center;
    let sigma = sigma.max(f64::EPSILON);
    let coefficient = sigma / std::f64::consts::PI;
    x.map(|x| coefficient / (x.powi(2) + sigma.powi(2)))
}

pub fn voigt<T: Into<Array1<f64>>>(x: T, center: f64, sigma: f64, gamma: f64) -> Array1<f64> {
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

#[cfg(test)]
mod tests {

    use std::vec;

    use super::*;

    #[test]
    fn test_is_sorted() {
        let v = vec![1, 2, 3, 4, 5];
        assert!(is_sorted(v));
    }

    #[test]
    fn test_is_sorted_float() {
        let v = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        assert!(is_sorted(v));
    }

    #[test]
    fn test_argsort() {
        let v = vec![1, 7, 4, 2];
        let i = argsort(&v);
        assert_eq!(i, &[0, 3, 2, 1]);
    }

    #[test]
    fn test_argsort_float() {
        let v = vec![1.0, 7.0, 4.0, 2.0];
        let i = argsort(&v);
        assert_eq!(i, &[0, 3, 2, 1]);
    }

    #[test]
    fn test_smooth() {
        let x = Array1::range(0.0, 10.0, 1.0);
        let y = Array1::range(0.0, 10.0, 1.0);
        smooth(x, y, None, None, None, None, ConvolveForm::Lorentzian);
    }

    #[test]
    fn test_gaussian() {
        let x = Array1::from_vec(vec![0., 1.0, 2.0, 3.0, 4.0, 5.0, 6., 7., 8., 9.]);
        let y = gaussian(x, 5.0, 1.0);
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
    }

    #[test]
    fn test_lorentzian() {
        let x = Array1::from_vec(vec![0., 1.0, 2.0, 3.0, 4.0, 5.0, 6., 7., 8., 9.]);
        let y = lorentzian(x, 5.0, 1.0);
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
    }

    #[test]
    fn test_voigt() {
        let x = Array1::from_vec(vec![0., 1.0, 2.0, 3.0, 4.0, 5.0, 6., 7., 8., 9.]);
        let y = voigt(x, 5.0, 1.0, 1.0);
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
    }
}
