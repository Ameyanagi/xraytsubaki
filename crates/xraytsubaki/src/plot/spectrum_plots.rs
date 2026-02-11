use crate::plot::config::{
    pad_ylim, r_component_ylabel, range_marker_traces, symmetric_ylim, truncate_pair,
    truncate_pair_in_xrange, PanelKind, PanelRenderData, PanelSpec, TraceData, DEFAULT_KWEIGHT,
    DEFAULT_R_XMAX,
};
use crate::plot::errors::PlotError;
use crate::xafs::xasspectrum::XASSpectrum;
use nalgebra::DVector;

fn ensure_background(spectrum: &mut XASSpectrum) -> Result<(), PlotError> {
    if spectrum.get_k().is_none() || spectrum.get_chi().is_none() {
        spectrum.calc_background()?;
    }
    Ok(())
}

fn ensure_norm(spectrum: &mut XASSpectrum) -> Result<(), PlotError> {
    if spectrum.get_norm().is_none() {
        spectrum.normalize()?;
    }
    Ok(())
}

fn ensure_fft(spectrum: &mut XASSpectrum) -> Result<(), PlotError> {
    if spectrum.get_r().is_none() || spectrum.get_chir_mag().is_none() {
        ensure_background(spectrum)?;
        spectrum.fft()?;
    }
    Ok(())
}

fn spectrum_label(spectrum: &XASSpectrum) -> String {
    spectrum
        .name
        .clone()
        .unwrap_or_else(|| "spectrum".to_string())
}

fn weighted_chi(k: &DVector<f64>, chi: &DVector<f64>, kweight: f64) -> DVector<f64> {
    if kweight.abs() < f64::EPSILON {
        return chi.clone();
    }

    let kpow = k.map(|v| v.powf(kweight));
    chi.component_mul(&kpow)
}

fn spectrum_window_krange(spectrum: &XASSpectrum, k: &DVector<f64>) -> Option<(f64, f64)> {
    if let Some(xftf) = spectrum.xftf.as_ref() {
        if let (Some(kmin), Some(kmax)) = (xftf.get_kmin(), xftf.get_kmax()) {
            if *kmin < *kmax && kmin.is_finite() && kmax.is_finite() {
                return Some((*kmin, *kmax));
            }
        }
    }

    let min = k
        .iter()
        .copied()
        .fold(f64::INFINITY, |acc, value| acc.min(value));
    let max = k
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, |acc, value| acc.max(value));
    (min < max && min.is_finite() && max.is_finite()).then_some((min, max))
}

pub(crate) fn extract_spectrum_panel_data(
    spectrum: &mut XASSpectrum,
    panel: &PanelSpec,
) -> Result<PanelRenderData, PlotError> {
    match panel.kind {
        PanelKind::Mu => {
            let energy = spectrum
                .energy
                .clone()
                .ok_or(PlotError::MissingData { field: "energy" })?;

            let mut title = PanelKind::Mu.title().to_string();
            let mut ylabel = PanelKind::Mu.ylabel(DEFAULT_KWEIGHT);

            let y_values = if let Some(flat) = spectrum.get_flat() {
                flat
            } else {
                let mut flattened = None;
                if energy.len() >= 8 && ensure_norm(spectrum).is_ok() {
                    flattened = spectrum.get_flat();
                }

                if let Some(flat) = flattened {
                    flat
                } else {
                    title = "mu(E)".to_string();
                    ylabel = "mu(E)".to_string();
                    spectrum
                        .mu
                        .clone()
                        .ok_or(PlotError::MissingData { field: "mu" })?
                }
            };

            let (x, y) = truncate_pair(&energy, &y_values);
            Ok(PanelRenderData::new(
                title,
                PanelKind::Mu.xlabel(),
                ylabel,
                vec![TraceData::new(x, y, spectrum_label(spectrum), false)],
            ))
        }
        PanelKind::Norm => {
            ensure_norm(spectrum)?;
            let energy = spectrum
                .energy
                .clone()
                .ok_or(PlotError::MissingData { field: "energy" })?;
            let norm = spectrum
                .get_norm()
                .ok_or(PlotError::MissingData { field: "norm" })?;

            let (x, y) = truncate_pair(&energy, &norm);
            let mut traces = vec![TraceData::new(
                x.clone(),
                y,
                spectrum_label(spectrum),
                false,
            )];

            if panel.edges {
                if let Some(pre_edge) = spectrum.get_pre_edge() {
                    let (x_pre, y_pre) = truncate_pair(&energy, &pre_edge);
                    traces.push(TraceData::new(x_pre, y_pre, "pre-edge", true));
                }
                if let Some(post_edge) = spectrum.get_post_edge() {
                    let (x_post, y_post) = truncate_pair(&energy, &post_edge);
                    traces.push(TraceData::new(x_post, y_post, "post-edge", true));
                }
            }

            Ok(PanelRenderData::new(
                PanelKind::Norm.title(),
                PanelKind::Norm.xlabel(),
                PanelKind::Norm.ylabel(DEFAULT_KWEIGHT),
                traces,
            ))
        }
        PanelKind::K => {
            ensure_background(spectrum)?;

            let k = spectrum
                .get_k()
                .ok_or(PlotError::MissingData { field: "k" })?;
            let chi = spectrum
                .get_chi()
                .ok_or(PlotError::MissingData { field: "chi" })?;
            let kweight = panel.kweight.unwrap_or(DEFAULT_KWEIGHT);

            let weighted = weighted_chi(&k, &chi, kweight);
            let (x, y) = truncate_pair(&k, &weighted);
            let mut traces = vec![TraceData::new(
                x.clone(),
                y.clone(),
                spectrum_label(spectrum),
                false,
            )];

            let max_abs = y
                .iter()
                .fold(0.0_f64, |acc, v| if v.abs() > acc { v.abs() } else { acc })
                .max(1.0);

            if panel.window_fn || panel.window_box {
                ensure_fft(spectrum)?;
            }

            if panel.window_fn {
                if let Some(window) = spectrum.get_kwin() {
                    let (xw, mut yw) = truncate_pair(&k, &window);
                    yw *= max_abs;
                    traces.push(TraceData::new(xw, yw, "window fn", true));
                }
            }

            if panel.window_box {
                let base_marker_ylim = symmetric_ylim(&traces).unwrap_or((-max_abs, max_abs));
                let marker_ylim = pad_ylim(base_marker_ylim.0, base_marker_ylim.1, 0.08)
                    .unwrap_or(base_marker_ylim);
                if let Some((kmin, kmax)) = spectrum_window_krange(spectrum, &k) {
                    if let Some(marker_traces) =
                        range_marker_traces(kmin, kmax, marker_ylim.0, marker_ylim.1, "window")
                    {
                        traces.extend(marker_traces);
                    }
                }
            }

            let ylim = symmetric_ylim(&traces).map(|(ymin, ymax)| {
                let limit = ymin.abs().max(ymax.abs()) * 1.03;
                (-limit, limit)
            });
            let mut data = PanelRenderData::new(
                PanelKind::K.title(),
                PanelKind::K.xlabel(),
                PanelKind::K.ylabel(kweight),
                traces,
            );
            if let Some((min, max)) = ylim {
                data = data.with_ylim(min, max);
            }
            Ok(data)
        }
        PanelKind::R => {
            ensure_fft(spectrum)?;
            let r = spectrum
                .get_r()
                .ok_or(PlotError::MissingData { field: "r" })?;
            let kweight = spectrum.get_kweight().copied().unwrap_or(DEFAULT_KWEIGHT);
            let mut traces = Vec::new();

            if panel.include_r_mag() {
                let chir_mag = spectrum
                    .get_chir_mag()
                    .ok_or(PlotError::MissingData { field: "chi_r_mag" })?;
                let (x, y) = truncate_pair_in_xrange(&r, &chir_mag, 0.0, DEFAULT_R_XMAX);
                traces.push(TraceData::new(x, y, spectrum_label(spectrum), false));
            }

            if panel.r_real {
                let chir_re = spectrum
                    .get_chir_real()
                    .ok_or(PlotError::MissingData { field: "chi_r_re" })?;
                let (x_re, y_re) = truncate_pair_in_xrange(&r, &chir_re, 0.0, DEFAULT_R_XMAX);
                traces.push(TraceData::new(x_re, y_re, "Re[chi(R)]", true));
            }

            if panel.r_imag {
                let chir_im = spectrum
                    .get_chir_imag()
                    .ok_or(PlotError::MissingData { field: "chi_r_im" })?;
                let (x_im, y_im) = truncate_pair_in_xrange(&r, &chir_im, 0.0, DEFAULT_R_XMAX);
                traces.push(TraceData::new(x_im, y_im, "Im[chi(R)]", true));
            }

            Ok(PanelRenderData::new(
                PanelKind::R.title(),
                PanelKind::R.xlabel(),
                r_component_ylabel(kweight, panel.include_r_mag(), panel.r_real, panel.r_imag),
                traces,
            )
            .with_xlim(0.0, DEFAULT_R_XMAX))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xafs::io::load_spectrum_QAS_trans;

    #[test]
    fn r_panel_triggers_compute_chain() {
        let path = format!("{}/tests/testfiles/Ru_QAS.dat", env!("CARGO_MANIFEST_DIR"));
        let mut spectrum = load_spectrum_QAS_trans(path).expect("fixture should load");
        assert!(spectrum.get_r().is_none());

        let panel = PanelSpec::new(PanelKind::R);
        let data = extract_spectrum_panel_data(&mut spectrum, &panel).expect("panel should build");
        assert!(!data.traces.is_empty());
        assert_eq!(data.xlim, Some((0.0, DEFAULT_R_XMAX)));
        assert!(spectrum.get_r().is_some());
    }

    #[test]
    fn missing_mu_data_returns_error() {
        let mut spectrum = XASSpectrum::new();
        let panel = PanelSpec::new(PanelKind::Mu);
        let error = extract_spectrum_panel_data(&mut spectrum, &panel).expect_err("must fail");
        assert!(matches!(
            error,
            PlotError::MissingData { field: "energy" } | PlotError::MissingData { field: "mu" }
        ));
    }

    #[test]
    fn mu_panel_uses_flattened_signal() {
        let path = format!("{}/tests/testfiles/Ru_QAS.dat", env!("CARGO_MANIFEST_DIR"));
        let mut spectrum = load_spectrum_QAS_trans(path).expect("fixture should load");

        let panel = PanelSpec::new(PanelKind::Mu);
        let data = extract_spectrum_panel_data(&mut spectrum, &panel).expect("panel should build");
        let flat = spectrum.get_flat().expect("flat must be computed");
        let trace = data.traces.first().expect("trace should exist");

        assert_eq!(data.ylabel, "Flattened $mu(E)$");
        assert_eq!(trace.y.len(), flat.len().min(trace.x.len()));
        assert!((trace.y[10] - flat[10]).abs() < 1.0e-12);
    }

    #[test]
    fn mu_panel_falls_back_to_raw_for_tiny_input() {
        let mut spectrum = XASSpectrum::new();
        spectrum.set_spectrum(
            DVector::from_vec(vec![1.0, 2.0, 3.0, 4.0]),
            DVector::from_vec(vec![2.0, 2.5, 3.0, 3.5]),
        );

        let panel = PanelSpec::new(PanelKind::Mu);
        let data = extract_spectrum_panel_data(&mut spectrum, &panel).expect("panel should build");

        assert_eq!(data.ylabel, "mu(E)");
        assert!((data.traces[0].y[0] - 2.0).abs() < 1.0e-12);
    }

    #[test]
    fn k_panel_sets_symmetric_ylim() {
        let path = format!("{}/tests/testfiles/Ru_QAS.dat", env!("CARGO_MANIFEST_DIR"));
        let mut spectrum = load_spectrum_QAS_trans(path).expect("fixture should load");

        let panel = PanelSpec::new(PanelKind::K);
        let data = extract_spectrum_panel_data(&mut spectrum, &panel).expect("panel should build");
        let (ymin, ymax) = data.ylim.expect("k panel must set y limits");

        assert!((ymin + ymax).abs() < 1.0e-12);
        assert!(ymax > 0.0);
    }

    #[test]
    fn r_panel_real_only_replaces_default_magnitude() {
        let path = format!("{}/tests/testfiles/Ru_QAS.dat", env!("CARGO_MANIFEST_DIR"));
        let mut spectrum = load_spectrum_QAS_trans(path).expect("fixture should load");

        let mut panel = PanelSpec::new(PanelKind::R);
        panel.r_mag = Some(false);
        panel.r_real = true;
        let data = extract_spectrum_panel_data(&mut spectrum, &panel).expect("panel should build");

        assert!(data.traces.iter().any(|trace| trace.label == "Re[chi(R)]"));
        assert!(!data.traces.iter().any(|trace| trace.label == "Im[chi(R)]"));
        assert_eq!(data.traces.len(), 1);
    }

    #[test]
    fn r_panel_mag_real_imag_includes_all_channels() {
        let path = format!("{}/tests/testfiles/Ru_QAS.dat", env!("CARGO_MANIFEST_DIR"));
        let mut spectrum = load_spectrum_QAS_trans(path).expect("fixture should load");

        let mut panel = PanelSpec::new(PanelKind::R);
        panel.r_mag = Some(true);
        panel.r_real = true;
        panel.r_imag = true;
        let data = extract_spectrum_panel_data(&mut spectrum, &panel).expect("panel should build");

        assert_eq!(data.traces.len(), 3);
        assert!(data
            .traces
            .iter()
            .any(|trace| trace.label.contains("spectrum")));
        assert!(data.traces.iter().any(|trace| trace.label == "Re[chi(R)]"));
        assert!(data.traces.iter().any(|trace| trace.label == "Im[chi(R)]"));
    }
}
