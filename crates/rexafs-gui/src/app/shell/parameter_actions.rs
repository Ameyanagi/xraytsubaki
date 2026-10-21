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
    (energy_offset_ev, "Energy offset (eV)", "Import"),
    (alignment_record, "Alignment record", "Import"),
    (mback, "MBACK model and atomic reference", "Normalization"),
    (e0, "E₀ (eV)", "Edge"),
    (edge_step, "Edge step", "Edge"),
    (refit_prepared, "Refit prepared absorption", "Normalization"),
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
    (
        bkg_kweight_linked,
        "Background k-weight linked to FFT",
        "AUTOBK"
    ),
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
    (bkg_nfft, "Background FFT length", "Solver"),
    (bkg_ek0, "k origin E₀ (eV)", "AUTOBK"),
    (bkg_linear_regularization, "Legacy ridge", "Solver"),
    (bkg_linear_condition_limit, "Condition limit", "Solver"),
    (bkg_linear_residual_ratio_limit, "Residual limit", "Solver"),
    (bft_qmax, "q max (Å⁻¹)", "Back FT  R → q"),
    (bft_dr2, "Back FT dR2 (Å)", "Back FT  R → q"),
    (bft_rweight, "Back FT R-weight", "Back FT  R → q"),
    (bft_kstep, "q step (Å⁻¹)", "Back FT  R → q"),
    (bft_nfft, "Back FFT length", "Back FT  R → q"),
    (bkg_linear_fallback_to_lm, "Legacy fallback", "Solver"),
    (bkg_linear_workspace_cache, "Matrix cache", "Solver"),
    (bkg_linear_fallback_solver, "Fallback solver", "Solver"),
    (bkg_standard, "Standard χ(k)", "AUTOBK"),
    (fft_kmin, "FT k min (Å⁻¹)", "Forward FT  k → R"),
    (fft_kmax, "FT k max (Å⁻¹)", "Forward FT  k → R"),
    (fft_dk, "FT dk (Å⁻¹)", "Forward FT  k → R"),
    (fft_window, "FT window", "Forward FT  k → R"),
    (fft_kweight, "FT k-weight", "Forward FT  k → R"),
    (fft_rmax, "FT output R max (Å)", "Forward FT  k → R"),
    (bft_rmin, "Back FT R min (Å)", "Back FT  R → q"),
    (bft_rmax, "Back FT R max (Å)", "Back FT  R → q"),
    (bft_dr, "Back FT dR (Å)", "Back FT  R → q"),
    (bft_window, "Back FT window", "Back FT  R → q"),
    (fft_grid, "FT sampling grid", "Advanced"),
    (fft_dk2, "FT dk2 (Å⁻¹)", "Advanced"),
    (fft_kstep, "FT k step (Å⁻¹)", "Advanced"),
    (fft_nfft, "Forward FFT length", "Advanced"),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ParamScope {
    /// Every processing stage, excluding raw-input interpretation and calibration.
    All,
    Mapping,
    Stage(Stage),
    Section(&'static str),
    Field(&'static str),
}
impl ParamScope {
    fn contains(self, s: &Setting) -> bool {
        match self {
            Self::All => s.section != "Import",
            Self::Mapping => s.key == "import",
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
    /// Import comparisons/resets retain their scope, but bulk copying mapping
    /// always uses the explicit guarded action, never its calibration settings.
    fn copy_action(self) -> Self {
        match self {
            Self::Stage(Stage::Data) | Self::Section("Import") | Self::Field("import") => {
                Self::Mapping
            }
            _ => self,
        }
    }

    pub(crate) fn default_for_stage(stage: Stage) -> Option<Self> {
        match stage {
            Stage::Normalize | Stage::Background | Stage::Transform => Some(Self::Stage(stage)),
            _ => None,
        }
    }

    fn apply_label(self, count: usize) -> String {
        match self.copy_action() {
            Self::Mapping => format!("Copy column mapping to {count} (same detection mode only)"),
            scope => format!("Apply {} to {count}", scope.label()),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::All => "All processing settings",
            Self::Mapping => "Column mapping (same detection mode only)",
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
            Self::EnergyOffset => "energy_offset_ev",
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
            Self::BkgEk0 => "bkg_ek0",
            Self::BkgRegularization => "bkg_linear_regularization",
            Self::BkgCondition => "bkg_linear_condition_limit",
            Self::BkgResidualRatio => "bkg_linear_residual_ratio_limit",
            Self::BftQmax => "bft_qmax",
            Self::BftDr2 => "bft_dr2",
            Self::BftRweight => "bft_rweight",
            Self::BftKstep => "bft_kstep",
            Self::BftNfft => "bft_nfft",
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
            Self::FftGrid => "fft_grid",
            Self::BftWindow => "bft_window",
            Self::BkgFallback => "bkg_linear_fallback_to_lm",
            Self::BkgCache => "bkg_linear_workspace_cache",
            Self::BkgFallbackSolver => "bkg_linear_fallback_solver",
        }
    }
}

/// Values come from typed PipelineParams; only registered fields may be copied.
pub(crate) fn copy_scope(dst: &mut PipelineParams, src: &PipelineParams, scope: ParamScope) {
    if scope == ParamScope::Mapping && dst.import.mode != src.import.mode {
        return;
    }
    let mut out = serde_json::to_value(&*dst).expect("serializable processing settings");
    let source = serde_json::to_value(src).expect("serializable processing settings");
    let previous_offset = dst.energy_offset_ev;
    for setting in SETTINGS.iter().filter(|s| scope.contains(s)) {
        out[setting.key] = source[setting.key].clone();
    }
    *dst =
        serde_json::from_value(out).expect("typed processing settings remain valid after copying");
    if scope.contains(&Setting {
        key: "energy_offset_ev",
        label: "",
        section: "Import",
    }) {
        let next_offset = dst.energy_offset_ev;
        dst.energy_offset_ev = previous_offset;
        dst.set_energy_offset(next_offset)
            .expect("validated energy offset");
    }
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
/// Source-backed channels retain their detection mode as part of their identity.
fn reset_to_defaults(
    params: &mut PipelineParams,
    defaults: &PipelineParams,
    scope: ParamScope,
    preserve_mode: bool,
) {
    let mode = params.import.mode;
    let scope = if scope == ParamScope::Mapping {
        ParamScope::Field("import")
    } else {
        scope
    };
    copy_scope(params, defaults, scope);
    if preserve_mode {
        params.import.mode = mode;
    }
}

fn shown(v: &Value) -> String {
    match v {
        Value::Null => "Auto".into(),
        Value::String(s) => s.clone(),
        Value::Object(map) if map.contains_key("chi") && map.contains_key("k") => format!(
            "{} · {} points",
            map.get("label")
                .and_then(Value::as_str)
                .unwrap_or("Standard χ(k)"),
            map.get("k").and_then(Value::as_array).map_or(0, Vec::len)
        ),
        _ => v.to_string(),
    }
}

/// Shared recipient selection for action counts and execution. Skip reasons
/// are disjoint: a locked incompatible group is reported as locked.
#[derive(Default)]
pub(crate) struct CopyTargets {
    pub indices: Vec<usize>,
    pub locked: usize,
    incompatible: usize,
}

fn collect_copy_targets(
    marked: impl IntoIterator<Item = usize>,
    current: Option<usize>,
    valid: impl Fn(usize) -> bool,
    locked: impl Fn(usize) -> bool,
    compatible: impl Fn(usize) -> bool,
) -> CopyTargets {
    let mut targets = CopyTargets::default();
    for ix in marked {
        if Some(ix) == current || !valid(ix) {
            continue;
        }
        if locked(ix) {
            targets.locked += 1;
        } else if !compatible(ix) {
            targets.incompatible += 1;
        } else {
            targets.indices.push(ix);
        }
    }
    targets
}

impl CopyTargets {
    fn status(&self, scope: ParamScope, updated: usize) -> String {
        let mut status = format!("{} · {updated} updated", scope.label());
        if self.locked > 0 {
            status.push_str(&format!(" · {} locked skipped", self.locked));
        }
        if self.incompatible > 0 {
            status.push_str(&format!(
                " · {} different detection mode skipped",
                self.incompatible
            ));
        }
        status
    }
}

impl StudioApp {
    pub(crate) fn copy_targets(&self, scope: ParamScope) -> CopyTargets {
        let mapping = scope.copy_action() == ParamScope::Mapping;
        let mode = self.ui_params().import.mode;
        collect_copy_targets(
            self.selection.iter().copied(),
            self.selected,
            |ix| self.valid_group_index(ix),
            |ix| self.frozen.contains(&ix),
            |ix| !mapping || self.effective_params(ix).import.mode == mode,
        )
    }

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
                .text_size(px(11.))
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
        let scope = scope.copy_action();
        let source = self.ui_params().clone();
        let targets = self.copy_targets(scope);
        let mut changes = Vec::new();
        for &ix in &targets.indices {
            let before = self.custom_params(ix).cloned();
            let mut next = self.effective_params(ix).clone();
            copy_scope(&mut next, &source, scope);
            let after = (next != self.params).then_some(next);
            if before == after {
                continue;
            }
            self.set_custom_params(ix, after.clone());
            if let Some(id) = self.group_id(ix) {
                changes.push((id.clone(), before, after));
            }
        }
        let n = changes.len();
        if n > 0 {
            self.record(
                format!("{} → {n} marked spectra", scope.label()),
                Some(UndoOp::Params { changes }),
            );
        }
        self.status = targets.status(scope, n).into();
        self.param_menu = None;
        self.param_context_menu = None;
        self.sync_param_fields(cx);
        self.ensure_compare_loaded(cx);
        self.schedule_recompute(cx);
        self.invalidate_explore_plots(cx);
        cx.notify();
    }
    pub(crate) fn reset_scope(&mut self, scope: ParamScope, cx: &mut Context<Self>) {
        let defaults = if self.override_target().is_some() {
            self.params.clone()
        } else {
            PipelineParams::default()
        };
        let preserve_mode = self.override_target().is_some_and(|ix| {
            ix.checked_sub(crate::app::DERIVED_BASE)
                .and_then(|i| self.derived.get(i))
                .is_some_and(|group| group.source.is_some())
        });
        self.param_menu = None;
        self.param_context_menu = None;
        self.edit_parameters(format!("{} → default", scope.label()), cx, |params| {
            reset_to_defaults(params, &defaults, scope, preserve_mode);
            Ok(())
        });
    }
    pub(crate) fn parameter_context_overlay(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        let (scope, position) = self.param_context_menu?;
        let t = self.theme;
        let marked = self.copy_targets(scope).indices.len();
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
                            .w(px(if scope.copy_action() == ParamScope::Mapping {
                                360.
                            } else {
                                240.
                            }))
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
                            .child(row("context-apply", scope.apply_label(marked)).on_click(
                                cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.apply_scope_to_marked(scope, cx)
                                }),
                            ))
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
                                        " · Processing locked"
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
        let marked = self.copy_targets(scope).indices.len();
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
                                .when(scope.copy_action() == ParamScope::Mapping, |d| d.flex_col())
                                .gap_2()
                                .child(
                                    button(
                                        &t,
                                        "apply-parameter-scope",
                                        scope.apply_label(marked),
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
    fn energy_offset_copy_moves_explicit_origins_and_preserves_other_settings() {
        let mut source = PipelineParams::default();
        source.set_energy_offset(3.5).unwrap();
        let mut target = PipelineParams {
            energy_offset_ev: 1.0,
            e0: Some(8980.0),
            bkg_ek0: Some(8981.0),
            fft_kweight: Some(3.0),
            ..Default::default()
        };
        for _ in 0..2 {
            copy_scope(&mut target, &source, ParamScope::Field("energy_offset_ev"));
            assert_eq!(target.energy_offset_ev, 3.5);
            assert_eq!(target.e0, Some(8982.5));
            assert_eq!(target.bkg_ek0, Some(8983.5));
            assert_eq!(target.fft_kweight, Some(3.0));
        }
    }

    #[test]
    fn mapping_reset_restores_project_stage_defaults_in_both_group_stores() {
        use crate::app::{DERIVED_BASE, store_custom_params};
        use crate::params::{DerivedSpectrum, DetectionMode};
        use std::collections::BTreeMap;
        let defaults = populated_params(DetectionMode::Transmission);
        for scope in [
            ParamScope::Stage(Stage::Data),
            ParamScope::Stage(Stage::Normalize),
            ParamScope::Stage(Stage::Background),
            ParamScope::Stage(Stage::Transform),
            ParamScope::Section("Import"),
            ParamScope::Field("e0"),
            ParamScope::Mapping,
        ] {
            let mut before = PipelineParams::default();
            before.import.mode = DetectionMode::Reference;
            let mut after = before.clone();
            reset_to_defaults(&mut after, &defaults, scope, false);
            let old = serde_json::to_value(&before).unwrap();
            let new = serde_json::to_value(&after).unwrap();
            let project = serde_json::to_value(&defaults).unwrap();
            for setting in SETTINGS {
                assert_eq!(
                    new[setting.key],
                    if scope.contains(setting) {
                        &project[setting.key]
                    } else {
                        &old[setting.key]
                    }
                    .clone(),
                    "{scope:?}: {}",
                    setting.key
                );
            }
            let mut channel = DerivedSpectrum {
                source: Some("Ru_QAS.dat".into()),
                params: Some(before.clone()),
                ..Default::default()
            };
            let channel_params = channel.params.as_mut().unwrap();
            reset_to_defaults(channel_params, &defaults, scope, channel.source.is_some());
            let mut expected_channel = after.clone();
            expected_channel.import.mode = DetectionMode::Reference;
            assert!(
                channel.params.as_ref() == Some(&expected_channel),
                "{scope:?}"
            );
            assert_eq!(
                channel.params.as_ref().unwrap().import.mode,
                DetectionMode::Reference
            );
            for ix in [0, DERIVED_BASE] {
                let mut overrides = BTreeMap::new();
                let mut derived = vec![DerivedSpectrum::default()];
                store_custom_params(1, &mut overrides, &mut derived, ix, Some(after.clone()));
                let stored = if ix == 0 {
                    overrides.get(&ix)
                } else {
                    derived[0].params.as_ref()
                };
                assert!(stored == Some(&after));
                // Returning fully to project defaults clears either custom store.
                store_custom_params(1, &mut overrides, &mut derived, ix, None);
                assert!(overrides.is_empty());
                assert!(derived[0].params.is_none());
            }
        }
    }

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
    fn populated_params(mode: crate::params::DetectionMode) -> PipelineParams {
        use rexafs::prelude::{AUTOBKClampScalePolicy, AUTOBKSolver, FTWindow};
        PipelineParams {
            mback: Some(rexafs::MBack::for_edge("Cu", "K").options),
            import: crate::params::ImportConfig {
                axis: crate::import_mapping::AxisConversion::EnergyKev,
                reference_mu_col: Some(8),
                mode,
                energy_col: Some(1),
                i0_col: Some(2),
                it_col: Some(3),
                ir_col: Some(4),
                fluor_cols: Some(vec![5, 6]),
                mu_col: Some(7),
            },
            align_to_ref: true,
            align_target: Some(9000.),
            energy_offset_ev: 2.5,
            alignment_record: None,
            e0: Some(8979.),
            edge_step: Some(1.2),
            refit_prepared: true,
            pre_edge_start: Some(-150.),
            pre_edge_end: Some(-20.),
            n_victoreen: Some(1),
            norm_start: Some(100.),
            norm_end: Some(600.),
            norm_polyorder: Some(2),
            rbkg: Some(1.1),
            bkg_kmin: Some(1.),
            bkg_kmax: Some(12.),
            bkg_kweight: Some(2),
            bkg_kweight_linked: true,
            bkg_nknots: Some(7),
            bkg_clamp_lo: Some(1),
            bkg_clamp_hi: Some(2),
            bkg_nclamp: Some(4),
            bkg_clamp_lambda: Some(0.02),
            bkg_clamp_policy: AUTOBKClampScalePolicy::Fixed,
            bkg_window: Some(FTWindow::Hanning),
            bkg_dk: Some(1.),
            bkg_solver: Some(AUTOBKSolver::LinearDirect),
            bkg_kstep: Some(0.05),
            bkg_nfft: Some(2048),
            bkg_ek0: Some(8980.),
            bkg_linear_regularization: Some(0.01),
            bkg_linear_condition_limit: Some(1e8),
            bkg_linear_residual_ratio_limit: Some(1.1),
            bkg_linear_fallback_to_lm: Some(false),
            bkg_linear_workspace_cache: Some(true),
            bkg_linear_fallback_solver: Some(AUTOBKSolver::LegacyLm),
            bkg_standard: Some(crate::params::ChiStandard {
                label: "standard".into(),
                k: vec![0., 1., 2.],
                chi: vec![0., 0.1, 0.],
            }),
            fft_kmin: Some(2.),
            fft_kmax: Some(11.),
            fft_dk: Some(1.),
            fft_window: Some(FTWindow::Hanning),
            fft_grid: rexafs::FFTGrid::Larch,
            fft_kweight: Some(2.),
            fft_rmax: Some(6.),
            fft_dk2: Some(1.),
            fft_kstep: Some(0.05),
            fft_nfft: Some(2048),
            bft_rmin: Some(1.),
            bft_rmax: Some(3.),
            bft_dr: Some(0.5),
            bft_window: Some(FTWindow::Hanning),
            bft_qmax: Some(12.),
            bft_dr2: Some(0.5),
            bft_rweight: Some(1.),
            bft_kstep: Some(0.05),
            bft_nfft: Some(2048),
        }
    }

    #[test]
    fn all_processing_preserves_mapping_and_calibration_for_every_mode() {
        use crate::params::DetectionMode::*;
        let src = populated_params(Transmission);
        for mode in [Auto, Transmission, Fluorescence, Reference, MuColumn] {
            let mut dst = PipelineParams::default();
            dst.import.mode = mode;
            dst.import.fluor_cols = Some(vec![8, 9]);
            dst.align_target = Some(7112.);
            let before = serde_json::to_value(&dst).unwrap();
            let raw_before = dst.raw_fingerprint();
            copy_scope(&mut dst, &src, ParamScope::All);
            let after = serde_json::to_value(&dst).unwrap();
            let source = serde_json::to_value(&src).unwrap();
            for (key, value) in after.as_object().unwrap() {
                let expected = if matches!(
                    key.as_str(),
                    "import"
                        | "align_to_ref"
                        | "align_target"
                        | "energy_offset_ev"
                        | "alignment_record"
                ) {
                    &before[key]
                } else {
                    &source[key]
                };
                assert_eq!(value, expected, "{mode:?}: {key}");
            }
            assert_eq!(dst.raw_fingerprint(), raw_before);
        }
    }

    #[test]
    fn stage_copies_change_exactly_their_processing_keys() {
        use crate::params::DetectionMode::*;
        let stages: &[(Stage, &[&str])] = &[
            (
                Stage::Normalize,
                &[
                    "mback",
                    "e0",
                    "edge_step",
                    "refit_prepared",
                    "pre_edge_start",
                    "pre_edge_end",
                    "n_victoreen",
                    "norm_start",
                    "norm_end",
                    "norm_polyorder",
                ],
            ),
            (
                Stage::Background,
                &[
                    "rbkg",
                    "bkg_kmin",
                    "bkg_kmax",
                    "bkg_kweight",
                    "bkg_kweight_linked",
                    "bkg_nknots",
                    "bkg_clamp_lo",
                    "bkg_clamp_hi",
                    "bkg_nclamp",
                    "bkg_clamp_lambda",
                    "bkg_clamp_policy",
                    "bkg_window",
                    "bkg_dk",
                    "bkg_solver",
                    "bkg_kstep",
                    "bkg_nfft",
                    "bkg_ek0",
                    "bkg_linear_regularization",
                    "bkg_linear_condition_limit",
                    "bkg_linear_residual_ratio_limit",
                    "bkg_linear_fallback_to_lm",
                    "bkg_linear_workspace_cache",
                    "bkg_linear_fallback_solver",
                    "bkg_standard",
                ],
            ),
            (
                Stage::Transform,
                &[
                    "fft_kmin",
                    "fft_kmax",
                    "fft_dk",
                    "fft_window",
                    "fft_grid",
                    "fft_kweight",
                    "fft_rmax",
                    "fft_dk2",
                    "fft_kstep",
                    "fft_nfft",
                    "bft_rmin",
                    "bft_rmax",
                    "bft_dr",
                    "bft_window",
                    "bft_qmax",
                    "bft_dr2",
                    "bft_rweight",
                    "bft_kstep",
                    "bft_nfft",
                ],
            ),
        ];
        let populated = populated_params(Transmission);
        // Exercise both setting values and clearing back to Auto/defaults.
        for (src, initial) in [
            (populated.clone(), PipelineParams::default()),
            (PipelineParams::default(), populated),
        ] {
            for mode in [Transmission, Reference, Fluorescence] {
                for &(stage, keys) in stages {
                    let mut dst = initial.clone();
                    dst.import.mode = mode;
                    let before = serde_json::to_value(&dst).unwrap();
                    let source = serde_json::to_value(&src).unwrap();
                    let raw_before = dst.raw_fingerprint();
                    copy_scope(&mut dst, &src, ParamScope::Stage(stage));
                    let after = serde_json::to_value(&dst).unwrap();
                    for (key, value) in after.as_object().unwrap() {
                        let expected = if keys.contains(&key.as_str()) {
                            assert_ne!(before[key], source[key], "fixture must exercise {key}");
                            &source[key]
                        } else {
                            &before[key]
                        };
                        assert_eq!(value, expected, "{stage:?} / {mode:?}: {key}");
                    }
                    assert_eq!(dst.raw_fingerprint(), raw_before);
                }
            }
        }
    }

    #[test]
    fn mapping_copy_requires_equal_modes_and_copies_no_processing_or_calibration() {
        use crate::params::DetectionMode::*;
        for source_mode in [Auto, Transmission, Reference, Fluorescence, MuColumn] {
            let src = populated_params(source_mode);
            for target_mode in [Auto, Transmission, Reference, Fluorescence, MuColumn] {
                let mut dst = PipelineParams::default();
                dst.import.mode = target_mode;
                let mut expected = dst.clone();
                if source_mode == target_mode {
                    expected.import = src.import.clone();
                }
                copy_scope(&mut dst, &src, ParamScope::Mapping);
                assert_eq!(
                    serde_json::to_value(dst).unwrap(),
                    serde_json::to_value(expected).unwrap()
                );
            }
        }
    }

    #[test]
    fn import_menu_copies_use_the_explicit_mapping_action() {
        for scope in [
            ParamScope::Stage(Stage::Data),
            ParamScope::Section("Import"),
            ParamScope::Field("import"),
            ParamScope::Mapping,
        ] {
            assert_eq!(scope.copy_action(), ParamScope::Mapping);
            assert_eq!(
                scope.apply_label(2),
                "Copy column mapping to 2 (same detection mode only)"
            );
        }
        for stage in Stage::ALL {
            let expected = match stage {
                Stage::Normalize | Stage::Background | Stage::Transform => {
                    Some(ParamScope::Stage(stage))
                }
                _ => None,
            };
            assert_eq!(ParamScope::default_for_stage(stage), expected);
        }
        assert_eq!(
            ParamScope::Stage(Stage::Normalize).apply_label(3),
            "Apply Normalize to 3"
        );
        assert_eq!(ParamScope::All.copy_action(), ParamScope::All);
    }

    #[test]
    fn recipients_exclude_current_invalid_locked_and_incompatible_groups() {
        use crate::app::DERIVED_BASE;
        let marked = [
            0,
            1,
            2,
            3,
            99,
            DERIVED_BASE,
            DERIVED_BASE + 1,
            DERIVED_BASE + 2,
        ];
        let valid = |ix| ix < 4 || (DERIVED_BASE..DERIVED_BASE + 2).contains(&ix);
        let locked = |ix| [0, 2, 99, DERIVED_BASE + 2].contains(&ix);
        let compatible = |ix| ix != 3;
        let targets = collect_copy_targets(marked, Some(0), valid, locked, compatible);
        assert_eq!(targets.indices, [1, DERIVED_BASE, DERIVED_BASE + 1]);
        assert_eq!(targets.locked, 1);
        assert_eq!(targets.incompatible, 1);
        assert_eq!(
            targets.status(ParamScope::Mapping, 2),
            "Column mapping (same detection mode only) · 2 updated · 1 locked skipped · 1 different detection mode skipped"
        );
        let processing = collect_copy_targets(marked, Some(0), valid, locked, |_| true);
        assert_eq!(processing.indices, [1, 3, DERIVED_BASE, DERIVED_BASE + 1]);
        let empty = collect_copy_targets([0, 2, 99], Some(0), valid, locked, compatible);
        assert!(empty.indices.is_empty());
        assert_eq!(
            empty.status(ParamScope::All, 0),
            "All processing settings · 0 updated · 1 locked skipped"
        );
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
