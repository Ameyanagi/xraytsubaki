//! Publish · Multi-panel: compose several publication figures into one
//! labelled grid at a journal column width (see
//! `publication::figures::composer`). The recipe is saved with the figure
//! settings in the project.
use super::editor::{enabled_when, with_tip};
use super::*;
use crate::app::shell::{button, chip, controls::icon_button, segment, segmented};
use crate::icons::Icon;
use crate::publication::figures::composer::{
    CompositeRecipe, JOURNAL_WIDTHS, MAX_PANELS, MM_PER_INCH, compose, panel_label,
};
use crate::theme::Theme;
use crate::widgets::numeric_field::{FieldEvent, FieldKind};
use gpui::{ImageFormat, ObjectFit, SharedString, div, img, prelude::*, px};

/// Logical pixels per inch used by the true-size preview: the CSS reference
/// of 96 px per inch. A display's physical pixel density differs, so the
/// preview is close to, not exactly, the printed size.
const TRUE_SIZE_PX_PER_INCH: f32 = 96.;
/// Highest preview resolution; exports always use the recipe's DPI.
const PREVIEW_DPI: f64 = 200.;

/// Session state of the multi-panel composer. The recipe itself lives in
/// `PublishState::settings.composite`.
#[derive(Default)]
pub(crate) struct ComposerState {
    /// The Publish stage shows the composer instead of the single-figure editor.
    pub open: bool,
    /// Show the preview at about its printed size instead of fitting the pane.
    true_size: bool,
    fields: Vec<Entity<NumericField>>,
    /// Figures and settings that produced the current preview.
    source: Option<(Vec<usize>, Vec<u8>)>,
    generation: u64,
    running: bool,
    image: Option<Arc<gpui::Image>>,
    /// Physical size in millimetres of the previewed figure.
    size_mm: (f64, f64),
    missing: Vec<String>,
    caption: String,
    error: Option<String>,
}

/// Small muted heading above a group of controls.
fn heading(t: &Theme, text: &'static str) -> gpui::Div {
    div()
        .mt_1()
        .text_size(px(12.))
        .text_color(t.text_muted)
        .child(text)
}

impl StudioApp {
    fn composite(&mut self) -> &mut CompositeRecipe {
        &mut self.publish.settings.composite
    }

    /// Single figure | Multi-panel switch shown above both editors.
    pub(super) fn publish_mode_switch(&mut self, cx: &mut Context<Self>) -> gpui::Div {
        let t = self.theme;
        let mut modes = segmented(&t);
        for (i, (composer, label, tip)) in [
            (
                false,
                "Single figure",
                "Edit and save one figure type at a time.",
            ),
            (
                true,
                "Multi-panel",
                "Compose several figures into one labelled grid, (a), (b), …, at a journal column width.",
            ),
        ]
        .into_iter()
        .enumerate()
        {
            modes = modes.child(
                with_tip(
                    segment(
                        &t,
                        ("publication-mode", i),
                        label,
                        self.publish.composer.open == composer,
                        i == 0,
                    ),
                    &t,
                    tip,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.publish.composer.open = composer;
                    if composer {
                        if this.publish.settings.composite.panels.is_empty() {
                            this.publish.settings.composite.panels =
                                CompositeRecipe::suggested_panels(
                                    this.publish.figures.iter().map(|f| f.key),
                                );
                        }
                        this.composer_fields(cx);
                        this.publish.composer.source = None;
                    }
                    cx.notify();
                })),
            );
        }
        div().flex().child(modes)
    }

    /// Rebuild the numeric fields from the current recipe.
    fn composer_fields(&mut self, cx: &mut Context<Self>) {
        let recipe = self.publish.settings.composite.clone();
        let auto_height = recipe
            .grid(recipe.panels.len().max(1), recipe.dpi)
            .cell_px
            .1 as f64
            / recipe.dpi
            * MM_PER_INCH;
        let defaults = CompositeRecipe::default();
        let values = [
            (
                "Width (mm)",
                Some(recipe.width_mm),
                format!("{:.1}", defaults.width_mm),
            ),
            (
                "Panel height (mm)",
                recipe.panel_height_mm,
                format!("Auto ({auto_height:.1})"),
            ),
            (
                "Font size (pt)",
                Some(recipe.font_size),
                format!("{}", defaults.font_size),
            ),
            ("Line width (pt)", recipe.line_width, "Auto".into()),
            ("DPI", Some(recipe.dpi), format!("{}", defaults.dpi)),
        ];
        self.publish.composer.fields.clear();
        for (index, (label, value, placeholder)) in values.into_iter().enumerate() {
            let theme = self.theme;
            let field = cx.new(|cx| {
                NumericField::new(label, placeholder, value, FieldKind::Float, theme, cx)
            });
            cx.subscribe(&field, move |this, _, event, cx| match event {
                FieldEvent::Changed(value) => {
                    let defaults = CompositeRecipe::default();
                    let recipe = this.composite();
                    match index {
                        0 => recipe.width_mm = value.unwrap_or(defaults.width_mm),
                        1 => recipe.panel_height_mm = *value,
                        2 => recipe.font_size = value.unwrap_or(defaults.font_size),
                        3 => recipe.line_width = *value,
                        _ => recipe.dpi = value.unwrap_or(defaults.dpi),
                    }
                    cx.notify();
                }
                FieldEvent::Invalid(message) => {
                    this.publish.composer.error = Some(message.to_string());
                    cx.notify();
                }
                _ => (),
            })
            .detach();
            self.publish.composer.fields.push(field);
        }
    }

    /// Re-render the preview when the recipe, any figure setting or the
    /// available figures changed since the last preview.
    fn refresh_composer_preview(&mut self, cx: &mut Context<Self>) {
        let figures = self.publish.figures.clone();
        let settings = self.publish.settings.clone();
        let source = (
            figures.iter().map(|f| Arc::as_ptr(f) as usize).collect(),
            serde_json::to_vec(&settings).unwrap_or_default(),
        );
        let state = &mut self.publish.composer;
        if state.source.as_ref() == Some(&source) {
            return;
        }
        state.source = Some(source);
        state.generation += 1;
        state.running = true;
        state.error = None;
        let generation = state.generation;
        let timer = cx
            .background_executor()
            .timer(std::time::Duration::from_millis(150));
        cx.spawn(async move |this, cx| {
            timer.await;
            if !this
                .update(cx, |app, _| app.publish.composer.generation == generation)
                .unwrap_or(false)
            {
                return;
            }
            let result = cx
                .background_executor()
                .spawn(async move {
                    let dpi = settings.composite.dpi.min(PREVIEW_DPI);
                    let composed = compose(&settings.composite, &figures, &settings, Some(dpi))?;
                    Ok::<_, String>((composed.png()?, composed))
                })
                .await;
            this.update(cx, |app, cx| {
                let state = &mut app.publish.composer;
                if state.generation != generation {
                    return;
                }
                state.running = false;
                match result {
                    Ok((png, composed)) => {
                        state.image =
                            Some(Arc::new(gpui::Image::from_bytes(ImageFormat::Png, png)));
                        state.size_mm = composed.grid.size_mm();
                        state.missing = composed.missing;
                        state.caption = composed.caption;
                    }
                    Err(error) => {
                        state.image = None;
                        state.error = Some(error);
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Render the recipe at its full DPI and save it as `extension`
    /// (png, svg, pdf or tiff).
    fn save_composite(&mut self, extension: &'static str, cx: &mut Context<Self>) {
        if !self.require_publication_source(cx) {
            return;
        }
        let figures = self.publish.figures.clone();
        let settings = self.publish.settings.clone();
        let home = crate::settings::home_dir().unwrap_or_else(std::env::temp_dir);
        let name = format!("rexafs-multi-panel.{extension}");
        let rx = cx.prompt_for_new_path(&home, Some(&name));
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(mut path))) = rx.await else {
                return;
            };
            if path.extension().is_none() {
                path.set_extension(extension);
            }
            let result = cx
                .background_executor()
                .spawn(async move {
                    let bytes = compose(&settings.composite, &figures, &settings, None)?
                        .bytes(extension)?;
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
                    Err(error) => app.publish.composer.error = Some(error),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// The composer's controls and preview, below the shared Publish header.
    pub(super) fn composer_panel(
        &mut self,
        header: gpui::Div,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        if self.publish.composer.fields.is_empty() {
            self.composer_fields(cx);
        }
        self.refresh_composer_preview(cx);
        let t = self.theme;
        let recipe = self.publish.settings.composite.clone();
        let ready = self.publication_ready();
        let mut controls = div()
            .id("composer-controls")
            .w(px(312.))
            .flex_none()
            .min_h_0()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_2()
            .pr_3()
            .child(self.publish_mode_switch(cx))
            .child(heading(&t, "Panels"));

        let available = |key: &str| self.publish.figures.iter().find(|f| f.key == key);
        let mut label_index = 0;
        for (index, key) in recipe.panels.iter().enumerate() {
            let figure = available(key);
            let label = match figure {
                Some(figure) => {
                    label_index += 1;
                    format!("{} {}", panel_label(label_index - 1), figure.label)
                }
                None => format!("{key} · unavailable"),
            };
            controls = controls.child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .text_size(px(12.))
                            .when(figure.is_none(), |d| d.text_color(t.text_muted))
                            .child(label),
                    )
                    .child(
                        enabled_when(
                            icon_button(
                                &t,
                                ("composer-panel-up", index),
                                Icon::ArrowUp,
                                "Move panel earlier",
                                false,
                            ),
                            index > 0,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if index > 0 {
                                this.composite().panels.swap(index, index - 1);
                                cx.notify();
                            }
                        })),
                    )
                    .child(
                        icon_button(
                            &t,
                            ("composer-panel-remove", index),
                            Icon::Close,
                            "Remove panel",
                            false,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.composite().panels.remove(index);
                            cx.notify();
                        })),
                    ),
            );
        }
        if recipe.panels.is_empty() {
            controls = controls.child(
                div()
                    .text_size(px(11.))
                    .text_color(t.text_muted)
                    .child("Add figures below; each becomes a labelled panel."),
            );
        }
        controls = controls.child(heading(&t, "Add panel"));
        let can_add = recipe.panels.len() < MAX_PANELS;
        let mut add = div().flex().flex_wrap().gap_1();
        for figure in self.publish.figures.iter() {
            let key = figure.key;
            add = add.child(
                enabled_when(
                    chip(
                        &t,
                        SharedString::from(format!("composer-add-{key}")),
                        figure.label.clone(),
                        recipe.panels.iter().any(|p| p == key),
                    ),
                    can_add,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    if this.composite().panels.len() < MAX_PANELS {
                        this.composite().panels.push(key.into());
                        cx.notify();
                    }
                })),
            );
        }
        controls = controls.child(add);

        controls = controls.child(heading(&t, "Journal width"));
        let mut widths = div().flex().flex_wrap().gap_1();
        for (i, width) in JOURNAL_WIDTHS.into_iter().enumerate() {
            widths = widths.child(
                with_tip(
                    chip(
                        &t,
                        ("composer-width", i),
                        width.label(),
                        (recipe.width_mm - width.mm).abs() < 1e-9,
                    ),
                    &t,
                    format!(
                        "{} {}: up to {:.1} mm ({:.2} in) wide. Check the journal's current artwork guide.",
                        width.publisher,
                        width.span,
                        width.mm,
                        width.mm / MM_PER_INCH
                    ),
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.composite().width_mm = width.mm;
                    this.composer_fields(cx);
                    cx.notify();
                })),
            );
        }
        controls = controls.child(widths);
        controls = controls.child(heading(&t, "Columns"));
        let mut columns = segmented(&t);
        for n in 1..=4usize {
            columns = columns.child(
                segment(
                    &t,
                    ("composer-columns", n),
                    n.to_string(),
                    recipe.columns == n,
                    n == 1,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.composite().columns = n;
                    this.composer_fields(cx);
                    cx.notify();
                })),
            );
        }
        controls = controls.child(div().flex().child(columns));
        controls = controls.child(heading(&t, "Shared style"));
        for field in &self.publish.composer.fields {
            controls = controls.child(field.clone());
        }
        controls = controls.child(
            div().text_size(px(11.)).text_color(t.text_muted).child(
                "Size, font, line width and DPI apply to every panel. Axis labels, limits, curves, legend, grid and guides come from each figure's Single figure settings; titles are replaced by panel labels.",
            ),
        );

        // Preview column.
        let state = &self.publish.composer;
        let (width_mm, height_mm) = state.size_mm;
        let can_save = ready && state.image.is_some() && !state.running;
        let mut save = div().flex().items_center().gap_1().child(
            div()
                .text_size(px(11.5))
                .text_color(t.text_muted)
                .child("Save figure"),
        );
        for (i, (extension, label, tip)) in [
            ("png", "PNG", "Save a PNG image at the DPI above."),
            ("tiff", "TIFF", "Save an LZW-compressed TIFF image at the DPI above, with the resolution recorded in the file."),
            ("svg", "SVG", "Save a vector SVG at the printed size; plain labels stay editable text."),
            ("pdf", "PDF", "Save a vector PDF whose page is the printed figure size."),
        ]
        .into_iter()
        .enumerate()
        {
            save = save.child(
                enabled_when(
                    with_tip(button(&t, ("composer-save", i), label, false), &t, tip),
                    can_save,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    if can_save {
                        this.save_composite(extension, cx);
                    }
                })),
            );
        }
        let mut zoom = segmented(&t);
        for (i, (true_size, label, tip)) in [
            (false, "Fit", "Fit the preview to the pane."),
            (
                true,
                "True size",
                "Show the figure at about its printed size (96 pixels per inch); the exact size depends on the display.",
            ),
        ]
        .into_iter()
        .enumerate()
        {
            zoom = zoom.child(
                with_tip(
                    segment(
                        &t,
                        ("composer-zoom", i),
                        label,
                        state.true_size == true_size,
                        i == 0,
                    ),
                    &t,
                    tip,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.publish.composer.true_size = true_size;
                    cx.notify();
                })),
            );
        }
        let mut canvas = div()
            .id("composer-canvas")
            .flex_1()
            .min_w_0()
            .min_h_0()
            .border_1()
            .border_color(t.border)
            .bg(gpui::white())
            .flex()
            .overflow_hidden();
        match &state.image {
            Some(image) if state.true_size => {
                let scale = TRUE_SIZE_PX_PER_INCH / MM_PER_INCH as f32;
                canvas = canvas.overflow_scroll().child(
                    img(image.clone())
                        .flex_none()
                        .w(px(width_mm as f32 * scale))
                        .h(px(height_mm as f32 * scale)),
                );
            }
            Some(image) => {
                canvas = canvas.items_center().justify_center().child(
                    img(image.clone())
                        .size_full()
                        .object_fit(ObjectFit::Contain),
                );
            }
            None => {
                canvas = canvas.items_center().justify_center().child(
                    div()
                        .p_4()
                        .text_color(gpui::rgb(0x606770))
                        .child(if state.running {
                            "Rendering figure…"
                        } else if self.publish.figures.is_empty() {
                            "Open and process a spectrum to prepare a figure."
                        } else {
                            "Add panels to compose a figure."
                        }),
                );
            }
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
                            .child(format!(
                                "{width_mm:.1} × {height_mm:.1} mm · {:.0} DPI",
                                recipe.dpi
                            )),
                    )
                    .child(zoom)
                    .child(save),
            )
            .child(canvas);
        if !state.missing.is_empty() {
            preview = preview.child(div().text_size(px(12.)).text_color(t.warn).child(format!(
                "Skipped unavailable panels: {}. Run a fit or process a spectrum to fill them.",
                state.missing.join(", ")
            )));
        }
        if !state.caption.is_empty() {
            let caption = state.caption.clone();
            preview = preview.child(
                div()
                    .flex()
                    .items_start()
                    .gap_1()
                    .child(div().flex_1().text_size(px(12.)).child(caption.clone()))
                    .child(
                        icon_button(
                            &t,
                            "copy-composite-caption",
                            Icon::Copy,
                            "Copy caption",
                            false,
                        )
                        .on_click(cx.listener(move |_, _, _, cx| {
                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(caption.clone()));
                        })),
                    ),
            );
        }
        if let Some(error) = &state.error {
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
