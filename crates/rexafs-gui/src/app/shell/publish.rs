mod editor;
use crate::publication::figures::{FigureData, FigureSettings, RenderedFigure};
use crate::widgets::{numeric_field::NumericField, text_input::TextInput};
use crate::{
    app::{DERIVED_BASE, StudioApp},
    publication::{Snapshot, SpectrumInput},
};
use gpui::{Context, Entity};
use std::sync::Arc;
#[derive(Default)]
pub(crate) struct PublishState {
    pub running: bool,
    pub destination: Option<std::path::PathBuf>,
    pub error: Option<String>,
    pub settings: FigureSettings,
    pub(super) source: Option<(usize, usize, usize, String)>,
    pub(super) figures: Vec<Arc<FigureData>>,
    pub(super) selected: usize,
    pub(super) numbers: Vec<Entity<NumericField>>,
    pub(super) labels: Vec<Entity<TextInput>>,
    pub(super) preview_generation: u64,
    pub(super) preview_running: bool,
    pub(super) preview: Option<Arc<RenderedFigure>>,
    pub(super) image: Option<Arc<gpui::Image>>,
}
impl PublishState {
    pub(crate) fn load_settings(&mut self, settings: FigureSettings) {
        let preview_generation = self.preview_generation + 1;
        *self = Self {
            settings,
            preview_generation,
            ..Default::default()
        };
    }
}
impl StudioApp {
    pub(crate) fn analysis_snapshot(&self) -> Snapshot {
        let mut paths = std::collections::BTreeSet::new();
        if !self.current_path.as_os_str().is_empty() {
            paths.insert(self.current_path.clone());
        }
        paths.extend(self.joint.config.datasets.iter().map(|d| d.file.clone()));
        paths.extend(
            self.selection
                .iter()
                .filter(|&&ix| ix < self.catalog.len() && ix < DERIVED_BASE)
                .map(|&ix| self.catalog.path(ix)),
        );
        let mut spectra: Vec<_> = paths
            .into_iter()
            .map(|path| {
                let params = self.joint_params(&path);
                SpectrumInput {
                    path,
                    params,
                    data: None,
                }
            })
            .collect();
        // Keep the active spectrum first, including a derived spectrum's actual arrays.
        spectra.sort_by_key(|s| s.path != self.current_path);
        if self.selected.is_some_and(|ix| ix >= DERIVED_BASE) {
            if let Some(sp) = &self.spectrum {
                spectra.insert(
                    0,
                    SpectrumInput {
                        path: self.spectrum_path.clone(),
                        params: self.ui_params().clone(),
                        data: Some(sp.clone()),
                    },
                );
            }
        }
        let mut results = self.fit_history_results.clone();
        if let Some(r) = &self.fit_result {
            if !results.values().any(|v| std::sync::Arc::ptr_eq(v, r)) {
                results.insert(
                    self.fit_history.iter().map(|f| f.id).max().unwrap_or(0) + 1,
                    r.clone(),
                );
            }
        }
        Snapshot {
            project: self.project_file(),
            current: self.current_path.clone(),
            spectra,
            results,
            analysis: serde_json::json!({"lcf":self.analysis.lcf,"ranked_lcf":self.analysis.ranked,"pca":self.analysis.pca,"pca_fit":self.analysis.pca_fit}),
            batch_csv: self
                .batch_fit
                .as_ref()
                .map(|b| crate::fitting::batch_csv(&b.rows, &b.varying_names, &b.frame_labels)),
            batch_stale: self.batch_fit_is_stale(),
            journal: self
                .journal
                .entries
                .iter()
                .map(|e| e.text.clone())
                .collect(),
            screen: serde_json::json!({"stage":self.stage.name(),"fit_step":format!("{:?}",self.stage_view.fit_step),"fit_view":format!("{:?}",self.stage_view.fit_view),"current":self.current_group_label().to_string(),"model_selection":self.joint.selected,"result_dataset_index":self.joint.result_index,"file_browser":self.data_panel_open,"inspector":self.context_panel_open,"plot_scope":format!("{:?}",self.stage_view.scope)}),
        }
    }
    pub(crate) fn export_publication(&mut self, cx: &mut Context<Self>) {
        if self.publish.running {
            return;
        }
        let snapshot = self.analysis_snapshot();
        let home = crate::settings::home_dir().unwrap_or_else(std::env::temp_dir);
        let rx = cx.prompt_for_new_path(std::path::Path::new(&home), Some("rexafs-publication"));
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(path))) = rx.await {
                this.update(cx, |app, cx| {
                    app.publish.running = true;
                    app.publish.error = None;
                    cx.notify();
                })
                .ok();
                let result = cx
                    .background_executor()
                    .spawn(async move { crate::publication::export(snapshot, &path) })
                    .await;
                this.update(cx, |app, cx| {
                    app.publish.running = false;
                    match result {
                        Ok(path) => {
                            app.status = format!("Exported {}", path.display()).into();
                            app.publish.destination = Some(path);
                        }
                        Err(e) => {
                            app.publish.error = Some(e.clone());
                            app.record_job_error("Publish export", e);
                        }
                    }
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }
}
