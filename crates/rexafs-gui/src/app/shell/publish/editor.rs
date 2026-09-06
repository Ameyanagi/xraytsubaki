use super::*;
use crate::app::shell::{button, chip};
use crate::publication::figures::{fit_figures, render_figure, spectrum_figures};
use crate::widgets::numeric_field::{FieldEvent, FieldKind};
use crate::widgets::text_input::InputEvent;
use gpui::{
    ClickEvent, ImageFormat, IntoElement, ObjectFit, SharedString, Styled, div, img, prelude::*, px,
};

impl StudioApp {
    fn refresh_publication_source(&mut self, cx: &mut Context<Self>) {
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
            format!("{}:{:?}", self.current_group_label(), self.theme.mode),
        );
        if self.publish.source.as_ref() == Some(&source) {
            return;
        }
        self.publish.source = Some(source);
        let mut figures = self
            .spectrum
            .as_ref()
            .map(|sp| spectrum_figures(sp.clone(), &self.current_group_label().to_string()))
            .unwrap_or_default();
        if let Some(result) = &self.fit_result {
            figures.extend(fit_figures(&crate::joint_fitting::result_view(
                result,
                self.joint.result_index,
            )));
        }
        self.publish.figures = figures.into_iter().map(Arc::new).collect();
        self.publish.selected = self
            .publish
            .selected
            .min(self.publish.figures.len().saturating_sub(1));
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
            (
                "DPI",
                options.dpi,
                format!("Auto ({:.0})", config.figure.dpi),
            ),
            ("Font size (pt)", options.font_size, "Auto".into()),
            ("Line width (pt)", options.line_width, "Auto".into()),
            ("X min", options.xmin, "Auto".into()),
            ("X max", options.xmax, "Auto".into()),
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
            ("No title".to_string(), options.title),
            (figure.xlabel.clone(), options.xlabel),
            (figure.ylabel.clone(), options.ylabel),
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

    fn save_publication_figure(&mut self, svg: bool, cx: &mut Context<Self>) {
        let Some(rendered) = self.publish.preview.clone() else {
            return;
        };
        let Some(figure) = self.publish.figures.get(self.publish.selected) else {
            return;
        };
        let extension = if svg { "svg" } else { "png" };
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
                        let bytes = if svg {
                            rendered.svg.as_bytes()
                        } else {
                            &rendered.png
                        };
                        std::fs::write(&path, bytes)
                            .map(|()| path)
                            .map_err(|e| e.to_string())
                    })
                    .await;
                this.update(cx, |app, cx| {
                    match result {
                        Ok(path) => {
                            app.status = format!("Saved {}", path.display()).into();
                            app.publish.destination = Some(path);
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
        let options = self
            .publish
            .figures
            .get(self.publish.selected)
            .map(|f| self.publish.settings.options(f.key))
            .unwrap_or_default();
        let ready = self.publish.preview.is_some() && !self.publish.preview_running;
        let header =
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .flex_1()
                        .text_size(px(22.))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child("Publish"),
                )
                .child(button(&t, "save-figure-png", "Save PNG…", ready).on_click(
                    cx.listener(|this, _, _, cx| this.save_publication_figure(false, cx)),
                ))
                .child(button(&t, "save-figure-svg", "Save SVG…", ready).on_click(
                    cx.listener(|this, _, _, cx| this.save_publication_figure(true, cx)),
                ));
        let mut controls = div()
            .id("publication-controls")
            .w(px(268.))
            .flex_none()
            .min_h_0()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_2()
            .pr_3();
        controls = controls.child(
            div()
                .text_color(t.text_muted)
                .text_size(px(12.))
                .child("Figure · current spectrum / fit"),
        );
        for (index, figure) in self.publish.figures.iter().enumerate() {
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
        controls = controls.child(
            div()
                .mt_3()
                .text_size(px(12.))
                .text_color(t.text_muted)
                .child("Size & style · blank = ruviz default"),
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
                    chip(&t, "publication-legend", "Legend", options.legend).on_click(cx.listener(
                        |this, _, _, cx| {
                            if let Some(f) = this.publish.figures.get(this.publish.selected) {
                                let o = this
                                    .publish
                                    .settings
                                    .figures
                                    .entry(f.key.into())
                                    .or_default();
                                o.legend = !o.legend;
                                this.refresh_publication_preview(cx);
                            }
                        },
                    )),
                )
                .child(
                    chip(
                        &t,
                        "publication-grid",
                        "Grid",
                        options
                            .grid
                            .unwrap_or(ruviz::core::GridStyle::default().visible),
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(f) = this.publish.figures.get(this.publish.selected) {
                            let o = this
                                .publish
                                .settings
                                .figures
                                .entry(f.key.into())
                                .or_default();
                            o.grid =
                                Some(!o.grid.unwrap_or(ruviz::core::GridStyle::default().visible));
                            this.refresh_publication_preview(cx);
                        }
                    })),
                )
                .child(
                    chip(&t, "publication-guides", "Guides", options.guides).on_click(cx.listener(
                        |this, _, _, cx| {
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
                        },
                    )),
                ),
        );
        for (label, field) in [
            "Title",
            "X label",
            "Y label",
            "Figure caption · Enter to apply",
            "Processing table caption",
            "Fit parameters caption",
            "Path results caption",
        ]
        .into_iter()
        .zip(&self.publish.labels)
        {
            controls = controls
                .child(
                    div()
                        .mt_1()
                        .text_size(px(12.))
                        .text_color(t.text_muted)
                        .child(label),
                )
                .child(field.clone());
        }
        controls = controls.child(
            div()
                .mt_2()
                .text_size(px(12.))
                .text_color(t.text_muted)
                .child("Axis limits · set or clear each pair"),
        );
        for field in self.publish.numbers.iter().skip(5) {
            controls = controls.child(field.clone());
        }
        controls = controls.child(
            div()
                .mt_2()
                .text_size(px(12.))
                .text_color(t.text_muted)
                .child("Visible curves"),
        );
        if let Some(figure) = self.publish.figures.get(self.publish.selected) {
            for series in &figure.series {
                let key = series.key.clone();
                controls = controls.child(
                    chip(
                        &t,
                        SharedString::from(format!("publication-series-{key}")),
                        series.label.clone(),
                        !options.hidden.contains(&key),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(f) = this.publish.figures.get(this.publish.selected) {
                            let o = this
                                .publish
                                .settings
                                .figures
                                .entry(f.key.into())
                                .or_default();
                            if !o.hidden.remove(&key) {
                                o.hidden.insert(key.clone());
                            }
                            this.refresh_publication_preview(cx);
                        }
                    })),
                );
            }
        }
        controls=controls.child(button(&t,"publication-reset","Reset figure to defaults",false).on_click(cx.listener(|this,_,_,cx| {
            if let Some(f)=this.publish.figures.get(this.publish.selected) { this.publish.settings.figures.remove(f.key); }
            this.publication_fields(cx);this.refresh_publication_preview(cx);
        }))).child(div().text_size(px(11.)).text_color(t.text_muted).child("Settings apply to this figure type in the export folder and are saved with the project."));
        let (width, height, dpi) = options.dimensions();
        let mut preview = div()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(t.text_muted)
                    .child(format!(
                        "{width:.2} × {height:.2} in · {dpi:.0} DPI · preview scaled to fit"
                    )),
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
        if let Some(figure) = self.publish.figures.get(self.publish.selected) {
            preview = preview.child(div().text_size(px(12.)).child(figure.caption(&options)));
            preview = preview.child(
                button(&t, "copy-figure-caption", "Copy caption", false).on_click(cx.listener(
                    |this, _, _, cx| {
                        if let Some(f) = this.publish.figures.get(this.publish.selected) {
                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                f.caption(&this.publish.settings.options(f.key)),
                            ));
                        }
                    },
                )),
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
        let footer = div()
            .flex()
            .flex_wrap()
            .gap_2()
            .items_center()
            .child(
                button(
                    &t,
                    "publish-export",
                    if self.publish.running {
                        "Exporting…"
                    } else {
                        "Export analysis folder…"
                    },
                    false,
                )
                .on_click(cx.listener(|this, _, _, cx| this.export_publication(cx))),
            )
            .child(
                button(&t, "copy-analysis-markdown", "Copy Markdown", false).on_click(cx.listener(
                    |this, _: &ClickEvent, _, cx| {
                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                            this.analysis_snapshot().markdown(),
                        ));
                        this.status = "Analysis record copied".into();
                        cx.notify();
                    },
                )),
            )
            .when_some(self.publish.destination.clone(), |d, path| {
                d.child(
                    button(&t, "open-publication", "Show saved output", false)
                        .on_click(cx.listener(move |_, _, _, cx| cx.reveal_path(&path))),
                )
            })
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(t.text_muted)
                    .child("PNG + SVG · captioned report & tables · methods · project · arrays"),
            );
        div()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .p_4()
            .gap_3()
            .child(header)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .gap_4()
                    .child(controls)
                    .child(preview),
            )
            .child(footer)
            .into_any_element()
    }
}
