//! Labeled numeric parameter field. Empty / "auto" commits as `None`
//! (= auto-determined by the core); a number commits as `Some(v)`.
//! Invalid input reverts to the last committed value.

use gpui::{
    Context, Entity, EventEmitter, IntoElement, ParentElement, Render, SharedString, Styled,
    Window, div, prelude::*, px,
};

use crate::theme::Theme;
use crate::widgets::text_input::{InputEvent, TextInput};

pub struct NumericField {
    label: SharedString,
    input: Entity<TextInput>,
    value: Option<f64>,
    theme: Theme,
}

pub enum FieldEvent {
    Changed(Option<f64>),
}

impl EventEmitter<FieldEvent> for NumericField {}

fn format_value(value: Option<f64>) -> String {
    value.map(|v| format!("{v}")).unwrap_or_default()
}

impl NumericField {
    pub fn new(
        label: impl Into<SharedString>,
        placeholder: impl Into<SharedString>,
        value: Option<f64>,
        theme: Theme,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| TextInput::new(placeholder, format_value(value), theme, cx));
        cx.subscribe(&input, |this: &mut Self, input, event, cx| {
            let InputEvent::Committed(text) = event;
            let text = text.trim();
            let parsed = if text.is_empty() || text.eq_ignore_ascii_case("auto") {
                Some(None)
            } else {
                text.parse::<f64>().ok().filter(|v| v.is_finite()).map(Some)
            };
            match parsed {
                Some(value) if value != this.value => {
                    this.value = value;
                    input.update(cx, |i, cx| i.set_text(format_value(value), cx));
                    cx.emit(FieldEvent::Changed(value));
                }
                Some(_) => {}
                None => {
                    // unparsable -> revert
                    let value = this.value;
                    input.update(cx, |i, cx| i.set_text(format_value(value), cx));
                }
            }
        })
        .detach();
        Self {
            label: label.into(),
            input,
            value,
            theme,
        }
    }

    pub fn set_theme(&mut self, theme: Theme, cx: &mut Context<Self>) {
        self.theme = theme;
        self.input.update(cx, |i, cx| i.set_theme(theme, cx));
        cx.notify();
    }

    pub fn set_placeholder(&mut self, placeholder: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.input
            .update(cx, |i, cx| i.set_placeholder(placeholder, cx));
    }

    /// Programmatically set value (None = auto/empty); does not emit.
    pub fn set_value(&mut self, value: Option<f64>, cx: &mut Context<Self>) {
        self.value = value;
        self.input
            .update(cx, |i, cx| i.set_text(format_value(value), cx));
        cx.notify();
    }

    /// Programmatically set the value (e.g. fitted result); does not emit.
    pub fn set_value_text(&mut self, text: String, cx: &mut Context<Self>) {
        if let Ok(v) = text.parse::<f64>() {
            self.value = Some(v);
        }
        self.input.update(cx, |i, cx| i.set_text(text, cx));
        cx.notify();
    }
}

impl Render for NumericField {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        div()
            .px_3()
            .py_0p5()
            .flex()
            .items_center()
            .gap_2()
            .child(
                div()
                    .flex_1()
                    .text_sm()
                    .text_color(t.text_muted)
                    .child(self.label.clone()),
            )
            .child(div().w(px(96.)).child(self.input.clone()))
    }
}
