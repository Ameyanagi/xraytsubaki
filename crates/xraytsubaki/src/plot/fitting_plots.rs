use crate::plot::config::{
    pad_ylim, r_component_ylabel, range_marker_traces, symmetric_ylim, truncate_pair,
    truncate_pair_in_xrange, FitPlotOptions, PanelKind, PanelRenderData, PanelSpec, TraceData,
    DEFAULT_R_XMAX,
};
use crate::plot::errors::PlotError;
use crate::xafs::fitting::types::{DatasetResult, FeffFitResult, PathContribution};
use nalgebra::DVector;

fn mag_from_components(re: &DVector<f64>, im: &DVector<f64>) -> DVector<f64> {
    let len = re.len().min(im.len());
    DVector::from_iterator(
        len,
        re.iter()
            .take(len)
            .zip(im.iter().take(len))
            .map(|(re, im)| (re * re + im * im).sqrt()),
    )
}

fn apply_kweight(k: &DVector<f64>, y: &DVector<f64>, kweight: f64) -> DVector<f64> {
    if kweight.abs() < f64::EPSILON {
        return y.clone();
    }
    let kpow = k.map(|value| value.powf(kweight));
    y.component_mul(&kpow)
}

enum FitView<'a> {
    Dataset(&'a DatasetResult),
    TopLevel(&'a FeffFitResult),
}

fn fit_view<'a>(
    fit: &'a FeffFitResult,
    options: &FitPlotOptions,
) -> Result<FitView<'a>, PlotError> {
    if let Some(index) = options.dataset {
        return fit
            .datasets
            .get(index)
            .map(FitView::Dataset)
            .ok_or(PlotError::IndexOutOfRange {
                index,
                len: fit.datasets.len(),
            });
    }

    if let Some(first) = fit.datasets.first() {
        return Ok(FitView::Dataset(first));
    }

    Ok(FitView::TopLevel(fit))
}

fn view_k_data<'a>(
    view: &'a FitView<'a>,
) -> (&'a DVector<f64>, &'a DVector<f64>, &'a DVector<f64>) {
    match view {
        FitView::Dataset(ds) => (&ds.k, &ds.data_chi, &ds.model_chi),
        FitView::TopLevel(fit) => (&fit.k, &fit.data_chi, &fit.model_chi),
    }
}

fn view_kweight(view: &FitView<'_>) -> f64 {
    match view {
        FitView::Dataset(ds) => ds.kweight,
        FitView::TopLevel(fit) => fit.kweight,
    }
}

fn view_r_data<'a>(
    view: &'a FitView<'a>,
) -> (
    &'a DVector<f64>,
    &'a DVector<f64>,
    &'a DVector<f64>,
    &'a DVector<f64>,
    &'a DVector<f64>,
) {
    match view {
        FitView::Dataset(ds) => (
            &ds.r,
            &ds.data_chir_re,
            &ds.data_chir_im,
            &ds.model_chir_re,
            &ds.model_chir_im,
        ),
        FitView::TopLevel(fit) => (
            &fit.r,
            &fit.data_chir_re,
            &fit.data_chir_im,
            &fit.model_chir_re,
            &fit.model_chir_im,
        ),
    }
}

fn view_model_mag<'a>(view: &'a FitView<'a>) -> &'a DVector<f64> {
    match view {
        FitView::Dataset(ds) => &ds.model_chir_mag,
        FitView::TopLevel(fit) => &fit.model_chir_mag,
    }
}

fn view_paths<'a>(view: &'a FitView<'a>) -> &'a [PathContribution] {
    match view {
        FitView::Dataset(ds) => &ds.path_contributions,
        FitView::TopLevel(fit) => &fit.path_contributions,
    }
}

fn view_kwin<'a>(view: &'a FitView<'a>) -> &'a DVector<f64> {
    match view {
        FitView::Dataset(ds) => &ds.kwin,
        FitView::TopLevel(fit) => &fit.kwin,
    }
}

fn view_k_range(view: &FitView<'_>) -> Option<(f64, f64)> {
    let (kmin, kmax) = match view {
        FitView::Dataset(ds) => (ds.kmin, ds.kmax),
        FitView::TopLevel(fit) => (fit.kmin, fit.kmax),
    };
    match (kmin, kmax) {
        (Some(min), Some(max)) if min.is_finite() && max.is_finite() && min < max => {
            Some((min, max))
        }
        _ => None,
    }
}

fn view_r_range(view: &FitView<'_>) -> Option<(f64, f64)> {
    let (rmin, rmax) = match view {
        FitView::Dataset(ds) => (ds.rmin, ds.rmax),
        FitView::TopLevel(fit) => (fit.rmin, fit.rmax),
    };
    match (rmin, rmax) {
        (Some(min), Some(max)) if min.is_finite() && max.is_finite() && min < max => {
            Some((min, max))
        }
        _ => None,
    }
}

fn y_extent(traces: &[TraceData]) -> Option<(f64, f64)> {
    let ymin = traces
        .iter()
        .flat_map(|trace| trace.y.iter())
        .fold(f64::INFINITY, |acc, value| acc.min(*value));
    let ymax = traces
        .iter()
        .flat_map(|trace| trace.y.iter())
        .fold(f64::NEG_INFINITY, |acc, value| acc.max(*value));

    if !ymin.is_finite() || !ymax.is_finite() {
        return None;
    }

    if (ymax - ymin).abs() <= f64::EPSILON {
        let pad = if ymax.abs() <= f64::EPSILON {
            1.0
        } else {
            ymax.abs() * 0.1
        };
        return Some((ymin - pad, ymax + pad));
    }

    Some((ymin, ymax))
}

pub(crate) fn extract_fit_panel_data(
    fit: &FeffFitResult,
    panel: &PanelSpec,
    options: &FitPlotOptions,
) -> Result<PanelRenderData, PlotError> {
    let view = fit_view(fit, options)?;

    match panel.kind {
        PanelKind::Mu | PanelKind::Norm => Err(PlotError::invalid_option(
            "fit results support only k() and r() panels",
        )),
        PanelKind::K => {
            let (k, data_chi, model_chi) = view_k_data(&view);
            if k.is_empty() {
                return Err(PlotError::MissingData { field: "fit k" });
            }

            let kweight = panel.kweight.unwrap_or_else(|| view_kweight(&view));
            let data = apply_kweight(k, data_chi, kweight);
            let model = apply_kweight(k, model_chi, kweight);
            let (x_data, y_data) = truncate_pair(k, &data);
            let (x_model, y_model) = truncate_pair(k, &model);

            let mut traces = vec![
                TraceData::new(x_data, y_data, "data", false),
                TraceData::new(x_model, y_model, "model", true),
            ];

            if options.paths {
                for path in view_paths(&view) {
                    let path_weighted = apply_kweight(k, &path.chi, kweight);
                    let (x_path, y_path) = truncate_pair(k, &path_weighted);
                    traces.push(TraceData::new(x_path, y_path, path.label.clone(), true));
                }
            }

            let base_ylim = symmetric_ylim(&traces);
            if panel.window_fn {
                let kwin = view_kwin(&view);
                if !kwin.is_empty() {
                    let (x_win, mut y_win) = truncate_pair(k, kwin);
                    let yscale = base_ylim.map(|(_, ymax)| ymax.abs()).unwrap_or(1.0);
                    y_win *= yscale;
                    traces.push(TraceData::new(x_win, y_win, "window fn", true));
                }
            }

            let marker_ylim = symmetric_ylim(&traces).or(base_ylim);
            if panel.window_box {
                let marker_ylim = marker_ylim
                    .and_then(|(ymin, ymax)| pad_ylim(ymin, ymax, 0.08))
                    .or(marker_ylim);
                if let (Some((kmin, kmax)), Some((ymin, ymax))) = (view_k_range(&view), marker_ylim)
                {
                    if let Some(marker_traces) =
                        range_marker_traces(kmin, kmax, ymin, ymax, "window")
                    {
                        traces.extend(marker_traces);
                    }
                }
            }

            let final_ylim = symmetric_ylim(&traces)
                .map(|(ymin, ymax)| {
                    let limit = ymin.abs().max(ymax.abs()) * 1.03;
                    (-limit, limit)
                })
                .or(marker_ylim);

            let mut panel_data =
                PanelRenderData::new(PanelKind::K.xlabel(), PanelKind::K.ylabel(kweight), traces);
            if let Some((min, max)) = final_ylim {
                panel_data = panel_data.with_ylim(min, max);
            }
            Ok(panel_data)
        }
        PanelKind::R => {
            let (r, data_re, data_im, model_re, model_im) = view_r_data(&view);
            if r.is_empty() {
                return Err(PlotError::MissingData { field: "fit r" });
            }
            let kweight = panel.kweight.unwrap_or_else(|| view_kweight(&view));

            let mut traces = Vec::new();
            let mut window_box_ylim = None;

            if panel.include_r_mag() {
                let data_mag = mag_from_components(data_re, data_im);
                let model_mag = {
                    let model_mag = view_model_mag(&view);
                    if model_mag.is_empty() {
                        mag_from_components(model_re, model_im)
                    } else {
                        model_mag.clone()
                    }
                };

                let (x_data_mag, y_data_mag) =
                    truncate_pair_in_xrange(r, &data_mag, 0.0, DEFAULT_R_XMAX);
                let (x_model_mag, y_model_mag) =
                    truncate_pair_in_xrange(r, &model_mag, 0.0, DEFAULT_R_XMAX);

                traces.push(TraceData::new(
                    x_data_mag,
                    y_data_mag,
                    "data |chi(R)|",
                    false,
                ));
                traces.push(TraceData::new(
                    x_model_mag,
                    y_model_mag,
                    "model |chi(R)|",
                    true,
                ));

                for (index, path) in view_paths(&view).iter().enumerate() {
                    let path_mag = if path.chir_mag.is_empty() {
                        mag_from_components(&path.chir_re, &path.chir_im)
                    } else {
                        path.chir_mag.clone()
                    };
                    let (x_path, y_path) =
                        truncate_pair_in_xrange(r, &path_mag, 0.0, DEFAULT_R_XMAX);
                    let label = if path.label.is_empty() {
                        format!("path[{index}] |chi(R)|")
                    } else {
                        format!("{} |chi(R)|", path.label)
                    };
                    traces.push(TraceData::new(x_path, y_path, label, true));
                }
            }

            if panel.r_real {
                let (x_data_re, y_data_re) =
                    truncate_pair_in_xrange(r, data_re, 0.0, DEFAULT_R_XMAX);
                let (x_model_re, y_model_re) =
                    truncate_pair_in_xrange(r, model_re, 0.0, DEFAULT_R_XMAX);

                traces.push(TraceData::new(
                    x_data_re,
                    y_data_re,
                    "data Re[chi(R)]",
                    true,
                ));
                traces.push(TraceData::new(
                    x_model_re,
                    y_model_re,
                    "model Re[chi(R)]",
                    true,
                ));
            }

            if panel.r_imag {
                let (x_data_im, y_data_im) =
                    truncate_pair_in_xrange(r, data_im, 0.0, DEFAULT_R_XMAX);
                let (x_model_im, y_model_im) =
                    truncate_pair_in_xrange(r, model_im, 0.0, DEFAULT_R_XMAX);

                traces.push(TraceData::new(
                    x_data_im,
                    y_data_im,
                    "data Im[chi(R)]",
                    true,
                ));
                traces.push(TraceData::new(
                    x_model_im,
                    y_model_im,
                    "model Im[chi(R)]",
                    true,
                ));
            }

            if panel.window_box {
                window_box_ylim =
                    y_extent(&traces).and_then(|(ymin, ymax)| pad_ylim(ymin, ymax, 0.08));
                if let (Some((mut rmin, mut rmax)), Some((ymin, ymax))) =
                    (view_r_range(&view), window_box_ylim)
                {
                    rmin = rmin.max(0.0);
                    rmax = rmax.min(DEFAULT_R_XMAX);
                    if let Some(marker_traces) =
                        range_marker_traces(rmin, rmax, ymin, ymax, "window")
                    {
                        traces.extend(marker_traces);
                    }
                }
            }

            let mut panel_data = PanelRenderData::new(
                PanelKind::R.xlabel(),
                r_component_ylabel(kweight, panel.include_r_mag(), panel.r_real, panel.r_imag),
                traces,
            )
            .with_xlim(0.0, DEFAULT_R_XMAX);

            if let Some((ymin, ymax)) = window_box_ylim {
                panel_data = panel_data.with_ylim(ymin, ymax);
            }

            Ok(panel_data)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dataset_index_is_validated() {
        let fit = FeffFitResult::default();
        let panel = PanelSpec::new(PanelKind::K);
        let options = FitPlotOptions {
            dataset: Some(2),
            paths: false,
        };
        let error = extract_fit_panel_data(&fit, &panel, &options).expect_err("must fail");
        assert!(matches!(
            error,
            PlotError::IndexOutOfRange { index: 2, len: 0 }
        ));
    }

    #[test]
    fn k_panel_defaults_to_dataset_kweight() {
        let mut fit = FeffFitResult::default();
        let mut dataset = DatasetResult::default();
        dataset.kweight = 3.0;
        dataset.k = DVector::from_vec(vec![1.0, 2.0]);
        dataset.data_chi = DVector::from_vec(vec![1.0, 1.0]);
        dataset.model_chi = DVector::from_vec(vec![0.5, 0.5]);
        fit.datasets.push(dataset);
        fit.sync_primary_dataset_fields();

        let panel = PanelSpec::new(PanelKind::K);
        let data = extract_fit_panel_data(&fit, &panel, &FitPlotOptions::default())
            .expect("panel should build");

        assert!(data.ylabel.contains("k^(3)") || data.ylabel.contains("k^3"));
        assert!(data.ylabel.contains("angstrom^(-3)"));
        assert!((data.traces[0].y[1] - 8.0).abs() < 1.0e-12);
        let (ymin, ymax) = data.ylim.expect("k panel must set y limits");
        assert!((ymin + ymax).abs() < 1.0e-12);
        assert!(ymax > 8.0);
    }

    #[test]
    fn r_panel_includes_path_magnitude_and_default_xlim() {
        let mut fit = FeffFitResult::default();
        let mut dataset = DatasetResult::default();
        dataset.r = DVector::from_vec(vec![0.0, 1.0, 2.0]);
        dataset.data_chir_re = DVector::from_vec(vec![1.0, 0.0, 0.0]);
        dataset.data_chir_im = DVector::from_vec(vec![0.0, 0.0, 0.0]);
        dataset.model_chir_re = DVector::from_vec(vec![0.8, 0.0, 0.0]);
        dataset.model_chir_im = DVector::from_vec(vec![0.0, 0.0, 0.0]);
        dataset.model_chir_mag = DVector::from_vec(vec![0.8, 0.0, 0.0]);
        dataset.path_contributions.push(PathContribution {
            label: "path-1".to_string(),
            chi: DVector::zeros(0),
            chir_re: DVector::zeros(0),
            chir_im: DVector::zeros(0),
            chir_mag: DVector::from_vec(vec![0.3, 0.1, 0.0]),
        });
        fit.datasets.push(dataset);
        fit.sync_primary_dataset_fields();

        let panel = PanelSpec::new(PanelKind::R);
        let data = extract_fit_panel_data(&fit, &panel, &FitPlotOptions::default())
            .expect("panel should build");

        assert_eq!(data.xlim, Some((0.0, DEFAULT_R_XMAX)));
        assert!(data
            .traces
            .iter()
            .any(|trace| trace.label == "path-1 |chi(R)|"));
    }

    #[test]
    fn r_panel_real_only_excludes_magnitude() {
        let mut fit = FeffFitResult::default();
        let mut dataset = DatasetResult::default();
        dataset.r = DVector::from_vec(vec![0.0, 1.0, 2.0]);
        dataset.data_chir_re = DVector::from_vec(vec![1.0, 0.0, 0.0]);
        dataset.data_chir_im = DVector::from_vec(vec![0.0, 0.0, 0.0]);
        dataset.model_chir_re = DVector::from_vec(vec![0.8, 0.0, 0.0]);
        dataset.model_chir_im = DVector::from_vec(vec![0.0, 0.0, 0.0]);
        dataset.model_chir_mag = DVector::from_vec(vec![0.8, 0.0, 0.0]);
        fit.datasets.push(dataset);
        fit.sync_primary_dataset_fields();

        let mut panel = PanelSpec::new(PanelKind::R);
        panel.r_mag = Some(false);
        panel.r_real = true;
        let data = extract_fit_panel_data(&fit, &panel, &FitPlotOptions::default())
            .expect("panel should build");

        assert!(data
            .traces
            .iter()
            .any(|trace| trace.label == "data Re[chi(R)]"));
        assert!(!data
            .traces
            .iter()
            .any(|trace| trace.label == "data |chi(R)|"));
    }

    #[test]
    fn k_panel_window_fn_and_box_are_supported() {
        let mut fit = FeffFitResult::default();
        let mut dataset = DatasetResult::default();
        dataset.k = DVector::from_vec(vec![0.0, 1.0, 2.0, 3.0]);
        dataset.data_chi = DVector::from_vec(vec![0.1, 0.4, -0.3, 0.2]);
        dataset.model_chi = DVector::from_vec(vec![0.1, 0.35, -0.25, 0.15]);
        dataset.kweight = 2.0;
        dataset.kwin = DVector::from_vec(vec![0.0, 0.5, 1.0, 0.0]);
        dataset.kmin = Some(1.0);
        dataset.kmax = Some(2.5);
        fit.datasets.push(dataset);
        fit.sync_primary_dataset_fields();

        let mut panel = PanelSpec::new(PanelKind::K);
        panel.window_fn = true;
        panel.window_box = true;
        let data = extract_fit_panel_data(&fit, &panel, &FitPlotOptions::default())
            .expect("panel should build");

        assert!(data.traces.iter().any(|trace| trace.label == "window fn"));
        let marker_count = data
            .traces
            .iter()
            .filter(|trace| trace.label == "window range")
            .count();
        assert_eq!(marker_count, 1);
    }

    #[test]
    fn r_panel_window_box_uses_fit_rrange() {
        let mut fit = FeffFitResult::default();
        let mut dataset = DatasetResult::default();
        dataset.r = DVector::from_vec(vec![0.0, 1.0, 2.0, 3.0]);
        dataset.data_chir_re = DVector::from_vec(vec![0.5, 0.1, 0.0, 0.0]);
        dataset.data_chir_im = DVector::from_vec(vec![0.0, 0.0, 0.0, 0.0]);
        dataset.model_chir_re = DVector::from_vec(vec![0.45, 0.08, 0.0, 0.0]);
        dataset.model_chir_im = DVector::from_vec(vec![0.0, 0.0, 0.0, 0.0]);
        dataset.model_chir_mag = DVector::from_vec(vec![0.45, 0.08, 0.0, 0.0]);
        dataset.rmin = Some(1.0);
        dataset.rmax = Some(2.5);
        fit.datasets.push(dataset);
        fit.sync_primary_dataset_fields();

        let mut panel = PanelSpec::new(PanelKind::R);
        panel.window_box = true;
        let data = extract_fit_panel_data(&fit, &panel, &FitPlotOptions::default())
            .expect("panel should build");

        let mut marker_positions = data
            .traces
            .iter()
            .filter(|trace| trace.label == "window range" || trace.label.is_empty())
            .map(|trace| trace.x[0])
            .collect::<Vec<_>>();
        marker_positions.sort_by(|a, b| a.partial_cmp(b).expect("positions should be finite"));
        assert_eq!(marker_positions, vec![1.0, 2.5]);
    }

    #[test]
    fn r_panel_ylabel_uses_dataset_kweight_units() {
        let mut fit = FeffFitResult::default();
        let mut dataset = DatasetResult::default();
        dataset.kweight = 3.0;
        dataset.r = DVector::from_vec(vec![0.0, 1.0, 2.0]);
        dataset.data_chir_re = DVector::from_vec(vec![1.0, 0.0, 0.0]);
        dataset.data_chir_im = DVector::from_vec(vec![0.0, 0.0, 0.0]);
        dataset.model_chir_re = DVector::from_vec(vec![0.8, 0.0, 0.0]);
        dataset.model_chir_im = DVector::from_vec(vec![0.0, 0.0, 0.0]);
        dataset.model_chir_mag = DVector::from_vec(vec![0.8, 0.0, 0.0]);
        fit.datasets.push(dataset);
        fit.sync_primary_dataset_fields();

        let panel = PanelSpec::new(PanelKind::R);
        let data = extract_fit_panel_data(&fit, &panel, &FitPlotOptions::default())
            .expect("panel should build");

        assert!(data.ylabel.contains("|chi(R)|"));
        assert!(data.ylabel.contains("angstrom^(-3)"));
    }
}
