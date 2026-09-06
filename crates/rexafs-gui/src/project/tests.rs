use super::*;
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
fn state(project: &ProjectFile) -> Value {
    let mut project = project.clone();
    let origins = project.source_origins.clone();
    storage::map_paths(&mut project, &mut |p| {
        Ok(origins.get(p).cloned().unwrap_or_else(|| p.to_owned()))
    })
    .unwrap();
    let mut value = serde_json::to_value(project).unwrap();
    for key in ["header", "embedded"] {
        value.as_object_mut().unwrap().remove(key);
    }
    value
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
