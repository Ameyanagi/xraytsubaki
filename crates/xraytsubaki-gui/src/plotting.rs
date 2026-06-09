//! Builders turning a processed `XASSpectrum` into ruviz `Plot`s for the four
//! Explore quadrants: mu(E), normalized mu(E), k-weighted chi(k), |chi(R)|.

use ruviz::plots::heatmap::HeatmapConfig;
use ruviz::prelude::Plot;
use xraytsubaki::prelude::XASSpectrum;

use crate::theme::Theme;

fn vecs(v: &nalgebra::DVector<f64>) -> Vec<f64> {
    v.iter().copied().collect()
}

pub struct QuadrantPlots {
    pub mu_e: Plot,
    pub norm: Plot,
    pub chi_k: Plot,
    pub chi_r: Plot,
}

pub fn build_quadrants(sp: &XASSpectrum, theme: &Theme) -> QuadrantPlots {
    let energy = sp
        .energy
        .as_ref()
        .map(vecs)
        .unwrap_or_default();
    let mu = sp.mu.as_ref().map(vecs).unwrap_or_default();

    let mu_e: Plot = Plot::new()
        .theme(theme.plot_theme())
        .line(&energy, &mu)
        .xlabel("Energy (eV)")
        .ylabel("mu(E)")
        .into();

    let flat = sp
        .get_flat()
        .or_else(|| sp.get_norm())
        .map(|v| vecs(&v))
        .unwrap_or_default();
    let norm: Plot = Plot::new()
        .theme(theme.plot_theme())
        .line(&energy, &flat)
        .xlabel("Energy (eV)")
        .ylabel("normalized mu(E)")
        .into();

    let k = sp.get_k().map(|v| vecs(&v)).unwrap_or_default();
    let kw = sp.get_kweight().copied().unwrap_or(2.0);
    let chik = sp
        .get_chi_kweighted()
        .map(|v| vecs(&v))
        .unwrap_or_default();
    let chi_k: Plot = Plot::new()
        .theme(theme.plot_theme())
        .line(&k, &chik)
        .xlabel("k (1/Angstrom)")
        .ylabel(format!("k^{kw:.0} chi(k)"))
        .into();

    let mut r = sp.get_r().map(|v| vecs(&v)).unwrap_or_default();
    let mut chir_mag = sp.get_chir_mag().map(|v| vecs(&v)).unwrap_or_default();
    let n = r.len().min(chir_mag.len());
    r.truncate(n);
    chir_mag.truncate(n);
    let chi_r: Plot = Plot::new()
        .theme(theme.plot_theme())
        .line(&r, &chir_mag)
        .xlabel("R (Angstrom)")
        .ylabel("|chi(R)|")
        .into();

    QuadrantPlots {
        mu_e,
        norm,
        chi_k,
        chi_r,
    }
}

/// Operando heatmap: rows = frames (time), cols = k-grid bins.
pub fn build_heatmap(matrix: &Vec<Vec<f64>>, kmax: f64, theme: &Theme) -> Plot {
    let _ = kmax;
    Plot::new()
        .theme(theme.plot_theme())
        .xlabel("k bin")
        .ylabel("frame")
        .heatmap(matrix, Some(HeatmapConfig::new().colorbar(true)))
        .into()
}

/// chi(k) of a single operando frame from the resampled grid row.
pub fn build_frame_chik(grid: &[f64], row: &[f64], kw: f64, theme: &Theme) -> Plot {
    let grid: Vec<f64> = grid.to_vec();
    let row: Vec<f64> = row.to_vec();
    Plot::new()
        .theme(theme.plot_theme())
        .line(&grid, &row)
        .xlabel("k (1/Angstrom)")
        .ylabel(format!("k^{kw:.0} chi(k)"))
        .into()
}

/// k-space fit overlay: k-weighted data vs model.
pub fn build_fit_k(result: &xraytsubaki::prelude::FeffFitResult, theme: &Theme) -> Plot {
    let k = vecs(&result.k);
    let kw = result.kweight;
    let weight = |chi: &nalgebra::DVector<f64>| -> Vec<f64> {
        chi.iter()
            .zip(k.iter())
            .map(|(c, kk)| c * kk.powf(kw))
            .collect()
    };
    let data = weight(&result.data_chi);
    let model = weight(&result.model_chi);
    Plot::new()
        .theme(theme.plot_theme())
        .line(&k, &data)
        .label("data")
        .line(&k, &model)
        .label("fit")
        .legend(ruviz::core::position::Position::TopRight)
        .xlabel("k (1/Angstrom)")
        .ylabel(format!("k^{kw:.0} chi(k)"))
        .into()
}

/// R-space fit overlay: |chi(R)| of data vs model.
pub fn build_fit_r(result: &xraytsubaki::prelude::FeffFitResult, theme: &Theme) -> Plot {
    let r = vecs(&result.r);
    let data_mag: Vec<f64> = result
        .data_chir_re
        .iter()
        .zip(result.data_chir_im.iter())
        .map(|(re, im)| (re * re + im * im).sqrt())
        .collect();
    let model_mag = vecs(&result.model_chir_mag);
    let n = r.len().min(data_mag.len()).min(model_mag.len());
    let r = r[..n].to_vec();
    let data_mag = data_mag[..n].to_vec();
    let model_mag = model_mag[..n].to_vec();
    Plot::new()
        .theme(theme.plot_theme())
        .line(&r, &data_mag)
        .label("data")
        .line(&r, &model_mag)
        .label("fit")
        .legend(ruviz::core::position::Position::TopRight)
        .xlabel("R (Angstrom)")
        .ylabel("|chi(R)|")
        .into()
}

/// Parameter-vs-frame trend with a cursor marker.
pub fn build_trend(values: &[f64], cursor: usize, ylabel: &str, theme: &Theme) -> Plot {
    let xs: Vec<f64> = (0..values.len()).map(|i| i as f64).collect();
    let ys: Vec<f64> = values.to_vec();
    let base = Plot::new()
        .theme(theme.plot_theme())
        .line(&xs, &ys)
        .xlabel("frame")
        .ylabel(ylabel);
    match values.get(cursor) {
        Some(&cy) if cy.is_finite() => {
            let cx = vec![cursor as f64];
            let cyv = vec![cy];
            base.scatter(&cx, &cyv).into()
        }
        _ => base.into(),
    }
}
