//! Journal + undo/redo. Every applied change (parameter edits, tools, fits)
//! appends a one-line entry; parameter edits and derived-group changes carry
//! an inverse so ⌘Z / ⇧⌘Z walk them back. The journal is also the recipe a
//! batch run repeats.

use gpui::{ClickEvent, Context, IntoElement, ParentElement, Styled, div, prelude::*, px};

use super::MONO;
use crate::app::{DERIVED_BASE, ParamKey, StudioApp};
use crate::params::{DerivedSpectrum, PipelineParams};

/// Inverse of a recorded change.
#[allow(clippy::large_enum_variant)]
pub enum UndoOp {
    /// Pipeline parameters of `target` (a group override, or the globals).
    Param {
        target: Option<usize>,
        key: Option<ParamKey>,
        before: PipelineParams,
        after: PipelineParams,
    },
    /// A derived group was created at `index`.
    DerivedAdd {
        index: usize,
        spectrum: DerivedSpectrum,
    },
    /// A derived group was removed from `index`.
    DerivedRemove {
        index: usize,
        spectrum: DerivedSpectrum,
    },
}

pub struct JournalEntry {
    pub text: String,
}

#[derive(Default)]
pub struct JournalState {
    pub entries: Vec<JournalEntry>,
    pub undo: Vec<UndoOp>,
    pub redo: Vec<UndoOp>,
    pub open: bool,
}

const JOURNAL_CAPACITY: usize = 500;

impl StudioApp {
    /// Append a journal line, optionally with its inverse.
    pub(crate) fn record(&mut self, text: impl Into<String>, op: Option<UndoOp>) {
        let text = text.into();
        self.journal.entries.push(JournalEntry { text });
        if self.journal.entries.len() > JOURNAL_CAPACITY {
            self.journal.entries.remove(0);
        }
        if let Some(op) = op {
            self.journal.undo.push(op);
            self.journal.redo.clear();
        }
    }

    /// Record a parameter edit. Consecutive edits of the same parameter on
    /// the same target (a drag, a stepper burst) collapse into one step.
    pub(crate) fn record_param_edit(
        &mut self,
        target: Option<usize>,
        key: Option<ParamKey>,
        before: PipelineParams,
        after: PipelineParams,
        text: String,
    ) {
        if before == after {
            return;
        }
        if let Some(UndoOp::Param {
            target: t,
            key: k,
            after: a,
            ..
        }) = self.journal.undo.last_mut()
            && *t == target
            && key.is_some()
            && *k == key
        {
            *a = after;
            if let Some(last) = self.journal.entries.last_mut() {
                last.text = text;
            }
            self.journal.redo.clear();
            return;
        }
        self.record(
            text,
            Some(UndoOp::Param {
                target,
                key,
                before,
                after,
            }),
        );
    }

    fn apply_params_to(&mut self, target: Option<usize>, params: PipelineParams) {
        match target {
            Some(ix) => {
                if params == self.params {
                    self.overrides.remove(&ix);
                } else {
                    self.overrides.insert(ix, params);
                }
            }
            None => self.params = params,
        }
    }

    fn insert_derived(&mut self, index: usize, spectrum: DerivedSpectrum, cx: &mut Context<Self>) {
        let index = index.min(self.derived.len());
        self.derived.insert(index, spectrum);
        self.cache.clear();
        self.select_entry(DERIVED_BASE + index, cx);
        self.sync_param_fields(cx);
    }

    fn take_derived(&mut self, index: usize, cx: &mut Context<Self>) -> Option<DerivedSpectrum> {
        if index >= self.derived.len() {
            return None;
        }
        let spectrum = self.derived.remove(index);
        self.selection.retain(|&ix| ix < DERIVED_BASE);
        if self.selected.is_some_and(|ix| ix >= DERIVED_BASE) {
            self.selected = None;
        }
        self.cache.clear();
        self.invalidate_explore_plots(cx);
        self.sync_param_fields(cx);
        Some(spectrum)
    }

    pub(crate) fn undo(&mut self, cx: &mut Context<Self>) {
        let Some(op) = self.journal.undo.pop() else {
            self.status = "nothing to undo".into();
            cx.notify();
            return;
        };
        let inverse = match op {
            UndoOp::Param {
                target,
                key,
                before,
                after,
            } => {
                self.apply_params_to(target, before.clone());
                self.after_param_undo(cx);
                UndoOp::Param {
                    target,
                    key,
                    before,
                    after,
                }
            }
            UndoOp::DerivedAdd { index, spectrum } => {
                let spectrum = self.take_derived(index, cx).unwrap_or(spectrum);
                UndoOp::DerivedAdd { index, spectrum }
            }
            UndoOp::DerivedRemove { index, spectrum } => {
                self.insert_derived(index, spectrum.clone(), cx);
                UndoOp::DerivedRemove { index, spectrum }
            }
        };
        self.journal.redo.push(inverse);
        self.journal.entries.push(JournalEntry {
            text: "undo".into(),
        });
        self.status = "undone".into();
        cx.notify();
    }

    pub(crate) fn redo(&mut self, cx: &mut Context<Self>) {
        let Some(op) = self.journal.redo.pop() else {
            self.status = "nothing to redo".into();
            cx.notify();
            return;
        };
        let forward = match op {
            UndoOp::Param {
                target,
                key,
                before,
                after,
            } => {
                self.apply_params_to(target, after.clone());
                self.after_param_undo(cx);
                UndoOp::Param {
                    target,
                    key,
                    before,
                    after,
                }
            }
            UndoOp::DerivedAdd { index, spectrum } => {
                self.insert_derived(index, spectrum.clone(), cx);
                UndoOp::DerivedAdd { index, spectrum }
            }
            UndoOp::DerivedRemove { index, spectrum } => {
                let spectrum = self.take_derived(index, cx).unwrap_or(spectrum);
                UndoOp::DerivedRemove { index, spectrum }
            }
        };
        self.journal.undo.push(forward);
        self.journal.entries.push(JournalEntry {
            text: "redo".into(),
        });
        self.status = "redone".into();
        cx.notify();
    }

    fn after_param_undo(&mut self, cx: &mut Context<Self>) {
        self.sync_param_fields(cx);
        self.schedule_recompute(cx);
        self.sync_handles(cx);
        self.invalidate_explore_plots(cx);
    }

    /// Bottom strip listing the journal, newest first.
    pub(crate) fn journal_panel(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let mut list = div()
            .id("journal-list")
            .flex_1()
            .min_h_0()
            .min_w_0()
            .overflow_y_scroll()
            .px_3()
            .py_1();
        for (i, entry) in self.journal.entries.iter().enumerate().rev() {
            list = list.child(
                div()
                    .h(px(20.))
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_size(px(11.))
                    .child(
                        div()
                            .w(px(36.))
                            .font_family(MONO)
                            .text_color(t.text_muted)
                            .child(format!("{}", i + 1)),
                    )
                    .child(div().text_color(t.text).child(entry.text.clone())),
            );
        }
        div()
            .h(px(150.))
            .flex_none()
            .flex()
            .flex_col()
            .bg(t.surface)
            .border_t_1()
            .border_color(t.border)
            .child(
                div()
                    .px_3()
                    .py_1()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_size(px(11.))
                    .text_color(t.text_muted)
                    .child(super::section_label(&t, "Journal"))
                    .child(format!(
                        "{} steps · {} undoable",
                        self.journal.entries.len(),
                        self.journal.undo.len()
                    ))
                    .child(div().flex_1())
                    .child(
                        div()
                            .id("journal-close")
                            .px_1()
                            .cursor_pointer()
                            .hover(|d| d.text_color(t.text))
                            .on_click(cx.listener(|this, _: &ClickEvent, _w, cx| {
                                this.journal.open = false;
                                cx.notify();
                            }))
                            .child("close"),
                    ),
            )
            .child(list)
    }
}
