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
