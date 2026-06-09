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

/// Compact entry: interned dir id + filename.
pub struct EntryMeta {
    pub dir: u32,
    pub name: Box<str>,
    #[allow(dead_code)] // feeds parameter-fingerprint cache keys in M2
    pub size: u64,
}

#[derive(Default)]
pub struct Catalog {
    dirs: Vec<Arc<str>>,
    dir_ids: HashMap<Arc<str>, u32>,
    pub entries: Vec<EntryMeta>,
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
            self.entries.push(EntryMeta {
                dir: dir_id,
                name: file.name,
                size: file.size,
            });
        }
    }

    pub fn name(&self, ix: usize) -> &str {
        &self.entries[ix].name
    }

    pub fn path(&self, ix: usize) -> PathBuf {
        let entry = &self.entries[ix];
        PathBuf::from(&*self.dirs[entry.dir as usize]).join(&*entry.name)
    }
}
