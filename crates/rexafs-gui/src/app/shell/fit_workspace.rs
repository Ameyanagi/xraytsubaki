//! Fitting is a workspace, not a stack of inspector sections. Navigation is
//! freely reversible; only actions with missing prerequisites are disabled.
use super::{
    button, chip,
    controls::{Tooltip, disclosure, icon_button},
    section_label,
};
use crate::app::StudioApp;
use crate::icons::Icon;
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
            Self::Model => "Model",
            Self::Results => "Results",
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
    /// A shared range must be safe for every dataset that inherits it. An
    /// individual range uses that dataset's own processing settings.
    pub(crate) fn fit_background_floor(&self, dataset_id: Option<usize>) -> f64 {
        if let Some(id) = dataset_id
            && let Some(dataset) = self.joint.config.datasets.iter().find(|d| d.id == id)
        {
            return self
                .joint_dataset_params(dataset)
                .and_then(|p| p.rbkg)
                .unwrap_or(1.0);
        }
        if self.joint.config.enabled {
            return self
                .joint
                .config
                .datasets
                .iter()
                .filter(|d| d.ranges.is_none())
                .map(|d| {
                    self.joint_dataset_params(d)
                        .and_then(|p| p.rbkg)
                        .unwrap_or(1.0)
                })
                .fold(0.0, f64::max);
        }
        self.ui_params().rbkg.unwrap_or(1.0)
    }

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
        if self.rmc.control.is_some() {
            return Some("Stop and save the active RMC run before starting a path fit.");
        }
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
            return Some("Prepare χ(k) in Process › Background before fitting.");
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
        let Some(rbkg) = crate::fitting::spectrum_rbkg(s) else {
            return Some("Prepare the spectrum with AUTOBK before fitting.");
        };
        if r.validate_background(rbkg).is_err()
            || r.validate_background(self.fit_background_floor(None))
                .is_err()
        {
            return Some("Fit R min must be at least the spectrum's background Rbkg.");
        }
        None
    }

    pub(crate) fn batch_blocker(&self) -> Option<&'static str> {
        if self.rmc.control.is_some() {
            return Some("Stop and save the active RMC run before starting a batch fit.");
        }
        if self.joint.config.enabled {
            return Some(
                "Batch fits spectra independently. Select Single under Spectra in Model & fit to run a batch.",
            );
        }
        if self
            .active_scan
            .and_then(|i| self.catalog.scans.get(i))
            .is_none()
        {
            return Some("Select a scan in Series.");
        }
        if let Some(reason) = self.fit_blocker() {
            return Some(reason);
        }
        let scan = &self.catalog.scans[self.active_scan.unwrap()];
        if (scan.start..scan.start + scan.len)
            .filter(|&ix| !self.group_registry.index_excluded(ix))
            .any(|ix| {
                self.fit_ranges
                    .validate_background(self.effective_params(ix).rbkg.unwrap_or(1.0))
                    .is_err()
            })
        {
            return Some("Fit R min must be at least Rbkg for every spectrum in the batch.");
        }
        None
    }

    pub(crate) fn fit_workspace(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        if self.fit_mode == crate::rmc_fitting::FitMode::Rmc {
            return self.rmc_workspace(cx);
        }
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
            .flex_wrap()
            .flex_none()
            .gap_1()
            .px_3()
            .py_1()
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
            nav = nav.child(
                chip(&t, ("fit-workflow", i), dest.label(), step == dest)
                    .when(done, |d| {
                        d.child(super::controls::icon(&t, Icon::Check).size(px(12.)))
                    })
                    .tooltip(move |_, cx| {
                        cx.new(|_| Tooltip {
                            label: dest.hint().into(),
                            theme: t,
                        })
                        .into()
                    })
                    .on_click(cx.listener(move |this, _, _, cx| this.set_fit_step(dest, cx))),
            );
        }
        nav = nav.child(div().flex_1()).child(self.fit_mode_picker(cx));
        // A stale result offers to rerun the fit instead of sending the user back to the model.
        let rerun = step == FitStep::Results && self.fit_result.is_some() && self.fit_is_stale();
        let (_title, description, action, enabled) = match step {
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
                if rerun && self.fit_running {
                    "Fitting…"
                } else if rerun {
                    "Run fit again"
                } else {
                    "Edit model →"
                },
                !rerun || self.fit_blocker().is_none(),
            ),
        };
        // The reason an action is unavailable sits under its button and in the tooltip.
        let blocker = match step {
            FitStep::Model => self.fit_blocker(),
            FitStep::Results if rerun => self.fit_blocker(),
            FitStep::Structure if !enabled => Some("Choose a structure"),
            FitStep::Calculate if !enabled && !self.feff_running => {
                Some("Choose a structure or FEFF input")
            }
            FitStep::Paths if selected == 0 => Some("Select at least one path"),
            _ => None,
        };
        let tip: &'static str = blocker.unwrap_or(description);
        let header = div()
            .flex_none()
            .px_3()
            .py_2()
            .flex()
            .items_start()
            .justify_end()
            .gap_2()
            .child(
                icon_button(
                    &t,
                    "fit-parameters-toggle",
                    Icon::Sliders,
                    "Show or hide fit controls",
                    self.context_panel_open,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.toggle_side_panel(super::assistant_shell::SidePanel::Inspector);
                    cx.notify();
                })),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_end()
                    .gap_1()
                    .child(
                        button(&t, "fit-workflow-action", action, enabled)
                            .tooltip(move |_, cx| {
                                cx.new(|_| Tooltip {
                                    label: tip.into(),
                                    theme: t,
                                })
                                .into()
                            })
                            .when(!enabled, |d| {
                                d.disabled(true).opacity(0.45).cursor_default()
                            })
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
                                    FitStep::Paths => this.set_fit_step(FitStep::Model, cx),
                                    FitStep::Results if rerun => this.run_fit_now(cx),
                                    FitStep::Results => this.set_fit_step(FitStep::Model, cx),
                                    FitStep::Model => this.run_fit_now(cx),
                                }
                            })),
                    )
                    .when_some(blocker, |d, reason| {
                        d.child(
                            div()
                                .max_w(px(360.))
                                .text_size(px(11.))
                                .text_color(t.warn)
                                .text_right()
                                .child(reason),
                        )
                    }),
            );
        let available = self.viewport_w
            - if self.data_panel_open { 260. } else { 0. }
            - if self.assistant_host == super::assistant_shell::AssistantHost::Docked {
                self.structure.settings.assistant_panel_width
            } else {
                0.
            };
        let stacked = available < 700.;
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
                let custom_open = self.ui.sections.contains("Custom FEFF input");
                let footer = div()
                    .px_2()
                    .py_1()
                    .border_t_1()
                    .border_color(t.border)
                    .child(
                        disclosure(
                            &t,
                            "custom-feff-input",
                            if library {
                                "Import paths"
                            } else {
                                "Custom FEFF input"
                            },
                            custom_open,
                            false,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            if !this.ui.sections.remove("Custom FEFF input") {
                                this.ui.sections.insert("Custom FEFF input");
                            }
                            cx.notify();
                        })),
                    )
                    .when(custom_open, |d| d.child(actions));
                let sidebar = div()
                    .w(px(if stacked {
                        available.max(240.) - 24.
                    } else {
                        340.
                    }))
                    .max_w_full()
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
                    .when(stacked, |d| d.flex_col())
                    .when(self.context_panel_open, |d| {
                        d.child(sidebar.when(stacked, |d| d.max_h(px(320.))))
                    })
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
                            div()
                                .flex()
                                .flex_col()
                                .children(indices.into_iter().map(|i| self.fit_path_cells(i, cx)))
                                .into_any_element()
                        }
                        _ => self.fit_params_section(cx).into_any_element(),
                    }
                };
                let panel = div()
                    .w(px(if stacked { available.max(240.) } else { 330. }))
                    .max_w_full()
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
                                self.batch_blocker().unwrap_or("Ready")
                            } else {
                                self.fit_blocker().unwrap_or("Ready")
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
                    .when(stacked, |d| d.flex_col())
                    .child(main)
                    .when(
                        self.context_panel_open
                            && !(step == FitStep::Model && self.joint.config.enabled),
                        |d| d.child(panel.when(stacked, |d| d.max_h(px(300.)))),
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
            .child(self.rmc_job_bar(cx))
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
                    .mb_1()
                    .px_2()
                    .py_1()
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
                            .max_w(px(230.))
                            .min_w_0()
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
