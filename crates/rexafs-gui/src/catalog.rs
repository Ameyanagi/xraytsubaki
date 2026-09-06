//! Lazy spectrum catalog: a compact in-RAM index of (possibly millions of)
//! data files, built by a background scanner thread that streams batches to
//! the UI. Nothing is parsed until a spectrum is actually shown.
//!
//! Memory model: one interned `Arc<str>` per directory + one `Box<str>`
//! filename and a u32 dir id per entry, so a million-file tree costs tens of
//! MB, not gigabytes.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures::channel::mpsc;

/// File extensions treated as spectrum data files during a scan.
pub const SPECTRUM_EXTENSIONS: &[&str] = &["dat", "txt", "xmu", "chi", "xdi"];

const BATCH_SIZE: usize = 2048;

/// A scanned file, streamed from the scanner thread. The directory is shared
/// per-batch via `Arc` so the per-file cost is one pointer clone.
pub struct FileMeta {
    pub dir: Arc<str>,
    pub name: Box<str>,
    pub size: u64,
}

pub enum ScanEvent {
    Batch(Vec<FileMeta>),
    Done { total: usize },
    Error(String),
}

/// Walk `root` on a dedicated thread, streaming matching files in batches.
/// Dropping the receiver cancels the scan (sends fail and the thread exits).
pub fn start_scan(root: PathBuf) -> mpsc::UnboundedReceiver<ScanEvent> {
    let (tx, rx) = mpsc::unbounded();
    std::thread::Builder::new()
        .name("catalog-scan".into())
        .spawn(move || {
            let mut batch: Vec<FileMeta> = Vec::with_capacity(BATCH_SIZE);
            let mut total = 0usize;
            let mut current_dir: Option<(PathBuf, Arc<str>)> = None;

            // Sorted traversal: scan/time-series members must appear in
            // index order, not OS directory order.
            for result in walkdir::WalkDir::new(&root)
                .follow_links(false)
                .sort_by_file_name()
                .into_iter()
            {
                let entry = match result {
                    Ok(entry) => entry,
                    Err(error) => {
                        let _ = tx.unbounded_send(ScanEvent::Error(error.to_string()));
                        return;
                    }
                };
                if !entry.file_type().is_file() {
                    continue;
                }
                let name = entry.file_name().to_string_lossy();
                let Some(ext) = name.rsplit('.').next() else {
                    continue;
                };
                if !SPECTRUM_EXTENSIONS
                    .iter()
                    .any(|e| ext.eq_ignore_ascii_case(e))
                {
                    continue;
                }
                let parent = entry.path().parent().unwrap_or(&root);
                let dir: Arc<str> = match &current_dir {
                    Some((p, arc)) if p == parent => arc.clone(),
                    _ => {
                        let arc: Arc<str> = Arc::from(parent.to_string_lossy().as_ref());
                        current_dir = Some((parent.to_path_buf(), arc.clone()));
                        arc
                    }
                };
                let size = match entry.metadata() {
                    Ok(metadata) => metadata.len(),
                    Err(error) => {
                        let _ = tx.unbounded_send(ScanEvent::Error(format!(
                            "cannot read metadata for {}: {error}",
                            entry.path().display()
                        )));
                        return;
                    }
                };
                batch.push(FileMeta {
                    dir,
                    name: name.into_owned().into_boxed_str(),
                    size,
                });
                total += 1;
                if batch.len() >= BATCH_SIZE
                    && tx
                        .unbounded_send(ScanEvent::Batch(std::mem::take(&mut batch)))
                        .is_err()
                {
                    return; // receiver dropped -> cancelled
                }
            }
            if !batch.is_empty() {
                let _ = tx.unbounded_send(ScanEvent::Batch(batch));
            }
            let _ = tx.unbounded_send(ScanEvent::Done { total });
        })
        .expect("spawn catalog-scan thread");
    rx
}

/// Compact entry: interned dir id (+ size). File names live in the chunked
/// name store so filter snapshots can share them (see [`NAME_CHUNK`]).
#[derive(Clone, PartialEq, Eq)]
pub struct EntryMeta {
    pub dir: u32,
    pub size: u64,
}

/// Name-store chunk size. Sealed chunks are Arc-shared, so a filter
/// snapshot costs len/NAME_CHUNK pointer clones plus one deep copy of the
/// unsealed tail (< NAME_CHUNK short strings) — sub-millisecond even at
/// 10^6 entries, while the O(n) match itself runs off the UI thread.
pub const NAME_CHUNK: usize = 4096;

/// Immutable, cheaply cloneable view of the catalog file names, handed to
/// the background filter job.
#[derive(Clone)]
pub struct NameSnapshot {
    sealed: Vec<Arc<[Box<str>]>>,
    tail: Arc<[Box<str>]>,
}

impl NameSnapshot {
    pub fn len(&self) -> usize {
        self.sealed.len() * NAME_CHUNK + self.tail.len()
    }

    #[allow(dead_code)] // completes the len() API; exercised in tests
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[allow(dead_code)] // random access for future selection expressions; tested
    pub fn get(&self, ix: usize) -> &str {
        match self.sealed.get(ix / NAME_CHUNK) {
            Some(chunk) => &chunk[ix % NAME_CHUNK],
            None => &self.tail[ix - self.sealed.len() * NAME_CHUNK],
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.sealed
            .iter()
            .flat_map(|chunk| chunk.iter())
            .chain(self.tail.iter())
            .map(|name| &**name)
    }
}

/// A scan = one directory of time-ordered spectra (entries are contiguous
/// because the scanner walks sorted, depth-first).
pub struct ScanGroup {
    pub dir: u32,
    pub label: String,
    pub start: usize,
    pub len: usize,
}

#[derive(Default)]
pub struct Catalog {
    dirs: Vec<Arc<str>>,
    dir_ids: HashMap<Arc<str>, u32>,
    entries: Arc<Vec<EntryMeta>>,
    sealed_names: Vec<Arc<[Box<str>]>>,
    tail_names: Vec<Box<str>>,
    pub scans: Vec<ScanGroup>,
    pub scanning: bool,
}

impl Catalog {
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn extend(&mut self, batch: Vec<FileMeta>) {
        for file in batch {
            let dir_id = match self.dir_ids.get(&file.dir) {
                Some(&id) => id,
                None => {
                    let id = self.dirs.len() as u32;
                    self.dirs.push(file.dir.clone());
                    self.dir_ids.insert(file.dir, id);
                    id
                }
            };
            let ix = self.entries.len();
            Arc::make_mut(&mut self.entries).push(EntryMeta {
                dir: dir_id,
                size: file.size,
            });
            self.tail_names.push(file.name);
            if self.tail_names.len() == NAME_CHUNK {
                self.sealed_names
                    .push(std::mem::take(&mut self.tail_names).into());
            }
            match self.scans.last_mut() {
                Some(scan) if scan.dir == dir_id => scan.len += 1,
                _ => {
                    let dir_path = &self.dirs[dir_id as usize];
                    let label = std::path::Path::new(&**dir_path)
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| dir_path.to_string());
                    self.scans.push(ScanGroup {
                        dir: dir_id,
                        label,
                        start: ix,
                        len: 1,
                    });
                }
            }
        }
    }

    pub fn name(&self, ix: usize) -> &str {
        match self.sealed_names.get(ix / NAME_CHUNK) {
            Some(chunk) => &chunk[ix % NAME_CHUNK],
            None => &self.tail_names[ix - self.sealed_names.len() * NAME_CHUNK],
        }
    }

    pub fn path(&self, ix: usize) -> PathBuf {
        let entry = &self.entries[ix];
        PathBuf::from(&*self.dirs[entry.dir as usize]).join(self.name(ix))
    }

    #[cfg(test)]
    fn entry_size(&self, ix: usize) -> u64 {
        self.entries[ix].size
    }

    /// Locate an entry by its full path (parent dir + file name) — used to
    /// re-key path-persisted per-spectrum overrides onto the current index.
    /// Scans are per-directory, so only the matching directory's members
    /// are searched, never the whole catalog.
    pub fn find_by_path(&self, path: &Path) -> Option<usize> {
        let dir = path.parent()?.to_string_lossy();
        let name = path.file_name()?.to_string_lossy();
        let dir_id = *self.dir_ids.get(dir.as_ref())?;
        self.scans
            .iter()
            .filter(|scan| scan.dir == dir_id)
            .flat_map(|scan| scan.start..scan.start + scan.len)
            .find(|&ix| self.name(ix) == name)
    }

    pub fn names_snapshot(&self) -> NameSnapshot {
        NameSnapshot {
            sealed: self.sealed_names.clone(),
            tail: self.tail_names.clone().into(),
        }
    }

    /// (start, len, label) per scan; one row per directory, so this stays
    /// tiny even for million-file catalogs.
    #[allow(dead_code)] // feeds scan-chip/selection-expression work; tested
    pub fn scan_spans(&self) -> Vec<(usize, usize, String)> {
        self.scans
            .iter()
            .map(|scan| (scan.start, scan.len, scan.label.clone()))
            .collect()
    }

    /// Cheap, `Send` view of everything the index file stores: interned dirs
    /// (Arc clones), per-entry (dir id, size), and the shared name store.
    /// Encoding and reconcile comparisons run off the UI thread on this.
    pub fn index_parts(&self) -> IndexParts {
        IndexParts {
            dirs: self.dirs.clone(),
            metas: self.entries.clone(),
            names: self.names_snapshot(),
        }
    }
}

// ---- persisted catalog index (doc: "Indexes persist next to the project
// file, so reopening a million-file project is < 1 s") -----------------------
//
// The index lives in a per-user cache directory keyed by a hash of the
// canonical root path rather than next to the project file: it also serves
// plain "Open Folder" sessions with no project, and beamline source trees
// are frequently read-only network mounts we must not write into. The file
// stores the canonical root, the interned dir table, and one (dir id, size,
// name) record per entry in walk order; scan grouping is re-derived by
// replaying the records through `Catalog::extend`.

/// Version tag; bump when the record layout changes (old files are ignored).
const INDEX_MAGIC: &[u8; 8] = b"XTIDX01\n";

/// Everything needed to encode or compare a catalog index off-thread.
pub struct IndexParts {
    pub dirs: Vec<Arc<str>>,
    pub metas: Arc<Vec<EntryMeta>>,
    pub names: NameSnapshot,
}

impl IndexParts {
    /// True when two walks produced the identical index (same dirs, order,
    /// sizes, names) — the "nothing changed on disk" reconcile fast path.
    pub fn same_index(&self, other: &IndexParts) -> bool {
        self.metas == other.metas
            && self.dirs.len() == other.dirs.len()
            && self
                .dirs
                .iter()
                .zip(&other.dirs)
                .all(|(a, b)| a.as_ref() == b.as_ref())
            && self.names.iter().eq(other.names.iter())
    }
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for &b in bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn canonical_root(root: &Path) -> PathBuf {
    root.canonicalize().unwrap_or_else(|_| root.to_path_buf())
}

/// Cache file for `root`'s index, keyed by the canonical path (stable across
/// runs via FNV-1a; the stored root string guards against collisions).
pub fn index_cache_path(root: &Path) -> Option<PathBuf> {
    let canon = canonical_root(root);
    let base = if cfg!(target_os = "macos") {
        crate::settings::home_dir()?.join("Library/Caches")
    } else if cfg!(target_os = "windows") {
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .or_else(|| crate::settings::home_dir().map(|home| home.join("AppData/Local")))?
    } else {
        std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .or_else(|| crate::settings::home_dir().map(|home| home.join(".cache")))?
    };
    let hash = fnv1a(canon.to_string_lossy().as_bytes());
    Some(
        base.join("rexafs/catalog")
            .join(format!("{hash:016x}.xtidx")),
    )
}

fn push_str(out: &mut Vec<u8>, s: &str) {
    out.extend_from_slice(&(s.len() as u32).to_le_bytes());
    out.extend_from_slice(s.as_bytes());
}

/// Serialize an index snapshot for `root`.
pub fn encode_index(root: &Path, parts: &IndexParts) -> Vec<u8> {
    debug_assert_eq!(parts.metas.len(), parts.names.len());
    // magic + root + dir table + ~(14 bytes + name) per entry
    let mut out = Vec::with_capacity(64 + parts.metas.len() * 32);
    out.extend_from_slice(INDEX_MAGIC);
    push_str(&mut out, canonical_root(root).to_string_lossy().as_ref());
    out.extend_from_slice(&(parts.dirs.len() as u32).to_le_bytes());
    for dir in &parts.dirs {
        push_str(&mut out, dir);
    }
    out.extend_from_slice(&(parts.metas.len() as u64).to_le_bytes());
    for (meta, name) in parts.metas.iter().zip(parts.names.iter()) {
        out.extend_from_slice(&meta.dir.to_le_bytes());
        out.extend_from_slice(&meta.size.to_le_bytes());
        out.extend_from_slice(&(name.len() as u16).to_le_bytes());
        out.extend_from_slice(name.as_bytes());
    }
    out
}

/// Sequential little-endian reader over the index byte format.
struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self
            .pos
            .checked_add(n)
            .filter(|&end| end <= self.bytes.len())
            .ok_or("truncated catalog index")?;
        let slice = &self.bytes[self.pos..end];
        self.pos = end;
        Ok(slice)
    }

    fn u16(&mut self) -> Result<u16, String> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }

    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn u64(&mut self) -> Result<u64, String> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }

    fn str(&mut self) -> Result<&'a str, String> {
        let len = self.u32()? as usize;
        std::str::from_utf8(self.take(len)?).map_err(|_| "corrupt catalog index".to_string())
    }
}

/// Rebuild a catalog from index bytes. `expected_root` must match the stored
/// canonical root (hash-collision and stale-cache guard).
pub fn decode_index(bytes: &[u8], expected_root: &Path) -> Result<Catalog, String> {
    let mut r = Reader { bytes, pos: 0 };
    if r.take(INDEX_MAGIC.len())? != INDEX_MAGIC {
        return Err("unknown catalog index format".into());
    }
    let root = r.str()?;
    if root != canonical_root(expected_root).to_string_lossy() {
        return Err(format!("catalog index is for a different root ({root})"));
    }
    let dir_count = r.u32()? as usize;
    let mut dirs: Vec<Arc<str>> = Vec::with_capacity(dir_count.min(1 << 20));
    for _ in 0..dir_count {
        dirs.push(Arc::from(r.str()?));
    }
    let entry_count = r.u64()? as usize;
    let mut catalog = Catalog::default();
    let mut batch: Vec<FileMeta> = Vec::with_capacity(BATCH_SIZE);
    for _ in 0..entry_count {
        let dir_id = r.u32()? as usize;
        let size = r.u64()?;
        let name_len = r.u16()? as usize;
        let name = std::str::from_utf8(r.take(name_len)?)
            .map_err(|_| "corrupt catalog index".to_string())?;
        let dir = dirs
            .get(dir_id)
            .ok_or("corrupt catalog index (dir id out of range)")?
            .clone();
        batch.push(FileMeta {
            dir,
            name: name.into(),
            size,
        });
        if batch.len() == BATCH_SIZE {
            catalog.extend(std::mem::take(&mut batch));
        }
    }
    catalog.extend(batch);
    if catalog.len() != entry_count {
        return Err("truncated catalog index".into());
    }
    Ok(catalog)
}

/// Atomically (write + rename) persist the index snapshot for `root`.
pub fn write_index(path: &Path, root: &Path, parts: &IndexParts) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let bytes = encode_index(root, parts);
    let tmp = path.with_extension("xtidx.tmp");
    std::fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

/// Read + decode the persisted index for `root`.
pub fn load_index(path: &Path, root: &Path) -> Result<Catalog, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    decode_index(&bytes, root)
}

#[cfg(test)]
mod tests {
    use futures::StreamExt;

    use super::*;

    fn meta(dir: &Arc<str>, name: &str) -> FileMeta {
        FileMeta {
            dir: dir.clone(),
            name: name.into(),
            size: 0,
        }
    }

    #[test]
    fn names_survive_chunk_boundaries_and_snapshots() {
        let mut catalog = Catalog::default();
        let dir: Arc<str> = Arc::from("/data/scan_01");
        let total = NAME_CHUNK + 3;
        catalog.extend(
            (0..total)
                .map(|i| meta(&dir, &format!("f{i:07}.dat")))
                .collect(),
        );
        assert_eq!(catalog.len(), total);
        assert_eq!(catalog.name(0), "f0000000.dat");
        assert_eq!(
            catalog.name(NAME_CHUNK - 1),
            format!("f{:07}.dat", NAME_CHUNK - 1)
        );
        assert_eq!(catalog.name(NAME_CHUNK), format!("f{NAME_CHUNK:07}.dat"));

        let snapshot = catalog.names_snapshot();
        assert_eq!(snapshot.len(), total);
        assert_eq!(snapshot.get(NAME_CHUNK + 2), catalog.name(NAME_CHUNK + 2));
        assert_eq!(snapshot.iter().count(), total);
        assert_eq!(snapshot.iter().next(), Some("f0000000.dat"));

        // Snapshots stay valid while the catalog keeps growing.
        catalog.extend(vec![meta(&dir, "late.dat")]);
        assert_eq!(snapshot.len(), total);
        assert_eq!(catalog.name(total), "late.dat");
        assert_eq!(
            catalog.scan_spans(),
            vec![(0, total + 1, "scan_01".to_string())]
        );
    }

    #[test]
    fn folder_scan_discovers_xdi_case_insensitively() {
        let root = std::env::temp_dir().join(format!("xts-xdi-catalog-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        for name in ["cu.xdi", "ni.XDI", "notes.md"] {
            std::fs::write(root.join(name), "").unwrap();
        }
        let mut scan = start_scan(root.clone());
        let mut names = Vec::new();
        futures::executor::block_on(async {
            while let Some(event) = scan.next().await {
                match event {
                    ScanEvent::Batch(files) => {
                        names.extend(files.into_iter().map(|f| f.name.to_string()))
                    }
                    ScanEvent::Done { total } => assert_eq!(total, 2),
                    ScanEvent::Error(e) => panic!("{e}"),
                }
            }
        });
        assert_eq!(names, ["cu.xdi", "ni.XDI"]);
        std::fs::remove_dir_all(root).unwrap();
    }

    fn sample_catalog() -> Catalog {
        let mut catalog = Catalog::default();
        let dir_a: Arc<str> = Arc::from("/data/scan_01");
        let dir_b: Arc<str> = Arc::from("/data/scan_02");
        let mut batch: Vec<FileMeta> = (0..NAME_CHUNK + 5)
            .map(|i| FileMeta {
                dir: dir_a.clone(),
                name: format!("a{i:05}.dat").into(),
                size: i as u64,
            })
            .collect();
        batch.push(FileMeta {
            dir: dir_b.clone(),
            name: "b00000.dat".into(),
            size: 7,
        });
        catalog.extend(batch);
        catalog
    }

    #[test]
    fn find_by_path_round_trips_catalog_paths() {
        let catalog = sample_catalog();
        for ix in [0, NAME_CHUNK, catalog.len() - 1] {
            assert_eq!(catalog.find_by_path(&catalog.path(ix)), Some(ix));
        }
        assert_eq!(
            catalog.find_by_path(Path::new("/data/scan_01/missing.dat")),
            None
        );
        assert_eq!(
            catalog.find_by_path(Path::new("/elsewhere/a00000.dat")),
            None
        );
    }

    #[test]
    fn index_round_trips_through_bytes() {
        let catalog = sample_catalog();
        let root = Path::new("/data");
        let bytes = encode_index(root, &catalog.index_parts());
        let loaded = decode_index(&bytes, root).expect("decode");
        assert_eq!(loaded.len(), catalog.len());
        assert_eq!(loaded.scans.len(), 2);
        for ix in [0, NAME_CHUNK - 1, NAME_CHUNK, catalog.len() - 1] {
            assert_eq!(loaded.name(ix), catalog.name(ix));
            assert_eq!(loaded.path(ix), catalog.path(ix));
            assert_eq!(loaded.entry_size(ix), catalog.entry_size(ix));
        }
        assert!(loaded.index_parts().same_index(&catalog.index_parts()));
    }

    #[test]
    fn index_rejects_wrong_root_magic_and_truncation() {
        let catalog = sample_catalog();
        let root = Path::new("/data");
        let bytes = encode_index(root, &catalog.index_parts());
        assert!(decode_index(&bytes, Path::new("/elsewhere")).is_err());
        assert!(decode_index(&bytes[..bytes.len() - 3], root).is_err());
        let mut corrupt = bytes.clone();
        corrupt[0] ^= 0xff;
        assert!(decode_index(&corrupt, root).is_err());
    }

    #[test]
    fn same_index_detects_changes() {
        let catalog = sample_catalog();
        let parts = catalog.index_parts();
        assert!(parts.same_index(&catalog.index_parts()));

        let mut grown = sample_catalog();
        grown.extend(vec![FileMeta {
            dir: Arc::from("/data/scan_02"),
            name: "b00001.dat".into(),
            size: 8,
        }]);
        assert!(!parts.same_index(&grown.index_parts()));

        // Same shape, one size changed (file rewritten in place).
        let root = Path::new("/data");
        let mut bytes = encode_index(root, &parts);
        let mut resized = decode_index(&bytes, root).unwrap().index_parts();
        Arc::make_mut(&mut resized.metas)[0].size += 1;
        assert!(!parts.same_index(&resized));
        // and one renamed file
        let pos = bytes.len() - 1;
        bytes[pos] = b'x';
        let renamed = decode_index(&bytes, root).unwrap();
        assert!(!parts.same_index(&renamed.index_parts()));
    }

    #[test]
    fn missing_scan_root_reports_error_without_done() {
        let root =
            std::env::temp_dir().join(format!("rexafs-missing-scan-root-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let mut rx = start_scan(root);
        let first = futures::executor::block_on(rx.next()).expect("scanner event");
        assert!(matches!(first, ScanEvent::Error(_)));
        assert!(futures::executor::block_on(rx.next()).is_none());
    }
}
