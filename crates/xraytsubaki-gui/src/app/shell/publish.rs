use super::button;
use crate::{
    app::{DERIVED_BASE, StudioApp},
    publication::{Snapshot, SpectrumInput},
};
use gpui::{ClickEvent, Context, IntoElement, ParentElement, Styled, div, prelude::*, px};
#[derive(Default)]
pub(crate) struct PublishState {
    pub running: bool,
    pub destination: Option<std::path::PathBuf>,
    pub error: Option<String>,
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
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        let rx =
            cx.prompt_for_new_path(std::path::Path::new(&home), Some("xraytsubaki-publication"));
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
    pub(crate) fn publish_panel(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let t = self.theme;
        let mut panel = div()
            .flex_1()
            .min_w_0()
            .p_6()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .text_size(px(22.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child("Publish"),
            );
        for (icon, title, detail) in [
            ("▧", "Figures", "PNG · spectra, fit overlays and residuals"),
            (
                "≡",
                "Analysis record",
                "Markdown · settings, path expressions, results and uncertainties",
            ),
            (
                "¶",
                "Methods & references",
                "Editable draft · Markdown and BibTeX",
            ),
            (
                "↗",
                "Reproducibility",
                "Project · JSON arrays · fit history · batch CSV",
            ),
        ] {
            panel = panel.child(
                div()
                    .flex()
                    .gap_3()
                    .p_3()
                    .rounded_md()
                    .bg(t.surface)
                    .border_1()
                    .border_color(t.border)
                    .child(div().text_size(px(24.)).text_color(t.accent).child(icon))
                    .child(
                        div().flex().flex_col().gap_1().child(title).child(
                            div()
                                .text_size(px(12.))
                                .text_color(t.text_muted)
                                .child(detail),
                        ),
                    ),
            );
        }
        panel = panel
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(t.text_muted)
                    .child("Current + marked spectra, assigned fit spectra, and recorded results."),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        button(
                            &t,
                            "publish-export",
                            if self.publish.running {
                                "Exporting…"
                            } else {
                                "Export folder…"
                            },
                            true,
                        )
                        .on_click(
                            cx.listener(|this, _: &ClickEvent, _, cx| this.export_publication(cx)),
                        ),
                    )
                    .child(
                        button(&t, "copy-analysis-markdown", "Copy Markdown", false).on_click(
                            cx.listener(|this, _: &ClickEvent, _, cx| {
                                cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                    this.analysis_snapshot().markdown(),
                                ));
                                this.status = "Analysis record copied".into();
                                cx.notify();
                            }),
                        ),
                    ),
            );
        if let Some(path) = &self.publish.destination {
            let path = path.clone();
            panel = panel.child(
                button(&t, "open-publication", "Open exported folder", false)
                    .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| cx.reveal_path(&path))),
            );
        }
        if let Some(error) = &self.publish.error {
            panel = panel.child(div().text_color(t.error).child(error.clone()));
        }
        panel.into_any_element()
    }
}
