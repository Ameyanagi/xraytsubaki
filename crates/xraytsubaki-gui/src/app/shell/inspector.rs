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
            Stage::Fit => self.fit_inspector(cx).into_any_element(),
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
                    .child(body),
            )
            .into_any_element()
    }

    fn inspector_header(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        if self.stage == Stage::Fit {
            return self.fit_inspector_header(cx).into_any_element();
        }
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
        header.into_any_element()
    }

    /// Copy the displayed parameter set onto every marked catalog group.
    fn apply_params_to_marked(&mut self, cx: &mut Context<Self>) {
        let params = self.ui_params().clone();
        let targets: Vec<usize> = self
            .selection
            .iter()
            .copied()
            .filter(|&ix| ix < DERIVED_BASE)
            .collect();
        for ix in targets {
            if params == self.params {
                self.overrides.remove(&ix);
            } else {
                self.overrides.insert(ix, params.clone());
            }
        }
        self.status = "parameters applied to marked groups".into();
        self.ensure_compare_loaded(cx);
        self.schedule_recompute(cx);
        cx.notify();
    }

    /// Drop the current group's override (or reset the globals to defaults).
    fn reset_params(&mut self, cx: &mut Context<Self>) {
        match self.override_target() {
            Some(ix) => {
                self.overrides.remove(&ix);
            }
            None => {
                self.params = crate::params::PipelineParams::default();
            }
        }
        self.sync_param_fields(cx);
        self.schedule_recompute(cx);
        self.sync_handles(cx);
        cx.notify();
    }

    pub(crate) fn field(&self, key: ParamKey) -> Option<gpui::AnyElement> {
        self.param_fields
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, f)| f.clone().into_any_element())
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
                sp.and_then(|s| s.get_e0())
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
        let e0 = sp.and_then(|s| s.get_e0());
        let step = self.edge_step();
        let whiteline = sp
            .and_then(|s| s.get_norm())
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
                vec![self.field(ParamKey::E0)].into_iter().flatten().collect(),
                cx,
            ))
            .child(self.section(
                "Pre-edge line",
                None,
                [
                    self.field(ParamKey::PreEdgeStart),
                    self.field(ParamKey::PreEdgeEnd),
                    self.field(ParamKey::NVictoreen),
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
                    self.field(ParamKey::NormStart),
                    self.field(ParamKey::NormEnd),
                    self.field(ParamKey::NormPolyorder),
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
            Some(xraytsubaki::prelude::BackgroundMethod::AUTOBK(a)) => a.nknots,
            _ => None,
        });
        div()
            .flex()
            .flex_col()
            .child(self.section(
                "AUTOBK",
                Some(ParamSection::Bkg),
                [
                    self.field(ParamKey::Rbkg),
                    self.field(ParamKey::BkgKmin),
                    self.field(ParamKey::BkgKmax),
                    self.field(ParamKey::BkgKweight),
                    self.field(ParamKey::BkgNknots),
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
                    self.field(ParamKey::BkgClampLo),
                    self.field(ParamKey::BkgClampHi),
                    Some(self.enum_row("window", EnumParam::BkgWindow, cx)),
                    self.field(ParamKey::BkgDk),
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
                    self.field(ParamKey::BkgKstep),
                    self.field(ParamKey::BkgNfft),
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
            let r = s.get_r()?;
            let m = s.get_chir_mag()?;
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
                    self.field(ParamKey::FftKmin),
                    self.field(ParamKey::FftKmax),
                    self.field(ParamKey::FftDk),
                    Some(self.enum_row("window", EnumParam::FftWindow, cx)),
                    self.field(ParamKey::FftKweight),
                    self.field(ParamKey::FftRmax),
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
                    self.field(ParamKey::BftRmin),
                    self.field(ParamKey::BftRmax),
                    self.field(ParamKey::BftDr),
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
                    self.field(ParamKey::FftDk2),
                    self.field(ParamKey::FftKstep),
                    self.field(ParamKey::FftNfft),
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
