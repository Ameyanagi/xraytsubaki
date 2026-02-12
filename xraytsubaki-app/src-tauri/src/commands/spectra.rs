use std::path::Path;

use tauri::{AppHandle, Emitter, State};
use xraytsubaki::prelude::*;

use crate::dto::{
    BatchError, BatchProgressEvent, BatchResult, BgOptions, FFTOptions, LoadError, LoadResult,
    NormOptions, PipelineOptions, SpectrumData, SpectrumMeta,
};
use crate::state::AppState;

fn dvec_to_vec(v: &Option<nalgebra::DVector<f64>>) -> Option<Vec<f64>> {
    v.as_ref().map(|d| d.iter().copied().collect())
}

#[tauri::command]
pub fn load_spectra(state: State<'_, AppState>, paths: Vec<String>) -> Result<LoadResult, String> {
    let mut group = state.group.lock().map_err(|e| e.to_string())?;
    let mut loaded = 0;
    let mut errors = Vec::new();

    for path_str in &paths {
        let path = Path::new(path_str);
        match io::load_spectrum_QAS_trans(path) {
            Ok(mut spectrum) => {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| path_str.clone());
                spectrum.set_name(name);
                group.add_spectrum(spectrum);
                loaded += 1;
            }
            Err(e) => {
                errors.push(LoadError {
                    path: path_str.clone(),
                    message: e.to_string(),
                });
            }
        }
    }

    Ok(LoadResult { loaded, errors })
}

#[tauri::command]
pub fn get_spectrum_list(state: State<'_, AppState>) -> Result<Vec<SpectrumMeta>, String> {
    let group = state.group.lock().map_err(|e| e.to_string())?;
    let mut list = Vec::with_capacity(group.len());

    for i in 0..group.len() {
        if let Ok(spec) = group.get_spectrum(i) {
            list.push(SpectrumMeta {
                index: i,
                name: spec
                    .name
                    .clone()
                    .unwrap_or_else(|| format!("Spectrum {}", i)),
                has_e0: spec.e0.is_some(),
                has_norm: spec
                    .normalization
                    .as_ref()
                    .map(|n| n.get_norm().is_some())
                    .unwrap_or(false),
                has_chi: spec.chi.is_some(),
                has_chir: spec.chi_r_mag.is_some(),
            });
        }
    }

    Ok(list)
}

#[tauri::command]
pub fn get_spectrum_data(state: State<'_, AppState>, index: usize) -> Result<SpectrumData, String> {
    let group = state.group.lock().map_err(|e| e.to_string())?;
    let spec = group.get_spectrum(index).map_err(|e| e.to_string())?;

    Ok(SpectrumData {
        index,
        name: spec
            .name
            .clone()
            .unwrap_or_else(|| format!("Spectrum {}", index)),
        energy: dvec_to_vec(&spec.energy),
        mu: dvec_to_vec(&spec.mu),
        e0: spec.e0,
        norm: spec.get_norm().map(|v| v.iter().copied().collect()),
        flat: spec.get_flat().map(|v| v.iter().copied().collect()),
        k: dvec_to_vec(&spec.k),
        chi: dvec_to_vec(&spec.chi),
        chi_kweighted: spec
            .get_chi_kweighted()
            .map(|v| v.iter().copied().collect()),
        r: spec.get_r().map(|v| v.iter().copied().collect()),
        chir_mag: spec.get_chir_mag().map(|v| v.iter().copied().collect()),
        chir_re: spec.get_chir_real().map(|v| v.iter().copied().collect()),
        chir_im: spec.get_chir_imag().map(|v| v.iter().copied().collect()),
        q: spec.get_q().map(|v| v.iter().copied().collect()),
        chiq: spec.get_chiq().map(|v| v.iter().copied().collect()),
        kwin: spec.get_kwin().map(|v| v.iter().copied().collect()),
        pre_edge: spec.get_pre_edge().map(|v| v.iter().copied().collect()),
        post_edge: spec.get_post_edge().map(|v| v.iter().copied().collect()),
    })
}

#[tauri::command]
pub fn find_e0(state: State<'_, AppState>, index: usize) -> Result<f64, String> {
    let mut group = state.group.lock().map_err(|e| e.to_string())?;
    let spec = group.get_spectrum_mut(index).map_err(|e| e.to_string())?;
    spec.find_e0().map_err(|e| e.to_string())?;
    Ok(spec.e0.unwrap_or(0.0))
}

fn apply_norm_options(spec: &mut XASSpectrum, opts: &NormOptions) -> Result<(), String> {
    let mut method = NormalizationMethod::new_prepostedge();
    if let NormalizationMethod::PrePostEdge(ref mut ppe) = method {
        if let Some(e0) = opts.e0 {
            ppe.e0 = Some(e0);
        }
        if let Some(v) = opts.pre_edge_start {
            ppe.pre_edge_start = Some(v);
        }
        if let Some(v) = opts.pre_edge_end {
            ppe.pre_edge_end = Some(v);
        }
        if let Some(v) = opts.norm_start {
            ppe.norm_start = Some(v);
        }
        if let Some(v) = opts.norm_end {
            ppe.norm_end = Some(v);
        }
        if let Some(v) = opts.norm_polyorder {
            ppe.norm_polyorder = Some(v);
        }
    }
    spec.set_normalization_method(Some(method))
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn apply_bg_options(spec: &mut XASSpectrum, opts: &BgOptions) -> Result<(), String> {
    let mut autobk = AUTOBK::new();
    if let Some(v) = opts.rbkg {
        autobk.rbkg = Some(v);
    }
    if let Some(v) = opts.kmin {
        autobk.kmin = Some(v);
    }
    if let Some(v) = opts.kmax {
        autobk.kmax = Some(v);
    }
    if let Some(v) = opts.kweight {
        autobk.kweight = Some(v);
    }
    spec.set_background_method(Some(BackgroundMethod::AUTOBK(autobk)))
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn apply_fft_options(spec: &mut XASSpectrum, opts: &FFTOptions) -> Result<(), String> {
    let mut fftf = XrayFFTF::new();
    if let Some(v) = opts.kmin {
        fftf.kmin = Some(v);
    }
    if let Some(v) = opts.kmax {
        fftf.kmax = Some(v);
    }
    if let Some(v) = opts.kweight {
        fftf.kweight = Some(v);
    }
    if let Some(v) = opts.dk {
        fftf.dk = Some(v);
    }
    if let Some(ref w) = opts.window {
        fftf.window = Some(match w.to_lowercase().as_str() {
            "hanning" => FTWindow::Hanning,
            "parzen" => FTWindow::Parzen,
            "welch" => FTWindow::Welch,
            "gaussian" => FTWindow::Gaussian,
            "sine" => FTWindow::Sine,
            "kaiserbessel" | "kaiser-bessel" => FTWindow::KaiserBessel,
            _ => FTWindow::Hanning,
        });
    }
    spec.xftf = Some(fftf);
    Ok(())
}

#[tauri::command]
pub fn normalize(
    state: State<'_, AppState>,
    index: usize,
    opts: Option<NormOptions>,
) -> Result<(), String> {
    let mut group = state.group.lock().map_err(|e| e.to_string())?;
    let spec = group.get_spectrum_mut(index).map_err(|e| e.to_string())?;
    if let Some(ref o) = opts {
        apply_norm_options(spec, o)?;
    }
    spec.normalize().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn calc_background(
    state: State<'_, AppState>,
    index: usize,
    opts: Option<BgOptions>,
) -> Result<(), String> {
    let mut group = state.group.lock().map_err(|e| e.to_string())?;
    let spec = group.get_spectrum_mut(index).map_err(|e| e.to_string())?;
    if let Some(ref o) = opts {
        apply_bg_options(spec, o)?;
    }
    spec.calc_background().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn fft(
    state: State<'_, AppState>,
    index: usize,
    opts: Option<FFTOptions>,
) -> Result<(), String> {
    let mut group = state.group.lock().map_err(|e| e.to_string())?;
    let spec = group.get_spectrum_mut(index).map_err(|e| e.to_string())?;
    if let Some(ref o) = opts {
        apply_fft_options(spec, o)?;
    }
    spec.fft().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn run_pipeline(
    state: State<'_, AppState>,
    index: usize,
    opts: Option<PipelineOptions>,
) -> Result<(), String> {
    let mut group = state.group.lock().map_err(|e| e.to_string())?;
    let spec = group.get_spectrum_mut(index).map_err(|e| e.to_string())?;

    if let Some(ref pipeline) = opts {
        if let Some(ref norm) = pipeline.norm {
            apply_norm_options(spec, norm)?;
        }
        if let Some(ref bg) = pipeline.bg {
            apply_bg_options(spec, bg)?;
        }
        if let Some(ref fft_opts) = pipeline.fft {
            apply_fft_options(spec, fft_opts)?;
        }
    }

    spec.find_e0().map_err(|e| e.to_string())?;
    spec.normalize().map_err(|e| e.to_string())?;
    spec.calc_background().map_err(|e| e.to_string())?;
    spec.fft().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn batch_process(
    app: AppHandle,
    state: State<'_, AppState>,
    indices: Vec<usize>,
    opts: Option<PipelineOptions>,
) -> Result<BatchResult, String> {
    let mut group = state.group.lock().map_err(|e| e.to_string())?;
    let mut succeeded = 0;
    let mut errors = Vec::new();
    let total = indices.len();

    for (position, &idx) in indices.iter().enumerate() {
        let result = (|| -> Result<(), String> {
            let spec = group.get_spectrum_mut(idx).map_err(|e| e.to_string())?;

            if let Some(ref pipeline) = opts {
                if let Some(ref norm) = pipeline.norm {
                    apply_norm_options(spec, norm)?;
                }
                if let Some(ref bg) = pipeline.bg {
                    apply_bg_options(spec, bg)?;
                }
                if let Some(ref fft_opts) = pipeline.fft {
                    apply_fft_options(spec, fft_opts)?;
                }
            }

            spec.find_e0().map_err(|e| e.to_string())?;
            spec.normalize().map_err(|e| e.to_string())?;
            spec.calc_background().map_err(|e| e.to_string())?;
            spec.fft().map_err(|e| e.to_string())?;
            Ok(())
        })();

        match result {
            Ok(()) => succeeded += 1,
            Err(msg) => {
                errors.push(BatchError {
                    index: idx,
                    name: group
                        .get_spectrum(idx)
                        .ok()
                        .and_then(|s| s.name.clone())
                        .unwrap_or_else(|| format!("Spectrum {}", idx)),
                    message: msg,
                });
            }
        }

        let progress = BatchProgressEvent {
            current: position + 1,
            total,
            succeeded,
            failed: errors.len(),
            index: idx,
            name: group
                .get_spectrum(idx)
                .ok()
                .and_then(|s| s.name.clone())
                .unwrap_or_else(|| format!("Spectrum {}", idx)),
        };

        if let Err(err) = app.emit("batch-progress", &progress) {
            eprintln!("[batch-progress] emit failed: {err}");
        }
    }

    Ok(BatchResult {
        succeeded,
        failed: errors.len(),
        errors,
    })
}

#[tauri::command]
pub fn remove_spectra(state: State<'_, AppState>, indices: Vec<usize>) -> Result<usize, String> {
    let mut group = state.group.lock().map_err(|e| e.to_string())?;
    group.remove_spectra(&indices).map_err(|e| e.to_string())?;
    Ok(group.len())
}
