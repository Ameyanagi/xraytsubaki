//! Spectrum → path editor. Fields always address an explicit dataset/path.
use super::{button, chip, fit_workspace::FitStep};
use crate::{
    app::StudioApp,
    fitting::{FitPathSpec, FitSpaceSpec, expr_identifiers},
    joint_fitting::{self, JointDataset},
    widgets::text_input::{InputEvent, InputStyle, TextInput},
};
use gpui::{Context, Entity, IntoElement, ParentElement, Styled, div, prelude::*, px};
use std::{collections::BTreeSet, path::PathBuf};

#[derive(Clone)]
enum Edit {
    Value(usize, String),
    Range(usize, usize),
    Expression(usize, PathBuf, usize),
}
fn terms(p: &FitPathSpec) -> [&str; 7] {
    [
        &p.s02, &p.e0, &p.deltar, &p.sigma2, &p.ei, &p.third, &p.fourth,
    ]
}
const TERMS: [&str; 7] = [
    "S₀²",
    "ΔE₀ (eV)",
    "ΔR (Å)",
    "σ² (Å²)",
    "Eᵢ (eV)",
    "C₃ (Å³)",
    "C₄ (Å⁴)",
];
fn term_mut(p: &mut FitPathSpec, i: usize) -> &mut String {
    match i {
        0 => &mut p.s02,
        1 => &mut p.e0,
        2 => &mut p.deltar,
        3 => &mut p.sigma2,
        4 => &mut p.ei,
        5 => &mut p.third,
        _ => &mut p.fourth,
    }
}
impl StudioApp {
    fn joint_field(
        &mut self,
        key: String,
        value: String,
        target: Edit,
        cx: &mut Context<Self>,
    ) -> Entity<TextInput> {
        if let Some((previous, field)) = self.joint.fields.get_mut(&key) {
            if *previous != value {
                field.update(cx, |f, cx| f.set_text(value.clone(), cx));
                *previous = value;
            }
            return field.clone();
        }
        let theme = self.theme;
        let field = cx.new(|cx| {
            TextInput::new("", value.clone(), theme, cx).with_style(InputStyle {
                mono: true,
                ..Default::default()
            })
        });
        cx.subscribe(&field, move |this: &mut Self, field, event, cx| {
            let InputEvent::Committed(text) = event else {
                return;
            };
            let text = text.trim();
            let number = text.parse::<f64>().ok().filter(|v| v.is_finite());
            if !matches!(target, Edit::Expression(..)) && number.is_none() {
                field.update(cx, |f, cx| f.set_error(true, cx));
                return;
            }
            field.update(cx, |f, cx| f.set_error(false, cx));
            match &target {
                Edit::Value(id, name) => {
                    if this.joint.config.is_local(*id, name) {
                        this.joint
                            .config
                            .values
                            .entry(*id)
                            .or_default()
                            .insert(name.clone(), number.unwrap());
                    } else if let Some(v) = this.fit_vars.iter_mut().find(|v| v.spec.name == *name)
                    {
                        v.spec.value = number.unwrap();
                        v.field.update(cx, |f, cx| f.set_text(text.to_string(), cx));
                    }
                }
                Edit::Range(id, axis) => {
                    if let Some(d) = this.joint.config.datasets.iter_mut().find(|d| d.id == *id) {
                        let r = d.ranges.get_or_insert_with(|| this.fit_ranges.clone());
                        match axis {
                            0 => r.kmin = number.unwrap(),
                            1 => r.kmax = number.unwrap(),
                            2 => r.rmin = number.unwrap(),
                            _ => r.rmax = number.unwrap(),
                        }
                    }
                }
                Edit::Expression(id, file, term) => {
                    let base = this
                        .fit_paths
                        .iter()
                        .find(|p| p.spec.file == *file)
                        .map(|p| p.spec.clone());
                    if let (Some(base), Some(d)) = (
                        base,
                        this.joint.config.datasets.iter_mut().find(|d| d.id == *id),
                    ) {
                        *term_mut(d.expressions.entry(file.clone()).or_insert(base), *term) =
                            text.to_string();
                    }
                }
            }
            this.fit_model_changed(cx);
            cx.notify();
        })
        .detach();
        self.joint.fields.insert(key, (value, field.clone()));
        field
    }
    fn joint_range_editor(
        &mut self,
        d: &JointDataset,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let t = self.theme;
        let id = d.id;
        let kw = self.joint_params(&d.file).fft_kweight;
        let r = d.ranges.as_ref().unwrap_or(&self.fit_ranges).resolved(kw);
        let mut row = div().flex().flex_wrap().items_center().gap_2();
        for (i, (label, v)) in [
            ("k min", r.kmin),
            ("k max", r.kmax),
            ("R min", r.rmin),
            ("R max", r.rmax),
        ]
        .into_iter()
        .enumerate()
        {
            let f = self.joint_field(
                format!("range-{id}-{i}"),
                v.to_string(),
                Edit::Range(id, i),
                cx,
            );
            row = row.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .w(px(73.))
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(t.text_muted)
                            .child(label),
                    )
                    .child(f),
            );
        }
        let mut weights = div()
            .flex()
            .items_center()
            .flex_wrap()
            .gap_1()
            .text_size(px(11.))
            .child("k-weight");
        weights = weights.child(
            chip(
                &t,
                format!("auto-kw-{id}"),
                format!("Transform ({:.0})", kw.unwrap_or(2.)),
                r.follow_transform,
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                let fallback = this.fit_ranges.clone();
                if let Some(d) = this.joint.config.datasets.iter_mut().find(|d| d.id == id) {
                    let r = d.ranges.get_or_insert(fallback);
                    r.follow_transform = !r.follow_transform;
                    if !r.follow_transform {
                        *r = r.resolved(kw);
                        r.kweight = kw.unwrap_or(2.);
                        r.kweights = vec![r.kweight];
                    }
                }
                this.fit_model_changed(cx);
                cx.notify();
            })),
        );
        for k in [1., 2., 3.] {
            weights = weights.child(
                chip(
                    &t,
                    format!("kw-{id}-{k}"),
                    format!("{k:.0}"),
                    !r.follow_transform && r.effective_kweights().contains(&k),
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    let fallback = this.fit_ranges.clone();
                    if let Some(d) = this.joint.config.datasets.iter_mut().find(|d| d.id == id) {
                        let r = d.ranges.get_or_insert(fallback);
                        *r = r.resolved(kw);
                        r.toggle_kweight(k);
                        if !r.kweights.contains(&r.kweight) {
                            r.kweight = r.kweights[0];
                        }
                    }
                    this.fit_model_changed(cx);
                    cx.notify();
                })),
            );
        }
        weights = weights.child(div().ml_2().child("Fit in"));
        for space in FitSpaceSpec::ALL {
            weights = weights.child(
                chip(
                    &t,
                    format!("space-{id}-{}", space.label()),
                    space.label(),
                    r.fitspace == space,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    let fallback = this.fit_ranges.clone();
                    if let Some(d) = this.joint.config.datasets.iter_mut().find(|d| d.id == id) {
                        d.ranges.get_or_insert(fallback).fitspace = space;
                    }
                    this.select_fit_space_view(space, cx);
                    this.fit_model_changed(cx);
                    cx.notify();
                })),
            );
        }
        div()
            .flex()
            .flex_col()
            .gap_2()
            .pb_3()
            .border_b_1()
            .border_color(t.border)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child("Fit range")
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(t.text_muted)
                            .child("k: Å⁻¹ · R: Å"),
                    ),
            )
            .child(row)
            .child(weights)
            .when(!r.valid(), |d| {
                d.child(
                    div()
                        .text_color(t.warn)
                        .child("Minimum must be below maximum."),
                )
            })
    }
    fn joint_path_editor(
        &mut self,
        d: &JointDataset,
        base: &FitPathSpec,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let t = self.theme;
        let id = d.id;
        let p = d.expressions.get(&base.file).unwrap_or(base).clone();
        let key = format!("{id}-{}", p.file.display());
        let meta = self
            .fit_paths
            .iter()
            .find(|r| r.spec.file == p.file)
            .and_then(|r| r.meta);
        let source = p
            .file
            .parent()
            .and_then(|p| p.file_name())
            .unwrap_or_default()
            .to_string_lossy();
        let mut card = div()
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .border_1()
            .border_color(t.border)
            .rounded_md()
            .bg(t.surface)
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .child(p.label.clone())
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(t.text_muted)
                            .child(format!(
                                "{source}{}",
                                meta.map(|m| format!(" · {:.3} Å · {} legs", m.reff, m.nleg))
                                    .unwrap_or_default()
                            )),
                    ),
            );
        let mut names = BTreeSet::new();
        for expr in terms(&p) {
            names.extend(expr_identifiers(expr));
        }
        // Include the independent inputs of constrained variables.
        let mut pending = names.iter().cloned().collect::<Vec<_>>();
        while let Some(name) = pending.pop() {
            if let Some(expr) = self
                .fit_vars
                .iter()
                .find(|v| v.spec.name == name)
                .and_then(|v| v.spec.expr.as_ref())
            {
                for dep in expr_identifiers(expr) {
                    if names.insert(dep.clone()) {
                        pending.push(dep);
                    }
                }
            }
        }
        card = card.child(
            div()
                .flex()
                .gap_2()
                .text_size(px(10.))
                .text_color(t.text_muted)
                .child(div().w(px(84.)).child("Parameter"))
                .child(div().w(px(70.)).child("Initial value"))
                .child(div().w(px(26.)).child("Fit"))
                .child("Scope"),
        );
        for name in names {
            let Some(v) = self
                .fit_vars
                .iter()
                .find(|v| v.spec.name == name)
                .map(|v| v.spec.clone())
            else {
                let create = name.clone();
                card = card.child(
                    div()
                        .flex()
                        .gap_2()
                        .text_color(t.warn)
                        .child(format!("Undefined: {name}"))
                        .child(
                            button(&t, format!("create-{key}-{name}"), "Add parameter", false)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.ensure_fit_var(&create, 0., cx);
                                    this.joint
                                        .config
                                        .scopes
                                        .entry(id)
                                        .or_default()
                                        .insert(create.clone(), true);
                                    this.fit_model_changed(cx);
                                    cx.notify();
                                })),
                        ),
                );
                continue;
            };
            let local = self.joint.config.is_local(id, &name);
            let value = self.joint.config.initial_value(id, &v);
            let varying = self.joint.config.varies(id, &v);
            let physical = terms(&p)
                .iter()
                .position(|e| e.trim() == name)
                .map(|i| TERMS[i]);
            let mut row = div()
                .flex()
                .items_center()
                .flex_wrap()
                .gap_2()
                .text_size(px(11.))
                .child(
                    div()
                        .w(px(84.))
                        .flex_none()
                        .flex()
                        .flex_col()
                        .child(physical.unwrap_or(&name).to_string())
                        .when(physical.is_some(), |d| {
                            d.child(
                                div()
                                    .text_size(px(10.))
                                    .text_color(t.text_muted)
                                    .child(name.clone()),
                            )
                        }),
                );
            if let Some(expr) = &v.expr {
                row = row.child(
                    div()
                        .w(px(150.))
                        .text_color(t.text_muted)
                        .child(format!("= {expr}")),
                );
            } else {
                let field = self.joint_field(
                    format!("value-{key}-{name}"),
                    value.to_string(),
                    Edit::Value(id, name.clone()),
                    cx,
                );
                let vn = name.clone();
                row = row.child(div().w(px(70.)).child(field)).child(
                    chip(
                        &t,
                        format!("vary-{key}-{name}"),
                        if varying { "☑" } else { "☐" },
                        varying,
                    )
                    .w(px(26.))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if this.joint.config.is_local(id, &vn) {
                            this.joint
                                .config
                                .varying
                                .entry(id)
                                .or_default()
                                .insert(vn.clone(), !varying);
                        } else if let Some(v) = this.fit_vars.iter_mut().find(|v| v.spec.name == vn)
                        {
                            v.spec.vary = !varying;
                        }
                        this.fit_model_changed(cx);
                        cx.notify();
                    })),
                );
            }
            for (scope, label) in [(false, "Global"), (true, "This spectrum")] {
                let vn = name.clone();
                row = row.child(
                    chip(
                        &t,
                        format!("scope-{key}-{name}-{scope}"),
                        label,
                        scope == local,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if scope && !this.joint.config.is_local(id, &vn) {
                            this.joint
                                .config
                                .values
                                .entry(id)
                                .or_default()
                                .insert(vn.clone(), value);
                            this.joint
                                .config
                                .varying
                                .entry(id)
                                .or_default()
                                .insert(vn.clone(), varying);
                        }
                        this.joint
                            .config
                            .scopes
                            .entry(id)
                            .or_default()
                            .insert(vn.clone(), scope);
                        this.fit_model_changed(cx);
                        cx.notify();
                    })),
                );
            }
            card = card.child(row);
        }
        // Fixed path constants stay visible even when a path uses no variables.
        for (i, expr) in terms(&p).into_iter().enumerate() {
            if let Ok(value) = expr.parse::<f64>() {
                let field = self.joint_field(
                    format!("constant-{key}-{i}"),
                    value.to_string(),
                    Edit::Expression(id, p.file.clone(), i),
                    cx,
                );
                let file = p.file.clone();
                card = card.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_size(px(11.))
                        .child(div().w(px(84.)).child(TERMS[i]))
                        .child(div().w(px(70.)).child(field))
                        .child(div().w(px(26.)).text_color(t.text_muted).child("Fixed"))
                        .child(
                            button(
                                &t,
                                format!("fit-constant-{key}-{i}"),
                                "Fit this value",
                                false,
                            )
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    let mut index = 1;
                                    let mut name = format!("p{id}_{i}_{index}");
                                    while this.fit_vars.iter().any(|v| v.spec.name == name) {
                                        index += 1;
                                        name = format!("p{id}_{i}_{index}");
                                    }
                                    this.ensure_fit_var(&name, value, cx);
                                    this.joint
                                        .config
                                        .scopes
                                        .entry(id)
                                        .or_default()
                                        .insert(name.clone(), true);
                                    let base = this
                                        .fit_paths
                                        .iter()
                                        .find(|r| r.spec.file == file)
                                        .map(|r| r.spec.clone());
                                    if let (Some(base), Some(d)) = (
                                        base,
                                        this.joint.config.datasets.iter_mut().find(|d| d.id == id),
                                    ) {
                                        *term_mut(
                                            d.expressions.entry(file.clone()).or_insert(base),
                                            i,
                                        ) = name;
                                    }
                                    this.fit_model_changed(cx);
                                    cx.notify();
                                },
                            )),
                        ),
                );
            }
        }
        if self.joint.advanced {
            card = card.child(
                div()
                    .pt_2()
                    .text_size(px(11.))
                    .text_color(t.text_muted)
                    .child("Path expressions"),
            );
            for (i, expr) in terms(&p).into_iter().enumerate() {
                let field = self.joint_field(
                    format!("expression-{key}-{i}"),
                    expr.to_string(),
                    Edit::Expression(id, p.file.clone(), i),
                    cx,
                );
                card = card.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_size(px(11.))
                        .child(div().w(px(90.)).child(TERMS[i]))
                        .child(div().flex_1().child(field)),
                );
            }
        }
        card
    }
    pub(super) fn joint_setup_panel(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let datasets = self.joint.config.datasets.clone();
        if self
            .joint
            .selected
            .as_ref()
            .is_none_or(|(id, _)| !datasets.iter().any(|d| d.id == *id))
        {
            self.joint.selected = datasets.first().map(|d| (d.id, None));
        }
        let selected = self.joint.selected.clone();
        let mut tree = div()
            .id("spectra-tree")
            .w(px(152.))
            .flex_none()
            .min_h_0()
            .overflow_y_scroll()
            .border_r_1()
            .border_color(t.border)
            .px_2()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_1()
                    .pb_2()
                    .child(
                        button(&t, "add-current", "+ Current", false).on_click(cx.listener(
                            |this, _, _, cx| {
                                this.add_joint_files(vec![this.current_path.clone()], cx)
                            },
                        )),
                    )
                    .child(
                        button(
                            &t,
                            "add-marked",
                            format!("+ Marked ({})", self.selection.len()),
                            false,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            let files = this
                                .selection
                                .iter()
                                .filter(|&&i| i < this.catalog.len())
                                .map(|&i| this.catalog.path(i))
                                .collect();
                            this.add_joint_files(files, cx);
                        })),
                    ),
            );
        for d in &datasets {
            let id = d.id;
            tree = tree.child(
                div()
                    .id(("spectrum-node", id))
                    .p_2()
                    .rounded_md()
                    .cursor_pointer()
                    .when(
                        selected
                            .as_ref()
                            .is_some_and(|(i, p)| *i == id && p.is_none()),
                        |d| d.bg(t.raised),
                    )
                    .hover(|d| d.bg(t.raised))
                    .text_size(px(12.))
                    .child(format!("▾ {}", d.label))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.joint.selected = Some((id, None));
                        cx.notify();
                    })),
            );
            for (index, file) in d.paths.iter().enumerate() {
                let label = self
                    .fit_paths
                    .iter()
                    .find(|p| p.spec.file == *file)
                    .map(|p| p.spec.label.clone())
                    .unwrap_or_else(|| "Missing path".into());
                let file = file.clone();
                let on = selected
                    .as_ref()
                    .is_some_and(|(i, p)| *i == id && p.as_ref() == Some(&file));
                tree = tree.child(
                    div()
                        .id(("path-node", id * 100000 + index))
                        .ml_3()
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .cursor_pointer()
                        .text_size(px(11.))
                        .when(on, |d| d.bg(t.raised).text_color(t.accent))
                        .hover(|d| d.bg(t.raised))
                        .child(label)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.joint.selected = Some((id, Some(file.clone())));
                            cx.notify();
                        })),
                );
            }
            tree = tree.child(div().ml_3().mb_2().child(
                button(&t, ("assign-paths", id), "± Paths", false).on_click(cx.listener(
                    move |this, _, _, cx| {
                        this.joint.selected = Some((id, None));
                        this.joint.edit_paths = if this.joint.edit_paths == Some(id) {
                            None
                        } else {
                            Some(id)
                        };
                        cx.notify();
                    },
                )),
            ));
        }
        let mut detail = div()
            .id("spectrum-path-detail")
            .flex_1()
            .min_w_0()
            .min_h_0()
            .overflow_y_scroll()
            .px_3()
            .pb_3()
            .flex()
            .flex_col()
            .gap_3();
        if let Some((id, path)) = selected
            && let Some(d) = datasets.iter().find(|d| d.id == id)
        {
            detail =
                detail.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(div().flex_1().text_size(px(15.)).child(d.label.clone()))
                        .child(
                            chip(&t, "joint-advanced", "Advanced", self.joint.advanced).on_click(
                                cx.listener(|this, _, _, cx| {
                                    this.joint.advanced = !this.joint.advanced;
                                    cx.notify();
                                }),
                            ),
                        )
                        .child(button(&t, "remove-spectrum", "Remove", false).on_click(
                            cx.listener(move |this, _, _, cx| {
                                this.joint.config.datasets.retain(|d| d.id != id);
                                this.joint.config.scopes.remove(&id);
                                this.joint.config.values.remove(&id);
                                this.joint.config.varying.remove(&id);
                                this.joint.fields.clear();
                                this.fit_model_changed(cx);
                                cx.notify();
                            }),
                        )),
                );
            detail = detail.child(self.joint_range_editor(d, cx));
            if self.joint.edit_paths == Some(id) {
                let mut assignments = div().flex().flex_col().gap_1().child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child("Paths")
                        .child(
                            button(&t, "finish-assign", "Done", false).on_click(cx.listener(
                                |this, _, _, cx| {
                                    this.joint.edit_paths = None;
                                    cx.notify();
                                },
                            )),
                        )
                        .child(
                            button(&t, "calculate-more", "Configure more…", false).on_click(
                                cx.listener(|this, _, _, cx| this.set_fit_step(FitStep::Paths, cx)),
                            ),
                        ),
                );
                for (i, row) in self.fit_paths.iter().enumerate() {
                    let p = &row.spec;
                    let on = d.paths.contains(&p.file);
                    if !on
                        && [&p.s02, &p.e0, &p.deltar, &p.sigma2]
                            .iter()
                            .any(|e| e.is_empty())
                    {
                        continue;
                    }
                    let file = p.file.clone();
                    let source = file
                        .parent()
                        .and_then(|p| p.file_name())
                        .unwrap_or_default()
                        .to_string_lossy();
                    assignments = assignments.child(
                        chip(
                            &t,
                            ("assign-path", i),
                            format!("{} {} · {source}", if on { "☑" } else { "☐" }, p.label),
                            on,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(d) =
                                this.joint.config.datasets.iter_mut().find(|d| d.id == id)
                            {
                                if d.paths.contains(&file) {
                                    d.paths.retain(|p| p != &file);
                                } else {
                                    d.paths.push(file.clone());
                                }
                            }
                            this.fit_model_changed(cx);
                            cx.notify();
                        })),
                    );
                }
                detail = detail.child(assignments);
            }
            let paths = self
                .fit_paths
                .iter()
                .filter(|p| {
                    d.paths.contains(&p.spec.file)
                        && path.as_ref().is_none_or(|f| *f == p.spec.file)
                })
                .map(|p| p.spec.clone())
                .collect::<Vec<_>>();
            for p in &paths {
                detail = detail.child(self.joint_path_editor(d, p, cx));
            }
            if paths.is_empty() {
                detail = detail.child(div().text_color(t.warn).child("Add paths with ± Paths."));
            }
        } else {
            detail = detail.child("Add spectra from the file browser.");
        }
        let paths = self
            .fit_paths
            .iter()
            .map(|p| p.spec.clone())
            .collect::<Vec<_>>();
        let vars = self
            .fit_vars
            .iter()
            .map(|v| v.spec.clone())
            .collect::<Vec<_>>();
        if let Err(e) = joint_fitting::prepare(&self.joint.config, &paths, &vars) {
            detail = detail.child(div().text_size(px(11.)).text_color(t.warn).child(e));
        }
        div()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .flex()
            .child(tree)
            .child(detail)
            .child(
                div()
                    .w(gpui::relative(0.43))
                    .flex_none()
                    .min_w(px(310.))
                    .min_h_0()
                    .flex()
                    .border_l_1()
                    .border_color(t.border)
                    .child(self.fit_preview_panel(cx)),
            )
    }
}
