//! RMC mode in the existing Fitting workspace.
#[path = "rmc/structural_view.rs"]
mod structural_view;
use super::{
    button, chip,
    controls::{Menu, Tooltip, disclosure},
    fit_workspace::FitStep,
    molecule_view::{MoleculeScene, SceneAtom},
    section_label, segment, segmented,
};
use crate::plot_ranges::plot_builder;
use crate::{
    accessibility::Control,
    app::StudioApp,
    rmc_fitting::{self as engine, Event, FitMode, Progress, Request, Source},
    text::{noun_for, plural},
    widgets::{
        numeric_field::{FieldEvent, FieldKind, FieldPreview, NumericField},
        text_input::{InputEvent, TextInput},
    },
};
use gpui::{Context, Entity, IntoElement, ParentElement, Styled, div, prelude::*, px};
use rexafs::{
    rmc::{Configuration, ResidualTrendStatus},
    structure::{Edge, Element},
};
use ruviz::prelude::Plot;
use ruviz_gpui::RuvizPlot;
use std::{path::PathBuf, sync::Arc, time::Duration};

#[derive(Default)]
pub(crate) struct RmcState {
    pub project: engine::Project,
    pub control: Option<engine::Control>,
    diagnostic: Option<engine::diagnostics::DiagnosticControl>,
    diagnostic_generation: u64,
    refining: bool,
    refinement_plots: Vec<Entity<RuvizPlot>>,
    pub live: Option<Progress>,
    pub request: Option<Request>,
    pub phase: String,
    pub error: Option<String>,
    pub generation: u64,
    pub recovery: Option<PathBuf>,
    builder_generation: u64,
    builder_busy: bool,
    builder_error: Option<String>,
    fields: Vec<Entity<NumericField>>,
    texts: Vec<Entity<TextInput>>,
    plots: Vec<Entity<RuvizPlot>>,
    plotted_best: Option<u64>,
    structural_plots: Vec<Entity<RuvizPlot>>,
    structural_key: Option<(usize, usize)>,
    structural_source: Option<Arc<engine::structural::History>>,
    structural_pair: usize,
    structural_view: usize,
    structural_settings: bool,
    plot_tab: usize,
    fit_plot_view: usize,
    hide_initial_curve: bool,
    full_history: bool,
    advanced: bool,
    continuation: Option<Entity<NumericField>>,
    show_initial: bool,
    scene_key: Option<(usize, u64, bool)>,
    page: Option<FitStep>,
    path_page: Option<FitStep>,
}
impl RmcState {
    pub(crate) fn from_project(mut project: engine::Project) -> Self {
        let error = project
            .saved
            .as_ref()
            .and_then(|saved| saved.validate().err());
        if error.is_some() {
            project.saved = None;
        }
        let builder_error = if let Some(structure) = &project.structure {
            match engine::build_preview(structure, project.draft.repeats) {
                Ok(configuration) => {
                    project.configuration = Some(configuration);
                    None
                }
                Err(e) => {
                    project.configuration = None;
                    Some(e)
                }
            }
        } else {
            None
        };
        Self {
            project,
            error,
            builder_error,
            ..Default::default()
        }
    }
    /// Records that the worker has ended with `phase` (for example "Stopped"
    /// or "Attempt limit reached") and returns the window-footer status line.
    ///
    /// Messages raised against the active run, such as "Stop and save the
    /// current RMC run before starting another.", no longer apply once it has
    /// ended, so they are cleared instead of sitting beside the Resume button.
    /// The returned line replaces whatever the footer showed before the run,
    /// which is often an unrelated path-fit R-factor. A worker error is set
    /// after this call, so it is not lost.
    fn end_run(&mut self, phase: String) -> String {
        self.control = None;
        self.error = None;
        let unit = self.request.as_ref().map_or("attempts", |r| r.unit());
        let status = run_end_status(&phase, self.live.as_ref().map(|p| p.completed), unit);
        self.phase = phase;
        status
    }
}
/// Footer line for an RMC run that ended with `phase` after `completed`
/// attempts or generations (`None` when it ended during preparation).
fn run_end_status(phase: &str, completed: Option<usize>, unit: &str) -> String {
    let phase = phase.to_lowercase();
    match completed {
        Some(n) => format!(
            "RMC {phase} · {} completed {unit} · checkpoint saved",
            count(n)
        ),
        None => format!("RMC {phase} before preparation finished"),
    }
}
impl StudioApp {
    pub(super) fn fit_mode_picker(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        button(
            &self.theme,
            "fit-mode-picker",
            format!("Method: {} ▾", self.fit_mode.label()),
            false,
        )
        .on_click(cx.listener(|app, event, window, cx| {
            app.open_chrome_menu(Menu::FitMode, event, window, cx)
        }))
    }
    pub(super) fn fit_mode_menu(&self, cx: &mut Context<Self>) -> gpui::Div {
        let mut out = div().flex().flex_col().gap_2().p_2();
        for (i, mode) in FitMode::ALL.into_iter().enumerate() {
            out = out.child(
                chip(
                    &self.theme,
                    ("fit-mode", i),
                    mode.label(),
                    self.fit_mode == mode,
                )
                .on_click(cx.listener(move |app, _, window, cx| {
                    app.set_fit_mode(mode, cx);
                    app.close_chrome_menu(window, cx);
                })),
            );
        }
        out
    }
    pub(crate) fn set_fit_mode(&mut self, mode: FitMode, cx: &mut Context<Self>) {
        if mode == self.fit_mode {
            return;
        }
        self.rmc.scene_key = None;
        if mode == FitMode::Rmc {
            if !self.rmc.project.spectrum_defaults_applied
                && self.rmc.project.saved.is_none()
                && self.spectrum.is_some()
            {
                self.use_rmc_spectrum_ranges(cx);
            }
            self.rmc.path_page = Some(self.stage_view.fit_step);
            self.stage_view.fit_step = self.rmc.page.unwrap_or(FitStep::Structure);
        } else {
            self.rmc.page = Some(self.stage_view.fit_step);
            self.stage_view.fit_step = self.rmc.path_page.unwrap_or(FitStep::Model);
            self.refresh_structure(cx);
            self.rebuild_structure_plot(cx);
        }
        self.fit_mode = mode;
        cx.notify();
    }
    pub(super) fn rmc_job_bar(&self, cx: &mut Context<Self>) -> gpui::Div {
        if self.rmc.control.is_none() {
            return div();
        }
        let unit = self.rmc.request.as_ref().map_or("attempts", |r| r.unit());
        let paused = self.rmc.phase == "Paused";
        let pending =
            self.rmc.phase.starts_with("Pausing") || self.rmc.phase.starts_with("Stopping");
        let calculation = self
            .rmc
            .control
            .as_ref()
            .and_then(|c| c.calculation_progress());
        let mut bar = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .px_3()
            .py_2()
            .bg(self.theme.raised)
            .child(div().flex_1().min_w_0().flex().flex_col().gap_1()
                .child(format!("{} · {}", self.rmc.request.as_ref().map_or("RMC", |r| r.mode().label()), self.rmc.phase))
                .child(div().text_size(px(11.)).text_color(self.theme.text_muted).child(
                    if let Some(p) = &self.rmc.live {
                        format!("{} / {} {unit} · {:.1}% of budget · {}", count(p.completed), count(p.limit),
                            100. * p.completed as f64 / p.limit.max(1) as f64,
                            if paused { "Calculator retained for a fast resume" } else { "Updates at completed search steps" })
                    } else {
                        "Preparing potentials and exact scattering paths. This may take several minutes; you can stop safely.".into()
                    })));
        if let Some(calculation) = calculation {
            let mut detail = div().w_full().flex().flex_col().gap_1().text_size(px(11.));
            if !paused {
                detail = detail.child(format!(
                    "{} · {:.0} s since worker start",
                    calculation.description, calculation.elapsed_seconds
                ));
            }
            detail = detail.child(format!(
                "Scattering cache: {:.0} / {:.0} MiB · {} · {} snapshots retained",
                calculation.cache.bytes as f64 / engine::memory::MIB as f64,
                calculation.budget.bytes as f64 / engine::memory::MIB as f64,
                if calculation.budget.automatic {
                    "Auto"
                } else {
                    "Fixed limit"
                },
                calculation.cache.snapshots
            ));
            if calculation.budget.automatic && calculation.budget.memory.is_none() {
                detail = detail.child(div().text_color(self.theme.warn).child(
                    "Available memory could not be read; Auto is using the 256 MiB fallback.",
                ));
            }
            if let Some(warning) = calculation.cache_warning {
                detail = detail.child(div().text_color(self.theme.warn).child(warning));
            }
            bar = bar.child(detail);
        }
        if paused {
            bar = bar.child(
                button(
                    &self.theme,
                    "rmc-resume-live",
                    if self.rmc.live.as_ref().is_some_and(|p| p.completed == 0) {
                        "Start optimization"
                    } else {
                        "Resume"
                    },
                    true,
                )
                .disabled(self.rmc.diagnostic.is_some())
                .on_click(cx.listener(|app, _, _, cx| app.resume_rmc(cx))),
            );
        } else {
            bar = bar.child(
                button(&self.theme, "rmc-pause", "Pause", false)
                    .disabled(pending)
                    .when(pending, |d| d.opacity(0.45).cursor_default())
                    .on_click(cx.listener(|app, _, _, cx| {
                        if let Some(c) = &app.rmc.control {
                            c.pause();
                            app.rmc.phase = "Pausing after current attempt or generation".into();
                        }
                        cx.notify();
                    })),
            );
        }
        bar = bar.child(
            button(&self.theme, "rmc-stop", "Stop and save", false)
                .disabled(self.rmc.phase.starts_with("Stopping"))
                .when(self.rmc.phase.starts_with("Stopping"), |d| {
                    d.opacity(0.45).cursor_default()
                })
                .on_click(cx.listener(|app, _, _, cx| {
                    if let Some(c) = &app.rmc.control {
                        c.stop();
                        app.rmc.phase = "Stopping and saving".into();
                    }
                    cx.notify();
                })),
        );
        let mut out = div().flex().flex_col().child(bar);
        if let Some(p) = &self.rmc.live {
            out = out.child(
                div().h(px(3.)).bg(self.theme.border).child(
                    div()
                        .h_full()
                        .w(gpui::relative(
                            (p.completed as f32 / p.limit.max(1) as f32).clamp(0., 1.),
                        ))
                        .bg(self.theme.accent),
                ),
            );
        }
        out
    }
    fn rmc_source(&self) -> Source {
        Source {
            group_id: self.current_group_index().and_then(|ix| self.group_id(ix)),
            label: self.spectrum_label.to_string(),
            path: self.spectrum_path.clone(),
            fingerprint: self.spectrum_fingerprint,
            recipe: self.ui_params().clone(),
        }
    }
    fn rmc_source_stale(&self) -> bool {
        self.rmc.request.as_ref().is_some_and(|r| {
            r.source.group_id != self.current_group_index().and_then(|ix| self.group_id(ix))
                || r.source.fingerprint != self.spectrum_fingerprint
                || r.source.path != self.spectrum_path
        })
    }
    fn new_rmc_request(&self) -> Result<Request, String> {
        if self.fit_running || self.feff_running || self.batch_running {
            return Err("Wait for the active fit or scattering calculation to finish.".into());
        }
        if self.load_running || self.stale_plots.is_some() {
            return Err("Wait for successful spectrum processing.".into());
        }
        if !cfg!(feature = "refeff-runner") {
            return Err("RMC requires a desktop build with ReFEFF.".into());
        }
        let spectrum = self
            .spectrum
            .as_ref()
            .ok_or("Select a processed spectrum.")?;
        let config = self
            .rmc
            .project
            .configuration
            .clone()
            .ok_or("Pick a structure and build its supercell first.")?;
        Request::new(spectrum, config, &self.rmc.project.draft, self.rmc_source())
    }
    fn begin_rmc(
        &mut self,
        prepare_only: bool,
        resume: Option<Box<engine::SavedRun>>,
        cx: &mut Context<Self>,
    ) {
        if self.rmc.diagnostic.is_some() {
            self.rmc.error = Some("Finish or cancel the calibration preview first.".into());
            cx.notify();
            return;
        }
        if self.fit_running || self.feff_running || self.batch_running {
            self.rmc.error =
                Some("Wait for the active fit or scattering calculation to finish.".into());
            cx.notify();
            return;
        }
        if self.rmc.control.is_some() {
            self.rmc.error =
                Some("Stop and save the current RMC run before starting another.".into());
            return;
        }
        if resume.is_none() {
            if self.rmc.builder_busy || self.rmc.builder_error.is_some() {
                self.rmc.error = Some(
                    "Wait for a valid supercell preview before preparing or running RMC.".into(),
                );
                cx.notify();
                return;
            }
            if let Err(e) = self.commit_rmc_fields(cx) {
                self.rmc.error = Some(e);
                cx.notify();
                return;
            }
        }
        let request = match &resume {
            Some(saved) => saved.validate().and_then(|()| {
                let mut request = saved.request.clone();
                request.cache_mib = if let Some(field) = self.rmc.fields.get(28) {
                    field
                        .read(cx)
                        .pending_value(cx)
                        .map_err(|_| "Cache memory: enter a valid number or Auto.")?
                        .map(|v| v as usize)
                } else {
                    self.rmc.project.draft.cache_mib
                };
                engine::memory::validate_manual(request.cache_mib)?;
                Ok(request)
            }),
            None => self.new_rmc_request(),
        };
        let request = match request {
            Ok(v) => v,
            Err(e) => {
                self.rmc.error = Some(e);
                cx.notify();
                return;
            }
        };
        let Some(root) = crate::settings::app_dir() else {
            self.rmc.error = Some("Cannot create the RMC recovery directory.".into());
            return;
        };
        let dir = match tempfile::Builder::new().prefix("run-").tempdir_in({
            let path = root.join("rmc");
            if let Err(e) = std::fs::create_dir_all(&path) {
                self.rmc.error = Some(e.to_string());
                return;
            }
            path
        }) {
            Ok(dir) => dir.keep(),
            Err(e) => {
                self.rmc.error = Some(e.to_string());
                return;
            }
        };
        let recovery = dir.join("checkpoint.json");
        let (control, rx) = engine::spawn(request.clone(), resume, recovery.clone(), prepare_only);
        self.rmc.generation += 1;
        let generation = self.rmc.generation;
        let project = self.project_generation;
        self.rmc.control = Some(control);
        self.rmc.request = Some(request);
        self.rmc.recovery = Some(recovery);
        self.rmc.error = None;
        self.rmc.live = None;
        self.rmc.project.saved = None;
        self.rmc.project.refinement = None;
        self.rmc.refinement_plots.clear();
        self.rmc.continuation = None;
        self.rmc.plots.clear();
        self.rmc.scene_key = None;
        self.rmc.phase = "Preparing exact ReFEFF".into();
        // Replace an unrelated earlier footer message, such as a path-fit R-factor.
        self.status = "RMC run started · preparing exact ReFEFF".into();
        self.stage_view.fit_step = FitStep::Results;
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(250))
                    .await;
                let mut disconnected = false;
                let mut messages = Vec::new();
                loop {
                    match rx.try_recv() {
                        Ok(m) => messages.push(m),
                        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                            disconnected = true;
                            break;
                        }
                        Err(_) => break,
                    }
                }
                let keep = this
                    .update(cx, |app, cx| {
                        if app.rmc.generation != generation || app.project_generation != project {
                            return false;
                        }
                        for message in messages {
                            match message {
                                Event::Progress(p) => {
                                    app.rmc.live = Some(*p);
                                    if !app.rmc.phase.starts_with("Pausing")
                                        && !app.rmc.phase.starts_with("Stopping")
                                    {
                                        app.rmc.phase = "Running".into();
                                    }
                                    app.rebuild_rmc_plots(cx);
                                }
                                Event::Saved(saved) => {
                                    app.rmc.live = Some(saved.progress.clone());
                                    app.rmc.project.saved = Some(saved);
                                    app.rebuild_rmc_plots(cx);
                                }
                                Event::Paused => {
                                    app.rmc.phase = "Paused".into();
                                    app.rebuild_rmc_plots(cx);
                                }
                                Event::Stopped => {
                                    app.status = app.rmc.end_run("Stopped".into()).into();
                                }
                                Event::Finished(saved) => {
                                    app.rmc.live = Some(saved.progress.clone());
                                    app.status = app.rmc.end_run(saved.status.clone()).into();
                                    app.rmc.project.saved = Some(saved);
                                    app.rebuild_rmc_plots(cx);
                                    if app.fit_mode == FitMode::Rmc {
                                        app.stage_view.fit_step = FitStep::Results;
                                    }
                                }
                                Event::Error(e) => {
                                    app.rmc.end_run("Stopped with error".into());
                                    app.set_status(
                                        crate::app::StatusKind::Error,
                                        "RMC stopped with an error; see the RMC panel.",
                                    );
                                    app.rmc.error = Some(e);
                                }
                            }
                        }
                        if disconnected {
                            if app.rmc.control.is_some() {
                                app.rmc.phase = "Worker stopped".into();
                                app.rmc.error = Some("The RMC worker ended unexpectedly. The latest saved checkpoint remains available for recovery.".into());
                                app.set_status(
                                    crate::app::StatusKind::Error,
                                    "The RMC worker ended unexpectedly; see the RMC panel.",
                                );
                            }
                            app.rmc.control = None;
                        }
                        cx.notify();
                        !disconnected
                    })
                    .unwrap_or(false);
                if !keep {
                    break;
                }
            }
        })
        .detach();
        cx.notify();
    }
    fn resume_rmc(&mut self, cx: &mut Context<Self>) {
        if self.rmc.diagnostic.is_some() {
            return;
        }
        let Some(p) = self.rmc.live.as_ref() else {
            return;
        };
        let evolutionary = self
            .rmc
            .request
            .as_ref()
            .is_some_and(|r| r.evolution.is_some());
        let unit = if evolutionary {
            "generations"
        } else {
            "attempts"
        };
        let additional = self.rmc.continuation.as_ref().map_or(
            Ok(Some(if evolutionary { 50. } else { 10_000. })),
            |field| field.read(cx).pending_value(cx),
        );
        let total = if p.completed < p.limit {
            Ok(p.limit)
        } else {
            additional
                .map_err(|_| format!("Enter a valid number of additional {unit}."))
                .and_then(|v| v.ok_or_else(|| format!("Enter the number of additional {unit}.")))
                .and_then(|v| engine::continuation_limit(p.completed, p.limit, v as usize))
        };
        let total = match total {
            Ok(total) => total,
            Err(e) => {
                self.rmc.error = Some(e);
                cx.notify();
                return;
            }
        };
        self.rmc.error = None;
        self.rmc.project.refinement = None;
        self.rmc.refinement_plots.clear();
        if let Some(c) = &self.rmc.control {
            c.resume(total);
            self.rmc.phase = "Running".into();
            cx.notify();
            return;
        }
        let Some(mut saved) = self.rmc.project.saved.clone() else {
            self.rmc.error = Some("Load a saved RMC checkpoint first.".into());
            cx.notify();
            return;
        };
        saved.request.set_limit(total);
        self.begin_rmc(false, Some(saved), cx);
    }
    fn rmc_checkpoint_dialog(&mut self, cx: &mut Context<Self>) {
        let project = self.project_generation;
        let prompt = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: None,
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = prompt.await
                && let Some(path) = paths.first()
            {
                this.update(cx, |app, cx| {
                    if app.project_generation == project {
                        app.load_rmc_checkpoint(path.clone(), cx);
                    }
                })
                .ok();
            }
        })
        .detach();
    }
    fn recover_latest_rmc(&mut self, cx: &mut Context<Self>) {
        let Some(root) = crate::settings::app_dir() else {
            return;
        };
        match engine::latest_recovery(&root.join("rmc")) {
            Ok(path) => self.load_rmc_checkpoint(path, cx),
            Err(e) => {
                self.rmc.error = Some(e);
                cx.notify();
            }
        }
    }
    fn load_rmc_checkpoint(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.rmc.control.is_some() || self.rmc.diagnostic.is_some() {
            self.rmc.error =
                Some("Finish or cancel the current RMC work before loading a checkpoint.".into());
            cx.notify();
            return;
        }
        self.rmc.generation += 1; // Ignore an older asynchronous file selection.
        let project = self.project_generation;
        let generation = self.rmc.generation;
        cx.spawn(async move |this, cx| {
            let work_path = path.clone();
            let result = cx
                .background_executor()
                .spawn(async move { engine::load_run(&work_path) })
                .await;
            this.update(cx, |app, cx| {
                if app.project_generation != project
                    || app.rmc.generation != generation
                    || app.rmc.control.is_some()
                    || app.rmc.diagnostic.is_some()
                {
                    return;
                }
                match result {
                    Ok(saved) => {
                        app.rmc.project.refinement = None;
                        app.rmc.refinement_plots.clear();
                        app.rmc.plot_tab = 0;
                        app.rmc.request = Some(saved.request.clone());
                        app.rmc.live = Some(saved.progress.clone());
                        app.rmc.phase = "Saved run · ready to resume".into();
                        app.rmc.project.saved = Some(Box::new(saved));
                        app.rmc.recovery = Some(path);
                        app.rmc.error = None;
                        app.rmc.continuation = None;
                        app.rmc.plots.clear();
                        app.rmc.plotted_best = None;
                        app.rebuild_rmc_plots(cx);
                        app.stage_view.fit_step = FitStep::Results;
                    }
                    Err(e) => app.rmc.error = Some(e),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
    fn use_rmc_spectrum_ranges(&mut self, cx: &mut Context<Self>) {
        let recipe = self.ui_params().clone();
        let result = self
            .spectrum
            .as_ref()
            .ok_or_else(|| "Prepare a spectrum in Background first.".to_string())
            .and_then(|spectrum| {
                self.rmc
                    .project
                    .draft
                    .use_spectrum_ranges(spectrum, &recipe)
            });
        match result {
            Ok(()) => {
                self.rmc.project.spectrum_defaults_applied = true;
                self.rmc.fields.clear();
                self.rmc.texts.clear();
                self.rmc.error = None;
            }
            Err(e) => self.rmc.error = Some(e),
        }
        cx.notify();
    }
    fn suggest_rmc_supercell(&mut self, cx: &mut Context<Self>) {
        let Some(structure) = &self.rmc.project.structure else {
            return;
        };
        let read = |index: usize, fallback: f64| -> Result<f64, String> {
            self.rmc.fields.get(index).map_or(Ok(fallback), |f| {
                f.read(cx)
                    .pending_value(cx)
                    .map_err(|_| "Enter a valid cluster radius and displacement.".to_string())?
                    .ok_or_else(|| "Enter an explicit cluster radius and displacement.".to_string())
            })
        };
        let values = read(3, self.rmc.project.draft.options.cluster_radius).and_then(|radius| {
            read(6, self.rmc.project.draft.max_displacement)
                .map(|displacement| (radius, displacement))
        });
        let (radius, displacement) = match values {
            Ok((radius, displacement)) if radius > 0.5 && radius < 50. && displacement > 0. => {
                (radius, displacement)
            }
            _ => {
                self.rmc.builder_error = Some(
                    "Set a cluster radius between 0.5 and 50 Å and a positive displacement.".into(),
                );
                cx.notify();
                return;
            }
        };
        self.rmc.project.draft.options.cluster_radius = radius;
        self.rmc.project.draft.max_displacement = displacement;
        let draft = &self.rmc.project.draft;
        match rexafs::rmc::suggested_supercell_repeats(
            structure,
            2. * (draft.options.cluster_radius + draft.max_displacement),
        ) {
            Ok(repeats) => {
                self.rmc.project.draft.repeats = repeats;
                for (i, value) in repeats.into_iter().enumerate() {
                    if let Some(field) = self.rmc.fields.get(i) {
                        field.update(cx, |f, cx| f.set_value(Some(value as f64), cx));
                    }
                }
                self.build_rmc_supercell(cx);
            }
            Err(e) => {
                self.rmc.builder_error = Some(e.to_string());
                cx.notify();
            }
        }
    }
    /// Debounce only the geometry preview. It never prepares scattering, and a
    /// generation token prevents an older build replacing a newer draft/project.
    fn build_rmc_supercell(&mut self, cx: &mut Context<Self>) {
        self.rmc.builder_generation += 1;
        let generation = self.rmc.builder_generation;
        let project = self.project_generation;
        let result = (|| -> Result<_, String> {
            let structure = self
                .rmc
                .project
                .structure
                .clone()
                .ok_or("Choose a structure first.")?;
            let mut repeats = self.rmc.project.draft.repeats;
            for (i, field) in self.rmc.fields.iter().take(3).enumerate() {
                let value = field
                    .read(cx)
                    .pending_value(cx)
                    .map_err(|_| "Enter positive integer repeat counts.")?
                    .ok_or("Enter all three repeat counts.")?;
                if !(1. ..=10_000.).contains(&value) {
                    return Err("Repeat counts must be between 1 and 10,000.".into());
                }
                repeats[i] = value as usize;
            }
            Ok((structure, repeats))
        })();
        let (structure, repeats) = match result {
            Ok(value) => value,
            Err(e) => {
                self.rmc.builder_busy = false;
                self.rmc.builder_error = Some(e);
                cx.notify();
                return;
            }
        };
        self.rmc.builder_busy = true;
        self.rmc.builder_error = None;
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(150))
                .await;
            if !this
                .update(cx, |app, _| {
                    app.project_generation == project && app.rmc.builder_generation == generation
                })
                .unwrap_or(false)
            {
                return;
            }
            let result = cx
                .background_executor()
                .spawn(async move { engine::build_preview(&structure, repeats) })
                .await;
            this.update(cx, |app, cx| {
                if app.project_generation != project || app.rmc.builder_generation != generation {
                    return;
                }
                app.rmc.builder_busy = false;
                match result {
                    Ok(configuration) => {
                        app.rmc.project.configuration = Some(configuration);
                        app.rmc.project.draft.repeats = repeats;
                        app.rmc.scene_key = None;
                    }
                    Err(e) => app.rmc.builder_error = Some(e),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }
    fn use_rmc_structure(&mut self, cx: &mut Context<Self>) {
        let Some(summary) = self.structure.summary.clone() else {
            return;
        };
        if !self.rmc.project.spectrum_defaults_applied && self.spectrum.is_some() {
            self.use_rmc_spectrum_ranges(cx);
        }
        self.rmc.fields.clear();
        self.rmc.texts.clear();
        self.rmc.project.configuration = None;
        self.rmc.project.structure = Some((*summary.structure).clone());
        self.rmc.project.label = summary.structure.formula();
        if let Some(z) = self
            .structure
            .absorber
            .as_ref()
            .and_then(|s| Element::from_symbol(s))
            .map(|e| e.z)
        {
            self.rmc.project.draft.absorber = z;
        }
        self.suggest_rmc_supercell(cx);
        self.stage_view.fit_step = FitStep::Calculate;
    }
    pub(super) fn rmc_workspace(&mut self, cx: &mut Context<Self>) -> gpui::Div {
        let t = self.theme;
        let step = self.stage_view.fit_step;
        if matches!(step, FitStep::Calculate | FitStep::Model) {
            self.ensure_rmc_fields(cx);
        }
        let mut nav = div().flex().flex_wrap().gap_2().px_3().py_2();
        for (i, (dest, label)) in [
            (FitStep::Structure, "1 · Structure"),
            (FitStep::Calculate, "2 · Supercell"),
            (FitStep::Model, "3 · Fit settings"),
            (FitStep::Results, "4 · Results"),
        ]
        .into_iter()
        .enumerate()
        {
            nav = nav.child(
                chip(&t, ("rmc-page", i), label, step == dest).on_click(cx.listener(
                    move |app, _, _, cx| {
                        app.stage_view.fit_step = dest;
                        app.rmc.scene_key = None;
                        cx.notify();
                    },
                )),
            );
        }
        nav = nav
            .child(div().flex_1())
            .child(
                button(&t, "rmc-recover-latest", "Recover latest run", false)
                    .disabled(self.rmc.control.is_some() || self.rmc.diagnostic.is_some())
                    .when(
                        self.rmc.control.is_some() || self.rmc.diagnostic.is_some(),
                        |d| d.opacity(0.45).cursor_default(),
                    )
                    .on_click(cx.listener(|app, _, _, cx| app.recover_latest_rmc(cx))),
            )
            .child(self.fit_mode_picker(cx));
        let mut out = div()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .flex()
            .flex_col()
            .child(nav)
            .child(self.rmc_workflow_header(cx))
            .child(self.rmc_job_bar(cx));
        if let Some(error) = &self.rmc.error {
            out = out.child(
                div()
                    .px_3()
                    .py_2()
                    .w_full()
                    .min_w_0()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap_2()
                    .bg(t.raised)
                    .child(div().flex_1().min_w_0().whitespace_normal().text_color(t.warn)
                        .child(error.replace("explicitly increase AccelerationSettings.max_total_paths", "increase Catalogue path limit in Fit settings and start a new run. Cache memory is a separate limit")))
                    .when(error.contains("max_total_paths"), |bar| bar.child(
                        button(&t, "rmc-path-settings", "Path limit…", false).flex_shrink_0()
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.stage_view.fit_step = FitStep::Model;
                                cx.notify();
                            }))))
                    .child(button(&t, "rmc-dismiss-error", "Dismiss", false).flex_shrink_0().on_click(
                        cx.listener(|app, _, _, cx| {
                            app.rmc.error = None;
                            cx.notify();
                        }),
                    )),
            );
        }
        if step != FitStep::Results && self.rmc.control.is_some() {
            out = out.child(div().px_3().py_2().text_color(t.warn)
                .child("A run is active. These settings are for the next run; use Results to pause, resume or stop the current run."));
        }
        if self.rmc_source_stale() {
            out=out.child(div().px_3().py_2().text_color(t.warn).child("Showing the saved run's inputs. The active spectrum or preprocessing differs; Resume continues the saved problem."));
        }
        if step == FitStep::Structure {
            let library = self.structure_library_panel(cx).into_any_element();
            let viewer = self.structure_center(cx).into_any_element();
            return out.child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .gap_2()
                    .child(
                        div()
                            .id("rmc-library")
                            .w(px(350.))
                            .min_h_0()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .overflow_y_scroll()
                            .child(library)
                            .child(
                                button(&t, "rmc-load-checkpoint", "Open RMC checkpoint…", false)
                                    .on_click(
                                        cx.listener(|app, _, _, cx| app.rmc_checkpoint_dialog(cx)),
                                    ),
                            ),
                    )
                    .child(viewer),
            );
        }
        if matches!(step, FitStep::Calculate | FitStep::Model) {
            let is_cell = step == FitStep::Calculate;
            let mut panel = div()
                .id("rmc-settings")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .p_3()
                .flex()
                .flex_col()
                .gap_3();
            // Label on the left, control starting at the value column of the
            // NumericField rows (98 px input + 14 px stepper + 22 px unit + gaps).
            let row = |label: &'static str, control: gpui::Div| {
                div()
                    .flex()
                    .items_center()
                    .px_3()
                    .gap_1p5()
                    .child(div().flex_1().min_w_0().child(label))
                    .child(div().w(px(140.)).flex_none().flex().child(control))
            };
            // Natural-width action instead of a stretched full-width bar.
            let action = |control: Control| div().flex().px_3().child(control);
            if is_cell {
                panel = panel.child(section_label(&t, "Periodic supercell")).child(with_tip(
                    &t,
                    "rmc-repeats",
                    "Edit the repeats to update the 3D preview. Scattering starts only when you prepare or run.",
                    div().flex().flex_col().gap_3().children(self.rmc.fields[..3].iter().cloned()),
                ));
                panel = panel.child(action(
                    button(&t, "rmc-supercell", "Use suggested size", false)
                        .on_click(cx.listener(|app, _, _, cx| app.suggest_rmc_supercell(cx))),
                ));
                if self.rmc.builder_busy {
                    panel = panel.child(hint(&t, "Updating preview…"));
                }
                if let Some(e) = &self.rmc.builder_error {
                    panel = panel.child(
                        div()
                            .text_color(t.warn)
                            .child(format!("{e} Showing the last valid preview.")),
                    );
                }
                if let Some(c) = &self.rmc.project.configuration {
                    let absorbers =
                        engine::selected_absorbers(c, &self.rmc.project.draft).map(|v| v.len());
                    if let Err(e) = &absorbers {
                        panel = panel.child(div().text_color(t.warn).child(e.clone()));
                    }
                    let sites = absorbers.unwrap_or(0);
                    panel = panel.child(div().rounded_md().p_2().bg(t.raised).child(format!(
                        "{} {} · {} absorbing {}",
                        count(c.atoms.len()),
                        noun_for(c.atoms.len(), "atom"),
                        count(sites),
                        noun_for(sites, "site")
                    )));
                    let elements: std::collections::BTreeSet<_> =
                        c.atoms.iter().map(|a| a.atomic_number).collect();
                    const ABSORBER_TIP: &str = "All sites of the selected element contribute to the average. Larger cells and more sites cost more.";
                    let mut choices = segmented(&t);
                    for (i, z) in elements.into_iter().enumerate() {
                        let label = Element::from_z(z).map_or("?", |e| e.symbol);
                        choices = choices.child(
                            describe(
                                &t,
                                segment(
                                    &t,
                                    ("rmc-element", z as usize),
                                    label,
                                    z == self.rmc.project.draft.absorber,
                                    i == 0,
                                ),
                                ABSORBER_TIP,
                            )
                            .on_click(cx.listener(
                                move |app, _, _, cx| {
                                    app.rmc.project.draft.absorber = z;
                                    app.rmc.scene_key = None;
                                    cx.notify();
                                },
                            )),
                        );
                    }
                    panel = panel.child(row("Absorber", choices));
                }
                let mut edges = segmented(&t);
                for (i, edge) in [Edge::K, Edge::L1, Edge::L2, Edge::L3]
                    .into_iter()
                    .enumerate()
                {
                    edges = edges.child(
                        segment(
                            &t,
                            ("rmc-edge", i),
                            edge.label(),
                            edge == self.rmc.project.draft.edge,
                            i == 0,
                        )
                        .on_click(cx.listener(move |app, _, _, cx| {
                            app.rmc.project.draft.edge = edge;
                            cx.notify();
                        })),
                    );
                }
                panel = panel.child(row("Edge", edges));
            } else {
                let auto_moves = self.rmc.project.draft.auto_moves;
                let search = self.rmc.project.draft.search;
                let evolutionary = search != engine::search::Mode::Rmc;
                let mut search_modes = div().flex().flex_wrap().gap_1();
                for (i, mode) in engine::search::Mode::ALL.into_iter().enumerate() {
                    search_modes = search_modes.child(
                        chip(&t, ("rmc-search", i), mode.label(), search == mode).on_click(
                            cx.listener(move |app, _, _, cx| {
                                app.rmc.project.draft.search = mode;
                                cx.notify();
                            }),
                        ),
                    );
                }
                panel = panel.child(section_label(&t, "Search method")).child(search_modes)
                    .when(evolutionary, |panel| panel.child(hint(&t,
                        "EA evaluates a population using selection, crossover and mutation. Hybrid adds local RMC moves per child. Both require fixed ΔE₀ and still pay the scattering cost.")))
                    .child(section_label(&t, "Run budget"))
                    .child(self.rmc.fields[if evolutionary { 35 } else { 8 }].clone())
                    .when(evolutionary, |panel| panel.child(self.rmc.fields[34].clone()).child(self.rmc.fields[36].clone()))
                    .when(search == engine::search::Mode::Hybrid, |panel| panel.child(self.rmc.fields[37].clone()))
                    .child(with_tip(
                        &t,
                        "rmc-workers",
                        format!("{} available · absorbers are distributed first", plural(engine::available_workers(), "CPU")),
                        self.rmc.fields[27].clone(),
                    ))
                    .child(with_tip(&t, "rmc-cache-memory",
                        "Auto adapts the scattering-cache limit to available physical memory while leaving headroom. This is a payload limit, not total app memory. A fixed MiB limit applies to new runs and cold resumes; live resumes keep the current policy.",
                        self.rmc.fields[28].clone()))
                    .child(with_tip(&t, "rmc-catalogue-limit",
                        "Auto resolves a total path-count guard from available memory when starting a new run. This is separate from the scattering cache. An explicit count overrides Auto; saved runs retain their captured limit. Increasing capacity does not omit or sample any paths.",
                        self.rmc.fields[38].clone()))
                    .child(hint(&t, format!("New run capacity: {} paths", count(engine::memory::catalogue_limit(
                        self.rmc.project.draft.max_total_paths, engine::memory::available_memory())))))
                    .child(disclosure(&t, "rmc-structural-settings", "Structural tracking", self.rmc.structural_settings, false)
                        .on_click(cx.listener(|app, _, _, cx| { app.rmc.structural_settings = !app.rmc.structural_settings; cx.notify(); })))
                    .when(self.rmc.structural_settings, |panel| panel.child(div().flex().flex_col().gap_2()
                        .child(hint(&t, "Moments cover this distance interval. Choose a shell interval for shell-specific coordination; settings apply to new runs."))
                        .children(self.rmc.fields[29..32].iter().cloned())
                        .when(!evolutionary, |panel| panel.child(self.rmc.fields[32].clone()))
                        .when(evolutionary, |panel| panel.child(hint(&t, "Samples each completed generation; current means the best individual.")))
                        .child(self.rmc.fields[33].clone())))
                    .when(search != engine::search::Mode::Genetic, |panel| panel.child(row("Moves", segmented(&t)
                        .child(describe(&t, segment(&t, "rmc-auto-moves", "Auto", auto_moves, true),
                            "Adjusts the starting move size during the run and cools toward improvements only.")
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.rmc.project.draft.auto_moves = true;
                                cx.notify();
                            })))
                        .child(describe(&t, segment(&t, "rmc-fixed-moves", "Fixed", !auto_moves, false),
                            "Keeps the move size and Metropolis tolerance fixed throughout the run.")
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.rmc.project.draft.auto_moves = false;
                                cx.notify();
                            }))))))
                    .child(with_tip(
                        &t,
                        "rmc-step-size",
                        if search == engine::search::Mode::Genetic {
                            "Base displacement for genetic mutation. Diversity and stagnation triggers can multiply this size."
                        } else if auto_moves {
                            "Move size at the start of the run. Auto moves adjust it as the run proceeds."
                        } else {
                            "Move size for the whole run."
                        },
                        self.rmc.fields[9].clone(),
                    ))
                    .child(section_label(&t, "R-space objective"));
                for i in 14..19 {
                    panel = panel.child(self.rmc.fields[i].clone());
                }
                if let Ok(draft) = self.pending_rmc_draft(cx)
                    && let Some(warning) =
                        engine::path_coverage_warning(draft.ranges.rmax, draft.options.path_radius)
                {
                    panel = panel.child(div().text_color(t.warn).child(warning));
                }
                let refine_energy = self.rmc.project.draft.refine_energy;
                panel = panel.child(action(describe(&t, button(&t, "rmc-spectrum-ranges", "Use spectrum ranges", false),
                    "Copies Transform windows and sampling. ReFEFF k coverage expands automatically.")
                    .on_click(cx.listener(|app, _, _, cx| app.use_rmc_spectrum_ranges(cx)))))
                    .child(section_label(&t, "Amplitude and energy"))
                    .child(self.rmc.fields[12].clone())
                    .child(row("ΔE₀", segmented(&t)
                        .child(describe(&t, segment(&t, "rmc-energy-fixed", "Fixed", !refine_energy, true),
                            "S₀² and ΔE₀ stay fixed during RMC.")
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.rmc.project.draft.refine_energy = false; cx.notify();
                            })))
                        .child(describe(&t, segment(&t, "rmc-energy-refine", "Refine", refine_energy, false),
                            "Refines theory ΔE₀ with S₀² fixed. Bounds and interval are in Advanced settings.")
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.rmc.project.draft.refine_energy = true; cx.notify();
                            })))))
                    .child(self.rmc.fields[13].clone());
                if self.rmc.diagnostic.is_some() {
                    panel = panel
                        .child(action(
                            button(&t, "rmc-cancel-calibration", "Cancel preview", false).on_click(
                                cx.listener(|app, _, _, cx| {
                                    app.rmc.diagnostic_generation += 1;
                                    app.rmc.diagnostic = None;
                                    app.rmc.refining = false;
                                    cx.notify();
                                }),
                            ),
                        ))
                        .child(hint(&t, "Estimating from the starting structure…"));
                } else {
                    panel = panel.child(action(
                        button(&t, "rmc-calibrate", "Estimate calibration…", false)
                            .disabled(self.rmc.control.is_some())
                            .on_click(
                                cx.listener(|app, _, _, cx| app.estimate_rmc_calibration(cx)),
                            ),
                    ));
                }
                if let Some(preview) = &self.rmc.project.calibration {
                    let b = &preview.result.best;
                    panel = panel.child(hint(
                        &t,
                        format!(
                            "Estimate: S₀² {:.3} · ΔE₀ {:+.2} eV · residual {:.5}",
                            b.s02, b.delta_e0, b.score
                        ),
                    ));
                    if let Some(warning) = engine::diagnostics::bound_warning(preview) {
                        panel = panel
                            .child(div().text_size(px(11.5)).text_color(t.warn).child(warning));
                    }
                    panel = panel.child(
                        describe(
                            &t,
                            button(&t, "rmc-use-calibration", "Use calibration", false),
                            "Fixed-structure estimate. Validate with a suitable reference before using it.",
                        )
                        .disabled(self.rmc.control.is_some() || self.rmc.diagnostic.is_some())
                        .on_click(cx.listener(|app, _, _, cx| app.apply_rmc_calibration(cx))),
                    );
                }
            }
            panel = panel.child(
                disclosure(
                    &t,
                    "rmc-advanced",
                    "Advanced settings",
                    self.rmc.advanced,
                    false,
                )
                .on_click(cx.listener(|app, _, _, cx| {
                    app.rmc.advanced = !app.rmc.advanced;
                    cx.notify();
                })),
            );
            if self.rmc.advanced {
                if is_cell {
                    panel = panel
                        .child(section_label(&t, "Exact scattering"))
                        .child(with_tip(
                            &t,
                            "rmc-scattering",
                            "Exact cached paths with fixed reference potentials. Adaptive scattering is experimental and disabled.",
                            div().flex().flex_col().gap_3().children(self.rmc.fields[3..6].iter().cloned()),
                        ))
                        .child(section_label(&t, "Physical bounds"))
                        .child(with_tip(
                            &t,
                            "rmc-bounds",
                            "Displacement is measured from the starting positions. Minimum distance rejects atomic overlaps.",
                            div().flex().flex_col().gap_3().children(self.rmc.fields[6..8].iter().cloned()),
                        ))
                        .child(with_tip(
                            &t,
                            "rmc-atom-indices",
                            "Indices start at 0. Atom 0 is fixed by default to anchor the structure.",
                            div()
                                .flex()
                                .flex_col()
                                .gap_3()
                                .child("Absorbing atom indices (blank = all)")
                                .child(self.rmc.texts[0].clone())
                                .child("Fixed atom indices")
                                .child(self.rmc.texts[1].clone()),
                        ));
                } else {
                    panel = panel
                        .child(with_tip(
                            &t,
                            "rmc-tolerance",
                            "Tolerance controls acceptance of worse moves; zero accepts improvements only. It is numerical, not a physical temperature.",
                            self.rmc.fields[10].clone(),
                        ))
                        .child(self.rmc.fields[11].clone())
                        .child(with_tip(
                            &t,
                            "rmc-pair-distances",
                            "Optional: Cu-O=1.5, Cu-Cu=2.0. Choose physically justified bounds for your material.",
                            div()
                                .flex()
                                .flex_col()
                                .gap_3()
                                .child("Pair minimum distances (Å)")
                                .child(self.rmc.texts[2].clone()),
                        ))
                        .child(section_label(&t, "Parallel calculation"))
                        .child(
                            describe(
                                &t,
                                chip(
                                    &t,
                                    "rmc-parallel-paths",
                                    "Parallel paths",
                                    self.rmc.project.draft.parallel_paths,
                                )
                                .role(accesskit::Role::CheckBox),
                                "Use spare CPU workers for paths within an absorber.",
                            )
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.rmc.project.draft.parallel_paths =
                                    !app.rmc.project.draft.parallel_paths;
                                cx.notify();
                            })),
                        );
                    if self.rmc.project.draft.refine_energy {
                        panel = panel.child(section_label(&t, "Energy refinement"));
                        for i in 24..27 {
                            panel = panel.child(self.rmc.fields[i].clone());
                        }
                    }
                    panel = panel.child(section_label(&t, "Calibration search"));
                    for i in 19..24 {
                        panel = panel.child(self.rmc.fields[i].clone());
                    }
                }
            }
            let scene = self.rmc_structure_panel(false, cx);
            return out.child(
                div()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .flex()
                    .child(
                        div()
                            .w(px(340.))
                            .flex_none()
                            .min_h_0()
                            .flex()
                            .flex_col()
                            .border_r_1()
                            .border_color(t.border)
                            .child(panel),
                    )
                    .child(scene),
            );
        }
        out.child(self.rmc_results(cx))
    }
}

impl StudioApp {
    /// Keep the next action above the workspace, like ordinary path fitting.
    /// Validation still reads pending controls before preparation or submission.
    fn rmc_workflow_header(&self, cx: &mut Context<Self>) -> gpui::Div {
        let t = self.theme;
        let step = self.stage_view.fit_step;
        let (action, blocker) = match step {
            FitStep::Structure => (
                "Use structure →",
                if self.structure.fetch_running {
                    Some("Loading structure…".into())
                } else if self.structure.summary.is_none() {
                    Some("Choose a structure".into())
                } else {
                    None
                },
            ),
            FitStep::Calculate => (
                "Next: fit settings →",
                if self.rmc.builder_busy {
                    Some("Updating supercell preview…".into())
                } else if let Some(error) = &self.rmc.builder_error {
                    Some(error.clone())
                } else if self.rmc.project.configuration.is_none() {
                    Some("Choose a structure and build its supercell".into())
                } else {
                    None
                },
            ),
            FitStep::Model => (
                match self.rmc.project.draft.search {
                    engine::search::Mode::Rmc => "Run RMC →",
                    engine::search::Mode::Genetic => "Run genetic search →",
                    engine::search::Mode::Hybrid => "Run hybrid search →",
                },
                self.rmc_form_blocker(cx),
            ),
            _ => ("Edit settings →", None),
        };
        let enabled = blocker.is_none();
        div()
            .flex_none()
            .px_3()
            .py_2()
            .flex()
            .items_center()
            .flex_wrap()
            .gap_2()
            .child(
                div()
                    .flex_1()
                    .min_w(px(140.))
                    .text_size(px(11.5))
                    .text_color(t.warn)
                    .when_some(blocker, |d, reason| d.child(reason)),
            )
            .when(step == FitStep::Model, |d| {
                d.child(
                    button(&t, "rmc-prepare", "Preview initial fit", false)
                        .disabled(!enabled)
                        .when(!enabled, |d| d.opacity(0.45).cursor_default())
                        .on_click(cx.listener(|app, _, _, cx| app.begin_rmc(true, None, cx))),
                )
            })
            .child(
                button(&t, "rmc-workflow-action", action, enabled)
                    .min_w(px(154.))
                    .justify_center()
                    .disabled(!enabled)
                    .when(!enabled, |d| d.opacity(0.45).cursor_default())
                    .on_click(cx.listener(move |app, _, _, cx| {
                        if !enabled {
                            return;
                        }
                        match step {
                            FitStep::Structure => app.use_rmc_structure(cx),
                            FitStep::Model => app.begin_rmc(false, None, cx),
                            _ => {
                                app.stage_view.fit_step = FitStep::Model;
                                app.rmc.advanced = false;
                                app.rmc.scene_key = None;
                                cx.notify();
                            }
                        }
                    })),
            )
    }

    /// Read uncommitted controls for both inline feedback and submission.
    fn pending_rmc_draft(&self, cx: &Context<Self>) -> Result<engine::Draft, String> {
        let mut draft = self.rmc.project.draft.clone();
        for (index, field) in self.rmc.fields.iter().enumerate() {
            let value = field
                .read(cx)
                .pending_value(cx)
                .map_err(|_| format!("{}: enter a valid number.", FIELD_LABELS[index]))?;
            if index == 27 {
                draft.workers = value.map(|v| v as usize);
                continue;
            }
            if index == 28 {
                draft.cache_mib = value.map(|v| v as usize);
                continue;
            }
            if index == 38 {
                draft.max_total_paths = value.map(|v| v as usize);
                continue;
            }
            let value = value
                .ok_or_else(|| format!("{} requires an explicit value.", FIELD_LABELS[index]))?;
            set_draft_field(&mut draft, index, value);
        }
        if self.rmc.texts.len() == 3 {
            draft.absorber_atoms = self.rmc.texts[0].read(cx).text().into();
            draft.fixed_atoms = self.rmc.texts[1].read(cx).text().into();
            draft.constraints.pairs = engine::pair_distances(self.rmc.texts[2].read(cx).text())?;
        }
        Ok(draft)
    }
    fn rmc_form_blocker(&self, cx: &Context<Self>) -> Option<String> {
        (|| -> Result<(), String> {
            if self.rmc.diagnostic.is_some() {
                return Err("Finish or cancel the calibration preview first.".into());
            }
            if self.rmc.control.is_some() {
                return Err("Stop and save the active run before starting a new one.".into());
            }
            if self.fit_running || self.feff_running || self.batch_running {
                return Err("Wait for the active calculation to finish.".into());
            }
            if self.load_running || self.stale_plots.is_some() {
                return Err("Finish spectrum processing in Background first.".into());
            }
            if !cfg!(feature = "refeff-runner") {
                return Err("This desktop build needs the ReFEFF backend.".into());
            }
            if self.rmc.builder_busy {
                return Err("Updating the supercell preview…".into());
            }
            if let Some(e) = &self.rmc.builder_error {
                return Err(e.clone());
            }
            let spectrum = self
                .spectrum
                .as_ref()
                .ok_or("Select a spectrum in the Groups panel.")?;
            let configuration = self
                .rmc
                .project
                .configuration
                .as_ref()
                .ok_or("Choose a structure and build its supercell first.")?;
            let draft = self.pending_rmc_draft(cx)?;
            draft.validate_form(spectrum, configuration)
        })()
        .err()
    }
    fn commit_rmc_fields(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        let draft = self.pending_rmc_draft(cx)?;
        if draft.repeats != self.rmc.project.draft.repeats {
            return Err("Wait for the supercell preview to finish updating.".into());
        }
        self.rmc.project.draft = draft;
        Ok(())
    }
    fn ensure_rmc_fields(&mut self, cx: &mut Context<Self>) {
        if !self.rmc.fields.is_empty() {
            return;
        }
        let d = &self.rmc.project.draft;
        let specs = [
            ("Repeat a", d.repeats[0] as f64),
            ("Repeat b", d.repeats[1] as f64),
            ("Repeat c", d.repeats[2] as f64),
            ("Cluster radius (Å)", d.options.cluster_radius),
            ("Path radius (Å)", d.options.path_radius),
            ("Maximum path legs", d.options.max_legs as f64),
            ("Maximum displacement (Å)", d.max_displacement),
            ("Minimum distance (Å)", d.min_distance),
            ("Attempt budget", d.steps as f64),
            ("Starting move size (Å)", d.step_size),
            ("Metropolis tolerance", d.temperature),
            ("Random seed", d.seed as f64),
            ("S₀²", d.s02),
            ("Fit ΔE₀ (eV)", d.delta_e0),
            ("k min (Å⁻¹)", d.ranges.kmin),
            ("k max (Å⁻¹)", d.ranges.kmax),
            ("R min (Å)", d.ranges.rmin),
            ("R max (Å)", d.ranges.rmax),
            ("k weight", d.ranges.kweight),
            ("ΔE₀ from (eV)", d.calibration_range[0]),
            ("ΔE₀ to (eV)", d.calibration_range[1]),
            ("ΔE₀ grid step (eV)", d.calibration_step),
            ("S₀² minimum", d.calibration_amplitude[0]),
            ("S₀² maximum", d.calibration_amplitude[1]),
            ("ΔE₀ minimum (eV)", d.energy_refinement.bounds[0]),
            ("ΔE₀ maximum (eV)", d.energy_refinement.bounds[1]),
            ("Every N attempts", d.energy_refinement.interval as f64),
            (
                "CPU workers",
                d.workers.unwrap_or_else(engine::available_workers) as f64,
            ),
            ("Cache memory (MiB)", d.cache_mib.unwrap_or(256) as f64),
            ("Distance min (Å)", d.structural.range[0]),
            ("Distance max (Å)", d.structural.range[1]),
            ("Distance bins", d.structural.bins as f64),
            ("Sample every N attempts", d.structural.stride as f64),
            ("Retain samples", d.structural.capacity as f64),
            ("Population", d.evolution.population as f64),
            ("Generation budget", d.evolution.generations as f64),
            ("Elite survivors", d.evolution.elite as f64),
            ("Local attempts per child", d.evolution.local_steps as f64),
            (
                "Catalogue path limit",
                d.max_total_paths.unwrap_or(1_000_000) as f64,
            ),
        ];
        for (index, (label, value)) in specs.into_iter().enumerate() {
            let kind = if matches!(index, 27 | 28 | 31..=38) {
                FieldKind::Integer { min: Some(1) }
            } else if matches!(index, 0..=2 | 5 | 8 | 11 | 18 | 26) {
                FieldKind::Integer { min: Some(0) }
            } else {
                FieldKind::Float
            };
            let step = match index {
                8 => 1_000.,
                9 => 0.01,
                10 => 0.0001,
                12 => 0.01,
                19..=21 | 24..=25 => 0.5,
                26 => 50.,
                28 => 64.,
                38 => 1_000_000.,
                22..=23 => 0.05,
                3 | 4 | 6 | 7 | 13..=17 => 0.1,
                _ => 1.,
            };
            let field = cx.new(|cx| {
                let placeholder = if index == 27 {
                    format!("auto ({})", engine::available_workers())
                } else if matches!(index, 28 | 38) {
                    "auto (available memory)".into()
                } else {
                    "required".into()
                };
                let value = if index == 27 {
                    d.workers.map(|v| v as f64)
                } else if index == 28 {
                    d.cache_mib.map(|v| v as f64)
                } else if index == 38 {
                    d.max_total_paths.map(|v| v as f64)
                } else {
                    Some(value)
                };
                NumericField::new(label, placeholder, value, kind, self.theme, cx).with_step(step)
            });
            cx.subscribe(&field, |_, _, _: &FieldPreview, cx| cx.notify())
                .detach();
            if index <= 2 {
                cx.subscribe(&field, |app, _, _: &FieldPreview, cx| {
                    app.build_rmc_supercell(cx)
                })
                .detach();
            }
            cx.subscribe(&field, move |app, _, event, cx| {
                if let FieldEvent::Changed(value) = event {
                    if index == 27 {
                        app.rmc.project.draft.workers = value.map(|v| v as usize);
                        cx.notify();
                        return;
                    }
                    if index == 28 {
                        app.rmc.project.draft.cache_mib = value.map(|v| v as usize);
                        cx.notify();
                        return;
                    }
                    if index == 38 {
                        app.rmc.project.draft.max_total_paths = value.map(|v| v as usize);
                        cx.notify();
                        return;
                    }
                    let Some(v) = value else {
                        if index <= 2 {
                            app.build_rmc_supercell(cx);
                        }
                        app.rmc.error = Some("RMC settings require explicit values.".into());
                        cx.notify();
                        return;
                    };
                    set_draft_field(&mut app.rmc.project.draft, index, *v);
                    if index <= 2 {
                        app.build_rmc_supercell(cx);
                    }
                    // Committing fields emits deferred change events. Do not let
                    // those erase a calibration/run error raised by the action
                    // that committed them.
                    if app.rmc.error.as_deref() == Some("RMC settings require explicit values.") {
                        app.rmc.error = None;
                    }
                    app.rmc.scene_key = None;
                    cx.notify();
                } else if index <= 2 && matches!(event, FieldEvent::Invalid(_)) {
                    // NumericField restored its previous value after rejection.
                    app.build_rmc_supercell(cx);
                }
            })
            .detach();
            self.rmc.fields.push(field);
        }
        let d = &self.rmc.project.draft;
        let pairs = d
            .constraints
            .pairs
            .iter()
            .map(|p| {
                format!(
                    "{}-{}={}",
                    Element::from_z(p.elements[0]).map_or("?", |e| e.symbol),
                    Element::from_z(p.elements[1]).map_or("?", |e| e.symbol),
                    p.minimum
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        for (index, (placeholder, value)) in [
            ("all atoms of selected element", d.absorber_atoms.clone()),
            ("0", d.fixed_atoms.clone()),
            ("Cu-O=1.5, Cu-Cu=2.0", pairs),
        ]
        .into_iter()
        .enumerate()
        {
            let field = cx.new(|cx| TextInput::new(placeholder, value, self.theme, cx));
            cx.subscribe(&field, move |app, _, event, cx| {
                if matches!(event, InputEvent::Edited(_)) {
                    cx.notify();
                }
                if let InputEvent::Edited(value) = event
                    && index == 0
                {
                    app.rmc.project.draft.absorber_atoms = value.to_string();
                    app.rmc.scene_key = None;
                    cx.notify();
                }
                if let InputEvent::Committed(value) = event {
                    match index {
                        0 => app.rmc.project.draft.absorber_atoms = value.to_string(),
                        1 => app.rmc.project.draft.fixed_atoms = value.to_string(),
                        _ => match engine::pair_distances(value) {
                            Ok(pairs) => app.rmc.project.draft.constraints.pairs = pairs,
                            Err(e) => {
                                app.rmc.error = Some(e);
                                cx.notify();
                                return;
                            }
                        },
                    }
                    app.rmc.scene_key = None;
                    app.rmc.error = None;
                    cx.notify();
                }
            })
            .detach();
            self.rmc.texts.push(field);
        }
    }
    pub(crate) fn restore_rmc_plots(&mut self, cx: &mut Context<Self>) {
        self.rmc.structural_key = None;
        self.rmc.structural_source = None;
        self.rmc.plotted_best = None;
        self.rebuild_rmc_plots(cx);
    }
    pub(crate) fn restyle_rmc(&mut self, cx: &mut Context<Self>) {
        for field in self.rmc.fields.iter().chain(self.rmc.continuation.iter()) {
            field.update(cx, |f, cx| f.set_theme(self.theme, cx));
        }
        for field in &self.rmc.texts {
            field.update(cx, |f, cx| f.set_theme(self.theme, cx));
        }
        self.restore_rmc_plots(cx);
    }
    fn rebuild_rmc_plots(&mut self, cx: &mut Context<Self>) {
        let (Some(progress), Some(request)) = (&self.rmc.live, &self.rmc.request) else {
            return;
        };
        let key = progress.best.evaluation.score.to_bits();
        let refresh_curves = self.rmc.plots.len() != 5 || self.rmc.plotted_best != Some(key);
        match result_plots(
            progress,
            request,
            self.theme,
            refresh_curves,
            !self.rmc.hide_initial_curve,
            self.rmc.full_history,
        ) {
            Ok(plots) => {
                for (i, plot) in plots {
                    if let Some(entity) = self.rmc.plots.get(i) {
                        entity.update(cx, |view, cx| view.set_plot_keep_view(plot, cx));
                    } else {
                        self.rmc
                            .plots
                            .push(plot_builder(plot).interactive().build(cx));
                    }
                }
                self.rmc.plotted_best = Some(key);
            }
            Err(e) => self.rmc.error = Some(e),
        }
        if refresh_curves {
            self.rmc.scene_key = None;
        }
    }
    fn rmc_results(&mut self, cx: &mut Context<Self>) -> gpui::Div {
        let t = self.theme;
        let mut out = div().flex_1().min_h_0().min_w_0().flex().flex_col();
        // Nothing to refine, reopen or export until a run exists.
        let no_run = self.rmc.live.is_none() && self.rmc.project.saved.is_none();
        let busy = self.rmc.control.is_some() || self.rmc.diagnostic.is_some();
        let export_disabled = self.rmc.project.saved.is_none()
            || (self.rmc.control.is_some() && self.rmc.phase != "Paused");
        let refine_disabled = self.rmc.project.saved.is_none()
            || (!self.rmc.refining
                && (self.rmc.diagnostic.is_some()
                    || (self.rmc.control.is_some() && self.rmc.phase != "Paused")));
        let open_disabled = no_run || busy;
        let mut actions = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .px_3()
            .py_2();
        for (i, label) in [
            "Fit plots",
            "Structure",
            "Convergence",
            "Run details",
            "Refinement",
            "Structural evolution",
        ]
        .into_iter()
        .enumerate()
        {
            actions = actions.child(
                chip(&t, ("rmc-result-tab", i), label, self.rmc.plot_tab == i).on_click(
                    cx.listener(move |app, _, _, cx| {
                        app.rmc.plot_tab = i;
                        cx.notify();
                    }),
                ),
            );
        }
        actions = actions
            .child(div().flex_1())
            .child(
                button(
                    &t,
                    "rmc-refine-best",
                    if self.rmc.refining {
                        "Cancel refinement"
                    } else {
                        "Refine best…"
                    },
                    false,
                )
                .disabled(refine_disabled)
                .when(refine_disabled, |d| d.opacity(0.45).cursor_default())
                .on_click(cx.listener(move |app, _, _, cx| {
                    if refine_disabled {
                        return;
                    }
                    if app.rmc.refining {
                        app.rmc.diagnostic_generation += 1;
                        app.rmc.diagnostic = None;
                        app.rmc.refining = false;
                    } else {
                        app.refine_rmc_best(cx);
                    }
                    cx.notify();
                })),
            )
            .child(
                button(&t, "rmc-open-run", "Open checkpoint…", false)
                    .disabled(open_disabled)
                    .when(open_disabled, |d| d.opacity(0.45).cursor_default())
                    .on_click(cx.listener(move |app, _, _, cx| {
                        if !open_disabled {
                            app.rmc_checkpoint_dialog(cx)
                        }
                    })),
            )
            .child(
                button(&t, "rmc-export", "Export result…", false)
                    .disabled(export_disabled)
                    .when(export_disabled, |d| d.opacity(0.45).cursor_default())
                    .on_click(cx.listener(move |app, _, _, cx| {
                        if !export_disabled {
                            app.export_rmc(cx)
                        }
                    })),
            );
        out = out.child(actions);
        if self.rmc.plot_tab == 5 {
            return out.child(self.rmc_structural_view(cx));
        }
        if self.rmc.plot_tab == 4 {
            return out.child(self.rmc_refinement_view(cx));
        }
        let Some(p) = &self.rmc.live else {
            return out.child(div().flex_1().flex().flex_col().justify_center().items_center().gap_3().p_4()
                .child(div().text_size(px(18.)).child(if self.rmc.control.is_some() { "Preparing your initial fit" } else { "Your RMC results will appear here" }))
                .child(hint(&t, if self.rmc.control.is_some() {
                    "Exact ReFEFF preparation runs in the background. Live curves will appear when the initial calculation finishes."
                } else { "Choose a structure, check the fit settings, then preview the initial fit or start optimization." }))
                .when(self.rmc.control.is_none(), |d| d.child(button(&t, "rmc-back-setup", "Go to fit settings →", true)
                    .on_click(cx.listener(|app, _, _, cx| { app.stage_view.fit_step = FitStep::Model; cx.notify(); })))));
        };
        if let Some(request) = &self.rmc.request {
            if let Ok(shifts) = p.best.energy_shifts(&request.problem) {
                let shift = shifts[0];
                let bounded = request
                    .settings
                    .energy_refinement
                    .as_ref()
                    .is_some_and(|policy| policy.bounds.contains(&shift));
                out = out.child(
                    div()
                        .px_3()
                        .py_1()
                        .text_size(px(12.))
                        .text_color(if bounded { t.warn } else { t.text_muted })
                        .child(format!(
                            "Best ΔE₀ {shift:+.2} eV · S₀² {:.3} fixed{}",
                            request.problem.datasets[0].exafs.s02,
                            if bounded {
                                " · energy bound reached"
                            } else {
                                ""
                            }
                        )),
                );
            }
        }
        let evolutionary = self
            .rmc
            .request
            .as_ref()
            .is_some_and(|r| r.evolution.is_some());
        let unit = if evolutionary {
            "generations"
        } else {
            "attempts"
        };
        let initial = p.initial.evaluation.score;
        let best = p.best.evaluation.score;
        let improvement = if initial > 0. {
            format!("{:.2}%", 100. * (initial - best) / initial)
        } else {
            "—".into()
        };
        if self.rmc.control.is_none() && self.rmc.project.saved.is_some() {
            let complete = p.completed >= p.limit;
            let mut resume = div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_3()
                .px_3()
                .py_2()
                .bg(t.raised)
                .child(div().flex_1().child(format!(
                    "{} · {} completed {unit}",
                    self.rmc.phase,
                    count(p.completed)
                )));
            if complete {
                let field = self.rmc.continuation.get_or_insert_with(|| {
                    cx.new(|cx| {
                        NumericField::new(
                            if evolutionary {
                                "Additional generations"
                            } else {
                                "Additional attempts"
                            },
                            "required",
                            Some(if evolutionary { 50. } else { 10_000. }),
                            FieldKind::Integer { min: Some(0) },
                            t,
                            cx,
                        )
                        .with_step(1_000.)
                    })
                });
                resume = resume.child(div().w(px(320.)).flex_none().child(field.clone()));
            }
            resume = resume.child(
                button(
                    &t,
                    "rmc-resume-saved",
                    if complete {
                        "Continue optimization →"
                    } else {
                        "Resume saved run →"
                    },
                    true,
                )
                .disabled(self.rmc.diagnostic.is_some())
                .on_click(cx.listener(|app, _, _, cx| app.resume_rmc(cx))),
            );
            out = out.child(resume);
        }
        let trend = if evolutionary {
            "Inspect population trend"
        } else {
            trend_label(&p.trend.status)
        };
        out = out.child(
            div()
                .flex()
                .gap_2()
                .px_3()
                .py_2()
                .child(metric(
                    &t,
                    "Best objective ↓",
                    format!("{best:.7}"),
                    format!("Initial {initial:.7}"),
                ))
                .child(metric(
                    &t,
                    "Reduction from initial",
                    improvement,
                    "Not a goodness-of-fit percentage".into(),
                ))
                .child(metric(
                    &t,
                    "Convergence",
                    trend.into(),
                    if evolutionary {
                        "Generations do not use the RMC plateau diagnostic".into()
                    } else {
                        match p.trend.status {
                            ResidualTrendStatus::InsufficientHistory => format!(
                                "Need at least {} attempts",
                                count(p.trend.settings.minimum_attempts)
                            ),
                            ResidualTrendStatus::StillChanging => {
                                "Continue and monitor the trend".into()
                            }
                            ResidualTrendStatus::ResidualPlateau => {
                                "Numerical plateau; inspect the fit".into()
                            }
                        }
                    },
                )),
        );
        if let Some(request) = &self.rmc.request {
            let data = &request.problem.datasets[0];
            if let rexafs::rmc::Objective::R(transform) = &data.objective {
                out = out.child(div().px_3().pb_2().text_size(px(11.5)).text_color(t.text_muted).child(format!(
                    "{} · R {:.2}–{:.2} Å · k {:.2}–{:.2} Å⁻¹ · k weight {} · fitting real + imaginary",
                    request.source.label, transform.rmin, transform.rmax, transform.kmin, transform.kmax, data.exafs.kweight)));
                if let Some(warning) =
                    engine::path_coverage_warning(transform.rmax, request.calculator.path_radius)
                {
                    out = out.child(
                        div()
                            .px_3()
                            .pb_2()
                            .text_size(px(11.5))
                            .text_color(t.warn)
                            .child(warning),
                    );
                }
            }
        }
        if self.rmc.plot_tab == 1 {
            return out.child(self.rmc_structure_panel(true, cx));
        }
        if self.rmc.plot_tab == 2 && evolutionary {
            let mut diagnostic = div().p_3().flex().flex_col().gap_2()
                .child(hint(&t, "Current curve: mean population objective. Best curve: elite individual. Neither a flat objective nor low diversity establishes a unique structure."));
            if let Some(generation) = p.evolution_history.last() {
                diagnostic = diagnostic.child(format!("Generation {} · diversity {:.5} Å · {} children retained their parent · {} local attempts ({} accepted)",
                    generation.generation, generation.diversity, generation.constraint_fallbacks,
                    generation.local_attempts, generation.local_accepted));
            }
            out = out.child(diagnostic);
            if let Some(plot) = self.rmc.plots.get(4) {
                out = out.child(div().flex_1().min_h_0().p_3().child(plot.clone()));
            }
            return out;
        }
        if self.rmc.plot_tab == 2 {
            let settings = &p.trend.settings;
            let mut diagnostic = div().px_3().py_2().flex().flex_col().gap_1().bg(t.surface)
                .child(match p.trend.status {
                    ResidualTrendStatus::InsufficientHistory => "Too early to assess convergence. Reaching the attempt budget alone does not establish a plateau.",
                    ResidualTrendStatus::StillChanging => "The recent residual is still changing. Continue the same run to assess whether it settles.",
                    ResidualTrendStatus::ResidualPlateau => "The recent residual has plateaued. This does not establish a unique or physically complete structure.",
                })
                .child(hint(&t, format!("Criterion: {} consecutive comparisons of {}-attempt windows, after {} attempts. This diagnostic does not stop the run.",
                    settings.stable_windows, count(settings.window), count(settings.minimum_attempts))));
            if let Some(w) = p.trend.windows.last() {
                diagnostic = diagnostic.child(format!(
                    "Recent attempts {}–{} · best improvement {} · mean change {}",
                    count(w.first_step),
                    count(w.last_step),
                    w.best_improvement
                        .map_or_else(|| "—".into(), |v| format!("{v:.3e}")),
                    w.mean_change
                        .map_or_else(|| "—".into(), |v| format!("{v:.3e}"))
                ));
            }
            diagnostic = diagnostic.child(
                div()
                    .flex()
                    .gap_3()
                    .child(
                        div()
                            .text_color(crate::plotting::trace_rgba(&t, 0))
                            .child("━ Current objective"),
                    )
                    .child(
                        div()
                            .text_color(crate::plotting::trace_rgba(&t, 2))
                            .child("━ Best objective"),
                    ),
            );
            let mut scope = div().flex().gap_2().items_center();
            for (i, label) in ["Recent trend", "All retained attempts"]
                .into_iter()
                .enumerate()
            {
                scope = scope.child(
                    chip(
                        &t,
                        ("rmc-trend-scope", i),
                        label,
                        self.rmc.full_history == (i == 1),
                    )
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.rmc.full_history = i == 1;
                        if app.rmc.plots.len() == 5 {
                            app.rmc.plots.pop();
                        }
                        app.rebuild_rmc_plots(cx);
                        cx.notify();
                    })),
                );
            }
            diagnostic = diagnostic.child(scope).child(hint(&t, "Recent view shows the latest four diagnostic windows. Changing this view does not change the convergence criterion."));
            out = out.child(diagnostic);
            if let Some(plot) = self.rmc.plots.get(4) {
                out = out.child(div().flex_1().min_h_0().p_3().child(plot.clone()));
            }
            return out;
        }
        if self.rmc.plot_tab == 3 {
            let attempts = if evolutionary {
                p.local_attempts
            } else {
                p.completed
            };
            let acceptance = if attempts > 0 {
                100. * p.accepted as f64 / attempts as f64
            } else {
                0.
            };
            let reuse = p
                .cache
                .active_reuse()
                .map(|v| format!("{:.1}%", 100. * v))
                .unwrap_or_else(|| "—".into());
            let sec = if p.completed > 0 {
                format!("{:.3} s inclusive", p.elapsed_seconds / p.completed as f64)
            } else {
                "Awaiting moves".into()
            };
            let about_open = self.ui.sections.contains("About these numbers");
            let section = |label: &'static str| div().mt_2().child(section_label(&t, label));
            let mut details = div()
                .id("rmc-run-details")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .p_3()
                .flex()
                .flex_col()
                .gap_1()
                .child(section_label(&t, "Optimization"))
                .child(detail_row(
                    &t,
                    if evolutionary {
                        "Generations"
                    } else {
                        "Attempts"
                    },
                    format!("{} / {}", count(p.completed), count(p.limit)),
                ))
                .child(detail_row(
                    &t,
                    if evolutionary {
                        "Local moves accepted"
                    } else {
                        "Accepted"
                    },
                    format!("{acceptance:.1}% of {attempts} attempts"),
                ))
                .child(detail_row(
                    &t,
                    "Constraint rejected",
                    count(p.constraint_rejected),
                ))
                .child(detail_row(
                    &t,
                    "Active time",
                    format!("{:.1} s", p.elapsed_seconds),
                ))
                .child(detail_row(
                    &t,
                    "Setup this session",
                    format!("{:.1} s", p.setup_seconds),
                ))
                .child(detail_row(
                    &t,
                    if evolutionary {
                        "Time per generation"
                    } else {
                        "Time per attempt"
                    },
                    sec,
                ))
                .child(section("Exact scattering cache"))
                .when_some(p.cache.warning(), |details, warning| {
                    details.child(div().text_color(t.warn).child(warning))
                })
                .when_some(self.rmc.request.as_ref(), |details, request| {
                    details
                        .child(detail_row(&t, "CPU workers", request.workers.to_string()))
                        .child(detail_row(
                            &t,
                            "Catalogue path limit",
                            count(request.max_total_paths.unwrap_or(1_000_000)),
                        ))
                        .child(detail_row(
                            &t,
                            "Parallel paths",
                            if request.parallel_paths { "On" } else { "Off" },
                        ))
                        .child(detail_row(
                            &t,
                            "Backend preparation threads",
                            request.calculator.threads.to_string(),
                        ))
                })
                .child(detail_row(&t, "Active path reuse", reuse))
                .child(detail_row(
                    &t,
                    "Cache limit",
                    format!(
                        "{:.0} MiB",
                        p.cache.limit_bytes as f64 / engine::memory::MIB as f64
                    ),
                ))
                .child(detail_row(
                    &t,
                    "One snapshot per absorber",
                    format!(
                        "{:.0} MiB estimated",
                        p.cache.minimum_bytes as f64 / engine::memory::MIB as f64
                    ),
                ))
                .child(detail_row(
                    &t,
                    "Retained snapshots",
                    format!(
                        "{} · {} prepared contexts",
                        p.cache.snapshots, p.cache.contexts
                    ),
                ))
                .child(detail_row(
                    &t,
                    "Cache hits / cold misses / repeat misses",
                    format!(
                        "{} / {} / {}",
                        p.cache.snapshot_hits, p.cache.cold_misses, p.cache.repeat_misses
                    ),
                ))
                .child(detail_row(
                    &t,
                    "Memory evictions / oversized results",
                    format!("{} / {}", p.cache.evictions, p.cache.oversized),
                ))
                .child(detail_row(
                    &t,
                    "Exact path calculations",
                    count(p.cache.exact as usize),
                ))
                .child(detail_row(
                    &t,
                    "Cached",
                    format!("{:.1} MiB", p.cache.bytes as f64 / 1048576.),
                ))
                .child(detail_row(
                    &t,
                    "Electronic setups",
                    format!(
                        "{} calculated · {} shared",
                        count(p.cache.electronic_preparations),
                        count(p.cache.shared_electronic_contexts)
                    ),
                ))
                .child(section("Objective"));
            for fit in &p.best.evaluation.datasets {
                details = details
                    .child(detail_row(
                        &t,
                        "Spectral residual",
                        format!("{:.7}", fit.score),
                    ))
                    .child(detail_row(
                        &t,
                        "Structural penalty",
                        format!("{:.7}", p.best.penalty),
                    ));
            }
            details = details.child(section("Recovery and provenance"));
            if let Some(scale) = p.move_scale {
                let step = self
                    .rmc
                    .request
                    .as_ref()
                    .map_or(0., |r| r.settings.moves.step_size)
                    * scale;
                details = details.child(detail_row(
                    &t,
                    "Auto move size",
                    format!("{step:.4} Å per coordinate"),
                ));
                if let Some(acceptance) = p.recent_acceptance {
                    details = details.child(detail_row(
                        &t,
                        "Last feedback window",
                        format!("{:.1}% accepted", acceptance * 100.),
                    ));
                }
            }
            if let Some(saved) = &self.rmc.project.saved {
                details = details.child(detail_row(
                    &t,
                    "Checkpoint state",
                    format!(
                        "{} completed {}",
                        count(saved.progress.completed),
                        saved.request.unit()
                    ),
                ));
            }
            if let Some(path) = &self.rmc.recovery {
                details =
                    details.child(detail_row(&t, "Recovery file", path.display().to_string()));
            }
            details = details
                .child(div().mt_2().child(
                    disclosure(&t, "rmc-about-numbers", "About these numbers", about_open, false).on_click(
                        cx.listener(|app, _, _, cx| {
                            if !app.ui.sections.remove("About these numbers") {
                                app.ui.sections.insert("About these numbers");
                            }
                            cx.notify();
                        }),
                    ),
                ))
                .when(about_open, |d| {
                    d.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .pl_2()
                            .child(hint(&t, "Time includes preparation and calculation but excludes pauses. Inclusive time per attempt is not an isolated scattering benchmark."))
                            .child(hint(&t, "Reuse = reused active paths / (reused active paths + exact calculations). Cache counters restart on a cold resume."))
                            .child(hint(&t, "The objective is the normalized R-space real + imaginary residual plus a structural penalty. R magnitude and k-space curves are diagnostic views."))
                            .child(hint(&t, "Checkpoints are saved after preparation, every 30 seconds at move boundaries, and on pause, stop or completion. Resume keeps the saved spectrum, preprocessing, structure, calibration and random sequence. Draft edits apply to a new run."))
                            .child(hint(&t, "Pause before exporting the latest completed state. Export includes the checkpoint, initial/best structures, k/R curves and a convergence report.")),
                    )
                });
            return out.child(details);
        }
        let mut views = div().flex().flex_wrap().items_center().gap_2().px_3();
        for (i, label) in ["k + R magnitude", "R real + imaginary", "All four plots"]
            .into_iter()
            .enumerate()
        {
            views = views.child(
                chip(
                    &t,
                    ("rmc-curve-view", i),
                    label,
                    self.rmc.fit_plot_view == i,
                )
                .on_click(cx.listener(move |app, _, _, cx| {
                    app.rmc.fit_plot_view = i;
                    cx.notify();
                })),
            );
        }
        views = views.child(
            button(
                &t,
                "rmc-toggle-initial",
                if self.rmc.hide_initial_curve {
                    "Show initial"
                } else {
                    "Hide initial"
                },
                false,
            )
            .on_click(cx.listener(|app, _, _, cx| {
                app.rmc.hide_initial_curve = !app.rmc.hide_initial_curve;
                app.rmc.plotted_best = None;
                // Recompute automatic limits when toggling traces, while ordinary
                // live updates keep the user's zoom/pan unchanged.
                app.rmc.plots.clear();
                app.rebuild_rmc_plots(cx);
                cx.notify();
            })),
        );
        let mut legend = div().flex().flex_wrap().items_center().gap_3();
        for (i, label) in ["Experimental", "Initial", "Best"].into_iter().enumerate() {
            if i == 1 && self.rmc.hide_initial_curve {
                continue;
            }
            legend = legend.child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .w(px(16.))
                            .h(px(3.))
                            .bg(crate::plotting::trace_rgba(&t, i)),
                    )
                    .child(hint(&t, label)),
            );
        }
        out = out.child(views.child(div().flex_1()).child(legend));
        let indices: &[usize] = match self.rmc.fit_plot_view {
            1 => &[2, 3],
            2 => &[0, 1, 2, 3],
            _ => &[0, 1],
        };
        let mut grid = div().flex_1().min_h_0().flex().flex_col().gap_2().p_3();
        for pair in indices.chunks(2) {
            let mut row = div().flex_1().min_h_0().min_w_0().flex().gap_2();
            for &index in pair {
                if let Some(plot) = self.rmc.plots.get(index) {
                    row = row.child(div().flex_1().min_w_0().min_h_0().child(plot.clone()));
                }
            }
            grid = grid.child(row);
        }
        out.child(grid).child(div().px_3().pb_2().child(hint(&t, "Experimental / initial / best · curves may extend outside the fitted window. Best fit and best structure always refer to the same state.")))
    }
    fn rmc_structure_panel(&mut self, result: bool, cx: &mut Context<Self>) -> gpui::Div {
        let configuration = if result {
            self.rmc
                .live
                .as_ref()
                .and_then(|p| {
                    if self.rmc.show_initial {
                        p.initial.structures.first()
                    } else {
                        p.best.structures.first()
                    }
                })
                .map(|s| s.configuration.clone())
        } else {
            self.rmc.project.configuration.clone()
        };
        let Some(configuration) = configuration else {
            return div()
                .flex_1()
                .p_3()
                .child("Choose a structure and build its supercell.");
        };
        let source = if result {
            self.rmc
                .request
                .as_ref()
                .and_then(|r| r.problem.datasets.first())
                .map(|d| d.exafs.absorbers.clone())
                .unwrap_or_default()
        } else {
            engine::selected_absorbers(&configuration, &self.rmc.project.draft).unwrap_or_default()
        };
        let score = self
            .rmc
            .live
            .as_ref()
            .map_or(0, |p| p.best.evaluation.score.to_bits());
        let key = (configuration.atoms.len(), score, self.rmc.show_initial);
        if self.rmc.scene_key != Some(key) {
            self.structure.scene = Some(Arc::new(configuration_scene(&configuration, &source)));
            self.rmc.scene_key = Some(key);
        }
        let canvas = self.molecule_canvas(cx);
        let mut out = div().flex_1().min_w_0().min_h_0().flex().flex_col();
        let mut bar = div().flex().flex_wrap().gap_2().p_2().child(format!(
            "{} atoms · {}",
            configuration.atoms.len(),
            if configuration.cell.is_some() {
                "Periodic supercell"
            } else {
                "Finite cluster"
            }
        ));
        if result {
            for (i, label) in ["Best structure", "Initial structure"]
                .into_iter()
                .enumerate()
            {
                bar = bar.child(
                    chip(
                        &self.theme,
                        ("rmc-structure-state", i),
                        label,
                        self.rmc.show_initial == (i == 1),
                    )
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.rmc.show_initial = i == 1;
                        app.rmc.scene_key = None;
                        cx.notify();
                    })),
                );
            }
        }
        if let Some(cell) = configuration.cell {
            bar = bar.child(format!(
                "a {:.3} Å · b {:.3} Å · c {:.3} Å",
                length(cell[0]),
                length(cell[1]),
                length(cell[2])
            ));
        }
        bar = bar.child(div().flex_1()).child(
            button(&self.theme, "rmc-reset-view", "Reset view", false).on_click(cx.listener(
                |app, _, _, cx| {
                    app.structure.camera = Default::default();
                    cx.notify();
                },
            )),
        );
        let elements: std::collections::BTreeSet<_> = configuration
            .atoms
            .iter()
            .map(|a| a.atomic_number)
            .collect();
        let legend = elements
            .into_iter()
            .map(|z| {
                let name = Element::from_z(z).map_or("?", |e| e.symbol);
                if source
                    .iter()
                    .any(|i| configuration.atoms[*i].atomic_number == z)
                {
                    format!("{name}: absorbing sites")
                } else {
                    name.into()
                }
            })
            .collect::<Vec<_>>()
            .join(" · ");
        out = out
            .child(bar)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .relative()
                    .overflow_hidden()
                    .child(canvas),
            )
            .child(div().px_3().py_2().child(hint(
                &self.theme,
                format!("{legend} · Drag to rotate · scroll to zoom"),
            )));
        out
    }
    fn refine_rmc_best(&mut self, cx: &mut Context<Self>) {
        if self.rmc.diagnostic.is_some()
            || (self.rmc.control.is_some() && self.rmc.phase != "Paused")
        {
            return;
        }
        let Some(saved) = self.rmc.project.saved.clone() else {
            return;
        };
        let (control, rx) = engine::diagnostics::spawn_refinement(saved, Default::default());
        self.rmc.diagnostic = Some(control);
        self.rmc.refining = true;
        self.rmc.error = None;
        self.rmc.plot_tab = 4;
        self.rmc.diagnostic_generation += 1;
        let generation = self.rmc.diagnostic_generation;
        let project = self.project_generation;
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(200))
                    .await;
                let result = match rx.try_recv() {
                    Ok(result) => Some(result),
                    Err(std::sync::mpsc::TryRecvError::Empty) => None,
                    Err(_) => Some(Err("Local refinement worker ended unexpectedly.".into())),
                };
                let keep = this
                    .update(cx, |app, cx| {
                        if app.project_generation != project
                            || app.rmc.diagnostic_generation != generation
                        {
                            return false;
                        }
                        if let Some(result) = result {
                            app.rmc.diagnostic = None;
                            app.rmc.refining = false;
                            match result {
                                Ok(result) => {
                                    app.rmc.project.refinement = Some(Box::new(result));
                                    app.rmc.refinement_plots.clear();
                                }
                                Err(error) => app.rmc.error = Some(error),
                            }
                            cx.notify();
                            false
                        } else {
                            true
                        }
                    })
                    .unwrap_or(false);
                if !keep {
                    break;
                }
            }
        })
        .detach();
        cx.notify();
    }

    fn rmc_refinement_view(&mut self, cx: &mut Context<Self>) -> gpui::Div {
        let t = self.theme;
        let mut out = div().flex_1().min_h_0().flex().flex_col().gap_2().p_3();
        if self.rmc.refining {
            return out.child("Refining the best structure…")
                .child(hint(&t, "Numerical coordinate gradients · original constraints · full scattering verification"));
        }
        let (Some(result), Some(saved)) = (&self.rmc.project.refinement, &self.rmc.project.saved)
        else {
            return out.child("Pause RMC, then choose Refine best.")
                .child(hint(&t, "Three local passes, limited to 5,000 geometry evaluations. The RMC checkpoint stays unchanged."));
        };
        let display = match engine::diagnostics::refinement_progress(saved, result) {
            Ok(p) => p,
            Err(error) => return out.child(div().text_color(t.warn).child(error)),
        };
        let stop = match result.stop {
            rexafs::rmc::LocalRefinementStop::SweepLimit => "Pass limit reached",
            rexafs::rmc::LocalRefinementStop::EvaluationLimit => "Evaluation limit reached",
            rexafs::rmc::LocalRefinementStop::NoDescent => "No further descent found",
            rexafs::rmc::LocalRefinementStop::Cancelled => "Cancelled",
        };
        out = out
            .child(format!(
                "Objective {:.6} → {:.6} · {} accepted moves · {} evaluations · {}",
                result.initial.evaluation.score,
                result.best.evaluation.score,
                result.history.len(),
                result.evaluations,
                stop
            ))
            .child(hint(
                &t,
                "Export result includes the RMC checkpoint and the separate refinement audit.",
            ));
        let mut legend = div().flex().flex_wrap().items_center().gap_3();
        for (i, label) in ["Experimental", "Before refinement", "After refinement"]
            .into_iter()
            .enumerate()
        {
            legend = legend.child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .w(px(16.))
                            .h(px(3.))
                            .bg(crate::plotting::trace_rgba(&t, i)),
                    )
                    .child(hint(&t, label)),
            );
        }
        out = out.child(legend);
        if self.rmc.refinement_plots.is_empty() {
            match result_plots(&display, &saved.request, t, true, true, false) {
                Ok(plots) => {
                    self.rmc.refinement_plots = plots
                        .into_iter()
                        .take(2)
                        .map(|(_, plot)| plot_builder(plot).interactive().build(cx))
                        .collect()
                }
                Err(error) => return out.child(div().text_color(t.warn).child(error)),
            }
        }
        let mut row = div().flex_1().min_h_0().flex().gap_2();
        for plot in &self.rmc.refinement_plots {
            row = row.child(div().flex_1().min_w_0().min_h_0().child(plot.clone()));
        }
        out.child(row)
    }

    fn estimate_rmc_calibration(&mut self, cx: &mut Context<Self>) {
        if self.rmc.diagnostic.is_some() || self.rmc.control.is_some() {
            return;
        }
        let input = (|| {
            if let Some(error) = self.rmc_form_blocker(cx) {
                return Err(error);
            }
            self.commit_rmc_fields(cx)?;
            Ok((
                self.new_rmc_request()?,
                engine::diagnostics::calibration_settings(&self.rmc.project.draft)?,
            ))
        })();
        let (request, settings) = match input {
            Ok(input) => input,
            Err(error) => {
                self.rmc.error = Some(error);
                cx.notify();
                return;
            }
        };
        let (control, rx) = engine::diagnostics::spawn_calibration(request, settings);
        self.rmc.diagnostic = Some(control);
        self.rmc.error = None;
        self.rmc.diagnostic_generation += 1;
        let generation = self.rmc.diagnostic_generation;
        let project = self.project_generation;
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(200))
                    .await;
                let result = match rx.try_recv() {
                    Ok(result) => Some(result),
                    Err(std::sync::mpsc::TryRecvError::Empty) => None,
                    Err(_) => Some(Err("Calibration worker ended unexpectedly.".into())),
                };
                let keep = this
                    .update(cx, |app, cx| {
                        if app.project_generation != project
                            || app.rmc.diagnostic_generation != generation
                        {
                            return false;
                        }
                        if let Some(result) = result {
                            app.rmc.diagnostic = None;
                            match result {
                                Ok(preview) => {
                                    app.rmc.project.calibration = Some(Box::new(preview))
                                }
                                Err(error) => app.rmc.error = Some(error),
                            }
                            cx.notify();
                            false
                        } else {
                            true
                        }
                    })
                    .unwrap_or(false);
                if !keep {
                    break;
                }
            }
        })
        .detach();
        cx.notify();
    }
    fn apply_rmc_calibration(&mut self, cx: &mut Context<Self>) {
        if self.rmc.control.is_some() || self.rmc.diagnostic.is_some() {
            return;
        }
        let apply = (|| {
            self.commit_rmc_fields(cx)?;
            let request = self.new_rmc_request()?;
            let preview = self
                .rmc
                .project
                .calibration
                .as_ref()
                .ok_or("Estimate calibration first.")?;
            if engine::diagnostics::input_key(&request)? != preview.input {
                return Err(
                    "Inputs changed. Estimate calibration again before applying it.".into(),
                );
            }
            let best = &preview.result.best;
            if !best.s02.is_finite() || best.s02 <= 0. || !best.delta_e0.is_finite() {
                return Err("Invalid saved calibration estimate.".into());
            }
            let mut draft = self.rmc.project.draft.clone();
            draft.s02 = best.s02;
            draft.delta_e0 = best.delta_e0;
            let spectrum = self
                .spectrum
                .as_ref()
                .ok_or("Select a processed spectrum.")?;
            draft.validate_form(spectrum, &request.problem.structures[0].configuration)?;
            self.rmc.project.draft = draft;
            self.rmc.fields.clear();
            self.rmc.texts.clear();
            Ok(())
        })();
        self.rmc.error = apply.err();
        cx.notify();
    }
    fn export_rmc(&mut self, cx: &mut Context<Self>) {
        if self.rmc.control.is_some() && self.rmc.phase != "Paused" {
            self.rmc.error =
                Some("Pause the run before exporting its latest completed state.".into());
            cx.notify();
            return;
        }
        let Some(saved) = self.rmc.project.saved.clone() else {
            return;
        };
        let refinement = self.rmc.project.refinement.clone();
        let calibration = self.rmc.project.calibration.clone();
        let prompt = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: None,
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = prompt.await
                && let Some(parent) = paths.first()
            {
                let parent = parent.clone();
                let result = cx
                    .background_executor()
                    .spawn(async move {
                        let directory = engine::export(&parent, &saved)?;
                        if let Some(result) = refinement {
                            engine::diagnostics::export_refinement(&directory, &saved, &result)?;
                        }
                        if let Some(calibration) = calibration {
                            std::fs::write(
                                directory.join("calibration-preview.json"),
                                serde_json::to_vec_pretty(&calibration)
                                    .map_err(|e| e.to_string())?,
                            )
                            .map_err(|e| e.to_string())?;
                        }
                        Ok::<_, String>(directory)
                    })
                    .await;
                this.update(cx, |app, cx| {
                    match result {
                        Ok(path) => {
                            app.status =
                                format!("RMC results exported to {}", path.display()).into()
                        }
                        Err(e) => app.rmc.error = Some(e),
                    }
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }
}
const FIELD_LABELS: [&str; 39] = [
    "Repeat a",
    "Repeat b",
    "Repeat c",
    "Cluster radius",
    "Path radius",
    "Maximum path legs",
    "Maximum displacement",
    "Minimum distance",
    "Attempt budget",
    "Starting move size",
    "Metropolis tolerance",
    "Random seed",
    "S₀²",
    "Fit ΔE₀",
    "k min",
    "k max",
    "R min",
    "R max",
    "k weight",
    "ΔE₀ from",
    "ΔE₀ to",
    "ΔE₀ grid step",
    "S₀² minimum",
    "S₀² maximum",
    "ΔE₀ minimum",
    "ΔE₀ maximum",
    "Every N attempts",
    "CPU workers",
    "Cache memory",
    "Distance min",
    "Distance max",
    "Distance bins",
    "Sample every N attempts",
    "Retain samples",
    "Population",
    "Generation budget",
    "Elite survivors",
    "Local attempts per child",
    "Catalogue path limit",
];
fn hint(t: &crate::theme::Theme, text: impl Into<gpui::SharedString>) -> gpui::Div {
    div()
        .text_size(px(11.5))
        .text_color(t.text_muted)
        .child(text.into())
}
/// Wrap a control that has no tooltip of its own (a numeric field or a group
/// of them) so its explanation appears on hover instead of as body text.
fn with_tip(
    t: &crate::theme::Theme,
    id: impl Into<gpui::ElementId>,
    text: impl Into<gpui::SharedString>,
    child: impl IntoElement,
) -> gpui::Stateful<gpui::Div> {
    let theme = *t;
    let label = text.into();
    div()
        .id(id)
        .tooltip(move |_, cx| {
            cx.new(|_| Tooltip {
                label: label.clone(),
                theme,
            })
            .into()
        })
        .child(child)
}
/// Attach one explanatory sentence to a control as both its accessible
/// description and its hover tooltip.
fn describe(t: &crate::theme::Theme, control: Control, text: &'static str) -> Control {
    let theme = *t;
    control.description(text).tooltip(move |_, cx| {
        cx.new(|_| Tooltip {
            label: text.into(),
            theme,
        })
        .into()
    })
}
/// One label/value line of the Run details table.
fn detail_row(
    t: &crate::theme::Theme,
    label: &'static str,
    value: impl Into<gpui::SharedString>,
) -> gpui::Div {
    div()
        .flex()
        .items_start()
        .gap_3()
        .text_size(px(11.5))
        .child(
            div()
                .w(px(170.))
                .flex_none()
                .text_color(t.text_muted)
                .child(label),
        )
        .child(div().flex_1().min_w_0().child(value.into()))
}
fn metric(t: &crate::theme::Theme, label: &str, value: String, detail: String) -> gpui::Div {
    div()
        .flex_1()
        .min_w_0()
        .p_2()
        .rounded_md()
        .border_1()
        .border_color(t.border)
        .bg(t.surface)
        .child(hint(t, label.to_string()))
        .child(div().text_size(px(17.)).child(value))
        .child(hint(t, detail))
}
fn count(n: usize) -> String {
    let digits = n.to_string();
    digits
        .chars()
        .enumerate()
        .fold(String::new(), |mut out, (i, c)| {
            if i > 0 && (digits.len() - i).is_multiple_of(3) {
                out.push(',');
            }
            out.push(c);
            out
        })
}
fn trend_label(status: &ResidualTrendStatus) -> &'static str {
    match status {
        ResidualTrendStatus::InsufficientHistory => "Not assessed yet",
        ResidualTrendStatus::StillChanging => "Still changing",
        ResidualTrendStatus::ResidualPlateau => "Residual plateau",
    }
}
fn set_draft_field(d: &mut engine::Draft, index: usize, v: f64) {
    match index {
        0..=2 => d.repeats[index] = v as usize,
        3 => d.options.cluster_radius = v,
        4 => d.options.path_radius = v,
        5 => d.options.max_legs = v.min(255.) as u8,
        6 => d.max_displacement = v,
        7 => d.min_distance = v,
        8 => d.steps = v as usize,
        9 => d.step_size = v,
        10 => d.temperature = v,
        11 => d.seed = v as u64,
        12 => d.s02 = v,
        13 => d.delta_e0 = v,
        14 => d.ranges.kmin = v,
        15 => d.ranges.kmax = v,
        16 => d.ranges.rmin = v,
        17 => d.ranges.rmax = v,
        18 => {
            d.ranges.kweight = v;
            d.ranges.kweights = vec![v];
            d.ranges.follow_transform = false;
        }
        19..=20 => d.calibration_range[index - 19] = v,
        21 => d.calibration_step = v,
        22..=23 => d.calibration_amplitude[index - 22] = v,
        24..=25 => d.energy_refinement.bounds[index - 24] = v,
        26 => d.energy_refinement.interval = v as usize,
        27 => d.workers = Some(v as usize),
        28 => d.cache_mib = Some(v as usize),
        29..=30 => d.structural.range[index - 29] = v,
        31 => d.structural.bins = v as usize,
        32 => d.structural.stride = v as usize,
        33 => d.structural.capacity = v as usize,
        34 => d.evolution.population = v as usize,
        35 => d.evolution.generations = v as usize,
        36 => d.evolution.elite = v as usize,
        37 => d.evolution.local_steps = v as usize,
        38 => d.max_total_paths = Some(v as usize),
        _ => unreachable!("unknown RMC field"),
    }
}
fn length(v: [f64; 3]) -> f64 {
    v.iter().map(|x| x * x).sum::<f64>().sqrt()
}
fn configuration_scene(c: &Configuration, absorbers: &[usize]) -> MoleculeScene {
    let center = c
        .cell
        .map(|cell| std::array::from_fn(|axis| cell.iter().map(|v| v[axis]).sum::<f64>() / 2.))
        .unwrap_or_else(|| {
            std::array::from_fn(|axis| {
                let lo = c
                    .atoms
                    .iter()
                    .map(|a| a.position[axis])
                    .fold(f64::INFINITY, f64::min);
                let hi = c
                    .atoms
                    .iter()
                    .map(|a| a.position[axis])
                    .fold(f64::NEG_INFINITY, f64::max);
                (lo + hi) / 2.
            })
        });
    let mut extent = c
        .atoms
        .iter()
        .map(|a| length(std::array::from_fn(|i| a.position[i] - center[i])))
        .fold(1.5, f64::max);
    if let Some(cell) = c.cell {
        for n in 0..8 {
            let corner = std::array::from_fn(|axis| {
                (0..3)
                    .filter(|i| n & (1 << i) != 0)
                    .map(|i| cell[i][axis])
                    .sum::<f64>()
                    - center[axis]
            });
            extent = extent.max(length(corner));
        }
    }
    let mut scene = MoleculeScene {
        center,
        extent,
        // A periodic configuration has no single absorber-centred radius guide.
        radius: 0.,
        atoms: c
            .atoms
            .iter()
            .enumerate()
            .map(|(i, a)| SceneAtom {
                pos: a.position,
                z: a.atomic_number as u32,
                index: Some(i),
                shell: 0,
                absorber: absorbers.contains(&i),
                faded: false,
                label: format!(
                    "{} {i}",
                    Element::from_z(a.atomic_number).map_or("?", |e| e.symbol)
                ),
            })
            .collect(),
        ..Default::default()
    };
    if let Some(cell) = c.cell {
        let corner = |n: usize| {
            std::array::from_fn(|j| {
                (0..3)
                    .filter(|i| n & (1 << i) != 0)
                    .map(|i| cell[i][j])
                    .sum()
            })
        };
        for n in 0..8 {
            for axis in 0..3 {
                if n & (1 << axis) == 0 {
                    scene.edges.push([corner(n), corner(n | (1 << axis))]);
                }
            }
        }
    }
    scene
}
fn result_plots(
    p: &Progress,
    request: &Request,
    theme: crate::theme::Theme,
    refresh_curves: bool,
    show_initial: bool,
    full_history: bool,
) -> Result<Vec<(usize, Plot)>, String> {
    use rexafs::rmc::{Objective, transform_spectrum_fourier};
    engine::validate_progress(p, request)?;
    let mut out = Vec::new();
    if refresh_curves {
        let dataset = request
            .problem
            .datasets
            .first()
            .ok_or("Missing saved dataset")?;
        let Objective::R(transform) = &dataset.objective else {
            return Err("The desktop result viewer currently requires R space.".into());
        };
        let weight = dataset.exafs.kweight as f64;
        let curves = [
            ("Experimental", &dataset.exafs.chi),
            ("Initial", &p.initial.evaluation.datasets[0].chi),
            ("Best", &p.best.evaluation.datasets[0].chi),
        ];
        let mut kp = Plot::new()
            .theme(theme.plot_theme())
            .xlabel("k (Å⁻¹)")
            .ylabel(crate::plotting::chik_label(weight));
        let mut rs: Vec<_> = ["|χ(R)|", "Re χ(R)", "Im χ(R)"]
            .iter()
            .map(|name| {
                Plot::new()
                    .theme(theme.plot_theme())
                    .xlabel("R (Å)")
                    .ylabel(*name)
                    .xlim(0., transform.rmax + 1.)
            })
            .collect();
        for (i, (label, chi)) in curves.into_iter().enumerate() {
            if i == 1 && !show_initial {
                continue;
            }
            let a =
                transform_spectrum_fourier(&dataset.exafs.k, chi, dataset.exafs.kweight, transform)
                    .map_err(|e| e.to_string())?;
            let weighted: Vec<_> = dataset
                .exafs
                .k
                .iter()
                .zip(chi)
                .map(|(k, x)| x * k.powf(weight))
                .collect();
            kp = kp
                .line(&dataset.exafs.k, &weighted)
                .label(label)
                .color(crate::plotting::trace_color(&theme, i))
                .into();
            for (plot, y) in
                rs.iter_mut()
                    .zip([&a.r_space.chir_mag, &a.r_space.chir_re, &a.r_space.chir_im])
            {
                *plot = plot
                    .clone()
                    .line(a.r_space.r.as_slice(), y.as_slice())
                    .label(label)
                    .color(crate::plotting::trace_color(&theme, i))
                    .into();
            }
        }

        let mut curves = vec![kp];
        curves.extend(rs);
        out.extend(curves.into_iter().enumerate());
    }
    let recent = p
        .trend
        .settings
        .window
        .saturating_mul(p.trend.settings.stable_windows.saturating_add(1));
    let history = if full_history {
        &p.history[..]
    } else {
        &p.history[p.history.len().saturating_sub(recent)..]
    };
    let mut x: Vec<_> = history.iter().map(|s| s.step as f64).collect();
    let mut current: Vec<_> = history.iter().map(|s| s.score).collect();
    let mut best: Vec<_> = history.iter().map(|s| s.best_score).collect();
    if request.evolution.is_some() {
        x = p
            .evolution_history
            .iter()
            .map(|g| g.generation as f64)
            .collect();
        current = p
            .evolution_history
            .iter()
            .map(|g| g.mean_score.unwrap_or(g.best_score))
            .collect();
        best = p.evolution_history.iter().map(|g| g.best_score).collect();
    }
    if x.is_empty() {
        x.push(p.completed as f64);
        current.push(p.current_score);
        best.push(p.best.evaluation.score);
    }
    let trend = Plot::new()
        .theme(theme.plot_theme())
        .xlabel(if request.evolution.is_some() {
            "Completed generations"
        } else {
            "Completed attempts"
        })
        .ylabel("Objective")
        .line(&x, &current)
        .label("Current")
        .color(crate::plotting::trace_color(&theme, 0))
        .line(&x, &best)
        .label("Best")
        .color(crate::plotting::trace_color(&theme, 2))
        .into();
    out.push((4, trend));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ending_a_run_clears_messages_about_the_active_run() {
        let mut state = RmcState {
            phase: "Stopping and saving".into(),
            error: Some("Stop and save the current RMC run before starting another.".into()),
            ..Default::default()
        };
        assert_eq!(
            state.end_run("Stopped".into()),
            "RMC stopped before preparation finished"
        );
        assert_eq!(state.phase, "Stopped");
        assert!(state.error.is_none() && state.control.is_none());
        assert_eq!(
            run_end_status("Stopped", Some(1_499), "attempts"),
            "RMC stopped · 1,499 completed attempts · checkpoint saved"
        );
    }
    #[test]
    fn periodic_preview_frames_cell_and_preserves_coordinates() {
        let configuration = Configuration {
            atoms: vec![
                rexafs::rmc::Atom {
                    atomic_number: 29,
                    position: [0.; 3],
                },
                rexafs::rmc::Atom {
                    atomic_number: 8,
                    position: [2., 2., 2.],
                },
            ],
            cell: Some([[8., 0., 0.], [2., 7., 0.], [0., 1., 6.]]),
        };
        let scene = configuration_scene(&configuration, &[0]);
        assert_eq!(scene.center, [5., 4., 3.]);
        assert_eq!(scene.atoms[1].pos, configuration.atoms[1].position);
        assert_eq!(scene.radius, 0.); // No absorber-centred guides on a periodic cell.
        for edge in scene.edges {
            for point in edge {
                assert!(
                    length(std::array::from_fn(|i| point[i] - scene.center[i])) <= scene.extent
                );
            }
        }
    }
}
