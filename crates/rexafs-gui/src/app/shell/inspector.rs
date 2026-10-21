//! Inspector: the right panel. A sticky header states the edit scope
//! ("Editing <group>") and offers Apply-to-marked / Reset; below it the
//! stage's parameter sections in pipeline order, then a Result card.

use gpui::{
    ClickEvent, Context, IntoElement, ParentElement, SharedString, Styled, div, prelude::*, px,
};

use super::{
    MONO, Stage, button,
    controls::{Tooltip, icon, icon_button},
    parameter_actions::ParamScope,
    section_label,
};
use crate::app::{EnumParam, ParamKey, ParamSection, StudioApp};
use crate::icons::Icon;
use crate::params::Quantity;

fn apply_hint(marked: usize, locked: usize) -> Option<String> {
    if marked == 0 && locked == 0 {
        None
    } else if locked == 0 {
        Some("excludes current".into())
    } else {
        Some(format!("excludes current · {locked} locked"))
    }
}

impl StudioApp {
    fn advanced_section_changed(&self, title: &str) -> bool {
        let p = self.ui_params();
        match title {
            "Background options" => {
                p.bkg_nknots.is_some() || p.bkg_ek0.is_some() || p.bkg_standard.is_some()
            }
            "Back FT  R → q" => {
                p.bft_rmin.is_some()
                    || p.bft_rmax.is_some()
                    || p.bft_dr.is_some()
                    || p.bft_dr2.is_some()
                    || p.bft_rweight.is_some()
                    || p.bft_qmax.is_some()
                    || p.bft_kstep.is_some()
                    || p.bft_nfft.is_some()
                    || p.bft_window.is_some()
            }
            "Advanced" => {
                p.fft_grid != rexafs::FFTGrid::Input
                    || p.fft_dk2.is_some()
                    || p.fft_kstep.is_some()
                    || p.fft_nfft.is_some()
            }
            "Clamps & window" => {
                p.bkg_clamp_lo.is_some()
                    || p.bkg_clamp_hi.is_some()
                    || p.bkg_nclamp.is_some()
                    || p.bkg_window.is_some()
                    || p.bkg_dk.is_some()
                    || p.bkg_clamp_policy
                        != crate::params::PipelineParams::default().bkg_clamp_policy
            }
            "Solver" => {
                p.bkg_solver.is_some()
                    || p.bkg_kstep.is_some()
                    || p.bkg_nfft.is_some()
                    || p.bkg_linear_condition_limit.is_some()
                    || p.bkg_linear_regularization.is_some()
                    || p.bkg_linear_residual_ratio_limit.is_some()
                    || p.bkg_linear_fallback_to_lm.is_some()
            }
            _ => false,
        }
    }
    pub(crate) fn inspector(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        if self.stage == Stage::Transform {
            self.ensure_wavelet_fields(cx);
        }
        if self.stage == Stage::Normalize {
            self.ensure_normalization_editor(cx);
        }
        let body = match self.stage {
            Stage::Data if self.fluorescence.open => self.fluorescence_inspector(cx),
            Stage::Data if self.peaks.open => self.peak_inspector(cx),
            Stage::Data => self.data_inspector(cx).into_any_element(),
            Stage::Normalize => self.normalize_inspector(cx).into_any_element(),
            Stage::Background => self.background_inspector(cx).into_any_element(),
            Stage::Transform => self.transform_inspector(cx).into_any_element(),
            Stage::Series => self.series_inspector(cx).into_any_element(),
            Stage::Fit | Stage::Publish => div().into_any_element(),
        };
        let body = if self.stage.is_processing()
            && let Some(group) = self
                .selected
                .filter(|&ix| ix >= crate::app::DERIVED_BASE)
                .and_then(|ix| self.derived.get(ix - crate::app::DERIVED_BASE))
        {
            // Only legacy groups whose stored quantity is unknown get the
            // confirmation prompt; a blocked quantity (Δμnorm) gets the plain
            // notice; every other derived group renders the normal body.
            let blocked = group.processing_block_reason().or_else(|| {
                (!group.corrections.is_empty()
                    && matches!(self.stage, Stage::Background | Stage::Transform))
                .then(|| crate::fluorescence_history::XANES_ONLY.into())
            });
            if !group.quantity_unconfirmed && (blocked.is_none() || self.stage == Stage::Data) {
                body
            } else {
                let mut notice = div()
                    .p_3()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(group.display_label());
                if let Some(reason) = &blocked {
                    notice = notice.child(reason.clone());
                }
                if group.quantity_unconfirmed {
                    notice = notice.child("Confirm what the stored arrays represent:");
                    for quantity in [
                        Quantity::RawMu,
                        Quantity::NormalizedMu,
                        Quantity::FlattenedMu,
                        Quantity::FlattenedDifference,
                        Quantity::NormalizedDifference,
                        Quantity::ChiK,
                    ] {
                        notice = notice.child(
                            button(&t, quantity.label(), quantity.label(), false).on_click(
                                cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.confirm_current_quantity(quantity, cx);
                                }),
                            ),
                        );
                    }
                }
                notice.into_any_element()
            }
        } else {
            body
        };
        div()
            .w(px(super::assistant_shell::PARAMETERS_WIDTH))
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
            .children(
                self.selected
                    .and_then(|g| g.checked_sub(crate::app::DERIVED_BASE))
                    .and_then(|i| self.derived.get(i))
                    .and_then(|d| {
                        crate::app::group_rows::input_missing(d, |id| {
                            self.group_registry.is_excluded(id)
                        })
                        .or_else(|| self.inputs_changed(d))
                    })
                    .map(|message| div().p_3().text_size(px(12.)).child(message)),
            )
            .child(
                crate::accessibility::scroll(
                    div().id("inspector-scroll"),
                    "Parameters",
                    &self.inspector_scroll,
                )
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

    fn confirm_current_quantity(&mut self, quantity: Quantity, cx: &mut Context<Self>) {
        if self.refuse_frozen_edit(cx) {
            return;
        }
        let Some(index) = self
            .selected
            .and_then(|ix| ix.checked_sub(crate::app::DERIVED_BASE))
        else {
            return;
        };
        let Some(group) = self.derived.get_mut(index) else {
            return;
        };
        if self.journal.confirm_quantity(index, group, quantity) {
            self.reprocess_current(cx);
            cx.notify();
        }
    }

    fn inspector_header(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        if self.stage == Stage::Series {
            return self.series_inspector_header(cx).into_any_element();
        }
        let t = self.theme;
        let label = self.current_group_label();
        let copy_scope = ParamScope::default_for_stage(self.stage);
        let targets = self.copy_targets(ParamScope::Stage(self.stage));
        let marked = targets.indices.len();
        let hint = copy_scope.and_then(|_| apply_hint(marked, targets.locked));
        // The group name keeps its own row so a long name is never squeezed by
        // the Apply button; Apply and its hint sit on a second row.
        let mut name_row = div().flex().items_center().gap_2().child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .text_size(px(11.5))
                .text_color(t.text)
                .font_weight(gpui::FontWeight::MEDIUM)
                .child(label),
        );
        if self.stage.is_processing() {
            name_row = name_row.child(
                icon_button(
                    &t,
                    "reset-params",
                    Icon::Undo,
                    format!("Reset {} for the current group", self.stage.name()),
                    false,
                )
                .on_click(cx.listener(|app, _, _, cx| app.reset_params(cx))),
            );
        }
        let apply_row =
            (self.stage.is_processing() && copy_scope.is_some() && marked > 0).then(|| {
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        button(&t, "apply-marked", format!("Apply to {marked}"), false).on_click(
                            cx.listener(|this, _: &ClickEvent, _w, cx| {
                                this.apply_params_to_marked(cx);
                            }),
                        ),
                    )
                    .when_some(hint.clone(), |row, hint| {
                        row.child(
                            div()
                                .text_size(px(11.))
                                .text_color(t.text_muted)
                                .whitespace_nowrap()
                                .child(hint),
                        )
                    })
            });
        let header = div()
            .flex_none()
            .px_3()
            .py_2()
            .flex()
            .flex_col()
            .gap_1()
            .border_b_1()
            .border_color(t.border)
            .child(name_row)
            .children(apply_row);
        let count = self
            .differing_settings(super::parameter_actions::ParamScope::Stage(self.stage))
            .len();
        div()
            .flex()
            .flex_col()
            .child(header)
            // With no eligible recipients there is no Apply button to sit next
            // to, so the lock explanation gets its own line.
            .when_some(hint.filter(|_| marked == 0), |d, hint| {
                d.child(
                    div()
                        .px_3()
                        .py_1()
                        .text_size(px(11.))
                        .text_color(t.text_muted)
                        .child(hint),
                )
            })
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

    /// Copy only the current processing stage to eligible marked groups.
    pub(crate) fn apply_params_to_marked(&mut self, cx: &mut Context<Self>) {
        if let Some(scope) = ParamScope::default_for_stage(self.stage) {
            self.apply_scope_to_marked(scope, cx);
        }
    }

    /// Restore the displayed stage from project defaults for every group kind.
    pub(crate) fn reset_params(&mut self, cx: &mut Context<Self>) {
        self.reset_scope(ParamScope::Stage(self.stage), cx);
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

    fn bkg_weight_control(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let t = self.theme;
        let p = self.ui_params();
        let linked = p.bkg_kweight_linked;
        let field = if linked {
            let text = crate::app::param_text::param_text(ParamKey::BkgKweight);
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .id("bkg-kweight-linked-label")
                        .text_size(px(11.))
                        .text_color(t.text_muted)
                        .when_some(text.tip, |d, tip| {
                            d.tooltip(move |_, cx| {
                                cx.new(|_| Tooltip {
                                    theme: t,
                                    label: tip.into(),
                                })
                                .into()
                            })
                        })
                        .child(text.label),
                )
                .child(
                    div()
                        .text_color(t.text)
                        .child(format!("{} · from FFT", p.effective_bkg_kweight())),
                )
                .into_any_element()
        } else {
            div()
                .children(self.field(ParamKey::BkgKweight, cx))
                .into_any_element()
        };
        let toggle = super::chip(&t, "bkg-kweight-link", "Link to FFT", linked)
            .on_click(cx.listener(|this, _, _, cx| {
                this.edit_parameters("Toggle background k-weight link to FFT".into(), cx, |p| {
                    p.bkg_kweight_linked = !p.bkg_kweight_linked;
                    Ok(())
                });
            }))
            .into_any_element();
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(field)
            .child(
                div()
                    .flex()
                    .justify_end()
                    .px_3()
                    .child(self.parameter_context(
                        super::parameter_actions::ParamScope::Field("bkg_kweight_linked"),
                        toggle,
                        cx,
                    )),
            )
            .into_any_element()
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
        let foldable = matches!(
            title,
            "Clamps & window"
                | "Solver"
                | "Advanced"
                | "Wavelet settings"
                | "Back FT  R → q"
                | "Background options"
                | "Result"
                | "Metadata"
        );
        let key = if title == "Back FT  R → q" {
            "back-transform"
        } else {
            title
        };
        let open = !foldable || self.ui.sections.contains(key);
        let display_title = match title {
            "Pre-edge line" => "Pre-edge · E − E₀",
            "Normalization" => "Normalization · E − E₀",
            _ => title,
        };
        let hint = match title {
            "Pre-edge line" => {
                "Range relative to E₀, in eV. Drag the blue plot handles to change it."
            }
            "Normalization" => {
                "Range relative to E₀, in eV. Drag the yellow plot handles; clear a value for Auto."
            }
            "AUTOBK" => {
                "Drag Rbkg on |χ(R)|, or the k-window edges on χ(k). Rbkg must remain below the shell being fitted."
            }
            "Back FT  R → q" => {
                "Select an R window to isolate shells in χ(q). Auto q step follows the R grid and inverse FFT length; q max changes the extent, not the spacing."
            }
            _ => "",
        };
        let heading = crate::accessibility::Control::new(
            div().id(SharedString::from(format!("section-toggle-{title}"))),
            display_title,
            if foldable {
                accesskit::Role::DisclosureTriangle
            } else {
                accesskit::Role::Heading
            },
        )
        .description(format!(
            "{}{}",
            if self.advanced_section_changed(title) {
                "Modified. "
            } else {
                ""
            },
            hint
        ))
        .when(foldable, |d| d.expanded(open))
        .min_h(px(28.))
        .min_w_0()
        .flex_1()
        .flex()
        .items_center()
        .gap_1()
        .when(foldable, |d| {
            d.tab_index(0)
                .key_context("Control")
                .cursor_pointer()
                .child(icon(
                    &t,
                    if open {
                        Icon::ChevronDown
                    } else {
                        Icon::ChevronRight
                    },
                ))
                .on_click(cx.listener(move |app, _, _, cx| {
                    if !app.ui.sections.remove(key) {
                        app.ui.sections.insert(key);
                    }
                    cx.notify();
                }))
        })
        .child(section_label(&t, display_title))
        .when(
            foldable && !open && self.advanced_section_changed(title),
            |d| {
                d.child(
                    div()
                        .text_size(px(11.))
                        .text_color(t.accent)
                        .child("Modified"),
                )
            },
        )
        .when(!hint.is_empty(), |d| {
            d.tooltip(move |_, cx| {
                cx.new(|_| Tooltip {
                    theme: t,
                    label: hint.into(),
                })
                .into()
            })
        });
        let mut head = div()
            .id(SharedString::from(format!("section-context-{title}")))
            .px_2()
            .pt_1()
            .flex()
            .items_center()
            .gap_1()
            .child(heading);
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
                icon_button(
                    &t,
                    SharedString::from(format!("section-actions-{title}")),
                    Icon::More,
                    format!("{title} actions"),
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
            .when(open, |d| d.children(rows))
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
        let processing = self
            .section(
                "Processing tools",
                None,
                vec![
                    div()
                        .px_2()
                        .child(self.tools_section(cx))
                        .into_any_element(),
                ],
                cx,
            )
            .into_any_element();
        let analysis = self
            .section(
                "Analysis",
                None,
                vec![
                    div()
                        .px_2()
                        .child(self.analysis_tools_section(cx))
                        .into_any_element(),
                ],
                cx,
            )
            .into_any_element();
        let (first, second) = if self.tools.open.is_some_and(super::tools::Tool::is_analysis) {
            (analysis, processing)
        } else {
            (processing, analysis)
        };
        div()
            .flex()
            .flex_col()
            .when(self.tools.open.is_none(), |d| {
                d.child(self.import_summary(cx))
            })
            .child(first)
            .child(second)
            .when(self.tools.open.is_some(), |d| {
                d.child(self.import_summary(cx))
            })
            .child(self.section(
                "Metadata",
                None,
                vec![self.result_card(meta).into_any_element()],
                cx,
            ))
            .child(div().h(px(12.)).bg(t.surface))
    }

    fn normalize_inspector(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let prepared = self.spectrum_quantity.prepared_space().is_some();
        let refit = self.ui_params().refit_prepared;
        let prepared_controls = prepared.then(|| {
            div()
                .p_3()
                .flex()
                .flex_col()
                .gap_2()
                .child(self.note(if refit {
                    "Refitting baselines. Original recovered values are retained."
                } else {
                    "Recovered values · unit edge step. Optional baseline correction below."
                }))
                .child(
                    super::chip(&self.theme, "refit-prepared", "Refit pre/post-edge", refit)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.edit_parameters(
                                "Toggle component pre/post-edge fitting".into(),
                                cx,
                                |p| {
                                    p.refit_prepared = !p.refit_prepared;
                                    Ok(())
                                },
                            );
                        })),
                )
                .into_any_element()
        });
        if prepared && !refit {
            return div()
                .flex()
                .flex_col()
                .children(prepared_controls)
                .child(self.section(
                    "Edge",
                    None,
                    self.field(ParamKey::E0, cx).into_iter().collect(),
                    cx,
                ));
        }
        let mback = self.normalization.show_mback;
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
            .children(prepared_controls)
            .child(self.normalization_controls(cx))
            .child(
                self.section(
                    "Edge",
                    Some(ParamSection::Norm),
                    [
                        self.field(ParamKey::E0, cx),
                        (!mback)
                            .then(|| self.field(ParamKey::EdgeStep, cx))
                            .flatten(),
                    ]
                    .into_iter()
                    .flatten()
                    .collect(),
                    cx,
                ),
            )
            .child(
                self.section(
                    "Pre-edge line",
                    None,
                    [
                        self.field(ParamKey::PreEdgeStart, cx),
                        self.field(ParamKey::PreEdgeEnd, cx),
                        (!mback)
                            .then(|| self.field(ParamKey::NVictoreen, cx))
                            .flatten(),
                        None,
                    ]
                    .into_iter()
                    .flatten()
                    .collect(),
                    cx,
                ),
            )
            .child(
                self.section(
                    "Normalization",
                    None,
                    [
                        self.field(ParamKey::NormStart, cx),
                        self.field(ParamKey::NormEnd, cx),
                        self.field(ParamKey::NormPolyorder, cx),
                        None,
                    ]
                    .into_iter()
                    .flatten()
                    .collect(),
                    cx,
                ),
            )
            .child(self.section(
                "Result",
                None,
                vec![
                    self.result_card(vec![
                        ("E₀".into(), fmt(e0, 1, " eV")),
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
        let p = self.ui_params();
        let linear = p
            .bkg_solver
            .is_none_or(|s| s == rexafs::prelude::AUTOBKSolver::LinearDirect);
        let fixed = p.bkg_clamp_policy == rexafs::prelude::AUTOBKClampScalePolicy::FixedPenalty;
        let mut solver = vec![self.enum_row(EnumParam::BkgSolver, cx)];
        solver.extend(
            [
                self.field(ParamKey::BkgKstep, cx),
                self.field(ParamKey::BkgNfft, cx),
            ]
            .into_iter()
            .flatten(),
        );
        if linear {
            solver.extend(self.field(ParamKey::BkgCondition, cx));
            solver.push(self.enum_row(EnumParam::BkgCache, cx));
            if fixed {
                solver.push(self.note("Fixed λ uses one linear solve. The condition limit controls the SVD cutoff; legacy ridge and iterative fallback are inactive.").into_any_element());
            } else {
                solver.extend(
                    [
                        self.field(ParamKey::BkgRegularization, cx),
                        self.field(ParamKey::BkgResidualRatio, cx),
                    ]
                    .into_iter()
                    .flatten(),
                );
                solver.push(self.enum_row(EnumParam::BkgFallback, cx));
                if p.bkg_linear_fallback_to_lm != Some(false) {
                    solver.push(self.enum_row(EnumParam::BkgFallbackSolver, cx));
                }
                solver.push(self.note("Legacy models can use ridge regularization and retry with the selected iterative solver when the direct solve fails its checks.").into_any_element());
            }
        } else {
            solver.push(self.note("Linear matrix controls apply to LinearDirect. Select Fixed λ for the single-solve method.").into_any_element());
        }
        solver.push(
            self.result_card(vec![(
                "Spline knots".into(),
                knots.map(|k| k.to_string()).unwrap_or("auto".into()),
            )])
            .into_any_element(),
        );
        let mut standard = div().px_3().py_1().flex().flex_col().gap_1()
            .child(self.note("Optional standard: two columns, k (Å⁻¹) and unweighted χ(k). Its values are saved in the project."))
            .child(button(&self.theme, "load-chi-standard", "Load standard…", false).on_click(cx.listener(|this, _, _, cx| this.choose_chi_standard(cx))));
        if let Some(s) = &p.bkg_standard {
            standard = standard
                .child(div().text_xs().text_color(self.theme.text).child(format!(
                    "{} · {} points",
                    s.label,
                    s.k.len()
                )))
                .child(
                    button(&self.theme, "clear-chi-standard", "Clear standard", false)
                        .on_click(cx.listener(|this, _, _, cx| this.set_chi_standard(None, cx))),
                );
        }
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
                    None,
                    Some(self.bkg_weight_control(cx)),

                ]
                .into_iter()
                .flatten()
                .collect(),
                cx,
            ))
            .child(self.section("Background options", None,
                [self.field(ParamKey::BkgNknots, cx), self.field(ParamKey::BkgEk0, cx), Some(standard.into_any_element())]
                    .into_iter().flatten().collect(), cx))
            .child(self.section(
                "Clamps & window",
                None,
                [
                    self.field(ParamKey::BkgClampLo, cx),
                    self.field(ParamKey::BkgClampHi, cx),
                    self.field(ParamKey::BkgNclamp, cx),
                    Some(self.enum_row(EnumParam::BkgClampPolicy, cx)),
                    if self.ui_params().bkg_clamp_policy == rexafs::prelude::AUTOBKClampScalePolicy::FixedPenalty {
                        self.field(ParamKey::BkgClampLambda, cx)
                    } else {
                        Some(self.note("λ is inactive for legacy clamp models. Select Fixed λ with the linear solver to use a constant penalty.").into_any_element())
                    },
                    Some(self.note("Clamp points sets the number of samples at each active end. Zero points disables clamping; in Fixed λ, λ = 0 also disables it.").into_any_element()),
                    Some(self.enum_row(EnumParam::BkgWindow, cx)),
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
                solver,
                cx,
            ))
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
        let back_ft = self.spectrum.as_ref().and_then(|sp| sp.xftr.as_ref());
        let rmin = p
            .bft_rmin
            .or_else(|| back_ft.and_then(|f| f.rmin))
            .unwrap_or(0.0);
        let rmax = p
            .bft_rmax
            .or_else(|| back_ft.and_then(|f| f.rmax))
            .unwrap_or(20.0);
        let nidp = 2.0 * (kmax - kmin) * (rmax - rmin) / std::f64::consts::PI;
        div()
            .flex()
            .flex_col()
            .child(
                self.section(
                    "Forward FT  k → R",
                    Some(ParamSection::Fft),
                    [
                        self.field(ParamKey::FftKmin, cx),
                        self.field(ParamKey::FftKmax, cx),
                        self.field(ParamKey::FftDk, cx),
                        Some(self.enum_row(EnumParam::FftWindow, cx)),
                        self.field(ParamKey::FftKweight, cx),
                        self.field(ParamKey::FftRmax, cx),
                    ]
                    .into_iter()
                    .flatten()
                    .collect(),
                    cx,
                ),
            )
            .child(self.section(
                "Wavelet settings",
                None,
                vec![self.wavelet_inspector(cx)],
                cx,
            ))
            .child(
                self.section(
                    "Back FT  R → q",
                    None,
                    [
                        self.field(ParamKey::BftRmin, cx),
                        self.field(ParamKey::BftRmax, cx),
                        self.field(ParamKey::BftDr, cx),
                        self.field(ParamKey::BftDr2, cx),
                        Some(self.enum_row(EnumParam::BftWindow, cx)),
                        self.field(ParamKey::BftRweight, cx),
                        self.field(ParamKey::BftQmax, cx),
                        self.field(ParamKey::BftNfft, cx),
                        self.field(ParamKey::BftKstep, cx),
                        None,
                        None,
                    ]
                    .into_iter()
                    .flatten()
                    .collect(),
                    cx,
                ),
            )
            .child(
                self.section(
                    "Advanced",
                    None,
                    [
                        Some(self.enum_row(EnumParam::FftGrid, cx)),
                        self.field(ParamKey::FftDk2, cx),
                        self.field(ParamKey::FftKstep, cx),
                        self.field(ParamKey::FftNfft, cx),
                    ]
                    .into_iter()
                    .flatten()
                    .collect(),
                    cx,
                ),
            )
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

#[cfg(test)]
mod tests {
    use super::apply_hint;

    #[test]
    fn apply_hint_is_hidden_without_recipients_or_locks() {
        assert_eq!(apply_hint(0, 0), None);
    }

    #[test]
    fn apply_hint_omits_zero_locks() {
        assert_eq!(apply_hint(2, 0).as_deref(), Some("excludes current"));
    }

    #[test]
    fn apply_hint_explains_locked_recipients() {
        assert_eq!(
            apply_hint(2, 3).as_deref(),
            Some("excludes current · 3 locked")
        );
    }

    #[test]
    fn apply_hint_explains_locks_without_eligible_recipients() {
        assert_eq!(
            apply_hint(0, 1).as_deref(),
            Some("excludes current · 1 locked")
        );
    }
}
