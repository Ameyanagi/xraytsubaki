//! Labeled numeric parameter field. Empty / "auto" commits as `None`
//! (= auto-determined by the core); a number commits as `Some(v)`.
//! Integer-kind fields round (and clamp) on commit so the display always
//! matches the value the pipeline uses. Rejected input reverts to the last
//! committed value with a brief error flash + [`FieldEvent::Invalid`].

use std::time::Duration;

use gpui::{
    ClickEvent, Context, Entity, EventEmitter, IntoElement, ParentElement, Render, SharedString,
    Styled, Window, div, prelude::*, px,
};

use crate::theme::Theme;
use crate::widgets::text_input::{InputEvent, InputStyle, TextInput};

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
    /// Unit shown outside the box (parsed from a trailing "(unit)" in the
    /// label, e.g. "k min (Å⁻¹)").
    unit: SharedString,
    input: Entity<TextInput>,
    value: Option<f64>,
    kind: FieldKind,
    theme: Theme,
    /// Increment of the ▲▼ steppers and the ↑/↓ keys.
    step: f64,
    /// Bumped per rejected commit so an old flash-clear timer never
    /// extinguishes a newer error.
    error_epoch: u64,
}

/// "pre-edge start (eV)" → ("pre-edge start", "eV").
fn split_unit(label: &str) -> (String, String) {
    let label = label.trim();
    if let Some(open) = label.rfind(" (")
        && label.ends_with(')')
    {
        let unit = &label[open + 2..label.len() - 1];
        if !unit.is_empty() && unit.chars().count() <= 6 {
            return (label[..open].to_string(), unit.to_string());
        }
    }
    (label.to_string(), String::new())
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
        let input = cx.new(|cx| {
            TextInput::new(placeholder, format_value(value), theme, cx).with_style(InputStyle {
                align_right: true,
                mono: true,
                placeholder_accent: true,
            })
        });
        cx.subscribe(&input, |this: &mut Self, input, event, cx| {
            let text = match event {
                InputEvent::Committed(text) => text,
                InputEvent::Edited(_) => {
                    // typing resumed — stop flashing a previous rejection
                    input.update(cx, |i, cx| i.set_error(false, cx));
                    return;
                }
                InputEvent::Step(direction) => {
                    this.nudge(*direction, cx);
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
        let (label, unit) = split_unit(&label.into());
        Self {
            label: label.into(),
            unit: unit.into(),
            input,
            value,
            kind,
            theme,
            step: match kind {
                FieldKind::Float => 1.0,
                FieldKind::Integer { .. } => 1.0,
            },
            error_epoch: 0,
        }
    }

    /// Increment used by the steppers and ↑/↓.
    pub fn with_step(mut self, step: f64) -> Self {
        self.step = step;
        self
    }

    /// Step the value (from `auto` the step starts at the displayed
    /// placeholder value when it parses, else at zero) and emit.
    fn nudge(&mut self, direction: i32, cx: &mut Context<Self>) {
        let base = self.value.or_else(|| self.placeholder_value(cx));
        let raw = base.unwrap_or(0.0) + direction as f64 * self.step;
        // Snap to the step grid so 0.1-steps don't accumulate 0.30000000004.
        let snapped = (raw / self.step).round() * self.step;
        let value = match self.kind {
            FieldKind::Float => Some((snapped * 1e9).round() / 1e9),
            FieldKind::Integer { min } => {
                let rounded = snapped.round();
                Some(min.map_or(rounded, |m| rounded.max(m as f64)))
            }
        };
        self.set_value(value, cx);
        cx.emit(FieldEvent::Changed(value));
    }

    /// The number inside an "auto (−200)" placeholder, if any.
    fn placeholder_value(&self, cx: &Context<Self>) -> Option<f64> {
        let text = self.input.read(cx).placeholder_text();
        let open = text.find('(')?;
        let close = text[open..].find(')')? + open;
        text[open + 1..close]
            .replace('−', "-")
            .trim()
            .parse::<f64>()
            .ok()
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

    /// Last committed value (`None` = auto).
    pub fn value(&self) -> Option<f64> {
        self.value
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
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let overridden = self.value.is_some();
        let stepper = |id: &'static str, glyph: &'static str, dir: i32| {
            div()
                .id(id)
                .w(px(14.))
                .h(px(11.))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(7.))
                .text_color(t.text_muted)
                .cursor_pointer()
                .hover(|d| d.bg(t.raised).text_color(t.text))
                .on_click(cx.listener(move |this, _: &ClickEvent, _w, cx| {
                    this.nudge(dir, cx);
                }))
                .child(glyph)
        };
        div()
            .px_3()
            .py_0p5()
            .flex()
            .items_center()
            .gap_1p5()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .text_size(px(12.))
                    .text_color(t.text_muted)
                    .child(self.label.clone()),
            )
            .child(
                div()
                    .id("reset-auto")
                    .flex_none()
                    .w(px(16.))
                    .text_size(px(10.))
                    .text_color(if overridden { t.accent } else { t.border })
                    .when(overridden, |d| {
                        d.cursor_pointer()
                            .hover(|d| d.text_color(t.text))
                            .on_click(cx.listener(|this, _: &ClickEvent, _w, cx| {
                                this.set_value(None, cx);
                                cx.emit(FieldEvent::Changed(None));
                            }))
                    })
                    .child(if overridden { "↺" } else { "" }),
            )
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .rounded_sm()
                    .child(div().w(px(84.)).child(self.input.clone()))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(stepper("step-up", "▲", 1))
                            .child(stepper("step-down", "▼", -1)),
                    ),
            )
            .child(
                div()
                    .flex_none()
                    .w(px(26.))
                    .font_family("Menlo")
                    .text_size(px(10.5))
                    .text_color(t.text_muted)
                    .child(self.unit.clone()),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::{FieldKind, parse_commit};

    #[test]
    fn unit_is_split_from_label() {
        assert_eq!(
            super::split_unit("k min (Å⁻¹)"),
            ("k min".into(), "Å⁻¹".into())
        );
        assert_eq!(
            super::split_unit("poly order"),
            ("poly order".into(), String::new())
        );
        assert_eq!(
            super::split_unit("ref E0 target"),
            ("ref E0 target".into(), String::new())
        );
    }

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
