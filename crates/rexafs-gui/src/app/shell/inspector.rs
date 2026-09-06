//! Inspector: the right panel. A sticky header states the edit scope
//! ("Editing <group>") and offers Apply-to-marked / Reset; below it the
//! stage's parameter sections in pipeline order, then a Result card.

use gpui::{
    ClickEvent, Context, IntoElement, ParentElement, SharedString, Styled, div, prelude::*, px,
};

use super::{MONO, Stage, button, section_label};
use crate::app::{DERIVED_BASE, EnumParam, ParamKey, ParamSection, StudioApp};

impl StudioApp {
    pub(crate) fn inspector(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let body = match self.stage {
            Stage::Data => self.data_inspector(cx).into_any_element(),
            Stage::Normalize => self.normalize_inspector(cx).into_any_element(),
            Stage::Background => self.background_inspector(cx).into_any_element(),
            Stage::Transform => self.transform_inspector(cx).into_any_element(),
            Stage::Series => self.series_inspector(cx).into_any_element(),
            Stage::Fit | Stage::Publish => div().into_any_element(),
        };
        div()
            .w(px(312.))
            .h_full()
            .min_h_0()
            .min_w_0()
            .flex_none()
            .flex()
            .flex_col()
            .bg(t.surface)
            .border_l_1()
            .border_color(t.border)
            .child(self.inspector_header(cx))
            .child(
                div()
                    .id("inspector-scroll")
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .overflow_y_scroll()
                    .track_scroll(&self.inspector_scroll)
                    .child(body),
            )
            .into_any_element()
    }

    fn inspector_header(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        if self.stage == Stage::Series {
            return self.series_inspector_header(cx).into_any_element();
        }
        let t = self.theme;
        let label = self.current_group_label();
        let marked = self
            .selection
            .iter()
            .filter(|&&ix| ix < DERIVED_BASE && Some(ix) != self.selected)
            .count();
        let mut header = div()
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
                        div().flex().gap_1().child("Editing").child(
                            div()
                                .text_color(t.text)
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .child(label),
                        ),
                    ),
            );
        if self.stage.is_processing() {
            header = header
                .child(
                    button(
                        &t,
                        "apply-marked",
                        format!("Apply to marked ({marked})"),
                        false,
                    )
                    .on_click(cx.listener(|this, _: &ClickEvent, _w, cx| {
                        this.apply_params_to_marked(cx);
                    })),
                )
                .child(
                    div()
                        .id("reset-params")
                        .px_1p5()
                        .h(px(24.))
                        .flex()
                        .items_center()
                        .rounded_md()
                        .text_size(px(11.5))
                        .text_color(t.text_muted)
                        .cursor_pointer()
                        .hover(|d| d.bg(t.raised).text_color(t.text))
                        .on_click(cx.listener(|this, _: &ClickEvent, _w, cx| {
                            this.reset_params(cx);
                        }))
                        .child("Reset"),
                );
        }
        let count = self
            .differing_settings(super::parameter_actions::ParamScope::Stage(self.stage))
            .len();
        div()
            .flex()
            .flex_col()
            .child(header)
            .when(
                self.stage_view.scope == super::PlotScope::Marked && count > 0,
                |d| {
                    d.child(
                        div()
                            .id("marked-settings-differ")
                            .px_3()
                            .py_1()
                            .text_size(px(11.5))
                            .text_color(t.warn)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.param_menu =
                                    Some(super::parameter_actions::ParamScope::Stage(this.stage));
                                cx.notify();
                            }))
                            .child(format!(
                                "⚠ {count} {}  ›",
                                if count == 1 {
                                    "setting differs"
                                } else {
                                    "settings differ"
                                }
                            )),
                    )
                },
            )
            .into_any_element()
    }

    /// Copy the displayed parameter set onto every marked catalog group.
    pub(crate) fn apply_params_to_marked(&mut self, cx: &mut Context<Self>) {
        self.apply_scope_to_marked(super::parameter_actions::ParamScope::All, cx);
    }

    /// Drop the current group's override (or reset the globals to defaults).
    pub(crate) fn reset_params(&mut self, cx: &mut Context<Self>) {
        let target = self.override_target();
        let before = self.ui_params().clone();
        match target {
            Some(ix) => {
                self.overrides.remove(&ix);
            }
            None => {
                self.params = crate::params::PipelineParams::default();
            }
        }
        let after = self.ui_params().clone();
        self.record_param_edit(target, None, before, after, "reset parameters".into());
        self.sync_param_fields(cx);
        self.schedule_recompute(cx);
        self.sync_handles(cx);
        cx.notify();
    }

    pub(crate) fn field(&self, key: ParamKey, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let mixed = self.stage_view.scope == super::PlotScope::Marked
            && !self
                .differing_settings(super::parameter_actions::ParamScope::Field(
                    key.setting_key(),
                ))
                .is_empty();
        self.param_fields
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, f)| {
                f.update(cx, |field, cx| field.set_mixed(mixed, cx));
                self.parameter_context(
                    super::parameter_actions::ParamScope::Field(key.setting_key()),
                    f.clone().into_any_element(),
                    cx,
                )
            })
    }

    /// Section: uppercase label, optional override chip, then rows.
    pub(crate) fn section(
        &self,
        title: &'static str,
        section: Option<ParamSection>,
        rows: Vec<gpui::AnyElement>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let t = self.theme;
        let mut head = div()
            .id(SharedString::from(format!("section-context-{title}")))
            .px_3()
            .pt_3()
            .pb_1()
            .flex()
            .items_center()
            .gap_2()
            .child(section_label(&t, title))
            .child(div().flex_1());
        if let Some(section) = section {
            head = head.children(self.override_chip(
                SharedString::from(format!("ovr-{title}")),
                section,
                cx,
            ));
        }
        if super::parameter_actions::SETTINGS
            .iter()
            .any(|s| s.section == title)
        {
            let scope = super::parameter_actions::ParamScope::Section(title);
            head = head.children(self.parameter_badge(scope, cx)).child(
                button(
                    &t,
                    SharedString::from(format!("section-actions-{title}")),
                    "⋯",
                    false,
                )
                .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                    this.param_context_menu = Some((scope, event.position()));
                    cx.notify();
                })),
            );
            head = head.on_mouse_down(
                gpui::MouseButton::Right,
                cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    this.param_context_menu = Some((scope, event.position));
                    cx.notify();
                }),
            );
        }
        div()
            .flex()
            .flex_col()
            .border_b_1()
            .border_color(t.border)
            .pb_2()
            .child(head)
            .children(rows)
    }

    /// Key/value result card.
    pub(crate) fn result_card(&self, rows: Vec<(String, String)>) -> impl IntoElement + use<> {
        let t = self.theme;
        let mut card = div().mx_3().mt_1().flex().flex_col().gap_1();
        for (k, v) in rows {
            card = card.child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .text_size(px(11.5))
                    .child(div().text_color(t.text_muted).child(k))
                    .child(div().font_family(MONO).text_color(t.text).child(v)),
            );
        }
        card
    }

    pub(crate) fn note(&self, text: &'static str) -> impl IntoElement + use<> {
        let t = self.theme;
        div()
            .mx_3()
            .mt_1()
            .px_2()
            .py_1()
            .rounded_md()
            .border_1()
            .border_color(t.border)
            .text_size(px(11.))
            .text_color(t.text_muted)
            .child(text)
    }

    fn data_inspector(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let sp = self.spectrum.as_deref();
        let meta = vec![
            ("Group".into(), self.current_group_label().to_string()),
            (
                "Points".into(),
                sp.and_then(|s| s.energy.as_ref())
                    .map(|e| format!("{} · {:.1}–{:.1} eV", e.len(), e[0], e[e.len() - 1]))
                    .unwrap_or("—".into()),
            ),
            (
                "E₀".into(),
                sp.and_then(|s| s.e0())
                    .map(|v| format!("{v:.1} eV"))
                    .unwrap_or("—".into()),
            ),
        ];
        div()
            .flex()
            .flex_col()
            .child(self.section("Import", Some(ParamSection::Import), self.import_rows(cx), cx))
            .child(self.section(
                "Processing tools",
                None,
                vec![
                    div().px_2().child(self.tools_section(cx)).into_any_element(),
                    self.note("Tools never modify the source: each Apply creates a derived group (↳) that runs through the same pipeline.")
                        .into_any_element(),
                ],
                cx,
            ))
            .child(self.section(
                "Analysis",
                None,
                vec![
                    div().px_2().child(self.analysis_tools_section(cx)).into_any_element(),
                    self.note("LCF fits the current group as a mix of the marked groups; PCA trains on the marked groups and projects the current one.")
                        .into_any_element(),
                ],
                cx,
            ))
            .child(self.section("Metadata", None, vec![self.result_card(meta).into_any_element()], cx))
            .child(div().h(px(12.)).bg(t.surface))
    }

    fn normalize_inspector(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let sp = self.spectrum.as_deref();
        let e0 = sp.and_then(|s| s.e0());
        let step = self.edge_step();
        let whiteline = sp
            .and_then(|s| s.norm())
            .map(|n| n.iter().copied().fold(f64::NEG_INFINITY, f64::max));
        let fmt = |v: Option<f64>, d: usize, unit: &str| {
            v.map(|v| format!("{v:.d$}{unit}")).unwrap_or("—".into())
        };
        div()
            .flex()
            .flex_col()
            .child(self.section(
                "Edge",
                Some(ParamSection::Norm),
                vec![self.field(ParamKey::E0, cx)].into_iter().flatten().collect(),
                cx,
            ))
            .child(self.section(
                "Pre-edge line",
                None,
                [
                    self.field(ParamKey::PreEdgeStart, cx),
                    self.field(ParamKey::PreEdgeEnd, cx),
                    self.field(ParamKey::NVictoreen, cx),
                    Some(
                        self.note("Relative to E₀. Default −200 … −30 eV; end before the pre-edge features.")
                            .into_any_element(),
                    ),
                ]
                .into_iter()
                .flatten()
                .collect(),
                cx,
            ))
            .child(self.section(
                "Normalization",
                None,
                [
                    self.field(ParamKey::NormStart, cx),
                    self.field(ParamKey::NormEnd, cx),
                    self.field(ParamKey::NormPolyorder, cx),
                ]
                .into_iter()
                .flatten()
                .collect(),
                cx,
            ))
            .child(self.section(
                "Result",
                None,
                vec![
                    self.result_card(vec![
                        ("E₀ (max. derivative)".into(), fmt(e0, 1, " eV")),
                        ("Edge step".into(), fmt(step, 4, "")),
                        ("White line".into(), fmt(whiteline, 3, "")),
                    ])
                    .into_any_element(),
                ],
                cx,
            ))
    }

    fn background_inspector(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let sp = self.spectrum.as_deref();
        let knots = sp.and_then(|s| match s.background.as_ref() {
            Some(rexafs::prelude::BackgroundMethod::AUTOBK(a)) => a.nknots,
            _ => None,
        });
        div()
            .flex()
            .flex_col()
            .child(self.section(
                "AUTOBK",
                Some(ParamSection::Bkg),
                [
                    self.field(ParamKey::Rbkg, cx),
                    self.field(ParamKey::BkgKmin, cx),
                    self.field(ParamKey::BkgKmax, cx),
                    self.field(ParamKey::BkgKweight, cx),
                    self.field(ParamKey::BkgNknots, cx),
                ]
                .into_iter()
                .flatten()
                .collect(),
                cx,
            ))
            .child(self.section(
                "Clamps & window",
                None,
                [
                    self.field(ParamKey::BkgClampLo, cx),
                    self.field(ParamKey::BkgClampHi, cx),
                    Some(self.enum_row("window", EnumParam::BkgWindow, cx)),
                    self.field(ParamKey::BkgDk, cx),
                ]
                .into_iter()
                .flatten()
                .collect(),
                cx,
            ))
            .child(self.section(
                "Solver",
                None,
                [
                    Some(self.enum_row("method", EnumParam::BkgSolver, cx)),
                    self.field(ParamKey::BkgKstep, cx),
                    self.field(ParamKey::BkgNfft, cx),
                    Some(
                        self.result_card(vec![(
                            "Spline knots".into(),
                            knots.map(|k| k.to_string()).unwrap_or("auto".into()),
                        )])
                        .into_any_element(),
                    ),
                ]
                .into_iter()
                .flatten()
                .collect(),
                cx,
            ))
            .child(self.note("Rbkg: drag the shaded region edge on |χ(R)|. Rbkg too large removes the first shell; slightly small is harmless."))
    }

    fn transform_inspector(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let sp = self.spectrum.as_deref();
        let peak = sp.and_then(|s| {
            let r = s.r()?;
            let m = s.chir_mag()?;
            let n = r.len().min(m.len());
            (0..n)
                .filter(|&i| r[i] > 0.5)
                .max_by(|&a, &b| m[a].total_cmp(&m[b]))
                .map(|i| (r[i], m[i]))
        });
        let (kmin, kmax, _, _) = self.fft_summary();
        let p = self.ui_params();
        let rmin = p.bft_rmin.unwrap_or(1.0);
        let rmax = p.bft_rmax.unwrap_or(3.0);
        let nidp = 2.0 * (kmax - kmin) * (rmax - rmin) / std::f64::consts::PI;
        div()
            .flex()
            .flex_col()
            .child(self.section(
                "Forward FT  k → R",
                Some(ParamSection::Fft),
                [
                    self.field(ParamKey::FftKmin, cx),
                    self.field(ParamKey::FftKmax, cx),
                    self.field(ParamKey::FftDk, cx),
                    Some(self.enum_row("window", EnumParam::FftWindow, cx)),
                    self.field(ParamKey::FftKweight, cx),
                    self.field(ParamKey::FftRmax, cx),
                ]
                .into_iter()
                .flatten()
                .collect(),
                cx,
            ))
            .child(self.section(
                "Back FT  R → q",
                None,
                [
                    self.field(ParamKey::BftRmin, cx),
                    self.field(ParamKey::BftRmax, cx),
                    self.field(ParamKey::BftDr, cx),
                    Some(
                        self.note("Isolates a shell: χ(q) overlays k-weighted χ(k) when the shell is well separated.")
                            .into_any_element(),
                    ),
                ]
                .into_iter()
                .flatten()
                .collect(),
                cx,
            ))
            .child(self.section(
                "Advanced",
                None,
                [
                    self.field(ParamKey::FftDk2, cx),
                    self.field(ParamKey::FftKstep, cx),
                    self.field(ParamKey::FftNfft, cx),
                ]
                .into_iter()
                .flatten()
                .collect(),
                cx,
            ))
            .child(self.section(
                "Result",
                None,
                vec![
                    self.result_card(vec![
                        (
                            "First-shell peak".into(),
                            peak.map(|(r, _)| format!("{r:.2} Å")).unwrap_or("—".into()),
                        ),
                        (
                            "|χ(R)| max".into(),
                            peak.map(|(_, m)| format!("{m:.3}")).unwrap_or("—".into()),
                        ),
                        (
                            format!("N idp (k {kmin:.1}–{kmax:.1}, R {rmin:.1}–{rmax:.1})"),
                            format!("{nidp:.1}"),
                        ),
                    ])
                    .into_any_element(),
                ],
                cx,
            ))
    }
}
