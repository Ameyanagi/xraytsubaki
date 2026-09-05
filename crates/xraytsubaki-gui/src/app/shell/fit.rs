//! Fit stage (Artemis, in one window): data vs model in k and R with
//! residual strips and draggable fit ranges in the center; paths,
//! parameters, settings, result, history and batch in the inspector.

use gpui::{
    ClickEvent, Context, Entity, IntoElement, ParentElement, SharedString, Styled, div, prelude::*,
    px, relative,
};
use ruviz_gpui::RuvizPlot;

use super::handles::{PLOT_FIT_K, PLOT_FIT_Q, PLOT_FIT_R};
use super::{FitView, MONO, button, chip, section_label, segment, segmented};
use crate::app::StudioApp;
use crate::fitting::{FitSpaceSpec, high_correlations};
use xraytsubaki::xafs::fitting::template::ParameterTemplate;

/// Correlation above which two parameters are flagged (Artemis warns at
/// 0.95; 0.9 catches the usual amp/σ² pair earlier).
const CORRELATION_WARN: f64 = 0.9;

impl StudioApp {
    pub(crate) fn fit_stage_center(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        if self.stage_view.fit_step == super::fit_workspace::FitStep::Model {
            return self.fit_preview_panel(cx);
        }
        let t = self.theme;
        if self.fit_plots.is_none() && (self.explore_plots_dirty || self.quadrants.is_empty()) {
            self.rebuild_explore_plots(cx);
            self.explore_plots_dirty = false;
        }
        let mut column = div()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .flex()
            .flex_col()
            .child(self.fit_plot_bar(cx))
            .when(self.joint.result_config.is_some(), |d| {
                d.child(self.joint_result_bar(cx))
            });
        if let Some(provenance) = self.fit_provenance.clone() {
            let stale = self.fit_is_stale();
            if stale {
                column = column.child(
                    div()
                        .mx_3()
                        .mt_2()
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .border_1()
                        .border_color(t.warn)
                        .text_size(px(11.))
                        .text_color(t.warn)
                        .child(format!(
                            "showing the fit of {} — the group, its parameters or the model changed since; press Fit to refresh",
                            provenance.label
                        )),
                );
            }
        }
        let Some(plots) = self.fit_plots.as_ref().map(|p| FitPlotHandles {
            k: p.k.clone(),
            k_residual: p.k_residual.clone(),
            r: p.r.clone(),
            r_residual: p.r_residual.clone(),
            q: p.q.clone(),
        }) else {
            let mut empty = column.child(
                div().flex_none().px_3().py_2().text_size(px(11.)).text_color(t.text_muted)
                    .child(if self.fit_running {"Fitting… Data shown below; the fitted model will appear when ready."} else {"Processed data · run the fit to see the model, residuals, and path contributions."}),
            );
            let slots: &[usize] = match self.stage_view.fit_view {
                FitView::Both => &[2, 3],
                FitView::K => &[2],
                FitView::R => &[3],
                FitView::Q => &[4],
            };
            let mut area = div()
                .flex_1()
                .min_h_0()
                .min_w_0()
                .flex()
                .flex_col()
                .gap_2()
                .px_3()
                .pb_3();
            for &slot in slots {
                if let Some((title, plot)) = self.quadrants.get(slot) {
                    area = area.child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .rounded_lg()
                            .bg(t.raised)
                            .border_1()
                            .border_color(t.border)
                            .child(
                                div()
                                    .flex_none()
                                    .px_3()
                                    .pt_2()
                                    .text_size(px(11.))
                                    .child(title.clone()),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_h_0()
                                    .relative()
                                    .child(plot.clone())
                                    .child(self.measure_card(slot, cx)),
                            ),
                    );
                }
            }
            empty = empty.child(area);
            if self.stage_view.fit_show_batch {
                empty = empty.child(self.batch_results_table(cx));
            }
            return empty;
        };
        let view = self.stage_view.fit_view;
        let kw = self
            .fit_result
            .as_ref()
            .map(|r| {
                if self.joint.result_config.is_some() {
                    r.datasets
                        .get(self.joint.result_index)
                        .map(|d| d.kweight)
                        .unwrap_or(r.kweight)
                } else {
                    r.kweight
                }
            })
            .unwrap_or(2.0);
        let k_title: SharedString =
            format!("{} · data vs fit", crate::plotting::chik_label(kw)).into();
        let r_title: SharedString = format!(
            "{}{}{} · data vs fit{}",
            crate::plotting::chir_label(kw),
            if self.stage_view.fit_show_re {
                " + Re"
            } else {
                ""
            },
            if self.stage_view.fit_show_im {
                " + Im"
            } else {
                ""
            },
            if self.stage_view.fit_show_paths {
                " · path contributions"
            } else {
                ""
            }
        )
        .into();
        let k_col = |this: &mut Self, cx: &mut Context<Self>| {
            this.fit_column(
                PLOT_FIT_K,
                k_title.clone(),
                plots.k.clone(),
                Some(plots.k_residual.clone()),
                cx,
            )
        };
        let r_col = |this: &mut Self, cx: &mut Context<Self>| {
            this.fit_column(
                PLOT_FIT_R,
                r_title.clone(),
                plots.r.clone(),
                Some(plots.r_residual.clone()),
                cx,
            )
        };
        let area = div()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .flex()
            .gap_2()
            .px_3()
            .pt_2()
            .pb_3();
        let area = match view {
            FitView::Both => area.child(k_col(self, cx)).child(r_col(self, cx)),
            FitView::K => area.child(k_col(self, cx)),
            FitView::R => area.child(r_col(self, cx)),
            FitView::Q => area.child(self.fit_column(
                usize::MAX,
                "Re χ(q) · back-transform of the fit R-window".into(),
                plots.q.clone(),
                None,
                cx,
            )),
        };
        column = column.child(area);
        if self.stage_view.fit_show_batch {
            column = column.child(self.batch_results_table(cx));
        }
        column
    }

    /// Main plot card with an optional residual strip beneath.
    pub(super) fn fit_column(
        &mut self,
        plot: usize,
        title: SharedString,
        main: Entity<RuvizPlot>,
        residual: Option<Entity<RuvizPlot>>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let t = self.theme;
        let overlay = (plot != usize::MAX)
            .then(|| self.handle_overlay(plot, cx))
            .flatten();
        let main_key = if plot == usize::MAX { PLOT_FIT_Q } else { plot };
        let residual_key = main_key + 2;
        let card = div()
            .id(("fit-card", plot))
            .flex_1()
            .min_h_0()
            .min_w_0()
            .relative()
            .flex()
            .flex_col()
            .rounded_lg()
            .bg(t.raised)
            .border_1()
            .border_color(t.border)
            .when(plot != usize::MAX, |d| {
                d.on_mouse_move(cx.listener(move |this, ev: &gpui::MouseMoveEvent, _w, cx| {
                    this.plot_pointer_move(plot, ev.position, cx);
                }))
            })
            .child(
                div()
                    .flex_none()
                    .px_3()
                    .pt_2()
                    .text_size(px(11.5))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .child(title),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .p_1()
                    .relative()
                    .child(main)
                    .child(self.measure_card(main_key, cx))
                    .children(
                        (plot != usize::MAX)
                            .then(|| self.handle_layer(plot, cx))
                            .flatten(),
                    ),
            )
            .children(overlay);
        let mut column = div()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_2()
            .child(card);
        if let Some(residual) = residual {
            column = column.child(
                div()
                    .flex_none()
                    .h(px(130.))
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .rounded_lg()
                    .bg(t.raised)
                    .border_1()
                    .border_color(t.border)
                    .child(
                        div()
                            .flex_none()
                            .px_3()
                            .pt_1()
                            .text_size(px(10.5))
                            .text_color(t.text_muted)
                            .child("residual"),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .min_w_0()
                            .p_1()
                            .relative()
                            .child(residual)
                            .child(self.measure_card(residual_key, cx)),
                    ),
            );
        }
        column
    }

    fn fit_plot_bar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let v = self.stage_view;
        let views = [
            (FitView::Both, "k + R"),
            (FitView::K, "k"),
            (FitView::R, "R"),
            (FitView::Q, "q"),
        ];
        let mut seg = segmented(&t);
        for (i, (fv, label)) in views.into_iter().enumerate() {
            seg = seg.child(
                segment(
                    &t,
                    SharedString::from(format!("fv-{label}")),
                    label,
                    v.fit_view == fv,
                    i == 0,
                )
                .on_click(cx.listener(move |this, _: &ClickEvent, _w, cx| {
                    this.stage_view.fit_view = fv;
                    this.rebuild_fit_plots(cx);
                    cx.notify();
                })),
            );
        }
        div()
            .h(px(36.))
            .w_full()
            .min_w_0()
            .flex_none()
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .bg(t.surface)
            .border_b_1()
            .border_color(t.border)
            .overflow_hidden()
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(t.text_muted)
                    .child("Space"),
            )
            .child(seg)
            .child(div().w(px(1.)).h(px(18.)).bg(t.border))
            .child(
                chip(&t, "fit-paths", "Contributions", v.fit_show_paths).on_click(cx.listener(
                    |this, _: &ClickEvent, _w, cx| {
                        this.stage_view.fit_show_paths = !this.stage_view.fit_show_paths;
                        this.rebuild_fit_plots(cx);
                        cx.notify();
                    },
                )),
            )
            .child(
                chip(&t, "fit-re", "Re χ(R)", v.fit_show_re).on_click(cx.listener(
                    |this, _: &ClickEvent, _w, cx| {
                        this.stage_view.fit_show_re = !this.stage_view.fit_show_re;
                        this.rebuild_fit_plots(cx);
                        this.explore_plots_dirty = true;
                        cx.notify();
                    },
                )),
            )
            .child(
                chip(&t, "fit-im", "Im χ(R)", v.fit_show_im).on_click(cx.listener(
                    |this, _: &ClickEvent, _w, cx| {
                        this.stage_view.fit_show_im = !this.stage_view.fit_show_im;
                        this.rebuild_fit_plots(cx);
                        this.explore_plots_dirty = true;
                        cx.notify();
                    },
                )),
            )
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(t.text_muted)
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .child(if self.fit_plots.is_some() {
                        "drag edges to adjust ranges"
                    } else {
                        "data preview"
                    }),
            )
            .child(div().flex_1())
    }

    // ---- inspector ----------------------------------------------------------

    fn sub_button(
        &self,
        id: &'static str,
        label: &'static str,
        cx: &mut Context<Self>,
        f: fn(&mut Self, &mut Context<Self>),
    ) -> impl IntoElement + use<> {
        let t = self.theme;
        div()
            .id(id)
            .px_1p5()
            .h(px(22.))
            .flex()
            .items_center()
            .rounded_md()
            .text_size(px(11.))
            .text_color(t.accent)
            .cursor_pointer()
            .hover(|d| d.bg(t.raised))
            .on_click(cx.listener(move |this, _: &ClickEvent, _w, cx| f(this, cx)))
            .child(label)
    }

    /// One selected path's parameter-expression cells.
    pub(super) fn fit_path_cells(
        &self,
        i: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let t = self.theme;
        let row = &self.fit_paths[i];
        let label: SharedString = self
            .fit_path_infos
            .get(i)
            .map(|p| format!("{} {} · {:.3} Å", i + 1, p.label, p.reff))
            .unwrap_or_else(|| format!("{} {}", i + 1, row.spec.label))
            .into();
        let more = row.more;
        let mut grid = div().mx_2().mb_1().flex().flex_col().gap_0p5();
        grid = grid.child(
            div()
                .text_size(px(11.))
                .text_color(t.text_muted)
                .child(label),
        );
        let mut cells = div().flex().flex_wrap().gap_1();
        for (param, field) in &row.fields {
            if !param.is_primary() && !more {
                continue;
            }
            cells = cells.child(
                div()
                    .w(px(124.))
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .w(px(28.))
                            .flex_none()
                            .text_size(px(11.))
                            .text_color(t.text_muted)
                            .child(param.label()),
                    )
                    .child(div().flex_1().min_w_0().child(field.clone())),
            );
        }
        cells = cells.child(
            div()
                .id(("fit-path-more", i))
                .h(px(24.))
                .px_1()
                .flex()
                .items_center()
                .text_size(px(10.5))
                .text_color(t.text_muted)
                .cursor_pointer()
                .hover(|d| d.text_color(t.text))
                .on_click(cx.listener(move |this, _: &ClickEvent, _w, cx| {
                    if let Some(p) = this.fit_paths.get_mut(i) {
                        p.more = !p.more;
                        cx.notify();
                    }
                }))
                .child(if more {
                    "▾ fewer"
                } else {
                    "▸ more (Ei, C₃, C₄)"
                }),
        );
        grid.child(cells)
    }

    pub(super) fn fit_params_section(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let result = self.fit_result.clone();
        let fresh = !self.fit_is_stale();
        let mut rows: Vec<gpui::AnyElement> = Vec::new();
        rows.push(self.fit_template_chooser(cx).into_any_element());
        if self.fit_vars.is_empty() {
            rows.push(
                self.note("No variables yet: select paths and apply a template, or type a name into a path cell (an expression such as dr_1*1.41 defines one from another).")
                    .into_any_element(),
            );
        }
        for var in &self.fit_vars {
            let name = var.spec.name.clone();
            let is_expr = var.spec.expr.is_some();
            let vary = var.spec.vary;
            let (kind, color) = if is_expr {
                ("expr", t.success)
            } else if vary {
                ("vary", t.accent)
            } else {
                ("fixed", t.text_muted)
            };
            let fitted: SharedString = result
                .as_ref()
                .filter(|_| fresh && vary && !is_expr)
                .and_then(|r| r.variables.get(&name))
                // History restores the original starting values. An uncertainty
                // belongs beside the fitted value, not a different starting value.
                .filter(|v| (v.value - var.spec.value).abs() <= 1e-10 * v.value.abs().max(1.0))
                .and_then(|v| v.stderr)
                .map(|e| format!("± {e:.4}"))
                .unwrap_or_default()
                .into();
            let var_name = name.clone();
            rows.push(
                div()
                    .px_3()
                    .py_0p5()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1p5()
                            .child(
                                div()
                                    .w(px(56.))
                                    .flex_none()
                                    .font_family(MONO)
                                    .text_size(px(11.5))
                                    .text_color(t.accent)
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .child(SharedString::from(name.clone())),
                            )
                            .child(
                                div()
                                    .id(SharedString::from(format!("pill-{name}")))
                                    .flex_none()
                                    .w(px(40.))
                                    .h(px(18.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded_full()
                                    .border_1()
                                    .border_color(color)
                                    .font_family(MONO)
                                    .text_size(px(10.))
                                    .text_color(color)
                                    .cursor_pointer()
                                    .hover(|d| d.bg(t.raised))
                                    .on_click(cx.listener(move |this, _: &ClickEvent, _w, cx| {
                                        this.cycle_var_kind(&var_name, cx);
                                    }))
                                    .child(kind),
                            )
                            .child(div().flex_1().min_w_0().child(var.field.clone()))
                            .child(
                                div()
                                    .w(px(58.))
                                    .flex_none()
                                    .font_family(MONO)
                                    .text_size(px(10.5))
                                    .text_color(t.text_muted)
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .child(fitted),
                            ),
                    )
                    .children((!is_expr).then(|| {
                        div()
                            .pl(px(56. + 46.))
                            .pt_0p5()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(
                                div()
                                    .text_size(px(10.5))
                                    .text_color(t.text_muted)
                                    .child("min"),
                            )
                            .child(div().w(px(56.)).child(var.min_field.clone()))
                            .child(
                                div()
                                    .text_size(px(10.5))
                                    .text_color(t.text_muted)
                                    .child("max"),
                            )
                            .child(div().w(px(56.)).child(var.max_field.clone()))
                    }))
                    .into_any_element(),
            );
        }
        let title: &'static str = "Parameters";
        let head = div()
            .px_3()
            .pt_3()
            .pb_1()
            .flex()
            .items_center()
            .gap_2()
            .child(section_label(&t, title))
            .child(div().flex_1())
            .child(
                div()
                    .font_family(MONO)
                    .text_size(px(10.))
                    .text_color(t.text_muted)
                    .child("vary · fixed · expression"),
            );
        div()
            .flex()
            .flex_col()
            .border_b_1()
            .border_color(t.border)
            .pb_2()
            .child(head)
            .children(rows)
    }

    /// guess → set → guess; an expression-defined variable becomes a guess
    /// (the expression is dropped) — same cycle as the old badge.
    /// Template segment + Apply, with the MS rule note.
    fn fit_template_chooser(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let mut seg = div().flex().flex_wrap().gap_1();
        for (i, tpl) in ParameterTemplate::ALL.into_iter().enumerate() {
            seg = seg.child(
                chip(
                    &t,
                    SharedString::from(format!("tpl-{i}")),
                    match tpl {
                        ParameterTemplate::PerShell => "Share by shell",
                        ParameterTemplate::PerPath => "Separate by path",
                        ParameterTemplate::FirstShellOnly => "Nearest shell",
                        ParameterTemplate::Manual => "Custom expressions",
                    },
                    self.fit_template == tpl,
                )
                .w(px(164.))
                .on_click(cx.listener(move |this, _: &ClickEvent, _w, cx| {
                    this.fit_template = tpl;
                    if tpl == ParameterTemplate::Manual {
                        this.fit_template_dirty = true;
                        cx.notify();
                    } else if !this.fit_template_dirty {
                        this.apply_fit_template(cx);
                    } else {
                        cx.notify();
                    }
                })),
            );
        }
        let dirty = self.fit_template_dirty;
        let n_sel = self.fit_paths.iter().filter(|r| r.spec.enabled).count();
        let apply_label: SharedString = if dirty {
            "Apply template (replaces edits)".into()
        } else {
            "Apply template".into()
        };
        let mut col = div()
            .mx_3()
            .mb_1()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_1()
                    .child(seg)
                    .when(self.fit_template!=ParameterTemplate::Manual,|d|d.child(
                        button(
                            &t,
                            "tpl-apply",
                            apply_label,
                            n_sel > 0 && (dirty || self.fit_vars.is_empty()),
                        )
                        .on_click(cx.listener(
                            |this, _: &ClickEvent, _w, cx| {
                                this.apply_fit_template(cx);
                                this.stage_view.fit_model_tab = 0;
                            },
                        )),
                    )),
            )
            .child(
                div()
                    .text_size(px(10.5))
                    .text_color(t.text_muted)
                    .child(match self.fit_template {
                        ParameterTemplate::PerShell=>"Paths in the same shell share ΔR and σ². S₀² and E₀ are shared within each structure. Multiple-scattering paths follow their constituent shells.",
                        ParameterTemplate::PerPath=>"Every path gets its own ΔR and σ². S₀² and E₀ stay shared within each structure; this adds more variables.",
                        ParameterTemplate::FirstShellOnly=>"Only the nearest selected shell varies in ΔR and σ². Other paths keep ΔR = 0 and σ² = 0.003 Å².",
                        ParameterTemplate::Manual=>"Keep your current model and edit Path expressions. A new name in a cell creates a variable; reuse a name to share it.",
                    }),
            );
        let selected = self.selected_path_infos();
        let shells: std::collections::BTreeSet<_> = selected
            .iter()
            .filter(|p| p.is_single_scattering)
            .map(|p| (self.fit_paths[p.index].spec.file.parent(), p.shell))
            .collect();
        col = col.child(div().text_size(px(11.)).text_color(t.text).child(format!(
            "{} selected paths · {} SS shells · {} parameter definitions",
            n_sel,
            shells.len(),
            self.fit_vars.len()
        )));
        if self.fit_template == ParameterTemplate::PerShell
            && shells.len() == 1
            && self.path_sources().len() == 1
        {
            col=col.child(div().text_size(px(10.5)).text_color(t.text_muted).child(if self.joint.config.enabled {
                "Four base parameters: S₀², E₀, ΔR and σ². Shared / Per spectrum scopes determine the total number of fitted values."
            } else { "One shell = 4 variables: S₀² + E₀ + ΔR + σ². Selecting another shell adds two variables." }));
        }
        for note in &self.fit_template_notes {
            col = col.child(
                div()
                    .text_size(px(10.5))
                    .text_color(t.text_muted)
                    .child(SharedString::from(note.clone())),
            );
        }
        if dirty {
            col = col.child(
                div()
                    .text_size(px(10.5))
                    .text_color(t.warn)
                    .child("edited by hand — the template no longer follows the selection; Apply template to regenerate"),
            );
        }
        col
    }

    fn cycle_var_kind(&mut self, name: &str, cx: &mut Context<Self>) {
        if let Some(v) = self.fit_vars.iter_mut().find(|v| v.spec.name == name) {
            if v.spec.expr.is_some() {
                v.spec.expr = None;
                v.spec.vary = true;
                let text = format!("{}", v.spec.value);
                v.field.update(cx, |f, cx| f.set_text(text, cx));
            } else {
                v.spec.vary = !v.spec.vary;
            }
            self.fit_template_dirty = true;
            self.fit_model_changed(cx);
        }
    }

    pub(super) fn fit_settings_section(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let resolved = self
            .fit_ranges
            .resolved(self.joint_params(&self.current_path).fft_kweight);
        let r = &resolved;
        let mut rows: Vec<gpui::AnyElement> = Vec::new();
        for (key, field) in &self.fit_range_fields {
            if *key == crate::app::RangeKey::Kweight {
                continue;
            }
            rows.push(field.clone().into_any_element());
        }
        rows.push(
            div()
                .px_3()
                .py_1()
                .child(
                    chip(
                        &t,
                        "fit-weight-transform",
                        format!(
                            "{} Transform k-weight ({:.0})",
                            if self.fit_ranges.follow_transform {
                                "☑"
                            } else {
                                "☐"
                            },
                            self.joint_params(&self.current_path)
                                .fft_kweight
                                .unwrap_or(2.)
                        ),
                        self.fit_ranges.follow_transform,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        let follow = !this.fit_ranges.follow_transform;
                        this.fit_ranges = this
                            .fit_ranges
                            .resolved(this.joint_params(&this.current_path).fft_kweight);
                        this.fit_ranges.follow_transform = follow;
                        this.fit_model_changed(cx);
                    })),
                )
                .into_any_element(),
        );
        // k-weights: 1 · 2 · 3 multi-select; the first selected is the plot weight.
        let mut kw_row = div().px_3().py_0p5().flex().items_center().gap_1p5().child(
            div()
                .flex_1()
                .text_size(px(12.))
                .text_color(t.text_muted)
                .child("k-weights (fit)"),
        );
        for kw in [1.0, 2.0, 3.0] {
            let on = r
                .effective_kweights()
                .iter()
                .any(|k| (*k - kw).abs() < 1e-9);
            kw_row = kw_row.child(
                div()
                    .id(SharedString::from(format!("fit-kw-{kw}")))
                    .w(px(26.))
                    .h(px(22.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_md()
                    .border_1()
                    .font_family(MONO)
                    .text_size(px(11.5))
                    .cursor_pointer()
                    .when(on, |d| {
                        d.bg(gpui::Rgba {
                            a: 0.16,
                            ..t.accent
                        })
                        .border_color(t.accent)
                        .text_color(t.text)
                    })
                    .when(!on, |d| {
                        d.border_color(t.border)
                            .text_color(t.text_muted)
                            .hover(|d| d.bg(t.raised))
                    })
                    .on_click(cx.listener(move |this, _: &ClickEvent, _w, cx| {
                        this.fit_ranges = this
                            .fit_ranges
                            .resolved(this.joint_params(&this.current_path).fft_kweight);
                        this.fit_ranges.toggle_kweight(kw);
                        let plot_kw = this.fit_ranges.kweight;
                        if !this.fit_ranges.kweights.contains(&plot_kw) {
                            this.fit_ranges.kweight = this.fit_ranges.kweights[0];
                        }
                        this.fit_model_changed(cx);
                    }))
                    .child(format!("{kw:.0}")),
            );
        }
        rows.push(kw_row.into_any_element());
        // fit space
        let mut space_seg = segmented(&t);
        for (i, sp) in FitSpaceSpec::ALL.into_iter().enumerate() {
            space_seg = space_seg.child(
                segment(
                    &t,
                    SharedString::from(format!("fs-{}", sp.label())),
                    sp.label(),
                    r.fitspace == sp,
                    i == 0,
                )
                .on_click(cx.listener(move |this, _: &ClickEvent, _w, cx| {
                    this.fit_ranges.fitspace = sp;
                    this.select_fit_space_view(sp, cx);
                    this.fit_model_changed(cx);
                })),
            );
        }
        rows.push(
            div()
                .px_3()
                .py_0p5()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .flex_1()
                        .text_size(px(12.))
                        .text_color(t.text_muted)
                        .child("fit space"),
                )
                .child(space_seg)
                .child(
                    chip(&t, "fit-noise", "ε from noise", r.noise).on_click(cx.listener(
                        |this, _: &ClickEvent, _w, cx| {
                            this.fit_ranges.noise = !this.fit_ranges.noise;
                            this.fit_model_changed(cx);
                        },
                    )),
                )
                .into_any_element(),
        );
        let nidp = r.n_idp();
        let nvar = self
            .fit_vars
            .iter()
            .filter(|v| v.spec.vary && v.spec.expr.is_none())
            .count();
        let over = nvar as f64 > nidp * 2.0 / 3.0;
        rows.push(
            div()
                .mx_3()
                .mt_1()
                .flex()
                .items_center()
                .justify_between()
                .text_size(px(11.5))
                .child(div().text_color(t.text_muted).child("N idp / N var"))
                .child(
                    div()
                        .font_family(MONO)
                        .text_color(if over { t.warn } else { t.text })
                        .child(format!(
                            "{nidp:.1} / {nvar}{}",
                            if over { " ⚠" } else { "" }
                        )),
                )
                .into_any_element(),
        );
        if over {
            rows.push(
                self.note("More than ⅔ of the independent points are being fit: fix a parameter or widen the ranges.")
                    .into_any_element(),
            );
        }
        self.section("Fit settings", None, rows, cx)
    }

    pub(super) fn fit_result_section(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let Some(result) = self.fit_result.clone() else {
            return self
                .section(
                    "Result",
                    None,
                    vec![self.note("No fit yet.").into_any_element()],
                    cx,
                )
                .into_any_element();
        };
        let stale = self.fit_is_stale();
        let eps: String = result
            .kweight_results
            .first()
            .map(|k| format!("{:.2e}", k.epsilon_k))
            .unwrap_or("—".into());
        let mut card = vec![
            ("R-factor".to_string(), format!("{:.5}", result.r_factor)),
            ("χ²".to_string(), format!("{:.4e}", result.chi_square)),
            (
                "reduced χ²".to_string(),
                format!("{:.4e}", result.reduced_chi_square),
            ),
            (
                "N idp / N var".to_string(),
                format!("{:.1} / {}", result.n_idp, result.n_vary),
            ),
            ("ε k (noise)".to_string(), eps),
        ];
        if let Some(d) = self.last_fit_duration {
            card.push(("time".into(), format!("{:.2} s", d.as_secs_f64())));
        }
        if let Some(report) = &result.solver_report {
            card.push((
                "optimizer".into(),
                if report.converged {
                    "converged"
                } else {
                    "stopped"
                }
                .into(),
            ));
            if let Some(iterations) = report.iterations {
                card.push(("iterations".into(), iterations.to_string()));
            }
            if let Some(evaluations) = report.evaluations {
                card.push(("evaluations".into(), evaluations.to_string()));
            }
            card.push((
                "objective".into(),
                format!("{:.3e} → {:.3e}", report.initial_cost, report.final_cost),
            ));
        }
        let mut rows: Vec<gpui::AnyElement> = vec![self.result_card(card).into_any_element()];
        if let Some(report) = &result.solver_report {
            rows.push(
                div()
                    .px_3()
                    .py_1()
                    .text_size(px(11.))
                    .text_color(t.text_muted)
                    .child(format!(
                        "{}. Numerical convergence alone does not establish a good fit.",
                        report.termination
                    ))
                    .into_any_element(),
            );
        }
        for notice in crate::fitting::fit_result_notices(&result) {
            rows.push(
                div()
                    .mx_3()
                    .mt_2()
                    .p_2()
                    .rounded_md()
                    .border_1()
                    .border_color(t.warn)
                    .text_size(px(11.))
                    .text_color(t.warn)
                    .child(notice)
                    .into_any_element(),
            );
        }
        rows.push(
            div()
                .px_3()
                .pt_3()
                .pb_1()
                .child(section_label(&t, "Fitted parameters · value ± uncertainty"))
                .into_any_element(),
        );
        for (name, variable) in &result.variables.vars {
            let value = match variable.stderr {
                Some(error) => format!("{:.5} ± {:.5}", variable.value, error),
                None => format!(
                    "{:.5} · {}",
                    variable.value,
                    if variable.vary && variable.expr.is_none() {
                        "± unavailable"
                    } else {
                        "fixed / expr"
                    }
                ),
            };
            rows.push(
                div()
                    .mx_3()
                    .py_1()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(t.border)
                    .font_family(MONO)
                    .text_size(px(11.))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(crate::joint_fitting::display_name(
                                name,
                                self.joint.result_config.as_ref(),
                            )),
                    )
                    .child(value)
                    .into_any_element(),
            );
        }
        if let Some(entry) = self
            .fit_history
            .iter()
            .find(|e| Some(e.id) == self.fit_history_selected)
        {
            rows.push(self.fit_path_details(entry).into_any_element());
        }
        let correlations = high_correlations(&result, CORRELATION_WARN);
        for (a, b, c) in correlations.iter().take(4) {
            rows.push(
                div()
                    .mx_3()
                    .mt_1()
                    .flex()
                    .items_center()
                    .justify_between()
                    .text_size(px(11.5))
                    .child(
                        div()
                            .font_family(MONO)
                            .text_color(t.warn)
                            .child(format!("corr({a}, {b})")),
                    )
                    .child(
                        div()
                            .font_family(MONO)
                            .text_color(t.warn)
                            .child(format!("{c:+.2} ⚠")),
                    )
                    .into_any_element(),
            );
        }
        if !correlations.is_empty() {
            rows.push(
                self.note("Strongly correlated parameters cannot be determined independently: fix one (e.g. S₀² from a standard), add a k-weight, or widen the R-range.")
                    .into_any_element(),
            );
        }
        for warning in &result.warnings {
            rows.push(
                div()
                    .mx_3()
                    .mt_1()
                    .text_size(px(11.))
                    .text_color(t.warn)
                    .child(format!("⚠ {warning:?}"))
                    .into_any_element(),
            );
        }
        let head = div()
            .px_3()
            .pt_3()
            .pb_1()
            .flex()
            .items_center()
            .gap_2()
            .child(section_label(&t, "Result"))
            .child(div().flex_1())
            .child(
                div()
                    .font_family(MONO)
                    .text_size(px(10.))
                    .px_1p5()
                    .rounded_full()
                    .border_1()
                    .border_color(if stale { t.warn } else { t.success })
                    .text_color(if stale { t.warn } else { t.success })
                    .child(if stale { "stale" } else { "current" }),
            );
        div()
            .flex()
            .flex_col()
            .border_b_1()
            .border_color(t.border)
            .pb_2()
            .child(head)
            .children(rows)
            .into_any_element()
    }

    fn fit_path_details(
        &self,
        entry: &crate::fitting::FitHistoryEntry,
    ) -> impl IntoElement + use<> {
        let t = self.theme;
        let mut details = div()
            .mx_3()
            .py_2()
            .flex()
            .flex_col()
            .gap_2()
            .text_size(px(11.));
        if entry.path_details.is_empty() {
            return details.child("Path distances were not recorded in this older history entry. Run a fit to record them.");
        }
        details = details.child(section_label(&t, "Fitted path distances"));
        for p in &entry.path_details {
            let value = |v: &Option<crate::fit_details::Estimate>, digits| {
                v.as_ref()
                    .map(|v| v.label(digits))
                    .unwrap_or_else(|| "unavailable".into())
            };
            let distance_label = if p.nleg.is_some_and(|n| n > 2) {
                "R_eff + ΔR · half path"
            } else {
                "R = R_eff + ΔR"
            };
            let source = p
                .file
                .parent()
                .and_then(|p| p.file_name())
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            details = details.child(
                div()
                    .py_1()
                    .border_b_1()
                    .border_color(t.border)
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .child(format!("{} · {}", p.label, source)),
                    )
                    .child(
                        div()
                            .text_color(t.accent)
                            .child(format!("{distance_label}   {} Å", value(&p.distance, 4))),
                    )
                    .child(div().text_color(t.text_muted).child(format!(
                            "FEFF R_eff {} Å · {} legs · degeneracy {}",
                            p.reff
                                .map(|r| format!("{r:.4}"))
                                .unwrap_or_else(|| "unavailable".into()),
                            p.nleg.map(|n| n.to_string()).unwrap_or_else(|| "?".into()),
                            p.degeneracy
                                .map(|n| n.to_string())
                                .unwrap_or_else(|| "?".into())
                        )))
                    .child(format!("ΔR  {} Å", value(&p.deltar, 4)))
                    .child(format!("σ²  {} Å²", value(&p.sigma2, 5)))
                    .child(format!(
                        "S₀²  {} · ΔE₀  {} eV",
                        value(&p.s02, 4),
                        value(&p.e0, 3)
                    )),
            );
        }
        details.child(div().text_size(px(10.)).text_color(t.text_muted).child("± one standard error, propagated from the fit covariance. FEFF reference geometry is treated as exact."))
    }

    pub(super) fn fit_history_section(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let mut rows: Vec<gpui::AnyElement> = Vec::new();
        if self.fit_history.is_empty() {
            rows.push(
                self.note("Every fit is recorded here with its model; click one to restore it.")
                    .into_any_element(),
            );
        }
        for entry in self.fit_history.iter().rev() {
            let id = entry.id;
            let on = self.fit_history_selected == Some(id);
            let summary: SharedString = entry.summary().into();
            rows.push(
                div()
                    .id(("fit-hist", id))
                    .mx_2()
                    .px_1p5()
                    .py_0p5()
                    .flex()
                    .flex_col()
                    .rounded_md()
                    .border_1()
                    .border_color(if on {
                        t.accent
                    } else {
                        gpui::Rgba { a: 0.0, ..t.border }
                    })
                    .when(on, |d| {
                        d.bg(gpui::Rgba {
                            a: 0.12,
                            ..t.accent
                        })
                    })
                    .cursor_pointer()
                    .hover(|d| d.bg(t.raised))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _w, cx| {
                        this.restore_fit_history(id, cx);
                    }))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .text_size(px(11.5))
                            .child(div().child(format!("fit {id} · {}", entry.group)))
                            .child(div().flex_1())
                            .child(
                                div()
                                    .font_family(MONO)
                                    .text_color(t.text_muted)
                                    .child(format!("R {:.4}", entry.r_factor)),
                            ),
                    )
                    .child(
                        div()
                            .font_family(MONO)
                            .text_size(px(10.))
                            .text_color(t.text_muted)
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .child(summary),
                    )
                    .when_some(entry.solver_report.as_ref(), |d, report| {
                        d.child(
                            div()
                                .text_size(px(10.))
                                .text_color(if report.converged {
                                    t.text_muted
                                } else {
                                    t.warn
                                })
                                .child(report.termination.clone()),
                        )
                    })
                    .into_any_element(),
            );
            if on {
                rows.push(
                    div()
                        .mx_3()
                        .py_2()
                        .text_size(px(11.))
                        .text_color(t.text_muted)
                        .child(format!(
                            "χ² {:.4} · reduced χ² {:.4} · independent points {:.1}",
                            entry.chi_square, entry.reduced_chi_square, entry.n_idp
                        ))
                        .into_any_element(),
                );
                for (name, value, error) in &entry.values {
                    let name = crate::joint_fitting::display_name(name, entry.joint.as_ref());
                    rows.push(
                        div()
                            .mx_3()
                            .font_family(MONO)
                            .text_size(px(11.))
                            .child(match error {
                                Some(e) => format!("{name}  {value:.6} ± {e:.6}"),
                                None => format!("{name}  {value:.6} ± unavailable"),
                            })
                            .into_any_element(),
                    );
                }
                rows.push(self.fit_path_details(entry).into_any_element());
            }
        }
        self.section("History", None, rows, cx)
    }

    pub(super) fn fit_batch_section(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let mut rows: Vec<gpui::AnyElement> = Vec::new();
        let can_run = self.batch_blocker().is_none();
        rows.push(
            div()
                .px_3()
                .py_0p5()
                .text_size(px(11.))
                .text_color(t.text_muted)
                .child(self.batch_scope_line())
                .into_any_element(),
        );
        let batch_label: SharedString = if self.batch_running {
            let (done, total) = self.batch_progress;
            format!("Cancel ({done}/{total})").into()
        } else {
            "Run batch fit".into()
        };
        rows.push(
            div()
                .px_3()
                .py_1()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    chip(
                        &t,
                        "batch-preview",
                        "preview · sampled frames",
                        self.batch_preview,
                    )
                    .on_click(
                        cx.listener(|this, _: &ClickEvent, _w, cx| this.toggle_batch_preview(cx)),
                    ),
                )
                .child(div().flex_1())
                .child(
                    button(&t, "batch-run", batch_label, can_run && !self.batch_running)
                        .when(!can_run && !self.batch_running, |d| {
                            d.opacity(0.45).cursor_default()
                        })
                        .when(self.batch_running, |d| {
                            d.border_color(t.error).text_color(t.error)
                        })
                        .on_click(cx.listener(|this, _: &ClickEvent, _w, cx| {
                            if this.batch_running {
                                this.cancel_batch_fit(cx);
                            } else {
                                this.run_batch_fit(cx);
                            }
                        })),
                )
                .into_any_element(),
        );
        if self.batch_running {
            let (done, total) = self.batch_progress;
            let frac = if total > 0 {
                done as f32 / total as f32
            } else {
                0.0
            };
            rows.push(
                div()
                    .mx_3()
                    .h(px(4.))
                    .rounded_full()
                    .bg(t.border)
                    .child(div().h_full().w(relative(frac)).rounded_full().bg(t.accent))
                    .into_any_element(),
            );
        }
        if let Some(bf) = &self.batch_fit {
            let stale = self.batch_fit_is_stale();
            let problems = bf.problems.len();
            let stopped = bf
                .rows
                .iter()
                .filter(|row| row.solver_report.as_ref().is_some_and(|r| !r.converged))
                .count();
            rows.push(
                div()
                    .px_3()
                    .py_0p5()
                    .flex()
                    .items_center()
                    .gap_1()
                    .text_size(px(11.))
                    .text_color(t.text_muted)
                    .child(format!(
                        "{} / {} complete · {stopped} stopped · {problems} errors{}",
                        bf.rows.len(),
                        bf.total,
                        if stale { " · stale" } else { "" }
                    ))
                    .child(div().flex_1())
                    .children((!stale).then(|| {
                        self.sub_button("batch-csv", "CSV…", cx, |this, cx| {
                            this.export_batch_csv(cx)
                        })
                    }))
                    .children((problems > 0).then(|| {
                        self.sub_button(
                            "batch-problems",
                            if bf.problems_open {
                                "hide problems"
                            } else {
                                "problems"
                            },
                            cx,
                            |this, cx| this.toggle_batch_problems(cx),
                        )
                    }))
                    .into_any_element(),
            );
            if bf.problems_open {
                rows.push(
                    div()
                        .mx_3()
                        .mb_1()
                        .child(self.batch_problems_list(cx))
                        .into_any_element(),
                );
            }
        }
        rows.push(
            self.note("Batch fits every frame of the active scan with this model; cancel keeps the completed rows. Results feed the Series trends.")
                .into_any_element(),
        );
        self.section("Batch fit", None, rows, cx)
    }
}

/// Cloned plot handles so the center can be built while `self` is borrowed
/// mutably for the handle overlay.
struct FitPlotHandles {
    k: Entity<RuvizPlot>,
    k_residual: Entity<RuvizPlot>,
    r: Entity<RuvizPlot>,
    r_residual: Entity<RuvizPlot>,
    q: Entity<RuvizPlot>,
}
