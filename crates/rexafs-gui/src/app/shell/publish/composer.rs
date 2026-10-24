//! Publish · Multi-panel: compose several publication figures into one
//! labelled grid at a journal column width (see
//! `publication::figures::composer`). Named recipes are saved with the figure
//! settings in the project.
use super::editor::{enabled_when, with_tip};
use super::*;
use crate::accessibility::Control;
use crate::app::shell::{button, chip, controls::icon_button, segment, segmented};
use crate::icons::Icon;
use crate::publication::figures::composer::{
    CompositeRecipe, JOURNAL_WIDTHS, MAX_PANELS, MM_PER_INCH, compose, panel_label,
};
use crate::theme::Theme;
use crate::widgets::numeric_field::{FieldEvent, FieldKind};
use crate::widgets::text_input::InputEvent;
use gpui::{Focusable, ImageFormat, ObjectFit, SharedString, Window, div, img, prelude::*, px};

/// Logical pixels per inch used by the true-size preview: the CSS reference
/// of 96 px per inch. A display's physical pixel density differs, so the
/// preview is close to, not exactly, the printed size.
const TRUE_SIZE_PX_PER_INCH: f32 = 96.;
/// Highest preview resolution; exports always use the recipe's DPI.
const PREVIEW_DPI: f64 = 200.;

/// Session state of the multi-panel composer. Named recipes live in
/// `PublishState::settings`; selection changes discard the preview immediately.
#[derive(Default)]
pub(crate) struct ComposerState {
    /// The Publish stage shows the composer instead of the single-figure editor.
    pub open: bool,
    /// Show the preview at about its printed size instead of fitting the pane.
    true_size: bool,
    /// Whether the named-recipe choice list is expanded.
    recipe_menu: bool,
    /// Inline name draft; Save applies it after validation, Cancel discards it.
    rename: Option<Entity<TextInput>>,
    /// Validation feedback for the current inline name draft.
    name_error: Option<String>,
    /// Index and name captured by the in-app recipe deletion confirmation.
    delete_recipe: Option<(usize, String)>,
    fields: Vec<Entity<NumericField>>,
    /// Reject events from fields replaced after a recipe or layout change.
    fields_generation: u64,
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

impl ComposerState {
    /// Discard all recipe-specific results and reject older background renders.
    fn invalidate_preview(&mut self) {
        self.source = None;
        self.generation += 1;
        self.running = false;
        self.image = None;
        self.size_mm = (0., 0.);
        self.missing.clear();
        self.caption.clear();
        self.error = None;
    }

    /// Reset session controls after a project load while rejecting old renders.
    pub(super) fn reset(&mut self) {
        let generation = self.generation + 1;
        let fields_generation = self.fields_generation + 1;
        *self = Self {
            generation,
            fields_generation,
            ..Default::default()
        };
    }
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
    /// Edit only the selected recipe, retaining every other saved recipe.
    fn composite(&mut self) -> &mut CompositeRecipe {
        self.publish.settings.active_composite_mut()
    }

    /// Cancel in-flight rendering and clear all results tied to the old recipe.
    fn invalidate_composer_preview(&mut self) {
        self.publish.composer.invalidate_preview();
    }

    /// Rebuild editor fields after selecting, creating or removing a recipe.
    fn composer_recipe_changed(&mut self, cx: &mut Context<Self>) {
        self.publish.composer.recipe_menu = false;
        self.publish.composer.rename = None;
        self.publish.composer.name_error = None;
        self.publish.composer.delete_recipe = None;
        self.invalidate_composer_preview();
        self.composer_fields(cx);
        cx.notify();
    }

    /// Begin an inline name draft and select its text for immediate replacement.
    fn rename_composer_recipe(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.publish.settings.active_recipe().name.clone();
        let theme = self.theme;
        let field = cx.new(|cx| {
            let mut field = TextInput::new("Recipe name", name, theme, cx);
            field.set_accessible_name("Recipe name");
            field.select_all_text(cx);
            field
        });
        cx.subscribe(&field, |this, field, event, cx| {
            if this
                .publish
                .composer
                .rename
                .as_ref()
                .is_none_or(|current| current.entity_id() != field.entity_id())
            {
                return;
            }
            // TextInput also commits on blur. Keep Save explicit so clicking
            // Cancel or selecting another recipe never applies a name draft.
            if let InputEvent::Edited(text) = event {
                this.publish.composer.name_error =
                    this.publish.settings.validate_recipe_name(text).err();
                cx.notify();
            }
        })
        .detach();
        field.read(cx).focus_handle(cx).focus(window, cx);
        self.publish.composer.rename = Some(field);
        self.publish.composer.name_error = None;
        self.publish.composer.recipe_menu = false;
        self.publish.composer.delete_recipe = None;
        cx.notify();
    }

    /// Apply the inline name after model validation; retain invalid drafts.
    fn save_composer_recipe_name(&mut self, cx: &mut Context<Self>) {
        let Some(field) = &self.publish.composer.rename else {
            return;
        };
        let name = field.read(cx).text().to_string();
        match self.publish.settings.rename_recipe(&name) {
            Ok(()) => {
                self.publish.composer.rename = None;
                self.publish.composer.name_error = None;
            }
            Err(error) => self.publish.composer.name_error = Some(error),
        }
        cx.notify();
    }

    /// Named recipe selection and management, using the shell's compact controls.
    fn composer_recipe_controls(&self, cx: &mut Context<Self>) -> gpui::Div {
        let t = self.theme;
        let state = &self.publish.composer;
        let active = self.publish.settings.active_recipe_index();
        let name = self.publish.settings.active_recipe().name.clone();
        let can_delete = self.publish.settings.recipes().len() > 1;
        let mut controls = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(heading(&t, "Recipe"))
            .child(
                Control::new(
                    div().id("composer-recipe-selector"),
                    format!("Recipe: {name}"),
                    accesskit::Role::ComboBox,
                )
                .expanded(state.recipe_menu)
                .tab_index(0)
                .key_context("Control")
                .min_w_0()
                .h(px(30.))
                .px_2()
                .flex()
                .items_center()
                .gap_2()
                .rounded_sm()
                .border_1()
                .border_color(t.border)
                .bg(t.surface)
                .text_size(px(12.))
                .cursor_pointer()
                .hover(|d| d.bg(t.raised))
                .focus(|d| d.border_color(t.accent))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .child(name),
                )
                .child(crate::app::shell::controls::icon(&t, Icon::ChevronDown))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.publish.composer.recipe_menu = !this.publish.composer.recipe_menu;
                    this.publish.composer.delete_recipe = None;
                    cx.notify();
                })),
            );
        if state.recipe_menu {
            let mut choices = div()
                .id("composer-recipe-choices")
                .max_h(px(180.))
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .gap_1()
                .p_1()
                .border_1()
                .border_color(t.border)
                .rounded_sm();
            for (index, recipe) in self.publish.settings.recipes().iter().enumerate() {
                choices = choices.child(
                    button(
                        &t,
                        ("composer-recipe", index),
                        recipe.name.clone(),
                        index == active,
                    )
                    .min_w_0()
                    .justify_start()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if this.publish.settings.select_recipe(index) {
                            this.composer_recipe_changed(cx);
                        } else {
                            this.publish.composer.recipe_menu = false;
                            cx.notify();
                        }
                    })),
                );
            }
            controls = controls.child(choices);
        }
        controls = controls.child(
            div()
                .flex()
                .flex_wrap()
                .gap_1()
                .child(
                    button(&t, "composer-recipe-new", "New", false).on_click(cx.listener(
                        |this, _, _, cx| {
                            this.publish.settings.new_recipe();
                            this.publish.settings.active_composite_mut().panels =
                                CompositeRecipe::suggested_panels(
                                    this.publish.figures.iter().map(|figure| figure.key),
                                );
                            this.composer_recipe_changed(cx);
                        },
                    )),
                )
                .child(
                    button(&t, "composer-recipe-duplicate", "Duplicate", false).on_click(
                        cx.listener(|this, _, _, cx| {
                            this.publish.settings.duplicate_recipe();
                            this.composer_recipe_changed(cx);
                        }),
                    ),
                )
                .child(
                    button(&t, "composer-recipe-rename", "Rename", false).on_click(cx.listener(
                        |this, _, window, cx| {
                            this.rename_composer_recipe(window, cx);
                        },
                    )),
                )
                .child(
                    enabled_when(
                        with_tip(
                            button(&t, "composer-recipe-delete", "Delete", false),
                            &t,
                            if can_delete {
                                "Delete this recipe after confirmation."
                            } else {
                                "Keep at least one recipe in the project."
                            },
                        ),
                        can_delete,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        if this.publish.settings.recipes().len() > 1 {
                            this.publish.composer.delete_recipe = Some((
                                this.publish.settings.active_recipe_index(),
                                this.publish.settings.active_recipe().name.clone(),
                            ));
                            this.publish.composer.rename = None;
                            this.publish.composer.name_error = None;
                            this.publish.composer.recipe_menu = false;
                            cx.notify();
                        }
                    })),
                ),
        );
        if let Some(field) = &state.rename {
            controls = controls.child(field.clone()).child(
                div()
                    .flex()
                    .justify_end()
                    .gap_1()
                    .child(
                        button(&t, "composer-recipe-rename-cancel", "Cancel", false).on_click(
                            cx.listener(|this, _, _, cx| {
                                this.publish.composer.rename = None;
                                this.publish.composer.name_error = None;
                                cx.notify();
                            }),
                        ),
                    )
                    .child(
                        enabled_when(
                            button(&t, "composer-recipe-rename-save", "Save name", true),
                            state.name_error.is_none(),
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.save_composer_recipe_name(cx);
                        })),
                    ),
            );
            if let Some(error) = &state.name_error {
                controls = controls.child(
                    div()
                        .text_size(px(11.))
                        .text_color(t.error)
                        .child(error.clone()),
                );
            }
        }
        if let Some((index, name)) = &state.delete_recipe {
            let index = *index;
            let name = name.clone();
            let expected_name = name.clone();
            controls = controls.child(
                div()
                    .p_2()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .rounded_sm()
                    .border_1()
                    .border_color(t.border)
                    .bg(t.raised)
                    .text_size(px(12.))
                    .child(format!("Delete “{name}”?"))
                    .child(div().text_size(px(11.)).text_color(t.text_muted).child(
                        "This removes the saved panel layout and shared style from this project.",
                    ))
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap_1()
                            .child(
                                button(&t, "composer-recipe-delete-cancel", "Cancel", false)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.publish.composer.delete_recipe = None;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                button(&t, "composer-recipe-delete-confirm", "Delete recipe", true)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if this.publish.settings.active_recipe_index() == index
                                            && this.publish.settings.active_recipe().name
                                                == expected_name
                                            && this.publish.settings.delete_recipe()
                                        {
                                            this.composer_recipe_changed(cx);
                                        } else {
                                            this.publish.composer.delete_recipe = None;
                                            cx.notify();
                                        }
                                    })),
                            ),
                    ),
            );
        }
        controls
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
                        // Preserve the original first-open suggestions only for
                        // an untouched default project. Other empty recipes stay
                        // empty when returning from the single-figure editor.
                        let settings = &this.publish.settings;
                        if settings.recipes().len() == 1
                            && settings.active_recipe().name == "Figure 1"
                            && settings.active_recipe().recipe == CompositeRecipe::default()
                        {
                            this.publish.settings.active_composite_mut().panels =
                                CompositeRecipe::suggested_panels(
                                    this.publish.figures.iter().map(|figure| figure.key),
                                );
                        }
                        this.composer_fields(cx);
                        this.invalidate_composer_preview();
                    }
                    cx.notify();
                })),
            );
        }
        div().flex().child(modes)
    }

    /// Rebuild the numeric fields from the current recipe.
    fn composer_fields(&mut self, cx: &mut Context<Self>) {
        self.publish.composer.fields_generation += 1;
        let generation = self.publish.composer.fields_generation;
        let recipe = self.publish.settings.active_recipe().recipe.clone();
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
            cx.subscribe(&field, move |this, _, event, cx| {
                if this.publish.composer.fields_generation != generation {
                    return;
                }
                match event {
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
                }
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
        // Never display or copy a caption or raster produced for another
        // recipe/settings revision while the replacement is rendering.
        state.invalidate_preview();
        state.source = Some(source);
        state.running = true;
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
                    let recipe = &settings.active_recipe().recipe;
                    let composed = compose(
                        recipe,
                        &figures,
                        &settings,
                        Some(recipe.dpi.min(PREVIEW_DPI)),
                    )?;
                    // Report the exported size; the preview raster uses a lower DPI.
                    let rendered = recipe.panels.len() - composed.missing.len();
                    let size_mm = recipe.grid(rendered, recipe.dpi).size_mm();
                    Ok::<_, String>((composed.png()?, size_mm, composed))
                })
                .await;
            this.update(cx, |app, cx| {
                let state = &mut app.publish.composer;
                if state.generation != generation {
                    return;
                }
                state.running = false;
                match result {
                    Ok((png, size_mm, composed)) => {
                        state.image =
                            Some(Arc::new(gpui::Image::from_bytes(ImageFormat::Png, png)));
                        state.size_mm = size_mm;
                        state.missing = composed.missing;
                        state.caption = composed.caption;
                    }
                    Err(error) => {
                        state.image = None;
                        state.missing.clear();
                        state.caption.clear();
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
        let name = format!("{}.{extension}", settings.active_recipe().file_stem());
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
                    let bytes =
                        compose(&settings.active_recipe().recipe, &figures, &settings, None)?
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

    /// Save the current combined caption as UTF-8 text using the recipe's name.
    fn save_composite_caption(&mut self, cx: &mut Context<Self>) {
        if !self.require_publication_source(cx) {
            return;
        }
        let state = &self.publish.composer;
        if state.running || state.image.is_none() || state.caption.is_empty() {
            return;
        }
        let caption = format!("{}\n", state.caption);
        let name = format!(
            "{}-caption.txt",
            self.publish.settings.active_recipe().file_stem()
        );
        let home = crate::settings::home_dir().unwrap_or_else(std::env::temp_dir);
        let rx = cx.prompt_for_new_path(&home, Some(&name));
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(mut path))) = rx.await else {
                return;
            };
            if path.extension().is_none() {
                path.set_extension("txt");
            }
            let result = cx
                .background_executor()
                .spawn(async move {
                    std::fs::write(&path, caption)
                        .map(|()| path)
                        .map_err(|error| error.to_string())
                })
                .await;
            this.update(cx, |app, cx| {
                match result {
                    Ok(path) => app.set_status(
                        crate::app::StatusKind::Success,
                        format!("Saved {}", path.display()),
                    ),
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
        let recipe = self.publish.settings.active_recipe().recipe.clone();
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
            .child(self.composer_recipe_controls(cx))
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
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(12.))
                            .child(caption.clone()),
                    )
                    .child(
                        enabled_when(
                            with_tip(
                                button(&t, "save-composite-caption", "Save caption…", false),
                                &t,
                                "Save the combined panel caption as a text file.",
                            ),
                            can_save,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if can_save {
                                this.save_composite_caption(cx);
                            }
                        })),
                    )
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switching_recipe_clears_results_and_rejects_its_in_flight_render() {
        let mut state = ComposerState {
            open: true,
            true_size: true,
            source: Some((vec![10], vec![1])),
            generation: 7,
            fields_generation: 3,
            running: true,
            image: Some(Arc::new(gpui::Image::from_bytes(
                ImageFormat::Png,
                Vec::<u8>::new(),
            ))),
            size_mm: (83., 60.),
            missing: vec!["fit-k".into()],
            caption: "Caption from the previous recipe.".into(),
            error: Some("Previous recipe error".into()),
            ..Default::default()
        };
        let old_generation = state.generation;
        state.invalidate_preview();
        assert_ne!(state.generation, old_generation);
        assert!(state.source.is_none());
        assert!(!state.running);
        assert!(state.image.is_none());
        assert_eq!(state.size_mm, (0., 0.));
        assert!(state.caption.is_empty());
        assert!(state.missing.is_empty());
        assert!(state.error.is_none());
        assert!(state.open && state.true_size);
        assert_eq!(state.fields_generation, 3);
    }

    #[test]
    fn project_load_reset_does_not_reuse_render_or_field_generations() {
        let mut state = ComposerState {
            open: true,
            recipe_menu: true,
            delete_recipe: Some((1, "Figure 2".into())),
            name_error: Some("Invalid draft".into()),
            generation: 7,
            fields_generation: 3,
            caption: "Caption from the previous project.".into(),
            ..Default::default()
        };
        state.reset();
        assert_eq!(state.generation, 8);
        assert_eq!(state.fields_generation, 4);
        assert!(!state.open);
        assert!(!state.recipe_menu);
        assert!(state.rename.is_none());
        assert!(state.delete_recipe.is_none());
        assert!(state.name_error.is_none());
        assert!(state.caption.is_empty());
    }
}
