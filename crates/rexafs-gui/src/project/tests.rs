use super::*;

#[test]
fn publication_recipes_migrate_old_rxs_and_save_only_the_new_format() {
    use crate::publication::figures::composer::CompositeRecipe;
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("legacy.rxs");
    let recipe = CompositeRecipe {
        panels: vec!["xanes".into(), "chi-k".into()],
        columns: 1,
        width_mm: 90.,
        dpi: 300.,
        ..Default::default()
    };
    save_with_storage(&path, &ProjectFile::default(), DataStorage::Paths).unwrap();
    let mut legacy = json_file(&path);
    legacy["publication"] = serde_json::json!({"composite": recipe});
    legacy["publication"]["composite"]["panels"] = serde_json::json!(["xanes", "chi-k"]);
    std::fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();
    let mut project = load_with_cache_root(&path, || Ok(temp.path().join("cache"))).unwrap();
    assert_eq!(project.publication.recipes().len(), 1);
    assert_eq!(project.publication.active_recipe().name, "Figure 1");
    assert_eq!(project.publication.active_recipe().recipe, recipe);
    project.publication.duplicate_recipe();
    project.publication.rename_recipe("SI Figure S4").unwrap();
    let saved = temp.path().join("named.rxs");
    save_with_storage(&saved, &project, DataStorage::Paths).unwrap();
    let json: serde_json::Value = serde_json::from_slice(&std::fs::read(&saved).unwrap()).unwrap();
    assert!(json["publication"].get("composite").is_none());
    assert_eq!(json["publication"]["recipes"][0]["name"], "Figure 1");
    assert_eq!(json["publication"]["recipes"][1]["name"], "SI Figure S4");
    assert_eq!(json["publication"]["active_recipe"], 1);
    let restored = load_with_cache_root(&saved, || Ok(temp.path().join("cache"))).unwrap();
    assert_eq!(restored.publication, project.publication);
}

#[test]
fn publication_samples_keep_order_and_panel_options_after_rxs_reopen() {
    use crate::group_identity::GroupId;
    use crate::publication::figures::composer::{CompositePanel, CurveLabelMode};
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("samples.rxs");
    let mut project = ProjectFile::default();
    let samples = vec![GroupId::legacy_result(9), GroupId::legacy_result(2)];
    let recipe = project.publication.active_composite_mut();
    recipe.samples = samples.clone();
    recipe.panels = vec![
        CompositePanel::from("xanes"),
        CompositePanel {
            key: "chi-k".into(),
            samples: Some(samples.iter().rev().cloned().collect()),
            offset_step: 0.3,
            reverse: true,
            label_mode: CurveLabelMode::CurveEnd,
            inset: None,
        },
        CompositePanel {
            samples: Some(Vec::new()),
            ..CompositePanel::from("chi-r")
        },
    ];
    for mode in [DataStorage::Paths, DataStorage::Embedded] {
        save_with_storage(&path, &project, mode).unwrap();
        let saved = json_file(&path);
        let recipe = &saved["publication"]["recipes"][0]["recipe"];
        assert_eq!(
            recipe["samples"],
            serde_json::json!(["result:9", "result:2"])
        );
        assert_eq!(
            recipe["panels"][1]["samples"],
            serde_json::json!(["result:2", "result:9"])
        );
        let reopened = load_with_cache_root(&path, || Ok(temp.path().join("cache"))).unwrap();
        assert_eq!(reopened.publication, project.publication);
    }
}

#[test]
fn publication_named_recipe_panel_strings_load_from_rxs() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("legacy-panels.rxs");
    save_with_storage(&path, &ProjectFile::default(), DataStorage::Paths).unwrap();
    let mut saved = json_file(&path);
    saved["publication"] = serde_json::json!({
        "recipes": [{"name": "Figure 1", "recipe": {"panels": ["xanes", "chi-k"]}}]
    });
    std::fs::write(&path, serde_json::to_vec(&saved).unwrap()).unwrap();
    let reopened = load_with_cache_root(&path, || Ok(temp.path().join("cache"))).unwrap();
    let recipe = &reopened.publication.active_recipe().recipe;
    assert!(recipe.samples.is_empty());
    assert_eq!(
        recipe
            .panels
            .iter()
            .map(|panel| panel.key.as_str())
            .collect::<Vec<_>>(),
        ["xanes", "chi-k"]
    );
    assert!(recipe.panels.iter().all(|panel| panel.samples.is_none()
        && panel.offset_step == 0.
        && !panel.reverse
        && panel.inset.is_none()));
    save_with_storage(&path, &reopened, DataStorage::Paths).unwrap();
    let saved = json_file(&path);
    assert!(saved["publication"]["recipes"][0]["recipe"]["panels"][0].is_object());
    let again = load_with_cache_root(&path, || Ok(temp.path().join("cache"))).unwrap();
    assert_eq!(again.publication, reopened.publication);
}

#[test]
fn legacy_publication_recipes_export_python_without_saved_schema_changes() {
    use crate::publication::figures::composer::{CompositeRecipe, prepare_panels};
    use crate::publication::figures::{FigureData, FigureSeries};
    use crate::publication::python_export::{SCRIPT_NAME, write_composite_python_export};

    let temp = tempfile::tempdir().unwrap();
    for (format, recipe_fields) in [
        (
            "composite",
            serde_json::json!({"composite": {"panels": ["xanes"]}}),
        ),
        (
            "named",
            serde_json::json!({"recipes": [{"name": "Figure 1", "recipe": {"panels": ["xanes"]}}]}),
        ),
    ] {
        let path = temp.path().join(format!("legacy-{format}.rxs"));
        save_with_storage(&path, &ProjectFile::default(), DataStorage::Paths).unwrap();
        let mut historical = json_file(&path);
        historical["publication"] = recipe_fields;
        historical["publication"]["figures"] = serde_json::json!({
            "xanes": {"normalized": true, "legend": false, "xmin": 7100.0, "xmax": 7102.0}
        });
        std::fs::write(&path, serde_json::to_vec(&historical).unwrap()).unwrap();

        let reopened = load_with_cache_root(&path, || Ok(temp.path().join("cache"))).unwrap();
        let named = reopened.publication.active_recipe();
        assert_eq!(named.name, "Figure 1");
        assert_eq!(
            named.recipe,
            CompositeRecipe {
                panels: vec!["xanes".into()],
                ..Default::default()
            }
        );
        let before_export = serde_json::to_value(&reopened.publication).unwrap();
        let baseline = temp.path().join(format!("before-export-{format}.rxs"));
        save_with_storage(&baseline, &reopened, DataStorage::Paths).unwrap();
        let before_export_saved = json_file(&baseline)["publication"].clone();

        // These small arrays are synthetic display data, not a scientific
        // processing fixture. Their distinct values expose the saved choice
        // of normalized rather than flattened absorption in the exported CSV.
        let curve = |y| FigureSeries {
            key: "mu".into(),
            label: "Synthetic sample".into(),
            x: vec![7100., 7101., 7102.],
            y,
            dashed: false,
            optional: false,
        };
        let available = vec![FigureData {
            key: "xanes",
            label: "XANES".into(),
            xlabel: "Energy (eV)".into(),
            ylabel: "Flattened μ(E) (dimensionless)".into(),
            math_ylabel: "Flattened μ(E) (dimensionless)".into(),
            series: vec![curve(vec![10., 20., 30.])],
            normalized_series: Some(vec![curve(vec![0.1, 0.2, 0.3])]),
            e0: None,
            guides: Vec::new(),
            default_xlim: None,
            residual: None,
        }];
        let prepared =
            prepare_panels(&named.recipe, &available, &[], &reopened.publication).unwrap();
        assert!(prepared.missing.is_empty());
        assert_eq!(prepared.panels.len(), 1);
        assert!(prepared.panels[0].options.normalized);
        assert!(!prepared.panels[0].options.legend);
        assert_eq!(prepared.panels[0].options.xmin, Some(7100.));
        assert_eq!(prepared.panels[0].options.xmax, Some(7102.));

        let folder = temp.path().join(format!("python-{format}"));
        let script =
            write_composite_python_export(&named.name, &named.recipe, &prepared, &folder).unwrap();
        assert_eq!(script, folder.join(SCRIPT_NAME));
        assert!(
            std::fs::read_to_string(&script)
                .unwrap()
                .contains("matplotlib")
        );
        let csv_files: Vec<_> = std::fs::read_dir(&folder)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "csv"))
            .collect();
        assert_eq!(csv_files.len(), 1);
        let mut reader = csv::Reader::from_path(&csv_files[0]).unwrap();
        let values: Vec<(f64, f64)> = reader.deserialize().map(Result::unwrap).collect();
        assert_eq!(values, vec![(7100., 0.1), (7101., 0.2), (7102., 0.3)]);

        save_with_storage(&path, &reopened, DataStorage::Paths).unwrap();
        let saved = json_file(&path);
        assert_eq!(
            serde_json::to_value(&reopened.publication).unwrap(),
            before_export
        );
        assert_eq!(saved["publication"], before_export_saved);
        assert!(saved["publication"].get("composite").is_none());
        assert_eq!(
            saved["publication"]["recipes"][0]["recipe"],
            serde_json::json!({
                "panels": [{"key": "xanes", "samples": null, "offset_step": 0.0,
                    "reverse": false, "label_mode": "legend"}],
                "samples": [], "columns": 2, "width_mm": 177.8,
                "panel_height_mm": null, "font_size": 8.0, "line_width": null, "dpi": 600.0
            })
        );
        let again = load_with_cache_root(&path, || Ok(temp.path().join("cache"))).unwrap();
        assert_eq!(again.publication, reopened.publication);
    }
}

#[test]
fn publication_panel_objects_without_insets_load_from_rxs() {
    use crate::group_identity::GroupId;
    use crate::publication::figures::composer::CurveLabelMode;
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("legacy-panel-objects.rxs");
    save_with_storage(&path, &ProjectFile::default(), DataStorage::Paths).unwrap();
    let mut saved = json_file(&path);
    saved["publication"] = serde_json::json!({
        "recipes": [{"name": "Figure 1", "recipe": {"panels": [{
            "key": "xanes",
            "samples": ["result:9", "result:2"],
            "offset_step": 0.3,
            "reverse": true,
            "label_mode": "curve_end"
        }]}}]
    });
    std::fs::write(&path, serde_json::to_vec(&saved).unwrap()).unwrap();
    let reopened = load_with_cache_root(&path, || Ok(temp.path().join("cache"))).unwrap();
    let panel = &reopened.publication.active_recipe().recipe.panels[0];
    assert_eq!(panel.key, "xanes");
    assert_eq!(
        panel.samples,
        Some(vec![GroupId::legacy_result(9), GroupId::legacy_result(2)])
    );
    assert_eq!(panel.offset_step, 0.3);
    assert!(panel.reverse);
    assert_eq!(panel.label_mode, CurveLabelMode::CurveEnd);
    assert!(panel.inset.is_none());
    save_with_storage(&path, &reopened, DataStorage::Paths).unwrap();
    let again = load_with_cache_root(&path, || Ok(temp.path().join("cache"))).unwrap();
    assert_eq!(again.publication, reopened.publication);
}

#[test]
fn publication_panel_insets_survive_rxs_save_and_reopen() {
    use crate::publication::figures::composer::{
        CompositePanel, InsetKind, InsetPlacement, PanelInset,
    };
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("panel-insets.rxs");
    let mut project = ProjectFile::default();
    project.publication.active_composite_mut().panels = [
        (
            "xanes",
            InsetKind::PreEdge,
            InsetPlacement::LowerRight,
            true,
        ),
        (
            "flattened-mu",
            InsetKind::Edge,
            InsetPlacement::UpperLeft,
            false,
        ),
        (
            "mu-energy",
            InsetKind::Difference,
            InsetPlacement::CenterRight,
            true,
        ),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (key, kind, placement, follow_offset))| {
        let mut inset = PanelInset::new(kind);
        inset.energy_range = (-5.5 - index as f64, 12.25 + index as f64);
        inset.placement = placement;
        inset.follow_offset = follow_offset;
        CompositePanel {
            inset: Some(inset),
            ..CompositePanel::from(key)
        }
    })
    .collect();
    for storage in [DataStorage::Paths, DataStorage::Embedded] {
        save_with_storage(&path, &project, storage).unwrap();
        let saved = json_file(&path);
        let panels = &saved["publication"]["recipes"][0]["recipe"]["panels"];
        assert!(panels[0]["inset"].is_object());
        assert_eq!(
            panels[0]["inset"]["energy_range"],
            serde_json::json!([-5.5, 12.25])
        );
        assert_eq!(panels[1]["inset"]["follow_offset"], false);
        let reopened = load_with_cache_root(&path, || Ok(temp.path().join("cache"))).unwrap();
        assert_eq!(reopened.publication, project.publication);
    }
}

#[test]
fn publication_recipes_default_when_old_rxs_has_no_recipe_or_publication() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("no-recipe.rxs");
    for publication in [None, Some(serde_json::json!({"figures": {}}))] {
        save_with_storage(&path, &ProjectFile::default(), DataStorage::Paths).unwrap();
        let mut input = json_file(&path);
        if let Some(publication) = publication {
            input["publication"] = publication;
        } else {
            input.as_object_mut().unwrap().remove("publication");
        }
        std::fs::write(&path, serde_json::to_vec(&input).unwrap()).unwrap();
        let project = load_with_cache_root(&path, || Ok(temp.path().join("cache"))).unwrap();
        assert_eq!(project.publication.recipes().len(), 1);
        assert_eq!(project.publication.active_recipe_index(), 0);
        assert_eq!(project.publication.active_recipe().name, "Figure 1");
        assert_eq!(
            project.publication.active_recipe().recipe,
            crate::publication::figures::composer::CompositeRecipe::default()
        );
        save_with_storage(&path, &project, DataStorage::Paths).unwrap();
        let json: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert!(json["publication"].get("composite").is_none());
        let restored = load_with_cache_root(&path, || Ok(temp.path().join("cache"))).unwrap();
        assert_eq!(restored.publication, project.publication);
    }
}

#[test]
fn bundled_copper_example_is_portable_and_reuses_its_extraction_folder() {
    let temp = tempfile::tempdir().unwrap();
    let project = super::copper_example_in(temp.path()).unwrap();
    assert!(project.origin.is_none());
    assert!(project.params == PipelineParams::default());
    assert!(project.overrides.is_empty());
    assert_eq!(project.raw_files.len(), 53);
    assert_eq!(project.source_groups.len(), 53);
    assert_eq!(project.group_state.marked.len(), 50);
    assert_eq!(
        project.extensions["synthetic_example"]["generation_space"],
        "raw mu(E)"
    );
    assert!(
        project
            .group_state
            .labels
            .values()
            .filter(|s| s.starts_with("Synthetic"))
            .all(|s| s.contains("CuO") && s.contains("Cu₂O") && s.contains('%'))
    );
    let reopened = super::copper_example_in(temp.path()).unwrap();
    assert_eq!(project.raw_files, reopened.raw_files);
    assert_eq!(
        std::fs::read_dir(temp.path().join("project-data"))
            .unwrap()
            .count(),
        1
    );
    let saved = temp.path().join("portable.rxs");
    super::save_with_storage(&saved, &project, DataStorage::Embedded).unwrap();
    let restored =
        super::load_with_cache_root(&saved, || Ok(temp.path().join("other-cache"))).unwrap();
    assert_eq!(restored.raw_files.len(), 53);
    assert_eq!(restored.group_state.labels, project.group_state.labels);
    assert_eq!(
        restored.extensions["synthetic_example"],
        project.extensions["synthetic_example"]
    );
}

/// Local qualification before publishing the user-reviewed example. This reads
/// only an explicitly supplied generated project and saves to a temporary folder.
#[test]
#[ignore = "requires REXAFS_CU_PREVIEW pointing to the locally generated example"]
fn local_copper_preview_preserves_raw_sources_labels_and_truth() {
    let path = std::path::PathBuf::from(
        std::env::var_os("REXAFS_CU_PREVIEW").expect("set REXAFS_CU_PREVIEW"),
    );
    let temp = tempfile::tempdir().unwrap();
    let project = super::load_with_cache_root(&path, || Ok(temp.path().join("cache"))).unwrap();
    assert_eq!(project.raw_files.len(), 53);
    assert_eq!(project.source_groups.len(), 53);
    assert_eq!(project.group_state.marked.len(), 50);
    assert!(
        project
            .group_state
            .labels
            .values()
            .filter(|s| s.starts_with("Synthetic"))
            .all(|s| s.contains("CuO") && s.contains("Cu₂O") && s.contains('%'))
    );
    assert_eq!(
        project.extensions["synthetic_example"]["generation_space"],
        "raw mu(E)"
    );
    assert_eq!(
        project.extensions["synthetic_example"]["truth"]
            .as_array()
            .unwrap()
            .len(),
        50
    );
    let saved = temp.path().join("reopened.rxs");
    super::save_with_storage(&saved, &project, DataStorage::Embedded).unwrap();
    let reopened = super::load_with_cache_root(&saved, || Ok(temp.path().join("cache2"))).unwrap();
    assert_eq!(reopened.raw_files.len(), 53);
    assert_eq!(reopened.group_state.labels, project.group_state.labels);
    assert_eq!(
        reopened.extensions["synthetic_example"],
        project.extensions["synthetic_example"]
    );
}

pub(crate) fn load(path: &Path) -> Result<ProjectFile, String> {
    super::load_with_cache_root(path, || {
        Ok(std::env::temp_dir().join(format!("rexafs-project-test-cache-{}", std::process::id())))
    })
}

use serde_json::{Value, json};
use std::sync::atomic::{AtomicU64, Ordering};

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "rexafs-project-tests-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn join(&self, path: &str) -> PathBuf {
        self.0.join(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/projects")
        .join(name)
}
fn json_file(path: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}
fn copy_inputs(dir: &Path) {
    for name in ["data/cu_150k.xmu", "data/second.xmu", "feff/feff0001.dat"] {
        let dest = dir.join(name);
        std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
        std::fs::copy(fixture(name), dest).unwrap();
    }
}
fn specimen(dir: &Path) -> ProjectFile {
    copy_inputs(dir);
    std::fs::copy(fixture("rexafs-0.1.0-links.rxs"), dir.join("session.rxs")).unwrap();
    load(&dir.join("session.rxs")).unwrap()
}

#[test]
fn energy_offset_and_alignment_history_survive_project_roundtrip_and_zero() {
    use crate::params::{AlignmentRecord, DerivedSpectrum, OperationInput};
    let temp = Temp::new();
    let mut params = PipelineParams::default();
    params.set_energy_offset(3.4).unwrap();
    params.alignment_record = Some(AlignmentRecord {
        inputs: vec![OperationInput {
            label: "Cu reference".into(),
            group_id: None,
            path: PathBuf::new(),
            derived_id: Some(2),
            fingerprint: 0,
            size: None,
        }],
        window_relative_e0_ev: [-50.0, 100.0],
        automatic_shift_ev: 3.0,
        manual_shift_ev: 0.4,
        offset_before_ev: 0.0,
        offset_after_ev: 3.4,
    });
    let group = DerivedSpectrum {
        id: 1,
        label: "Cu test".into(),
        energy: vec![8970.0, 8980.0, 8990.0],
        mu: vec![0.0, 0.5, 1.0],
        params: Some(params.clone()),
        ..Default::default()
    };
    let project = ProjectFile {
        derived: vec![group.clone()],
        ..Default::default()
    };
    for mode in [DataStorage::Paths, DataStorage::Embedded] {
        let file = temp.join("energy-offset.rxs");
        save_with_storage(&file, &project, mode).unwrap();
        let mut loaded = load(&file).unwrap();
        let restored = &mut loaded.derived[0];
        assert_eq!(restored.energy, group.energy);
        assert_eq!(restored.mu, group.mu);
        assert!(restored.params.as_ref().unwrap() == &params);
        assert_eq!(
            restored.raw(&params).unwrap().0,
            vec![8973.4, 8983.4, 8993.4]
        );
        restored
            .params
            .as_mut()
            .unwrap()
            .set_energy_offset(0.0)
            .unwrap();
        save_with_storage(&file, &loaded, mode).unwrap();
        let reset = load(&file).unwrap();
        let reset_group = &reset.derived[0];
        let reset_params = reset_group.params.as_ref().unwrap();
        assert_eq!(reset_group.raw(reset_params).unwrap().0, group.energy);
        assert_eq!(reset_params.alignment_record, params.alignment_record);
    }
}

#[test]
fn joint_materialized_groups_keep_empty_locators_on_save_and_reopen() {
    let temp = Temp::new();
    let mut project = specimen(&temp.join("source"));
    project.joint.datasets[0].file = PathBuf::new();
    project.joint.datasets[0].group_id = Some(project.derived[0].id);
    project.fit_history[0].joint = Some(project.joint.clone());
    let arrays = project.derived[0].mu.clone();
    for mode in [DataStorage::Paths, DataStorage::Embedded] {
        let file = temp.join(if mode == DataStorage::Paths {
            "linked-joint.rxs"
        } else {
            "embedded-joint.rxs"
        });
        let header = save_with_storage(&file, &project, mode).unwrap();
        assert!(
            header
                .files
                .iter()
                .all(|source| source.path.is_relative() && !source.path.as_os_str().is_empty())
        );
        let mut loaded = load(&file).unwrap();
        loaded.assign_group_ids();
        assert!(loaded.joint.datasets[0].file.as_os_str().is_empty());
        assert!(
            loaded.fit_history[0].joint.as_ref().unwrap().datasets[0]
                .file
                .as_os_str()
                .is_empty()
        );
        assert_eq!(loaded.derived[0].mu, arrays);
        save_with_storage(&file, &loaded, mode).unwrap();
    }
}

#[test]
fn fit_result_arrays_and_transforms_survive_reopen_and_old_projects_still_open() {
    use nalgebra::DVector;
    use rexafs::prelude::FeffFitResult;
    use std::sync::Arc;
    let temp = Temp::new();
    // A 0.1.0 project predates saved result arrays and transforms.
    let mut project = specimen(&temp.join("source"));
    assert!(project.fit_results.is_empty());
    assert!(project.fit_history[0].transforms.is_empty());
    assert_eq!(project.current_fit, None);
    assert_eq!(project.fit_history[0].params_fingerprint, None);

    let id = project.fit_history[0].id;
    let transform = project.fit_history[0].ranges.transform();
    project.fit_history[0].transforms = vec![transform.clone()];
    // The fit was current and up to date on a derived group when saved.
    let fingerprint = |p: &ProjectFile| {
        let group = &p.derived[0];
        (
            group.id,
            group.fingerprint(group.params.as_ref().unwrap_or(&p.params)),
        )
    };
    let (group_id, params_fingerprint) = fingerprint(&project);
    let label = project.fit_history[0].group.clone();
    project.fit_history[0].group_id = Some(group_id);
    project.fit_history[0].params_fingerprint = Some(params_fingerprint);
    project.current_fit = Some(id);
    let k = DVector::from_fn(64, |i, _| 0.05 * i as f64);
    let result = FeffFitResult {
        r_factor: 0.0123,
        data_chi: k.map(f64::sin),
        model_chi: k.map(|x| 0.9 * x.sin()),
        k,
        kweights: vec![2.0],
        ..Default::default()
    };
    let unreadable = FeffFitResult {
        r_factor: f64::NAN,
        ..Default::default()
    };
    let mut second = project.fit_history[0].clone();
    second.id = id + 1;
    project.fit_history.push(second);
    project.fit_results = [
        (id, Arc::new(result.clone())),
        (id + 1, Arc::new(unreadable)),
        (id + 99, Arc::new(result.clone())), // No history entry.
    ]
    .into();

    for mode in [DataStorage::Paths, DataStorage::Embedded] {
        let file = temp.join(&format!("fit-results-{mode:?}.rxs"));
        save_with_storage(&file, &project, mode).unwrap();
        let loaded = load(&file).unwrap();
        // Non-finite and orphaned results are dropped, not the whole save.
        assert_eq!(loaded.fit_results.keys().copied().collect::<Vec<_>>(), [id]);
        assert_eq!(*loaded.fit_results[&id], result);
        assert_eq!(
            loaded.fit_history[0].transforms,
            std::slice::from_ref(&transform)
        );
        assert_eq!(loaded.fit_history.len(), 2);
        // Reopening shows the saved current fit, and the reloaded, unedited
        // spectrum is recognised as the fitted one (not stale).
        assert_eq!(loaded.current_fit, Some(id));
        let (group_id, params_fingerprint) = fingerprint(&loaded);
        let entry = &loaded.fit_history[0];
        assert!(entry.matches_spectrum(Some(group_id), &label, params_fingerprint));
        assert!(!entry.matches_spectrum(None, &label, params_fingerprint));
        assert!(!entry.matches_spectrum(Some(group_id), &label, params_fingerprint ^ 1));
    }
    // A current fit whose arrays cannot be saved is not restored.
    project.current_fit = Some(id + 1);
    let file = temp.join("unreadable-current.rxs");
    save_with_storage(&file, &project, DataStorage::Paths).unwrap();
    assert_eq!(load(&file).unwrap().current_fit, None);
}

#[test]
fn embedded_restore_uses_the_injected_cache_and_links_need_no_cache() {
    let temp = Temp::new();
    let project = specimen(&temp.join("source"));
    super::load_with_cache_root(&temp.join("source/session.rxs"), || {
        panic!("linked projects must not initialize a cache")
    })
    .unwrap();
    let portable = temp.join("portable.rxs");
    save_with_storage(&portable, &project, DataStorage::Embedded).unwrap();
    let mut restored_paths = Vec::new();
    for name in ["cache-a", "cache-b"] {
        let root = temp.join(name);
        let restored = super::load_with_cache_root(&portable, || Ok(root.clone())).unwrap();
        let source = restored.spectrum_file.unwrap();
        assert!(source.starts_with(root));
        assert_eq!(
            std::fs::read(&source).unwrap(),
            std::fs::read(project.spectrum_file.as_ref().unwrap()).unwrap()
        );
        restored_paths.push(source);
    }
    assert_ne!(restored_paths[0], restored_paths[1]);
}

#[test]
fn parser_totals_and_line_examples_survive_linked_and_embedded_reopen() {
    let temp = Temp::new();
    let mut project = specimen(&temp.join("source"));
    project.assign_group_ids();
    let path = project.spectrum_file.clone().unwrap();
    let mut text = std::fs::read_to_string(&path).unwrap();
    text.push_str(&"\nmalformed row".repeat(9));
    std::fs::write(&path, text).unwrap();
    let preview = crate::params::preview_import(&path, &project.params.import).unwrap();
    assert_eq!(preview.diagnostics.malformed_rows.count, 9);
    assert_eq!(preview.diagnostics.malformed_rows.examples.len(), 5);
    let group = project
        .source_groups
        .iter()
        .find(|g| g.path == path)
        .unwrap()
        .id
        .clone();
    project.parser_evidence.insert(
        group.clone(),
        crate::source_evidence::ParserRecord {
            path: path.clone(),
            channel: preview.resolved.mode,
            mapping_revision: crate::import_recipes::mapping_revision(&project.params.import),
            diagnostics: preview.diagnostics.clone(),
            declared_edge: None,
        },
    );
    for mode in [DataStorage::Paths, DataStorage::Embedded] {
        let saved = temp.join(if mode == DataStorage::Paths {
            "linked.rxs"
        } else {
            "embedded.rxs"
        });
        save_with_storage(&saved, &project, mode).unwrap();
        let restored = load(&saved).unwrap();
        let record = &restored.parser_evidence[&group];
        assert_eq!(record.diagnostics, preview.diagnostics);
        assert_eq!(record.path, *restored.spectrum_file.as_ref().unwrap());
        assert!(record.matches(&restored.params.import));
        let mut edited = restored.params.import.clone();
        edited.axis = crate::import_mapping::AxisConversion::EnergyKev;
        assert!(
            !record.matches(&edited),
            "old diagnostics cannot describe another mapping"
        );
    }
}
#[test]
fn resaving_a_live_copy_and_its_original_keeps_one_verified_source() {
    let temp = Temp::new();
    let original = temp.join("average.dat");
    let copy = temp.join("portable-average.dat");
    std::fs::write(&original, b"1 2\n2 3\n").unwrap();
    std::fs::copy(&original, &copy).unwrap();
    let mut project = ProjectFile::default();
    project.spectrum_file = Some(original.clone());
    project.raw_files = vec![copy.clone()];
    project.source_groups = vec![crate::group_identity::SourceGroup {
        id: crate::group_identity::GroupId::source(&copy, crate::params::DetectionMode::Auto),
        path: copy.clone(),
        channel: crate::params::DetectionMode::Auto,
    }];
    project
        .source_origins
        .insert(copy.clone(), original.clone());
    let saved = temp.join("saved.rxs");
    save_with_storage(&saved, &project, DataStorage::Embedded).unwrap();
    assert_eq!(
        json_file(&saved)["header"]["files"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let reopened = load(&saved).unwrap();
    assert_eq!(
        reopened.spectrum_file.as_ref(),
        Some(&reopened.source_groups[0].path)
    );
    assert_eq!(
        std::fs::read(reopened.spectrum_file.unwrap()).unwrap(),
        b"1 2\n2 3\n"
    );
    std::fs::write(&original, b"1 4\n2 5\n").unwrap();
    let before = std::fs::read(&saved).unwrap();
    assert!(
        save_with_storage(&saved, &project, DataStorage::Embedded)
            .unwrap_err()
            .contains("Conflicting input revisions")
    );
    assert_eq!(
        std::fs::read(&saved).unwrap(),
        before,
        "a failed save preserves the previous project"
    );
}

fn state(project: &ProjectFile) -> Value {
    let mut project = project.clone();
    let origins = project.source_origins.clone();
    storage::map_paths(&mut project, &mut |p| {
        let source = origins.get(p).cloned().unwrap_or_else(|| p.to_owned());
        // Embedded reloads may spell the same original file through a directory
        // alias (notably /tmp vs /private/tmp on macOS). Compare file identity
        // after undoing the cache mapping. Resolve an existing ancestor when
        // the portability test has deliberately removed the original files.
        Ok(storage::resolved_location(&source))
    })
    .unwrap();
    let mut value = serde_json::to_value(project).unwrap();
    for key in ["header", "embedded"] {
        value.as_object_mut().unwrap().remove(key);
    }
    value
}

#[test]
fn recipes_exact_applications_and_pending_sources_survive_conflicting_machine_library() {
    use crate::{
        app::{
            import_review::PendingSource,
            import_state::{IntakeBatch, IntakeState, SourceOutcome},
        },
        group_identity::{GroupId, SourceGroup},
        import_recipes::*,
        params::{DetectionMode, ImportConfig},
    };
    for mode in [DataStorage::Paths, DataStorage::Embedded] {
        let dir = Temp::new();
        let source = dir.join("sample.dat");
        std::fs::write(
            &source,
            "# energy i0 it ir\n8900 10 5 2\n9000 12 5 2\n9100 14 5 2\n",
        )
        .unwrap();
        let source = source.canonicalize().unwrap();
        let pending = source.parent().unwrap().join("unreadable.dat");
        let table = crate::params::preview_import(&source, &ImportConfig::default()).unwrap();
        let recipe = RecipeVersion::from_review(
            "Saved beamline",
            RecipeScope::for_source(&source, None),
            &table,
            DetectionMode::Transmission,
            &[
                ImportConfig {
                    mode: DetectionMode::Transmission,
                    ..Default::default()
                },
                ImportConfig {
                    mode: DetectionMode::Reference,
                    ..Default::default()
                },
            ],
            true,
        )
        .unwrap();
        let primary = GroupId::source(&source, DetectionMode::Transmission);
        let reference = GroupId::source(&source, DetectionMode::Reference);
        let mappings = recipe.channels.clone();
        let params = PipelineParams {
            import: mappings[0].clone(),
            ..Default::default()
        };
        let mut project = ProjectFile {
            spectrum_file: Some(source.clone()),
            raw_files: vec![source.clone()],
            params: params.clone(),
            source_groups: vec![SourceGroup {
                id: primary.clone(),
                path: source.clone(),
                channel: DetectionMode::Transmission,
            }],
            overrides: vec![ParamOverride {
                path: source.clone(),
                params,
            }],
            derived: vec![DerivedSpectrum {
                id: 1,
                group_id: Some(reference.clone()),
                source: Some(source.clone()),
                params: Some(PipelineParams {
                    import: mappings[1].clone(),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            import_history: vec![IntakeBatch {
                paths: vec![source.parent().unwrap().into()],
                stopped: false,
                finished: true,
                dropped_queue: 0,
                sources: std::collections::BTreeMap::from([
                    (
                        source.clone(),
                        SourceOutcome {
                            created: vec![primary.clone(), reference.clone()],
                            ..Default::default()
                        },
                    ),
                    (
                        pending.clone(),
                        SourceOutcome {
                            pending: Some(PendingSource {
                                measurement_reader: false,
                                detection: None,
                                suggestion: None,
                                reason: "Locate source".into(),
                            }),
                            ..Default::default()
                        },
                    ),
                ]),
            }],
            ..Default::default()
        };
        project.imports.recipes.remember(recipe.clone()).unwrap();
        project.imports.applications.push(ImportApplication {
            id: "application:original-batch".into(),
            batch: 0,
            recipe: recipe.reference.clone(),
            members: [primary.clone(), reference.clone()]
                .into_iter()
                .zip(&mappings)
                .map(|(group, mapping)| ApplicationMember {
                    source_id: primary.clone(),
                    path: source.clone(),
                    group,
                    channel: mapping.mode,
                    mapping_revision: mapping_revision(mapping),
                })
                .collect(),
        });
        let saved = dir.join("recipe.rxs");
        save_with_storage(&saved, &project, mode).unwrap();

        let mut machine = crate::settings::UserSettings::default();
        machine.import_recipes.remember(recipe.clone()).unwrap();
        let mut conflict = recipe.clone();
        conflict.channels[0].axis = crate::import_mapping::AxisConversion::EnergyKev;
        machine.import_recipes.commit_review(conflict);
        let machine: crate::settings::UserSettings =
            serde_json::from_value(serde_json::to_value(machine).unwrap()).unwrap();
        assert_eq!(machine.import_recipes.versions.len(), 2);

        let mut reopened = load(&saved).unwrap();
        reopened.assign_group_ids();
        assert_eq!(
            reopened.imports.recipes.get(&recipe.reference).unwrap(),
            &recipe
        );
        assert!(reopened.overrides[0].params.import == mappings[0]);
        assert!(reopened.derived[0].params.as_ref().unwrap().import == mappings[1]);
        let application = reopened.imports.application_for(&reference).unwrap();
        assert_eq!(application.batch, 0);
        assert_eq!(application.members.len(), 2);
        assert_eq!(application.members[0].group, primary);
        assert_eq!(
            application.members[1].mapping_revision,
            mapping_revision(&mappings[1])
        );
        assert_eq!(application.members[0].path, reopened.overrides[0].path);
        let history = IntakeState::from_history(reopened.import_history.clone());
        assert!(history.is_pending(&pending));
        assert_eq!(
            history.history[0].sources[&reopened.overrides[0].path].created,
            vec![primary, reference]
        );
        assert_eq!(history.receipt, Some(0));
    }
}

#[test]
fn assistant_conversations_roundtrip_in_both_storage_modes_and_limit_at_save() {
    use super::assistant::{Conversation, ConversationEntry, ConversationMode};
    for mode in [DataStorage::Paths, DataStorage::Embedded] {
        let dir = Temp::new();
        let mut project = specimen(&dir.0);
        for n in 0..2 {
            let mut conversation = Conversation::new(&format!("Check fit {n}"), n == 1);
            conversation.id = format!("conversation-{n}");
            conversation.updated_at = format!("2026-09-08T00:0{n}:00Z");
            conversation.thread_id = Some(format!("server-thread-{n}"));
            conversation.entries = vec![
                ConversationEntry::User {
                    text: format!("Check fit {n}"),
                    edit: n == 1,
                },
                ConversationEntry::Thinking {
                    id: "reason".into(),
                    text: "Inspect normalization first.".into(),
                },
                ConversationEntry::Assistant {
                    id: "answer".into(),
                    text: "The ranges are consistent.".into(),
                },
            ];
            project.assistant.upsert(conversation);
        }
        let path = dir.join("conversations.rxs");
        save_with_storage(&path, &project, mode).unwrap();
        let restored = load(&path).unwrap();
        assert_eq!(
            restored.assistant.conversations,
            project.assistant.conversations
        );
        assert_eq!(
            restored.assistant.conversations[0].mode,
            ConversationMode::Edit
        );
        assert_eq!(
            json_file(&path)["assistant"]["conversations"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        project.assistant.limit = Some(1);
        save_with_storage(&path, &project, mode).unwrap();
        let restored = load(&path).unwrap();
        assert_eq!(restored.assistant.conversations.len(), 1);
        assert_eq!(restored.assistant.conversations[0].id, "conversation-1");
        project.assistant.limit = Some(0);
        save_with_storage(&path, &project, mode).unwrap();
        assert!(load(&path).unwrap().assistant.conversations.is_empty());
    }
}

/// Explicit maintainer operation; never modifies a retained fixture by default.
#[test]
#[ignore = "requires REXAFS_FIXTURE_OUTPUT; writes a new release fixture pair"]
fn write_release_compatibility_fixtures() {
    use crate::spectrum_colors::{Assignment, Palette};
    let output = std::env::var_os("REXAFS_FIXTURE_OUTPUT")
        .map(PathBuf::from)
        .expect("set REXAFS_FIXTURE_OUTPUT to the retained fixture directory");
    std::fs::create_dir_all(&output).unwrap();
    let version = env!("CARGO_PKG_VERSION");
    for suffix in ["links", "embedded"] {
        assert!(
            !output
                .join(format!("rexafs-{version}-{suffix}.rxs"))
                .exists(),
            "never overwrite a retained release fixture"
        );
    }
    let mut project = load(&fixture("rexafs-0.2.0-links.rxs")).unwrap();
    project.assign_group_ids();
    project.params.bkg_kweight = Some(1);
    project.params.fft_kweight = Some(3.);
    project.params.bkg_kweight_linked = true;
    let current = project
        .source_groups
        .iter()
        .find(|group| Some(&group.path) == project.spectrum_file.as_ref())
        .unwrap()
        .id
        .clone();
    project.active_derived = None;
    project.group_state.current = Some(current.clone());
    for entry in &mut project.overrides {
        if Some(&entry.path) == project.spectrum_file.as_ref() {
            entry.params = project.params.clone();
        }
    }
    project
        .group_state
        .overrides
        .retain(|(id, _)| id != &current);
    project
        .group_state
        .overrides
        .push((current, project.params.clone()));
    let count = project.source_groups.len();
    for (index, group) in project.source_groups.iter().enumerate() {
        project.group_state.plot_colors.insert(
            group.id.clone(),
            Assignment {
                palette: Palette::Viridis,
                index,
                count,
                reversed: true,
            },
        );
    }
    project.publication.figures.insert(
        "chi-k".into(),
        crate::publication::figures::FigureOptions {
            title: Some("銅 K-edge / Cu".into()),
            caption: Some("Synthetic persistence example; retained spectrum data.".into()),
            line_width: Some(2.),
            dpi: Some(300.),
            ..Default::default()
        },
    );
    add_synthetic_collection_results(&mut project);
    // Retain explicit 0.2.11 RMC controls without presenting a synthetic saved
    // refinement as an experimental result. Native worker tests cover checkpoints.
    project.rmc.draft.auto_moves = true;
    project.rmc.draft.workers = Some(4);
    project.rmc.draft.parallel_paths = true;
    project.rmc.draft.refine_energy = true;
    project.rmc.draft.energy_refinement.bounds = [-10., 10.];
    project.rmc.draft.energy_refinement.interval = 100;
    for (suffix, mode) in [
        ("links", DataStorage::Paths),
        ("embedded", DataStorage::Embedded),
    ] {
        let path = output.join(format!("rexafs-{version}-{suffix}.rxs"));
        save_with_storage(&path, &project, mode).unwrap();
        let restored = load(&path).unwrap();
        assert_eq!(state(&restored), state(&project));
        assert_eq!(restored.assistant.conversations.len(), 2);
        assert_eq!(restored.fit_paths[0].degen, "4");
    }
}

/// Small numerical persistence example, not an experimental recovery claim.
fn add_synthetic_collection_results(project: &mut ProjectFile) {
    use rexafs::prelude::*;
    let energy: Vec<_> = (0..=25).map(|i| 8980. + 2. * i as f64).collect();
    let spectra: Vec<_> = [0., 0.4, 1.]
        .into_iter()
        .enumerate()
        .map(|(index, fraction)| {
            let values: Vec<_> = energy
                .iter()
                .map(|e| {
                    let first = 0.5 + ((e - 8998.) / 4.).atan() / std::f64::consts::PI;
                    let second = 0.5 + ((e - 9005.) / 3.).atan() / std::f64::consts::PI;
                    fraction * first + (1. - fraction) * second
                })
                .collect();
            let mut spectrum =
                Spectrum::from_prepared(&energy, &values, AnalysisSpace::Flat, 9000.).unwrap();
            spectrum.set_name(format!("Synthetic persistence sample {}", index + 1));
            spectrum
        })
        .collect();
    let inputs: Vec<_> = (0..spectra.len())
        .map(|index| AnalysisInput {
            corrections: Vec::new(),
            group_id: None,
            label: format!("Synthetic persistence sample {}", index + 1),
            fingerprint: 0,
        })
        .collect();
    let lcf_config = LcfConfig {
        space: AnalysisSpace::Flat,
        ..Default::default()
    };
    let standards = [&spectra[0], &spectra[2]];
    let result = lcf(&spectra[1], &standards, &lcf_config).unwrap();
    project.lcf_analysis = Some(LcfAnalysis {
        config: Some(lcf_config.clone()),
        result,
        inputs: vec![inputs[1].clone(), inputs[0].clone(), inputs[2].clone()],
    });
    let rows = lcf_batch(&spectra, &standards, &lcf_config)
        .into_iter()
        .enumerate()
        .map(|(index, result)| {
            let result = result.unwrap();
            let mut values: Vec<_> = result.weights.iter().map(|weight| weight.weight).collect();
            values.push(result.r_factor);
            (index, values)
        })
        .collect();
    project.lcf_series_analysis = Some(LcfSeriesAnalysis {
        config: lcf_config,
        inputs: inputs.iter().cloned().enumerate().collect(),
        standards: vec![inputs[0].clone(), inputs[2].clone()],
        rows,
        errors: Default::default(),
        complete: true,
        cancelled: false,
    });
    let model = pca_train(
        &spectra,
        &PcaConfig {
            space: AnalysisSpace::Flat,
            center: true,
            ..Default::default()
        },
    )
    .unwrap();
    let target = model.target_transform(&spectra[1], 1).unwrap();
    project.pca_analysis = Some(PcaAnalysis {
        model,
        target: Some(target),
        inputs: std::iter::once(inputs[1].clone())
            .chain(inputs.iter().cloned())
            .collect(),
    });
    project.mcr_analysis = Some(McrAnalysis {
        result: mcr_als(
            &spectra,
            &McrConfig {
                space: AnalysisSpace::Flat,
                components: 2,
                ..Default::default()
            },
        )
        .unwrap(),
        inputs,
        comparison: None,
    });
}

#[test]
fn released_029_fixtures_retain_collection_results() {
    use rexafs::prelude::AnalysisSpace;
    for suffix in ["links", "embedded"] {
        let project = load(&fixture(&format!("rexafs-0.2.9-{suffix}.rxs"))).unwrap();
        let lcf = project.lcf_analysis.unwrap();
        assert_eq!(lcf.result.space, AnalysisSpace::Flat);
        assert!((lcf.result.weights[0].weight - 0.6).abs() < 1e-10);
        assert!((lcf.result.weights[1].weight - 0.4).abs() < 1e-10);
        assert_eq!(project.lcf_series_analysis.unwrap().rows.len(), 3);
        let pca = project.pca_analysis.unwrap();
        assert!(pca.model.centered);
        assert_eq!(pca.model.numerical_rank(), 1);
        assert!(pca.target.unwrap().r_factor < 1e-20);
        let mcr = project.mcr_analysis.unwrap();
        assert_eq!(mcr.result.config.space, AnalysisSpace::Flat);
        assert_eq!(mcr.result.concentrations.shape(), (3, 2));
        assert!(mcr.result.relative_error < 1e-20);
        assert!(mcr.result.component_spectrum(0).is_ok());
    }
}

#[test]
fn released_021_fixtures_preserve_weight_link_palette_and_publication() {
    use crate::spectrum_colors::Palette;
    for suffix in ["links", "embedded"] {
        let mut project = load(&fixture(&format!("rexafs-0.2.1-{suffix}.rxs"))).unwrap();
        assert!(project.params.bkg_kweight_linked);
        assert_eq!(project.params.effective_bkg_kweight(), 3);
        project.params.bkg_kweight_linked = false;
        assert_eq!(project.params.effective_bkg_kweight(), 1);
        assert!(!project.group_state.plot_colors.is_empty());
        assert_eq!(
            project.group_state.plot_colors.len(),
            project.source_groups.len()
        );
        for group in &project.source_groups {
            let assignment = project.group_state.plot_colors[&group.id];
            assert_eq!(assignment.palette, Palette::Viridis);
            assert!(assignment.reversed);
            assert_eq!(assignment.count, project.source_groups.len());
        }
        let figure = project.publication.options("chi-k");
        assert_eq!(figure.title.as_deref(), Some("銅 K-edge / Cu"));
        assert_eq!(figure.line_width, Some(2.));
        assert_eq!(figure.dpi, Some(300.));
        assert_eq!(project.imports.applications[0].recipe.version, 1);
        assert_eq!(project.assistant.conversations.len(), 2);
    }
}

#[test]
fn publication_series_colors_default_for_old_projects_and_survive_save_and_reopen() {
    let temp = Temp::new();
    for suffix in ["links", "embedded"] {
        let mut project =
            super::load_with_cache_root(&fixture(&format!("rexafs-0.2.1-{suffix}.rxs")), || {
                Ok(temp.join(&format!("source-cache-{suffix}")))
            })
            .unwrap();
        assert!(
            project
                .publication
                .figures
                .values()
                .all(|options| options.series_colors.is_empty())
        );
        assert!(
            project
                .publication
                .options("chi-k")
                .series_colors
                .is_empty()
        );
        assert!(
            project
                .publication
                .active_recipe()
                .recipe
                .series_colors
                .is_empty()
        );

        // Series keys are opaque: retain unfamiliar keys for future curves.
        let future_key = "future:dataset/χ series \"v2\" #7";
        let figure = project
            .publication
            .figures
            .entry("chi-k".into())
            .or_default();
        figure.series_colors = [
            ("data".into(), "#0072b2".into()),
            (future_key.into(), "#e69f00".into()),
        ]
        .into();
        project.publication.active_composite_mut().series_colors = [
            ("data".into(), "#009e73".into()),
            (future_key.into(), "#cc79a7".into()),
        ]
        .into();
        let expected = project.publication.clone();

        for mode in [DataStorage::Paths, DataStorage::Embedded] {
            let saved = temp.join(&format!("series-colors-{suffix}-{mode:?}.rxs"));
            save_with_storage(&saved, &project, mode).unwrap();
            let reopened = super::load_with_cache_root(&saved, || {
                Ok(temp.join(&format!("reopened-cache-{suffix}-{mode:?}")))
            })
            .unwrap();
            assert_eq!(reopened.publication, expected);
        }
    }
}

#[test]
fn released_020_fixtures_retain_import_state_and_historical_evidence() {
    for suffix in ["links", "embedded"] {
        let project = load(&fixture(&format!("rexafs-0.2.0-{suffix}.rxs"))).unwrap();
        assert_eq!(project.imports.recipes.versions.len(), 2);
        let application = &project.imports.applications[0];
        assert_eq!(application.recipe.version, 1);
        assert_eq!(application.members.len(), 1);
        let member = &application.members[0];
        let recipe = project.imports.recipes.get(&application.recipe).unwrap();
        assert!(!recipe.reuse);
        assert!(project.imports.recipes.versions[1].reuse);
        assert!(recipe.scope.contains(&member.path, None));
        assert_eq!(
            member.mapping_revision,
            crate::import_recipes::mapping_revision(&recipe.channels[0])
        );
        let record = &project.parser_evidence[&member.group];
        assert_eq!(record.path, member.path);
        assert_eq!(record.declared_edge.as_ref().unwrap().label(), "Cu K");
        assert!(record.matches(&recipe.channels[0]));
        let (group, warning) = project
            .parser_evidence
            .iter()
            .find(|(_, record)| record.diagnostics.malformed_rows.count == 9)
            .unwrap();
        assert_eq!(warning.diagnostics.malformed_rows.examples.len(), 5);
        assert!(project.group_state.marked.contains(group));
        assert!(project.group_state.frozen.contains(group));
        assert_eq!(
            project.import_history[0]
                .sources
                .values()
                .filter(|s| s.pending.is_some())
                .count(),
            1
        );
        assert!(
            project
                .derived
                .iter()
                .any(|d| d.source.is_none() && d.declared_edge.is_some())
        );
        assert_eq!(project.assistant.conversations.len(), 2);
    }
}

#[test]
fn format_one_defaults_keep_their_released_meaning() {
    fn preserved(expected: &Value, actual: &Value) -> bool {
        match expected.as_object() {
            Some(fields) => {
                actual.is_object()
                    && fields.iter().all(|(key, value)| {
                        actual
                            .get(key)
                            .is_some_and(|actual| preserved(value, actual))
                    })
            }
            None => expected == actual,
        }
    }
    let actual = state(&load(&fixture("minimal-v1.rxs")).unwrap());
    let expected = json_file(&fixture("format-v1-defaults.json"));
    assert!(
        preserved(&expected, &actual),
        "Format 1 defaults changed: migrate existing files instead of reinterpreting omitted fields. Current defaults: {}",
        serde_json::to_string(&actual).unwrap()
    );
}

#[test]
fn every_release_fixture_loads_saves_and_reopens_without_losing_state() {
    let manifest = json_file(&fixture("manifest.json"));
    let invalid = manifest["invalid_projects"].as_array().unwrap();
    let temp = Temp::new();
    for name in manifest["sha256"]
        .as_object()
        .unwrap()
        .keys()
        .filter(|n| n.ends_with(".rxs"))
    {
        let original = std::fs::read(fixture(name)).unwrap();
        if invalid.contains(&json!(name)) {
            assert!(load(&fixture(name)).is_err(), "{name}");
        } else {
            let project = load(&fixture(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
            let saved = temp.join(name);
            save(&saved, &project).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(std::fs::read(&saved).unwrap().starts_with(b"{\"header\":"));
            let reopened = load(&saved).unwrap();
            assert_eq!(state(&project), state(&reopened), "{name}");
            assert_eq!(
                project.header.as_ref().unwrap().created_utc,
                reopened.header.as_ref().unwrap().created_utc
            );
            assert_eq!(project.data_storage, reopened.data_storage);
        }
        assert_eq!(
            original,
            std::fs::read(fixture(name)).unwrap(),
            "fixture was modified"
        );
    }
}

#[test]
fn linked_project_moves_with_data_and_save_as_rebases_every_owned_path() {
    let temp = Temp::new();
    let old = temp.join("old");
    let project = specimen(&old);
    save(&old.join("session.rxs"), &project).unwrap();
    let disk = json_file(&old.join("session.rxs"));
    assert_eq!(disk["header"]["storage"], "paths");
    assert_eq!(disk["header"]["path_base"], "project_directory");
    assert_eq!(disk["spectrum_file"], "data/cu_150k.xmu");
    assert!(disk.get("embedded").is_none());
    let moved = temp.join("moved");
    std::fs::rename(old, &moved).unwrap();
    let reopened = load(&moved.join("session.rxs")).unwrap();
    let mut paths = Vec::new();
    storage::map_paths(&mut reopened.clone(), &mut |p| {
        paths.push(p.to_owned());
        Ok(p.to_owned())
    })
    .unwrap();
    assert!(paths.len() >= 8);
    assert!(paths.iter().all(|p| p.starts_with(&moved) && p.exists()));
    std::fs::create_dir(moved.join("projects")).unwrap();
    let save_as = moved.join("projects/new.rxs");
    save(&save_as, &reopened).unwrap();
    assert_eq!(json_file(&save_as)["spectrum_file"], "../data/cu_150k.xmu");
    assert_eq!(state(&reopened), state(&load(&save_as).unwrap()));
}

#[test]
#[cfg(unix)]
fn saving_through_a_directory_alias_keeps_links_portable() {
    use std::os::unix::fs::symlink;
    let temp = Temp::new();
    let real = temp.join("real");
    let project = specimen(&real);
    let alias = temp.join("alias");
    symlink(&real, &alias).unwrap();
    let mut opened_through_alias = project.clone();
    storage::map_paths(&mut opened_through_alias, &mut |p| {
        Ok(alias.join(p.strip_prefix(&real).unwrap()))
    })
    .unwrap();
    let file = real.join("native-dialog.rxs");
    save(&file, &opened_through_alias).unwrap();
    assert_eq!(json_file(&file)["source_dir"], "data");
    assert_eq!(json_file(&file)["spectrum_file"], "data/cu_150k.xmu");
    std::fs::remove_file(alias).unwrap();
    let moved = temp.join("moved");
    std::fs::rename(real, &moved).unwrap();
    let restored = load(&moved.join("native-dialog.rxs")).unwrap();
    assert!(restored.spectrum_file.unwrap().is_file());
}

#[test]
fn embedded_project_is_lossless_self_contained_and_can_be_saved_again() {
    let temp = Temp::new();
    let source = temp.join("original");
    let mut project = specimen(&source);
    // These workspace files must travel with the paths, including raw bytes
    // that are not representable as UTF-8 JSON text.
    let engine = b"engine: refeff\r\n\xff\x00";
    std::fs::write(source.join("feff/engine.txt"), engine).unwrap();
    std::fs::write(source.join("feff/crystal.json"), b"{\"name\":\"Cu\"}").unwrap();
    // Cover nested path-map keys as well as values, in current and historical fits.
    project.joint.datasets[0].expressions.insert(
        project.fit_paths[0].file.clone(),
        project.fit_paths[0].clone(),
    );
    project.fit_history[0].joint = Some(project.joint.clone());
    let expected = state(&project);
    let before =
        crate::params::process_file(project.spectrum_file.as_ref().unwrap(), &project.params)
            .unwrap();
    let portable = temp.join("portable.rxs");
    save_with_storage(&portable, &project, DataStorage::Embedded).unwrap();
    let stored = json_file(&portable);
    assert_eq!(stored["header"]["storage"], "embedded");
    assert_eq!(
        stored["embedded"].as_object().unwrap().len(),
        4,
        "identical raw files deduplicate"
    );
    let file_count = stored["header"]["files"].as_array().unwrap().len();
    assert_eq!(file_count, 5);
    std::fs::remove_dir_all(&source).unwrap();
    let reopened = load(&portable).unwrap();
    assert_eq!(state(&reopened), expected);
    assert_eq!(
        std::fs::read(reopened.spectrum_file.as_ref().unwrap()).unwrap(),
        std::fs::read(fixture("data/cu_150k.xmu")).unwrap()
    );
    assert_eq!(
        std::fs::read(&reopened.fit_paths[0].file).unwrap(),
        std::fs::read(fixture("feff/feff0001.dat")).unwrap()
    );
    assert_eq!(
        std::fs::read(reopened.feff_workspace.as_ref().unwrap().join("engine.txt")).unwrap(),
        engine
    );
    assert_eq!(
        reopened.fit_paths[0].file.parent(),
        reopened.feff_workspace.as_deref()
    );
    let after =
        crate::params::process_file(reopened.spectrum_file.as_ref().unwrap(), &reopened.params)
            .unwrap();
    assert_eq!(before.e0(), after.e0());
    assert_eq!(
        before
            .chi()
            .unwrap()
            .iter()
            .map(|v| v.to_bits())
            .collect::<Vec<_>>(),
        after
            .chi()
            .unwrap()
            .iter()
            .map(|v| v.to_bits())
            .collect::<Vec<_>>()
    );
    let resaved = temp.join("resaved.rxs");
    save(&resaved, &reopened).unwrap();
    assert_eq!(state(&load(&resaved).unwrap()), expected);
    assert_eq!(
        stored["header"]["files"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| (&f["path"], &f["modified_unix_seconds"]))
            .collect::<Vec<_>>(),
        json_file(&resaved)["header"]["files"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| (&f["path"], &f["modified_unix_seconds"]))
            .collect::<Vec<_>>()
    );
}

#[test]
fn default_paths_can_record_missing_sources_but_embedding_cannot_drop_them() {
    let temp = Temp::new();
    let project = ProjectFile {
        spectrum_file: Some(temp.join("missing.xmu")),
        ..Default::default()
    };
    assert_eq!(project.data_storage, DataStorage::Paths);
    let path = temp.join("session.rxs");
    save(&path, &project).unwrap();
    let before = std::fs::read(&path).unwrap();
    let header = json_file(&path)["header"].clone();
    assert_eq!(header["files"][0]["path"], "missing.xmu");
    assert!(header["files"][0].get("sha256").is_none());
    assert!(load(&path).is_ok());
    assert!(save_with_storage(&path, &project, DataStorage::Embedded).is_err());
    assert_eq!(before, std::fs::read(path).unwrap());
}

#[test]
fn metadata_records_source_comments_checksums_and_writer() {
    let temp = Temp::new();
    let project = specimen(&temp.join("source"));
    let path = temp.join("session.rxs");
    let header = save_with_storage(&path, &project, DataStorage::Paths).unwrap();
    assert_eq!(header.software, "rexafs");
    assert_eq!(header.software_version, env!("CARGO_PKG_VERSION"));
    assert_eq!(header.format_version, PROJECT_VERSION);
    assert!(chrono::DateTime::parse_from_rfc3339(&header.saved_utc).is_ok());
    let raw = header
        .files
        .iter()
        .find(|f| f.path.ends_with("cu_150k.xmu"))
        .unwrap();
    assert_eq!(raw.bytes, Some(20737));
    assert_eq!(
        raw.sha256.as_deref(),
        Some("c309e53ec6b681024718d5c25694c6426818725974afd7b124a2f076be618cf2")
    );
    assert!(raw.source_header.iter().any(|l| l.contains("Cu foil 150K")));
    assert!(raw.modified_unix_seconds.is_some());
}

#[test]
fn malformed_headers_payloads_and_unsafe_archive_paths_are_rejected() {
    let temp = Temp::new();
    let original = json_file(&fixture("rexafs-0.1.0-embedded.rxs"));
    let mut bad = vec![];
    for path in [
        "../escape.xmu",
        "/tmp/escape.xmu",
        "raw/../../escape.xmu",
        "raw\\..\\escape.xmu",
        "C:/escape.xmu",
    ] {
        let mut v = original.clone();
        v["header"]["files"][0]["archive_path"] = json!(path);
        bad.push(v);
    }
    let mut v = original.clone();
    v["header"]["files"][1]["archive_path"] = v["header"]["files"][0]["archive_path"].clone();
    bad.push(v);
    let mut v = original.clone();
    v["header"]["files"][0]["bytes"] = json!(1);
    bad.push(v);
    let mut v = original.clone();
    v["header"]["files"][0]["bytes"] = json!(u64::MAX);
    bad.push(v);
    let mut v = original.clone();
    v["header"]["files"].as_array_mut().unwrap().remove(0);
    bad.push(v);
    let mut v = original.clone();
    v["embedded"] = json!({});
    bad.push(v);
    let mut v = original.clone();
    v["embedded"]
        .as_object_mut()
        .unwrap()
        .values_mut()
        .for_each(|v| *v = json!("invalid!"));
    bad.push(v);
    let mut v = original.clone();
    v["header"]["storage"] = json!("paths");
    bad.push(v);
    let mut v = original.clone();
    v["header"]["path_base"] = json!("cwd");
    bad.push(v);
    let mut v = original.clone();
    v.as_object_mut().unwrap().remove("header");
    bad.push(v);
    let mut v = original.clone();
    v["header"]["format_version"] = json!(2);
    bad.push(v);
    for (i, v) in bad.into_iter().enumerate() {
        let path = temp.join(&format!("bad-{i}.rxs"));
        let bytes = serde_json::to_vec(&v).unwrap();
        std::fs::write(&path, &bytes).unwrap();
        assert!(load(&path).is_err(), "accepted malformed case {i}");
        assert_eq!(std::fs::read(path).unwrap(), bytes);
    }
    assert!(!temp.join("escape.xmu").exists());
}

#[test]
fn only_rxs_is_supported_and_future_formats_cannot_be_overwritten() {
    let temp = Temp::new();
    for name in ["session.rxs", "session.RXS"] {
        assert!(is_project(Path::new(name)));
    }
    for name in ["session.xtproj", "session.xproj", "session.json", "rxs"] {
        assert!(!is_project(Path::new(name)));
        assert!(save(&temp.join(name), &ProjectFile::default()).is_err());
        assert!(load(&temp.join(name)).is_err());
    }
    let path = temp.join("future.rxs");
    let bytes = std::fs::read(fixture("future-version.rxs")).unwrap();
    std::fs::write(&path, &bytes).unwrap();
    assert!(
        save(&path, &ProjectFile::default())
            .unwrap_err()
            .contains("newer")
    );
    assert_eq!(bytes, std::fs::read(path).unwrap());
    for invalid in [
        "{}",
        "[]",
        "{\"version\":0}",
        "{\"version\":-1}",
        "{\"version\":1.5}",
    ] {
        assert!(parse(invalid).is_err());
    }
}

#[test]
fn replacements_keep_exact_backup_and_partial_writes_leave_previous_file() {
    use std::io::Write;
    let temp = Temp::new();
    let path = temp.join("session.rxs");
    save(&path, &ProjectFile::default()).unwrap();
    let original = std::fs::read(&path).unwrap();
    let project = ProjectFile {
        derived: vec![DerivedSpectrum {
            label: "retained".into(),
            energy: vec![1.0],
            mu: vec![2.0],
            ..Default::default()
        }],
        ..Default::default()
    };
    save(&path, &project).unwrap();
    assert_eq!(
        std::fs::read(temp.join("session.rxs.bak")).unwrap(),
        original
    );
    let current = std::fs::read(&path).unwrap();
    let error = replace_with(&path, |file| {
        file.write_all(b"partial")?;
        Err(std::io::Error::other("injected failure"))
    });
    assert!(error.is_err());
    assert_eq!(std::fs::read(&path).unwrap(), current);
    assert_eq!(
        std::fs::read_dir(&temp.0).unwrap().count(),
        2,
        "temporary write leaked"
    );
}

#[test]
fn compact_writer_preserves_all_finite_double_bits_and_opaque_metadata() {
    let mut samples = vec![
        -0.0,
        f64::from_bits(1),
        f64::MIN_POSITIVE,
        1e-200,
        std::f64::consts::PI,
        f64::MAX,
    ];
    let mut seed = 7u64;
    for _ in 0..512 {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let f = f64::from_bits(seed);
        if f.is_finite() {
            samples.push(f);
        }
    }
    let project = ProjectFile {
        version: 1,
        derived: vec![DerivedSpectrum {
            label: "Cu μ\n  keep spacing ".into(),
            energy: samples.clone(),
            mu: samples.clone(),
            ..Default::default()
        }],
        extensions: [(
            "metadata".into(),
            json!({"null": null, "zero": -0.0, "empty": {}, "text": " ../a + b "}),
        )]
        .into(),
        ..Default::default()
    };
    let full = serde_json::to_value(&project).unwrap();
    let compact = compact::encode(full.clone()).unwrap();
    let reopened = parse(std::str::from_utf8(&compact).unwrap()).unwrap();
    assert_eq!(serde_json::to_value(&reopened).unwrap(), full);
    for values in [&reopened.derived[0].energy, &reopened.derived[0].mu] {
        assert_eq!(
            values.iter().map(|f| f.to_bits()).collect::<Vec<_>>(),
            samples.iter().map(|f| f.to_bits()).collect::<Vec<_>>()
        );
    }
    assert!(compact.len() < serde_json::to_vec(&full).unwrap().len());
}

#[test]
fn compact_defaults_preserve_explicit_fit_range_and_publication_settings() {
    for explicit in [false, true] {
        let mut project = load(&fixture("rexafs-0.1.0-links.rxs")).unwrap();
        project.fit_ranges.follow_transform = explicit;
        let value = serde_json::to_value(&project).unwrap();
        let bytes = compact::encode(value.clone()).unwrap();
        assert_eq!(
            serde_json::to_value(parse(std::str::from_utf8(&bytes).unwrap()).unwrap()).unwrap(),
            value
        );
        assert!(bytes.len() < serde_json::to_vec_pretty(&value).unwrap().len() * 3 / 4);
    }
}

#[test]
fn fixed_lambda_persists_while_missing_legacy_clamp_fields_keep_their_meaning() {
    use rexafs::prelude::AUTOBKClampScalePolicy;
    for json in [r#"{"version":1}"#, r#"{"version":1,"params":{}}"#] {
        assert_eq!(
            parse(json).unwrap().params.bkg_clamp_policy,
            AUTOBKClampScalePolicy::Fixed
        );
    }
    let modern = PipelineParams::default();
    assert_eq!(
        modern.bkg_clamp_policy,
        AUTOBKClampScalePolicy::FixedPenalty
    );
    assert_ne!(
        modern.fingerprint(),
        PipelineParams::legacy_defaults().fingerprint()
    );
    for lambda in [None, Some(0.0), Some(0.001), Some(0.1)] {
        let params = PipelineParams {
            bkg_clamp_lambda: lambda,
            ..modern.clone()
        };
        let original = ProjectFile {
            version: 1,
            params,
            ..Default::default()
        };
        let bytes = compact::encode(serde_json::to_value(&original).unwrap()).unwrap();
        let restored = parse(std::str::from_utf8(&bytes).unwrap()).unwrap();
        assert_eq!(restored.params.bkg_clamp_lambda, lambda);
        assert_eq!(
            restored.params.bkg_clamp_policy,
            AUTOBKClampScalePolicy::FixedPenalty
        );
        assert_eq!(restored.params.fingerprint(), original.params.fingerprint());
        assert_ne!(
            PipelineParams {
                bkg_clamp_lambda: Some(0.2),
                ..modern.clone()
            }
            .fingerprint(),
            original.params.fingerprint()
        );
    }
}

#[test]
fn independent_channels_roundtrip_linked_and_embedded_with_one_raw_source() {
    use crate::params::{DetectionMode, ImportConfig};
    for mode in [DataStorage::Paths, DataStorage::Embedded] {
        let temp = Temp::new();
        let source = temp.join("channels.dat");
        let raw = b"# energy i0 it ir\n100 100 50 10\n101 100 40 8\n102 100 30 6\n";
        std::fs::write(&source, raw).unwrap();
        let reference = PipelineParams {
            import: ImportConfig {
                mode: DetectionMode::Reference,
                ..Default::default()
            },
            rbkg: Some(1.3),
            ..Default::default()
        };
        let project = ProjectFile {
            version: 1,
            spectrum_file: Some(source.clone()),
            raw_files: vec![source.clone()],
            active_derived: Some(12),
            derived: vec![DerivedSpectrum {
                id: 12,
                label: "reference".into(),
                source: Some(source.clone()),
                params: Some(reference.clone()),
                ..Default::default()
            }],
            ..Default::default()
        };
        let path = temp.join("channels.rxs");
        save_with_storage(&path, &project, mode).unwrap();
        if mode == DataStorage::Embedded {
            std::fs::remove_file(source).unwrap();
        }
        let loaded = load(&path).unwrap();
        assert_eq!(loaded.active_derived, Some(12));
        assert_eq!(loaded.derived[0].id, 12);
        assert!(loaded.derived[0].params.as_ref() == Some(&reference));
        assert_eq!(
            std::fs::read(loaded.derived[0].source.as_ref().unwrap()).unwrap(),
            raw
        );
        assert_eq!(loaded.raw_files.len(), 1);
        assert_eq!(loaded.derived[0].source, loaded.spectrum_file);
    }
}

#[test]
fn invalid_channel_ids_are_rejected_and_legacy_groups_get_stable_ids() {
    let base = json!({"version":1,"derived":[{"label":"A","energy":[],"mu":[]},{"label":"B","energy":[],"mu":[]}]});
    let mut parsed = parse(&base.to_string()).unwrap();
    let mut repeat = parsed.clone();
    parsed.assign_group_ids();
    repeat.assign_group_ids();
    assert_eq!(parsed.derived[0].group_id, repeat.derived[0].group_id);
    assert_ne!(parsed.derived[0].group_id, parsed.derived[1].group_id);
    let mut duplicate = serde_json::to_value(&parsed).unwrap();
    duplicate["derived"][1]["group_id"] = duplicate["derived"][0]["group_id"].clone();
    assert!(parse(&duplicate.to_string()).is_err());
    assert_ne!(parsed.derived[0].id, parsed.derived[1].id);
    assert!(parsed.derived.iter().all(|d| d.id > 0));
    for id in [1, u64::MAX] {
        let mut bad = base.clone();
        bad["derived"][0]["id"] = id.into();
        bad["derived"][1]["id"] = id.into();
        assert!(parse(&bad.to_string()).is_err());
    }
}

#[test]
fn typed_outputs_difference_calibration_and_merge_roundtrip_linked_and_embedded() {
    use crate::params::{Operation, OperationInput, Quantity, process_file};
    use crate::publication::{SpectrumInput, figures};
    let temp = Temp::new();
    let mut project = specimen(&temp.join("original"));
    let source = project.spectrum_file.clone().unwrap();
    let params = PipelineParams {
        e0: Some(8979.0),
        bkg_ek0: Some(8979.0),
        ..Default::default()
    };
    let sp = process_file(&source, &params).unwrap();
    let difference =
        rexafs::xafs::tools::difference(&sp, &sp, rexafs::xafs::tools::DiffSpace::Norm).unwrap();
    let baseline = DerivedSpectrum {
        id: 1,
        label: "baseline".into(),
        energy: sp.energy.as_ref().unwrap().as_slice().to_vec(),
        mu: sp.mu.as_ref().unwrap().as_slice().to_vec(),
        params: Some(params.clone()),
        ..Default::default()
    };
    let input = OperationInput {
        group_id: None,
        label: "Cu".into(),
        path: source,
        derived_id: None,
        fingerprint: params.fingerprint(),
        size: Some(20737),
    };
    let diff = DerivedSpectrum {
        id: 2,
        label: "arbitrary renamed result".into(),
        energy: difference.energy.unwrap().as_slice().to_vec(),
        mu: difference.mu.unwrap().as_slice().to_vec(),
        quantity: Quantity::NormalizedDifference,
        params: Some(params.clone()),
        operation: Some(Operation {
            tool: "Difference spectrum".into(),
            parameters: json!({"space": "NormalizedMu"}),
            inputs: vec![
                input.clone(),
                OperationInput {
                    group_id: None,
                    label: "baseline".into(),
                    path: PathBuf::new(),
                    derived_id: Some(1),
                    fingerprint: baseline.fingerprint(&params),
                    size: None,
                },
            ],
            applied_energy_shift_ev: 0.0,
        }),
        ..Default::default()
    };
    let mut shifted = sp.clone();
    shifted.shift_energy(3.25);
    let calibrated = DerivedSpectrum {
        id: 3,
        label: "calibrated Cu".into(),
        energy: shifted.energy.unwrap().as_slice().to_vec(),
        mu: shifted.mu.unwrap().as_slice().to_vec(),
        params: Some(params.for_materialized(3.25)),
        operation: Some(Operation {
            tool: "Calibrate energy".into(),
            parameters: json!({"expected_energy_ev": 8982.25}),
            inputs: vec![input],
            applied_energy_shift_ev: 3.25,
        }),
        ..Default::default()
    };
    project.derived = vec![baseline, diff.clone(), calibrated.clone()];
    let mut merged = calibrated.clone();
    merged.id = 4;
    merged.label = "calibrated Cu · merge 2".into();
    merged.operation = Some(Operation {
        tool: "merge".into(),
        parameters: json!({"template": "calibrated Cu", "count": 2}),
        inputs: [2, 0]
            .map(|i| OperationInput {
                group_id: None,
                label: project.derived[i].label.clone(),
                path: PathBuf::new(),
                derived_id: Some(project.derived[i].id),
                fingerprint: project.derived[i].fingerprint(&params),
                size: None,
            })
            .to_vec(),
        applied_energy_shift_ev: 0.0,
    });
    project.derived.push(merged.clone());
    project.active_derived = Some(2);
    for mode in [DataStorage::Paths, DataStorage::Embedded] {
        let saved = temp.join(&format!("typed-{mode:?}.rxs"));
        save_with_storage(&saved, &project, mode).unwrap();
        let loaded = load(&saved).unwrap();
        assert_eq!(state(&project), state(&loaded));
        let mean = &loaded.derived[3];
        assert_eq!(mean.quantity, Quantity::RawMu);
        assert_eq!(mean.operation, merged.operation);
        assert_eq!(mean.params.as_ref().unwrap().e0, Some(8982.25));
        assert_eq!(
            mean.raw(mean.params.as_ref().unwrap()).unwrap(),
            (merged.energy.clone(), merged.mu.clone())
        );
        let result = &loaded.derived[1];
        assert_eq!(result.quantity, Quantity::NormalizedDifference);
        assert!(!result.quantity_unconfirmed);
        assert!(result.display_label().contains("Δμnorm"));
        assert!(
            result
                .process(&params)
                .unwrap_err()
                .contains("normalization/AUTOBK disabled")
        );
        let display = result.for_display(&params).unwrap();
        assert_eq!(display.mu.as_ref().unwrap().as_slice(), diff.mu);
        assert!(display.normalization.is_none() && display.background.is_none());
        let input = SpectrumInput {
            group: Some(result.clone()),
            params: params.clone(),
            data: Some(std::sync::Arc::new(display)),
            ..Default::default()
        };
        assert!(
            input.process().is_err(),
            "cached display data cannot enter fitting"
        );
        let plots = figures::quantity_figures(
            input.for_display().unwrap(),
            &result.display_label(),
            Some(result.quantity),
        );
        assert_eq!(plots.len(), 1);
        assert_eq!(plots[0].series[0].x, diff.energy);
        assert_eq!(plots[0].series[0].y, diff.mu);
        let csv = plots[0].csv(&Default::default()).unwrap();
        assert!(
            csv.lines()
                .next()
                .unwrap()
                .contains("Δμnorm (dimensionless)")
        );
        assert_eq!(csv.lines().count(), diff.energy.len() + 1);
        let result = &loaded.derived[2];
        let settings = result.params.as_ref().unwrap();
        assert_eq!(settings.e0, Some(8982.25));
        assert_eq!(settings.bkg_ek0, Some(8982.25));
        assert_eq!(
            result.operation.as_ref().unwrap().applied_energy_shift_ev,
            3.25
        );
        for _ in 0..2 {
            let processed = result.process(settings).unwrap();
            assert_eq!(processed.energy.unwrap().as_slice(), calibrated.energy);
            assert_eq!(processed.e0, settings.e0);
        }
        // A second save/reopen must preserve provenance and never add ΔE again.
        save(&saved, &loaded).unwrap();
        assert_eq!(state(&loaded), state(&load(&saved).unwrap()));
    }
}

#[test]
fn typed_outputs_legacy_quantity_hints_require_explicit_confirmation() {
    use crate::params::Quantity;
    for invalid in [json!(5), json!("invalid"), json!(null)] {
        assert!(parse(&json!({"version": 1, "derived": [invalid]}).to_string()).is_err());
    }
    for (label, expected) in [
        ("unknown", Quantity::RawMu),
        ("diff: A − B", Quantity::NormalizedDifference),
        ("A − B · Δμnorm", Quantity::NormalizedDifference),
    ] {
        let value = json!({"version": 1, "derived": [{"label": label, "energy": [1., 2.], "mu": [0., 0.]}]});
        let mut project = parse(&value.to_string()).unwrap();
        let group = &mut project.derived[0];
        assert_eq!(group.quantity, expected);
        assert!(group.quantity_unconfirmed);
        assert!(
            group
                .process(&PipelineParams::default())
                .unwrap_err()
                .contains("Quantity unconfirmed")
        );
        group.label = "renamed again".into();
        let encoded = compact::encode(serde_json::to_value(&project).unwrap()).unwrap();
        let mut reopened = parse(std::str::from_utf8(&encoded).unwrap()).unwrap();
        assert_eq!(reopened.derived[0].quantity, expected);
        assert!(reopened.derived[0].quantity_unconfirmed);
        reopened.derived[0].confirm_quantity(Quantity::NormalizedDifference);
        let encoded = compact::encode(serde_json::to_value(&reopened).unwrap()).unwrap();
        let confirmed = parse(std::str::from_utf8(&encoded).unwrap()).unwrap();
        assert!(!confirmed.derived[0].quantity_unconfirmed);
        assert_eq!(
            confirmed.derived[0].quantity,
            Quantity::NormalizedDifference
        );
    }
    let channel = parse(r#"{"version":1,"derived":[{"label":"diff: editable name","source":"scan.dat","energy":[],"mu":[]}]}"#).unwrap();
    assert_eq!(channel.derived[0].quantity, Quantity::RawMu);
    assert!(!channel.derived[0].quantity_unconfirmed);
}

#[test]
fn group_identity_roundtrip_relocation_channels_results_and_three_more_imports() {
    use crate::app::DERIVED_BASE;
    use crate::group_identity::{GroupId, GroupRegistry};
    use crate::params::{DetectionMode, Operation, OperationInput};
    use std::collections::{BTreeMap, BTreeSet};

    let temp = Temp::new();
    for mode in [DataStorage::Paths, DataStorage::Embedded] {
        let old = temp.join(&format!("old-{mode:?}"));
        let mut project = specimen(&old);
        let old = storage::resolved_location(&old);
        let source = storage::resolved_location(project.spectrum_file.as_ref().unwrap());
        project.derived = vec![
            DerivedSpectrum {
                id: 1,
                source: Some(source.clone()),
                params: Some(PipelineParams {
                    import: crate::params::ImportConfig {
                        mode: DetectionMode::Reference,
                        ..Default::default()
                    },
                    e0: Some(8000.),
                    ..Default::default()
                }),
                ..Default::default()
            },
            DerivedSpectrum {
                id: 2,
                group_id: Some(GroupId::new_result()),
                label: "tool result".into(),
                energy: vec![1., 2.],
                mu: vec![3., 4.],
                ..Default::default()
            },
        ];
        project.assign_group_ids();
        let mut files = project.raw_files.clone();
        files.sort();
        files.dedup();
        let build = |project: &mut ProjectFile, files: &[PathBuf]| {
            GroupRegistry::rebuild(
                files
                    .iter()
                    .cloned()
                    .enumerate()
                    .map(|(ix, path)| (ix, path, DetectionMode::Auto)),
                &mut project.derived,
                &mut project.source_groups,
                &project.source_origins,
            )
        };
        let registry = build(&mut project, &files);
        let source_ix = files.iter().position(|p| *p == source).unwrap();
        let marked = BTreeSet::from([source_ix, DERIVED_BASE, DERIVED_BASE + 1]);
        let frozen = BTreeSet::from([source_ix, DERIVED_BASE]);
        let custom = PipelineParams {
            e0: Some(9001.),
            ..Default::default()
        };
        project.group_state.capture(
            &registry,
            &marked,
            &frozen,
            &BTreeMap::from([(source_ix, custom.clone())]),
            Some(DERIVED_BASE),
        );
        for ix in &marked {
            let id = registry.id(*ix).unwrap();
            project
                .group_state
                .labels
                .insert(id.clone(), format!("Renamed {ix}"));
            project.group_state.colors.insert(id, (*ix % 8) as u8);
        }
        let inputs: Vec<_> = marked
            .iter()
            .filter(|&&ix| ix != DERIVED_BASE + 1)
            .map(|&ix| OperationInput {
                group_id: registry.id(ix),
                label: format!("input {ix}"),
                path: if ix == source_ix {
                    source.clone()
                } else {
                    PathBuf::new()
                },
                derived_id: ix.checked_sub(DERIVED_BASE).map(|i| project.derived[i].id),
                fingerprint: 123,
                size: None,
            })
            .collect();
        project.derived[1].operation = Some(Operation {
            tool: "test".into(),
            parameters: json!({}),
            inputs: inputs.clone(),
            applied_energy_shift_ev: 0.,
        });
        let identities = serde_json::to_value(&project.group_state).unwrap();
        save_with_storage(&old.join("durable.rxs"), &project, mode).unwrap();
        let moved = temp.join(&format!("moved-{mode:?}"));
        std::fs::rename(&old, &moved).unwrap();
        let mut reopened = load(&moved.join("durable.rxs")).unwrap();
        reopened.assign_group_ids();
        assert_eq!(
            serde_json::to_value(&reopened.group_state).unwrap(),
            identities
        );
        assert_eq!(reopened.derived[0].params.as_ref().unwrap().e0, Some(8000.));
        assert_eq!(
            reopened.derived[1]
                .operation
                .as_ref()
                .unwrap()
                .inputs
                .iter()
                .map(|i| &i.group_id)
                .collect::<Vec<_>>(),
            inputs.iter().map(|i| &i.group_id).collect::<Vec<_>>()
        );
        // The real catalog can reopen in a different order, then append files.
        let mut files: Vec<_> = files
            .iter()
            .map(|path| {
                let tail = path.strip_prefix(&old).unwrap();
                let moved_source = storage::resolved_location(&moved.join(tail));
                reopened
                    .source_origins
                    .iter()
                    .find(|(_, original)| **original == moved_source)
                    .map(|(cache, _)| cache.clone())
                    .unwrap_or(moved_source)
            })
            .collect();
        files.reverse();
        let before = build(&mut reopened, &files);
        for i in 0..3 {
            let path = moved.join(format!("added-{i}.xmu"));
            std::fs::copy(fixture("data/cu_150k.xmu"), &path).unwrap();
            files.push(path);
        }
        let after = build(&mut reopened, &files);
        assert!(!before.indices_changed(&after));
        assert_eq!(
            before.indices(&reopened.group_state.marked),
            after.indices(&reopened.group_state.marked)
        );
        assert_eq!(
            after.indices(&reopened.group_state.marked).len(),
            3,
            "{mode:?}"
        );
        assert_eq!(after.indices(&reopened.group_state.frozen).len(), 2);
        assert!(
            reopened
                .group_state
                .resolved_overrides(&after)
                .values()
                .any(|p| *p == custom)
        );
        for input in &reopened.derived[1].operation.as_ref().unwrap().inputs {
            assert!(after.index(input.group_id.as_ref().unwrap()).is_some());
        }
        reopened.raw_files = files;
        let again = moved.join("again.rxs");
        save_with_storage(&again, &reopened, mode).unwrap();
        assert_eq!(state(&reopened), state(&load(&again).unwrap()));
    }
}

#[test]
fn standalone_processing_lock_roundtrip_linked_and_embedded() {
    use crate::group_identity::GroupRegistry;
    use std::collections::BTreeMap;
    let temp = Temp::new();
    for mode in [DataStorage::Paths, DataStorage::Embedded] {
        let dir = temp.join(&format!("standalone-lock-{mode:?}"));
        let mut project = specimen(&dir);
        let registry = GroupRegistry::default();
        let source = storage::resolved_location(project.spectrum_file.as_ref().unwrap());
        let id =
            registry.register_source(None, source, project.params.import.mode, &BTreeMap::new());
        project
            .group_state
            .capture_standalone_lock(&registry, &id, true);
        let path = dir.join("locked.rxs");
        save_with_storage(&path, &project, mode).unwrap();
        let mut reopened = load(&path).unwrap();
        assert!(reopened.group_state.frozen.contains(&id));
        reopened
            .group_state
            .capture_standalone_lock(&registry, &id, false);
        save_with_storage(&path, &reopened, mode).unwrap();
        assert!(!load(&path).unwrap().group_state.frozen.contains(&id));
    }
}

#[test]
fn removal_exclusions_roundtrip_linked_and_embedded_without_resurrection() {
    use crate::group_identity::{GroupId, GroupRegistry};
    use crate::params::{DetectionMode, Operation, OperationInput};
    use std::collections::BTreeSet;
    for mode in [DataStorage::Paths, DataStorage::Embedded] {
        let temp = Temp::new();
        let source = temp.join("input.dat");
        std::fs::write(&source, "1 2\n2 3\n").unwrap();
        let mut project = ProjectFile {
            version: 1,
            raw_files: vec![source.clone()],
            spectrum_file: Some(source.clone()),
            ..Default::default()
        };
        project.assign_group_ids();
        let id = project.source_groups[0].id.clone();
        project.group_state.excluded.insert(id.clone());
        let retired_channel = GroupId::source(&source, DetectionMode::Reference);
        project.group_state.excluded.insert(retired_channel.clone());
        project.derived.push(DerivedSpectrum {
            group_id: Some(GroupId::new_result()),
            id: 1,
            operation: Some(Operation {
                tool: "merge".into(),
                inputs: vec![OperationInput {
                    group_id: Some(id.clone()),
                    label: "Input".into(),
                    path: source.clone(),
                    derived_id: None,
                    fingerprint: 0,
                    size: None,
                }],
                parameters: Value::Null,
                applied_energy_shift_ev: 0.,
            }),
            ..Default::default()
        });
        let path = temp.join("excluded.rxs");
        save_with_storage(&path, &project, mode).unwrap();
        let mut loaded = load(&path).unwrap();
        loaded.assign_group_ids();
        assert_eq!(
            loaded.group_state.excluded,
            BTreeSet::from([id.clone(), retired_channel.clone()])
        );
        let registry = GroupRegistry::from_sources(loaded.source_groups.clone());
        registry.set_excluded(&loaded.group_state.excluded);
        let location = loaded.source_groups[0].path.clone();
        registry.register_source(
            Some(0),
            location,
            DetectionMode::Auto,
            &loaded.source_origins,
        );
        assert_eq!(registry.id(0), None);
        assert_eq!(registry.index(&id), None);
        let mut replacement = DerivedSpectrum {
            source: Some(source.clone()),
            params: Some(PipelineParams::default()),
            ..Default::default()
        };
        replacement.params.as_mut().unwrap().import.mode = DetectionMode::Reference;
        registry.assign_group(&mut replacement, &Default::default());
        assert_ne!(replacement.group_id, Some(retired_channel));
        assert_eq!(
            loaded.derived[0].operation.as_ref().unwrap().inputs[0].group_id,
            Some(id.clone())
        );
        let location = registry.sources()[0].path.clone();
        let fresh = registry.reimport_source(&location, Some(0)).unwrap();
        loaded.source_groups = registry.sources();
        save_with_storage(&path, &loaded, mode).unwrap();
        let mut reimported = load(&path).unwrap();
        reimported.assign_group_ids();
        let reopened = GroupRegistry::from_sources(reimported.source_groups.clone());
        reopened.set_excluded(&reimported.group_state.excluded);
        reopened.register_source(
            Some(0),
            reimported.source_groups[0].path.clone(),
            DetectionMode::Auto,
            &reimported.source_origins,
        );
        assert_eq!(reopened.id(0), Some(fresh));
        assert!(reopened.is_excluded(&id));
        assert_eq!(
            reimported.derived[0].operation.as_ref().unwrap().inputs[0].group_id,
            Some(id)
        );
        assert!(source.is_file());
    }
}

#[test]
fn unsaved_changes_follow_content_not_header_or_empty_sessions() {
    // A fresh window has nothing to save, even with no saved fingerprint.
    assert!(!ProjectFile::default().has_unsaved_changes(None));

    let temp = tempfile::tempdir().unwrap();
    let project = super::copper_example_in(temp.path()).unwrap();
    // Imported work that was never saved must be offered for saving.
    assert!(project.has_unsaved_changes(None));

    let saved = project.content_fingerprint();
    assert!(saved.is_some());
    assert_eq!(project.clone().content_fingerprint(), saved);
    assert!(!project.has_unsaved_changes(saved));

    // Saving rewrites the header; that alone is not an edit.
    let mut reheadered = project.clone();
    reheadered.header = None;
    assert!(!reheadered.has_unsaved_changes(saved));

    // Reading the project's spectra after opening refills the parser
    // diagnostics cache; that is not an edit (#151 follow-up).
    let mut reread = project.clone();
    let (id, group) = project
        .source_groups
        .first()
        .map(|g| (g.id.clone(), g.path.clone()))
        .unwrap();
    reread.parser_evidence.insert(
        id,
        crate::source_evidence::ParserRecord {
            path: group,
            channel: Default::default(),
            mapping_revision: 1,
            diagnostics: Default::default(),
            declared_edge: None,
        },
    );
    assert!(!reread.has_unsaved_changes(saved));

    let mut edited = project.clone();
    edited.params.energy_offset_ev += 1.0;
    assert!(edited.has_unsaved_changes(saved));

    // Emptying a saved project is still a change.
    assert!(ProjectFile::default().has_unsaved_changes(saved));
}
