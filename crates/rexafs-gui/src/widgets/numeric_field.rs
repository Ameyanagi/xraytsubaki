//! Labeled numeric parameter field. Empty / "auto" commits as `None`
//! (= auto-determined by the core); a number commits as `Some(v)`.
//! Integer-kind fields round (and clamp) on commit so the display always
//! matches the value the pipeline uses. Rejected input remains editable with
//! an inline explanation; the last committed calculation value is unchanged.

use gpui::{
    ClickEvent, Context, Entity, EventEmitter, IntoElement, ParentElement, Render, SharedString,
    Styled, Window, div, prelude::*, px,
};

use crate::theme::Theme;
use crate::widgets::text_input::{InputEvent, InputStyle, TextInput};

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
    /// Full automatic-value hint, kept outside the compact input box.
    automatic_hint: SharedString,
    error: Option<SharedString>,
    mixed: bool,
    display_decimals: Option<usize>,
    /// Longer explanation shown as a tooltip on the label.
    description: Option<SharedString>,
    /// Amber note when processing used a different, data-limited value.
    adjusted: Option<SharedString>,
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
    InspectMixed,
    /// Input was rejected; the payload is a ready-to-show status message.
    Invalid(SharedString),
}

impl EventEmitter<FieldEvent> for NumericField {}

/// Opt-in notification for lightweight previews while typing. Committed values
/// and existing processing subscribers are unchanged; read `pending_value`.
pub(crate) struct FieldPreview;
impl EventEmitter<FieldPreview> for NumericField {}

/// Significant figures shown when a field has no fixed decimal count. Six
/// covers every value the pipeline displays (energies in eV, k in Å⁻¹, R in Å)
/// without exposing binary round-off such as `1.349999999`.
const DISPLAY_SIGNIFICANT: usize = 6;

/// Shortest text for `value` with at most `significant` significant figures
/// and no trailing zeros: 1.349999999 → "1.35", 8979 → "8979", 0.05 → "0.05".
/// Integers wider than `significant` digits keep all their digits.
fn format_significant(value: f64, significant: usize) -> String {
    if value == 0.0 || !value.is_finite() {
        return format!("{value}");
    }
    let magnitude = value.abs().log10().floor() as i64;
    let decimals = (significant as i64 - 1 - magnitude).max(0) as usize;
    let text = format!("{value:.decimals$}");
    if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        text
    }
}

fn format_value(value: Option<f64>, decimals: Option<usize>) -> String {
    value
        .map(|v| match decimals {
            Some(d) => format!("{v:.d$}"),
            None => format_significant(v, DISPLAY_SIGNIFICANT),
        })
        .unwrap_or_default()
}

fn parse_displayed(
    text: &str,
    value: Option<f64>,
    kind: FieldKind,
    decimals: Option<usize>,
) -> Result<Option<f64>, ()> {
    // Focusing and submitting an unchanged rounded display must not alter the
    // calculation. Newly entered text still uses the full numeric parser.
    if value.is_some() && text.trim() == format_value(value, decimals) {
        Ok(value)
    } else {
        parse_commit(text, kind)
    }
}

/// Keep the editing affordance short; the resolved automatic value is rendered
/// separately so it remains readable even in narrow inspectors.
fn compact_placeholder(hint: &str) -> SharedString {
    if hint.to_ascii_lowercase().starts_with("auto") {
        "Auto".into()
    } else {
        hint.to_string().into()
    }
}

fn automatic_detail(hint: &str) -> Option<String> {
    let (_, detail) = hint.split_once('(')?;
    Some(detail.strip_suffix(')')?.to_string())
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
        let automatic_hint = placeholder.into();
        let placeholder = compact_placeholder(&automatic_hint);
        let input = cx.new(|cx| {
            TextInput::new(placeholder, format_value(value, None), theme, cx).with_style(
                InputStyle {
                    align_right: true,
                    mono: true,
                    placeholder_accent: true,
                    ..Default::default()
                },
            )
        });
        cx.subscribe(&input, |this: &mut Self, input, event, cx| {
            let text = match event {
                InputEvent::Committed(text) => text,
                InputEvent::Edited(text) => {
                    if this.error.is_some() && parse_commit(text, this.kind).is_ok() {
                        this.error = None;
                        input.update(cx, |i, cx| i.set_error(false, cx));
                    }
                    cx.notify();
                    cx.emit(FieldPreview);
                    return;
                }
                InputEvent::Step(direction) => {
                    this.nudge(*direction, cx);
                    return;
                }
            };
            match parse_displayed(text, this.value, this.kind, this.display_decimals) {
                Ok(value) => {
                    this.error = None;
                    cx.notify();
                    input.update(cx, |i, cx| {
                        i.set_error(false, cx);
                        // normalized display (e.g. "2.7" -> "3" for integers)
                        i.set_text(format_value(value, this.display_decimals), cx);
                    });
                    if value != this.value {
                        this.value = value;
                        cx.emit(FieldEvent::Changed(value));
                    }
                }
                Err(()) => {
                    let message: SharedString = format!(
                        "Invalid value for {}: '{}' — expected a number or 'auto'",
                        this.label,
                        text.trim()
                    )
                    .into();
                    this.error =
                        Some("Enter a number or auto. The previous value is still in use.".into());
                    input.update(cx, |i, cx| i.set_error(true, cx));
                    cx.notify();
                    cx.emit(FieldEvent::Invalid(message));
                }
            }
        })
        .detach();
        let label: SharedString = label.into();
        input.update(cx, |input, _| input.set_accessible_name(label.clone()));
        let (label, unit) = split_unit(&label);
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
            automatic_hint,
            error: None,
            mixed: false,
            display_decimals: None,
            description: None,
            adjusted: None,
        }
    }

    /// Attach a one-sentence explanation, shown as a tooltip when the label is
    /// hovered. Keeps the visible label short.
    pub fn with_description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Increment used by the steppers and ↑/↓.
    pub fn with_step(mut self, step: f64) -> Self {
        self.step = step;
        self
    }

    /// Round only the visible text. Unedited submissions preserve the full
    /// stored value; newly entered numbers retain their entered precision.
    pub fn with_display_decimals(mut self, decimals: usize, cx: &mut Context<Self>) -> Self {
        self.display_decimals = Some(decimals);
        self.set_value(self.value, cx);
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
    fn placeholder_value(&self, _cx: &Context<Self>) -> Option<f64> {
        let text = &self.automatic_hint;
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
        self.automatic_hint = placeholder.into();
        let compact = compact_placeholder(&self.automatic_hint);
        self.input
            .update(cx, |i, cx| i.set_placeholder(compact, cx));
        cx.notify();
    }

    /// Show (or clear) an amber note that processing used another value, e.g.
    /// a range endpoint limited to the measured data. The value is unchanged.
    pub fn set_adjusted(&mut self, note: Option<SharedString>, cx: &mut Context<Self>) {
        if self.adjusted != note {
            self.adjusted = note;
            cx.notify();
        }
    }

    pub fn set_mixed(&mut self, mixed: bool, cx: &mut Context<Self>) {
        if self.mixed != mixed {
            self.mixed = mixed;
            cx.notify();
        }
    }

    /// Last committed value (`None` = auto).
    pub fn value(&self) -> Option<f64> {
        self.value
    }

    /// Validate the visible text before a submit button removes the field.
    /// Uses the same integer rounding and bounds as Enter, without committing
    /// or emitting events. Invalid text is an error, never the previous value.
    pub(crate) fn pending_value(&self, cx: &gpui::App) -> Result<Option<f64>, ()> {
        parse_displayed(
            self.input.read(cx).text(),
            self.value,
            self.kind,
            self.display_decimals,
        )
    }

    /// Programmatically set value (None = auto/empty); does not emit.
    pub fn set_value(&mut self, value: Option<f64>, cx: &mut Context<Self>) {
        self.value = value;
        self.error = None;
        self.input.update(cx, |i, cx| {
            i.set_error(false, cx);
            i.set_text(format_value(value, self.display_decimals), cx)
        });
        cx.notify();
    }
}

impl Render for NumericField {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let overridden = self.value.is_some();
        // `↺ auto` only makes sense for fields that have an automatic value
        // (an "auto (…)" placeholder); plain numeric fields have none.
        let has_auto = self.automatic_hint.to_ascii_lowercase().starts_with("auto");
        let can_reset = overridden && has_auto;
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
        let row = div()
            .flex()
            .items_center()
            .gap_1p5()
            .child(
                div()
                    .id("field-label")
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .text_size(px(12.))
                    .text_color(if self.adjusted.is_some() {
                        t.warn
                    } else {
                        t.text_muted
                    })
                    .when_some(self.description.clone(), |d, description| {
                        d.tooltip(move |_, cx| {
                            cx.new(|_| crate::app::Tooltip {
                                label: description.clone(),
                                theme: t,
                            })
                            .into()
                        })
                    })
                    .child(self.label.clone()),
            )
            .child(
                div()
                    .id("reset-auto")
                    .flex_none()
                    .w(px(16.))
                    .text_size(px(11.))
                    .text_color(if self.mixed {
                        t.warn
                    } else if can_reset {
                        t.accent
                    } else {
                        t.border
                    })
                    .when(self.mixed, |d| {
                        d.cursor_pointer().on_click(
                            cx.listener(|_, _: &ClickEvent, _, cx| {
                                cx.emit(FieldEvent::InspectMixed)
                            }),
                        )
                    })
                    .when(can_reset && !self.mixed, |d| {
                        d.cursor_pointer()
                            .hover(|d| d.text_color(t.text))
                            .on_click(cx.listener(|this, _: &ClickEvent, _w, cx| {
                                this.set_value(None, cx);
                                cx.emit(FieldEvent::Changed(None));
                            }))
                    })
                    .child(if self.mixed {
                        "⚠"
                    } else if can_reset {
                        "↺"
                    } else {
                        ""
                    }),
            )
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .rounded_sm()
                    .child(div().w(px(98.)).child(self.input.clone()))
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
                    .w(px(22.))
                    .font_family(crate::theme::MONO)
                    .text_size(px(11.))
                    .text_color(t.text_muted)
                    .child(self.unit.clone()),
            );
        div()
            .px_3()
            .py_0p5()
            .flex()
            .flex_col()
            .gap_1()
            .child(row)
            .when_some(
                self.adjusted.clone().filter(|_| self.error.is_none()),
                |d, note| d.child(div().text_size(px(11.)).text_color(t.warn).child(note)),
            )
            .when(
                !overridden && self.error.is_none() && self.adjusted.is_none() && has_auto,
                |d| {
                    d.when_some(automatic_detail(&self.automatic_hint), |d, detail| {
                        d.child(
                            div().text_size(px(11.)).text_color(t.text_muted).child(
                                format!(
                                    "Automatic: {detail} {}",
                                    if detail.parse::<f64>().is_ok() {
                                        self.unit.as_ref()
                                    } else {
                                        ""
                                    }
                                )
                                .trim()
                                .to_string(),
                            ),
                        )
                    })
                },
            )
            .when_some(self.error.clone(), |d, error| {
                let action = |id, label: &'static str| {
                    crate::accessibility::Control::new(
                        div()
                            .id(id)
                            .cursor_pointer()
                            .text_size(px(11.))
                            .text_color(t.accent)
                            .child(label),
                        label,
                        accesskit::Role::Button,
                    )
                };
                d.child(div().text_size(px(11.)).text_color(t.error).child(error))
                    .child(
                        div()
                            .flex()
                            .gap_3()
                            .child(action("restore-value", "Restore previous value").on_click(
                                cx.listener(|this, _, _, cx| this.set_value(this.value, cx)),
                            ))
                            .when(has_auto, |d| {
                                d.child(action("use-auto", "Use Auto").on_click(cx.listener(
                                    |this, _, _, cx| {
                                        this.set_value(None, cx);
                                        cx.emit(FieldEvent::Changed(None));
                                    },
                                )))
                            }),
                    )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::{FieldKind, parse_commit};

    #[test]
    fn automatic_hints_keep_resolved_values_outside_the_input() {
        for (hint, detail) in [
            ("auto (8977.5)", "8977.5"),
            ("Auto (8957.49)", "8957.49"),
            ("auto (spectrum end)", "spectrum end"),
        ] {
            assert_eq!(super::compact_placeholder(hint).as_ref(), "Auto");
            assert_eq!(super::automatic_detail(hint).as_deref(), Some(detail));
        }
        assert!(super::automatic_detail("Auto").is_none());
        assert_eq!(super::compact_placeholder("required").as_ref(), "required");
    }

    #[test]
    fn rounded_display_does_not_round_the_calculation_on_submit() {
        let value = Some(3.485252958794127);
        assert_eq!(super::format_value(value, Some(2)), "3.49");
        assert_eq!(
            super::parse_displayed("3.49", value, FieldKind::Float, Some(2)),
            Ok(value)
        );
        assert_eq!(
            super::parse_displayed("0", value, FieldKind::Float, Some(2)),
            Ok(Some(0.0))
        );
        assert_eq!(
            super::parse_displayed("3.4856", value, FieldKind::Float, Some(2)),
            Ok(Some(3.4856))
        );
        assert!(super::parse_displayed("NaN", value, FieldKind::Float, Some(2)).is_err());
    }

    #[test]
    fn auto_display_uses_six_significant_figures() {
        for (value, text) in [
            (1.349999999, "1.35"),
            (8979.0, "8979"),
            (0.05, "0.05"),
            (12000.0, "12000"),
            (-200.0, "-200"),
            (0.0, "0"),
            (8978.987654321, "8978.99"),
            (1234567.0, "1234567"),
        ] {
            assert_eq!(super::format_value(Some(value), None), text);
        }
        assert_eq!(super::format_value(None, None), "");
        // Resubmitting the rounded text keeps the stored value exactly.
        let value = Some(1.349999999);
        assert_eq!(
            super::parse_displayed("1.35", value, FieldKind::Float, None),
            Ok(value)
        );
    }

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
