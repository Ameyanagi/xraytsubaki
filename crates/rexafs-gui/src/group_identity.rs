//! Durable identities and the index adapter shared by catalog mutations.
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::app::DERIVED_BASE;
use crate::params::{DerivedSpectrum, DetectionMode, PipelineParams};

/// Opaque, immutable identity. Source locators can move without changing it.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct GroupId(String);

impl GroupId {
    /// Return the stable identifier used in saved references and curve keys.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// `path` is a canonical source location (or its saved original location).
    /// Auto denotes the primary, lazily detected interpretation of a file.
    pub fn source(path: &Path, channel: DetectionMode) -> Self {
        let hash = Sha256::digest(format!("{path:?}:{channel:?}"));
        Self(format!(
            "source:{}",
            hash.iter().map(|b| format!("{b:02x}")).collect::<String>()
        ))
    }

    pub fn legacy_result(id: u64) -> Self {
        Self(format!("result:{id}"))
    }

    pub fn new_result() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        Self(format!(
            "result:{}:{}:{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default(),
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ))
    }
}

/// A primary channel's immutable identity and separately relocatable locator.
#[derive(Clone, Serialize, Deserialize)]
pub struct SourceGroup {
    pub id: GroupId,
    pub path: PathBuf,
    pub channel: DetectionMode,
}

#[derive(Clone, Default)]
pub struct GroupRegistry {
    inner: RefCell<RegistryData>,
}

#[derive(Clone, Default)]
struct RegistryData {
    by_id: BTreeMap<GroupId, usize>,
    by_index: BTreeMap<usize, GroupId>,
    sources: BTreeMap<PathBuf, SourceGroup>,
    // Includes removed groups: undo/redo owns identities even while absent.
    issued: BTreeSet<GroupId>,
    excluded: BTreeSet<GroupId>,
    #[cfg(test)]
    append_visits: usize,
}

impl GroupRegistry {
    pub fn from_sources(sources: Vec<SourceGroup>) -> Self {
        let registry = Self::default();
        for source in sources {
            let mut data = registry.inner.borrow_mut();
            data.issued.insert(source.id.clone());
            data.sources.insert(source.path.clone(), source);
        }
        registry
    }

    pub fn sources(&self) -> Vec<SourceGroup> {
        self.inner.borrow().sources.values().cloned().collect()
    }

    pub fn source(&self, path: &Path) -> Option<SourceGroup> {
        self.inner.borrow().sources.get(path).cloned()
    }

    /// An explicit import creates a new group, leaving historical inputs missing.
    pub fn reimport_source(&self, path: &Path, ix: Option<usize>) -> Option<GroupId> {
        let mut data = self.inner.borrow_mut();
        let mut source = data.sources.get(path)?.clone();
        if !data.excluded.contains(&source.id) {
            return None;
        }
        data.by_id.remove(&source.id);
        source.id = reserve_id(&mut data.issued, GroupId::source(path, source.channel));
        let id = source.id.clone();
        data.sources.insert(path.to_path_buf(), source);
        if let Some(ix) = ix {
            data.by_index.insert(ix, id.clone());
            data.by_id.insert(id.clone(), ix);
        }
        Some(id)
    }

    /// Undo may need to reintroduce a standalone source after a catalog import.
    pub fn restore_source(&self, source: SourceGroup, ix: usize) {
        let mut data = self.inner.borrow_mut();
        if let Some(old) = data.by_index.insert(ix, source.id.clone()) {
            data.by_id.remove(&old);
        }
        data.by_id.insert(source.id.clone(), ix);
        data.issued.insert(source.id.clone());
        data.sources.insert(source.path.clone(), source);
    }

    /// Resolve the would-be source identity without growing the registry.
    pub fn peek_source(
        &self,
        path: &Path,
        channel: DetectionMode,
        origins: &BTreeMap<PathBuf, PathBuf>,
    ) -> GroupId {
        let data = self.inner.borrow();
        if let Some(source) = data.sources.get(path) {
            return source.id.clone();
        }
        let base = GroupId::source(
            origins.get(path).map(PathBuf::as_path).unwrap_or(path),
            channel,
        );
        available_id(&data.issued, base)
    }

    /// Allocate only when a source is referenced, not for every scanned file.
    /// A standalone source has a locator but no catalog index yet.
    pub fn register_source(
        &self,
        ix: Option<usize>,
        path: PathBuf,
        channel: DetectionMode,
        origins: &BTreeMap<PathBuf, PathBuf>,
    ) -> GroupId {
        let mut data = self.inner.borrow_mut();
        let id = if let Some(source) = data.sources.get(&path) {
            source.id.clone()
        } else {
            let base = GroupId::source(origins.get(&path).unwrap_or(&path), channel);
            let id = reserve_id(&mut data.issued, base);
            data.sources.insert(
                path.clone(),
                SourceGroup {
                    id: id.clone(),
                    path,
                    channel,
                },
            );
            id
        };
        if let Some(ix) = ix {
            data.by_id.insert(id.clone(), ix);
            data.by_index.insert(ix, id.clone());
        }
        id
    }

    pub fn assign_group(&self, group: &mut DerivedSpectrum, origins: &BTreeMap<PathBuf, PathBuf>) {
        let mut data = self.inner.borrow_mut();
        if let Some(id) = &group.group_id {
            data.issued.insert(id.clone());
        } else {
            let base = match &group.source {
                Some(path) => GroupId::source(
                    origins.get(path).unwrap_or(path),
                    group
                        .params
                        .as_ref()
                        .map(|p| p.import.mode)
                        .unwrap_or_default(),
                ),
                None => GroupId::legacy_result(group.id),
            };
            group.group_id = Some(reserve_id(&mut data.issued, base));
        }
    }

    /// Pure legacy preparation. Live catalogs use lazy source registration.
    pub fn rebuild(
        files: impl IntoIterator<Item = (usize, PathBuf, DetectionMode)>,
        derived: &mut [DerivedSpectrum],
        sources: &mut Vec<SourceGroup>,
        origins: &BTreeMap<PathBuf, PathBuf>,
    ) -> Self {
        let registry = Self::from_sources(sources.clone());
        registry.reserve_groups(derived);
        for (ix, path, channel) in files {
            registry.register_source(Some(ix), path, channel, origins);
        }
        for group in derived.iter_mut() {
            registry.assign_group(group, origins);
        }
        registry.replace_derived(derived);
        *sources = registry.sources();
        registry
    }

    pub fn reserve_groups(&self, derived: &[DerivedSpectrum]) {
        self.inner
            .borrow_mut()
            .issued
            .extend(derived.iter().filter_map(|g| g.group_id.clone()));
    }

    /// Append examines only new locators; unreferenced files allocate no IDs.
    pub fn append_catalog(&self, catalog: &crate::catalog::Catalog, start: usize) {
        let mut data = self.inner.borrow_mut();
        if data.sources.is_empty() {
            return;
        }
        for ix in start..catalog.len() {
            #[cfg(test)]
            {
                data.append_visits += 1;
            }
            if let Some(id) = data.sources.get(&catalog.path(ix)).map(|s| s.id.clone()) {
                data.by_id.insert(id.clone(), ix);
                data.by_index.insert(ix, id);
            }
        }
    }

    /// Resolve locators and build both maps on the background executor.
    pub fn prepare_catalog(
        catalog: &crate::catalog::Catalog,
        sources: &[SourceGroup],
    ) -> PreparedCatalog {
        let mut prepared = PreparedCatalog::default();
        for source in sources {
            prepared.known.insert(source.id.clone());
            if let Some(ix) = catalog.find_by_canonical_path(&source.path) {
                prepared.by_id.insert(source.id.clone(), ix);
                prepared.by_index.insert(ix, source.id.clone());
            }
        }
        prepared
    }

    pub fn replace_catalog(&self, mut prepared: PreparedCatalog) {
        let mut data = self.inner.borrow_mut();
        for (ix, id) in data.by_index.range(DERIVED_BASE..) {
            prepared.by_id.insert(id.clone(), *ix);
            prepared.by_index.insert(*ix, id.clone());
        }
        data.by_id = prepared.by_id;
        data.by_index = prepared.by_index;
    }

    fn clear_indices(&self, derived: bool) {
        let mut data = self.inner.borrow_mut();
        let removed = data
            .by_index
            .range(if derived {
                DERIVED_BASE..usize::MAX
            } else {
                0..DERIVED_BASE
            })
            .map(|(ix, id)| (*ix, id.clone()))
            .collect::<Vec<_>>();
        for (ix, id) in removed {
            data.by_index.remove(&ix);
            data.by_id.remove(&id);
        }
    }

    pub fn append_derived(&self, derived: &[DerivedSpectrum], start: usize) {
        let mut data = self.inner.borrow_mut();
        for (i, group) in derived.iter().enumerate().skip(start) {
            if let Some(id) = &group.group_id {
                data.issued.insert(id.clone());
                data.by_id.insert(id.clone(), DERIVED_BASE + i);
                data.by_index.insert(DERIVED_BASE + i, id.clone());
            }
        }
    }

    pub fn replace_derived(&self, derived: &[DerivedSpectrum]) -> bool {
        let changed = self
            .inner
            .borrow()
            .by_index
            .range(DERIVED_BASE..)
            .any(|(ix, id)| {
                derived
                    .get(ix - DERIVED_BASE)
                    .and_then(|g| g.group_id.as_ref())
                    != Some(id)
            });
        self.clear_indices(true);
        let mut data = self.inner.borrow_mut();
        for (i, group) in derived.iter().enumerate() {
            if let Some(id) = &group.group_id {
                data.issued.insert(id.clone());
                data.by_id.insert(id.clone(), DERIVED_BASE + i);
                data.by_index.insert(DERIVED_BASE + i, id.clone());
            }
        }
        changed
    }

    pub fn set_excluded(&self, excluded: &BTreeSet<GroupId>) {
        let mut data = self.inner.borrow_mut();
        data.excluded.clone_from(excluded);
        data.issued.extend(excluded.iter().cloned());
    }

    pub fn is_excluded(&self, id: &GroupId) -> bool {
        self.inner.borrow().excluded.contains(id)
    }

    pub fn excluded_indices(&self) -> BTreeSet<usize> {
        let data = self.inner.borrow();
        data.excluded
            .iter()
            .filter_map(|id| data.by_id.get(id).copied())
            .collect()
    }

    pub fn index_excluded(&self, ix: usize) -> bool {
        let data = self.inner.borrow();
        data.by_index
            .get(&ix)
            .is_some_and(|id| data.excluded.contains(id))
    }

    pub fn id(&self, ix: usize) -> Option<GroupId> {
        let data = self.inner.borrow();
        data.by_index
            .get(&ix)
            .filter(|id| !data.excluded.contains(*id))
            .cloned()
    }
    pub fn index(&self, id: &GroupId) -> Option<usize> {
        (!self.is_excluded(id))
            .then(|| self.inner.borrow().by_id.get(id).copied())
            .flatten()
    }
    #[cfg(test)]
    pub fn indices_changed(&self, next: &Self) -> bool {
        self.inner
            .borrow()
            .by_index
            .iter()
            .any(|(ix, id)| next.index(id) != Some(*ix))
    }
    pub fn ids(&self, indices: &BTreeSet<usize>) -> BTreeSet<GroupId> {
        indices.iter().filter_map(|ix| self.id(*ix)).collect()
    }
    pub fn indices(&self, ids: &BTreeSet<GroupId>) -> BTreeSet<usize> {
        ids.iter().filter_map(|id| self.index(id)).collect()
    }
}

#[derive(Default)]
pub struct PreparedCatalog {
    by_id: BTreeMap<GroupId, usize>,
    by_index: BTreeMap<usize, GroupId>,
    known: BTreeSet<GroupId>,
}

impl PreparedCatalog {
    /// Only sources referenced while preparation was in flight need resolving.
    pub fn include_recent(&mut self, catalog: &crate::catalog::Catalog, registry: &GroupRegistry) {
        let data = registry.inner.borrow();
        for source in data
            .sources
            .values()
            .filter(|s| !self.known.contains(&s.id))
        {
            if let Some(ix) = catalog.find_by_canonical_path(&source.path) {
                if let Some(old) = self.by_index.insert(ix, source.id.clone()) {
                    self.by_id.remove(&old);
                }
                self.by_id.insert(source.id.clone(), ix);
            }
        }
    }
}

fn reserve_id(issued: &mut BTreeSet<GroupId>, base: GroupId) -> GroupId {
    let id = available_id(issued, base);
    issued.insert(id.clone());
    id
}

fn available_id(issued: &BTreeSet<GroupId>, base: GroupId) -> GroupId {
    let mut id = base.clone();
    let mut variant = 0;
    while issued.contains(&id) {
        variant += 1;
        id = GroupId(format!("{}:variant:{variant}", base.0));
    }
    id
}

/// Index-based UI state is captured before a swap and resolved afterwards.
/// Unresolved identities remain here, including when saved during a scan.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct GroupState {
    /// Project exclusions survive rescans without deleting source files.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub excluded: BTreeSet<GroupId>,
    pub labels: BTreeMap<GroupId, String>,
    pub colors: BTreeMap<GroupId, u8>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub plot_colors: BTreeMap<GroupId, crate::spectrum_colors::Assignment>,
    pub marked: BTreeSet<GroupId>,
    pub frozen: BTreeSet<GroupId>,
    pub overrides: Vec<(GroupId, PipelineParams)>,
    pub current: Option<GroupId>,
}

impl GroupState {
    pub(crate) fn display_label(
        &self,
        id: Option<&GroupId>,
        default: impl FnOnce() -> String,
    ) -> String {
        id.and_then(|id| self.labels.get(id))
            .cloned()
            .unwrap_or_else(default)
    }

    /// Standalone rows have a durable identity but no registry index yet.
    pub(crate) fn capture_standalone_lock(
        &mut self,
        registry: &GroupRegistry,
        id: &GroupId,
        locked: bool,
    ) {
        if locked {
            self.frozen.insert(id.clone());
        } else if registry.index(id).is_none() {
            self.frozen.remove(id);
        }
    }

    pub fn resolved_overrides(&self, registry: &GroupRegistry) -> BTreeMap<usize, PipelineParams> {
        self.overrides
            .iter()
            .filter_map(|(id, p)| registry.index(id).map(|ix| (ix, p.clone())))
            .collect()
    }

    pub fn capture(
        &mut self,
        registry: &GroupRegistry,
        marked: &BTreeSet<usize>,
        frozen: &BTreeSet<usize>,
        overrides: &BTreeMap<usize, PipelineParams>,
        current: Option<usize>,
    ) {
        self.marked.retain(|id| registry.index(id).is_none());
        self.marked.extend(registry.ids(marked));
        self.frozen.retain(|id| registry.index(id).is_none());
        self.frozen.extend(registry.ids(frozen));
        self.overrides
            .retain(|(id, _)| registry.index(id).is_none());
        self.overrides.extend(
            overrides
                .iter()
                .filter_map(|(ix, p)| registry.id(*ix).map(|id| (id, p.clone()))),
        );
        if current.is_some()
            || self
                .current
                .as_ref()
                .is_some_and(|id| registry.index(id).is_some())
        {
            self.current = current.and_then(|ix| registry.id(ix));
        }
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn explicit_reimport_preserves_exclusion_and_replaces_stale_prepared_binding() {
        let mut catalog = crate::catalog::Catalog::default();
        catalog.extend(vec![crate::catalog::FileMeta {
            dir: "/data".into(),
            name: "a.dat".into(),
            size: 0,
        }]);
        let path = catalog.path(0);
        let registry = GroupRegistry::default();
        let old = registry.register_source(
            Some(0),
            path.clone(),
            DetectionMode::Auto,
            &Default::default(),
        );
        let excluded = BTreeSet::from([old.clone()]);
        registry.set_excluded(&excluded);
        let mut pending = GroupRegistry::prepare_catalog(&catalog, &registry.sources());
        let fresh = registry.reimport_source(&path, Some(0)).unwrap();
        assert_ne!(fresh, old);
        pending.include_recent(&catalog, &registry);
        registry.replace_catalog(pending);
        assert_eq!(registry.id(0), Some(fresh.clone()));
        assert_eq!(registry.index(&old), None);
        assert!(registry.is_excluded(&old));
        assert!(registry.reimport_source(&path, Some(0)).is_none());
        let reopened = GroupRegistry::from_sources(
            serde_json::from_value(serde_json::to_value(registry.sources()).unwrap()).unwrap(),
        );
        reopened.set_excluded(&excluded);
        reopened.append_catalog(&catalog, 0);
        assert_eq!(reopened.id(0), Some(fresh));
        assert!(reopened.is_excluded(&old));
    }

    use super::*;

    #[test]
    fn group_identity_colour_peek_is_lazy_and_agrees_with_registration() {
        let registry = GroupRegistry::default();
        let source = PathBuf::from("/original/scan.dat");
        let cached = PathBuf::from("/cache/scan.dat");
        let origins = BTreeMap::from([(cached.clone(), source.clone())]);
        for i in 0..1000 {
            let path = PathBuf::from(format!("/scan/{i}.dat"));
            assert_eq!(
                registry.peek_source(&path, DetectionMode::Auto, &origins),
                GroupId::source(&path, DetectionMode::Auto)
            );
        }
        assert!(registry.sources().is_empty());
        assert!(registry.inner.borrow().issued.is_empty());
        assert!(registry.inner.borrow().by_index.is_empty());
        let expected = registry.peek_source(&cached, DetectionMode::Reference, &origins);
        assert_eq!(expected, GroupId::source(&source, DetectionMode::Reference));
        assert_eq!(
            registry.register_source(Some(42), cached.clone(), DetectionMode::Reference, &origins),
            expected
        );
        // The saved identity wins even after the current import mode changes.
        assert_eq!(
            registry.peek_source(&cached, DetectionMode::Transmission, &origins),
            expected
        );
        assert_eq!(registry.sources().len(), 1);
        let saved = GroupRegistry::from_sources(registry.sources());
        assert_eq!(
            saved.peek_source(&cached, DetectionMode::Auto, &origins),
            expected
        );
        // Reserved derived identities produce the same variant with and without insertion.
        let mut derived = DerivedSpectrum {
            source: Some("/variant.dat".into()),
            ..Default::default()
        };
        registry.assign_group(&mut derived, &origins);
        let variant = registry.peek_source(
            std::path::Path::new("/variant.dat"),
            DetectionMode::Auto,
            &origins,
        );
        assert_ne!(Some(&variant), derived.group_id.as_ref());
        assert_eq!(
            registry.register_source(None, "/variant.dat".into(), DetectionMode::Auto, &origins),
            variant
        );
    }

    #[test]
    fn group_identity_removed_channel_reserves_id_through_import_and_redo() {
        let registry = GroupRegistry::default();
        let origins = BTreeMap::new();
        let channel = |id| DerivedSpectrum {
            id,
            source: Some("/standalone/multi.dat".into()),
            params: Some(PipelineParams {
                import: crate::params::ImportConfig {
                    mode: DetectionMode::Reference,
                    ..Default::default()
                },
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut a = channel(1);
        registry.assign_group(&mut a, &origins);
        registry.replace_derived(&[a.clone()]);
        registry.replace_derived(&[]); // undo; journal retains A
        registry.register_source(
            Some(0),
            "/standalone/multi.dat".into(),
            DetectionMode::Auto,
            &origins,
        );
        let mut b = channel(2); // automatic reference detection during import
        registry.assign_group(&mut b, &origins);
        assert_ne!(a.group_id, b.group_id);
        registry.replace_derived(&[b.clone()]);
        let mut state = GroupState::default();
        state.capture(
            &registry,
            &BTreeSet::from([DERIVED_BASE]),
            &BTreeSet::from([DERIVED_BASE]),
            &BTreeMap::from([(
                DERIVED_BASE,
                PipelineParams {
                    e0: Some(123.),
                    ..Default::default()
                },
            )]),
            Some(DERIVED_BASE),
        );
        let b_id = b.group_id.clone();
        registry.replace_derived(&[a.clone(), b.clone()]); // redo inserts A before B
        assert_eq!(registry.id(DERIVED_BASE), a.group_id);
        assert_eq!(registry.id(DERIVED_BASE + 1), b_id);
        assert_eq!(state.marked, BTreeSet::from([b_id.unwrap()]));
        assert_eq!(
            registry.indices(&state.marked),
            BTreeSet::from([DERIVED_BASE + 1])
        );
        assert_eq!(
            registry.indices(&state.frozen),
            BTreeSet::from([DERIVED_BASE + 1])
        );
        assert_eq!(
            state.resolved_overrides(&registry)[&(DERIVED_BASE + 1)].e0,
            Some(123.)
        );
    }

    #[test]
    fn group_identity_million_file_append_is_lazy_and_visits_only_new_ranges() {
        use crate::catalog::{Catalog, FileMeta};
        let mut catalog = Catalog::default();
        let registry = GroupRegistry::default();
        let id = registry.register_source(
            None,
            "/scan/0000999.dat".into(),
            DetectionMode::Transmission,
            &BTreeMap::new(),
        );
        let mut visited = 0;
        for batch in 0..1000 {
            let start = catalog.len();
            catalog.extend(
                (batch * 1000..(batch + 1) * 1000)
                    .map(|i| FileMeta {
                        dir: std::sync::Arc::from("/scan"),
                        name: format!("{i:07}.dat").into_boxed_str(),
                        size: 1,
                    })
                    .collect(),
            );
            registry.append_catalog(&catalog, start);
            visited += catalog.len() - start;
        }
        assert_eq!(visited, 1_000_000);
        assert_eq!(registry.index(&id), Some(999));
        let data = registry.inner.borrow();
        assert_eq!(data.append_visits, 1_000_000);
        assert_eq!(
            data.sources.len(),
            1,
            "unreferenced sources must not be hashed/persisted"
        );
        assert_eq!(data.issued.len(), 1);
        assert_eq!(data.by_index.len(), 1);
    }

    #[test]
    fn group_identity_rekeys_state_and_retains_missing_sources_until_restored() {
        let mut sources = Vec::new();
        let origins = BTreeMap::new();
        let build = |names: &[&str], sources: &mut Vec<SourceGroup>| {
            GroupRegistry::rebuild(
                names
                    .iter()
                    .enumerate()
                    .map(|(ix, name)| (ix, PathBuf::from(name), DetectionMode::Auto)),
                &mut [],
                sources,
                &origins,
            )
        };
        let old = build(&["/b.dat", "/c.dat"], &mut sources);
        let params = PipelineParams {
            e0: Some(42.),
            ..Default::default()
        };
        let mut state = GroupState::default();
        state.capture(
            &old,
            &BTreeSet::from([1]),
            &BTreeSet::from([0]),
            &BTreeMap::from([(1, params.clone())]),
            Some(1),
        );
        let color_id = old.id(1).unwrap();
        state.colors.insert(color_id.clone(), 6);
        state.labels.insert(color_id.clone(), "Cu foil".into());
        let saved = serde_json::to_value(&state).unwrap();
        let missing = build(&["/a.dat", "/b.dat"], &mut sources);
        assert!(old.indices_changed(&missing));
        assert!(missing.indices(&state.marked).is_empty());
        state.capture(
            &missing,
            &missing.indices(&state.marked),
            &missing.indices(&state.frozen),
            &state.resolved_overrides(&missing),
            None,
        );
        assert_eq!(serde_json::to_value(&state).unwrap(), saved);
        let restored = build(&["/a.dat", "/b.dat", "/c.dat"], &mut sources);
        assert_eq!(restored.indices(&state.marked), BTreeSet::from([2]));
        assert_eq!(restored.indices(&state.frozen), BTreeSet::from([1]));
        assert!(state.resolved_overrides(&restored)[&2] == params);
        assert_eq!(restored.index(state.current.as_ref().unwrap()), Some(2));
        assert_eq!(restored.id(2), Some(color_id.clone()));
        assert_eq!(state.colors[&color_id], 6);
        assert_eq!(state.labels[&color_id], "Cu foil");
        // Explicitly unmarking/resetting a present source must not resurrect it.
        state.capture(
            &restored,
            &BTreeSet::new(),
            &BTreeSet::new(),
            &BTreeMap::new(),
            None,
        );
        assert!(state.marked.is_empty() && state.frozen.is_empty() && state.overrides.is_empty());
    }

    #[test]
    fn group_identity_channels_variants_origins_and_result_ids() {
        let source = PathBuf::from("/source/data.dat");
        let cache = PathBuf::from("/cache/raw/data.dat");
        assert_ne!(
            GroupId::source(&source, DetectionMode::Reference),
            GroupId::source(&source, DetectionMode::Transmission)
        );
        assert_ne!(GroupId::new_result(), GroupId::new_result());
        let mut groups: Vec<_> = [1, 2]
            .into_iter()
            .map(|id| DerivedSpectrum {
                id,
                source: Some(cache.clone()),
                ..Default::default()
            })
            .collect();
        let origins = BTreeMap::from([(cache, source.clone())]);
        let registry = GroupRegistry::default();
        registry.reserve_groups(&groups);
        for group in &mut groups {
            registry.assign_group(group, &origins);
        }
        assert_eq!(
            groups[0].group_id,
            Some(GroupId::source(&source, DetectionMode::Auto))
        );
        assert_ne!(groups[0].group_id, groups[1].group_id);
        let ids: Vec<_> = groups.iter().map(|g| g.group_id.clone()).collect();
        groups.reverse();
        let registry = GroupRegistry::default();
        registry.reserve_groups(&groups);
        for group in &mut groups {
            registry.assign_group(group, &origins);
        }
        assert_eq!(groups[0].group_id, ids[1]);
        assert_eq!(groups[1].group_id, ids[0]);
    }
}
