//! Joint fit setup: every spectrum owns an explicit list of path identities.
use super::{chip, fit_workspace::FitStep};
use crate::{
    app::{FitProvenance, StudioApp},
    fitting::{FitHistoryEntry, FitPathSpec, FitVarSpec},
    joint_fitting::{self, JointConfig, JointDataset},
    params::PipelineParams,
};
use gpui::{Context, IntoElement, ParentElement, Styled, div, prelude::*, px};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

pub(crate) struct JointState {
    pub config: JointConfig,
    pub setup: bool,
    pub selected: Option<(usize, Option<PathBuf>)>,
    pub edit_paths: Option<usize>,
    pub advanced: bool,
    pub fields: std::collections::BTreeMap<
        String,
        (String, gpui::Entity<crate::widgets::text_input::TextInput>),
    >,
    pub result_config: Option<JointConfig>,
    pub result_index: usize,
}
impl Default for JointState {
    fn default() -> Self {
        Self {
            config: Default::default(),
            setup: true,
            selected: None,
            edit_paths: None,
            advanced: false,
            fields: Default::default(),
            result_config: None,
            result_index: 0,
        }
    }
}
impl StudioApp {
    pub(super) fn joint_plotted_dataset_id(&self) -> Option<usize> {
        if self.stage_view.fit_step == FitStep::Model {
            return self.model_preview_dataset_id();
        }
        if !self.joint.config.enabled {
            return None;
        }
        let id = self
            .joint
            .result_config
            .as_ref()?
            .datasets
            .get(self.joint.result_index)?
            .id;
        self.joint
            .config
            .datasets
            .iter()
            .any(|d| d.id == id)
            .then_some(id)
    }
    pub(crate) fn joint_params(&self, file: &Path) -> PipelineParams {
        self.catalog
            .find_by_path(file)
            .map(|i| self.effective_params(i))
            .unwrap_or(&self.params)
            .clone()
    }
    pub(crate) fn joint_dataset_params(&self, dataset: &JointDataset) -> Option<PipelineParams> {
        match dataset.group_id {
            Some(id) => self
                .derived
                .iter()
                .find(|d| d.id == id)
                .map(|d| d.params.as_ref().unwrap_or(&self.params).clone()),
            None => Some(self.joint_params(&dataset.file)),
        }
    }
    pub(crate) fn joint_dataset_input(
        &self,
        dataset: &JointDataset,
    ) -> Result<crate::publication::SpectrumInput, String> {
        let params = self.joint_dataset_params(dataset).ok_or_else(|| {
            format!(
                "{}: source group is missing. Restore it or remove this fit dataset.",
                dataset.label
            )
        })?;
        Ok(crate::publication::SpectrumInput {
            source_error: None,
            label: dataset.label.clone(),
            path: dataset.file.clone(),
            params,
            group: dataset
                .group_id
                .and_then(|id| self.derived.iter().find(|d| d.id == id).cloned()),
            data: None,
        })
    }
    pub(crate) fn current_spectrum_input(&self) -> crate::publication::SpectrumInput {
        crate::publication::SpectrumInput {
            source_error: None,
            label: self.current_group_label().to_string(),
            path: self.current_path.clone(),
            params: self.ui_params().clone(),
            group: self
                .selected
                .filter(|&ix| ix >= crate::app::DERIVED_BASE)
                .and_then(|ix| self.derived.get(ix - crate::app::DERIVED_BASE))
                .cloned(),
            data: None,
        }
    }
    pub(super) fn add_joint_groups(&mut self, indices: Vec<usize>, cx: &mut Context<Self>) {
        let inputs: Vec<_> = indices
            .into_iter()
            .filter(|&ix| self.valid_group_index(ix))
            .map(|ix| {
                if ix >= crate::app::DERIVED_BASE {
                    let group = &self.derived[ix - crate::app::DERIVED_BASE];
                    (
                        group.source.clone().unwrap_or_default(),
                        Some(group.id),
                        self.entry_label(ix),
                    )
                } else {
                    let file = self.catalog.path(ix);
                    let label = self.entry_label(ix);
                    (file, None, label)
                }
            })
            .collect();
        self.add_joint_sources(inputs, cx);
    }
    pub(super) fn add_joint_current(&mut self, cx: &mut Context<Self>) {
        if let Some(ix) = self.selected {
            self.add_joint_groups(vec![ix], cx);
        } else if !self.current_path.as_os_str().is_empty() {
            self.add_joint_sources(
                vec![(
                    self.current_path.clone(),
                    None,
                    self.current_group_label().to_string(),
                )],
                cx,
            );
        }
    }
    fn add_joint_sources(
        &mut self,
        sources: Vec<(PathBuf, Option<u64>, String)>,
        cx: &mut Context<Self>,
    ) {
        let paths: Vec<_> = self
            .fit_paths
            .iter()
            .filter(|p| p.spec.enabled)
            .map(|p| p.spec.file.clone())
            .collect();
        for (file, group_id, label) in sources {
            if self
                .joint
                .config
                .datasets
                .iter()
                .any(|d| d.file == file && d.group_id == group_id)
            {
                continue;
            }
            let id = self
                .joint
                .config
                .datasets
                .iter()
                .map(|d| d.id)
                .max()
                .unwrap_or(0)
                + 1;
            self.joint.config.datasets.push(JointDataset {
                id,
                file,
                group_id,
                label,
                paths: paths.clone(),
                ..Default::default()
            });
        }
        self.joint.setup = true;
        self.fit_model_changed(cx);
        cx.notify();
    }
    pub(crate) fn joint_blocker(&self) -> Option<&'static str> {
        if self
            .joint
            .config
            .datasets
            .iter()
            .any(|d| self.joint_dataset_params(d).is_none())
        {
            return Some("A fit source group is missing. Restore it or remove that dataset.");
        }
        if self.fit_running {
            return Some("Fitting spectra…");
        }
        if self.feff_running {
            return Some("Wait for the path calculation to finish.");
        }
        if self.joint.config.datasets.len() < 2 {
            return Some("Add at least two spectra in Spectra & paths.");
        }
        if self
            .joint
            .config
            .datasets
            .iter()
            .any(|d| !d.ranges.as_ref().unwrap_or(&self.fit_ranges).valid())
        {
            return Some("Set valid k and R fit ranges for the selected spectra.");
        }
        if self.joint.config.datasets.iter().any(|d| {
            d.ranges
                .as_ref()
                .unwrap_or(&self.fit_ranges)
                .validate_background(
                    self.joint_dataset_params(d)
                        .and_then(|p| p.rbkg)
                        .unwrap_or(1.0),
                )
                .is_err()
        }) {
            return Some("Each spectrum's fit R min must be at least its background Rbkg.");
        }
        let paths: Vec<_> = self.fit_paths.iter().map(|p| p.spec.clone()).collect();
        let vars: Vec<_> = self.fit_vars.iter().map(|v| v.spec.clone()).collect();
        if joint_fitting::prepare(&self.joint.config, &paths, &vars).is_err() {
            return Some("Review the assignments and parameter scopes in Spectra & paths.");
        }
        None
    }
    pub(super) fn joint_mode_bar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let enabled = self.joint.config.enabled;
        div()
            .flex()
            .items_center()
            .flex_wrap()
            .gap_2()
            .px_4()
            .pb_2()
            .text_size(px(11.))
            .child("Fit mode")
            .child(
                chip(&t, "fit-single", "Single spectrum", !enabled).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.joint.config.enabled = false;
                        this.fit_model_changed(cx);
                        cx.notify();
                    },
                )),
            )
            .child(
                chip(&t, "fit-joint", "Fit multiple spectra", enabled).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.joint.config.enabled = true;
                        this.joint.setup = true;
                        if this.joint.config.datasets.is_empty() {
                            for p in this.fit_paths.iter().filter(|p| p.spec.enabled) {
                                for expr in [&p.spec.deltar, &p.spec.sigma2] {
                                    this.joint
                                        .config
                                        .local
                                        .extend(crate::fitting::expr_identifiers(expr));
                                }
                            }
                            this.add_joint_current(cx);
                        }
                        this.fit_model_changed(cx);
                        cx.notify();
                    },
                )),
            )
            .when(enabled, |d| {
                d.child(
                    chip(&t, "joint-setup", "Spectra & paths", self.joint.setup).on_click(
                        cx.listener(|this, _, _, cx| {
                            this.joint.setup = true;
                            cx.notify();
                        }),
                    ),
                )
                .child(
                    chip(&t, "joint-view-fit", "Plots", !self.joint.setup).on_click(cx.listener(
                        |this, _, _, cx| {
                            this.joint.setup = false;
                            cx.notify();
                        },
                    )),
                )
            })
    }
    pub(super) fn joint_result_bar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let mut bar = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_1()
            .px_3()
            .py_1()
            .text_size(px(11.));
        if let Some(config) = &self.joint.result_config {
            bar = bar.child("Spectrum");
            for (i, d) in config.datasets.iter().enumerate() {
                bar = bar.child(
                    chip(
                        &t,
                        ("joint-result", i),
                        format!("{} · {}", d.id, d.label),
                        self.joint.result_index == i,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.joint.result_index = i;
                        this.rebuild_fit_plots(cx);
                        cx.notify();
                    })),
                );
            }
            if let Some(d) = self
                .fit_result
                .as_ref()
                .and_then(|r| r.datasets.get(self.joint.result_index))
            {
                bar = bar.child(format!(
                    "{} paths · spectrum R-factor {:.5}",
                    d.path_contributions.len(),
                    d.r_factor
                ));
            }
        }
        bar
    }
    pub(crate) fn run_joint_fit_now(&mut self, cx: &mut Context<Self>) {
        if let Some(reason) = self.joint_blocker() {
            self.status = reason.into();
            self.joint.setup = true;
            cx.notify();
            return;
        }
        let mut config = self.joint.config.clone();
        let inputs: Vec<_> = config
            .datasets
            .iter()
            .map(|d| self.joint_dataset_input(d))
            .collect();
        let paths: Vec<FitPathSpec> = self.fit_paths.iter().map(|p| p.spec.clone()).collect();
        let vars: Vec<FitVarSpec> = self.fit_vars.iter().map(|v| v.spec.clone()).collect();
        let ranges = self.fit_ranges.clone();
        for d in &mut config.datasets {
            d.ranges = Some(
                d.ranges
                    .as_ref()
                    .unwrap_or(&ranges)
                    .resolved(self.joint_dataset_params(d).and_then(|p| p.fft_kweight)),
            );
        }
        let saved = (config.clone(), paths.clone(), vars.clone(), ranges.clone());
        let provenance = FitProvenance {
            group_id: None,
            label: format!("Multiple spectra · {} spectra", config.datasets.len()).into(),
            path: PathBuf::new(),
            params_fingerprint: 0,
            model_fingerprint: self.fit_model_fingerprint(),
        };
        self.fit_gen += 1;
        let generation = self.fit_gen;
        self.fit_running = true;
        self.fit_error = None;
        self.status = "Preparing spectra…".into();
        cx.notify();
        let started = std::time::Instant::now();
        let job = cx.background_executor().spawn(async move {
            let mut data = vec![];
            for input in inputs {
                let input = input?;
                let sp = input
                    .process()
                    .map_err(|e| format!("{}: {e}", input.label()))?;
                data.push((*sp).clone());
            }
            joint_fitting::run_processed(&config, &data, &paths, &vars, &ranges)
        });
        cx.spawn(async move |this, cx| {
            let result = job.await;
            this.update(cx, |app, cx| {
                if app.fit_gen != generation {
                    return;
                }
                app.fit_running = false;
                app.last_fit_duration = Some(started.elapsed());
                match result {
                    Ok((result, expanded)) => {
                        let (config, paths, vars, ranges) = saved;
                        let id = app.fit_history.last().map(|h| h.id + 1).unwrap_or(1);
                        let mut entry = FitHistoryEntry::from_result(
                            id,
                            provenance.label.to_string(),
                            paths,
                            vars,
                            ranges,
                            &result,
                        );
                        entry.path_details = crate::fit_details::snapshot(&expanded, &result);
                        entry.joint = Some(config.clone());
                        app.joint.result_config = Some(config);
                        app.joint.result_index = 0;
                        app.joint.setup = false;
                        app.status = format!(
                            "Fit {} · {} spectra · R-factor {:.5}",
                            if result.solver_report.as_ref().is_some_and(|r| r.converged) {
                                "converged"
                            } else {
                                "stopped"
                            },
                            result.datasets.len(),
                            result.r_factor
                        )
                        .into();
                        let result = Arc::new(result);
                        app.fit_result = Some(result.clone());
                        app.fit_provenance = Some(provenance);
                        app.fit_history.push(entry);
                        app.fit_history_results.insert(id, result);
                        app.fit_history_selected = Some(id);
                        app.set_fit_step(FitStep::Results, cx);
                        app.stage_view.fit_result_tab = 0;
                        app.stage_view.fit_show_batch = false;
                        app.rebuild_fit_plots(cx);
                    }
                    Err(e) => {
                        app.fit_error = Some(e.clone());
                        app.status = format!("Fit failed: {e}").into();
                        app.record_job_error("joint fit", e);
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}
