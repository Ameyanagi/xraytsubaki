//! Portable paths, source metadata, and lossless embedded input files.
use super::*;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use flate2::{Compression, read::GzDecoder, write::GzEncoder};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Read, Write},
    path::Component,
};

pub(super) const MAX_PROJECT_BYTES: u64 = 512 * 1024 * 1024;
const MAX_RAW_BYTES: u64 = 1024 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataStorage {
    #[default]
    Paths,
    Embedded,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectHeader {
    pub format: String,
    pub format_version: u32,
    pub software: String,
    pub software_version: String,
    pub created_utc: String,
    pub saved_utc: String,
    pub storage: DataStorage,
    pub path_base: String,
    pub files: Vec<SourceFile>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SourceFile {
    pub path: PathBuf,
    pub kind: SourceKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modified_unix_seconds: Option<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_header: Vec<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub header_truncated: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archive_path: Option<PathBuf>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    Spectrum,
    Feff,
}

fn digest(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn absolute(path: &Path) -> Result<PathBuf, String> {
    let path = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()
            .map_err(|e| e.to_string())?
            .join(path)
    };
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => (),
            Component::ParentDir => {
                out.pop();
            }
            c => out.push(c.as_os_str()),
        }
    }
    Ok(out)
}
fn base(path: &Path) -> Result<PathBuf, String> {
    Ok(absolute(path)?
        .parent()
        .ok_or("Project has no parent directory")?
        .to_owned())
}
fn relative(path: &Path, base: &Path) -> Result<PathBuf, String> {
    let absolute = absolute(path)?;
    // Native dialogs can return a resolved directory while imported files use
    // an alias (for example /private/tmp versus /tmp on macOS). Compare both
    // locations consistently so saving beside the data still stores data/file.
    let source_location = resolved_location(&absolute);
    let base_location = resolved_location(base);
    let a: Vec<_> = source_location.components().collect();
    let b: Vec<_> = base_location.components().collect();
    if a.first() != b.first() {
        return Ok(absolute);
    } // Different Windows volumes.
    let shared = a.iter().zip(&b).take_while(|(a, b)| a == b).count();
    let mut out = PathBuf::new();
    for _ in shared..b.len() {
        out.push("..");
    }
    for c in &a[shared..] {
        out.push(c.as_os_str());
    }
    if out.as_os_str().is_empty() {
        out.push(".");
    }
    Ok(out)
}

pub(super) fn resolved_location(path: &Path) -> PathBuf {
    let mut parent = path;
    let mut tail = Vec::new();
    loop {
        if let Ok(mut resolved) = std::fs::canonicalize(parent) {
            for name in tail.into_iter().rev() {
                resolved.push(name);
            }
            return resolved;
        }
        // Missing linked inputs still need portable references. Resolve their
        // nearest existing ancestor while retaining the missing tail.
        match (parent.file_name(), parent.parent()) {
            (Some(name), Some(next)) => {
                tail.push(name);
                parent = next;
            }
            _ => return path.to_owned(),
        }
    }
}
fn portable(path: PathBuf) -> PathBuf {
    #[cfg(windows)]
    {
        return PathBuf::from(path.to_string_lossy().replace('\\', "/"));
    }
    #[cfg(not(windows))]
    {
        path
    }
}

/// Every schema-owned path is transformed; scientific expressions and extension
/// metadata are opaque strings and must never be rewritten as paths.
pub(super) fn map_paths(
    project: &mut ProjectFile,
    f: &mut impl FnMut(&Path) -> Result<PathBuf, String>,
) -> Result<(), String> {
    fn option(
        p: &mut Option<PathBuf>,
        f: &mut impl FnMut(&Path) -> Result<PathBuf, String>,
    ) -> Result<(), String> {
        if let Some(v) = p {
            *v = f(v)?;
        }
        Ok(())
    }
    fn paths(
        p: &mut [FitPathSpec],
        f: &mut impl FnMut(&Path) -> Result<PathBuf, String>,
    ) -> Result<(), String> {
        for v in p {
            v.file = f(&v.file)?;
        }
        Ok(())
    }
    fn joint(
        j: &mut crate::joint_fitting::JointConfig,
        f: &mut impl FnMut(&Path) -> Result<PathBuf, String>,
    ) -> Result<(), String> {
        for d in &mut j.datasets {
            d.file = f(&d.file)?;
            for p in &mut d.paths {
                *p = f(p)?;
            }
            d.expressions = std::mem::take(&mut d.expressions)
                .into_iter()
                .map(|(key, mut value)| {
                    value.file = f(&value.file)?;
                    Ok((f(&key)?, value))
                })
                .collect::<Result<_, String>>()?;
        }
        Ok(())
    }
    option(&mut project.source_dir, f)?;
    option(&mut project.spectrum_file, f)?;
    option(&mut project.feff_workspace, f)?;
    for p in &mut project.overrides {
        p.path = f(&p.path)?;
    }
    for group in &mut project.derived {
        option(&mut group.source, f)?;
    }
    for p in &mut project.raw_files {
        *p = f(p)?;
    }
    paths(&mut project.fit_paths, f)?;
    joint(&mut project.joint, f)?;
    for h in &mut project.fit_history {
        paths(&mut h.paths, f)?;
        for p in &mut h.path_details {
            p.file = f(&p.file)?;
        }
        if let Some(j) = &mut h.joint {
            joint(j, f)?;
        }
    }
    Ok(())
}

fn inputs(
    project: &ProjectFile,
    mode: DataStorage,
    include_workspace_metadata: bool,
) -> Result<BTreeMap<PathBuf, SourceKind>, String> {
    let mut files = BTreeMap::new();
    let mut raw = |p: &Path| {
        if !p.as_os_str().is_empty() {
            files.insert(p.to_owned(), SourceKind::Spectrum);
        }
    };
    for group in &project.derived {
        if let Some(path) = &group.source {
            raw(path);
        }
    }
    if mode == DataStorage::Embedded || project.source_dir.is_none() {
        for p in &project.raw_files {
            raw(p);
        }
    }
    if mode == DataStorage::Embedded {
        if let Some(dir) = &project.source_dir {
            for entry in walkdir::WalkDir::new(dir).follow_links(false) {
                let entry =
                    entry.map_err(|e| format!("Cannot include the complete source folder: {e}"))?;
                if entry.file_type().is_file()
                    && entry.path().extension().is_some_and(|ext| {
                        crate::catalog::SPECTRUM_EXTENSIONS
                            .iter()
                            .any(|e| ext.eq_ignore_ascii_case(e))
                    })
                {
                    raw(entry.path());
                }
            }
        }
    }
    if let Some(p) = &project.spectrum_file {
        raw(p);
    }
    for p in &project.overrides {
        raw(&p.path);
    }
    for d in &project.joint.datasets {
        raw(&d.file);
    }
    for h in &project.fit_history {
        if let Some(j) = &h.joint {
            for d in &j.datasets {
                raw(&d.file);
            }
        }
    }
    let mut feff = |p: &Path| {
        files.insert(p.to_owned(), SourceKind::Feff);
    };
    for p in &project.fit_paths {
        feff(&p.file);
    }
    for d in &project.joint.datasets {
        for p in &d.paths {
            feff(p);
        }
        for (p, v) in &d.expressions {
            feff(p);
            feff(&v.file);
        }
    }
    for h in &project.fit_history {
        for p in &h.paths {
            feff(&p.file);
        }
        for p in &h.path_details {
            feff(&p.file);
        }
        if let Some(j) = &h.joint {
            for d in &j.datasets {
                for p in &d.paths {
                    feff(p);
                }
                for (p, v) in &d.expressions {
                    feff(p);
                    feff(&v.file);
                }
            }
        }
    }
    let mut workspaces: BTreeSet<_> = files
        .iter()
        .filter(|(_, k)| **k == SourceKind::Feff)
        .filter_map(|(p, _)| p.parent().map(Path::to_owned))
        .collect();
    workspaces.extend(project.feff_workspace.clone());
    for workspace in workspaces
        .into_iter()
        .filter(|_| include_workspace_metadata)
    {
        for name in ["feff.inp", "crystal.json", "engine.txt"] {
            let p = workspace.join(name);
            if p.is_file() {
                files.insert(p, SourceKind::Feff);
            }
        }
    }
    Ok(files)
}

fn archive_path(
    path: &Path,
    kind: SourceKind,
    source_dir: Option<&Path>,
) -> Result<PathBuf, String> {
    if kind == SourceKind::Spectrum {
        if let Some(root) = source_dir
            && let Ok(tail) = path.strip_prefix(root)
            && safe_tail(tail)
        {
            return Ok(portable(Path::new("raw").join(tail)));
        }
    }
    let parent = path.parent().ok_or("Source has no directory")?;
    let group = &digest(parent.to_string_lossy().as_bytes())[..16];
    let name = path.file_name().ok_or("Source has no filename")?;
    Ok(portable(
        Path::new(if kind == SourceKind::Spectrum {
            "raw/external"
        } else {
            "feff"
        })
        .join(group)
        .join(name),
    ))
}
fn safe_tail(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path.components().all(|c| matches!(c, Component::Normal(_)))
        && !path.to_string_lossy().contains(['\\', ':'])
}

pub(super) fn prepare(
    project: &ProjectFile,
    path: &Path,
    mode: DataStorage,
) -> Result<ProjectFile, String> {
    let folder = base(path)?;
    let mut out = project.clone();
    map_paths(&mut out, &mut absolute)?;
    out.version = PROJECT_VERSION;
    out.embedded.clear();
    let mut records = Vec::new();
    let mut total = 0u64;
    for (input, kind) in inputs(&out, mode, true)? {
        let reference = if mode == DataStorage::Embedded {
            out.source_origins.get(&input).unwrap_or(&input)
        } else {
            &input
        };
        let mut record = SourceFile {
            path: portable(relative(reference, &folder)?),
            kind,
            bytes: None,
            sha256: None,
            modified_unix_seconds: None,
            source_header: vec![],
            header_truncated: false,
            archive_path: None,
        };
        match std::fs::File::open(&input) {
            Ok(mut file) => {
                let meta = file.metadata().map_err(|e| e.to_string())?;
                if !meta.is_file() {
                    return Err(format!("{} is not a regular input file", input.display()));
                }
                total = total.checked_add(meta.len()).ok_or("Input size overflow")?;
                if mode == DataStorage::Embedded && total > MAX_RAW_BYTES {
                    return Err(
                        "Embedded inputs exceed the 1 GiB limit; use paths or a smaller selection."
                            .into(),
                    );
                }
                let mut hash = Sha256::new();
                let mut prefix = Vec::new();
                let mut count = 0u64;
                let mut compressed = (mode == DataStorage::Embedded)
                    .then(|| GzEncoder::new(Vec::new(), Compression::best()));
                let mut buffer = [0u8; 65536];
                loop {
                    let n = file.read(&mut buffer).map_err(|e| e.to_string())?;
                    if n == 0 {
                        break;
                    }
                    count += n as u64;
                    hash.update(&buffer[..n]);
                    if prefix.len() < 32768 {
                        let take = n.min(32768 - prefix.len());
                        prefix.extend_from_slice(&buffer[..take]);
                    }
                    if let Some(writer) = &mut compressed {
                        writer.write_all(&buffer[..n]).map_err(|e| e.to_string())?;
                    }
                    if mode == DataStorage::Embedded && count > MAX_RAW_BYTES {
                        return Err("Input grew beyond the embedded size limit.".into());
                    }
                }
                if count != meta.len()
                    || file.metadata().map_err(|e| e.to_string())?.modified().ok()
                        != meta.modified().ok()
                {
                    return Err(format!(
                        "{} changed while saving; retry the save.",
                        input.display()
                    ));
                }
                let sha = hex(&hash.finalize());
                record.bytes = Some(count);
                record.sha256 = Some(sha.clone());
                record.modified_unix_seconds = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs());
                // Extracting a portable project must not replace original
                // acquisition provenance with the cache file's timestamp.
                if input != *reference
                    && let (Some(header), Some(origin)) = (&project.header, &project.origin)
                    && let Some(previous) = header.files.iter().find(|f| {
                        absolute(&origin.parent().unwrap().join(&f.path))
                            .ok()
                            .as_ref()
                            == Some(reference)
                            && f.sha256.as_ref() == Some(&sha)
                    })
                {
                    record.modified_unix_seconds = previous.modified_unix_seconds;
                }
                if let Ok(text) = std::str::from_utf8(&prefix) {
                    record.source_header = text
                        .lines()
                        .take_while(|l| {
                            l.trim().is_empty() || l.trim_start().starts_with(['#', ';', '%', '!'])
                        })
                        .filter(|l| !l.trim().is_empty())
                        .map(str::to_owned)
                        .collect();
                    record.header_truncated = count > prefix.len() as u64
                        && text.lines().all(|l| {
                            l.trim().is_empty() || l.trim_start().starts_with(['#', ';', '%', '!'])
                        });
                }
                if let Some(writer) = compressed {
                    record.archive_path =
                        Some(archive_path(&input, kind, out.source_dir.as_deref())?);
                    out.embedded
                        .entry(sha)
                        .or_insert(STANDARD.encode(writer.finish().map_err(|e| e.to_string())?));
                }
            }
            Err(e) if mode == DataStorage::Paths && e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(format!("Cannot include {}: {e}", input.display())),
        }
        records.push(record);
    }
    records.sort_by(|a, b| a.path.cmp(&b.path));
    let origins = out.source_origins.clone();
    map_paths(&mut out, &mut |p| {
        relative(
            if mode == DataStorage::Embedded {
                origins.get(p).map(PathBuf::as_path).unwrap_or(p)
            } else {
                p
            },
            &folder,
        )
        .map(portable)
    })?;
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    out.header = Some(ProjectHeader {
        format: "rxs".into(),
        format_version: PROJECT_VERSION,
        software: "rexafs".into(),
        software_version: env!("CARGO_PKG_VERSION").into(),
        created_utc: project
            .header
            .as_ref()
            .map(|h| h.created_utc.clone())
            .unwrap_or_else(|| now.clone()),
        saved_utc: now,
        storage: mode,
        path_base: "project_directory".into(),
        files: records,
    });
    out.origin = None;
    Ok(out)
}

pub(super) fn restore(
    mut project: ProjectFile,
    path: &Path,
    json: &[u8],
) -> Result<ProjectFile, String> {
    let header = project
        .header
        .as_ref()
        .ok_or("Missing .rxs metadata header")?
        .clone();
    if header.format != "rxs"
        || header.format_version != project.version
        || header.path_base != "project_directory"
    {
        return Err("Unsupported .rxs header or path base.".into());
    }
    let folder = base(path)?;
    map_paths(&mut project, &mut |p| absolute(&folder.join(p)))?;
    project.raw_files = header
        .files
        .iter()
        .filter(|f| f.kind == SourceKind::Spectrum)
        .map(|f| absolute(&folder.join(&f.path)))
        .collect::<Result<_, _>>()?;
    if header.storage == DataStorage::Embedded {
        let root = crate::settings::app_dir()
            .ok_or("Project cache directory unavailable")?
            .join("project-data")
            .join(digest(json));
        restore_embedded(&mut project, &header, &folder, &root)?;
    } else if !project.embedded.is_empty() {
        return Err("A paths-only project contains unexpected embedded payloads.".into());
    }
    project.data_storage = header.storage;
    project.origin = Some(absolute(path)?);
    Ok(project)
}

fn restore_embedded(
    project: &mut ProjectFile,
    header: &ProjectHeader,
    folder: &Path,
    root: &Path,
) -> Result<(), String> {
    let mut mapping = BTreeMap::new();
    let mut destinations = BTreeSet::new();
    let mut sources = BTreeSet::new();
    let mut total = 0u64;
    // Check every destination before creating anything. Archive paths are never
    // interpreted as arbitrary filesystem locations.
    for entry in &header.files {
        let stored = entry
            .archive_path
            .as_ref()
            .ok_or("Embedded file has no archive path")?;
        if !safe_tail(stored)
            || !matches!(stored.components().next(),Some(Component::Normal(x)) if x == if entry.kind == SourceKind::Spectrum { "raw" } else { "feff" })
            || !destinations.insert(stored.clone())
            || !sources.insert(absolute(&folder.join(&entry.path))?)
        {
            return Err("Unsafe or duplicate embedded file path.".into());
        }
        total = total
            .checked_add(entry.bytes.ok_or("Embedded file has no byte count")?)
            .ok_or("Embedded size overflow")?;
        if total > MAX_RAW_BYTES {
            return Err("Embedded data exceed the 1 GiB limit.".into());
        }
    }
    // A purportedly self-contained project cannot silently fall back to an
    // unrelated file on the receiving computer for a referenced input.
    for required in inputs(project, DataStorage::Paths, false)?.keys() {
        if !sources.contains(required) {
            return Err(format!("Embedded input is missing: {}", required.display()));
        }
    }
    std::fs::create_dir_all(root.join("raw")).map_err(|e| e.to_string())?;
    for entry in &header.files {
        let sha = entry
            .sha256
            .as_ref()
            .ok_or("Embedded file has no checksum")?;
        let encoded = project
            .embedded
            .get(sha)
            .ok_or("Embedded payload is missing")?;
        let bytes = STANDARD.decode(encoded).map_err(|e| e.to_string())?;
        let expected = entry.bytes.unwrap();
        let mut decoded = Vec::new();
        GzDecoder::new(bytes.as_slice())
            .take(expected + 1)
            .read_to_end(&mut decoded)
            .map_err(|e| e.to_string())?;
        if decoded.len() as u64 != expected || digest(&decoded) != *sha {
            return Err("Embedded file integrity check failed.".into());
        }
        let dest = root.join(entry.archive_path.as_ref().unwrap());
        std::fs::create_dir_all(dest.parent().unwrap()).map_err(|e| e.to_string())?;
        // Replace only files inside our private cache, never the original inputs.
        atomic_write(&dest, &decoded)?;
        let source = absolute(&folder.join(&entry.path))?;
        if entry.kind == SourceKind::Feff {
            if let (Some(a), Some(b)) = (source.parent(), dest.parent()) {
                mapping.insert(a.to_owned(), b.to_owned());
            }
        }
        mapping.insert(source, dest);
    }
    if let Some(dir) = &project.source_dir {
        mapping.insert(dir.clone(), root.join("raw"));
    }
    project.source_origins = mapping
        .iter()
        .map(|(from, to)| (to.clone(), from.clone()))
        .collect();
    map_paths(project, &mut |p| {
        Ok(mapping.get(p).cloned().unwrap_or_else(|| p.to_owned()))
    })?;
    Ok(())
}
