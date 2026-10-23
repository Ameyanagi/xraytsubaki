//! Publish actions for manuscripts: the fit parameter table (LaTeX, CSV and
//! Word) and the supporting-information zip.
use super::editor::{enabled_when, with_tip};
use crate::accessibility::Control;
use crate::app::{StatusKind, StudioApp, shell::button};
use gpui::Context;

impl StudioApp {
    /// The most recent recorded fit with archived path values, if any.
    fn table_fit(&self) -> Option<&crate::fitting::FitHistoryEntry> {
        self.fit_history
            .iter()
            .rev()
            .find(|fit| !fit.path_details.is_empty())
    }

    /// Write `<name>.tex`, `<name>.csv` and `<name>.docx` for the latest fit.
    pub(crate) fn export_fit_table(&mut self, cx: &mut Context<Self>) {
        let Some(fit) = self.table_fit().cloned() else {
            return;
        };
        let home = crate::settings::home_dir().unwrap_or_else(std::env::temp_dir);
        let name = format!("rexafs-fit-{}-parameters", fit.id);
        let rx = cx.prompt_for_new_path(&home, Some(&name));
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(path))) = rx.await else {
                return;
            };
            let result = cx
                .background_executor()
                .spawn(async move { crate::publication::fit_table::write_all(&fit, &path) })
                .await;
            this.update(cx, |app, cx| {
                match result {
                    Ok(files) => {
                        let shown = files.first().map(|p| p.with_extension("{tex,csv,docx}"));
                        app.set_status(
                            StatusKind::Success,
                            format!("Saved fit table {}", shown.unwrap_or_default().display()),
                        );
                    }
                    Err(e) => app.publish.error = Some(e),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Write the supporting-information zip (report, figures, CSV data,
    /// project, fit tables and SHA-256 manifest).
    pub(crate) fn export_supporting_information(&mut self, cx: &mut Context<Self>) {
        if self.publish.running || !self.require_publication_source(cx) {
            return;
        }
        let snapshot = self.analysis_snapshot();
        let project_generation = self.project_generation;
        let home = crate::settings::home_dir().unwrap_or_else(std::env::temp_dir);
        let rx = cx.prompt_for_new_path(&home, Some("rexafs-supporting-information.zip"));
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(path))) = rx.await else {
                return;
            };
            this.update(cx, |app, cx| {
                app.publish.running = true;
                app.publish.error = None;
                cx.notify();
            })
            .ok();
            let result = cx
                .background_executor()
                .spawn(
                    async move { crate::publication::supporting::export_bundle(snapshot, &path) },
                )
                .await;
            this.update(cx, |app, cx| {
                if app.project_generation != project_generation {
                    return;
                }
                app.publish.running = false;
                match result {
                    Ok(path) => app.set_status(
                        StatusKind::Success,
                        format!("Saved supporting information {}", path.display()),
                    ),
                    Err(e) => {
                        app.publish.error = Some(e.clone());
                        app.record_job_error("Export supporting information", e);
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// "Fit table…" and "Supporting information…" header buttons.
    pub(super) fn manuscript_export_buttons(&self, cx: &mut Context<Self>) -> [Control; 2] {
        let t = self.theme;
        let table = self.table_fit().is_some();
        let ready = self.publication_ready() && !self.publish.running;
        [
            enabled_when(
                with_tip(
                    button(&t, "export-fit-table", "Export fit table…", false),
                    &t,
                    "Save the per-path parameters (N, S₀², ΔE₀, R, σ²) of the latest fit as LaTeX (booktabs), CSV and Word (.docx). Uncertainties are rounded to one significant figure (two if the leading digit is 1) and values to the same decimal place; fixed parameters are marked.",
                ),
                table,
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                if table {
                    this.export_fit_table(cx);
                }
            })),
            enabled_when(
                with_tip(
                    button(
                        &t,
                        "export-supporting-information",
                        "Export supporting information…",
                        false,
                    ),
                    &t,
                    "Save one .zip for journal supporting information: the report and figures, spectra and fit curves as CSV, a self-contained project file (raw data embedded), fit parameter tables and a SHA-256 checksum manifest.",
                ),
                ready,
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                if ready {
                    this.export_supporting_information(cx);
                }
            })),
        ]
    }
}
