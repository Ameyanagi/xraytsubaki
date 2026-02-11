use nalgebra::DVector;

pub const DEFAULT_WIDTH: u32 = 800;
pub const DEFAULT_HEIGHT: u32 = 600;
pub const DEFAULT_DPI: u32 = 300;
pub const DEFAULT_KWEIGHT: f64 = 2.0;
pub const DEFAULT_R_XMAX: f64 = 6.0;
pub const WINDOW_MARKER_COLOR: (u8, u8, u8) = (214, 39, 40);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelKind {
    Mu,
    Norm,
    K,
    R,
}

impl PanelKind {
    pub fn title(self) -> &'static str {
        match self {
            Self::Mu => "Flattened mu(E)",
            Self::Norm => "Normalized mu(E)",
            Self::K => "chi(k)",
            Self::R => "|chi(R)|",
        }
    }

    pub fn xlabel(self) -> &'static str {
        match self {
            Self::Mu | Self::Norm => "Energy [eV]",
            Self::K => "$k$ [$angstrom^(-1)$]",
            Self::R => "$R$ [$angstrom$]",
        }
    }

    pub fn ylabel(self, kweight: f64) -> String {
        match self {
            Self::Mu => "Flattened $mu(E)$".to_string(),
            Self::Norm => "Normalized $mu(E)$".to_string(),
            Self::K => {
                let weight = fmt_kweight(kweight);
                format!("$k^({weight}) chi(k)$ [$angstrom^(-{weight})$]")
            }
            Self::R => r_component_ylabel(kweight, true, false, false),
        }
    }
}

pub fn r_component_ylabel(
    kweight: f64,
    show_mag: bool,
    show_real: bool,
    show_imag: bool,
) -> String {
    let weight = fmt_kweight(kweight);
    let head = match (show_mag, show_real, show_imag) {
        (true, false, false) => "$|chi(R)|$",
        (false, true, false) => "$Re[chi(R)]$",
        (false, false, true) => "$Im[chi(R)]$",
        _ => "$chi(R)$",
    };
    format!("{head} [$angstrom^(-{weight})$]")
}

fn fmt_kweight(weight: f64) -> String {
    let rounded = weight.round();
    if (weight - rounded).abs() < 1.0e-9 {
        format!("{rounded:.0}")
    } else {
        let mut out = format!("{weight:.3}");
        while out.ends_with('0') {
            out.pop();
        }
        if out.ends_with('.') {
            out.pop();
        }
        out
    }
}

#[derive(Debug, Clone)]
pub struct PanelSpec {
    pub kind: PanelKind,
    pub kweight: Option<f64>,
    pub r_mag: Option<bool>,
    pub r_real: bool,
    pub r_imag: bool,
    pub edges: bool,
    pub window_fn: bool,
    pub window_box: bool,
}

impl PanelSpec {
    pub fn new(kind: PanelKind) -> Self {
        Self {
            kind,
            kweight: None,
            r_mag: None,
            r_real: false,
            r_imag: false,
            edges: false,
            window_fn: false,
            window_box: false,
        }
    }

    pub fn include_r_mag(&self) -> bool {
        self.r_mag.unwrap_or(!(self.r_real || self.r_imag))
    }
}

#[derive(Debug, Clone)]
pub struct PlotConfig {
    pub width: u32,
    pub height: u32,
    pub dpi: u32,
    pub title: Option<String>,
    pub show_legend: bool,
}

impl Default for PlotConfig {
    fn default() -> Self {
        Self {
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
            dpi: DEFAULT_DPI,
            title: None,
            show_legend: true,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct GroupPlotOptions {
    pub stacked: Option<f64>,
    pub selected: Option<Vec<usize>>,
}

#[derive(Debug, Clone, Default)]
pub struct FitPlotOptions {
    pub dataset: Option<usize>,
    pub paths: bool,
}

#[derive(Debug, Clone)]
pub struct TraceData {
    pub x: DVector<f64>,
    pub y: DVector<f64>,
    pub label: String,
    pub dashed: bool,
    pub color: Option<(u8, u8, u8)>,
    pub legend_group: Option<String>,
}

impl TraceData {
    pub fn new(x: DVector<f64>, y: DVector<f64>, label: impl Into<String>, dashed: bool) -> Self {
        Self {
            x,
            y,
            label: label.into(),
            dashed,
            color: None,
            legend_group: None,
        }
    }

    pub fn with_color(mut self, color: (u8, u8, u8)) -> Self {
        self.color = Some(color);
        self
    }

    pub fn with_legend_group(mut self, group: impl Into<String>) -> Self {
        self.legend_group = Some(group.into());
        self
    }
}

#[derive(Debug, Clone)]
pub struct PanelRenderData {
    pub title: String,
    pub xlabel: String,
    pub ylabel: String,
    pub traces: Vec<TraceData>,
    pub xlim: Option<(f64, f64)>,
    pub ylim: Option<(f64, f64)>,
}

impl PanelRenderData {
    pub fn new(
        title: impl Into<String>,
        xlabel: impl Into<String>,
        ylabel: impl Into<String>,
        traces: Vec<TraceData>,
    ) -> Self {
        Self {
            title: title.into(),
            xlabel: xlabel.into(),
            ylabel: ylabel.into(),
            traces,
            xlim: None,
            ylim: None,
        }
    }

    pub fn with_xlim(mut self, min: f64, max: f64) -> Self {
        if min < max && min.is_finite() && max.is_finite() {
            self.xlim = Some((min, max));
        }
        self
    }

    pub fn with_ylim(mut self, min: f64, max: f64) -> Self {
        if min < max && min.is_finite() && max.is_finite() {
            self.ylim = Some((min, max));
        }
        self
    }
}

pub fn truncate_pair(x: &DVector<f64>, y: &DVector<f64>) -> (DVector<f64>, DVector<f64>) {
    let len = x.len().min(y.len());
    (
        DVector::from_iterator(len, x.iter().take(len).copied()),
        DVector::from_iterator(len, y.iter().take(len).copied()),
    )
}

pub fn truncate_pair_in_xrange(
    x: &DVector<f64>,
    y: &DVector<f64>,
    xmin: f64,
    xmax: f64,
) -> (DVector<f64>, DVector<f64>) {
    let len = x.len().min(y.len());
    let filtered = x
        .iter()
        .take(len)
        .zip(y.iter().take(len))
        .filter(|(xv, _)| **xv >= xmin && **xv <= xmax)
        .map(|(xv, yv)| (*xv, *yv))
        .collect::<Vec<_>>();

    let xs = DVector::from_iterator(filtered.len(), filtered.iter().map(|(xv, _)| *xv));
    let ys = DVector::from_iterator(filtered.len(), filtered.iter().map(|(_, yv)| *yv));
    (xs, ys)
}

pub fn symmetric_ylim(traces: &[TraceData]) -> Option<(f64, f64)> {
    let max_abs = traces
        .iter()
        .flat_map(|trace| trace.y.iter())
        .fold(0.0_f64, |acc, value| acc.max(value.abs()));

    if !max_abs.is_finite() {
        return None;
    }

    let limit = if max_abs <= f64::EPSILON {
        1.0
    } else {
        max_abs
    };
    Some((-limit, limit))
}

pub fn pad_ylim(ymin: f64, ymax: f64, fraction: f64) -> Option<(f64, f64)> {
    if !(ymin.is_finite() && ymax.is_finite() && ymin < ymax) {
        return None;
    }

    let span = ymax - ymin;
    let pad = if span <= f64::EPSILON {
        1.0
    } else {
        span * fraction.max(0.0)
    };
    Some((ymin - pad, ymax + pad))
}

pub fn range_marker_traces(
    xmin: f64,
    xmax: f64,
    ymin: f64,
    ymax: f64,
    label_prefix: &str,
) -> Option<Vec<TraceData>> {
    if !(xmin.is_finite() && xmax.is_finite() && ymin.is_finite() && ymax.is_finite()) {
        return None;
    }
    if !(xmin < xmax && ymin < ymax) {
        return None;
    }

    let label = format!("{label_prefix} range");
    Some(vec![
        TraceData::new(
            DVector::from_vec(vec![xmin, xmin]),
            DVector::from_vec(vec![ymin, ymax]),
            label.clone(),
            true,
        )
        .with_color(WINDOW_MARKER_COLOR)
        .with_legend_group(label.clone()),
        TraceData::new(
            DVector::from_vec(vec![xmax, xmax]),
            DVector::from_vec(vec![ymin, ymax]),
            "",
            true,
        )
        .with_color(WINDOW_MARKER_COLOR)
        .with_legend_group(label),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_markers_share_group_and_color() {
        let traces = range_marker_traces(1.0, 3.0, -2.0, 2.0, "window")
            .expect("range markers should be produced");
        assert_eq!(traces.len(), 2);
        assert_eq!(traces[0].label, "window range");
        assert_eq!(traces[1].label, "");
        assert_eq!(traces[0].legend_group.as_deref(), Some("window range"));
        assert_eq!(traces[1].legend_group.as_deref(), Some("window range"));
        assert_eq!(traces[0].color, Some(WINDOW_MARKER_COLOR));
        assert_eq!(traces[1].color, Some(WINDOW_MARKER_COLOR));
    }
}
