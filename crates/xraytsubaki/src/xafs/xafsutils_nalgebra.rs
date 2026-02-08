#![allow(dead_code)]
#![allow(unused_imports)]

use std::error::Error;

use nalgebra::DVector;
use serde::{Deserialize, Serialize};

use super::mathutils::{index_nearest_sorted, MathUtils};

pub const TINY_ENERGY: f64 = 0.005;

pub mod constants {
    #![allow(non_upper_case_globals)]

    pub const h: f64 = 6.62607015e-34;
    pub const hbar: f64 = h / (2.0 * std::f64::consts::PI);
    pub const m_e: f64 = 9.1093837015e-31;
    pub const e: f64 = 1.602176634e-19;
    pub const KTOE: f64 = 1.0e20 * hbar * hbar / (2.0 * m_e * e);
    pub const ETOK: f64 = 1.0 / KTOE;
}

pub trait XAFSUtils {
    fn etok(&self) -> Self;
    fn ktoe(&self) -> Self;
}

impl XAFSUtils for f64 {
    fn etok(&self) -> Self {
        if *self < 0.0 {
            0.0
        } else {
            self.sqrt() * constants::KTOE
        }
    }

    fn ktoe(&self) -> Self {
        self.powi(2) * constants::ETOK
    }
}

impl XAFSUtils for Vec<f64> {
    fn etok(&self) -> Self {
        self.iter().map(|x| x.etok()).collect()
    }

    fn ktoe(&self) -> Self {
        self.iter().map(|x| x.ktoe()).collect()
    }
}

impl XAFSUtils for DVector<f64> {
    fn etok(&self) -> Self {
        self.map(|x| {
            if x < 0.0 {
                0.0
            } else {
                x.sqrt() * constants::KTOE
            }
        })
    }

    fn ktoe(&self) -> Self {
        self.map(|x| x.powi(2) * constants::ETOK)
    }
}

pub fn find_energy_step(
    energy: &DVector<f64>,
    frac_ignore: Option<f64>,
    nave: Option<usize>,
    _verbose: Option<bool>,
) -> f64 {
    if energy.len() < 2 {
        return TINY_ENERGY;
    }

    let frac_ignore = frac_ignore.unwrap_or(0.05).clamp(0.0, 0.45);
    let nave = nave.unwrap_or(5).max(1);

    let mut diffs: Vec<f64> = energy
        .as_slice()
        .windows(2)
        .map(|w| w[1] - w[0])
        .filter(|d| d.is_finite() && *d > 0.0)
        .collect();

    if diffs.is_empty() {
        return TINY_ENERGY;
    }

    diffs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let start = ((diffs.len() as f64) * frac_ignore).floor() as usize;
    let end = diffs.len().saturating_sub(start).max(start + 1);
    let trimmed = &diffs[start..end];

    let center = trimmed.len() / 2;
    let half = nave / 2;
    let lo = center.saturating_sub(half);
    let hi = (center + half + 1).min(trimmed.len());
    let window = &trimmed[lo..hi];
    let avg = window.iter().sum::<f64>() / window.len() as f64;

    if avg.is_finite() && avg > 0.0 {
        avg
    } else {
        TINY_ENERGY
    }
}

pub fn find_e0(energy: &DVector<f64>, mu: &DVector<f64>) -> Result<f64, Box<dyn Error>> {
    if energy.len() != mu.len() {
        return Err("energy and mu length mismatch".into());
    }
    if energy.len() < 3 {
        return Err("need at least 3 points".into());
    }

    let mut best_idx = 1usize;
    let mut best_grad = f64::NEG_INFINITY;

    for i in 1..energy.len() - 1 {
        let de = energy[i + 1] - energy[i - 1];
        if !de.is_finite() || de.abs() < f64::EPSILON {
            continue;
        }
        let grad = (mu[i + 1] - mu[i - 1]) / de;
        if grad.is_finite() && grad > best_grad {
            best_grad = grad;
            best_idx = i;
        }
    }

    Ok(energy[best_idx])
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub enum FTWindow {
    #[default]
    Hanning,
    Parzen,
    Welch,
    Gaussian,
    Sine,
    KaiserBessel,
    FHanning,
}

impl FTWindow {
    pub fn window(
        &self,
        x: &DVector<f64>,
        xmin: Option<f64>,
        xmax: Option<f64>,
        dx: Option<f64>,
        dx2: Option<f64>,
    ) -> Result<DVector<f64>, Box<dyn Error>> {
        ftwindow(x, xmin, xmax, dx, dx2, Some(*self))
    }
}

pub fn ftwindow(
    x: &DVector<f64>,
    xmin: Option<f64>,
    xmax: Option<f64>,
    _dx: Option<f64>,
    _dx2: Option<f64>,
    _window: Option<FTWindow>,
) -> Result<DVector<f64>, Box<dyn Error>> {
    if x.is_empty() {
        return Ok(DVector::zeros(0));
    }

    let xmin = xmin.unwrap_or(x.min());
    let xmax = xmax.unwrap_or(x.max());

    let mut out = DVector::zeros(x.len());
    for (i, value) in x.iter().enumerate() {
        out[i] = if *value >= xmin && *value <= xmax {
            1.0
        } else {
            0.0
        };
    }
    Ok(out)
}
