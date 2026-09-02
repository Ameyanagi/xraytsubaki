//! Center for the four processing stages: a plot bar, the stage's main
//! plot(s) with drag handles, the shared legend strip, and the four-thumbnail
//! ripple strip.

use gpui::{
    ClickEvent, Context, IntoElement, ParentElement, SharedString, Styled, div, prelude::*, px,
};

use super::{
    BkgView, EQuantity, MONO, PlotScope, Stage, StageStatus, TfView, chip, segment, segmented,
};
use crate::app::{ParamKey, StudioApp};

/// Quadrant slots in `StudioApp::quadrants` (built by `build_quadrant_specs`).
pub const PLOT_MU: usize = 0;
pub const PLOT_NORM: usize = 1;
pub const PLOT_CHIK: usize = 2;
pub const PLOT_CHIR: usize = 3;
pub const PLOT_CHIQ: usize = 4;

impl StudioApp {
    pub(crate) fn stage_center(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let ready = self.quadrants.len() > PLOT_CHIQ;
        let mut column = div()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .flex()
            .flex_col()
            .child(self.plot_bar(cx));
        if !ready {
            return column.child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(t.text_muted)
                    .child(self.status.clone()),
            );
        }
        let plots: Vec<(usize, SharedString)> = self.stage_plots();
        let mut area = div()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_2()
            .px_3()
            .pt_2();
        for (index, title) in plots {
            area = area.child(self.plot_card(index, title, cx));
        }
        if self.stage == Stage::Data
            && let Some(plot) = self.analysis.plot.clone()
            && let Some(tool) = self.analysis.shown
        {
            let title: SharedString = match tool {
                super::tools::Tool::Lcf => {
                    "linear combination fit · data / fit / components / residual".into()
                }
                _ => "PCA target transform · data / reconstruction / residual".into(),
            };
            area = area.child(
                div()
                    .flex_none()
                    .h(px(300.))
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
                            .text_size(px(11.5))
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .child(title),
                    )
                    .child(div().flex_1().min_h_0().min_w_0().p_1().child(plot)),
            );
        }
        column = column.child(area);
        if self.view.legend && !self.legend_entries.is_empty() {
            column = column.child(self.legend_strip());
        }
        column.child(self.thumbnail_strip(cx))
    }

    /// Which quadrant plots the current stage shows, top to bottom.
    pub(crate) fn stage_plots(&self) -> Vec<(usize, SharedString)> {
        let v = self.stage_view;
        let kw = self.fft_summary().3;
        let chik: SharedString = crate::plotting::chik_label(kw).into();
        let chir: SharedString = crate::plotting::chir_label(kw).into();
        match self.stage {
            Stage::Data | Stage::Normalize => vec![match v.e_quantity {
                EQuantity::Mu => (PLOT_MU, "μ(E)".into()),
                EQuantity::Norm => (PLOT_NORM, "normalized μ(E)".into()),
                EQuantity::Flat => (PLOT_NORM, "flattened μ(E)".into()),
            }],
            Stage::Background => match v.bkg_view {
                BkgView::Energy => vec![
                    (PLOT_MU, "μ(E) with AUTOBK spline".into()),
                    (
                        PLOT_CHIR,
                        "|χ(R)| · R < Rbkg is the background region".into(),
                    ),
                ],
                BkgView::K => vec![(PLOT_CHIK, chik), (PLOT_CHIR, chir)],
            },
            Stage::Transform => match v.tf_view {
                TfView::K => vec![(PLOT_CHIK, chik)],
                TfView::R => vec![(PLOT_CHIR, chir)],
                TfView::Q => vec![(PLOT_CHIQ, "χ(q) back-transform".into())],
                TfView::Both => vec![(PLOT_CHIK, chik), (PLOT_CHIR, chir)],
            },
            Stage::Fit | Stage::Series => Vec::new(),
        }
    }

    fn plot_bar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let v = self.stage_view;
        let marked = self.selection.len();
        let mut bar = div()
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
            .overflow_hidden();
        // scope
        bar = bar
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(t.text_muted)
                    .child("Plot"),
            )
            .child(
                segmented(&t)
                    .child(
                        segment(
                            &t,
                            "scope-current",
                            "current",
                            v.scope == PlotScope::Current,
                            true,
                        )
                        .on_click(cx.listener(
                            |this, _: &ClickEvent, _w, cx| {
                                this.stage_view.scope = PlotScope::Current;
                                this.stage_view_changed(cx);
                            },
                        )),
                    )
                    .child(
                        segment(
                            &t,
                            "scope-marked",
                            format!("marked ({marked})"),
                            v.scope == PlotScope::Marked,
                            false,
                        )
                        .on_click(cx.listener(
                            |this, _: &ClickEvent, _w, cx| {
                                this.stage_view.scope = PlotScope::Marked;
                                this.stage_view_changed(cx);
                            },
                        )),
                    ),
            )
            .child(div().w(px(1.)).h(px(18.)).bg(t.border));
        match self.stage {
            Stage::Data | Stage::Normalize => {
                let q = v.e_quantity;
                bar =
                    bar.child(
                        segmented(&t)
                            .child(
                                segment(&t, "eq-mu", "μ(E)", q == EQuantity::Mu, true).on_click(
                                    cx.listener(|this, _: &ClickEvent, _w, cx| {
                                        this.stage_view.e_quantity = EQuantity::Mu;
                                        this.stage_view_changed(cx);
                                    }),
                                ),
                            )
                            .child(
                                segment(&t, "eq-norm", "norm", q == EQuantity::Norm, false)
                                    .on_click(cx.listener(|this, _: &ClickEvent, _w, cx| {
                                        this.stage_view.e_quantity = EQuantity::Norm;
                                        this.stage_view_changed(cx);
                                    })),
                            )
                            .child(
                                segment(&t, "eq-flat", "flat", q == EQuantity::Flat, false)
                                    .on_click(cx.listener(|this, _: &ClickEvent, _w, cx| {
                                        this.stage_view.e_quantity = EQuantity::Flat;
                                        this.stage_view_changed(cx);
                                    })),
                            ),
                    );
                let lines_on = self.view.show_pre && self.view.show_post;
                bar = bar
                    .child(chip(&t, "chip-lines", "pre/post lines", lines_on).on_click(
                        cx.listener(move |this, _: &ClickEvent, _w, cx| {
                            this.view.show_pre = !lines_on;
                            this.view.show_post = !lines_on;
                            this.stage_view_changed(cx);
                        }),
                    ))
                    .child(
                        chip(&t, "chip-e0", "E₀", self.view.show_e0).on_click(cx.listener(
                            |this, _: &ClickEvent, _w, cx| {
                                this.view.show_e0 = !this.view.show_e0;
                                this.stage_view_changed(cx);
                            },
                        )),
                    )
                    .child(
                        chip(&t, "chip-deriv", "derivative", self.view.show_deriv).on_click(
                            cx.listener(|this, _: &ClickEvent, _w, cx| {
                                this.view.show_deriv = !this.view.show_deriv;
                                this.stage_view_changed(cx);
                            }),
                        ),
                    );
            }
            Stage::Background => {
                bar = bar
                    .child(
                        segmented(&t)
                            .child(
                                segment(
                                    &t,
                                    "bv-e",
                                    "μ(E) + bkg",
                                    v.bkg_view == BkgView::Energy,
                                    true,
                                )
                                .on_click(cx.listener(
                                    |this, _: &ClickEvent, _w, cx| {
                                        this.stage_view.bkg_view = BkgView::Energy;
                                        this.stage_view_changed(cx);
                                    },
                                )),
                            )
                            .child(
                                segment(&t, "bv-k", "χ(k)", v.bkg_view == BkgView::K, false)
                                    .on_click(cx.listener(|this, _: &ClickEvent, _w, cx| {
                                        this.stage_view.bkg_view = BkgView::K;
                                        this.stage_view_changed(cx);
                                    })),
                            ),
                    )
                    .child(self.kweight_buttons(cx))
                    .child(
                        chip(&t, "chip-bkg", "show spline", v.show_bkg).on_click(cx.listener(
                            |this, _: &ClickEvent, _w, cx| {
                                this.stage_view.show_bkg = !this.stage_view.show_bkg;
                                this.stage_view_changed(cx);
                            },
                        )),
                    );
            }
            Stage::Transform => {
                let tv = v.tf_view;
                bar = bar
                    .child(
                        segmented(&t)
                            .child(segment(&t, "tv-k", "χ(k)", tv == TfView::K, true).on_click(
                                cx.listener(|this, _: &ClickEvent, _w, cx| {
                                    this.stage_view.tf_view = TfView::K;
                                    this.stage_view_changed(cx);
                                }),
                            ))
                            .child(
                                segment(&t, "tv-r", "χ(R)", tv == TfView::R, false).on_click(
                                    cx.listener(|this, _: &ClickEvent, _w, cx| {
                                        this.stage_view.tf_view = TfView::R;
                                        this.stage_view_changed(cx);
                                    }),
                                ),
                            )
                            .child(
                                segment(&t, "tv-q", "χ(q)", tv == TfView::Q, false).on_click(
                                    cx.listener(|this, _: &ClickEvent, _w, cx| {
                                        this.stage_view.tf_view = TfView::Q;
                                        this.stage_view_changed(cx);
                                    }),
                                ),
                            )
                            .child(
                                segment(&t, "tv-both", "k + R", tv == TfView::Both, false)
                                    .on_click(cx.listener(|this, _: &ClickEvent, _w, cx| {
                                        this.stage_view.tf_view = TfView::Both;
                                        this.stage_view_changed(cx);
                                    })),
                            ),
                    )
                    .child(self.kweight_buttons(cx))
                    .child(chip(&t, "chip-re", "Re", v.show_re).on_click(cx.listener(
                        |this, _: &ClickEvent, _w, cx| {
                            this.stage_view.show_re = !this.stage_view.show_re;
                            this.stage_view_changed(cx);
                        },
                    )))
                    .child(
                        chip(&t, "chip-win", "window", self.view.show_kwin).on_click(cx.listener(
                            |this, _: &ClickEvent, _w, cx| {
                                this.view.show_kwin = !this.view.show_kwin;
                                this.invalidate_explore_plots(cx);
                                cx.notify();
                            },
                        )),
                    );
            }
            _ => {}
        }
        let waterfall = self.view.layout == crate::plotting::TraceLayout::Waterfall;
        bar.child(div().flex_1())
            .child(
                chip(&t, "chip-offset", "offset", waterfall).on_click(cx.listener(
                    |this, _: &ClickEvent, _w, cx| {
                        this.view.layout = match this.view.layout {
                            crate::plotting::TraceLayout::Overlay => {
                                crate::plotting::TraceLayout::Waterfall
                            }
                            crate::plotting::TraceLayout::Waterfall => {
                                crate::plotting::TraceLayout::Overlay
                            }
                        };
                        this.invalidate_explore_plots(cx);
                        cx.notify();
                    },
                )),
            )
            .child(
                chip(&t, "chip-legend", "legend", self.view.legend).on_click(cx.listener(
                    |this, _: &ClickEvent, _w, cx| {
                        this.view.legend = !this.view.legend;
                        this.invalidate_explore_plots(cx);
                        cx.notify();
                    },
                )),
            )
            .child(
                chip(&t, "chip-grid", "grid", self.view.grid).on_click(cx.listener(
                    |this, _: &ClickEvent, _w, cx| {
                        this.view.grid = !this.view.grid;
                        this.invalidate_explore_plots(cx);
                        cx.notify();
                    },
                )),
            )
    }

    /// k-weight 0/1/2/3 (plotting + forward FT; AUTOBK's k-weight is its own).
    fn kweight_buttons(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let current = self.fft_summary().3.round() as i32;
        let mut row = div().flex().items_center().gap_1().child(
            div()
                .text_size(px(11.))
                .text_color(t.text_muted)
                .child("k-weight"),
        );
        for kw in 0..=3 {
            let on = kw == current;
            row = row.child(
                div()
                    .id(SharedString::from(format!("kw-{kw}")))
                    .w(px(24.))
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
                        this.apply_param(ParamKey::FftKweight, Some(kw as f64), cx);
                        this.sync_param_fields(cx);
                    }))
                    .child(kw.to_string()),
            );
        }
        row
    }

    /// One main plot with its title, the handle overlay, and a hover hint.
    fn plot_card(
        &mut self,
        index: usize,
        title: SharedString,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let t = self.theme;
        let plot = self.quadrants[index].1.clone();
        let label = self.current_group_label();
        let overlay = self.handle_overlay(index, cx);
        div()
            .id(SharedString::from(format!("plot-card-{index}")))
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
            .on_mouse_move(cx.listener(move |this, ev: &gpui::MouseMoveEvent, _w, cx| {
                this.plot_pointer_move(index, ev.position, cx);
            }))
            .child(
                div()
                    .flex_none()
                    .px_3()
                    .pt_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_size(px(11.5))
                    .child(div().font_weight(gpui::FontWeight::MEDIUM).child(title))
                    .child(div().text_color(t.text_muted).child(label)),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .p_1()
                    .relative()
                    .child(plot)
                    .child(self.measure_card(index, cx))
                    .children(self.handle_layer(index, cx)),
            )
            .children(overlay)
    }

    pub(crate) fn current_group_label(&self) -> SharedString {
        match self.selected {
            Some(ix) if ix != crate::app::NO_ENTRY => self.entry_label(ix).into(),
            _ => self.spectrum_label.clone(),
        }
    }

    /// Four static thumbnails of the current group, one per downstream
    /// stage; clicking one jumps to that stage.
    fn thumbnail_strip(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let kw = self.fft_summary().3;
        let captions: [(Stage, String); 4] = [
            (Stage::Normalize, "norm μ(E)".into()),
            (Stage::Background, crate::plotting::chik_label(kw)),
            (
                Stage::Transform,
                format!("{} · window", crate::plotting::chik_label(kw)),
            ),
            (Stage::Fit, "|χ(R)|".into()),
        ];
        let mut strip = div()
            .h(px(118.))
            .flex_none()
            .flex()
            .gap_2()
            .px_3()
            .pt_2()
            .pb_3();
        for (i, (stage, caption)) in captions.into_iter().enumerate() {
            let active = self.stage == stage;
            let (status, _) = self.stage_summary(stage);
            let dot = if status == StageStatus::Idle {
                t.text_muted
            } else {
                status.color(&t)
            };
            let data = self.thumbs.clone();
            strip = strip.child(
                div()
                    .id(SharedString::from(format!("thumb-{i}")))
                    .flex_1()
                    .min_w_0()
                    .relative()
                    .rounded_md()
                    .bg(t.raised)
                    .border_1()
                    .border_color(if active { t.accent } else { t.border })
                    .cursor_pointer()
                    .hover(|d| d.border_color(t.accent))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _w, cx| {
                        this.set_stage(stage, cx);
                    }))
                    .child(
                        div()
                            .absolute()
                            .top_1()
                            .left_2()
                            .flex()
                            .items_center()
                            .gap_1p5()
                            .text_size(px(10.5))
                            .text_color(t.text_muted)
                            .child(div().w(px(6.)).h(px(6.)).rounded_full().bg(dot))
                            .child(caption),
                    )
                    .children(data.map(|d| {
                        div()
                            .size_full()
                            .pt_4()
                            .child(super::thumbnails::sparkline(d, i, t))
                    })),
            );
        }
        strip
    }
}
