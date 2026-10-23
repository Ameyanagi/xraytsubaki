//! App-authored receipts and permission rules; no model-authored receipt content.
use super::{Stage, journal::JournalState, parameter_actions::SETTINGS};
use serde_json::{Value, json};

pub(super) fn requires_edit(tool: &str) -> bool {
    !matches!(
        tool,
        "xray_get_state"
            | "xray_get_import_status"
            | "xray_get_plots"
            | "xray_navigate"
            | "xray_set_layout"
            | "xray_search_structures"
    )
}

pub(super) fn changes_allowed(live: bool, turn_edit: bool, busy: bool) -> bool {
    live && turn_edit && busy
}

pub(super) fn undo_eligible(token: Option<u64>, journal: &JournalState, running: bool) -> bool {
    !running && !journal.undo.is_empty() && token == Some(journal.receipt_revision)
}

pub(super) fn scope_label(shared: Option<usize>) -> String {
    shared.map_or_else(
        || "This spectrum".into(),
        |n| format!("Shared across {n} datasets"),
    )
}

pub(super) fn processing_scope_label(target: Option<usize>) -> &'static str {
    if target.is_some() {
        "This spectrum"
    } else {
        "Shared across all spectra without overrides"
    }
}

pub(super) fn navigation_target(
    file: &str,
    current: &std::path::Path,
    catalog_index: Option<usize>,
) -> Result<Option<usize>, String> {
    if catalog_index.is_some() || current.to_string_lossy() == file {
        Ok(catalog_index)
    } else {
        Err("Spectrum must already be in the open catalog".into())
    }
}

/// History entries lack the complete source identity needed for a stale check.
pub(super) fn select_history_result(
    provenance: &mut Option<crate::app::FitProvenance>,
    selected: &mut Option<usize>,
    id: usize,
) {
    *provenance = None;
    *selected = Some(id);
}

pub(super) fn history_result_notice(
    has_provenance: bool,
    selected: Option<usize>,
) -> Option<String> {
    selected
        .filter(|_| !has_provenance)
        .map(|id| format!("Viewing history entry {id}"))
}

/// Flatten changed object fields, retaining arrays (path selections, weights) as values.
/// Units come from the application's field catalog, never from the assistant.
pub(super) fn diff(before: &Value, after: &Value) -> Vec<String> {
    fn visit(name: &str, before: &Value, after: &Value, lines: &mut Vec<String>) {
        if before == after {
            return;
        }
        if let (Some(a), Some(b)) = (before.as_object(), after.as_object()) {
            for key in a
                .keys()
                .chain(b.keys())
                .collect::<std::collections::BTreeSet<_>>()
            {
                visit(
                    &if name.is_empty() {
                        key.clone()
                    } else {
                        format!("{name}.{key}")
                    },
                    &before[key],
                    &after[key],
                    lines,
                );
            }
        } else if let (Some(a), Some(b)) = (before.as_array(), after.as_array())
            && a.iter().chain(b).any(Value::is_object)
        {
            for index in 0..a.len().max(b.len()) {
                visit(
                    &format!("{name}[{index}]"),
                    &before[index],
                    &after[index],
                    lines,
                );
            }
        } else {
            let key = name.rsplit('.').next().unwrap_or(name);
            let label = SETTINGS
                .iter()
                .find(|s| s.key == key)
                .map(|s| s.label)
                .unwrap_or(key);
            let (label, unit) = label
                .split_once(" (")
                .map_or((label, ""), |(l, u)| (l, u.trim_end_matches(')')));
            let unit = match key {
                "kmin" | "kmax" => "Å⁻¹",
                "rmin" | "rmax" => "Å",
                _ => unit,
            };
            let value = |v: &Value| {
                if v.is_null() {
                    "Auto".into()
                } else if let Some(s) = v.as_str() {
                    s.into()
                } else {
                    v.to_string()
                }
            };
            let label = if name == key {
                label.to_owned()
            } else {
                format!("{}{label}", &name[..name.len() - key.len()])
            };
            lines.push(format!(
                "{label}: {} → {}{}",
                value(before),
                value(after),
                if unit.is_empty() {
                    String::new()
                } else {
                    format!(" {unit}")
                }
            ));
        }
    }
    let mut lines = Vec::new();
    visit("", before, after, &mut lines);
    lines
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Receipt {
    /// One-time access confirmation, rendered with the same receipt card.
    pub permission: Option<String>,
    pub header: String,
    pub lines: Vec<String>,
    pub scope: String,
    pub navigation: Value,
    pub journal: Option<u64>,
    pub state: String,
    /// Processing output identity; model receipts instead wait for the live preview.
    pub processing: Option<(u64, u64)>,
    pub history: Option<usize>,
    pub model: u64,
    pub undo_retired: bool,
}
impl Receipt {
    /// Parameter values apply synchronously; a cached preview error is unrelated.
    pub fn finish_parameter_edit(&mut self, preview_loading: bool) {
        if !preview_loading {
            self.state = "Recalculation complete".into();
        }
    }

    pub fn change(spectrum: &str, stage: Stage, lines: Vec<String>, journal: u64) -> Self {
        let label = std::path::Path::new(spectrum)
            .file_name()
            .map(|s| s.to_string_lossy())
            .unwrap_or_default();
        Self {
            permission: None,
            header: format!("{label} · {}", stage.name()),
            lines,
            scope: scope_label(None),
            navigation: json!({"spectrum":spectrum,"stage":stage.name()}),
            journal: Some(journal),
            state: "Recalculating…".into(),
            processing: None,
            history: None,
            model: 0,
            undo_retired: false,
        }
    }
    pub fn job(header: String, history: Option<usize>) -> Self {
        Self {
            permission: None,
            header,
            lines: vec![],
            scope: String::new(),
            navigation: json!({"stage":"fit","fit_step":if history.is_some() {"results"} else {"paths"}}),
            journal: None,
            state: String::new(),
            processing: None,
            history,
            model: 0,
            undo_retired: false,
        }
    }
    pub fn permission(id: String, question: String, lines: Vec<String>) -> Self {
        let mut receipt = Self::job(question, None);
        receipt.permission = Some(id);
        receipt.lines = lines;
        receipt.state = "Awaiting approval · expires in 5 minutes".into();
        receipt
    }
    pub fn can_undo(&self, journal: &JournalState, running: bool, model: u64) -> bool {
        !self.undo_retired
            && self.model == model
            && self.state != "Recalculating…"
            && undo_eligible(self.journal, journal, running)
    }
    pub fn text(&self) -> String {
        [
            &self.header,
            &self.lines.join("\n"),
            &self.scope,
            &self.state,
        ]
        .into_iter()
        .filter(|s| !s.is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join("\n")
    }
}

/// None means the scheduled output is still pending. A later edit retires that wait.
pub(super) fn completion(
    same_edit: bool,
    running: bool,
    output_matches: bool,
    error: bool,
) -> Option<&'static str> {
    if !same_edit {
        Some("Superseded by a later change")
    } else if running {
        None
    } else if error {
        Some("Recalculation failed")
    } else if output_matches {
        Some("Recalculation complete")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::super::journal::UndoOp;
    use super::*;

    #[test]
    fn parameter_receipt_ignores_old_preview_error_but_waits_for_loading_preview() {
        for loading in [false, true] {
            let mut receipt = Receipt::change("Cu", Stage::Fit, vec![], 1);
            receipt.finish_parameter_edit(loading);
            // Match refresh_receipts: only pending receipts consult preview errors.
            if receipt.state == "Recalculating…"
                && let Some(state) = completion(true, loading, true, true)
            {
                receipt.state = state.into();
            }
            assert_eq!(
                receipt.state,
                if loading {
                    "Recalculating…"
                } else {
                    "Recalculation complete"
                }
            );
        }
    }

    #[test]
    fn history_selection_discards_newer_provenance_and_identifies_shown_entry() {
        let mut provenance = Some(crate::app::FitProvenance {
            label: "fit 2".into(),
            group_id: None,
            path: "Cu".into(),
            params_fingerprint: 2,
            model_fingerprint: 2,
        });
        let mut selected = Some(2);
        assert_eq!(history_result_notice(true, selected), None);
        select_history_result(&mut provenance, &mut selected, 1);
        assert!(provenance.is_none());
        assert_eq!(selected, Some(1));
        assert_eq!(
            history_result_notice(provenance.is_some(), selected).as_deref(),
            Some("Viewing history entry 1")
        );
        assert_eq!(history_result_notice(false, None), None);
    }

    #[test]
    fn processing_scope_and_navigation_cover_non_catalog_current_spectrum() {
        assert_eq!(processing_scope_label(Some(0)), "This spectrum");
        assert_eq!(
            processing_scope_label(None),
            "Shared across all spectra without overrides"
        );
        let current = std::path::Path::new("/data/Cu.dat");
        assert_eq!(navigation_target("/data/Cu.dat", current, None), Ok(None));
        assert_eq!(
            navigation_target("/data/Cu.dat", current, Some(0)),
            Ok(Some(0))
        );
        assert_eq!(
            navigation_target("/data/Fe.dat", current, Some(1)),
            Ok(Some(1))
        );
        assert!(navigation_target("/data/Fe.dat", current, None).is_err());
    }

    #[test]
    fn every_tool_has_review_or_edit_classification() {
        let tools = crate::codex_client::dynamic_tools();
        let edit = [
            "xray_import_files",
            "xray_fetch_structure",
            "xray_choose_structure",
            "xray_calculate_paths",
            "xray_select_paths",
            "xray_set_fit_ranges",
            "xray_set_fit_parameter",
            "xray_set_processing",
            "xray_run_fit",
            "xray_configure_fit",
            "xray_run_analysis",
        ];
        let review = [
            "xray_get_import_status",
            "xray_get_state",
            "xray_get_plots",
            "xray_navigate",
            "xray_set_layout",
            "xray_search_structures",
        ];
        assert_eq!(tools.as_array().unwrap().len(), edit.len() + review.len());
        for tool in tools.as_array().unwrap() {
            let name = tool["name"].as_str().unwrap();
            assert!(edit.contains(&name) || review.contains(&name));
            assert_eq!(requires_edit(name), edit.contains(&name));
        }
        assert!(requires_edit("xray_future_tool"));
    }
    #[test]
    fn edit_revoked_before_apply_is_rejected_and_enabling_waits_for_send() {
        assert!(changes_allowed(true, true, true));
        assert!(!changes_allowed(false, true, true));
        assert!(!changes_allowed(true, false, true));
        assert!(!changes_allowed(true, true, false));
    }
    #[test]
    fn receipt_values_units_scope_and_completion() {
        assert_eq!(
            diff(
                &json!({"e0":null,"kmin":2,"same":1}),
                &json!({"e0":8979,"kmin":3,"same":1})
            ),
            ["E₀: Auto → 8979 eV", "kmin: 2 → 3 Å⁻¹"]
        );
        assert_eq!(
            diff(&json!({"paths":["a"]}), &json!({"paths":["b"]})),
            ["paths: [\"a\"] → [\"b\"]"]
        );
        assert_eq!(
            diff(
                &json!({"paths":[{"enabled":false,"e0":"0"}]}),
                &json!({"paths":[{"enabled":true,"e0":"shift"}]})
            ),
            [
                "paths[0].E₀: 0 → shift eV",
                "paths[0].enabled: false → true"
            ]
        );
        assert_eq!(scope_label(None), "This spectrum");
        assert_eq!(scope_label(Some(3)), "Shared across 3 datasets");
        assert_eq!(completion(true, true, false, false), None);
        assert_eq!(completion(true, false, false, false), None);
        assert_eq!(
            completion(true, false, true, false),
            Some("Recalculation complete")
        );
        assert_eq!(
            completion(true, false, false, true),
            Some("Recalculation failed")
        );
        assert_eq!(
            completion(false, true, false, false),
            Some("Superseded by a later change")
        );
        let r = Receipt::change(
            "/data/Cu.dat",
            Stage::Process,
            vec!["E₀: Auto → 8979 eV".into()],
            1,
        );
        assert!(
            r.text()
                .contains("Cu.dat · Process\nE₀: Auto → 8979 eV\nThis spectrum\nRecalculating…")
        );
    }
    #[test]
    fn receipt_undo_retires_on_manual_edit_undo_redo_and_new_journal() {
        let mut j = JournalState::default();
        j.record("assistant", Some(UndoOp::Params { changes: vec![] }));
        let token = Some(j.receipt_revision);
        assert!(undo_eligible(token, &j, false));
        assert!(!undo_eligible(token, &j, true));
        let op = j.take_history(false).unwrap();
        j.redo.push(op);
        assert!(!undo_eligible(token, &j, false));
        let op = j.take_history(true).unwrap();
        j.undo.push(op);
        assert!(!undo_eligible(token, &j, false));
        let token = Some(j.receipt_revision);
        j.record("manual", Some(UndoOp::Params { changes: vec![] }));
        assert!(!undo_eligible(token, &j, false));
        let mut fresh = JournalState::default();
        fresh.record("new project", Some(UndoOp::Params { changes: vec![] }));
        assert!(!undo_eligible(token, &fresh, false));
        assert!(!undo_eligible(None, &fresh, false));
        let mut receipt = Receipt::change("Cu", Stage::Fit, vec![], fresh.receipt_revision);
        assert!(!receipt.can_undo(&fresh, false, 0));
        receipt.state = "Recalculation complete".into();
        assert!(receipt.can_undo(&fresh, false, 0));
        assert!(!receipt.can_undo(&fresh, false, 1)); // unjournalled manual model edit
        receipt.undo_retired = true;
        assert!(!receipt.can_undo(&fresh, false, 0)); // manually changing back stays retired
        assert!(fresh.take_history(true).is_none());
    }
    #[test]
    fn show_results_is_navigation_only() {
        let r = Receipt::job("Fit complete · 2 variables · converged".into(), Some(4));
        assert_eq!(r.navigation, json!({"stage":"fit","fit_step":"results"}));
        assert_eq!(r.journal, None);
        // Guard the actual click target, including its navigation primitives, against
        // accidentally wiring history restoration into this read-only action.
        let source = include_str!("assistant_actions.rs");
        let body = source
            .split("fn assistant_show_result(")
            .nth(1)
            .unwrap()
            .split("fn model_settings(")
            .next()
            .unwrap();
        for forbidden in [
            "restore_fit_history",
            "fit_paths",
            "fit_vars",
            "fit_ranges",
            "restore_model_settings",
            "select_entry",
        ] {
            assert!(
                !body.contains(forbidden),
                "Show Results contains {forbidden}"
            );
        }
    }
}
