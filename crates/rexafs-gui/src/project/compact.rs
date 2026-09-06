//! Compact v1 JSON by omitting only fields whose reader supplies the same value.
//! Scientific arrays, strings, expressions and opaque extension data stay intact.
use super::*;
use serde_json::{Value, json};

fn omit(value: &mut Value, defaults: Value) {
    if let (Some(value), Some(defaults)) = (value.as_object_mut(), defaults.as_object()) {
        value.retain(|key, actual| defaults.get(key) != Some(actual));
    }
}
fn visit(value: &mut Value, key: &str, apply: fn(&mut Value)) {
    if let Some(child) = value.get_mut(key) {
        apply(child);
    }
}
fn each(value: &mut Value, key: &str, apply: fn(&mut Value)) {
    if let Some(items) = value.get_mut(key).and_then(Value::as_array_mut) {
        for item in items {
            apply(item);
        }
    }
}
fn params(value: &mut Value) {
    omit(
        value,
        serde_json::to_value(PipelineParams::default()).unwrap(),
    );
    if let Some(import) = value.get_mut("import") {
        omit(
            import,
            serde_json::to_value(crate::params::ImportConfig::default()).unwrap(),
        );
    }
}
fn ranges(value: &mut Value) {
    let mut defaults = serde_json::to_value(FitRanges::default()).unwrap();
    // A missing *field* means false for legacy fits, while a missing whole
    // fit_ranges object creates the current model default (true).
    defaults["follow_transform"] = false.into();
    omit(value, defaults);
}
fn path(value: &mut Value) {
    omit(value, json!({"ei":"", "third":"", "fourth":""}));
}
fn variable(value: &mut Value) {
    omit(value, json!({"min":null, "max":null, "expr":null}));
}
fn joint(value: &mut Value) {
    omit(
        value,
        serde_json::to_value(crate::joint_fitting::JointConfig::default()).unwrap(),
    );
    each(value, "datasets", |dataset| {
        omit(dataset, json!({"ranges":null,"expressions":{}}));
        visit(dataset, "ranges", ranges);
        if let Some(expressions) = dataset
            .get_mut("expressions")
            .and_then(Value::as_object_mut)
        {
            for expression in expressions.values_mut() {
                path(expression);
            }
        }
    });
}
fn history(value: &mut Value) {
    omit(
        value,
        json!({"joint":null,"path_details":[],"solver_report":null}),
    );
    each(value, "paths", path);
    each(value, "vars", variable);
    visit(value, "ranges", ranges);
    visit(value, "joint", joint);
    each(value, "path_details", |details| {
        omit(
            details,
            json!({"reff":null,"nleg":null,"degeneracy":null,"distance":null,"deltar":null,"sigma2":null,"s02":null,"e0":null}),
        );
        for key in ["distance", "deltar", "sigma2", "s02", "e0"] {
            if let Some(estimate) = details.get_mut(key) {
                omit(estimate, json!({"stderr":null}));
            }
        }
    });
}

pub(super) fn encode(mut value: Value) -> Result<Vec<u8>, String> {
    let expected = value.clone();
    // Compare parent defaults before pruning children: {} may itself have
    // different legacy meaning from an absent parent (notably fit_ranges).
    omit(
        &mut value,
        serde_json::to_value(ProjectFile::default()).map_err(|e| e.to_string())?,
    );
    visit(&mut value, "params", params);
    each(&mut value, "overrides", |entry| {
        visit(entry, "params", params)
    });
    each(&mut value, "fit_paths", path);
    each(&mut value, "fit_vars", variable);
    visit(&mut value, "fit_ranges", ranges);
    each(&mut value, "fit_history", history);
    visit(&mut value, "joint", joint);
    if let Some(publication) = value.get_mut("publication") {
        omit(
            publication,
            serde_json::to_value(crate::publication::figures::FigureSettings::default()).unwrap(),
        );
        if let Some(figures) = publication
            .get_mut("figures")
            .and_then(Value::as_object_mut)
        {
            let defaults =
                serde_json::to_value(crate::publication::figures::FigureOptions::default())
                    .unwrap();
            for figure in figures.values_mut() {
                omit(figure, defaults.clone());
            }
        }
    }
    // Keep provenance at the beginning of the file even though Value's map
    // normally serializes keys alphabetically.
    let bytes = if let Some(header) = value.as_object_mut().and_then(|v| v.remove("header")) {
        let mut bytes = b"{\"header\":".to_vec();
        serde_json::to_writer(&mut bytes, &header).map_err(|e| e.to_string())?;
        let rest = serde_json::to_vec(&value).map_err(|e| e.to_string())?;
        if rest.len() > 2 {
            bytes.push(b',');
            bytes.extend_from_slice(&rest[1..]);
        } else {
            bytes.push(b'}');
        }
        bytes
    } else {
        serde_json::to_vec(&value).map_err(|e| e.to_string())?
    };
    let restored = parse(std::str::from_utf8(&bytes).map_err(|e| e.to_string())?)?;
    if serde_json::to_value(restored).map_err(|e| e.to_string())? != expected {
        return Err(
            "Compact project validation failed: refusing to save a file that changes project data."
                .into(),
        );
    }
    Ok(bytes)
}
