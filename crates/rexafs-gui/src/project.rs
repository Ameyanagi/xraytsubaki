//! Project persistence (.rxs): JSON capturing the data source, pipeline
//! parameters, and fit model so a session can be reopened. Catalog contents
//! restore from the per-user index cache (see `catalog::index_cache_path`)
//! with a background freshness re-walk; processed data is recomputed
//! through the fingerprint cache.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::fitting::{FitHistoryEntry, FitPathSpec, FitRanges, FitVarSpec};
use crate::params::DerivedSpectrum;
use crate::params::PipelineParams;
pub mod assistant;
mod compact;
mod storage;
pub use storage::{DataStorage, ProjectHeader};

/// Stable input identity for a retained collection analysis. Result arrays own the
/// prepared input, so historical calculations survive subsequent preprocessing.
#[derive(Clone, Serialize, Deserialize)]
pub struct AnalysisInput {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub corrections: Vec<crate::fluorescence_history::CorrectionReceipt>,
    pub group_id: Option<crate::group_identity::GroupId>,
    pub label: String,
    pub fingerprint: u64,
}

/// Retained native MCR result, with its exact settings and ordered input identities.
#[derive(Clone, Serialize, Deserialize)]
pub struct McrAnalysis {
    pub result: rexafs::prelude::McrResult,
    pub inputs: Vec<AnalysisInput>,
    #[serde(default)]
    pub comparison: Option<McrReferenceComparison>,
}

/// Independently selected references on the retained MCR energy grid.
#[derive(Clone, Serialize, Deserialize)]
pub struct McrReferenceComparison {
    pub spectra: nalgebra::DMatrix<f64>,
    /// Reference-row order mapped to recovered component rows; no rescaling.
    pub component_indices: Vec<usize>,
    /// Relative Euclidean spectral errors after permutation matching.
    pub relative_errors: Vec<f64>,
    pub inputs: Vec<AnalysisInput>,
}

/// Owned PCA model and target reconstruction retained for reproducible figures.
#[derive(Clone, Serialize, Deserialize)]
pub struct PcaAnalysis {
    pub model: rexafs::prelude::PcaModel,
    pub target: Option<rexafs::prelude::PcaFit>,
    /// Target identity first, followed by training-row identities. A marked
    /// target occurs twice because it participates in both roles.
    pub inputs: Vec<AnalysisInput>,
}

/// Owned LCF result and the identities of the target and standards.
#[derive(Clone, Serialize, Deserialize)]
pub struct LcfAnalysis {
    #[serde(default)]
    pub config: Option<rexafs::prelude::LcfConfig>,
    pub result: rexafs::prelude::LcfResult,
    pub inputs: Vec<AnalysisInput>,
}

/// Historical LCF coefficient series. Frame keys are zero-based acquisition
/// indices; each row contains weights in `standards` order and an R-factor last.
/// These retained results are not recomputed by opening the project.
#[derive(Clone, Serialize, Deserialize)]
pub struct LcfSeriesAnalysis {
    pub config: rexafs::prelude::LcfConfig,
    pub inputs: std::collections::BTreeMap<usize, AnalysisInput>,
    pub standards: Vec<AnalysisInput>,
    pub rows: std::collections::BTreeMap<usize, Vec<f64>>,
    pub errors: std::collections::BTreeMap<usize, String>,
    pub complete: bool,
    pub cancelled: bool,
}

/// One spectrum's parameter override. Catalog indices are only stable
/// within a single scan session, so persistence keys overrides by the
/// file's full path (dir + name) and re-resolves them to indices when the
/// catalog is rebuilt on project load.
#[derive(Clone, Serialize, Deserialize)]
pub struct ParamOverride {
    pub path: PathBuf,
    pub params: PipelineParams,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectFile {
    pub normalizations: crate::normalization_history::NormalizationHistory,
    pub wavelets: crate::wavelet_history::WaveletArchive,
    pub peak_fits: crate::peak_fits::PeakArchive,
    pub series_measurements: crate::series_measurements::SeriesArchive,
    pub lcf_series_analysis: Option<LcfSeriesAnalysis>,
    pub pca_analysis: Option<PcaAnalysis>,
    pub lcf_analysis: Option<LcfAnalysis>,
    pub mcr_analysis: Option<McrAnalysis>,
    pub parser_evidence: std::collections::BTreeMap<
        crate::group_identity::GroupId,
        crate::source_evidence::ParserRecord,
    >,
    pub imports: crate::import_recipes::ProjectImports,
    pub import_history: Vec<crate::app::import_state::IntakeBatch>,
    pub source_groups: Vec<crate::group_identity::SourceGroup>,
    pub group_state: crate::group_identity::GroupState,
    pub header: Option<ProjectHeader>,
    pub version: u32,
    /// Root folder of the catalog (re-scanned on open).
    pub source_dir: Option<PathBuf>,
    /// Standalone spectra also need a source when there is no catalog.
    pub spectrum_file: Option<PathBuf>,
    #[serde(default = "PipelineParams::legacy_defaults")]
    pub params: PipelineParams,
    /// Per-spectrum parameter overrides.
    pub overrides: Vec<ParamOverride>,
    pub fit_paths: Vec<FitPathSpec>,
    pub fit_vars: Vec<FitVarSpec>,
    pub fit_ranges: FitRanges,
    pub fit_mode: crate::rmc_fitting::FitMode,
    pub rmc: crate::rmc_fitting::Project,
    pub feff_workspace: Option<PathBuf>,
    pub derived: Vec<DerivedSpectrum>,
    pub active_derived: Option<u64>,
    /// Completed fits (model snapshot + statistics), oldest first.
    pub fit_history: Vec<FitHistoryEntry>,
    /// Full result arrays of history fits, keyed by [`FitHistoryEntry::id`]:
    /// measured and modeled χ(k), the k window, the R-space transforms and the
    /// path contributions. Reopening restores them so Publish can export the
    /// fit curves without refitting. Absent in projects saved by older
    /// releases; those fits keep their statistics and model, and their curves
    /// return only after a refit. Results with non-finite numbers, which JSON
    /// cannot represent, are dropped by [`save_with_storage`].
    /// History id of the fit that was current and up to date when the project
    /// was saved. Reopening shows that result again (Fit results and Publish
    /// figures) and marks it as needing an update only if the reloaded
    /// spectrum or model differs from the one it was fitted to.
    pub current_fit: Option<usize>,
    pub fit_results:
        std::collections::BTreeMap<usize, std::sync::Arc<rexafs::prelude::FeffFitResult>>,
    pub joint: crate::joint_fitting::JointConfig,
    pub(crate) publication: crate::publication::figures::FigureSettings,
    pub assistant: assistant::AssistantHistory,
    /// Preserve additive top-level metadata during open/edit/save.
    #[serde(flatten)]
    pub extensions: std::collections::BTreeMap<String, serde_json::Value>,
    /// Losslessly compressed raw file payloads, deduplicated by SHA-256.
    pub embedded: std::collections::BTreeMap<String, String>,
    /// Imported catalog files and load origin are runtime identities; their
    /// portable references and metadata are recorded once in the header.
    #[serde(skip)]
    pub raw_files: Vec<PathBuf>,
    #[serde(skip)]
    pub origin: Option<PathBuf>,
    #[serde(skip)]
    pub source_origins: std::collections::BTreeMap<PathBuf, PathBuf>,
    #[serde(skip)]
    pub data_storage: DataStorage,
}

impl ProjectFile {
    /// Assign identities only when a legacy project enters an editable session.
    /// Decoding/validating archives itself remains a lossless operation.
    pub fn assign_group_ids(&mut self) -> u64 {
        // The scanner uses canonical paths. Resolve aliases and missing tails
        // before matching saved locators; this transform cannot return an error.
        let _ = storage::map_paths(self, &mut |p| Ok(storage::resolved_location(p)));
        self.source_origins = std::mem::take(&mut self.source_origins)
            .into_iter()
            .map(|(path, origin)| {
                (
                    storage::resolved_location(&path),
                    storage::resolved_location(&origin),
                )
            })
            .collect();
        let mut next = self.derived.iter().map(|d| d.id).max().unwrap_or(0) + 1;
        for group in &mut self.derived {
            if group.id == 0 {
                group.id = next;
                next += 1;
            }
        }
        let mut files: std::collections::BTreeSet<_> =
            self.source_groups.iter().map(|s| s.path.clone()).collect();
        files.extend(self.spectrum_file.clone());
        files.extend(self.overrides.iter().map(|p| p.path.clone()));
        files.extend(
            self.derived
                .iter()
                .filter_map(|g| g.operation.as_ref())
                .flat_map(|op| &op.inputs)
                .filter(|input| input.derived_id.is_none() && !input.path.as_os_str().is_empty())
                .map(|input| input.path.clone()),
        );
        let registry = crate::group_identity::GroupRegistry::rebuild(
            files.into_iter().enumerate().map(|(ix, path)| {
                let mode = self
                    .overrides
                    .iter()
                    .find(|p| p.path == path)
                    .map(|p| p.params.import.mode)
                    .unwrap_or(self.params.import.mode);
                (ix, path, mode)
            }),
            &mut self.derived,
            &mut self.source_groups,
            &self.source_origins,
        );
        let ids: Vec<_> = self.derived.iter().map(|g| g.id).collect();
        for group in &mut self.derived {
            if let Some(operation) = &mut group.operation {
                for input in &mut operation.inputs {
                    if input.group_id.is_none() {
                        input.group_id = match input.derived_id {
                            Some(id) => Some(
                                ids.iter()
                                    .position(|i| *i == id)
                                    .and_then(|i| registry.id(crate::app::DERIVED_BASE + i))
                                    .unwrap_or_else(|| {
                                        crate::group_identity::GroupId::legacy_result(id)
                                    }),
                            ),
                            None => self
                                .source_groups
                                .iter()
                                .find(|g| g.path == input.path)
                                .map(|g| g.id.clone()),
                        };
                    }
                }
            }
        }
        next
    }

    /// Whether the session holds work worth offering to save: an open
    /// project, imported data, derived groups, a fit model or history,
    /// measurement records or assistant conversations. A fresh window whose
    /// only state is default settings returns `false`, so it closes silently.
    pub fn has_user_content(&self) -> bool {
        self.origin.is_some()
            || self.source_dir.is_some()
            || self.spectrum_file.is_some()
            || !self.raw_files.is_empty()
            || !self.derived.is_empty()
            || !self.fit_paths.is_empty()
            || !self.fit_history.is_empty()
            || !self.series_measurements.runs.is_empty()
            || !self.series_measurements.live_sessions.is_empty()
            || !self.assistant.conversations.is_empty()
    }

    /// A process-local hash of the saved content, used only to detect
    /// unsaved changes. Objects are canonicalized through `serde_json::Value`
    /// (sorted keys). Two saved fields are excluded because they change
    /// without a user edit: the header, which saving rewrites, and
    /// `parser_evidence`, the diagnostics cache that each file read refills
    /// (opening a project reads its spectra again). Edits that change how
    /// files are read still show up in the import settings and parameters.
    /// Returns `None` when the model cannot be serialized.
    pub fn content_fingerprint(&self) -> Option<u64> {
        use std::hash::{Hash, Hasher};
        let mut value = serde_json::to_value(self).ok()?;
        if let Some(object) = value.as_object_mut() {
            object.remove("header");
            object.remove("parser_evidence");
        }
        let mut hasher = std::hash::DefaultHasher::new();
        value.to_string().hash(&mut hasher);
        Some(hasher.finish())
    }

    /// Whether closing would lose work. `saved` is the fingerprint recorded
    /// when this project was last saved or opened (`None` if never). A
    /// session with no user content never asks; otherwise any difference
    /// from the saved fingerprint, or no saved fingerprint, counts as unsaved.
    pub fn has_unsaved_changes(&self, saved: Option<u64>) -> bool {
        match saved {
            None => self.has_user_content(),
            Some(saved) => self.content_fingerprint() != Some(saved),
        }
    }
}

/// Independent of the application version. Optional additions keep this version;
/// incompatible changes require an explicit migration and new fixture.
pub const PROJECT_VERSION: u32 = 1;
pub const PROJECT_EXTENSION: &str = "rxs";

/// Test only whether the path has a case-insensitive `.rxs` extension.
/// This does not read or validate the file; use [`load`] to decode a project.
pub fn is_project(path: &Path) -> bool {
    path.extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case(PROJECT_EXTENSION))
}

/// Save a format-1 `.rxs` snapshot using the project's selected storage mode.
///
/// The input model is borrowed and cloned during preparation; the caller's
/// paths, settings and conversation history are not changed. See
/// [`save_with_storage`] for source-file requirements and replacement behavior.
pub fn save(path: &Path, project: &ProjectFile) -> Result<(), String> {
    save_with_storage(path, project, project.data_storage).map(|_| ())
}

/// Serialize a project and return the metadata header written to disk.
///
/// Paths mode records linked inputs and allows unavailable sources. Embedded
/// mode reads every required input, preserves its original bytes, and fails if
/// a required source cannot be read. References are made relative to the new
/// project directory. Completed conversations are pruned to the selected limit
/// in the saved copy; processed caches and the session undo stack are not saved.
///
/// Serialization and the 512 MiB project limit are checked before replacement.
/// An existing destination is copied to `<path>.bak`, then an atomic write
/// replaces the project. Only one previous save is retained. A final write
/// failure can leave the backup updated while the previous project stays intact.
/// Invalid suffixes, unsupported project versions, serialization failures and
/// filesystem errors return a message without modifying the borrowed model.
pub fn save_with_storage(
    path: &Path,
    project: &ProjectFile,
    mode: DataStorage,
) -> Result<ProjectHeader, String> {
    if !is_project(path) {
        return Err("Save rexafs projects with the .rxs extension.".into());
    }
    check_version(project.version.max(1))?;
    let mut prepared = storage::prepare(project, path, mode)?;
    prepared.assistant.prune_for_save();
    retain_portable_fit_results(&mut prepared);
    // Measurement history already has lossless dictionary encoding. Keep it
    // out of the generic compactor's Value/validation copies: a 100k-frame run
    // otherwise expands into several simultaneous JSON object trees.
    let measurements = std::mem::take(&mut prepared.series_measurements);
    let mut value = serde_json::to_value(&prepared).map_err(|e| e.to_string())?;
    value["version"] = PROJECT_VERSION.into();
    let mut json = compact::encode(value)?;
    if !measurements.live_sessions.is_empty()
        || !measurements.series.is_empty()
        || !measurements.runs.is_empty()
        || !measurements.presets.is_empty()
        || !measurements.recipes.is_empty()
    {
        json.pop(); // The generic compactor always produces one JSON object.
        json.extend_from_slice(b",\"series_measurements\":");
        serde_json::to_writer(&mut json, &measurements).map_err(|e| e.to_string())?;
        json.push(b'}');
    }
    if json.len() as u64 > storage::MAX_PROJECT_BYTES {
        return Err(
            "Project exceeds the 512 MiB file limit; use paths or a smaller selection.".into(),
        );
    }
    // Complete serialization/validation before touching either the old project
    // or its backup. A future-format file must never be downgraded in place.
    match std::fs::read(path) {
        Ok(previous) => {
            if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&previous) {
                version(&value)?;
            }
            let mut backup = path.as_os_str().to_os_string();
            backup.push(".bak");
            atomic_write(Path::new(&backup), &previous)?;
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
        Err(e) => return Err(e.to_string()),
    }
    atomic_write(path, &json)?;
    Ok(prepared.header.unwrap())
}

/// Keep only result arrays that belong to a history entry and survive a JSON
/// round trip. JSON has no NaN or infinity (serde writes `null`), so such a
/// result could not be read back; dropping it saves the project without that
/// fit's curves instead of refusing the whole save.
fn retain_portable_fit_results(project: &mut ProjectFile) {
    let ids: std::collections::BTreeSet<_> = project.fit_history.iter().map(|f| f.id).collect();
    project.fit_results.retain(|id, result| {
        ids.contains(id)
            && serde_json::to_value(result.as_ref())
                .and_then(serde_json::from_value::<rexafs::prelude::FeffFitResult>)
                .is_ok()
    });
    if project
        .current_fit
        .is_some_and(|id| !project.fit_results.contains_key(&id))
    {
        project.current_fit = None;
    }
}

/// Read and validate a `.rxs` project without rewriting its source file.
///
/// Relative input paths resolve against the project directory. Embedded inputs
/// are size/hash checked and extracted into the private project-data cache;
/// originals are never replaced. Paths mode restores references without checking
/// their current bytes against the recorded source hashes. Missing linked files
/// are reported when a subsequent operation tries to read them.
///
/// Rejects unsupported formats, malformed metadata, files larger than 512 MiB,
/// unsafe embedded paths and damaged payloads. Returns a fresh owned model;
/// the application applies it only after loading succeeds.
pub fn load(path: &Path) -> Result<ProjectFile, String> {
    load_with_cache_root(path, || {
        crate::settings::app_dir().ok_or("Project cache directory unavailable".into())
    })
}

/// Open the immutable bundled raw-mu example as a new, unsaved project. Embedded
/// sources reuse one content-addressed extraction folder across repeated opens.
pub(crate) fn copper_example() -> Result<ProjectFile, String> {
    let root = crate::settings::app_dir().ok_or("Project cache directory unavailable")?;
    copper_example_in(&root)
}

fn copper_example_in(root: &Path) -> Result<ProjectFile, String> {
    let json = include_str!("../data/examples/cu-reduction.rxs");
    let path = root.join("examples/Synthetic copper reduction.rxs");
    let mut project =
        storage::restore(parse(json)?, &path, json.as_bytes(), || Ok(root.to_owned()))?;
    project.origin = None;
    Ok(project)
}

pub(crate) fn load_with_cache_root(
    path: &Path,
    cache_root: impl FnOnce() -> Result<PathBuf, String>,
) -> Result<ProjectFile, String> {
    if !is_project(path) {
        return Err("Open a .rxs project file.".into());
    }
    if std::fs::metadata(path).map_err(|e| e.to_string())?.len() > storage::MAX_PROJECT_BYTES {
        return Err("Project exceeds the 512 MiB file limit.".into());
    }
    let json = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let project = parse(&json)?;
    storage::restore(project, path, json.as_bytes(), cache_root)
}

fn check_version(version: u32) -> Result<(), String> {
    if version == 0 {
        return Err("Project format version must be at least 1.".into());
    }
    if version > PROJECT_VERSION {
        return Err(format!(
            "Project format {version} is newer than supported format {PROJECT_VERSION}. Open it with a newer rexafs release; the file has not been changed."
        ));
    }
    Ok(())
}
fn version(value: &serde_json::Value) -> Result<u32, String> {
    if !value.is_object() {
        return Err("A project must be a JSON object.".into());
    }
    let v = match value.get("version") {
        None => return Err("Project format version is missing.".into()),
        Some(v) => v
            .as_u64()
            .and_then(|v| u32::try_from(v).ok())
            .ok_or("Project version must be a nonnegative integer.")?,
    };
    check_version(v)?;
    Ok(v)
}
fn parse(json: &str) -> Result<ProjectFile, String> {
    let mut value: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    version(&value)?;
    if let Some(groups) = value.get_mut("derived").and_then(|v| v.as_array_mut()) {
        for group in groups {
            if !group.is_object() {
                continue; // Let serde report malformed groups without indexing them.
            }
            if group.get("quantity").is_none() {
                let materialized = group.get("source").is_none_or(|s| s.is_null());
                let label = group["label"].as_str().unwrap_or_default();
                // Historical tool labels are hints only: even a recognizable
                // marker remains unconfirmed until explicitly reviewed.
                let difference =
                    materialized && (label.starts_with("diff:") || label.contains("Δμnorm"));
                group["quantity"] = serde_json::json!(if difference {
                    crate::params::Quantity::NormalizedDifference
                } else {
                    crate::params::Quantity::RawMu
                });
                group["quantity_unconfirmed"] = materialized.into();
            }
        }
    }
    let mut project: ProjectFile = serde_json::from_value(value).map_err(|e| e.to_string())?;
    let mut ids = std::collections::BTreeSet::new();
    for group in &project.derived {
        if group.id != 0
            && (!ids.insert(group.id) || group.id >= u64::MAX - project.derived.len() as u64 - 1)
        {
            return Err("Additional spectrum IDs must be unique and within range.".into());
        }
    }
    let mut durable_ids = std::collections::BTreeSet::new();
    for id in project
        .source_groups
        .iter()
        .map(|g| &g.id)
        .chain(project.derived.iter().filter_map(|g| g.group_id.as_ref()))
    {
        if !durable_ids.insert(id) {
            return Err("Group IDs must be unique.".into());
        }
    }
    project.version = PROJECT_VERSION;
    Ok(project)
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    replace_with(path, |file| {
        use std::io::Write;
        file.write_all(bytes)
    })
}

/// The same-directory rename is the commit point. Until then the previous file
/// remains intact, including when writing, syncing or renaming fails.
fn replace_with(
    path: &Path,
    write: impl FnOnce(&mut std::fs::File) -> std::io::Result<()>,
) -> Result<(), String> {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let mut name = path.as_os_str().to_os_string();
    name.push(format!(
        ".{}.{}.tmp",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let tmp = PathBuf::from(name);
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut file = opts.open(&tmp).map_err(|e| e.to_string())?;
    let result = write(&mut file).and_then(|()| file.sync_all());
    drop(file);
    let result = result.and_then(|()| std::fs::rename(&tmp, path));
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result.map_err(|e| e.to_string())
}

#[cfg(test)]
pub(crate) mod tests;

impl ProjectFile {
    pub(crate) fn correction_receipts(
        &self,
    ) -> Vec<&crate::fluorescence_history::CorrectionReceipt> {
        let mut out = self
            .derived
            .iter()
            .flat_map(|d| d.corrections.iter())
            .collect::<Vec<_>>();
        for input in self
            .lcf_analysis
            .iter()
            .flat_map(|a| a.inputs.iter())
            .chain(self.pca_analysis.iter().flat_map(|a| a.inputs.iter()))
            .chain(self.mcr_analysis.iter().flat_map(|a| a.inputs.iter()))
            .chain(
                self.mcr_analysis
                    .iter()
                    .flat_map(|a| a.comparison.iter())
                    .flat_map(|c| c.inputs.iter()),
            )
            .chain(
                self.lcf_series_analysis
                    .iter()
                    .flat_map(|a| a.inputs.values().chain(a.standards.iter())),
            )
        {
            out.extend(&input.corrections);
        }
        out
    }
    pub(crate) fn relocate_corrections(
        &mut self,
        f: &mut impl FnMut(&Path) -> Result<PathBuf, String>,
    ) -> Result<(), String> {
        for r in self
            .derived
            .iter_mut()
            .flat_map(|d| d.corrections.iter_mut())
        {
            r.path = f(&r.path)?;
        }
        for input in self
            .lcf_analysis
            .iter_mut()
            .flat_map(|a| a.inputs.iter_mut())
            .chain(
                self.pca_analysis
                    .iter_mut()
                    .flat_map(|a| a.inputs.iter_mut()),
            )
            .chain(
                self.mcr_analysis
                    .iter_mut()
                    .flat_map(|a| a.inputs.iter_mut()),
            )
            .chain(
                self.lcf_series_analysis
                    .iter_mut()
                    .flat_map(|a| a.inputs.values_mut().chain(a.standards.iter_mut())),
            )
        {
            for r in &mut input.corrections {
                r.path = f(&r.path)?;
            }
        }
        for input in self
            .mcr_analysis
            .iter_mut()
            .flat_map(|a| a.comparison.iter_mut())
            .flat_map(|c| c.inputs.iter_mut())
        {
            for r in &mut input.corrections {
                r.path = f(&r.path)?;
            }
        }
        Ok(())
    }
}
