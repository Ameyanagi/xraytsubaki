use crate::plot::config::{
    symmetric_ylim, GroupPlotOptions, PanelKind, PanelRenderData, PanelSpec, TraceData,
};
use crate::plot::errors::PlotError;
use crate::plot::spectrum_plots::extract_spectrum_panel_data;
use crate::xafs::xasgroup::XASGroup;

fn selected_indices(group: &XASGroup, options: &GroupPlotOptions) -> Result<Vec<usize>, PlotError> {
    if let Some(selected) = &options.selected {
        let len = group.spectra.len();
        for &index in selected {
            if index >= len {
                return Err(PlotError::IndexOutOfRange { index, len });
            }
        }
        if selected.is_empty() {
            return Err(PlotError::EmptySelection);
        }
        return Ok(selected.clone());
    }

    if group.spectra.is_empty() {
        return Err(PlotError::EmptySelection);
    }

    Ok((0..group.spectra.len()).collect())
}

pub(crate) fn extract_group_panel_data(
    group: &mut XASGroup,
    panel: &PanelSpec,
    options: &GroupPlotOptions,
) -> Result<PanelRenderData, PlotError> {
    let indices = selected_indices(group, options)?;

    let mut xlabel = String::new();
    let mut ylabel = String::new();
    let mut xlim = None;
    let mut ylim = None;
    let mut combined = Vec::new();
    let len = group.spectra.len();

    for (order, index) in indices.into_iter().enumerate() {
        let spectrum = group
            .spectra
            .get_mut(index)
            .ok_or(PlotError::IndexOutOfRange { index, len })?;
        let data = extract_spectrum_panel_data(spectrum, panel).map_err(|source| {
            if let PlotError::Xafs(err) = source {
                PlotError::SpectrumCompute { index, source: err }
            } else {
                source
            }
        })?;

        if order == 0 {
            xlabel = data.xlabel.clone();
            ylabel = data.ylabel.clone();
            xlim = data.xlim;
            ylim = data.ylim;
        }

        let spectrum_name = spectrum
            .name
            .clone()
            .unwrap_or_else(|| format!("spectrum[{index}]"));

        for trace in data.traces {
            let mut y = trace.y;
            if let Some(offset) = options.stacked {
                y = y.map(|value| value + offset * order as f64);
            }

            let label = if trace.label.is_empty() {
                String::new()
            } else {
                format!("{spectrum_name}: {}", trace.label)
            };
            let mut combined_trace = TraceData::new(trace.x, y, label, trace.dashed);
            if let Some(color) = trace.color {
                combined_trace = combined_trace.with_color(color);
            }
            if let Some(group) = trace.legend_group {
                combined_trace = combined_trace.with_legend_group(group);
            }
            combined.push(combined_trace);
        }
    }

    if panel.kind == PanelKind::K {
        ylim = symmetric_ylim(&combined);
    }

    let mut panel_data = PanelRenderData::new(xlabel, ylabel, combined);
    if let Some((min, max)) = xlim {
        panel_data = panel_data.with_xlim(min, max);
    }
    if let Some((min, max)) = ylim {
        panel_data = panel_data.with_ylim(min, max);
    }

    Ok(panel_data)
}
