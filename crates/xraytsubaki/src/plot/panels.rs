use crate::plot::config::{PanelRenderData, TraceData};
use crate::plot::errors::PlotError;
use ruviz::core::{subplots, Plot, SubplotFigure};
use ruviz::render::{Color, LineStyle};

fn is_window_range_marker(label: &str, legend_group: Option<&str>) -> bool {
    label == "window range" || legend_group == Some("window range")
}

fn trace_style(trace: &TraceData) -> Option<LineStyle> {
    if !trace.dashed {
        return None;
    }
    if is_window_range_marker(&trace.label, trace.legend_group.as_deref()) {
        // Keep long dash/gap spacing so range markers stay visibly dashed at high DPI.
        return Some(LineStyle::Dashed.scaled(4.0));
    }
    Some(LineStyle::Dashed)
}

pub(crate) fn panel_grid(count: usize) -> Result<(usize, usize), PlotError> {
    match count {
        0 => Err(PlotError::EmptySelection),
        1 => Ok((1, 1)),
        2 => Ok((1, 2)),
        3 => Ok((1, 3)),
        4 => Ok((2, 2)),
        n => {
            let cols = (n as f64).sqrt().ceil() as usize;
            let rows = n.div_ceil(cols);
            Ok((rows, cols))
        }
    }
}

fn append_trace(plot: Plot, trace: TraceData) -> Plot {
    let style = trace_style(&trace);
    let mut series = plot.line(&trace.x, &trace.y);
    if !trace.label.is_empty() {
        series = series.label(trace.label);
    }
    if let Some((r, g, b)) = trace.color {
        series = series.color(Color::new(r, g, b));
    }
    if let Some(style) = style {
        series = series.style(style);
    }
    series.into()
}

fn append_trace_group(plot: Plot, group_label: String, traces: Vec<TraceData>) -> Plot {
    if traces.is_empty() {
        return plot;
    }

    let is_window_group = group_label == "window range";
    plot.group(|group| {
        let mut group = group.group_label(group_label);
        if let Some(first) = traces.first() {
            if let Some((r, g, b)) = first.color {
                group = group.color(Color::new(r, g, b));
            }
            if first.dashed {
                let style = if is_window_group {
                    LineStyle::Dashed.scaled(4.0)
                } else {
                    LineStyle::Dashed
                };
                group = group.line_style(style);
            }
        }

        for trace in traces {
            group = group.line(&trace.x, &trace.y);
        }
        group
    })
}

pub(crate) fn render_panel(data: PanelRenderData, show_legend: bool) -> Result<Plot, PlotError> {
    let PanelRenderData {
        title: _title,
        xlabel,
        ylabel,
        traces,
        xlim,
        ylim,
    } = data;

    if traces.is_empty() {
        return Err(PlotError::EmptySelection);
    }

    let mut plot = Plot::new();
    let mut traces = traces.into_iter().peekable();
    while let Some(trace) = traces.next() {
        if let Some(group_label) = trace.legend_group.clone() {
            let mut grouped = vec![trace];
            while let Some(next_trace) = traces.peek() {
                if next_trace.legend_group.as_deref() == Some(group_label.as_str()) {
                    let next = traces.next().expect("peek confirmed trace");
                    grouped.push(next);
                } else {
                    break;
                }
            }
            plot = append_trace_group(plot, group_label, grouped);
        } else {
            plot = append_trace(plot, trace);
        }
    }

    plot = plot.xlabel(xlabel).ylabel(ylabel).typst(true);

    if show_legend {
        plot = plot.legend_best();
    }

    if let Some((min, max)) = xlim {
        plot = plot.xlim(min, max);
    }

    if let Some((min, max)) = ylim {
        plot = plot.ylim(min, max);
    }

    Ok(plot)
}

pub(crate) fn build_subplot_figure(
    plots: Vec<Plot>,
    width: u32,
    height: u32,
    suptitle: Option<&str>,
) -> Result<SubplotFigure, PlotError> {
    let (rows, cols) = panel_grid(plots.len())?;
    let mut figure = subplots(rows, cols, width, height)?;

    if let Some(title) = suptitle {
        figure = figure.suptitle(title);
    }

    for (index, plot) in plots.into_iter().enumerate() {
        figure = figure.subplot_at(index, plot)?;
    }

    Ok(figure)
}
