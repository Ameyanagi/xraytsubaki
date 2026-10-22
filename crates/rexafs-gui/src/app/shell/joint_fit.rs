//! Joint fit setup: every spectrum owns an explicit list of path identities.
use super::{chip, fit_workspace::FitStep, segment, segmented};
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
/// Resolve identity first. A reused file path must not repair historical inputs.
fn joint_source_index(
    dataset: &JointDataset,
    catalog: &crate::catalog::Catalog,
    derived: &[crate::params::DerivedSpectrum],
    registry: &crate::group_identity::GroupRegistry,
) -> Result<Option<usize>, ()> {
    if let Some(id) = &dataset.source_id {
        if registry.is_excluded(id) {
            return Err(());
        }
        if let Some(ix) = registry.index(id) {
            return Ok(Some(ix));
        }
        return registry
            .source(&dataset.file)
            .filter(|s| &s.id == id)
            .map(|_| None)
            .ok_or(());
    }
    if let Some(id) = dataset.group_id {
        return derived
            .iter()
            .position(|d| d.id == id)
            .map(|i| Some(crate::app::DERIVED_BASE + i))
            .ok_or(());
    }
    if let Some(ix) = catalog.find_by_path(&dataset.file) {
        return (!registry.index_excluded(ix)).then_some(Some(ix)).ok_or(());
    }
    if registry
        .source(&dataset.file)
        .is_some_and(|s| registry.is_excluded(&s.id))
    {
        return Err(());
    }
    Ok(None)
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
        let ix =
            joint_source_index(dataset, &self.catalog, &self.derived, &self.group_registry).ok()?;
        Some(
            ix.map(|ix| self.effective_params(ix))
                .unwrap_or(&self.params)
                .clone(),
        )
    }

    pub(crate) fn bind_joint_sources(&mut self) {
        let identities: Vec<_> = self
            .joint
            .config
            .datasets
            .iter()
            .map(|d| {
                d.source_id.clone().or_else(|| {
                    if let Some(id) = d.group_id {
                        return Some(
                            self.derived
                                .iter()
                                .find(|g| g.id == id)
                                .and_then(|g| g.group_id.clone())
                                .unwrap_or_else(|| {
                                    crate::group_identity::GroupId::legacy_result(id)
                                }),
                        );
                    }
                    let ix = self.catalog.find_by_path(&d.file);
                    let path = ix
                        .map(|ix| self.catalog.path(ix))
                        .unwrap_or_else(|| d.file.clone());
                    Some(self.group_registry.register_source(
                        ix,
                        path,
                        crate::params::DetectionMode::Auto,
                        &self.project_source_origins,
                    ))
                })
            })
            .collect();
        for (dataset, id) in self.joint.config.datasets.iter_mut().zip(identities) {
            dataset.source_id = id;
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
        self.bind_joint_sources();
        self.joint.setup = true;
        self.fit_model_changed(cx);
        cx.notify();
    }
    pub(crate) fn joint_blocker(&self) -> Option<&'static str> {
        if self.rmc.control.is_some() {
            return Some("Stop and save the active RMC run before starting a joint fit.");
        }
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
            .child("Spectra")
            .child(
                segmented(&t)
                    .child(
                        segment(&t, "fit-single", "Single", !enabled, true)
                            .description("Fit the active spectrum on its own.")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.joint.config.enabled = false;
                                this.fit_model_changed(cx);
                                cx.notify();
                            })),
                    )
                    .child(
                        segment(&t, "fit-joint", "Multiple", enabled, false)
                            .description("Fit several spectra together with shared variables.")
                            .on_click(cx.listener(|this, _, _, cx| {
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
                            })),
                    ),
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
        self.bind_joint_sources();
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
        self.job_inputs[3] = config
            .datasets
            .iter()
            .filter_map(|d| d.source_id.clone())
            .collect();
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
                        // Same per-dataset rule as `joint_fitting::run`.
                        let transforms = config
                            .datasets
                            .iter()
                            .map(|d| d.ranges.as_ref().unwrap_or(&ranges).transform())
                            .collect();
                        let mut entry = FitHistoryEntry::from_result(
                            id,
                            provenance.label.to_string(),
                            paths,
                            vars,
                            ranges,
                            &result,
                        );
                        entry.transforms = transforms;
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
                        app.pending_fit_restore = None;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joint_catalog_source_stays_missing_after_explicit_reimport() {
        let mut catalog = crate::catalog::Catalog::default();
        catalog.extend(vec![crate::catalog::FileMeta {
            dir: "/data".into(),
            name: "a.dat".into(),
            size: 0,
        }]);
        let registry = crate::group_identity::GroupRegistry::default();
        let path = catalog.path(0);
        let id = registry.register_source(
            Some(0),
            path.clone(),
            Default::default(),
            &Default::default(),
        );
        let mut dataset = JointDataset {
            file: path.clone(),
            source_id: Some(id.clone()),
            ..Default::default()
        };
        assert_eq!(
            joint_source_index(&dataset, &catalog, &[], &registry),
            Ok(Some(0))
        );
        registry.set_excluded(&std::collections::BTreeSet::from([id.clone()]));
        assert!(joint_source_index(&dataset, &catalog, &[], &registry).is_err());
        dataset.source_id = None; // legacy projects also flag excluded primary sources
        assert!(joint_source_index(&dataset, &catalog, &[], &registry).is_err());
        dataset.source_id = Some(id.clone());
        let fresh = registry.reimport_source(&path, Some(0)).unwrap();
        assert_ne!(fresh, id);
        let saved = serde_json::to_value(&dataset).unwrap();
        let dataset: JointDataset = serde_json::from_value(saved).unwrap();
        assert!(joint_source_index(&dataset, &catalog, &[], &registry).is_err());
        registry.restore_source(
            crate::group_identity::SourceGroup {
                id,
                path,
                channel: Default::default(),
            },
            0,
        );
        registry.set_excluded(&Default::default());
        assert_eq!(
            joint_source_index(&dataset, &catalog, &[], &registry),
            Ok(Some(0))
        );
        let mut legacy = serde_json::to_value(&dataset).unwrap();
        legacy.as_object_mut().unwrap().remove("source_id");
        let legacy: JointDataset = serde_json::from_value(legacy).unwrap();
        assert!(legacy.source_id.is_none());
    }
}
