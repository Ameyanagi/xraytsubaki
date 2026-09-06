//! Scoped copying and comparison of requested processing settings.
use super::{PlotScope, Stage, button, journal::UndoOp};
use crate::{
    app::{EnumParam, ParamKey, StudioApp},
    params::PipelineParams,
};
use gpui::{
    ClickEvent, Context, IntoElement, ParentElement, SharedString, Styled, div, prelude::*, px,
};
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Setting {
    pub key: &'static str,
    pub label: &'static str,
    pub section: &'static str,
}
macro_rules! settings { ($(($key:ident, $label:literal, $section:literal)),* $(,)?) => {
    pub(crate) const SETTINGS: &[Setting] = &[$(Setting {key: stringify!($key), label: $label, section: $section}),*];
}; }
settings![
    (import, "Import", "Import"),
    (align_to_ref, "Reference alignment", "Import"),
    (align_target, "Alignment energy (eV)", "Import"),
    (e0, "E₀ (eV)", "Edge"),
    (edge_step, "Edge step", "Edge"),
    (pre_edge_start, "Pre-edge start (eV)", "Pre-edge line"),
    (pre_edge_end, "Pre-edge end (eV)", "Pre-edge line"),
    (n_victoreen, "Victoreen n", "Pre-edge line"),
    (norm_start, "Normalization start (eV)", "Normalization"),
    (norm_end, "Normalization end (eV)", "Normalization"),
    (norm_polyorder, "Polynomial order", "Normalization"),
    (rbkg, "Rbkg (Å)", "AUTOBK"),
    (bkg_kmin, "Background k min (Å⁻¹)", "AUTOBK"),
    (bkg_kmax, "Background k max (Å⁻¹)", "AUTOBK"),
    (bkg_kweight, "Background k-weight", "AUTOBK"),
    (bkg_nknots, "Spline knots", "AUTOBK"),
    (bkg_clamp_lo, "Clamp low", "Clamps & window"),
    (bkg_clamp_hi, "Clamp high", "Clamps & window"),
    (bkg_nclamp, "Clamp points", "Clamps & window"),
    (bkg_clamp_lambda, "Clamp λ", "Clamps & window"),
    (bkg_clamp_policy, "Clamp model", "Clamps & window"),
    (bkg_window, "Background window", "Clamps & window"),
    (bkg_dk, "Background dk (Å⁻¹)", "Clamps & window"),
    (bkg_solver, "Background solver", "Solver"),
    (bkg_kstep, "Background k step (Å⁻¹)", "Solver"),
    (bkg_nfft, "Background NFFT", "Solver"),
    (fft_kmin, "FT k min (Å⁻¹)", "Forward FT  k → R"),
    (fft_kmax, "FT k max (Å⁻¹)", "Forward FT  k → R"),
    (fft_dk, "FT dk (Å⁻¹)", "Forward FT  k → R"),
    (fft_window, "FT window", "Forward FT  k → R"),
    (fft_kweight, "FT k-weight", "Forward FT  k → R"),
    (fft_rmax, "FT R max (Å)", "Forward FT  k → R"),
    (bft_rmin, "Back FT R min (Å)", "Back FT  R → q"),
    (bft_rmax, "Back FT R max (Å)", "Back FT  R → q"),
    (bft_dr, "Back FT dR (Å)", "Back FT  R → q"),
    (bft_window, "Back FT window", "Back FT  R → q"),
    (fft_dk2, "FT dk2 (Å⁻¹)", "Advanced"),
    (fft_kstep, "FT k step (Å⁻¹)", "Advanced"),
    (fft_nfft, "FT NFFT", "Advanced"),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ParamScope {
    All,
    Stage(Stage),
    Section(&'static str),
    Field(&'static str),
}
impl ParamScope {
    fn contains(self, s: &Setting) -> bool {
        match self {
            Self::All => true,
            Self::Field(key) => s.key == key,
            Self::Section(section) => s.section == section,
            Self::Stage(Stage::Data) => s.section == "Import",
            Self::Stage(Stage::Normalize) => {
                matches!(s.section, "Edge" | "Pre-edge line" | "Normalization")
            }
            Self::Stage(Stage::Background) => {
                matches!(s.section, "AUTOBK" | "Clamps & window" | "Solver")
            }
            Self::Stage(Stage::Transform) => matches!(
                s.section,
                "Forward FT  k → R" | "Back FT  R → q" | "Advanced"
            ),
            Self::Stage(_) => false,
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::All => "All processing settings",
            Self::Stage(s) => s.name(),
            Self::Section(s) => s,
            Self::Field(k) => SETTINGS
                .iter()
                .find(|s| s.key == k)
                .map(|s| s.label)
                .unwrap_or(k),
        }
    }
}
impl ParamKey {
    pub(crate) fn setting_key(self) -> &'static str {
        match self {
            Self::ImpEnergyCol | Self::ImpI0Col | Self::ImpItCol | Self::ImpIrCol => "import",
            Self::AlignTarget => "align_target",
            Self::E0 => "e0",
            Self::EdgeStep => "edge_step",
            Self::PreEdgeStart => "pre_edge_start",
            Self::PreEdgeEnd => "pre_edge_end",
            Self::NormStart => "norm_start",
            Self::NormEnd => "norm_end",
            Self::NormPolyorder => "norm_polyorder",
            Self::NVictoreen => "n_victoreen",
            Self::Rbkg => "rbkg",
            Self::BkgKmin => "bkg_kmin",
            Self::BkgKmax => "bkg_kmax",
            Self::BkgKstep => "bkg_kstep",
            Self::BkgNknots => "bkg_nknots",
            Self::BkgKweight => "bkg_kweight",
            Self::BkgClampLo => "bkg_clamp_lo",
            Self::BkgClampHi => "bkg_clamp_hi",
            Self::BkgNclamp => "bkg_nclamp",
            Self::BkgClampLambda => "bkg_clamp_lambda",
            Self::BkgDk => "bkg_dk",
            Self::BkgNfft => "bkg_nfft",
            Self::FftKmin => "fft_kmin",
            Self::FftKmax => "fft_kmax",
            Self::FftDk => "fft_dk",
            Self::FftKweight => "fft_kweight",
            Self::FftDk2 => "fft_dk2",
            Self::FftRmax => "fft_rmax",
            Self::FftKstep => "fft_kstep",
            Self::FftNfft => "fft_nfft",
            Self::BftRmin => "bft_rmin",
            Self::BftRmax => "bft_rmax",
            Self::BftDr => "bft_dr",
        }
    }
}
impl EnumParam {
    pub(crate) fn setting_key(self) -> &'static str {
        match self {
            Self::ImportMode => "import",
            Self::BkgWindow => "bkg_window",
            Self::BkgSolver => "bkg_solver",
            Self::BkgClampPolicy => "bkg_clamp_policy",
            Self::FftWindow => "fft_window",
            Self::BftWindow => "bft_window",
        }
    }
}

/// Values come from typed PipelineParams; only registered fields may be copied.
pub(crate) fn copy_scope(dst: &mut PipelineParams, src: &PipelineParams, scope: ParamScope) {
    let mut out = serde_json::to_value(&*dst).expect("serializable processing settings");
    let source = serde_json::to_value(src).expect("serializable processing settings");
    for setting in SETTINGS.iter().filter(|s| scope.contains(s)) {
        out[setting.key] = source[setting.key].clone();
    }
    *dst =
        serde_json::from_value(out).expect("typed processing settings remain valid after copying");
    // Solver and clamp-model selections are coupled in the inspector. Scoped
    // copies must maintain the same valid pairing as an explicit selection.
    let copies = |key| SETTINGS.iter().any(|s| s.key == key && scope.contains(s));
    use rexafs::prelude::{AUTOBKClampScalePolicy, AUTOBKSolver};
    if dst.bkg_clamp_policy == AUTOBKClampScalePolicy::FixedPenalty
        && matches!(
            dst.bkg_solver,
            Some(AUTOBKSolver::LegacyLm | AUTOBKSolver::TrustRegionDogLeg)
        )
    {
        if copies("bkg_clamp_policy") {
            dst.bkg_solver = Some(AUTOBKSolver::LinearDirect);
        } else if copies("bkg_solver") {
            dst.bkg_clamp_policy = AUTOBKClampScalePolicy::Fixed;
        }
    }
}
fn shown(v: &Value) -> String {
    match v {
        Value::Null => "Auto".into(),
        Value::String(s) => s.clone(),
        _ => v.to_string(),
    }
}

impl StudioApp {
    fn comparison_indices(&self) -> Vec<usize> {
        let mut indices = self.selection.clone();
        indices.extend(self.selected);
        indices
            .into_iter()
            .filter(|&ix| self.valid_group_index(ix))
            .collect()
    }
    pub(crate) fn differing_settings(&self, scope: ParamScope) -> Vec<Setting> {
        let indices = self.comparison_indices();
        if indices.len() < 2 {
            return Vec::new();
        }
        let values: Vec<Value> = indices
            .iter()
            .map(|&ix| serde_json::to_value(self.effective_params(ix)).unwrap())
            .collect();
        SETTINGS
            .iter()
            .filter(|s| {
                scope.contains(s) && values[1..].iter().any(|v| v[s.key] != values[0][s.key])
            })
            .copied()
            .collect()
    }
    pub(crate) fn parameter_badge(
        &self,
        scope: ParamScope,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        if self.stage_view.scope != PlotScope::Marked || self.differing_settings(scope).is_empty() {
            return None;
        }
        let t = self.theme;
        Some(
            div()
                .id(SharedString::from(format!("mixed-{scope:?}")))
                .px_1()
                .text_size(px(10.))
                .text_color(t.warn)
                .cursor_pointer()
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.param_menu = Some(scope);
                    cx.notify();
                }))
                .child("Mixed")
                .into_any_element(),
        )
    }
    pub(crate) fn parameter_context(
        &self,
        scope: ParamScope,
        body: gpui::AnyElement,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        div()
            .id(SharedString::from(format!("param-context-{scope:?}")))
            .flex()
            .items_center()
            .on_mouse_down(
                gpui::MouseButton::Right,
                cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    this.param_context_menu = Some((scope, event.position));
                    cx.notify();
                }),
            )
            .child(div().flex_1().min_w_0().child(body))
            .into_any_element()
    }
    pub(crate) fn apply_scope_to_marked(&mut self, scope: ParamScope, cx: &mut Context<Self>) {
        let source = self.ui_params().clone();
        let targets: Vec<_> = self
            .selection
            .iter()
            .copied()
            .filter(|&ix| {
                self.valid_group_index(ix)
                    && Some(ix) != self.selected
                    && !self.frozen.contains(&ix)
            })
            .collect();
        let skipped = self
            .selection
            .iter()
            .filter(|ix| self.frozen.contains(ix) && Some(**ix) != self.selected)
            .count();
        let mut changes = Vec::new();
        for ix in targets {
            let before = self.custom_params(ix).cloned();
            let mut next = self.effective_params(ix).clone();
            copy_scope(&mut next, &source, scope);
            let after = (next != self.params).then_some(next);
            if before == after {
                continue;
            }
            self.set_custom_params(ix, after.clone());
            changes.push((ix, before, after));
        }
        let n = changes.len();
        if n > 0 {
            self.record(
                format!("{} → {n} marked spectra", scope.label()),
                Some(UndoOp::Params { changes }),
            );
        }
        self.status = format!(
            "{} · {n} updated{}",
            scope.label(),
            if skipped > 0 {
                format!(" · {skipped} frozen skipped")
            } else {
                String::new()
            }
        )
        .into();
        self.param_menu = None;
        self.param_context_menu = None;
        self.sync_param_fields(cx);
        self.ensure_compare_loaded(cx);
        self.schedule_recompute(cx);
        self.invalidate_explore_plots(cx);
        cx.notify();
    }
    fn reset_scope(&mut self, scope: ParamScope, cx: &mut Context<Self>) {
        let target = self.override_target();
        if target.is_some_and(|ix| self.frozen.contains(&ix)) {
            self.status = "Thaw this spectrum to edit it".into();
            cx.notify();
            return;
        }
        let before = self.ui_params().clone();
        copy_scope(self.edit_params(), &PipelineParams::default(), scope);
        let after = self.ui_params().clone();
        self.record_param_edit(
            target,
            None,
            before,
            after,
            format!("{} → default", scope.label()),
        );
        self.param_menu = None;
        self.param_context_menu = None;
        self.sync_param_fields(cx);
        self.schedule_recompute(cx);
        self.sync_handles(cx);
        cx.notify();
    }
    pub(crate) fn parameter_context_overlay(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        let (scope, position) = self.param_context_menu?;
        let t = self.theme;
        let marked = self
            .selection
            .iter()
            .filter(|&&ix| {
                self.valid_group_index(ix)
                    && Some(ix) != self.selected
                    && !self.frozen.contains(&ix)
            })
            .count();
        let row = |id: &'static str, label: String| {
            div()
                .id(id)
                .h(px(28.))
                .px_3()
                .flex()
                .items_center()
                .rounded_sm()
                .text_size(px(12.))
                .text_color(t.text)
                .cursor_pointer()
                .hover(|d| d.bg(t.raised))
                .child(label)
        };
        Some(
            div()
                .id("parameter-context-dismiss")
                .absolute()
                .inset_0()
                .on_mouse_down(
                    gpui::MouseButton::Left,
                    cx.listener(|this, _: &gpui::MouseDownEvent, _, cx| {
                        this.param_context_menu = None;
                        cx.notify();
                    }),
                )
                .on_mouse_down(
                    gpui::MouseButton::Right,
                    cx.listener(|this, _: &gpui::MouseDownEvent, _, cx| {
                        this.param_context_menu = None;
                        cx.notify();
                    }),
                )
                .child(
                    gpui::anchored().position(position).snap_to_window().child(
                        div()
                            .id("parameter-context-popup")
                            .w(px(210.))
                            .p_1()
                            .rounded_md()
                            .bg(t.surface)
                            .border_1()
                            .border_color(t.border)
                            .shadow_lg()
                            .flex()
                            .flex_col()
                            .on_mouse_down(
                                gpui::MouseButton::Left,
                                cx.listener(|_, _: &gpui::MouseDownEvent, _, cx| {
                                    cx.stop_propagation()
                                }),
                            )
                            .child(
                                row("context-apply", format!("Apply to marked ({marked})"))
                                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                        this.apply_scope_to_marked(scope, cx)
                                    })),
                            )
                            .child(row("context-reset", "Reset to default".into()).on_click(
                                cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.reset_scope(scope, cx)
                                }),
                            ))
                            .child(div().h(px(1.)).my_1().bg(t.border))
                            .child(row("context-compare", "Compare marked…".into()).on_click(
                                cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.param_context_menu = None;
                                    this.param_menu = Some(scope);
                                    cx.notify();
                                }),
                            )),
                    ),
                )
                .into_any_element(),
        )
    }
    pub(crate) fn parameter_menu_overlay(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        let scope = self.param_menu?;
        let t = self.theme;
        let source = serde_json::to_value(self.ui_params()).unwrap();
        let indices = self.comparison_indices();
        let different = self.differing_settings(scope);
        let fields: Vec<_> = match scope {
            ParamScope::Field(_) => SETTINGS
                .iter()
                .filter(|s| scope.contains(s))
                .copied()
                .collect(),
            _ => different,
        };
        let mut list = div()
            .id("parameter-differences")
            .max_h(px(360.))
            .overflow_y_scroll()
            .flex()
            .flex_col();
        for setting in fields {
            list = list.child(
                div()
                    .px_3()
                    .pt_2()
                    .text_size(px(12.))
                    .text_color(t.text)
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(setting.label),
            );
            for &ix in &indices {
                let value = serde_json::to_value(self.effective_params(ix)).unwrap();
                let different = value[setting.key] != source[setting.key];
                let current = Some(ix) == self.selected;
                list = list.child(
                    div()
                        .px_3()
                        .py_1()
                        .flex()
                        .gap_3()
                        .text_size(px(12.))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_ellipsis()
                                .overflow_hidden()
                                .child(format!(
                                    "{}{}{}",
                                    self.entry_label(ix),
                                    if current { " · current" } else { "" },
                                    if self.frozen.contains(&ix) {
                                        " · frozen"
                                    } else {
                                        ""
                                    }
                                )),
                        )
                        .child(
                            div()
                                .font_family(super::MONO)
                                .text_color(if different { t.warn } else { t.text_muted })
                                .child(shown(&value[setting.key])),
                        ),
                );
            }
        }
        let marked = self
            .selection
            .iter()
            .filter(|&&ix| {
                self.valid_group_index(ix)
                    && Some(ix) != self.selected
                    && !self.frozen.contains(&ix)
            })
            .count();
        Some(
            div()
                .id("parameter-menu-overlay")
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(gpui::Rgba {
                    r: 0.,
                    g: 0.,
                    b: 0.,
                    a: 0.3,
                })
                .on_mouse_down(
                    gpui::MouseButton::Left,
                    cx.listener(|this, _: &gpui::MouseDownEvent, _, cx| {
                        this.param_menu = None;
                        cx.notify();
                    }),
                )
                .child(
                    div()
                        .id("parameter-menu")
                        .w(px(470.))
                        .rounded_lg()
                        .bg(t.surface)
                        .border_1()
                        .border_color(t.border)
                        .shadow_lg()
                        .flex()
                        .flex_col()
                        .on_mouse_down(
                            gpui::MouseButton::Left,
                            cx.listener(|_, _: &gpui::MouseDownEvent, _, cx| cx.stop_propagation()),
                        )
                        .child(
                            div()
                                .px_3()
                                .py_2()
                                .flex()
                                .items_center()
                                .child(
                                    div()
                                        .flex_1()
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .child(scope.label()),
                                )
                                .child(button(&t, "close-parameter-menu", "×", false).on_click(
                                    cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.param_menu = None;
                                        cx.notify();
                                    }),
                                )),
                        )
                        .child(
                            div()
                                .px_3()
                                .pb_2()
                                .text_size(px(11.))
                                .text_color(t.text_muted)
                                .child(format!("Current: {}", self.current_group_label())),
                        )
                        .child(list)
                        .child(
                            div()
                                .px_3()
                                .py_3()
                                .flex()
                                .gap_2()
                                .child(
                                    button(
                                        &t,
                                        "apply-parameter-scope",
                                        format!("Apply current to marked ({marked})"),
                                        true,
                                    )
                                    .on_click(cx.listener(
                                        move |this, _: &ClickEvent, _, cx| {
                                            this.apply_scope_to_marked(scope, cx)
                                        },
                                    )),
                                )
                                .child(
                                    button(&t, "reset-parameter-scope", "Reset to default", false)
                                        .on_click(cx.listener(
                                            move |this, _: &ClickEvent, _, cx| {
                                                this.reset_scope(scope, cx)
                                            },
                                        )),
                                ),
                        ),
                )
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scoped_copy_preserves_independent_ranges_and_weights() {
        let src = PipelineParams {
            fft_kweight: Some(1.),
            fft_kmin: Some(2.),
            rbkg: Some(1.),
            e0: Some(8979.),
            ..Default::default()
        };
        let mut dst = PipelineParams {
            fft_kweight: Some(3.),
            fft_kmin: Some(4.),
            rbkg: Some(1.4),
            e0: Some(8981.),
            ..Default::default()
        };
        copy_scope(&mut dst, &src, ParamScope::Field("fft_kweight"));
        assert_eq!(dst.fft_kweight, Some(1.));
        assert_eq!(dst.fft_kmin, Some(4.));
        assert_eq!(dst.rbkg, Some(1.4));
        copy_scope(&mut dst, &src, ParamScope::Section("Forward FT  k → R"));
        assert_eq!(dst.fft_kmin, Some(2.));
        assert_eq!(dst.e0, Some(8981.));
        copy_scope(
            &mut dst,
            &PipelineParams::default(),
            ParamScope::Stage(Stage::Transform),
        );
        assert_eq!(dst.fft_kweight, None);
        assert_eq!(dst.rbkg, Some(1.4));
    }
    #[test]
    fn scoped_copy_keeps_fixed_lambda_and_solver_compatible() {
        use rexafs::prelude::{AUTOBKClampScalePolicy, AUTOBKSolver};
        let legacy = PipelineParams {
            bkg_solver: Some(AUTOBKSolver::LegacyLm),
            ..PipelineParams::legacy_defaults()
        };
        let mut current = PipelineParams::default();
        copy_scope(&mut current, &legacy, ParamScope::Field("bkg_solver"));
        assert_eq!(current.bkg_solver, legacy.bkg_solver);
        assert_eq!(current.bkg_clamp_policy, AUTOBKClampScalePolicy::Fixed);
        copy_scope(
            &mut current,
            &PipelineParams::default(),
            ParamScope::Field("bkg_clamp_policy"),
        );
        assert_eq!(current.bkg_solver, Some(AUTOBKSolver::LinearDirect));
        assert_eq!(
            current.bkg_clamp_policy,
            AUTOBKClampScalePolicy::FixedPenalty
        );
        let source = PipelineParams {
            bkg_clamp_lambda: Some(0.02),
            ..PipelineParams::default()
        };
        copy_scope(&mut current, &source, ParamScope::Field("bkg_clamp_lambda"));
        assert_eq!(current.bkg_clamp_lambda, Some(0.02));
    }
    #[test]
    fn settings_registry_covers_every_persisted_parameter_once() {
        let value = serde_json::to_value(PipelineParams::default()).unwrap();
        let keys: std::collections::BTreeSet<_> = SETTINGS.iter().map(|s| s.key).collect();
        assert_eq!(keys.len(), SETTINGS.len());
        assert_eq!(
            keys,
            value
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect()
        );
    }
}
