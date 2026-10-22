//! Peak-model editing and retained current/series results, within Data analysis.
use super::*;
use crate::app::{PEAK_PROBLEM_PREFIX, ProblemSeverity, replace_problem};
use crate::plot_ranges::plot_builder;
use crate::{
    peak_fits::{self, PeakArchive, PeakMetric, PeakRecord, PeakRun},
    series_measurements::{FrameInput, FrameStatus},
    widgets::{
        numeric_field::{FieldEvent, FieldKind, NumericField},
        text_input::{InputEvent, TextInput},
    },
};
use gpui::{AppContext, Entity};
use rexafs::prelude::{
    Measurement, MeasurementSpace, PeakFit, PeakRole, PeakShape, PeakTermination,
};
use ruviz_gpui::RuvizPlot;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub(crate) const PLOT_PEAKS: usize = 410;

/// Axis label for the signal a peak model is fitted to.
fn space_label(space: MeasurementSpace) -> &'static str {
    match space {
        MeasurementSpace::Mu => "μ(E)",
        MeasurementSpace::Norm => "Normalized μ(E)",
        MeasurementSpace::Flat => "Flattened μ(E)",
        MeasurementSpace::Chi { .. } => "Weighted χ(k)",
        MeasurementSpace::Fourier => "Fourier magnitude",
    }
}

/// Shape name as listed in the Add menu.
fn shape_label(shape: PeakShape) -> &'static str {
    match shape {
        PeakShape::Gaussian => "Gaussian",
        PeakShape::Lorentzian => "Lorentzian",
        PeakShape::PseudoVoigt => "Pseudo-Voigt",
        PeakShape::Voigt => "Voigt",
        PeakShape::ErfStep => "Erf step",
        PeakShape::ArctanStep => "Arctan step",
        PeakShape::Constant => "Constant baseline",
        PeakShape::Linear => "Linear baseline",
    }
}

/// Why the peak solver stopped, in plain words.
fn termination_label(termination: &PeakTermination) -> &'static str {
    match termination {
        PeakTermination::FixedModel => "Fixed model evaluated",
        PeakTermination::Converged => "Converged",
        PeakTermination::NotConverged => "Not converged",
        PeakTermination::Cancelled => "Cancelled",
    }
}

#[derive(Clone, Copy, PartialEq)]
enum View {
    Fit,
    Correlation,
    Trend,
}
#[derive(Clone, Copy)]
enum Menu {
    Add,
    History,
    Models,
    Trend,
    Export,
}

struct ParameterFields {
    name: String,
    value: Entity<NumericField>,
    minimum: Entity<NumericField>,
    maximum: Entity<NumericField>,
    expression: Entity<TextInput>,
}
struct Preview {
    x: Vec<f64>,
    y: Vec<f64>,
    origin: f64,
    label: String,
}

/// Each included native segment is drawn separately, so masked gaps remain gaps.
pub(super) fn peak_curve(
    mut plot: ruviz::prelude::Plot,
    energy: &[f64],
    y: &[f64],
    indices: &[usize],
    label: &str,
    color: ruviz::prelude::Color,
) -> ruviz::prelude::Plot {
    let mut start = 0;
    for end in 1..=indices.len() {
        if end < indices.len() && indices[end] == indices[end - 1] + 1 {
            continue;
        }
        let xs = energy[start..end].to_vec();
        let ys = y[start..end].to_vec();
        plot = if end - start == 1 {
            let line = plot.scatter(&xs, &ys).color(color);
            if start == 0 {
                line.label(label).into()
            } else {
                line.into()
            }
        } else {
            let line = plot.line(&xs, &ys).color(color);
            if start == 0 {
                line.label(label).into()
            } else {
                line.into()
            }
        };
        start = end;
    }
    plot
}

pub(crate) struct PeakState {
    pub open: bool,
    pub archive: PeakArchive,
    model: PeakFit,
    fields: Vec<ParameterFields>,
    range: Vec<Entity<NumericField>>,
    extra_range: Vec<Entity<NumericField>>,
    ranges_open: bool,
    name: Option<Entity<TextInput>>,
    advanced: bool,
    next_component: usize,
    revision: u64,
    generation: u64,
    preview_key: Option<(crate::group_identity::GroupId, u64, u64)>,
    preview: Option<Preview>,
    pub plot: Option<Entity<RuvizPlot>>,
    residual: Option<Entity<RuvizPlot>>,
    record: Option<PeakRecord>,
    run: Option<usize>,
    row: usize,
    view: View,
    trend: PeakMetric,
    pub busy: bool,
    cancel: Option<Arc<AtomicBool>>,
    message: String,
    menu: Option<Menu>,
    menu_position: gpui::Point<gpui::Pixels>,
    menu_focus: Option<gpui::FocusHandle>,
    focus: Option<gpui::FocusHandle>,
}
/// Problems-drawer entry for one finished peak-fit row: an error for a failed
/// fit, a warning listing the fit's warnings, or `None` (a clean or cancelled
/// fit clears the group's previous entry).
fn peak_row_problem(row: &peak_fits::PeakRow) -> Option<(ProblemSeverity, String)> {
    match row.status {
        FrameStatus::Failed | FrameStatus::Unavailable => Some((
            ProblemSeverity::Error,
            format!(
                "{PEAK_PROBLEM_PREFIX} failed: {}",
                row.reason.as_deref().unwrap_or("no reason recorded")
            ),
        )),
        FrameStatus::Succeeded => {
            row.summary
                .as_ref()
                .filter(|s| !s.warnings.is_empty())
                .map(|s| {
                    (
                        ProblemSeverity::Warning,
                        format!("{PEAK_PROBLEM_PREFIX}: {}", s.warnings.join("; ")),
                    )
                })
        }
        FrameStatus::Pending | FrameStatus::Cancelled => None,
    }
}
impl Default for PeakState {
    fn default() -> Self {
        Self {
            open: false,
            archive: PeakArchive::default(),
            model: PeakFit::new(-20.0..=70.0)
                .gaussian("p1", 10., 3., 5.)
                .erf_step("edge", 0., 1., 3.)
                .constant_baseline(0.),
            fields: vec![],
            range: vec![],
            extra_range: vec![],
            ranges_open: false,
            name: None,
            advanced: false,
            next_component: 2,
            revision: 0,
            generation: 0,
            preview_key: None,
            preview: None,
            plot: None,
            residual: None,
            record: None,
            run: None,
            row: 0,
            view: View::Fit,
            trend: PeakMetric::Area("p1".into()),
            busy: false,
            cancel: None,
            message: String::new(),
            menu: None,
            menu_position: Default::default(),
            menu_focus: None,
            focus: None,
        }
    }
}
impl PeakState {
    pub fn stop(&mut self) {
        if let Some(c) = self.cancel.take() {
            c.store(true, Ordering::Relaxed);
        }
        self.generation += 1;
    }
}
impl Drop for PeakState {
    fn drop(&mut self) {
        self.stop();
    }
}

impl StudioApp {
    pub(super) fn refresh_live_peak_trend(
        &mut self,
        id: &crate::group_identity::GroupId,
        cx: &mut Context<Self>,
    ) {
        let Some(run) = self.peaks.run.and_then(|i| self.peaks.archive.runs.get(i)) else {
            return;
        };
        if !self.peaks.open || &run.id != id {
            return;
        }
        if self.peaks.record.is_some() {
            let succeeded = run
                .rows
                .iter()
                .filter(|r| r.status == FrameStatus::Succeeded)
                .count();
            self.peaks.message = format!("Saved fit · {succeeded} / {} fitted", run.rows.len());
        }
        if self.peaks.view == View::Trend {
            self.rebuild_peak_plot(false, cx);
        }
    }
    pub(super) fn open_peak_run(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.peaks.busy {
            return;
        }
        let Some(run) = self.peaks.archive.runs.get(index).cloned() else {
            return;
        };
        self.peaks.model = (*run.model).clone();
        self.open_peaks(cx);
        if let Some(name) = &self.peaks.name {
            name.update(cx, |field, cx| field.set_text(run.name.clone(), cx));
        }
        self.peaks.view = View::Fit;
        self.load_peak_row(index, 0, cx);
    }
    pub(crate) fn open_peaks(&mut self, cx: &mut Context<Self>) {
        self.fluorescence.close();
        self.wavelet.cancel();
        self.wavelet.open = false;
        self.peaks.open = true;
        if self.peaks.focus.is_none() {
            self.peaks.focus = Some(cx.focus_handle());
        }
        self.stage = Stage::Data;
        self.workspace = Workspace::Explore;
        self.tools.open = None;
        if self.peaks.name.is_none() {
            self.peaks.name =
                Some(cx.new(|cx| TextInput::new("Model name", "XANES peaks", self.theme, cx)));
        }
        self.sync_peak_fields(cx);
        self.preview_peaks(cx);
    }
    fn peak_input(&self) -> Option<FrameInput> {
        let ix = self.current_group_index()?;
        let id = self.group_id(ix)?;
        Some(self.measurement_input(&id, self.entry_label(ix)))
    }
    fn sync_peak_fields(&mut self, cx: &mut Context<Self>) {
        self.peaks.fields.clear();
        self.peaks.range.clear();
        for (index, label) in ["From (eV)", "To (eV)"].into_iter().enumerate() {
            let field = cx.new(|cx| {
                NumericField::new(
                    label,
                    "required",
                    Some(self.peaks.model.range[index]),
                    FieldKind::Float,
                    self.theme,
                    cx,
                )
                .with_step(0.1)
            });
            cx.subscribe(&field, move |app, _, event, cx| {
                if let FieldEvent::Changed(Some(value)) = event {
                    app.update_peak_range(index, *value, cx);
                    app.peak_model_changed(cx);
                }
            })
            .detach();
            self.peaks.range.push(field);
        }
        if self.peaks.extra_range.is_empty() {
            for (label, value) in [("Exclude from (eV)", 5.), ("Exclude to (eV)", 25.)] {
                self.peaks.extra_range.push(cx.new(|cx| {
                    NumericField::new(
                        label,
                        "required",
                        Some(value),
                        FieldKind::Float,
                        self.theme,
                        cx,
                    )
                    .with_step(0.1)
                }));
            }
        }
        for (name, variable) in self.peaks.model.parameters.vars.clone() {
            let role = self.peaks.model.components.iter().find_map(|c| {
                c.parameters
                    .iter()
                    .find(|(_, n)| **n == name)
                    .map(|(role, _)| (c.shape, role.as_str()))
            });
            let display = role.map_or(name.as_str(), |(_, role)| role);
            let display = match display {
                "center" => "Center (eV)",
                "width" if role.is_some_and(|(s, _)| s == PeakShape::Voigt) => "Gaussian FWHM (eV)",
                "lorentz_width" => "Lorentz FWHM (eV)",
                "width" => "FWHM (eV)",
                "area" => "Area",
                "offset" => "Offset",
                "slope" => "Slope",
                "height" => "Height",
                "scale" => "Scale (eV)",
                "fraction" => "Lorentz fraction",
                "reference" => "Reference (eV)",
                _ => display,
            };
            let mut entities = Vec::new();
            for (which, label, value) in [
                (0, display, Some(variable.value)),
                (1, "Min", variable.min),
                (2, "Max", variable.max),
            ] {
                let field = cx.new(|cx| {
                    NumericField::new(
                        label.to_owned(),
                        if which == 0 { "required" } else { "none" },
                        value,
                        FieldKind::Float,
                        self.theme,
                        cx,
                    )
                    .with_step(0.1)
                });
                let key = name.clone();
                cx.subscribe(&field, move |app, _, event, cx| {
                    if let FieldEvent::Changed(value) = event {
                        if let Some(p) = app.peaks.model.parameters.vars.get_mut(&key) {
                            match which {
                                0 => {
                                    if let Some(v) = value {
                                        p.value = *v;
                                        p.init_value = *v;
                                    }
                                }
                                1 => p.min = *value,
                                _ => p.max = *value,
                            }
                        }
                        app.peak_model_changed(cx);
                    }
                })
                .detach();
                entities.push(field);
            }
            let expression = cx.new(|cx| {
                TextInput::new(
                    "Tie expression",
                    variable.expr.clone().unwrap_or_default(),
                    self.theme,
                    cx,
                )
            });
            let key = name.clone();
            cx.subscribe(&expression, move |app, _, event, cx| {
                if let InputEvent::Committed(text) = event {
                    if let Some(p) = app.peaks.model.parameters.vars.get_mut(&key) {
                        p.expr = (!text.trim().is_empty()).then(|| text.trim().to_owned());
                        if p.expr.is_some() {
                            p.vary = false;
                        }
                    }
                    app.peak_model_changed(cx);
                }
            })
            .detach();
            self.peaks.fields.push(ParameterFields {
                name,
                value: entities[0].clone(),
                minimum: entities[1].clone(),
                maximum: entities[2].clone(),
                expression,
            });
        }
    }
    fn peak_model_changed(&mut self, cx: &mut Context<Self>) {
        if self.peaks.busy {
            return;
        }
        self.peaks.revision += 1;
        self.peaks.record = None;
        self.peaks.run = None;
        self.peaks.view = View::Fit;
        self.preview_peaks(cx);
    }
    fn update_peak_range(&mut self, index: usize, value: f64, cx: &mut Context<Self>) {
        let old = self.peaks.model.range[index];
        self.peaks.model.range[index] = value;
        for component in &self.peaks.model.components {
            if let Some(name) = component.parameters.get("center") {
                let p = self.peaks.model.parameters.vars.get_mut(name).unwrap();
                let bound = if index == 0 { &mut p.min } else { &mut p.max };
                if *bound == Some(old) {
                    *bound = Some(value);
                    if let Some(field) = self.peaks.fields.iter().find(|f| &f.name == name) {
                        let field = if index == 0 {
                            &field.minimum
                        } else {
                            &field.maximum
                        };
                        field.update(cx, |f, cx| f.set_value(Some(value), cx));
                    }
                }
            }
        }
    }
    fn peak_definition(&self, cx: &Context<Self>) -> Result<PeakFit, String> {
        let mut model = self.peaks.model.clone();
        for (i, field) in self.peaks.range.iter().enumerate() {
            model.range[i] = field
                .read(cx)
                .pending_value(cx)
                .map_err(|_| "Invalid fit-range value")?
                .ok_or("Fit-range values are required")?;
        }
        for field in &self.peaks.fields {
            let Some(p) = model.parameters.vars.get_mut(&field.name) else {
                continue;
            };
            p.value = field
                .value
                .read(cx)
                .pending_value(cx)
                .map_err(|_| format!("Invalid {}", field.name))?
                .ok_or_else(|| format!("{} is required", field.name))?;
            p.init_value = p.value;
            p.min = field
                .minimum
                .read(cx)
                .pending_value(cx)
                .map_err(|_| format!("Invalid {} minimum", field.name))?;
            p.max = field
                .maximum
                .read(cx)
                .pending_value(cx)
                .map_err(|_| format!("Invalid {} maximum", field.name))?;
            let text = field.expression.read(cx).text().trim();
            p.expr = (!text.is_empty()).then(|| text.to_owned());
        }
        model.validate().map_err(|e| e.to_string())?;
        Ok(model)
    }
    fn peak_exclusion(&self, cx: &Context<Self>) -> Result<[f64; 2], String> {
        let mut values = [0.; 2];
        for (i, field) in self.peaks.extra_range.iter().enumerate() {
            values[i] = field
                .read(cx)
                .pending_value(cx)
                .map_err(|_| "Invalid exclusion energy")?
                .ok_or("Exclusion energies are required")?;
        }
        if values[0] >= values[1] {
            return Err("Exclusion start must be below its end".into());
        }
        Ok(values)
    }
    fn initialize_peak_baseline(&mut self, cx: &mut Context<Self>) {
        if self.peaks.busy {
            return;
        }
        let setup = self
            .peak_definition(cx)
            .and_then(|model| Ok((model, self.peak_exclusion(cx)?)));
        let (model, range) = match setup {
            Ok(v) => v,
            Err(e) => {
                self.peaks.message = e;
                cx.notify();
                return;
            }
        };
        let Some(input) = self.peak_input() else {
            return;
        };
        self.peaks.busy = true;
        self.peaks.generation += 1;
        let generation = self.peaks.generation;
        let project = self.project_generation;
        let cancel = Arc::new(AtomicBool::new(false));
        self.peaks.cancel = Some(cancel.clone());
        self.peaks.message = "Estimating the baseline outside the peak region…".into();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let (sp, _, _) =
                        input.prepare(&peak_fits::preparation_definition(&model), None)?;
                    model
                        .initialize_baseline_with_progress(&sp, &[range[0]..=range[1]], |_, _| {
                            !cancel.load(Ordering::Relaxed)
                        })
                        .map_err(|e| e.to_string())
                })
                .await;
            this.update(cx, |app, cx| {
                if app.project_generation != project || app.peaks.generation != generation {
                    return;
                }
                app.peaks.busy = false;
                app.peaks.cancel = None;
                match result {
                    Ok(model) => {
                        app.peaks.model = model;
                        app.sync_peak_fields(cx);
                        app.peak_model_changed(cx);
                    }
                    Err(e) => app.peaks.message = e,
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }
    fn preview_peaks(&mut self, cx: &mut Context<Self>) {
        if self.peaks.busy {
            return;
        }
        let Some(input) = self.peak_input() else {
            self.peaks.message = "Choose a spectrum".into();
            cx.notify();
            return;
        };
        self.peaks.preview_key = Some((
            input.group.clone(),
            input.settings.fingerprint(),
            self.peaks.revision,
        ));
        self.peaks.generation += 1;
        let model = match self.peak_definition(cx) {
            Ok(model) => model,
            Err(e) => {
                self.peaks.message = e;
                self.peaks.plot = None;
                self.peaks.residual = None;
                cx.notify();
                return;
            }
        };
        self.peaks.model = model.clone();
        let generation = self.peaks.generation;
        let project = self.project_generation;
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let (sp, _, _) =
                        input.prepare(&peak_fits::preparation_definition(&model), None)?;
                    let mut measurement = Measurement::mean(model.range[0]..=model.range[1]);
                    measurement.space = model.space;
                    measurement.origin = model.origin;
                    let arrays = measurement.arrays(&sp).map_err(|e| e.to_string())?;
                    let origin = match model.origin {
                        rexafs::prelude::AxisOrigin::Absolute => 0.,
                        rexafs::prelude::AxisOrigin::Reference { energy_ev } => energy_ev,
                        _ => arrays.e0_ev.ok_or("E₀ unavailable")?,
                    };
                    Ok::<_, String>(Preview {
                        x: arrays.axis,
                        y: arrays.signal,
                        origin,
                        label: input.label,
                    })
                })
                .await;
            this.update(cx, |app, cx| {
                if app.project_generation != project || app.peaks.generation != generation {
                    return;
                }
                match result {
                    Ok(preview) => {
                        app.peaks.preview = Some(preview);
                        app.peaks.message = "Starting model · drag the fit boundaries".into();
                        app.rebuild_peak_plot(false, cx);
                    }
                    Err(e) => {
                        app.peaks.message = e;
                        app.peaks.preview = None;
                        app.peaks.plot = None;
                        app.peaks.residual = None;
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }
    fn start_peak_fits(&mut self, scope: usize, cx: &mut Context<Self>) {
        if self.peaks.busy {
            return;
        }
        self.peaks.model = match self.peak_definition(cx) {
            Ok(m) => m,
            Err(e) => {
                self.peaks.message = e;
                cx.notify();
                return;
            }
        };
        let inputs: Vec<_> = match scope {
            1 => super::tools::marked_group_indices(&self.selection)
                .filter_map(|ix| {
                    self.group_id(ix)
                        .map(|id| self.measurement_input(&id, self.entry_label(ix)))
                })
                .collect(),
            2 => self
                .measurements
                .selected_series
                .and_then(|i| self.measurements.archive.series.get(i))
                .map(|s| {
                    s.frames
                        .iter()
                        .map(|f| self.measurement_input(&f.group, f.label.clone()))
                        .collect()
                })
                .unwrap_or_default(),
            _ => self.peak_input().into_iter().collect(),
        };
        if inputs.is_empty() {
            self.peaks.message = "Choose input spectra first".into();
            cx.notify();
            return;
        }
        let root = match peak_fits::root() {
            Ok(r) => r,
            Err(e) => {
                self.peaks.message = e;
                cx.notify();
                return;
            }
        };
        let name = self
            .peaks
            .name
            .as_ref()
            .map(|f| f.read(cx).text().to_owned())
            .unwrap_or_else(|| "XANES peaks".into());
        let mut run = PeakRun::new(name, self.peaks.model.clone(), &inputs);
        if scope == 2 {
            run.series = self
                .measurements
                .selected_series
                .and_then(|i| self.measurements.archive.series.get(i))
                .cloned();
        }
        let index = self.peaks.archive.runs.len();
        self.peaks.archive.runs.push(Arc::new(run.clone()));
        self.peaks.run = Some(index);
        self.peaks.row = 0;
        self.peaks.record = None;
        self.peaks.busy = true;
        self.peaks.generation += 1;
        let generation = self.peaks.generation;
        let project = self.project_generation;
        let cancel = Arc::new(AtomicBool::new(false));
        self.peaks.cancel = Some(cancel.clone());
        self.peaks.message = format!("Checking {} input revisions…", inputs.len());
        cx.spawn(async move |this, cx| {
            let freeze_cancel = cancel.clone();
            let (mut run, inputs) = cx
                .background_spawn(async move {
                    let mut run = run;
                    run.freeze(&inputs, || freeze_cancel.load(Ordering::Relaxed));
                    (run, inputs)
                })
                .await;
            for (i, input) in inputs.into_iter().enumerate() {
                if cancel.load(Ordering::Relaxed) {
                    break;
                }
                let row = run.rows[i].clone();
                let model = run.model.clone();
                let root = root.clone();
                let flag = cancel.clone();
                let row = cx
                    .background_spawn(async move {
                        peak_fits::calculate(&model, &row, &input, &root, || {
                            flag.load(Ordering::Relaxed)
                        })
                    })
                    .await;
                run.rows[i] = row.clone();
                if this
                    .update(cx, |app, cx| {
                        if app.project_generation != project || app.peaks.generation != generation {
                            return false;
                        }
                        Arc::make_mut(&mut app.peaks.archive.runs[index]).rows[i] = row;
                        app.peaks.message = format!("Fitting {} / {}", i + 1, run.rows.len());
                        cx.notify();
                        true
                    })
                    .ok()
                    != Some(true)
                {
                    cancel.store(true, Ordering::Relaxed);
                    break;
                }
            }
            run.finish(cancel.load(Ordering::Relaxed));
            this.update(cx, |app, cx| {
                if app.project_generation != project || app.peaks.generation != generation {
                    return;
                }
                let succeeded = run
                    .rows
                    .iter()
                    .filter(|r| r.status == FrameStatus::Succeeded)
                    .count();
                app.peaks.message = format!(
                    "{succeeded} / {} fitted{}",
                    run.rows.len(),
                    if run.cancelled { " · cancelled" } else { "" }
                );
                for row in &run.rows {
                    let label = app
                        .group_registry
                        .index(&row.group)
                        .map_or_else(|| row.label.clone(), |ix| app.problem_label(ix));
                    replace_problem(
                        &mut app.job_errors,
                        &label,
                        PEAK_PROBLEM_PREFIX,
                        peak_row_problem(row),
                    );
                }
                app.peaks.archive.runs[index] = Arc::new(run);
                app.peaks.busy = false;
                app.peaks.cancel = None;
                app.record("XANES peak fit: retained results", None);
                app.load_peak_row(index, 0, cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }
    fn load_peak_row(&mut self, run: usize, row: usize, cx: &mut Context<Self>) {
        let Some(input) = self
            .peaks
            .archive
            .runs
            .get(run)
            .and_then(|r| r.rows.get(row))
            .cloned()
        else {
            return;
        };
        if let Some(index) = self.group_registry.index(&input.group) {
            self.select_entry(index, cx);
        }
        self.peaks.run = Some(run);
        self.peaks.row = row;
        self.peaks.generation += 1;
        self.peaks.record = None;
        self.peaks.plot = None;
        self.peaks.residual = None;
        let saved = &self.peaks.archive.runs[run];
        let succeeded = saved
            .rows
            .iter()
            .filter(|r| r.status == FrameStatus::Succeeded)
            .count();
        self.peaks.message = format!(
            "Saved fit · {succeeded} / {} fitted{}",
            saved.rows.len(),
            if saved.cancelled { " · cancelled" } else { "" }
        );
        let generation = self.peaks.generation;
        let project = self.project_generation;
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { peak_fits::read(&input) })
                .await;
            this.update(cx, |app, cx| {
                if app.project_generation != project || app.peaks.generation != generation {
                    return;
                }
                match result {
                    Ok(record) => {
                        app.peaks.record = Some(record);
                        app.rebuild_peak_plot(false, cx);
                    }
                    Err(e) => app.peaks.message = e,
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }
    pub(crate) fn rebuild_peak_plot(&mut self, preserve_view: bool, cx: &mut Context<Self>) {
        use ruviz::prelude::*;
        let base = || Plot::new().size(9., 4.).theme(self.theme.plot_theme());
        let mut residual = None;
        let mut plot: Plot = base();
        if self.peaks.view == View::Trend {
            let Some(run) = self.peaks.run.and_then(|i| self.peaks.archive.runs.get(i)) else {
                return;
            };
            let (coordinates, xlabel) = run.plot_coordinates();
            let values: Vec<_> = run
                .rows
                .iter()
                .zip(coordinates)
                .filter(|(r, _)| r.status == FrameStatus::Succeeded)
                .filter_map(|(r, x)| {
                    self.peaks
                        .trend
                        .value(r.summary.as_ref()?)
                        .0
                        .map(|value| (x, value))
                })
                .collect();
            if values.is_empty() {
                self.peaks.message = "This parameter has no successful fits".into();
                return;
            }
            let x: Vec<_> = values.iter().map(|v| v.0).collect();
            let y: Vec<_> = values.iter().map(|v| v.1).collect();
            plot = plot
                .scatter(&x, &y)
                .xlabel(xlabel)
                .ylabel(self.peaks.trend.label(&run.model))
                .into();
        } else if let Some(record) = &self.peaks.record {
            let r = &record.result;
            if self.peaks.view == View::Correlation {
                if let Some(matrix) = &r.correlation {
                    let n = matrix.len() as f64;
                    plot = plot
                        .heatmap_with(
                            matrix,
                            HeatmapConfig::new()
                                .colorbar(true)
                                .origin(HeatmapOrigin::Lower)
                                .extent(0.5, n + 0.5, 0.5, n + 0.5)
                                .vmin(-1.)
                                .vmax(1.),
                        )
                        .xlabel("Parameter number")
                        .ylabel("Parameter number")
                        .into();
                } else {
                    self.peaks.message = r
                        .uncertainty_unavailable
                        .clone()
                        .unwrap_or_else(|| "Correlation unavailable".into());
                    self.peaks.plot = None;
                    self.peaks.residual = None;
                    return;
                }
            } else {
                let color = |i| crate::plotting::trace_color(&self.theme, i);
                plot = peak_curve(
                    plot,
                    &r.energy,
                    &r.data,
                    &r.source_indices,
                    "Data",
                    color(0),
                );
                plot = peak_curve(
                    plot,
                    &r.energy,
                    &r.model,
                    &r.source_indices,
                    "Model",
                    color(1),
                );
                for (i, component) in r.components.iter().enumerate() {
                    plot = peak_curve(
                        plot,
                        &r.energy,
                        &component.curve,
                        &r.source_indices,
                        &component.name,
                        color(i + 2),
                    );
                }
                plot = plot
                    .xlabel("Energy (eV)")
                    .ylabel(space_label(r.definition.space))
                    .legend_position(LegendPosition::UpperRight);
                let p: Plot = peak_curve(
                    base().size(9., 1.7),
                    &r.energy,
                    &r.residual,
                    &r.source_indices,
                    "Residual",
                    color(0),
                )
                .xlabel("Energy (eV)")
                .ylabel("Data − model");
                residual = Some(p);
            }
        } else if let Some(preview) = &self.peaks.preview {
            match self.peaks.model.evaluate(&preview.x, Some(preview.origin)) {
                Ok(model) => {
                    plot = plot.line(&preview.x, &preview.y).label("Data").into();
                    plot = plot.line(&preview.x, &model).label("Starting model").into();
                    for component in &self.peaks.model.components {
                        let mut definition = self.peaks.model.clone();
                        definition.components = vec![component.clone()];
                        if let Ok(y) = definition.evaluate(&preview.x, Some(preview.origin)) {
                            plot = plot
                                .line(&preview.x, &y)
                                .label(component.name.clone())
                                .into();
                        }
                    }
                    let [lo, hi] = self.peaks.model.range;
                    let padding = (hi - lo) * 0.15;
                    let difference: Vec<_> =
                        preview.y.iter().zip(&model).map(|(a, b)| a - b).collect();
                    residual = Some(
                        base()
                            .size(9., 1.7)
                            .line(&preview.x, &difference)
                            .xlabel("Energy (eV)")
                            .ylabel("Data − starting model")
                            .xlim(preview.origin + lo - padding, preview.origin + hi + padding)
                            .into(),
                    );
                    plot = plot
                        .xlabel("Energy (eV)")
                        .ylabel(space_label(self.peaks.model.space))
                        .xlim(preview.origin + lo - padding, preview.origin + hi + padding)
                        .legend_position(LegendPosition::UpperRight);
                }
                Err(e) => {
                    self.peaks.message = e.to_string();
                    return;
                }
            }
        } else {
            return;
        }
        if self.peaks.view == View::Fit && !self.handles.hidden {
            let definition = self
                .peaks
                .record
                .as_ref()
                .map_or(&self.peaks.model, |r| &r.result.definition);
            let origin = self
                .peaks
                .record
                .as_ref()
                .map(|r| r.result.origin_ev)
                .or_else(|| self.peaks.preview.as_ref().map(|p| p.origin));
            if let Some(origin) = origin {
                for range in &definition.exclude {
                    plot = plot.axvspan(origin + range[0], origin + range[1]);
                    residual = residual.map(|p| p.axvspan(origin + range[0], origin + range[1]));
                }
            }
        }
        if preserve_view && let Some(entity) = &self.peaks.plot {
            entity.update(cx, |view, cx| view.set_plot_keep_view(plot, cx));
        } else {
            self.peaks.plot = Some(plot_builder(plot).interactive().build(cx));
        }
        self.peaks.residual = residual.map(|plot| {
            if preserve_view && let Some(entity) = &self.peaks.residual {
                entity.update(cx, |view, cx| view.set_plot_keep_view(plot, cx));
                entity.clone()
            } else {
                plot_builder(plot).interactive().build(cx)
            }
        });
    }
    fn add_peak_component(&mut self, shape: PeakShape, cx: &mut Context<Self>) {
        while self
            .peaks
            .model
            .components
            .iter()
            .any(|c| c.name == format!("p{}", self.peaks.next_component))
        {
            self.peaks.next_component += 1;
        }
        let name = format!("p{}", self.peaks.next_component);
        self.peaks.next_component += 1;
        let base = PeakFit::new(self.peaks.model.range[0]..=self.peaks.model.range[1]);
        let center = (self.peaks.model.range[0] + self.peaks.model.range[1]) / 2.;
        let added = match shape {
            PeakShape::Gaussian => base.gaussian(name, center, 3., 5.),
            PeakShape::Lorentzian => base.lorentzian(name, center, 3., 5.),
            PeakShape::PseudoVoigt => base.pseudo_voigt(name, center, 3., 5., 0.5),
            PeakShape::Voigt => base.voigt(name, center, 3., 4., 2.),
            PeakShape::ErfStep => base.erf_step(name, center, 1., 3.),
            PeakShape::ArctanStep => base.arctan_step(name, center, 1., 3.),
            PeakShape::Constant => base.constant_baseline(0.),
            PeakShape::Linear => base.linear_baseline(0., 0.),
        };
        if added.components.iter().any(|c| {
            self.peaks
                .model
                .components
                .iter()
                .any(|old| old.name == c.name)
        }) {
            self.peaks.message = "Remove the existing baseline before changing its shape".into();
            return;
        }
        self.peaks.model.components.extend(added.components);
        self.peaks
            .model
            .parameters
            .vars
            .extend(added.parameters.vars);
        self.sync_peak_fields(cx);
        self.peak_model_changed(cx);
    }
    fn open_peak_menu(
        &mut self,
        menu: Menu,
        event: &ClickEvent,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        if self.peaks.busy {
            return;
        }
        self.peaks.menu = Some(menu);
        self.peaks.menu_position = event.position();
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        self.peaks.menu_focus = Some(focus);
        cx.notify();
    }
    fn export_peaks(&mut self, kind: usize, cx: &mut Context<Self>) {
        let record = self.peaks.record.clone();
        let run = self
            .peaks
            .run
            .and_then(|i| self.peaks.archive.runs.get(i))
            .cloned();
        if (kind < 2 && record.is_none()) || (kind == 2 && run.is_none()) {
            self.peaks.message = "Select a retained result first".into();
            cx.notify();
            return;
        }
        let request = cx.prompt_for_new_path(
            &std::env::temp_dir(),
            Some(["peak-curves.csv", "peak-fit.json", "peak-trends.csv"][kind]),
        );
        let project = self.project_generation;
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(path))) = request.await else {
                return;
            };
            let result = cx
                .background_spawn(async move {
                    let parent = path.parent().ok_or("Export folder unavailable")?;
                    let mut file =
                        tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
                    match kind {
                        0 => peak_fits::write_curves(record.as_ref().unwrap(), &mut file)?,
                        1 => serde_json::to_writer_pretty(&mut file, record.as_ref().unwrap())
                            .map_err(|e| e.to_string())?,
                        _ => peak_fits::write_trend(run.as_ref().unwrap(), &mut file)?,
                    }
                    file.as_file().sync_all().map_err(|e| e.to_string())?;
                    file.persist(&path).map_err(|e| e.to_string())?;
                    Ok::<_, String>(format!("Exported {}", path.display()))
                })
                .await;
            this.update(cx, |app, cx| {
                if app.project_generation == project {
                    app.peaks.message = result.unwrap_or_else(|e| e);
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }
    pub(super) fn peak_menu_overlay(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let kind = self.peaks.menu?;
        let focus = self.peaks.menu_focus.as_ref()?;
        let choices: Vec<String> = match kind {
            Menu::Add => vec![
                "Gaussian",
                "Lorentzian",
                "Pseudo-Voigt",
                "Voigt",
                "Erf step",
                "Arctan step",
                "Constant baseline",
                "Linear baseline",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            Menu::History => self
                .peaks
                .archive
                .runs
                .iter()
                .map(|r| {
                    format!(
                        "{} · {} · {}",
                        r.name,
                        r.created,
                        crate::text::plural(r.rows.len(), "frame")
                    )
                })
                .collect(),
            Menu::Models => self
                .peaks
                .archive
                .models
                .iter()
                .map(|m| format!("{} · v{}", m.name, m.revision))
                .collect(),
            Menu::Trend => self
                .peaks
                .run
                .and_then(|i| self.peaks.archive.runs.get(i))
                .map(|r| {
                    PeakMetric::choices(&r.model)
                        .iter()
                        .map(|m| m.label(&r.model))
                        .collect()
                })
                .unwrap_or_default(),
            Menu::Export => vec![
                "Curves CSV…".into(),
                "Fit JSON…".into(),
                "All-frame trends CSV…".into(),
            ],
        };
        let mut menu = div()
            .id("peak-menu")
            .w(px(350.))
            .max_h(px(420.))
            .overflow_y_scroll()
            .p_1()
            .flex()
            .flex_col()
            .gap_1()
            .bg(self.theme.surface)
            .border_1()
            .border_color(self.theme.border)
            .rounded_lg()
            .shadow_lg()
            .on_any_mouse_down(|_, _, cx| cx.stop_propagation());
        if choices.is_empty() {
            menu = menu.child(div().p_2().child("No saved entries"));
        }
        for (index, label) in choices.into_iter().enumerate() {
            menu = menu.child(
                button(&self.theme, ("peak-menu-choice", index), label, false).on_click(
                    cx.listener(move |app, _, _, cx| {
                        app.peaks.menu = None;
                        match kind {
                            Menu::Add => app.add_peak_component(
                                [
                                    PeakShape::Gaussian,
                                    PeakShape::Lorentzian,
                                    PeakShape::PseudoVoigt,
                                    PeakShape::Voigt,
                                    PeakShape::ErfStep,
                                    PeakShape::ArctanStep,
                                    PeakShape::Constant,
                                    PeakShape::Linear,
                                ][index],
                                cx,
                            ),
                            Menu::History => {
                                app.peaks.model = (*app.peaks.archive.runs[index].model).clone();
                                app.sync_peak_fields(cx);
                                app.peaks.view = View::Fit;
                                app.load_peak_row(index, 0, cx);
                            }
                            Menu::Models => {
                                let saved = &app.peaks.archive.models[index];
                                app.peaks.model = saved.model.clone();
                                if let Some(name) = &app.peaks.name {
                                    name.update(cx, |field, cx| {
                                        field.set_text(saved.name.clone(), cx)
                                    });
                                }
                                app.peaks.next_component = app.peaks.model.components.len() + 1;
                                app.sync_peak_fields(cx);
                                app.peak_model_changed(cx);
                            }
                            Menu::Trend => {
                                if let Some(run) =
                                    app.peaks.run.and_then(|i| app.peaks.archive.runs.get(i))
                                {
                                    if let Some(metric) = PeakMetric::choices(&run.model).get(index)
                                    {
                                        app.peaks.trend = metric.clone();
                                    }
                                }
                                app.peaks.view = View::Trend;
                                app.rebuild_peak_plot(false, cx);
                            }
                            Menu::Export => app.export_peaks(index, cx),
                        }
                        cx.notify();
                    }),
                ),
            );
        }
        Some(
            div()
                .id("peak-menu-dismiss")
                .absolute()
                .inset_0()
                .occlude()
                .track_focus(focus)
                .on_key_down(cx.listener(|app, event: &gpui::KeyDownEvent, _, cx| {
                    if event.keystroke.key == "escape" {
                        app.peaks.menu = None;
                        cx.notify();
                        cx.stop_propagation();
                    }
                }))
                .on_any_mouse_down(cx.listener(|app, _, _, cx| {
                    app.peaks.menu = None;
                    cx.notify();
                    cx.stop_propagation();
                }))
                .child(
                    gpui::anchored()
                        .position(self.peaks.menu_position)
                        .offset(gpui::point(px(0.), px(15.)))
                        .snap_to_window()
                        .child(menu),
                )
                .into_any_element(),
        )
    }
    pub(crate) fn peak_center(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        if self.peaks.record.is_none() && !self.peaks.busy && self.peaks.run.is_none() {
            if let Some(input) = self.peak_input() {
                let key = (
                    input.group,
                    input.settings.fingerprint(),
                    self.peaks.revision,
                );
                if self.peaks.preview_key.as_ref() != Some(&key) {
                    self.preview_peaks(cx);
                }
            }
        }
        let t = self.theme;
        let header = div()
            .flex()
            .items_center()
            .gap_2()
            .child(
                button(&t, "peaks-back", "← Data", false).on_click(cx.listener(|app, _, _, cx| {
                    app.peaks.open = false;
                    cx.notify();
                })),
            )
            .child(div().flex_1().text_size(px(16.)).child("XANES peaks"))
            .child(self.plot_ranges_button(cx))
            .child(
                button(&t, "peaks-export", "Export ▾", false).on_click(
                    cx.listener(|app, e, w, cx| app.open_peak_menu(Menu::Export, e, w, cx)),
                ),
            )
            .child(
                button(&t, "peaks-history", "History ▾", false).on_click(cx.listener(
                    |app, event, window, cx| app.open_peak_menu(Menu::History, event, window, cx),
                )),
            );
        let controls = div()
            .flex()
            .items_center()
            .gap_2()
            .child(
                button(&t, "peaks-preview", "Preview", false).on_click(cx.listener(
                    |app, _, _, cx| {
                        if app.peaks.busy {
                            return;
                        }
                        app.peaks.run = None;
                        app.peaks.record = None;
                        app.peaks.view = View::Fit;
                        app.preview_peaks(cx);
                    },
                )),
            )
            .child(
                button(&t, "peaks-fit", "Fit current", true)
                    .on_click(cx.listener(|app, _, _, cx| app.start_peak_fits(0, cx))),
            )
            .child(
                button(&t, "peaks-batch", "Fit marked", false)
                    .on_click(cx.listener(|app, _, _, cx| app.start_peak_fits(1, cx))),
            )
            .child(
                button(&t, "peaks-series", "Fit series", false)
                    .on_click(cx.listener(|app, _, _, cx| app.start_peak_fits(2, cx))),
            )
            .child(
                button(&t, "peaks-cancel", "Cancel", false)
                    .opacity(if self.peaks.busy { 1. } else { 0.4 })
                    .on_click(cx.listener(|app, _, _, cx| {
                        if let Some(c) = &app.peaks.cancel {
                            c.store(true, Ordering::Relaxed);
                        }
                        cx.notify();
                    })),
            );
        let label = self
            .peaks
            .record
            .as_ref()
            .map(|r| r.label.clone())
            .or_else(|| self.peaks.preview.as_ref().map(|p| p.label.clone()))
            .unwrap_or_default();
        let mut body = div()
            .id("peak-workspace")
            .key_context("PeakFits")
            .track_focus(self.peaks.focus.as_ref().unwrap())
            .on_action(cx.listener(|app, _: &crate::app::FramePrev, window, cx| {
                app.step_peak(-1, cx);
                app.peaks.focus.as_ref().unwrap().focus(window, cx);
            }))
            .on_action(cx.listener(|app, _: &crate::app::FrameNext, window, cx| {
                app.step_peak(1, cx);
                app.peaks.focus.as_ref().unwrap().focus(window, cx);
            }))
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .child(header)
            .child(controls)
            .child(div().text_color(t.text_muted).child(label));
        if let Some(index) = self.peaks.run {
            let count = self.peaks.archive.runs[index].rows.len();
            let row = self.peaks.row;
            let mut rowbar = div().flex().gap_2().items_center();
            for (id, label, target) in [
                ("peak-prev", "←", row.saturating_sub(1)),
                ("peak-next", "→", (row + 1).min(count.saturating_sub(1))),
            ] {
                rowbar = rowbar.child(button(&t, id, label, false).on_click(cx.listener(
                    move |app, _, _, cx| {
                        if !app.peaks.busy {
                            app.load_peak_row(index, target, cx);
                        }
                    },
                )));
            }
            rowbar = rowbar.child(format!("{} / {}", row + 1, count));
            for (view, label) in [
                (View::Fit, "Fit"),
                (View::Correlation, "Correlation"),
                (View::Trend, "Trend"),
            ] {
                rowbar = rowbar.child(
                    button(
                        &t,
                        SharedString::from(format!("peak-view-{label}")),
                        label,
                        self.peaks.view == view,
                    )
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.peaks.view = view;
                        app.rebuild_peak_plot(false, cx);
                        cx.notify();
                    })),
                );
            }
            if self.peaks.view == View::Trend {
                rowbar = rowbar.child(
                    button(
                        &t,
                        "peak-trend-parameter",
                        format!("{} ▾", self.peaks.trend.label(&self.peaks.model)),
                        false,
                    )
                    .on_click(
                        cx.listener(|app, e, w, cx| app.open_peak_menu(Menu::Trend, e, w, cx)),
                    ),
                );
            }
            body = body.child(rowbar);
        }
        body = body.child(
            div()
                .text_size(px(12.))
                .text_color(t.text_muted)
                .child(self.peaks.message.clone()),
        );
        if let Some(record) = &self.peaks.record {
            let r = &record.result;
            if !r.warnings.is_empty() {
                body = body.child(
                    div()
                        .text_color(t.warn)
                        .text_size(px(12.))
                        .child(r.warnings.join(" · ")),
                );
            }
            if let Some(reason) = &r.uncertainty_unavailable {
                body = body.child(
                    div()
                        .text_color(t.warn)
                        .text_size(px(12.))
                        .child(format!("Uncertainty: {reason}")),
                );
            }
        }
        if let Some(plot) = &self.peaks.plot {
            body = body.child(
                div()
                    .id("peak-plot")
                    .relative()
                    .h(px(350.))
                    .min_h(px(350.))
                    .w_full()
                    .on_mouse_move(cx.listener(|app, event: &gpui::MouseMoveEvent, _, cx| {
                        app.plot_pointer_move(PLOT_PEAKS, event.position, cx)
                    }))
                    .capture_any_mouse_down(cx.listener(
                        |app, event: &gpui::MouseDownEvent, window, cx| {
                            if let Some(focus) = &app.peaks.focus {
                                focus.focus(window, cx);
                            }
                            app.capture_handle_press(PLOT_PEAKS, event, cx);
                        },
                    ))
                    .child(plot.clone())
                    .children(self.handle_layer(PLOT_PEAKS, cx))
                    .children(self.handle_overlay(PLOT_PEAKS, cx)),
            );
        }
        if let Some(residual) = &self.peaks.residual {
            body = body.child(div().h(px(180.)).min_h(px(180.)).child(residual.clone()));
        }
        if let Some(record) = &self.peaks.record {
            let r = &record.result;
            body = body.child(div().text_size(px(12.)).child(format!(
                "{} · objective {:.5} · {} · {} free {}",
                termination_label(&r.termination),
                r.objective,
                crate::text::plural(r.points, "point"),
                r.free_parameters,
                crate::text::noun_for(r.free_parameters, "parameter")
            )));
            if self.peaks.view == View::Correlation {
                body = body.child(
                    div().text_size(px(12.)).child(
                        r.covariance_names
                            .iter()
                            .enumerate()
                            .map(|(i, n)| format!("{}: {n}", i + 1))
                            .collect::<Vec<_>>()
                            .join(" · "),
                    ),
                );
            }
            for c in &r.components {
                if c.area.is_some() {
                    body = body.child(div().text_size(px(12.)).child(format!(
                        "{} · center {} eV · area {} · FWHM {} eV",
                        c.name,
                        estimate(c.center_ev, c.center_standard_error_ev),
                        estimate(c.area, c.area_standard_error),
                        estimate(c.fwhm_ev, c.fwhm_standard_error_ev)
                    )));
                }
            }
        }
        body.into_any_element()
    }
    pub(crate) fn peak_inspector(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let t = self.theme;
        let mut body = div().flex().flex_col().gap_2().p_2();
        body = body.child(
            div()
                .text_size(px(12.))
                .text_color(t.text_muted)
                .child("Starting model"),
        );
        if let Some(name) = &self.peaks.name {
            body = body.child(name.clone());
        }
        body =
            body.child(
                div()
                    .flex()
                    .gap_2()
                    .child(button(&t, "peak-save-model", "Save model", false).on_click(
                        cx.listener(|app, _, _, cx| {
                            match app.peak_definition(cx) {
                                Err(e) => app.peaks.message = e,
                                Ok(model) => {
                                    let name = app
                                        .peaks
                                        .name
                                        .as_ref()
                                        .map(|n| n.read(cx).text().trim().to_owned())
                                        .unwrap_or_default();
                                    if name.is_empty() || name.chars().count() > 200 {
                                        app.peaks.message =
                                            "Use a model name of 1–200 characters".into();
                                    } else {
                                        app.peaks.archive.save_model(name, model);
                                        app.record("Saved peak model", None);
                                        app.peaks.message = "Model saved".into();
                                    }
                                }
                            }
                            cx.notify();
                        }),
                    ))
                    .child(button(&t, "peak-load-model", "Models ▾", false).on_click(
                        cx.listener(|app, e, w, cx| app.open_peak_menu(Menu::Models, e, w, cx)),
                    )),
            );
        let mut spacebar = div().flex().gap_1();
        for (space, label) in [
            (MeasurementSpace::Norm, "Norm"),
            (MeasurementSpace::Flat, "Flat"),
            (MeasurementSpace::Mu, "μ(E)"),
        ] {
            spacebar = spacebar.child(
                button(
                    &t,
                    SharedString::from(format!("peak-space-{label}")),
                    label,
                    self.peaks.model.space == space,
                )
                .on_click(cx.listener(move |app, _, _, cx| {
                    app.peaks.model.space = space;
                    app.peak_model_changed(cx);
                })),
            );
        }
        body = body
            .child(spacebar)
            .child(div().text_size(px(12.)).text_color(t.text_muted).child(
                match self.peaks.model.origin {
                    rexafs::prelude::AxisOrigin::Absolute => "Fit range · absolute eV".to_owned(),
                    rexafs::prelude::AxisOrigin::Reference { energy_ev } => {
                        format!("Fit range · eV from {energy_ev:.1}")
                    }
                    _ => "Fit range · eV from E₀".to_owned(),
                },
            ));
        for field in &self.peaks.range {
            body = body.child(field.clone());
        }
        body = body.child(
            button(
                &t,
                "peak-ranges",
                "Exclusions & baseline",
                self.peaks.ranges_open,
            )
            .on_click(cx.listener(|app, _, _, cx| {
                app.peaks.ranges_open = !app.peaks.ranges_open;
                cx.notify();
            })),
        );
        if self.peaks.ranges_open {
            for field in &self.peaks.extra_range {
                body = body.child(field.clone());
            }
            body = body
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(t.text_muted)
                        .child("Initialize outside this region, or exclude it from the final fit."),
                )
                .child(
                    div()
                        .flex()
                        .gap_1()
                        .child(
                            button(&t, "peak-initialize-baseline", "Initialize baseline", false)
                                .on_click(
                                    cx.listener(|app, _, _, cx| app.initialize_peak_baseline(cx)),
                                ),
                        )
                        .child(
                            button(&t, "peak-add-mask", "Exclude from fit", false).on_click(
                                cx.listener(|app, _, _, cx| {
                                    if app.peaks.busy {
                                        return;
                                    }
                                    match app.peak_exclusion(cx) {
                                        Ok(range) => {
                                            app.peaks.model.exclude.push(range);
                                            app.peak_model_changed(cx);
                                        }
                                        Err(e) => {
                                            app.peaks.message = e;
                                            cx.notify();
                                        }
                                    }
                                }),
                            ),
                        ),
                );
            for (index, range) in self.peaks.model.exclude.iter().enumerate() {
                body = body.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(format!("Excluded: {:.1} to {:.1} eV", range[0], range[1]))
                        .child(
                            button(&t, ("peak-remove-mask", index), "×", false).on_click(
                                cx.listener(move |app, _, _, cx| {
                                    if !app.peaks.busy {
                                        app.peaks.model.exclude.remove(index);
                                        app.peak_model_changed(cx);
                                    }
                                }),
                            ),
                        ),
                );
            }
        }
        body =
            body.child(
                div()
                    .flex()
                    .gap_2()
                    .child(button(&t, "peak-add", "Add component ▾", false).on_click(
                        cx.listener(|app, e, w, cx| app.open_peak_menu(Menu::Add, e, w, cx)),
                    ))
                    .child(
                        button(&t, "peak-advanced", "Constraints", self.peaks.advanced).on_click(
                            cx.listener(|app, _, _, cx| {
                                app.peaks.advanced = !app.peaks.advanced;
                                cx.notify();
                            }),
                        ),
                    ),
            );
        for component in &self.peaks.model.components {
            let name = component.name.clone();
            let mut card = div()
                .flex()
                .flex_col()
                .gap_1()
                .p_2()
                .border_1()
                .border_color(t.border)
                .rounded_md()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(div().flex_1().child(format!(
                            "{} · {}",
                            component.name,
                            shape_label(component.shape)
                        )))
                        .child(
                            button(
                                &t,
                                SharedString::from(format!("peak-remove-{name}")),
                                "×",
                                false,
                            )
                            .on_click(cx.listener(
                                move |app, _, _, cx| {
                                    if let Some(c) = app
                                        .peaks
                                        .model
                                        .components
                                        .iter()
                                        .find(|c| c.name == name)
                                        .cloned()
                                    {
                                        app.peaks.model.components.retain(|c| c.name != name);
                                        for p in c.parameters.values() {
                                            if !app.peaks.model.components.iter().any(|other| {
                                                other.parameters.values().any(|v| v == p)
                                            }) {
                                                app.peaks.model.parameters.vars.remove(p);
                                            }
                                        }
                                    }
                                    app.sync_peak_fields(cx);
                                    app.peak_model_changed(cx);
                                },
                            )),
                        ),
                );
            for field in self
                .peaks
                .fields
                .iter()
                .filter(|f| component.parameters.values().any(|n| n == &f.name))
            {
                let p = &self.peaks.model.parameters.vars[&field.name];
                let name = field.name.clone();
                if !self.peaks.advanced && field.name.ends_with("_reference") {
                    continue;
                }
                card = card.child(field.value.clone());
                if self.peaks.advanced {
                    card = card
                        .child(
                            button(
                                &t,
                                SharedString::from(format!("peak-fixed-{name}")),
                                "Fixed",
                                !p.vary,
                            )
                            .on_click(cx.listener(
                                move |app, _, _, cx| {
                                    if let Some(p) = app.peaks.model.parameters.vars.get_mut(&name)
                                    {
                                        p.vary = !p.vary;
                                    }
                                    app.peak_model_changed(cx);
                                },
                            )),
                        )
                        .child(field.minimum.clone())
                        .child(field.maximum.clone())
                        .child(field.expression.clone());
                }
            }
            if self.peaks.advanced
                && matches!(
                    component.shape,
                    PeakShape::Gaussian
                        | PeakShape::Lorentzian
                        | PeakShape::PseudoVoigt
                        | PeakShape::Voigt
                )
            {
                let name = component.name.clone();
                card = card.child(
                    button(
                        &t,
                        SharedString::from(format!("peak-baseline-{name}")),
                        "Baseline role",
                        component.role == PeakRole::Baseline,
                    )
                    .on_click(cx.listener(move |app, _, _, cx| {
                        if let Some(c) = app
                            .peaks
                            .model
                            .components
                            .iter_mut()
                            .find(|c| c.name == name)
                        {
                            c.role = if c.role == PeakRole::Baseline {
                                PeakRole::Peak
                            } else {
                                PeakRole::Baseline
                            };
                        }
                        app.peak_model_changed(cx);
                    })),
                )
            }
            body = body.child(card);
        }
        body.into_any_element()
    }
    pub(crate) fn peak_handle_specs(&self) -> Vec<(super::handles::HandleKey, f64)> {
        if !self.peaks.open
            || self.stage != Stage::Data
            || self.peaks.record.is_some()
            || self.peaks.busy
        {
            return vec![];
        }
        let Some(preview) = &self.peaks.preview else {
            return vec![];
        };
        vec![
            (
                super::handles::HandleKey::MeasurementStart,
                self.peaks.model.range[0] + preview.origin,
            ),
            (
                super::handles::HandleKey::MeasurementEnd,
                self.peaks.model.range[1] + preview.origin,
            ),
        ]
    }
    pub(crate) fn drag_peak_boundary(
        &mut self,
        key: super::handles::HandleKey,
        x: f64,
        cx: &mut Context<Self>,
    ) {
        let Some(preview) = &self.peaks.preview else {
            return;
        };
        let index = usize::from(key == super::handles::HandleKey::MeasurementEnd);
        let value = (((x - preview.origin) * 10.).round() / 10.).clamp(
            preview.x[0] - preview.origin,
            preview.x[preview.x.len() - 1] - preview.origin,
        );
        let mut range = self.peaks.model.range;
        range[index] = value;
        if range[0] >= range[1] {
            return;
        }
        self.update_peak_range(index, value, cx);
        self.peaks.range[index].update(cx, |field, cx| field.set_value(Some(value), cx));
        cx.notify();
    }
    fn step_peak(&mut self, delta: isize, cx: &mut Context<Self>) {
        if self.peaks.busy {
            return;
        }
        if let Some(index) = self.peaks.run {
            let count = self.peaks.archive.runs[index].rows.len();
            if count > 0 {
                let row = self.peaks.row.saturating_add_signed(delta).min(count - 1);
                self.load_peak_row(index, row, cx);
            }
        }
    }
    pub(crate) fn peak_handle_readout(&self, x: f64) -> String {
        let origin = self.peaks.preview.as_ref().map_or(0., |p| p.origin);
        match self.peaks.model.origin {
            rexafs::prelude::AxisOrigin::Absolute => format!("{x:.1} eV"),
            rexafs::prelude::AxisOrigin::Reference { energy_ev } => {
                format!("{:.1} eV from {energy_ev:.1}", x - origin)
            }
            _ => format!("{:.1} eV from E₀", x - origin),
        }
    }
}
fn number(value: Option<f64>) -> String {
    value
        .map(|v| format!("{v:.4}"))
        .unwrap_or_else(|| "—".into())
}

fn estimate(value: Option<f64>, error: Option<f64>) -> String {
    match (value, error) {
        (Some(v), Some(e)) => format!("{v:.4} ± {e:.2e}"),
        (value, _) => number(value),
    }
}

#[cfg(test)]
mod problem_tests {
    use super::*;
    use crate::{
        group_identity::GroupId,
        params::{DetectionMode, PipelineParams},
    };

    #[test]
    fn finished_rows_map_to_one_problem_each() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("a.dat");
        let mut data = String::from("# energy mu\n");
        for i in 0..=200 {
            let x = 8900. + i as f64 / 20.;
            let y = 0.2 + (-0.5 * ((x - 8905.) / 0.8).powi(2)).exp();
            data.push_str(&format!("{x:.6} {y:.15}\n"));
        }
        std::fs::write(&path, data).unwrap();
        let mut settings = PipelineParams::default();
        settings.import.mode = DetectionMode::MuColumn;
        settings.import.energy_col = Some(0);
        settings.import.mu_col = Some(1);
        let inputs = [FrameInput {
            group: GroupId::source(&path, DetectionMode::MuColumn),
            label: "a.dat".into(),
            path,
            derived: None,
            settings,
            recipe: None,
        }];
        let model = PeakFit::new(8900.0..=8910.0)
            .raw_mu()
            .absolute()
            .gaussian("p", 8904.5, 1.8, 2.)
            .constant_baseline(0.1);
        let mut run = PeakRun::new("test".into(), model, &inputs);
        run.freeze(&inputs, || false);
        let root = tmp.path().join("artifacts");
        let mut row = peak_fits::calculate(&run.model, &run.rows[0], &inputs[0], &root, || false);
        assert_eq!(row.status, FrameStatus::Succeeded, "{:?}", row.reason);
        let clean = row.summary.as_ref().unwrap().warnings.is_empty();
        assert_eq!(peak_row_problem(&row).is_none(), clean);

        row.summary.as_mut().unwrap().warnings = vec!["a".into(), "b".into()];
        let (severity, message) = peak_row_problem(&row).unwrap();
        assert_eq!(severity, ProblemSeverity::Warning);
        assert_eq!(message, format!("{PEAK_PROBLEM_PREFIX}: a; b"));

        row.status = FrameStatus::Failed;
        row.reason = Some("did not converge".into());
        let (severity, message) = peak_row_problem(&row).unwrap();
        assert_eq!(severity, ProblemSeverity::Error);
        assert_eq!(
            message,
            format!("{PEAK_PROBLEM_PREFIX} failed: did not converge")
        );

        row.status = FrameStatus::Cancelled;
        assert!(peak_row_problem(&row).is_none());
    }
}
