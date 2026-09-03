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

/// Correlation above which two parameters are flagged (Artemis warns at
/// 0.95; 0.9 catches the usual amp/σ² pair earlier).
const CORRELATION_WARN: f64 = 0.9;

impl StudioApp {
    /// "Editing <group>" · ▶ Fit · Batch fit…
    pub(crate) fn fit_inspector_header(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let label = self.current_group_label();
        let can_fit = self.fit_paths.iter().any(|p| p.spec.enabled) && !self.fit_running;
        let fit_label: SharedString = if self.fit_running {
            "fitting…".into()
        } else {
            "▶ Fit".into()
        };
        div()
            .flex_none()
            .px_3()
            .py_2()
            .flex()
            .items_center()
            .gap_2()
            .border_b_1()
            .border_color(t.border)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .text_size(px(11.5))
                    .text_color(t.text_muted)
                    .child(
                        div().flex().gap_1().child("Fitting").child(
                            div()
                                .text_color(t.text)
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .child(label),
                        ),
                    ),
            )
            .child(
                button(&t, "run-fit", fit_label, can_fit)
                    .when(!can_fit, |d| d.text_color(t.text_muted))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _w, cx| {
                        if can_fit {
                            this.run_fit_now(cx);
                        }
                    })),
            )
            .child(
                button(&t, "batch-fit-toggle", "Batch fit…", false).on_click(cx.listener(
                    |this, _: &ClickEvent, _w, cx| {
                        this.stage_view.fit_show_batch = !this.stage_view.fit_show_batch;
                        cx.notify();
                    },
                )),
            )
    }

    pub(crate) fn fit_stage_center(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let mut column = div()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .flex()
            .flex_col()
            .child(self.fit_plot_bar(cx));
        if self.structure.show {
            return column.child(self.structure_center(cx));
        }
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
            let hint: SharedString = if self.fit_paths.is_empty() {
                "Add FEFF paths (Generate… or + Add path) in the inspector, then press Fit".into()
            } else if self.fit_running {
                "fitting…".into()
            } else {
                "Press Fit to see data vs model".into()
            };
            let mut empty = column.child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(t.text_muted)
                    .child(hint),
            );
            if self.batch_fit.is_some() || self.stage_view.fit_show_batch {
                empty = empty.child(self.batch_results_table(cx));
            }
            return empty;
        };
        let view = self.stage_view.fit_view;
        let kw = self.fit_result.as_ref().map(|r| r.kweight).unwrap_or(2.0);
        let k_title: SharedString =
            format!("{} · data vs fit", crate::plotting::chik_label(kw)).into();
        let r_title: SharedString = format!(
            "{} · data vs fit{}",
            crate::plotting::chir_label(kw),
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
        if self.batch_fit.is_some() || self.stage_view.fit_show_batch {
            column = column.child(self.batch_results_table(cx));
        }
        column
    }

    /// Main plot card with an optional residual strip beneath.
    fn fit_column(
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
                    .h(px(110.))
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
            .child(
                chip(&t, "fit-structure", "structure", self.structure.show).on_click(cx.listener(
                    |this, _: &ClickEvent, _w, cx| {
                        this.structure.show = !this.structure.show;
                        if this.structure.show {
                            this.refresh_structure(cx);
                        }
                        cx.notify();
                    },
                )),
            )
            .child(div().w(px(1.)).h(px(18.)).bg(t.border))
            .child(
                chip(&t, "fit-paths", "paths", v.fit_show_paths).on_click(cx.listener(
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
                    .child("fit-range edges are draggable"),
            )
            .child(div().flex_1())
            .child(
                chip(&t, "fit-batch", "batch results", v.fit_show_batch).on_click(cx.listener(
                    |this, _: &ClickEvent, _w, cx| {
                        this.stage_view.fit_show_batch = !this.stage_view.fit_show_batch;
                        cx.notify();
                    },
                )),
            )
    }

    // ---- inspector ----------------------------------------------------------

    pub(crate) fn fit_inspector(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        div()
            .flex()
            .flex_col()
            .child(self.structure_section(cx))
            .child(self.fit_paths_section(cx))
            .child(self.fit_params_section(cx))
            .child(self.fit_settings_section(cx))
            .child(self.fit_result_section(cx))
            .child(self.fit_history_section(cx))
            .children(
                self.stage_view
                    .fit_show_batch
                    .then(|| self.fit_batch_section(cx)),
            )
            .child(div().h(px(12.)).bg(t.surface))
    }

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

    fn fit_paths_section(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let mut rows: Vec<gpui::AnyElement> = Vec::new();
        if self.fit_paths.is_empty() {
            rows.push(
                self.note("No paths yet. Generate them from a crystal structure (Generate…) or add FEFF path files (+ Add).")
                    .into_any_element(),
            );
        }
        for (i, row) in self.fit_paths.iter().enumerate() {
            let enabled = row.spec.enabled;
            let expanded = row.expanded;
            let label: SharedString = row.spec.label.clone().into();
            let meta: SharedString = match &row.meta {
                Some(m) => format!("{:.3} Å · N {:.0} · {} legs", m.reff, m.degen, m.nleg).into(),
                None => "".into(),
            };
            let mut card = div().mx_2().flex().flex_col().child(
                div()
                    .id(("fit-path", i))
                    .h(px(26.))
                    .px_1()
                    .flex()
                    .items_center()
                    .gap_2()
                    .rounded_md()
                    .cursor_pointer()
                    .hover(|d| d.bg(t.raised))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _w, cx| {
                        if let Some(p) = this.fit_paths.get_mut(i) {
                            p.expanded = !p.expanded;
                            cx.notify();
                        }
                    }))
                    .child(
                        div()
                            .id(("fit-path-en", i))
                            .flex_none()
                            .w(px(14.))
                            .h(px(14.))
                            .rounded_sm()
                            .border_1()
                            .flex()
                            .items_center()
                            .justify_center()
                            .when(enabled, |d| d.bg(t.accent).border_color(t.accent))
                            .when(!enabled, |d| d.bg(t.raised).border_color(t.border))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _w, cx| {
                                cx.stop_propagation();
                                if let Some(p) = this.fit_paths.get_mut(i) {
                                    p.spec.enabled = !p.spec.enabled;
                                    this.fit_model_changed(cx);
                                }
                            }))
                            .child(div().text_size(px(10.)).text_color(t.bg).child(if enabled {
                                "✓"
                            } else {
                                ""
                            })),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .text_color(if enabled { t.text } else { t.text_muted })
                            .child(label),
                    )
                    .child(
                        div()
                            .flex_none()
                            .font_family(MONO)
                            .text_size(px(10.5))
                            .text_color(t.text_muted)
                            .child(meta),
                    )
                    .child(
                        div()
                            .flex_none()
                            .text_color(t.text_muted)
                            .text_size(px(10.))
                            .child(if expanded { "▾" } else { "▸" }),
                    ),
            );
            if expanded {
                let more = row.more;
                let mut grid = div().ml_6().mr_1().mb_1().flex().flex_wrap().gap_1();
                for (param, field) in &row.fields {
                    if !param.is_primary() && !more {
                        continue;
                    }
                    grid = grid.child(
                        div()
                            .w(px(120.))
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
                grid = grid.child(
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
                card = card.child(grid);
            }
            rows.push(card.into_any_element());
        }
        rows.push(
            div()
                .mx_2()
                .mt_1()
                .flex()
                .items_center()
                .gap_1()
                .child(self.sub_button("add-path", "+ Add path…", cx, |this, cx| {
                    this.add_fit_path_dialog(cx)
                }))
                .child(
                    self.sub_button("show-structure", "Structure view", cx, |this, cx| {
                        this.structure.show = true;
                        this.refresh_structure(cx);
                        cx.notify();
                    }),
                )
                .into_any_element(),
        );
        let title: &'static str = "Paths";
        let count = self.fit_paths.iter().filter(|p| p.spec.enabled).count();
        let head = div()
            .px_3()
            .pt_3()
            .pb_1()
            .flex()
            .items_center()
            .gap_2()
            .child(section_label(&t, title))
            .child(
                div()
                    .font_family(MONO)
                    .text_size(px(10.5))
                    .text_color(t.text_muted)
                    .child(format!("{count} / {}", self.fit_paths.len())),
            )
            .child(div().flex_1());
        let table = (!self.fit_paths.is_empty()).then(|| {
            div()
                .mx_2()
                .mb_1()
                .max_h(px(240.))
                .flex()
                .flex_col()
                .rounded_md()
                .border_1()
                .border_color(t.border)
                .overflow_hidden()
                .child(self.structure_paths_table(false, cx))
        });
        div()
            .flex()
            .flex_col()
            .border_b_1()
            .border_color(t.border)
            .pb_2()
            .child(head)
            .children(table)
            .children(rows)
    }

    /// Structure database + cluster generator (replaces the Atoms-lite form).
    fn structure_section(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let head = div()
            .px_3()
            .pt_3()
            .pb_1()
            .flex()
            .items_center()
            .gap_2()
            .child(section_label(&t, "Structure"))
            .child(div().flex_1());
        let advanced = div()
            .px_3()
            .pt_1()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_1()
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(t.text_muted)
                    .child("custom feff.inp:"),
            )
            .child(self.sub_button("feff-new", "New…", cx, |this, cx| this.new_feff_inp(cx)))
            .child(self.sub_button("feff-choose", "Choose…", cx, |this, cx| {
                this.choose_feff_inp(cx)
            }))
            .child(self.sub_button("feff-run", "Run FEFF", cx, |this, cx| {
                this.run_feff10_now(cx)
            }));
        div()
            .flex()
            .flex_col()
            .border_b_1()
            .border_color(t.border)
            .pb_2()
            .child(head)
            .child(self.structure_panel(cx))
            .child(advanced)
    }

    fn fit_params_section(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let result = self.fit_result.clone();
        let fresh = !self.fit_is_stale();
        let mut rows: Vec<gpui::AnyElement> = Vec::new();
        if self.fit_vars.is_empty() {
            rows.push(
                self.note("Variables appear when a path cell references a name (amp, de0, sig2_1 …). Type an expression such as dr_1*1.41 to define one from another.")
                    .into_any_element(),
            );
        }
        for var in &self.fit_vars {
            let name = var.spec.name.clone();
            let is_expr = var.spec.expr.is_some();
            let vary = var.spec.vary;
            let (kind, color) = if is_expr {
                ("def", t.success)
            } else if vary {
                ("guess", t.accent)
            } else {
                ("set", t.text_muted)
            };
            let fitted: SharedString = result
                .as_ref()
                .filter(|_| fresh && vary && !is_expr)
                .and_then(|r| r.variables.get(&name))
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
                    .child("guess · def · set"),
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
            self.fit_model_changed(cx);
        }
    }

    fn fit_settings_section(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let r = &self.fit_ranges;
        let mut rows: Vec<gpui::AnyElement> = Vec::new();
        for (key, field) in &self.fit_range_fields {
            if *key == crate::app::RangeKey::Kweight {
                continue;
            }
            rows.push(field.clone().into_any_element());
        }
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

    fn fit_result_section(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
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
        let mut rows: Vec<gpui::AnyElement> = vec![self.result_card(card).into_any_element()];
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

    fn fit_history_section(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
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
                    .into_any_element(),
            );
        }
        self.section("History", None, rows, cx)
    }

    fn fit_batch_section(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let mut rows: Vec<gpui::AnyElement> = Vec::new();
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
                    button(&t, "batch-run", batch_label, !self.batch_running)
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
                        "{} / {} fitted · {problems} problems{}",
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
