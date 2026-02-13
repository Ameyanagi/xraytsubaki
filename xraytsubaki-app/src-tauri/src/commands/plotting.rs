use std::fs;
use std::path::PathBuf;

use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use tauri::State;
use uuid::Uuid;
use xraytsubaki::prelude::{BackgroundMethod, PlotXAS};

use crate::commands::spectra::{apply_bg_options, apply_fft_options, apply_norm_options};
use crate::dto::{PipelineOptions, PlotResult, PlotTrace};
use crate::state::AppState;
use xraytsubaki::prelude::XASSpectrum;

fn dvec_to_vec(v: &nalgebra::DVector<f64>) -> Vec<f64> {
    v.iter().copied().collect()
}

fn k_axis_data(spec: &XASSpectrum) -> Option<Vec<f64>> {
    spec.get_k()
        .or_else(|| spec.k.clone())
        .map(|v| v.iter().copied().collect())
}

fn chi_data(spec: &XASSpectrum) -> Option<Vec<f64>> {
    spec.get_chi()
        .or_else(|| spec.chi.clone())
        .map(|v| v.iter().copied().collect())
}

fn background_kweight(spec: &XASSpectrum) -> Option<i32> {
    match spec.background.as_ref() {
        Some(BackgroundMethod::AUTOBK(autobk)) => autobk.get_kweight().copied(),
        _ => None,
    }
}

fn k_trace_data(spec: &XASSpectrum) -> Option<Vec<f64>> {
    let chi = chi_data(spec)?;
    let Some(kweight) = background_kweight(spec) else {
        return Some(chi);
    };

    let k = k_axis_data(spec)?;
    Some(
        chi.iter()
            .zip(k.iter())
            .map(|(chi_i, k_i)| {
                if kweight == 0 {
                    *chi_i
                } else {
                    chi_i * k_i.powi(kweight)
                }
            })
            .collect(),
    )
}

/// Create a main trace (not an overlay).
fn main_trace(x: Vec<f64>, y: Vec<f64>, label: String, panel: &str) -> PlotTrace {
    PlotTrace {
        x,
        y,
        label,
        panel: panel.into(),
        overlay: None,
        dash: None,
        color: None,
    }
}

/// Create an overlay trace.
fn overlay_trace(
    x: Vec<f64>,
    y: Vec<f64>,
    label: String,
    panel: &str,
    overlay_id: &str,
    dash: &str,
    color: &str,
) -> PlotTrace {
    PlotTrace {
        x,
        y,
        label,
        panel: panel.into(),
        overlay: Some(overlay_id.into()),
        dash: Some(dash.into()),
        color: Some(color.into()),
    }
}

/// Numerical derivative dy/dx using central differences.
fn derivative(x: &[f64], y: &[f64]) -> Vec<f64> {
    let n = x.len().min(y.len());
    if n == 0 {
        return Vec::new();
    }
    if n == 1 {
        return vec![0.0];
    }

    let slope = |dy: f64, dx: f64| {
        if dx.abs() <= f64::EPSILON {
            0.0
        } else {
            dy / dx
        }
    };
    let mut d = Vec::with_capacity(n);
    for i in 0..n {
        if i == 0 {
            d.push(slope(y[1] - y[0], x[1] - x[0]));
        } else if i == n - 1 {
            d.push(slope(y[i] - y[i - 1], x[i] - x[i - 1]));
        } else {
            d.push(slope(y[i + 1] - y[i - 1], x[i + 1] - x[i - 1]));
        }
    }
    d
}

/// Build overlay traces for a single spectrum in a given panel mode.
fn build_overlays(spec: &XASSpectrum, name: &str, panel: &str) -> Vec<PlotTrace> {
    let mut overlays = Vec::new();

    match panel {
        "mu" => {
            if let (Some(energy), Some(mu)) = (&spec.energy, &spec.mu) {
                let e_vec = dvec_to_vec(energy);
                let mu_vec = dvec_to_vec(mu);

                // dμ/dE — scaled to fit on the same y-axis
                let dmude = derivative(&e_vec, &mu_vec);
                let max_dmu = dmude.iter().map(|v| v.abs()).fold(0.0_f64, f64::max);
                let mu_min = mu_vec.iter().copied().fold(f64::INFINITY, f64::min);
                let mu_max = mu_vec.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                let mu_range = mu_max - mu_min;
                let scale = if max_dmu > 0.0 {
                    mu_range * 0.4 / max_dmu
                } else {
                    1.0
                };
                let scaled_dmude: Vec<f64> = dmude.iter().map(|d| d * scale + mu_min).collect();
                overlays.push(overlay_trace(
                    e_vec.clone(),
                    scaled_dmude,
                    "d\u{03BC}/dE".into(),
                    panel,
                    "dmude",
                    "dot",
                    "#ef4444",
                ));

                // Pre-edge line
                if let Some(pre_edge) = spec.get_pre_edge() {
                    overlays.push(overlay_trace(
                        e_vec.clone(),
                        pre_edge.iter().copied().collect(),
                        "Pre-edge".into(),
                        panel,
                        "preedge",
                        "dash",
                        "#f97316",
                    ));
                }

                // Post-edge line
                if let Some(post_edge) = spec.get_post_edge() {
                    overlays.push(overlay_trace(
                        e_vec.clone(),
                        post_edge.iter().copied().collect(),
                        "Post-edge".into(),
                        panel,
                        "postedge",
                        "dash",
                        "#22c55e",
                    ));
                }

                // E0 vertical marker
                if let Some(e0) = spec.e0 {
                    overlays.push(overlay_trace(
                        vec![e0, e0],
                        vec![mu_min, mu_max],
                        format!("E0 = {:.1} eV", e0),
                        panel,
                        "e0marker",
                        "dashdot",
                        "#06b6d4",
                    ));
                }
            }
        }
        "norm" => {
            if let (Some(energy), Some(norm)) = (&spec.energy, &spec.get_norm()) {
                let e_vec = dvec_to_vec(energy);
                let norm_vec: Vec<f64> = norm.iter().copied().collect();

                // Flattened
                if let Some(flat) = spec.get_flat() {
                    overlays.push(overlay_trace(
                        e_vec.clone(),
                        flat.iter().copied().collect(),
                        "Flattened".into(),
                        panel,
                        "flattened",
                        "dot",
                        "#a855f7",
                    ));
                }

                // dNorm/dE — scaled
                let dnormde = derivative(&e_vec, &norm_vec);
                let max_d = dnormde.iter().map(|v| v.abs()).fold(0.0_f64, f64::max);
                let scale = if max_d > 0.0 { 0.4 / max_d } else { 1.0 };
                let scaled: Vec<f64> = dnormde.iter().map(|d| d * scale).collect();
                overlays.push(overlay_trace(
                    e_vec.clone(),
                    scaled,
                    "dNorm/dE".into(),
                    panel,
                    "dnormde",
                    "dot",
                    "#ef4444",
                ));

                // Pre-edge in normalized space
                if let (Some(pre_edge), Some(post_edge)) =
                    (spec.get_pre_edge(), spec.get_post_edge())
                {
                    let fallback_e0 = e_vec.get(e_vec.len() / 3).copied().unwrap_or(0.0);
                    let e0 = spec.e0.unwrap_or(fallback_e0);
                    let e0_idx = e_vec.iter().position(|e| *e >= e0).unwrap_or(0);
                    let pre_at_e0 = pre_edge.get(e0_idx).copied().unwrap_or(0.0);
                    let post_at_e0 = post_edge.get(e0_idx).copied().unwrap_or(1.0);
                    let step = (post_at_e0 - pre_at_e0).max(0.01);

                    let pre0 = pre_edge.get(0).copied().unwrap_or(0.0);
                    let norm_pre: Vec<f64> = pre_edge.iter().map(|p| (p - pre0) / step).collect();
                    let norm_post: Vec<f64> =
                        post_edge.iter().map(|p| (p - pre_at_e0) / step).collect();

                    overlays.push(overlay_trace(
                        e_vec.clone(),
                        norm_pre,
                        "Pre-edge".into(),
                        panel,
                        "preedge",
                        "dash",
                        "#f97316",
                    ));
                    overlays.push(overlay_trace(
                        e_vec.clone(),
                        norm_post,
                        "Post-edge".into(),
                        panel,
                        "postedge",
                        "dash",
                        "#22c55e",
                    ));
                }
            }
        }
        "k" => {
            if let Some(k_vec) = k_axis_data(spec) {
                // |chi(k)| magnitude envelope
                if let Some(chi_kw_vec) = k_trace_data(spec) {
                    let mag: Vec<f64> = chi_kw_vec.iter().map(|v| v.abs()).collect();
                    let neg_mag: Vec<f64> = mag.iter().map(|v| -v).collect();
                    overlays.push(overlay_trace(
                        k_vec.clone(),
                        mag,
                        "|\u{03C7}(k)|".into(),
                        panel,
                        "chimag",
                        "dot",
                        "#94a3b8",
                    ));
                    overlays.push(overlay_trace(
                        k_vec.clone(),
                        neg_mag,
                        "-|\u{03C7}(k)|".into(),
                        panel,
                        "chimag",
                        "dot",
                        "#94a3b8",
                    ));

                    // k-window scaled to chi range
                    if let Some(kwin) = spec.get_kwin() {
                        let max_chi = chi_kw_vec.iter().map(|v| v.abs()).fold(0.0_f64, f64::max);
                        let scaled_win: Vec<f64> = kwin.iter().map(|w| w * max_chi).collect();
                        overlays.push(overlay_trace(
                            k_vec.clone(),
                            scaled_win,
                            "Window".into(),
                            panel,
                            "window",
                            "dash",
                            "#eab308",
                        ));
                    }
                }
            }
        }
        "r" => {
            if let Some(r) = spec.get_r() {
                let r_vec: Vec<f64> = r.iter().copied().collect();

                // Re[chi(R)]
                if let Some(chir_re) = spec.get_chir_real() {
                    overlays.push(overlay_trace(
                        r_vec.clone(),
                        chir_re.iter().copied().collect(),
                        format!("{} Re[\u{03C7}(R)]", name),
                        panel,
                        "chir_re",
                        "dash",
                        "#3b82f6",
                    ));
                }

                // Im[chi(R)]
                if let Some(chir_im) = spec.get_chir_imag() {
                    overlays.push(overlay_trace(
                        r_vec.clone(),
                        chir_im.iter().copied().collect(),
                        format!("{} Im[\u{03C7}(R)]", name),
                        panel,
                        "chir_im",
                        "dash",
                        "#ef4444",
                    ));
                }

                // R-space window (using default Hanning from 1-3 Å if not stored)
                if let Some(chir_mag) = spec.get_chir_mag() {
                    let max_mag = chir_mag.iter().copied().fold(0.0_f64, f64::max);
                    let rmin = 1.0_f64;
                    let rmax = 3.0_f64;
                    let dr = 0.2_f64;
                    let rwin: Vec<f64> = r_vec
                        .iter()
                        .map(|rv| {
                            let w = if *rv < rmin - dr || *rv > rmax + dr {
                                0.0
                            } else if *rv < rmin + dr {
                                0.5 * (1.0
                                    + (std::f64::consts::PI * (rv - rmin - dr) / (2.0 * dr)).cos())
                            } else if *rv > rmax - dr {
                                0.5 * (1.0
                                    + (std::f64::consts::PI * (rv - rmax + dr) / (2.0 * dr)).cos())
                            } else {
                                1.0
                            };
                            w * max_mag
                        })
                        .collect();
                    overlays.push(overlay_trace(
                        r_vec.clone(),
                        rwin,
                        "Window".into(),
                        panel,
                        "window",
                        "dash",
                        "#eab308",
                    ));
                }
            }
        }
        _ => {}
    }

    overlays
}

fn panel_labels(panel: &str) -> (String, String) {
    match panel {
        "mu" => ("Energy (eV)".to_string(), "\u{03BC}(E)".to_string()),
        "norm" => (
            "Energy (eV)".to_string(),
            "Normalized \u{03BC}(E)".to_string(),
        ),
        "k" => (
            "k (\u{00C5}\u{207B}\u{00B9})".to_string(),
            "k\u{00B2}\u{03C7}(k)".to_string(),
        ),
        "r" => (
            "R (\u{00C5})".to_string(),
            "|\u{03C7}(R)| (\u{00C5}\u{207B}\u{00B3})".to_string(),
        ),
        _ => (String::new(), String::new()),
    }
}

fn panel_builder<'a>(
    spec: &'a mut XASSpectrum,
    panel: &str,
) -> Option<xraytsubaki::plot::XASPlotBuilder<'a>> {
    let builder = spec.plot();
    match panel {
        "mu" => Some(builder.mu()),
        "norm" => Some(builder.norm()),
        "k" => Some(builder.k()),
        "r" => Some(builder.r()),
        _ => None,
    }
}

fn ensure_e0(spec: &mut XASSpectrum) -> Result<(), String> {
    if spec.e0.is_none() {
        spec.find_e0().map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn has_owned_vec_data(v: Option<nalgebra::DVector<f64>>) -> bool {
    v.map(|data| !data.is_empty()).unwrap_or(false)
}

fn ensure_normalized(spec: &mut XASSpectrum, opts: Option<&PipelineOptions>) -> Result<(), String> {
    if !spec.get_norm().map(|v| !v.is_empty()).unwrap_or(false)
        || !spec.get_flat().map(|v| !v.is_empty()).unwrap_or(false)
        || !spec.get_pre_edge().map(|v| !v.is_empty()).unwrap_or(false)
        || !spec.get_post_edge().map(|v| !v.is_empty()).unwrap_or(false)
    {
        if let Some(norm_opts) = opts.and_then(|pipeline| pipeline.norm.as_ref()) {
            apply_norm_options(spec, norm_opts)?;
        }
        ensure_e0(spec)?;
        spec.normalize().map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn ensure_background(spec: &mut XASSpectrum, opts: Option<&PipelineOptions>) -> Result<(), String> {
    if !has_owned_vec_data(spec.get_k()) || !has_owned_vec_data(spec.get_chi()) {
        ensure_normalized(spec, opts)?;
        if let Some(bg_opts) = opts.and_then(|pipeline| pipeline.bg.as_ref()) {
            apply_bg_options(spec, bg_opts)?;
        }
        spec.calc_background().map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn ensure_fft(spec: &mut XASSpectrum, opts: Option<&PipelineOptions>) -> Result<(), String> {
    if !spec.get_r().map(|v| !v.is_empty()).unwrap_or(false)
        || !spec.get_chir_mag().map(|v| !v.is_empty()).unwrap_or(false)
        || !spec.get_chir_real().map(|v| !v.is_empty()).unwrap_or(false)
        || !spec.get_chir_imag().map(|v| !v.is_empty()).unwrap_or(false)
    {
        ensure_background(spec, opts)?;
        if let Some(fft_opts) = opts.and_then(|pipeline| pipeline.fft.as_ref()) {
            apply_fft_options(spec, fft_opts)?;
        }
        spec.fft().map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn ensure_panel_data(
    spec: &mut XASSpectrum,
    panel: &str,
    opts: Option<&PipelineOptions>,
) -> Result<(), String> {
    match panel {
        // Mu overlays depend on normalization outputs and e0 marker.
        "mu" | "norm" => ensure_normalized(spec, opts),
        "k" => ensure_background(spec, opts),
        "r" => ensure_fft(spec, opts),
        _ => Ok(()),
    }
}

fn render_panel_png_data_url(
    spec: &mut XASSpectrum,
    panel: &str,
) -> Result<Option<String>, String> {
    let Some(builder) = panel_builder(spec, panel) else {
        return Ok(None);
    };

    let mut png_path: PathBuf = std::env::temp_dir();
    png_path.push(format!("xraytsubaki-core-{}-{}.png", panel, Uuid::new_v4()));

    builder
        .save_png(&png_path)
        .map_err(|e| format!("Failed to render PNG for panel '{panel}': {e}"))?;

    let png_bytes = fs::read(&png_path)
        .map_err(|e| format!("Failed to read rendered PNG for panel '{panel}': {e}"))?;
    let _ = fs::remove_file(&png_path);

    Ok(Some(format!(
        "data:image/png;base64,{}",
        BASE64_STANDARD.encode(png_bytes)
    )))
}

fn render_panel_svg(spec: &mut XASSpectrum, panel: &str) -> Result<Option<String>, String> {
    let Some(builder) = panel_builder(spec, panel) else {
        return Ok(None);
    };
    let svg = builder
        .to_svg()
        .map_err(|e| format!("Failed to render SVG for panel '{panel}': {e}"))?;
    Ok(Some(svg))
}

#[tauri::command]
pub fn plot_spectrum(
    state: State<'_, AppState>,
    index: usize,
    panels: Vec<String>,
    opts: Option<PipelineOptions>,
) -> Result<PlotResult, String> {
    let mut group = state.group.lock().map_err(|e| e.to_string())?;
    let spec = group.get_spectrum_mut(index).map_err(|e| e.to_string())?;
    let name = spec
        .name
        .clone()
        .unwrap_or_else(|| format!("Spectrum {}", index));

    let mut traces = Vec::new();
    let mut x_label = String::new();
    let mut y_label = String::new();

    for panel in &panels {
        ensure_panel_data(spec, panel, opts.as_ref()).map_err(|err| {
            format!(
                "Failed to prepare panel '{}' for spectrum #{}: {}",
                panel, index, err
            )
        })?;
        match panel.as_str() {
            "mu" => {
                if let (Some(energy), Some(mu)) = (&spec.energy, &spec.mu) {
                    traces.push(main_trace(
                        dvec_to_vec(energy),
                        dvec_to_vec(mu),
                        name.clone(),
                        "mu",
                    ));
                    x_label = "Energy (eV)".into();
                    y_label = "\u{03BC}(E)".into();
                }
            }
            "norm" => {
                if let (Some(energy), Some(norm)) = (&spec.energy, &spec.get_norm()) {
                    traces.push(main_trace(
                        dvec_to_vec(energy),
                        norm.iter().copied().collect(),
                        name.clone(),
                        "norm",
                    ));
                    x_label = "Energy (eV)".into();
                    y_label = "Normalized \u{03BC}(E)".into();
                }
            }
            "k" => {
                if let (Some(k), Some(y_data)) = (k_axis_data(spec), k_trace_data(spec)) {
                    traces.push(main_trace(k, y_data, name.clone(), "k"));
                    x_label = "k (\u{00C5}\u{207B}\u{00B9})".into();
                    y_label = "k\u{00B2}\u{03C7}(k)".into();
                }
            }
            "r" => {
                if let (Some(r), Some(chir_mag)) = (&spec.get_r(), &spec.get_chir_mag()) {
                    traces.push(main_trace(
                        r.iter().copied().collect(),
                        chir_mag.iter().copied().collect(),
                        format!("{} |\u{03C7}(R)|", name),
                        "r",
                    ));
                    x_label = "R (\u{00C5})".into();
                    y_label = "|\u{03C7}(R)| (\u{00C5}\u{207B}\u{00B3})".into();
                }
            }
            _ => {}
        }

        // Append overlay traces for this panel
        traces.extend(build_overlays(spec, &name, panel));
    }

    Ok(PlotResult {
        traces,
        pngs: Vec::new(),
        svgs: Vec::new(),
        x_label,
        y_label,
    })
}

#[tauri::command]
pub fn plot_group(
    state: State<'_, AppState>,
    indices: Vec<usize>,
    panels: Vec<String>,
    opts: Option<PipelineOptions>,
) -> Result<PlotResult, String> {
    let mut group = state.group.lock().map_err(|e| e.to_string())?;
    let mut all_traces = Vec::new();
    let mut x_label = String::new();
    let mut y_label = String::new();
    let mut prep_errors: Vec<String> = Vec::new();

    for &idx in &indices {
        let spec = match group.get_spectrum_mut(idx) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let name = spec
            .name
            .clone()
            .unwrap_or_else(|| format!("Spectrum {}", idx));

        for panel in &panels {
            if let Err(err) = ensure_panel_data(spec, panel, opts.as_ref()) {
                prep_errors.push(format!(
                    "panel '{}' for spectrum #{} ({}): {}",
                    panel, idx, name, err
                ));
                continue;
            }
            match panel.as_str() {
                "mu" => {
                    if let (Some(energy), Some(mu)) = (&spec.energy, &spec.mu) {
                        all_traces.push(main_trace(
                            dvec_to_vec(energy),
                            dvec_to_vec(mu),
                            name.clone(),
                            "mu",
                        ));
                        x_label = "Energy (eV)".into();
                        y_label = "\u{03BC}(E)".into();
                    }
                }
                "norm" => {
                    if let (Some(energy), Some(norm)) = (&spec.energy, &spec.get_norm()) {
                        all_traces.push(main_trace(
                            dvec_to_vec(energy),
                            norm.iter().copied().collect(),
                            name.clone(),
                            "norm",
                        ));
                        x_label = "Energy (eV)".into();
                        y_label = "Normalized \u{03BC}(E)".into();
                    }
                }
                "k" => {
                    if let (Some(k), Some(y_data)) = (k_axis_data(spec), k_trace_data(spec)) {
                        all_traces.push(main_trace(k, y_data, name.clone(), "k"));
                        x_label = "k (\u{00C5}\u{207B}\u{00B9})".into();
                        y_label = "k\u{00B2}\u{03C7}(k)".into();
                    }
                }
                "r" => {
                    if let (Some(r), Some(chir_mag)) = (&spec.get_r(), &spec.get_chir_mag()) {
                        all_traces.push(main_trace(
                            r.iter().copied().collect(),
                            chir_mag.iter().copied().collect(),
                            format!("{} |\u{03C7}(R)|", name),
                            "r",
                        ));
                        x_label = "R (\u{00C5})".into();
                        y_label = "|\u{03C7}(R)| (\u{00C5}\u{207B}\u{00B3})".into();
                    }
                }
                _ => {}
            }
        }
    }

    if all_traces.is_empty() && !prep_errors.is_empty() {
        let details = prep_errors
            .iter()
            .take(5)
            .cloned()
            .collect::<Vec<_>>()
            .join(" | ");
        return Err(format!(
            "Failed to prepare all selected spectra for plotting. {}",
            details
        ));
    }

    Ok(PlotResult {
        traces: all_traces,
        pngs: Vec::new(),
        svgs: Vec::new(),
        x_label,
        y_label,
    })
}

#[tauri::command]
pub fn plot_core(
    state: State<'_, AppState>,
    index: usize,
    panels: Vec<String>,
    opts: Option<PipelineOptions>,
) -> Result<PlotResult, String> {
    let mut group = state.group.lock().map_err(|e| e.to_string())?;
    let spec = group.get_spectrum_mut(index).map_err(|e| e.to_string())?;

    let mut pngs = Vec::new();
    let mut svgs = Vec::new();
    let mut x_label = String::new();
    let mut y_label = String::new();

    for panel in &panels {
        ensure_panel_data(spec, panel, opts.as_ref()).map_err(|err| {
            format!(
                "Failed to prepare panel '{}' for core plot on spectrum #{}: {}",
                panel, index, err
            )
        })?;
        if x_label.is_empty() && y_label.is_empty() {
            let (x, y) = panel_labels(panel);
            x_label = x;
            y_label = y;
        }

        if let Some(png) = render_panel_png_data_url(spec, panel)? {
            pngs.push(png);
        }
        if let Some(svg) = render_panel_svg(spec, panel)? {
            svgs.push(svg);
        }
    }

    Ok(PlotResult {
        traces: Vec::new(),
        pngs,
        svgs,
        x_label,
        y_label,
    })
}

#[tauri::command]
pub fn plot_svg(
    state: State<'_, AppState>,
    index: usize,
    panels: Vec<String>,
) -> Result<Vec<String>, String> {
    let result = plot_core(state, index, panels, None)?;
    Ok(result.svgs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::{BgOptions, NormOptions};
    use std::path::Path;
    use xraytsubaki::prelude::io;

    fn load_test_spectrum() -> XASSpectrum {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let repo_root = manifest_dir
            .parent()
            .and_then(|p| p.parent())
            .expect("repo root");
        let test_file = repo_root.join("crates/xraytsubaki/tests/testfiles/Ru_QAS.dat");
        io::load_spectrum_QAS_trans(&test_file).expect("load Ru_QAS.dat")
    }

    #[test]
    fn k_panel_prep_produces_non_empty_main_trace() {
        let mut spec = load_test_spectrum();
        let opts = PipelineOptions {
            norm: Some(NormOptions::default()),
            bg: Some(BgOptions {
                rbkg: Some(1.0),
                kmin: Some(0.0),
                kmax: Some(15.0),
                kweight: Some(2),
            }),
            fft: None,
        };

        ensure_panel_data(&mut spec, "k", Some(&opts)).expect("prepare k panel");

        let k_len = k_axis_data(&spec).map(|v| v.len()).unwrap_or(0);
        let y_len = k_trace_data(&spec).map(|v| v.len()).unwrap_or(0);
        assert!(k_len > 0, "expected non-empty k axis after prep");
        assert!(y_len > 0, "expected non-empty chi(k) trace data after prep");
    }
}
