//! Lazy spectrum catalog: a compact in-RAM index of (possibly millions of)
//! data files, built by a background scanner thread that streams batches to
//! the UI. Nothing is parsed until a spectrum is actually shown.
//!
//! Memory model: one interned `Arc<str>` per directory + one `Box<str>`
//! filename and a u32 dir id per entry, so a million-file tree costs tens of
//! MB, not gigabytes.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use futures::channel::mpsc;

/// File extensions treated as spectrum data files during a scan.
pub const SPECTRUM_EXTENSIONS: &[&str] = &["dat", "txt", "xmu", "chi"];

const BATCH_SIZE: usize = 2048;

/// A scanned file, streamed from the scanner thread. The directory is shared
/// per-batch via `Arc` so the per-file cost is one pointer clone.
pub struct FileMeta {
    pub dir: Arc<str>,
    pub name: Box<str>,
    pub size: u64,
}

pub enum ScanEvent {
    // Error is reserved for scanner-side failures (e.g. root vanishes mid-scan).
    Batch(Vec<FileMeta>),
    Done {
        total: usize,
    },
    #[allow(dead_code)]
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
            for entry in walkdir::WalkDir::new(&root)
                .follow_links(false)
                .sort_by_file_name()
                .into_iter()
                .filter_map(|e| e.ok())
            {
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
                let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
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
pub struct EntryMeta {
    pub dir: u32,
    #[allow(dead_code)] // feeds parameter-fingerprint cache keys in M2
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
    pub entries: Vec<EntryMeta>,
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
            self.entries.push(EntryMeta {
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
}

#[cfg(test)]
mod tests {
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
}
