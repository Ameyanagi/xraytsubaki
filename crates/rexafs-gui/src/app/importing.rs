//! Append-only import. Read bounded headers off the UI thread; retain lazy sources.
use super::*;
use crate::catalog::FileMeta;
use crate::params::ImportConfig;
use futures::{SinkExt, channel::mpsc};

struct ImportFile {
    meta: FileMeta,
    reference: bool,
}
enum ImportEvent {
    Batch(Vec<ImportFile>),
    Error(String),
    Done,
}

fn start_import(
    paths: Vec<PathBuf>,
    import: ImportConfig,
    detect_channels: bool,
) -> mpsc::Receiver<ImportEvent> {
    let (mut tx, rx) = mpsc::channel(2);
    std::thread::spawn(move || {
        let send = |tx: &mut mpsc::Sender<ImportEvent>, event| {
            futures::executor::block_on(tx.send(event)).is_ok()
        };
        let mut batch = Vec::new();
        for path in paths {
            let root = match path.canonicalize() {
                Ok(root) => root,
                Err(e) => {
                    if !send(
                        &mut tx,
                        ImportEvent::Error(format!("{}: {e}", path.display())),
                    ) {
                        return;
                    }
                    continue;
                }
            };
            let explicit_file = root.is_file();
            for entry in walkdir::WalkDir::new(&root)
                .follow_links(false)
                .sort_by_file_name()
            {
                let entry = match entry {
                    Ok(entry) => entry,
                    Err(e) => {
                        if !send(&mut tx, ImportEvent::Error(e.to_string())) {
                            return;
                        }
                        continue;
                    }
                };
                if !entry.file_type().is_file()
                    || (!explicit_file
                        && !entry.path().extension().is_some_and(|ext| {
                            crate::catalog::SPECTRUM_EXTENSIONS
                                .iter()
                                .any(|e| ext.eq_ignore_ascii_case(e))
                        }))
                {
                    continue;
                }
                let reference = if detect_channels {
                    match preview_import(entry.path(), &import) {
                        Ok(preview) => {
                            preview.resolved.mode != DetectionMode::Reference
                                && preview
                                    .available_channels()
                                    .contains(&DetectionMode::Reference)
                        }
                        Err(e) => {
                            if !send(&mut tx, ImportEvent::Error(e)) {
                                return;
                            }
                            // Keep the source visible so manual column assignment can repair it.
                            false
                        }
                    }
                } else {
                    false
                };
                let meta = FileMeta {
                    dir: Arc::from(entry.path().parent().unwrap().to_string_lossy().as_ref()),
                    name: entry
                        .file_name()
                        .to_string_lossy()
                        .into_owned()
                        .into_boxed_str(),
                    size: entry.metadata().map(|m| m.len()).unwrap_or(0),
                };
                batch.push(ImportFile { meta, reference });
                if batch.len() == 128
                    && !send(&mut tx, ImportEvent::Batch(std::mem::take(&mut batch)))
                {
                    return;
                }
                if tx.is_closed() {
                    return;
                }
            }
        }
        if !batch.is_empty() && !send(&mut tx, ImportEvent::Batch(batch)) {
            return;
        }
        send(&mut tx, ImportEvent::Done);
    });
    rx
}

impl StudioApp {
    pub(super) fn set_chi_standard(
        &mut self,
        standard: Option<crate::params::ChiStandard>,
        cx: &mut Context<Self>,
    ) {
        let target = self.override_target();
        if target.is_some_and(|ix| self.frozen.contains(&ix)) {
            self.status = "This group is frozen — thaw it to edit its parameters.".into();
            cx.notify();
            return;
        }
        let before = self.ui_params().clone();
        self.edit_params().bkg_standard = standard;
        self.record_param_edit(
            target,
            None,
            before,
            self.ui_params().clone(),
            "Set AUTOBK standard χ(k)".into(),
        );
        self.schedule_recompute(cx);
        cx.notify();
    }

    pub(super) fn choose_chi_standard(&mut self, cx: &mut Context<Self>) {
        let generation = self.project_generation;
        let path = self.current_path.clone();
        let group = self.active_group_id();
        let fingerprint = self.ui_params().fingerprint();
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: None,
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await && let Some(file) = paths.into_iter().next() {
                let result = cx.background_executor().spawn(async move {
                    crate::params::ChiStandard::load(&file)
                }).await;
                this.update(cx, |app, cx| {
                    if app.project_generation != generation || app.current_path != path
                        || app.active_group_id() != group || app.ui_params().fingerprint() != fingerprint {
                        app.status = "Standard not applied because the active group or its settings changed. Load it again for the intended group.".into();
                        cx.notify();
                        return;
                    }
                    match result {
                        Ok(standard) => app.set_chi_standard(Some(standard), cx),
                        Err(e) => { app.status = format!("Standard χ(k): {e}").into(); cx.notify(); }
                    }
                }).ok();
            }
        }).detach();
    }

    pub(super) fn append_import(
        &mut self,
        mut paths: Vec<PathBuf>,
        restore: bool,
        cx: &mut Context<Self>,
    ) {
        if self.catalog.scanning {
            self.status = "Wait for the current import to finish before adding more files.".into();
            cx.notify();
            return;
        }
        if !restore && self.selected.is_none() && !self.current_path.as_os_str().is_empty() {
            paths.insert(0, self.current_path.clone());
            self.pending_project_spectrum = Some(self.current_path.clone());
        }
        self.source_dir = None;
        self.catalog_index_path = None;
        self.catalog.scanning = true;
        let generation = self.catalog_gen;
        let params = self.params.clone();
        let existing_references: BTreeSet<_> = self
            .derived
            .iter()
            .filter(|d| {
                d.params
                    .as_ref()
                    .is_some_and(|p| p.import.mode == DetectionMode::Reference)
            })
            .filter_map(|d| d.source.clone())
            .collect();
        let mut rx = start_import(paths, params.import.clone(), !restore);
        self.status = "Importing files and detecting reference channels…".into();
        cx.spawn(async move |this, cx| {
            let (mut added, mut channels, mut notices) = (0, 0, 0);
            while let Some(event) = rx.next().await {
                let done = matches!(event, ImportEvent::Done);
                let current = this.update(cx, |app, cx| {
                    if app.catalog_gen != generation { return false; }
                    match event {
                        ImportEvent::Batch(batch) => {
                            for file in batch {
                                let path = PathBuf::from(file.meta.dir.as_ref()).join(file.meta.name.as_ref());
                                if app.catalog.find_by_canonical_path(&path).is_some() { continue; }
                                app.catalog.extend(vec![file.meta]);
                                added += 1;
                                if file.reference && !existing_references.contains(&path) {
                                    let mut reference_params = params.clone();
                                    reference_params.import.mode = DetectionMode::Reference;
                                    // A reference edge must resolve independently of sample overrides.
                                    reference_params.e0 = None;
                                    reference_params.edge_step = None;
                                    reference_params.bkg_ek0 = None;
                                    reference_params.bkg_standard = None;
                                    let group = DerivedSpectrum {
                                        id: app.next_group_id(),
                                        label: path.file_name().unwrap_or_default().to_string_lossy().into_owned(),
                                        source: Some(path), params: Some(reference_params), ..Default::default()
                                    };
                                    app.derived.push(group);
                                    channels += 1;
                                }
                            }
                            if app.selected.is_none() && app.pending_project_spectrum.is_none() && app.pending_derived.is_none() && !app.catalog.is_empty() { app.select_entry(0, cx); }
                            app.status = format!("Importing · {added} files · {channels} reference channels").into();
                        }
                        ImportEvent::Error(e) => { notices += 1; app.record_job_error("import", e); }
                        ImportEvent::Done => {
                            app.catalog.scanning = false;
                            app.resolve_pending_overrides(cx);
                            app.restore_project_selection(cx);
                            if !app.filter_text.is_empty() { app.apply_filter(cx); }
                            app.status = format!("Imported {added} files + {channels} reference channels · {notices} notices").into();
                            app.record(app.status.to_string(), None);
                        }
                    }
                    cx.notify();
                    true
                }).unwrap_or(false);
                if !current || done { break; }
            }
        }).detach();
        cx.notify();
    }

    pub(super) fn add_import_channel(&mut self, mode: DetectionMode, cx: &mut Context<Self>) {
        let path = self.current_path.clone();
        if path.as_os_str().is_empty() {
            return;
        }
        let mut params = self.ui_params().clone();
        params.import.mode = mode;
        params.e0 = None;
        params.edge_step = None;
        params.bkg_ek0 = None;
        params.bkg_standard = None;
        if let Some(i) = self.derived.iter().position(|d| {
            d.source.as_ref() == Some(&path)
                && d.params.as_ref().is_some_and(|p| p.import == params.import)
        }) {
            self.select_entry(DERIVED_BASE + i, cx);
            return;
        }
        let group = DerivedSpectrum {
            id: self.next_group_id(),
            label: path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            source: Some(path),
            params: Some(params),
            ..Default::default()
        };
        let index = self.derived.len();
        self.record(
            format!("Add {} channel", mode.label()),
            Some(shell::journal::UndoOp::DerivedAdd {
                index,
                spectrum: group.clone(),
            }),
        );
        self.derived.push(group);
        self.select_entry(DERIVED_BASE + index, cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn qas_import_detects_reference_but_project_restore_preserves_saved_groups() {
        let source =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../rexafs/tests/testfiles/Ru_QAS.dat");
        for detect in [true, false] {
            let events = futures::executor::block_on(
                start_import(vec![source.clone()], ImportConfig::default(), detect)
                    .collect::<Vec<_>>(),
            );
            let files: Vec<_> = events
                .iter()
                .filter_map(|e| {
                    if let ImportEvent::Batch(files) = e {
                        Some(files.as_slice())
                    } else {
                        None
                    }
                })
                .flatten()
                .collect();
            assert_eq!(files.len(), 1);
            assert_eq!(files[0].reference, detect);
            assert!(matches!(events.last(), Some(ImportEvent::Done)));
            assert!(!events.iter().any(|e| matches!(e, ImportEvent::Error(_))));
        }
    }
}
