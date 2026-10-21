use super::*;
use easyfft::dyn_size::{DynFft, DynIfftMut};
use easyfft::num_complex::Complex;
use std::f64::consts::PI;

pub(super) const MAX_INPUT: usize = 1_000_000;
pub(super) const MAX_GRID: usize = 32768;
pub(super) const MAX_CELLS: usize = 4_000_000;
pub(super) struct Layout {
    pub k: Vec<f64>,
    pub r: Vec<f64>,
    pub size: WaveletSize,
    /// Selected k support limited to the measured k range (since 0.2.15).
    pub k_range: [f64; 2],
}
pub(super) fn layout(model: &Wavelet, k: &[f64]) -> Result<Layout> {
    if k.len() < 2
        || k.len() > MAX_INPUT
        || k.iter().any(|v| !v.is_finite() || *v < 0.)
        || k.windows(2).any(|w| w[0] >= w[1])
    {
        return Err(invalid(
            "provide 2 to 1,000,000 finite, nonnegative, strictly increasing k samples",
        ));
    }
    let [lo, hi] = model.k_range;
    if !lo.is_finite() || !hi.is_finite() || lo >= hi {
        return Err(invalid("k min must be finite and below k max"));
    }
    // Endpoints beyond the measured k range are limited to it (since 0.2.15).
    let (first, last) = (k[0], k[k.len() - 1]);
    let (lo, hi) = (lo.max(first), hi.min(last));
    if lo >= hi {
        return Err(invalid(format!(
            "the selected k range {:.2}–{:.2} Å⁻¹ lies outside the measured k range {first:.2}–{last:.2} Å⁻¹; change k min or k max",
            model.k_range[0], model.k_range[1]
        )));
    }
    if !model.kstep.is_finite()
        || model.kstep <= 0.
        || model.kweight > 6
        || !(1..=4096).contains(&model.order)
    {
        return Err(invalid(
            "kstep must be positive; weight must be 0–6 and order 1–4096",
        ));
    }
    match model.window {
        WaveletWindow::Cosine { width }
            if !width.is_finite() || width <= 0. || 2. * width > hi - lo =>
        {
            return Err(invalid(
                "cosine taper width must be positive and at most half the k support",
            ))
        }
        _ => {}
    }
    let count = (k[k.len() - 1] / model.kstep + 1e-10).floor() + 1.;
    if !count.is_finite() || !(2. ..=MAX_GRID as f64).contains(&count) {
        return Err(invalid(
            "prepared grid needs 2–32768 k samples; choose a coarser kstep",
        ));
    }
    let nk = count as usize;
    let nfft = model.nfft.unwrap_or((2 * nk).next_power_of_two());
    if !nfft.is_power_of_two() || nfft < 2 * nk || nfft > 262144 {
        return Err(invalid("FFT length must be a power of two, at least twice the k grid length and at most 262144; no truncation is allowed"));
    }
    let max_rows = (MAX_CELLS / nk).min(32768).min(64_000_000 / nfft);
    let r = if let Some(radii) = &model.radii {
        if radii.is_empty() || radii.len() > max_rows {
            return Err(invalid(
                "R grid exceeds the map or FFT-work budget; choose fewer rows",
            ));
        }
        radii.clone()
    } else {
        let step = model.rstep.unwrap_or(PI / (nfft as f64 * model.kstep));
        if !step.is_finite() || step <= 0. || !model.rmax.is_finite() || model.rmax <= 0. {
            return Err(invalid(
                "R maximum and sampling step must be finite and positive",
            ));
        }
        let nr = (model.rmax / step + 1e-10).floor();
        if !nr.is_finite() || nr < 1. || nr > max_rows as f64 {
            return Err(invalid("R grid exceeds the map or FFT-work budget, or has no rows; choose a coarser R step"));
        }
        (1..=nr as usize).map(|i| i as f64 * step).collect()
    };
    if r.iter()
        .any(|v| !v.is_finite() || *v <= 0. || !(model.order as f64 / (2. * v)).is_finite())
        || r.windows(2).any(|w| w[0] >= w[1])
    {
        return Err(invalid(
            "R coordinates must be finite, positive and strictly increasing; zero is singular",
        ));
    }
    if *r.last().unwrap() >= PI / (2. * model.kstep) {
        return Err(invalid("R grid reaches the k-sampling Nyquist limit; reduce R or use a smaller qualified kstep"));
    }
    let axis: Vec<_> = (0..nk).map(|i| i as f64 * model.kstep).collect();
    if axis.iter().filter(|v| **v >= lo && **v <= hi).count() < 2 {
        return Err(invalid(
            "selected support contains fewer than two prepared k samples",
        ));
    }
    let cells = nk * r.len();
    let size = WaveletSize {
        k_points: nk,
        r_points: r.len(),
        nfft,
        cells,
        bytes: 16 * cells + 25 * nk + 8 * r.len(),
    };
    Ok(Layout {
        k: axis,
        r,
        size,
        k_range: [lo, hi],
    })
}
fn interpolate(k: &[f64], chi: &[f64], x: f64) -> f64 {
    let upper = k.partition_point(|v| *v < x);
    if upper == 0 {
        return chi[0];
    }
    if upper == k.len() {
        return chi[k.len() - 1];
    }
    let t = (x - k[upper - 1]) / (k[upper] - k[upper - 1]);
    chi[upper - 1] * (1. - t) + chi[upper] * t
}
pub(super) fn calculate(
    model: &Wavelet,
    k: &[f64],
    chi: &[f64],
    cancelled: &dyn Fn() -> bool,
) -> Result<WaveletMap> {
    if cancelled() {
        return Err(WaveletError::Cancelled);
    }
    if chi.len() != k.len() || chi.iter().any(|v| !v.is_finite()) {
        return Err(invalid(
            "unweighted chi must be finite and match the k length",
        ));
    }
    let layout = layout(model, k)?;
    let [lo, hi] = layout.k_range;
    let mut prepared = Vec::with_capacity(layout.k.len());
    let mut window = Vec::with_capacity(layout.k.len());
    let mut support = Vec::with_capacity(layout.k.len());
    let mut signal = vec![0.; layout.size.nfft];
    for (i, &x) in layout.k.iter().enumerate() {
        let inside = x >= lo && x <= hi && x >= k[0] && x <= k[k.len() - 1];
        support.push(inside);
        let value = if inside { interpolate(k, chi, x) } else { 0. };
        let weight = if !inside {
            0.
        } else {
            match model.window {
                WaveletWindow::None => 1.,
                WaveletWindow::Cosine { width } => {
                    let distance = (x - lo).min(hi - x);
                    if distance < width {
                        0.5 * (1. - (PI * distance / width).cos())
                    } else {
                        1.
                    }
                }
            }
        };
        prepared.push(value);
        window.push(weight);
        signal[i] = value * weight * x.powi(model.kweight as i32);
        if !signal[i].is_finite() {
            return Err(invalid(
                "k weighting overflowed; inspect chi units and weight",
            ));
        }
    }
    if cancelled() {
        return Err(WaveletError::Cancelled);
    }
    let transform = signal.fft();
    if transform
        .iter()
        .any(|v| !v.re.is_finite() || !v.im.is_finite())
    {
        return Err(invalid("forward transform overflowed"));
    }
    let nfft = layout.size.nfft;
    let log_scale = (2. * PI).ln() - (1..=model.order).map(|i| (i as f64).ln()).sum::<f64>();
    let mut real = Vec::with_capacity(layout.size.cells);
    let mut imaginary = Vec::with_capacity(layout.size.cells);
    let mut row = vec![Complex::new(0., 0.); nfft];
    for &radius in &layout.r {
        if cancelled() {
            return Err(WaveletError::Cancelled);
        }
        row.fill(Complex::new(0., 0.));
        let a = model.order as f64 / (2. * radius);
        // DC, Nyquist and negative frequencies are zero. easyfft inverse is
        // unnormalized: explicit division by L supplies the named 1/L convention.
        for frequency in 1..nfft / 2 {
            let omega = 2. * PI * frequency as f64 / (nfft as f64 * model.kstep);
            let x = a * omega;
            let filter = if x.is_infinite() {
                0.
            } else {
                (log_scale + model.order as f64 * x.ln() - x).exp()
            };
            row[frequency] = transform[frequency] * filter;
        }
        row.ifft_mut();
        for value in row.iter().take(layout.k.len()) {
            let re = value.re / nfft as f64;
            let im = value.im / nfft as f64;
            if !re.is_finite() || !im.is_finite() {
                return Err(invalid("inverse transform overflowed"));
            }
            real.push(re);
            imaginary.push(im);
        }
    }
    let mut warnings=vec!["Features are not phase-corrected bond lengths; magnitude is not a concentration or coordination number.".into(),
        "Finite measured support and zero padding can create boundary artifacts; inspect range and taper sensitivity.".into()];
    if k.windows(2)
        .any(|w| ((w[1] - w[0]) - model.kstep).abs() > 1e-8 * model.kstep.max(1.))
        || k[0] != 0.
    {
        warnings.push("Original samples were linearly resampled onto the recorded zero-origin grid; sampling does not add resolution.".into());
    }
    if layout.k_range != model.k_range {
        warnings.push(format!(
            "The requested k range {:.2}–{:.2} Å⁻¹ was limited to the measured data: {lo:.2}–{hi:.2} Å⁻¹.",
            model.k_range[0], model.k_range[1]
        ));
    }
    if *layout.r.last().unwrap() > 0.8 * PI / (2. * model.kstep) {
        warnings.push(
            "The upper R grid approaches the Nyquist limit; part of the Cauchy filter is cut off."
                .into(),
        );
    }
    if cancelled() {
        return Err(WaveletError::Cancelled);
    }
    map::MapData {
        method: "cauchy_v1".into(),
        settings: model.clone(),
        size: layout.size,
        input_k: k.to_vec(),
        input_chi: chi.to_vec(),
        k: layout.k,
        r: layout.r,
        prepared_chi: prepared,
        window,
        support,
        real,
        imaginary,
        preparation: None,
        warnings,
    }
    .try_into()
}
