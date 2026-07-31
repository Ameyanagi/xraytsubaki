//! Labeled numeric parameter field. Empty / "auto" commits as `None`
//! (= auto-determined by the core); a number commits as `Some(v)`.
//! Integer-kind fields round (and clamp) on commit so the display always
//! matches the value the pipeline uses. Rejected input reverts to the last
//! committed value with a brief error flash + [`FieldEvent::Invalid`].

use std::time::Duration;

use gpui::{
    Context, Entity, EventEmitter, IntoElement, ParentElement, Render, SharedString, Styled,
    Window, div, prelude::*, px,
};

use crate::theme::Theme;
use crate::widgets::text_input::{InputEvent, TextInput};

/// How long the rejected-input border stays lit.
const ERROR_FLASH: Duration = Duration::from_millis(1400);

/// Value domain of a field; integers round on commit, optionally clamped
/// to a lower bound (e.g. column indices are >= 0).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FieldKind {
    Float,
    Integer { min: Option<i64> },
}

/// Parse committed text: `Ok(None)` = auto, `Ok(Some(v))` = normalized
/// value (rounded/clamped for integer kinds), `Err(())` = rejected.
fn parse_commit(text: &str, kind: FieldKind) -> Result<Option<f64>, ()> {
    let text = text.trim();
    if text.is_empty() || text.eq_ignore_ascii_case("auto") {
        return Ok(None);
    }
    let value = text.parse::<f64>().map_err(|_| ())?;
    if !value.is_finite() {
        return Err(());
    }
    Ok(Some(match kind {
        FieldKind::Float => value,
        FieldKind::Integer { min } => {
            let rounded = value.round();
            match min {
                Some(min) => rounded.max(min as f64),
                None => rounded,
            }
        }
    }))
}

pub struct NumericField {
    label: SharedString,
    input: Entity<TextInput>,
    value: Option<f64>,
    kind: FieldKind,
    theme: Theme,
    /// Bumped per rejected commit so an old flash-clear timer never
    /// extinguishes a newer error.
    error_epoch: u64,
}

pub enum FieldEvent {
    Changed(Option<f64>),
    /// Input was rejected; the payload is a ready-to-show status message.
    Invalid(SharedString),
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
        kind: FieldKind,
        theme: Theme,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| TextInput::new(placeholder, format_value(value), theme, cx));
        cx.subscribe(&input, |this: &mut Self, input, event, cx| {
            let text = match event {
                InputEvent::Committed(text) => text,
                InputEvent::Edited(_) => {
                    // typing resumed — stop flashing a previous rejection
                    input.update(cx, |i, cx| i.set_error(false, cx));
                    return;
                }
            };
            match parse_commit(text, this.kind) {
                Ok(value) => {
                    input.update(cx, |i, cx| {
                        i.set_error(false, cx);
                        // normalized display (e.g. "2.7" -> "3" for integers)
                        i.set_text(format_value(value), cx);
                    });
                    if value != this.value {
                        this.value = value;
                        cx.emit(FieldEvent::Changed(value));
                    }
                }
                Err(()) => {
                    let message: SharedString = format!(
                        "invalid value for {}: '{}' — expected a number or 'auto'",
                        this.label,
                        text.trim()
                    )
                    .into();
                    let value = this.value;
                    input.update(cx, |i, cx| {
                        i.set_error(true, cx);
                        i.set_text(format_value(value), cx);
                    });
                    this.error_epoch += 1;
                    let epoch = this.error_epoch;
                    let timer = cx.background_executor().timer(ERROR_FLASH);
                    let input = input.clone();
                    cx.spawn(async move |this, cx| {
                        timer.await;
                        this.update(cx, |this, cx| {
                            if this.error_epoch == epoch {
                                input.update(cx, |i, cx| i.set_error(false, cx));
                            }
                        })
                        .ok();
                    })
                    .detach();
                    cx.emit(FieldEvent::Invalid(message));
                }
            }
        })
        .detach();
        Self {
            label: label.into(),
            input,
            value,
            kind,
            theme,
            error_epoch: 0,
        }
    }

    pub fn set_theme(&mut self, theme: Theme, cx: &mut Context<Self>) {
        self.theme = theme;
        self.input.update(cx, |i, cx| i.set_theme(theme, cx));
        cx.notify();
    }

    pub fn set_placeholder(
        &mut self,
        placeholder: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) {
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

#[cfg(test)]
mod tests {
    use super::{FieldKind, parse_commit};

    #[test]
    fn parses_auto_and_floats() {
        assert_eq!(parse_commit("", FieldKind::Float), Ok(None));
        assert_eq!(parse_commit("  auto ", FieldKind::Float), Ok(None));
        assert_eq!(parse_commit("2.7", FieldKind::Float), Ok(Some(2.7)));
        assert_eq!(parse_commit(" -1.5 ", FieldKind::Float), Ok(Some(-1.5)));
    }

    #[test]
    fn integers_round_and_clamp_on_commit() {
        let int = FieldKind::Integer { min: None };
        let col = FieldKind::Integer { min: Some(0) };
        assert_eq!(parse_commit("2.7", int), Ok(Some(3.0)));
        assert_eq!(parse_commit("-1.2", int), Ok(Some(-1.0)));
        assert_eq!(parse_commit("-1.2", col), Ok(Some(0.0)));
        assert_eq!(parse_commit("4", col), Ok(Some(4.0)));
        assert_eq!(parse_commit("auto", int), Ok(None));
    }

    #[test]
    fn rejects_garbage_and_non_finite() {
        for kind in [FieldKind::Float, FieldKind::Integer { min: None }] {
            assert_eq!(parse_commit("1..5", kind), Err(()));
            assert_eq!(parse_commit("abc", kind), Err(()));
            assert_eq!(parse_commit("inf", kind), Err(()));
            assert_eq!(parse_commit("nan", kind), Err(()));
        }
    }
}
