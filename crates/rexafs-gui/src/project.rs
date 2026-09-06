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
mod compact;
mod storage;
pub use storage::{DataStorage, ProjectHeader};

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
    pub feff_workspace: Option<PathBuf>,
    pub derived: Vec<DerivedSpectrum>,
    /// Completed fits (model snapshot + statistics), oldest first.
    pub fit_history: Vec<FitHistoryEntry>,
    pub joint: crate::joint_fitting::JointConfig,
    pub(crate) publication: crate::publication::figures::FigureSettings,
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

/// Independent of the application version. Optional additions keep this version;
/// incompatible changes require an explicit migration and new fixture.
pub const PROJECT_VERSION: u32 = 1;
pub const PROJECT_EXTENSION: &str = "rxs";

pub fn is_project(path: &Path) -> bool {
    path.extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case(PROJECT_EXTENSION))
}

pub fn save(path: &Path, project: &ProjectFile) -> Result<(), String> {
    save_with_storage(path, project, project.data_storage).map(|_| ())
}

pub fn save_with_storage(
    path: &Path,
    project: &ProjectFile,
    mode: DataStorage,
) -> Result<ProjectHeader, String> {
    if !is_project(path) {
        return Err("Save rexafs projects with the .rxs extension.".into());
    }
    check_version(project.version.max(1))?;
    let prepared = storage::prepare(project, path, mode)?;
    let mut value = serde_json::to_value(&prepared).map_err(|e| e.to_string())?;
    value["version"] = PROJECT_VERSION.into();
    let json = compact::encode(value)?;
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

pub fn load(path: &Path) -> Result<ProjectFile, String> {
    if !is_project(path) {
        return Err("Open a .rxs project file.".into());
    }
    if std::fs::metadata(path).map_err(|e| e.to_string())?.len() > storage::MAX_PROJECT_BYTES {
        return Err("Project exceeds the 512 MiB file limit.".into());
    }
    let json = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let project = parse(&json)?;
    storage::restore(project, path, json.as_bytes())
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
    let value: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    version(&value)?;
    let mut project: ProjectFile = serde_json::from_value(value).map_err(|e| e.to_string())?;
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
mod tests;
