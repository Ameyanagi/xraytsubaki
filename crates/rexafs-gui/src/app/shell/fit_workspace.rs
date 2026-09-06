//! Fitting is a workspace, not a stack of inspector sections. Navigation is
//! freely reversible; only actions with missing prerequisites are disabled.
use super::{button, chip, section_label};
use crate::app::StudioApp;
use gpui::{ClickEvent, Context, IntoElement, ParentElement, Styled, div, prelude::*, px};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FitStep {
    Structure,
    Calculate,
    Paths,
    Model,
    Results,
}
impl FitStep {
    const ALL: [Self; 5] = [
        Self::Structure,
        Self::Calculate,
        Self::Paths,
        Self::Model,
        Self::Results,
    ];
    fn label(self) -> &'static str {
        match self {
            Self::Structure => "Structure",
            Self::Calculate => "Calculate",
            Self::Paths => "Paths",
            Self::Model => "Model & fit",
            Self::Results => "Results & batch",
        }
    }
    fn hint(self) -> &'static str {
        match self {
            Self::Structure => "Find a reference material",
            Self::Calculate => "Configure FEFF / ReFEFF",
            Self::Paths => "Inspect scattering geometry",
            Self::Model => "Parameters and fit ranges",
            Self::Results => "Review, compare, apply",
        }
    }
}

impl StudioApp {
    pub(crate) fn set_fit_step(&mut self, step: FitStep, cx: &mut Context<Self>) {
        self.stage_view.fit_step = step;
        self.structure.show = matches!(
            step,
            FitStep::Structure | FitStep::Calculate | FitStep::Paths
        );
        self.structure.pick = None;
        self.stage_view.fit_show_batch =
            step == FitStep::Results && self.stage_view.fit_result_tab == 2;
        self.refresh_structure(cx);
        self.rebuild_structure_plot(cx);
        cx.notify();
    }

    pub(crate) fn fit_blocker(&self) -> Option<&'static str> {
        if self.joint.config.enabled {
            return self.joint_blocker();
        }
        if self.load_running {
            return Some("Wait for the selected spectrum to finish processing.");
        }
        if self.stale_plots.is_some() {
            return Some("The selected spectrum could not be processed. Review the data errors.");
        }
        if self.feff_running {
            return Some("Wait for the path calculation to finish.");
        }
        if self.fit_running {
            return Some("Fitting the current spectrum…");
        }
        let Some(s) = &self.spectrum else {
            return Some("Load a spectrum to fit.");
        };
        if s.k().is_none() || s.chi().is_none() {
            return Some("Prepare χ(k) in Background before fitting.");
        }
        if !self.fit_paths.iter().any(|p| p.spec.enabled) {
            return Some("Select at least one scattering path in Paths.");
        }
        let r = &self.fit_ranges;
        if ![r.kmin, r.kmax, r.rmin, r.rmax]
            .iter()
            .all(|v| v.is_finite())
            || r.kmin < 0.
            || r.rmin < 0.
            || r.kmin >= r.kmax
            || r.rmin >= r.rmax
        {
            return Some("Set valid k and R ranges: minimum must be below maximum.");
        }
        None
    }

    pub(crate) fn batch_blocker(&self) -> Option<&'static str> {
        if self.joint.config.enabled {
            return Some(
                "Batch fits spectra independently. Select Single spectrum in Model & fit to run a batch.",
            );
        }
        if self
            .active_scan
            .and_then(|i| self.catalog.scans.get(i))
            .is_none()
        {
            return Some("Open Scans in the data panel and select a scan.");
        }
        self.fit_blocker()
    }

    pub(crate) fn fit_workspace(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let step = self.stage_view.fit_step;
        let selected = self.fit_paths.iter().filter(|p| p.spec.enabled).count();
        let mut nvar = self
            .fit_vars
            .iter()
            .filter(|v| v.spec.vary && v.spec.expr.is_none())
            .count();
        if self.joint.config.enabled {
            let paths: Vec<_> = self.fit_paths.iter().map(|p| p.spec.clone()).collect();
            let vars: Vec<_> = self.fit_vars.iter().map(|v| v.spec.clone()).collect();
            nvar = crate::joint_fitting::prepare(&self.joint.config, &paths, &vars)
                .map(|p| p.vars.varying_names().len())
                .unwrap_or(0);
        }
        let mut nav = div()
            .flex()
            .flex_none()
            .gap_2()
            .px_3()
            .py_2()
            .bg(t.surface)
            .border_b_1()
            .border_color(t.border);
        for (i, dest) in FitStep::ALL.into_iter().enumerate() {
            let done = match dest {
                FitStep::Structure => self.structure.summary.is_some(),
                FitStep::Calculate => !self.fit_paths.is_empty(),
                FitStep::Paths => selected > 0,
                FitStep::Model => self.fit_result.is_some() && !self.fit_is_stale(),
                FitStep::Results => self.fit_result.is_some(),
            };
            nav =
                nav.child(
                    div()
                        .id(("fit-workflow", i))
                        .flex_1()
                        .min_w_0()
                        .px_2()
                        .py_2()
                        .rounded_md()
                        .border_1()
                        .border_color(if step == dest { t.accent } else { t.border })
                        .when(step == dest, |d| d.bg(t.raised))
                        .cursor_pointer()
                        .hover(|d| d.bg(t.raised))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _w, cx| {
                            this.set_fit_step(dest, cx)
                        }))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    div()
                                        .size(px(20.))
                                        .flex_none()
                                        .rounded_full()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_size(px(10.))
                                        .bg(if done {
                                            t.success
                                        } else if step == dest {
                                            t.accent
                                        } else {
                                            t.border
                                        })
                                        .text_color(if done || step == dest {
                                            t.bg
                                        } else {
                                            t.text_muted
                                        })
                                        .child(if done {
                                            "✓".to_string()
                                        } else {
                                            (i + 1).to_string()
                                        }),
                                )
                                .child(
                                    div()
                                        .text_size(px(12.))
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .child(dest.label()),
                                ),
                        )
                        .child(
                            div()
                                .mt_1()
                                .text_size(px(10.5))
                                .text_color(t.text_muted)
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .child(dest.hint()),
                        ),
                );
        }
        let (title, description, action, enabled) = match step {
            FitStep::Structure => (
                "Start with a structure",
                "Browse curated standards, search a database, or import your own structure.",
                "Use structure →",
                self.structure.summary.is_some() && !self.structure.fetch_running,
            ),
            FitStep::Calculate => (
                "Calculate scattering paths",
                "Check the absorber, edge, and cluster before running the calculation.",
                if self.feff_running {
                    "Calculating…"
                } else {
                    "Calculate paths"
                },
                !self.feff_running
                    && (self.structure.summary.is_some() || self.feff_workspace.is_some()),
            ),
            FitStep::Paths => (
                "Choose paths for your model",
                "Click a row to inspect geometry. Check its box to include it in the fit.",
                "Set up model →",
                selected > 0 && !self.feff_running,
            ),
            FitStep::Model => (
                "Build and fit the model",
                "Define shared variables, inspect path expressions, and set the fit ranges.",
                if self.fit_running {
                    "Fitting…"
                } else {
                    "Run fit"
                },
                self.fit_blocker().is_none(),
            ),
            FitStep::Results => (
                "Review the fit",
                "Inspect uncertainties and correlations, restore a fit, or fit multiple spectra.",
                "Edit model →",
                true,
            ),
        };
        let header = div()
            .flex_none()
            .px_4()
            .py_3()
            .flex()
            .items_center()
            .gap_3()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .text_size(px(20.))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(title),
                    )
                    .child(
                        div()
                            .mt_1()
                            .text_size(px(12.))
                            .text_color(t.text_muted)
                            .child(description),
                    ),
            )
            .child(
                button(&t, "fit-workflow-action", action, enabled)
                    .h(px(32.))
                    .px_3()
                    .when(!enabled, |d| d.opacity(0.45).cursor_default())
                    .on_click(cx.listener(move |this, _: &ClickEvent, _w, cx| {
                        if !enabled {
                            return;
                        }
                        match step {
                            FitStep::Structure => this.set_fit_step(FitStep::Calculate, cx),
                            FitStep::Calculate => {
                                if this.structure.summary.is_some() {
                                    this.structure_generate_paths(cx)
                                } else {
                                    this.run_feff10_now(cx)
                                }
                            }
                            FitStep::Paths | FitStep::Results => {
                                this.set_fit_step(FitStep::Model, cx)
                            }
                            FitStep::Model => this.run_fit_now(cx),
                        }
                    })),
            );
        let content = match step {
            FitStep::Structure | FitStep::Calculate => {
                let library = step == FitStep::Structure;
                let depth_open = self.structure.depth.open;
                let panel = if depth_open {
                    self.structure_depth_panel(cx).into_any_element()
                } else {
                    self.structure_panel(cx).into_any_element()
                };
                let mut actions = div().flex().flex_wrap().gap_2().child(
                    button(&t, "workflow-import-paths", "Import path files…", false).on_click(
                        cx.listener(|this, _: &ClickEvent, _w, cx| this.add_fit_path_dialog(cx)),
                    ),
                );
                if !library {
                    actions = actions
                        .child(
                            button(&t, "workflow-input", "Choose feff.inp…", false).on_click(
                                cx.listener(|this, _: &ClickEvent, _w, cx| {
                                    this.choose_feff_inp(cx)
                                }),
                            ),
                        )
                        .child(
                            button(&t, "workflow-new-input", "New input…", false).on_click(
                                cx.listener(|this, _: &ClickEvent, _w, cx| this.new_feff_inp(cx)),
                            ),
                        );
                }
                let footer = div()
                    .px_3()
                    .py_3()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .border_t_1()
                    .border_color(t.border)
                    .child(section_label(
                        &t,
                        if library {
                            "Already have FEFF paths?"
                        } else {
                            "Custom FEFF input"
                        },
                    ))
                    .child(actions);
                let sidebar = div()
                    .w(px(410.))
                    .flex_none()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .rounded_lg()
                    .border_1()
                    .border_color(t.border)
                    .bg(t.surface)
                    .child(div().px_3().py_3().child(section_label(
                        &t,
                        if depth_open {
                            "Slice & depth"
                        } else if library {
                            "Structure library"
                        } else {
                            "Calculation setup"
                        },
                    )))
                    .child(
                        div()
                            .id(("fit-structure-scroll", usize::from(!library)))
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .child(panel),
                    )
                    .when(!depth_open, |d| d.child(footer))
                    .when(
                        !library
                            && (self.feff_running || self.status.starts_with("Path calculation")),
                        |d| {
                            d.child(
                                div()
                                    .px_3()
                                    .pb_3()
                                    .text_size(px(11.))
                                    .text_color(if self.status.contains("failed") {
                                        t.warn
                                    } else {
                                        t.accent
                                    })
                                    .child(self.status.clone()),
                            )
                        },
                    );
                div()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .flex()
                    .gap_2()
                    .px_3()
                    .pb_3()
                    .child(sidebar)
                    .child(self.structure_center(cx))
                    .into_any_element()
            }
            FitStep::Paths => self.structure_center(cx).into_any_element(),
            FitStep::Model | FitStep::Results => {
                let results = step == FitStep::Results;
                let tab = if results {
                    self.stage_view.fit_result_tab
                } else {
                    self.stage_view.fit_model_tab
                };
                let labels = if results {
                    ["Result", "History", "Batch"]
                } else {
                    ["Parameters", "Fit ranges", "Path expressions"]
                };
                let mut tabs = div()
                    .flex_none()
                    .flex()
                    .gap_1()
                    .px_2()
                    .py_2()
                    .border_b_1()
                    .border_color(t.border);
                for (i, label) in labels.into_iter().enumerate() {
                    tabs = tabs.child(chip(&t, ("fit-detail-tab", i), label, tab == i).on_click(
                        cx.listener(move |this, _: &ClickEvent, _w, cx| {
                            if results {
                                this.stage_view.fit_result_tab = i;
                                this.stage_view.fit_show_batch = i == 2;
                            } else {
                                this.stage_view.fit_model_tab = i;
                            }
                            cx.notify();
                        }),
                    ));
                }
                let body = if results {
                    match tab {
                        1 => self.fit_history_section(cx).into_any_element(),
                        2 => self.fit_batch_section(cx).into_any_element(),
                        _ => self.fit_result_section(cx).into_any_element(),
                    }
                } else {
                    match tab {
                        1 => self.fit_settings_section(cx).into_any_element(),
                        2 => {
                            let indices: Vec<_> = self
                                .fit_paths
                                .iter()
                                .enumerate()
                                .filter(|(_, p)| p.spec.enabled)
                                .map(|(i, _)| i)
                                .collect();
                            div().flex().flex_col().child(self.note("Path expressions reference the shared variables in Parameters. Numeric constants keep a path quantity fixed."))
                            .children(indices.into_iter().map(|i|self.fit_path_cells(i,cx))).into_any_element()
                        }
                        _ => self.fit_params_section(cx).into_any_element(),
                    }
                };
                let panel = div()
                    .w(px(390.))
                    .min_h_0()
                    .flex_none()
                    .flex()
                    .flex_col()
                    .bg(t.surface)
                    .border_l_1()
                    .border_color(t.border)
                    .child(tabs)
                    .child(
                        div()
                            .id(("fit-detail-scroll", tab + usize::from(results) * 3))
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .child(body),
                    )
                    .child(
                        div()
                            .flex_none()
                            .px_3()
                            .py_2()
                            .border_t_1()
                            .border_color(t.border)
                            .text_size(px(11.))
                            .text_color(if self.fit_blocker().is_some() {
                                t.warn
                            } else {
                                t.text_muted
                            })
                            .child(if results && tab == 2 {
                                self.batch_blocker()
                                    .unwrap_or("Ready to apply this model to the selected scan.")
                            } else {
                                self.fit_blocker()
                                    .unwrap_or("Ready to fit · results are saved in History.")
                            }),
                    );
                let main = if !results && self.joint.config.enabled && self.joint.setup {
                    self.joint_setup_panel(cx).into_any_element()
                } else if results && tab == 2 {
                    div().flex_1().min_w_0().min_h_0().flex().flex_col().px_3().pb_3()
                        .child(div().flex_none().py_2().text_size(px(12.)).text_color(t.text_muted).child("Batch results · select a row to inspect its spectrum. Parameter trends are available in Series."))
                        .child(self.batch_results_table(cx)).into_any_element()
                } else {
                    self.fit_stage_center(cx).into_any_element()
                };
                div()
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .flex()
                    .child(main)
                    .when(
                        self.context_panel_open
                            && !(step == FitStep::Model && self.joint.config.enabled),
                        |d| d.child(panel),
                    )
                    .into_any_element()
            }
        };
        let structure_label = if self.path_sources().len() > 1 {
            format!("{} structures in model", self.path_sources().len())
        } else {
            self.structure
                .cluster
                .as_ref()
                .map(|c| c.title.clone())
                .unwrap_or_else(|| "Choose a structure or import paths".into())
        };
        div()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .child(nav)
            .child(header)
            .when(step == FitStep::Model, |d| d.child(self.joint_mode_bar(cx)))
            .when(step == FitStep::Model && self.fit_error.is_some(), |d| {
                d.child(
                    div()
                        .mx_3()
                        .mb_2()
                        .p_2()
                        .rounded_md()
                        .border_1()
                        .border_color(t.warn)
                        .text_color(t.warn)
                        .text_size(px(12.))
                        .child(format!(
                            "Fit could not run: {}",
                            self.fit_error.as_deref().unwrap_or_default()
                        )),
                )
            })
            .child(
                div()
                    .flex_none()
                    .mx_3()
                    .mb_2()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(t.surface)
                    .flex()
                    .items_center()
                    .flex_wrap()
                    .gap_3()
                    .text_size(px(11.))
                    .child(div().text_color(t.accent).child(
                        if self.joint.config.enabled
                            && matches!(step, FitStep::Model | FitStep::Results)
                        {
                            format!("Data · {} spectra", self.joint.config.datasets.len())
                        } else {
                            format!("Data · {}", self.current_group_label())
                        },
                    ))
                    .child(
                        div()
                            .w(px(230.))
                            .flex_none()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .text_color(t.text_muted)
                            .child(structure_label),
                    )
                    .child(if self.joint.config.enabled {
                        format!(
                            "{} spectra · {} path assignments",
                            self.joint.config.datasets.len(),
                            self.joint
                                .config
                                .datasets
                                .iter()
                                .map(|d| d.paths.len())
                                .sum::<usize>()
                        )
                    } else {
                        format!("{selected} / {} paths", self.fit_paths.len())
                    })
                    .child(format!("{nvar} free variables"))
                    .when(self.fit_result.is_some(), |d| {
                        d.child(
                            div()
                                .text_color(if self.fit_is_stale() {
                                    t.warn
                                } else {
                                    t.success
                                })
                                .child(if self.fit_is_stale() {
                                    "Fit needs updating"
                                } else {
                                    "Fit available"
                                }),
                        )
                    }),
            )
            .child(content)
    }
}
