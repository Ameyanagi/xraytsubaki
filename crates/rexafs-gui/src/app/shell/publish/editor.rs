use super::*;
use crate::accessibility::Control;
use crate::app::shell::{
    button, chip,
    controls::{Tooltip, disclosure, icon_button},
    segment, segmented,
};
use crate::icons::Icon;
use crate::publication::figures::{LegendPlacement, StylePreset, fit_figures, render_figure};
use crate::theme::Theme;
use crate::widgets::numeric_field::{FieldEvent, FieldKind};
use crate::widgets::text_input::InputEvent;
use gpui::{
    FontWeight, ImageFormat, IntoElement, ObjectFit, SharedString, Styled, div, img, prelude::*, px,
};

/// Small layout previews retain full accessible names without visible labels.
fn preset_icon(t: &Theme, index: usize, preset: StylePreset, selected: bool) -> Control {
    let color = if selected { t.accent } else { t.text_muted };
    let mut preview = div()
        .flex()
        .items_center()
        .justify_center()
        .gap(px(2.))
        .w(px(24.))
        .h(px(18.))
        .border_1()
        .border_color(color)
        .rounded_sm();
    match preset {
        StylePreset::SingleColumn => {
            preview = preview.child(div().w(px(7.)).h(px(11.)).border_1().border_color(color));
        }
        StylePreset::DoubleColumn => {
            for _ in 0..2 {
                preview = preview.child(div().w(px(7.)).h(px(11.)).border_1().border_color(color));
            }
        }
        StylePreset::Slide => {
            preview = preview
                .h(px(14.))
                .child(div().w(px(16.)).h(px(7.)).border_1().border_color(color));
        }
    }
    Control::new(
        div().id(("publication-preset", index)),
        preset.label(),
        accesskit::Role::Button,
    )
    .selected(selected)
    .tab_index(0)
    .key_context("Control")
    .w(px(36.))
    .h(px(32.))
    .flex_none()
    .flex()
    .items_center()
    .justify_center()
    .rounded_md()
    .border_1()
    .border_color(if selected { t.accent } else { t.border })
    .when(selected, |d| d.bg(t.raised))
    .cursor_pointer()
    .hover(|d| d.bg(t.raised))
    .focus(|d| d.border_color(t.accent))
    .child(preview)
}

/// Attach a hover tooltip and matching accessibility description to a control,
/// so explanations live on the control instead of as text under it.
pub(super) fn with_tip(control: Control, t: &Theme, text: impl Into<SharedString>) -> Control {
    let tip = Tooltip {
        label: text.into(),
        theme: *t,
    };
    control
        .description(tip.label.to_string())
        .tooltip(move |_, cx| cx.new(|_| tip.clone()).into())
}

/// Grey out a control and ignore its clicks while `enabled` is false.
pub(super) fn enabled_when(control: Control, enabled: bool) -> Control {
    control.when(!enabled, |d| {
        d.disabled(true).opacity(0.45).cursor_default()
    })
}

/// Muted 12 px heading above a group of fields.
fn heading(t: &Theme, text: &'static str) -> gpui::Div {
    div()
        .mt_1()
        .text_size(px(12.))
        .text_color(t.text_muted)
        .child(text)
}

impl StudioApp {
    fn refresh_publication_source(&mut self, cx: &mut Context<Self>) {
        let ready = self.publication_ready();
        let source = (
            self.spectrum
                .as_ref()
                .map(|s| Arc::as_ptr(s) as usize)
                .unwrap_or(0),
            self.fit_result
                .as_ref()
                .map(|s| Arc::as_ptr(s) as usize)
                .unwrap_or(0),
            self.joint.result_index,
            format!(
                "{}:{:?}:{:?}:{ready}",
                self.spectrum_label, self.theme.mode, self.spectrum_quantity
            ),
        );
        if self.publish.source.as_ref() == Some(&source) {
            return;
        }
        self.publish.source = Some(source);
        let selected_key = self
            .publish
            .figures
            .get(self.publish.selected)
            .map(|f| f.key);
        let current = self.tool_target(self.selected.unwrap_or(crate::app::NO_ENTRY));
        let mut figures = publication_spectrum_figures(
            current.as_ref(),
            self.spectrum_group.as_ref(),
            self.spectrum.as_ref(),
            self.spectrum_quantity,
            self.stale_plots.is_some(),
            self.load_running,
        );
        if let Some(result) = &self.fit_result {
            figures.extend(fit_figures(&crate::joint_fitting::result_view(
                result,
                self.joint.result_index,
            )));
        }
        self.publish.figures = figures.into_iter().map(Arc::new).collect();
        self.publish.selected = self
            .publish
            .figures
            .iter()
            .position(|f| Some(f.key) == selected_key)
            .unwrap_or(0);
        self.publication_fields(cx);
        self.refresh_publication_preview(cx);
    }

    fn publication_fields(&mut self, cx: &mut Context<Self>) {
        self.publish.numbers.clear();
        self.publish.labels.clear();
        let Some(figure) = self.publish.figures.get(self.publish.selected).cloned() else {
            return;
        };
        let options = self.publish.settings.options(figure.key);
        let defaults = ruviz::prelude::Plot::new();
        let config = defaults.get_config();
        let values = [
            (
                "Width (in)",
                options.width,
                format!("Auto ({:.1})", config.figure.width),
            ),
            (
                "Height (in)",
                options.height,
                format!("Auto ({:.1})", config.figure.height),
            ),
            ("DPI", options.dpi, "Auto (300)".into()),
            ("Font size (pt)", options.font_size, "Auto".into()),
            ("Line width (pt)", options.line_width, "Auto".into()),
            (
                "X min",
                options.xmin,
                figure
                    .default_xlim
                    .map(|(min, _)| format!("Auto ({min:.2})"))
                    .unwrap_or("Auto".into()),
            ),
            (
                "X max",
                options.xmax,
                figure
                    .default_xlim
                    .map(|(_, max)| format!("Auto ({max:.2})"))
                    .unwrap_or("Auto".into()),
            ),
            ("Y min", options.ymin, "Auto".into()),
            ("Y max", options.ymax, "Auto".into()),
        ];
        for (index, (label, value, placeholder)) in values.into_iter().enumerate() {
            let theme = self.theme;
            let field = cx.new(|cx| {
                NumericField::new(label, placeholder, value, FieldKind::Float, theme, cx)
            });
            let key = figure.key;
            cx.subscribe(&field, move |this, _, event, cx| match event {
                FieldEvent::Changed(value) => {
                    let options = this.publish.settings.figures.entry(key.into()).or_default();
                    match index {
                        0 => options.width = *value,
                        1 => options.height = *value,
                        2 => options.dpi = *value,
                        3 => options.font_size = *value,
                        4 => options.line_width = *value,
                        5 => options.xmin = *value,
                        6 => options.xmax = *value,
                        7 => options.ymin = *value,
                        _ => options.ymax = *value,
                    }
                    this.refresh_publication_preview(cx);
                }
                FieldEvent::Invalid(message) => {
                    this.publish.error = Some(message.to_string());
                    cx.notify();
                }
                _ => (),
            })
            .detach();
            self.publish.numbers.push(field);
        }
        let caption = figure.caption(&options);
        let table_defaults = crate::publication::report::TABLE_CAPTIONS;
        for (index, (placeholder, value)) in [
            ("No title".to_string(), options.title.clone()),
            (figure.math_xlabel().into(), options.xlabel.clone()),
            (figure.math_ylabel(&options).into(), options.ylabel.clone()),
            (caption, options.caption),
            (
                table_defaults[0].2.into(),
                self.publish
                    .settings
                    .table_captions
                    .get("processing")
                    .cloned(),
            ),
            (
                table_defaults[1].2.into(),
                self.publish
                    .settings
                    .table_captions
                    .get("parameters")
                    .cloned(),
            ),
            (
                table_defaults[2].2.into(),
                self.publish.settings.table_captions.get("paths").cloned(),
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let theme = self.theme;
            let field =
                cx.new(|cx| TextInput::new(placeholder, value.unwrap_or_default(), theme, cx));
            let key = figure.key;
            cx.subscribe(&field, move |this, _, event, cx| {
                if let InputEvent::Committed(text) = event {
                    let value = (!text.trim().is_empty()).then(|| text.trim().to_string());
                    if index >= 4 {
                        let kind = crate::publication::report::TABLE_CAPTIONS[index - 4].0;
                        if let Some(value) = value {
                            this.publish
                                .settings
                                .table_captions
                                .insert(kind.into(), value);
                        } else {
                            this.publish.settings.table_captions.remove(kind);
                        }
                        cx.notify();
                        return;
                    }
                    let options = this.publish.settings.figures.entry(key.into()).or_default();
                    match index {
                        0 => options.title = value,
                        1 => options.xlabel = value,
                        2 => options.ylabel = value,
                        _ => options.caption = value,
                    }
                    this.refresh_publication_preview(cx);
                }
            })
            .detach();
            self.publish.labels.push(field);
        }
    }

    fn refresh_publication_preview(&mut self, cx: &mut Context<Self>) {
        self.publish.preview_generation += 1;
        self.publish.preview = None;
        self.publish.image = None;
        self.publish.error = None;
        self.publish.preview_running = false;
        let Some(data) = self.publish.figures.get(self.publish.selected).cloned() else {
            return;
        };
        let options = self.publish.settings.options(data.key);
        if let Err(error) = options.validate() {
            self.publish.error = Some(error);
            cx.notify();
            return;
        }
        let generation = self.publish.preview_generation;
        self.publish.preview_running = true;
        let timer = cx
            .background_executor()
            .timer(std::time::Duration::from_millis(120));
        cx.spawn(async move |this, cx| {
            timer.await;
            if !this
                .update(cx, |app, _| app.publish.preview_generation == generation)
                .unwrap_or(false)
            {
                return;
            }
            let result = cx
                .background_executor()
                .spawn(async move { render_figure(&data, &options) })
                .await;
            this.update(cx, |app, cx| {
                if app.publish.preview_generation != generation {
                    return;
                }
                app.publish.preview_running = false;
                match result {
                    Ok(rendered) => {
                        app.publish.image = Some(Arc::new(gpui::Image::from_bytes(
                            ImageFormat::Png,
                            rendered.png.clone(),
                        )));
                        app.publish.preview = Some(Arc::new(rendered));
                    }
                    Err(error) => app.publish.error = Some(error),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn save_publication_figure(&mut self, extension: &'static str, cx: &mut Context<Self>) {
        if !self.require_publication_source(cx) {
            return;
        }
        self.refresh_publication_source(cx);
        let Some(figure) = self.publish.figures.get(self.publish.selected) else {
            return;
        };
        let bytes = if extension == "csv" {
            match figure.csv(&self.publish.settings.options(figure.key)) {
                Ok(csv) => csv.into_bytes(),
                Err(error) => {
                    self.publish.error = Some(error);
                    cx.notify();
                    return;
                }
            }
        } else {
            let Some(rendered) = self
                .publish
                .preview
                .as_ref()
                .filter(|_| !self.publish.preview_running)
            else {
                return;
            };
            if extension == "svg" {
                rendered.svg.as_bytes().to_vec()
            } else {
                rendered.png.clone()
            }
        };
        let name = format!("rexafs-{}.{}", figure.key, extension);
        let home = crate::settings::home_dir().unwrap_or_else(std::env::temp_dir);
        let rx = cx.prompt_for_new_path(&home, Some(&name));
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(mut path))) = rx.await {
                if path.extension().is_none() {
                    path.set_extension(extension);
                }
                if path
                    .extension()
                    .and_then(|s| s.to_str())
                    .is_none_or(|s| !s.eq_ignore_ascii_case(extension))
                {
                    this.update(cx, |app, cx| {
                        app.publish.error =
                            Some(format!("Use the .{extension} extension for this format."));
                        cx.notify();
                    })
                    .ok();
                    return;
                }
                let result = cx
                    .background_executor()
                    .spawn(async move {
                        std::fs::write(&path, bytes)
                            .map(|()| path)
                            .map_err(|e| e.to_string())
                    })
                    .await;
                this.update(cx, |app, cx| {
                    match result {
                        Ok(path) => {
                            app.set_status(
                                crate::app::StatusKind::Success,
                                format!("Saved {}", path.display()),
                            );
                            app.publish.saved_figure = Some(path);
                        }
                        Err(error) => app.publish.error = Some(error),
                    }
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    pub(crate) fn publish_panel(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        self.refresh_publication_source(cx);
        let t = self.theme;
        let figure = self.publish.figures.get(self.publish.selected).cloned();
        let options = figure
            .as_ref()
            .map(|f| self.publish.settings.options(f.key))
            .unwrap_or_default();
        let ready = self.publication_ready() && !self.publish.running;
        let changed = self.publish.destination.is_some()
            && self
                .publish
                .changed_since_publish(&self.publication_revision());
        let header = div()
            .flex_none()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .text_size(px(12.5))
                            .font_weight(FontWeight::MEDIUM)
                            .child(self.report_scope()),
                    )
                    .child(
                        with_tip(
                            button(&t, "copy-analysis-record", "Copy analysis record", false),
                            &t,
                            "Copy the Markdown analysis record (analysis.md) to the clipboard.",
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.copy_analysis_record(cx))),
                    )
                    .children(self.manuscript_export_buttons(cx))
                    .child(
                        enabled_when(
                            with_tip(
                                button(
                                    &t,
                                    "publish-report",
                                    if self.publish.running {
                                        "Publishing…"
                                    } else if changed {
                                        "Publish changes…"
                                    } else {
                                        "Publish report…"
                                    },
                                    true,
                                ),
                                &t,
                                "Publish the current report to a new folder, preserving previous exports. Includes report.html, analysis.md, PNG/SVG/CSV figures, tables, captions, resolved data and the project file.",
                            ),
                            ready,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if ready {
                                this.export_publication(cx);
                            }
                        })),
                    ),
            )
            .when_some(self.publish.destination.clone(), |d, folder| {
                let url = file_url(&folder.join("report.html"));
                d.child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .text_size(px(11.))
                                .text_color(t.text_muted)
                                .child(if changed {
                                    format!("Changes since last publish · {}", self.publish.published_at.as_deref().unwrap_or(""))
                                } else {
                                    format!("Published {} · {}", self.publish.published_at.as_deref().unwrap_or(""), folder.display())
                                })
                                .when(changed, |d| d.text_color(t.warn)),
                        )
                        .child(
                            button(&t, "open-report", "Open last published report", false)
                                .on_click(cx.listener(move |_, _, _, cx| cx.open_url(&url))),
                        )
                        .child(
                            button(&t, "show-report-folder", "Show folder", false).on_click(
                                cx.listener(move |_, _, _, cx| cx.reveal_path(&folder)),
                            ),
                        ),
                )
            });
        let style_open = !self.ui.sections.contains("Publication style closed");
        let caption_open = self.ui.sections.contains("Publication caption");
        let mut controls = div()
            .id("publication-controls")
            .w(px(312.))
            .flex_none()
            .min_h_0()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_2()
            .pr_3();
        controls = controls.child(heading(&t, "Figures"));
        let more_figures = self.ui.sections.contains("Publication more figures");
        for common in [true, false] {
            if !common {
                controls = controls.child(
                    disclosure(
                        &t,
                        "publication-more-figures",
                        "More figures",
                        more_figures,
                        false,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        if !this.ui.sections.remove("Publication more figures") {
                            this.ui.sections.insert("Publication more figures");
                        }
                        cx.notify();
                    })),
                );
                if !more_figures {
                    continue;
                }
            }
            for (index, figure) in self
                .publish
                .figures
                .iter()
                .enumerate()
                .filter(|(_, f)| f.common() == common)
            {
                controls = controls.child(
                    chip(
                        &t,
                        SharedString::from(format!("publication-figure-{index}")),
                        figure.label.clone(),
                        index == self.publish.selected,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.publish.selected = index;
                        this.publication_fields(cx);
                        this.refresh_publication_preview(cx);
                    })),
                );
            }
        }
        if figure
            .as_ref()
            .is_some_and(|f| f.normalized_series.is_some())
        {
            let mut modes = segmented(&t);
            for (i, (normalized, label)) in [(false, "Flattened"), (true, "Normalized")]
                .into_iter()
                .enumerate()
            {
                modes = modes.child(
                    segment(
                        &t,
                        ("publication-energy", i),
                        label,
                        options.normalized == normalized,
                        i == 0,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(f) = this.publish.figures.get(this.publish.selected) {
                            this.publish
                                .settings
                                .figures
                                .entry(f.key.into())
                                .or_default()
                                .normalized = normalized;
                            this.publication_fields(cx);
                            this.refresh_publication_preview(cx);
                        }
                    })),
                );
            }
            controls = controls.child(div().flex().child(modes));
        }
        controls = controls.child(
            disclosure(
                &t,
                "publication-style",
                "Style",
                style_open,
                options.width.is_some()
                    || options.height.is_some()
                    || options.dpi.is_some()
                    || options.font_size.is_some()
                    || options.line_width.is_some()
                    || options.grid.is_some()
                    || options.guides
                    || !options.legend
                    || options.legend_position != LegendPlacement::Auto
                    || !options.hidden.is_empty()
                    || !options.shown.is_empty()
                    || options.xmin.is_some()
                    || options.ymin.is_some(),
            )
            .on_click(cx.listener(|this, _, _, cx| {
                if !this.ui.sections.remove("Publication style closed") {
                    this.ui.sections.insert("Publication style closed");
                }
                cx.notify();
            })),
        );
        if style_open {
            let mut presets = div().flex().flex_wrap().gap_1();
            for (i, preset) in StylePreset::ALL.into_iter().enumerate() {
                let (width, height, dpi, font) = preset.values();
                presets = presets.child(
                    with_tip(
                        preset_icon(&t, i, preset, preset.matches(&options)),
                        &t,
                        format!(
                            "{} · {width} × {height} in, {dpi} DPI, {font} pt text",
                            preset.label()
                        ),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(f) = this.publish.figures.get(this.publish.selected) {
                            preset.apply(
                                this.publish
                                    .settings
                                    .figures
                                    .entry(f.key.into())
                                    .or_default(),
                            );
                            this.publication_fields(cx);
                            this.refresh_publication_preview(cx);
                        }
                    })),
                );
            }
            controls = controls.child(presets);
            controls = controls.child(
                div()
                    .text_size(px(11.))
                    .text_color(t.text_muted)
                    .child("Editing this figure type · all report spectra"),
            );
            controls = controls.child(
                div()
                    .mt_1()
                    .flex()
                    .flex_wrap()
                    .gap_1()
                    .child(
                        with_tip(
                            button(
                                &t,
                                "publication-apply-style",
                                "Apply style to all figures",
                                false,
                            ),
                            &t,
                            "Copy size, DPI, font size, line width, legend and its placement, grid and guides from this figure to every figure in the report.",
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            let keys: Vec<&'static str> =
                                this.publish.figures.iter().map(|f| f.key).collect();
                            if let Some(source) =
                                this.publish.figures.get(this.publish.selected).map(|f| f.key)
                            {
                                this.publish.settings.apply_style_to_all(source, keys);
                            }
                            cx.notify();
                        })),
                    )
                    .child(
                        with_tip(
                            button(&t, "publication-reset", "Reset", false),
                            &t,
                            "Return this figure type to its defaults. Figure settings are saved with the project and used for every spectrum in the report.",
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            if let Some(f) = this.publish.figures.get(this.publish.selected) {
                                this.publish.settings.figures.remove(f.key);
                            }
                            this.publication_fields(cx);
                            this.refresh_publication_preview(cx);
                        })),
                    ),
            );
            for field in self.publish.numbers.iter().take(5) {
                controls = controls.child(field.clone());
            }
            controls = controls.child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_1()
                    .child(
                        chip(&t, "publication-grid", "Grid", options.grid.unwrap_or(true))
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Some(f) = this.publish.figures.get(this.publish.selected) {
                                    let o = this
                                        .publish
                                        .settings
                                        .figures
                                        .entry(f.key.into())
                                        .or_default();
                                    o.grid = Some(!o.grid.unwrap_or(true));
                                    this.refresh_publication_preview(cx);
                                }
                            })),
                    )
                    .child(
                        chip(&t, "publication-guides", "Guides", options.guides).on_click(
                            cx.listener(|this, _, _, cx| {
                                if let Some(f) = this.publish.figures.get(this.publish.selected) {
                                    let o = this
                                        .publish
                                        .settings
                                        .figures
                                        .entry(f.key.into())
                                        .or_default();
                                    o.guides = !o.guides;
                                    this.refresh_publication_preview(cx);
                                }
                            }),
                        ),
                    ),
            );
            controls = controls.child(heading(&t, "Legend"));
            let mut legend = div().flex().flex_wrap().gap_1();
            for (i, placement) in LegendPlacement::ALL
                .into_iter()
                .map(Some)
                .chain([None])
                .enumerate()
            {
                let selected = match placement {
                    Some(p) => options.legend && options.legend_position == p,
                    None => !options.legend,
                };
                let tip = match placement {
                    Some(LegendPlacement::Auto) => {
                        "Place the legend in the inside corner that covers the least of the curves, or above the plot when every corner covers data (recommended)."
                    }
                    Some(LegendPlacement::Outside) => {
                        "Place the legend to the right of the plot; the plot area narrows to make room."
                    }
                    Some(_) => "Place the legend in this corner of the plot.",
                    None => "Hide the legend.",
                };
                legend = legend.child(
                    with_tip(
                        chip(
                            &t,
                            ("publication-legend", i),
                            placement.map_or("Off", LegendPlacement::label),
                            selected,
                        ),
                        &t,
                        tip,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(f) = this.publish.figures.get(this.publish.selected) {
                            let o = this
                                .publish
                                .settings
                                .figures
                                .entry(f.key.into())
                                .or_default();
                            o.legend = placement.is_some();
                            if let Some(p) = placement {
                                o.legend_position = p;
                            }
                            this.refresh_publication_preview(cx);
                        }
                    })),
                );
            }
            controls = controls.child(legend);
            for (label, field) in ["Title", "X label", "Y label"]
                .into_iter()
                .zip(self.publish.labels.iter().take(3))
            {
                controls = controls.child(heading(&t, label)).child(field.clone());
            }
            controls = controls.child(
                div()
                    .text_size(px(11.))
                    .text_color(t.text_muted)
                    .child("Text is literal; Typst math in $…$, e.g. $k^2 chi(k)$"),
            );
            controls = controls.child(heading(&t, "Axis limits"));
            for field in self.publish.numbers.iter().skip(5) {
                controls = controls.child(field.clone());
            }
            controls = controls.child(heading(&t, "Curves"));
            if let Some(figure) = &figure {
                let mut curves = div().flex().flex_wrap().gap_1();
                for series in figure.series(&options) {
                    let key = series.key.clone();
                    let visible = series.visible(&options);
                    curves = curves.child(
                        chip(
                            &t,
                            SharedString::from(format!("publication-series-{key}")),
                            series.label.clone(),
                            visible,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(f) = this.publish.figures.get(this.publish.selected) {
                                let o = this
                                    .publish
                                    .settings
                                    .figures
                                    .entry(f.key.into())
                                    .or_default();
                                if visible {
                                    o.shown.remove(&key);
                                    o.hidden.insert(key.clone());
                                } else {
                                    o.hidden.remove(&key);
                                    o.shown.insert(key.clone());
                                }
                                this.refresh_publication_preview(cx);
                            }
                        })),
                    );
                }
                controls = controls.child(curves);
            }
        }
        controls = controls.child(
            disclosure(
                &t,
                "publication-caption",
                "Caption",
                caption_open,
                options.caption.is_some(),
            )
            .on_click(cx.listener(|this, _, _, cx| {
                if !this.ui.sections.remove("Publication caption") {
                    this.ui.sections.insert("Publication caption");
                }
                cx.notify();
            })),
        );
        if caption_open {
            for (label, field) in [
                "Figure caption · Enter to apply",
                "Processing table caption",
                "Fit parameters caption",
                "Path results caption",
            ]
            .into_iter()
            .zip(self.publish.labels.iter().skip(3))
            {
                controls = controls.child(heading(&t, label)).child(field.clone());
            }
        }
        let (width, height, dpi) = options.dimensions();
        let preview_ready = self.publish.preview.is_some() && !self.publish.preview_running;
        let mut save = div().flex().items_center().gap_1().child(
            div()
                .text_size(px(11.5))
                .text_color(t.text_muted)
                .child("Save figure"),
        );
        let formats: [(&'static str, &'static str, bool, &'static str); 3] = [
            (
                "png",
                "PNG",
                preview_ready,
                "Save this figure as a PNG image at the size and DPI above.",
            ),
            (
                "svg",
                "SVG",
                preview_ready,
                "Save this figure as an SVG vector image.",
            ),
            (
                "csv",
                "CSV",
                figure.is_some(),
                "Save the visible curves as CSV. Axis limits do not crop the data.",
            ),
        ];
        for (i, (extension, label, enabled, tip)) in formats.into_iter().enumerate() {
            let enabled = enabled && ready;
            save = save.child(
                enabled_when(
                    with_tip(button(&t, ("save-figure", i), label, false), &t, tip),
                    enabled,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    if enabled {
                        this.save_publication_figure(extension, cx);
                    }
                })),
            );
        }
        if let Some(path) = self.publish.saved_figure.clone() {
            save = save.child(
                icon_button(
                    &t,
                    "show-saved-figure",
                    Icon::Folder,
                    "Show saved figure",
                    false,
                )
                .on_click(cx.listener(move |_, _, _, cx| cx.reveal_path(&path))),
            );
        }
        let mut preview = div()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(12.))
                            .text_color(t.text_muted)
                            .child(format!("{width:.2} × {height:.2} in · {dpi:.0} DPI")),
                    )
                    .child(save),
            );
        let mut canvas = div()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .border_1()
            .border_color(t.border)
            .bg(gpui::white())
            .flex()
            .items_center()
            .justify_center()
            .overflow_hidden();
        if let Some(image) = &self.publish.image {
            canvas = canvas.child(
                img(image.clone())
                    .size_full()
                    .object_fit(ObjectFit::Contain),
            );
        } else {
            canvas = canvas.child(div().p_4().text_color(gpui::rgb(0x606770)).child(
                if self.publish.preview_running {
                    "Rendering figure…"
                } else if self.publish.figures.is_empty() {
                    "Open and process a spectrum to prepare a figure."
                } else {
                    "Adjust the settings to preview this figure."
                },
            ));
        }
        preview = preview.child(canvas);
        if let Some(figure) = &figure {
            preview = preview.when(caption_open, |d| {
                d.child(div().text_size(px(12.)).child(figure.caption(&options)))
            });
            preview = preview.child(
                icon_button(&t, "copy-figure-caption", Icon::Copy, "Copy caption", false).on_click(
                    cx.listener(|this, _, _, cx| {
                        if let Some(f) = this.publish.figures.get(this.publish.selected) {
                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                f.caption(&this.publish.settings.options(f.key)),
                            ));
                        }
                    }),
                ),
            );
        }
        if let Some(error) = &self.publish.error {
            preview = preview.child(
                div()
                    .text_size(px(12.))
                    .text_color(t.error)
                    .child(error.clone()),
            );
        }
        div()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .p_3()
            .gap_2()
            .child(header)
            .when(self.fit_result.is_some() && self.fit_is_stale(), |d| {
                d.child(
                    div().text_size(px(12.)).text_color(t.warn).child(
                        "Fit figures show the previous result; rerun the fit to update them.",
                    ),
                )
            })
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .gap_4()
                    .child(controls)
                    .child(preview),
            )
            .into_any_element()
    }
}
