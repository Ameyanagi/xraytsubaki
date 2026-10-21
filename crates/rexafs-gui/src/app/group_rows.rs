//! Pure presentation order. Indices are the session adapter; expansion is
//! resolved from durable GroupIds by the caller. No source files are read.
use std::collections::{BTreeMap, BTreeSet};

use super::{DERIVED_BASE, catalog_row_index, filter_match_lower};
use crate::{catalog::Catalog, params::DerivedSpectrum};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    Results,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row {
    Primary {
        group: usize,
        expanded: bool,
        extra_channels: usize,
    },
    Child {
        group: usize,
        parent: usize,
    },
    Header(Section),
    Result {
        group: usize,
    },
}

impl Row {
    pub fn group(self) -> Option<usize> {
        match self {
            Self::Primary { group, .. } | Self::Child { group, .. } | Self::Result { group } => {
                Some(group)
            }
            Self::Header(_) => None,
        }
    }
}

/// Only stack metadata and inserted rows are stored. Catalog primaries remain
/// virtual, including filtered catalogs (the filter's Arc is shared).
pub struct Rows {
    catalog_len: usize,
    excluded: BTreeSet<usize>,
    scan_scope: Option<Box<Rows>>,
    collapsed: BTreeSet<usize>,
    /// Visible ancestors supplied only as context for a matching child.
    context_only: BTreeSet<usize>,
    filtered: Option<std::sync::Arc<Vec<usize>>>,
    base_len: usize,
    base_start: usize,
    primaries: BTreeMap<usize, Row>,
    inserted: Vec<(usize, Row)>,
    inserted_groups: BTreeMap<usize, usize>,
}

impl Rows {
    /// The existing folder-run browser exposes one contiguous expanded run.
    pub fn catalog_range(catalog_len: usize, start: usize, len: usize) -> Self {
        Self {
            catalog_len,
            excluded: BTreeSet::new(),
            scan_scope: None,
            base_start: start.min(catalog_len),
            base_len: len.min(catalog_len.saturating_sub(start)),
            filtered: None,
            collapsed: BTreeSet::new(),
            context_only: BTreeSet::new(),
            primaries: BTreeMap::new(),
            inserted: Vec::new(),
            inserted_groups: BTreeMap::new(),
        }
    }

    /// Use the same filtered members for scan rendering and interaction. Keep
    /// the full filtered model to distinguish collapsed rows from filter misses.
    pub fn in_catalog_range(self, start: usize, len: usize) -> Self {
        let mut rows = Self::catalog_range(self.catalog_len, start, len);
        if let Some(filtered) = &self.filtered {
            rows.base_start = filtered.partition_point(|&g| g < start);
            rows.base_len =
                filtered.partition_point(|&g| g < start.saturating_add(len)) - rows.base_start;
            rows.filtered = Some(filtered.clone());
        }
        rows.excluded = self
            .excluded
            .range(start..start.saturating_add(len))
            .copied()
            .collect();
        rows.base_len = rows.base_len.saturating_sub(rows.excluded.len());
        rows.scan_scope = Some(Box::new(self));
        rows
    }

    pub fn scroll_row(&self, group: usize, expanded_scan: Option<usize>) -> Option<usize> {
        let row = self.row_index(group)?;
        if self.scan_scope.is_some() {
            Some(expanded_scan? + 1 + row)
        } else {
            Some(row)
        }
    }

    pub fn row_count(&self) -> usize {
        self.base_len + self.inserted.len()
    }

    pub fn row_at(&self, row: usize) -> Option<Row> {
        if row >= self.row_count() {
            return None;
        }
        let before = self.inserted.partition_point(|(i, _)| *i < row);
        if let Some(&(i, value)) = self.inserted.get(before)
            && i == row
        {
            return Some(value);
        }
        let mut ordinal = row - before + self.base_start;
        for &skip in &self.excluded {
            let skip = self
                .filtered
                .as_ref()
                .map_or(skip, |f| f.partition_point(|&g| g < skip));
            if skip <= ordinal {
                ordinal += 1;
            }
        }
        let group = catalog_row_index(
            self.filtered.as_deref().map(Vec::as_slice),
            ordinal,
            self.catalog_len,
        )?;
        Some(self.primaries.get(&group).copied().unwrap_or(Row::Primary {
            group,
            expanded: false,
            extra_channels: 0,
        }))
    }

    pub fn row_index(&self, group: usize) -> Option<usize> {
        if let Some(&row) = self.inserted_groups.get(&group) {
            return Some(row);
        }
        if group >= self.catalog_len || self.excluded.contains(&group) {
            return None;
        }
        let base = match &self.filtered {
            Some(f) => f.binary_search(&group).ok()?.checked_sub(self.base_start)?,
            None => group.checked_sub(self.base_start)?,
        };
        let base = base - self.excluded.range(..group).count();
        if base >= self.base_len {
            return None;
        }
        // Insertion anchors are recoverable by subtracting the sparse ordinal.
        let mut lo = 0;
        let mut hi = self.inserted.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            if self.inserted[mid].0 - mid <= base {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        Some(base + lo)
    }

    pub fn neighbor(&self, active: Option<usize>, delta: isize) -> Option<usize> {
        let Some(mut row) = active.and_then(|g| self.row_index(g)) else {
            return (0..self.row_count()).find_map(|i| self.row_at(i)?.group());
        };
        for _ in 0..delta.unsigned_abs() {
            loop {
                row = row.checked_add_signed(delta.signum())?;
                if self.row_at(row)?.group().is_some() {
                    break;
                }
            }
        }
        self.row_at(row)?.group()
    }

    pub fn range(&self, anchor: Option<usize>, endpoint: usize) -> Vec<usize> {
        let Some(end) = self.row_index(endpoint) else {
            return Vec::new();
        };
        let start = anchor.and_then(|g| self.row_index(g)).unwrap_or(end);
        (start.min(end)..=start.max(end))
            .filter_map(|i| self.row_at(i)?.group())
            .collect()
    }

    pub fn shown(&self) -> impl Iterator<Item = usize> + '_ {
        (0..self.row_count()).filter_map(|i| self.row_at(i)?.group())
    }

    pub fn mark_shown(&self, marks: &mut BTreeSet<usize>) {
        marks.extend(self.shown().filter(|g| !self.context_only.contains(g)));
    }

    pub fn invert_shown(&self, marks: &mut BTreeSet<usize>) {
        for group in self.shown().filter(|g| !self.context_only.contains(g)) {
            if !marks.remove(&group) {
                marks.insert(group);
            }
        }
    }

    pub fn hidden_by_filter(&self, group: usize) -> bool {
        if let Some(scope) = &self.scan_scope {
            return scope.hidden_by_filter(group);
        }
        self.row_index(group).is_none() && !self.collapsed.contains(&group)
    }

    pub fn mark_counts(&self, marks: &BTreeSet<usize>) -> (usize, usize, usize) {
        (
            marks.len(),
            marks.iter().filter(|&&g| self.hidden_by_filter(g)).count(),
            marks
                .iter()
                .filter(|&&g| {
                    if self.scan_scope.is_some() {
                        !self.hidden_by_filter(g) && self.row_index(g).is_none()
                    } else {
                        self.collapsed.contains(&g)
                    }
                })
                .count(),
        )
    }

    /// Search the old display order, resolving only survivors still displayed
    /// after removal (a context-only ancestor can disappear with its child).
    pub fn after_removal(
        &self,
        removed: usize,
        remap: impl Fn(usize) -> Option<usize>,
    ) -> Option<usize> {
        let row = self.row_index(removed)?;
        (row + 1..self.row_count())
            .chain((0..row).rev())
            .find_map(|i| self.row_at(i)?.group().and_then(&remap))
    }
}

/// Remap removal candidates without registering undisplayed catalog sources.
pub fn removal_survivor(
    before: &Rows,
    after: &Rows,
    current: usize,
    previous: &crate::group_identity::GroupRegistry,
    registry: &crate::group_identity::GroupRegistry,
    standalone: bool,
) -> Option<usize> {
    before.after_removal(current, |old| {
        let ix = if old < DERIVED_BASE || old == super::NO_ENTRY {
            (!registry.index_excluded(old) && (old != super::NO_ENTRY || standalone)).then_some(old)
        } else {
            previous.id(old).and_then(|id| registry.index(&id))
        }?;
        after.row_index(ix).map(|_| ix)
    })
}

/// Transient interaction state follows durable identities across replacements.
#[derive(Clone)]
pub struct InteractionIds([Option<crate::group_identity::GroupId>; 3]);

impl InteractionIds {
    pub fn capture(
        indices: [Option<usize>; 3],
        mut identify: impl FnMut(usize) -> Option<crate::group_identity::GroupId>,
    ) -> Self {
        Self(indices.map(|ix| ix.and_then(&mut identify)))
    }

    pub fn resolve(
        self,
        mut index: impl FnMut(&crate::group_identity::GroupId) -> Option<usize>,
    ) -> [Option<usize>; 3] {
        self.0.map(|id| id.as_ref().and_then(&mut index))
    }
}

pub fn migrate_standalone(
    marks: &mut BTreeSet<usize>,
    focus: &mut Option<usize>,
    anchor: &mut Option<usize>,
    reveal: &mut Option<usize>,
    index: usize,
) {
    if marks.remove(&super::NO_ENTRY) {
        marks.insert(index);
    }
    for slot in [focus, anchor, reveal] {
        if *slot == Some(super::NO_ENTRY) {
            *slot = Some(index);
        }
    }
}

#[derive(Clone, Copy)]
pub enum Gesture {
    Current,
    Toggle,
    Range,
}

/// Return whether the endpoint should become current. Mark gestures only move
/// focus; a range keeps its anchor until that anchor leaves displayed order.
pub fn interact(
    rows: &Rows,
    focus: &mut Option<usize>,
    anchor: &mut Option<usize>,
    marks: &mut BTreeSet<usize>,
    endpoint: usize,
    gesture: Gesture,
) -> bool {
    *focus = Some(endpoint);
    match gesture {
        Gesture::Current => {
            *anchor = Some(endpoint);
            true
        }
        Gesture::Toggle => {
            *anchor = Some(endpoint);
            if !marks.remove(&endpoint) {
                marks.insert(endpoint);
            }
            false
        }
        Gesture::Range => {
            if anchor.is_none_or(|g| rows.row_index(g).is_none()) {
                *anchor = Some(endpoint);
            }
            marks.extend(rows.range(*anchor, endpoint));
            false
        }
    }
}

pub fn compare_set(current: Option<usize>, marks: &BTreeSet<usize>) -> BTreeSet<usize> {
    marks.iter().copied().chain(current).collect()
}

/// Catalog order, orphan stacks, then Results. A matching child can reveal
/// its primary without mutating saved expansion. Work scales with derived
/// groups, never with the number of catalog primaries.
pub fn build_rows(
    catalog: &Catalog,
    derived: &[DerivedSpectrum],
    expanded: &BTreeSet<usize>,
    filtered: Option<std::sync::Arc<Vec<usize>>>,
    query: &str,
    standalone: Option<&std::path::Path>,
) -> Rows {
    build_rows_revealing(
        catalog,
        derived,
        |g| expanded.contains(&g).then_some(true),
        filtered,
        query,
        standalone,
        &BTreeSet::new(),
    )
}

pub fn build_rows_revealing(
    catalog: &Catalog,
    derived: &[DerivedSpectrum],
    disclosure: impl Fn(usize) -> Option<bool>,
    filtered: Option<std::sync::Arc<Vec<usize>>>,
    query: &str,
    standalone: Option<&std::path::Path>,
    reveal: &BTreeSet<usize>,
) -> Rows {
    build_rows_active(
        catalog,
        derived,
        disclosure,
        filtered,
        query,
        standalone,
        reveal,
        &BTreeSet::new(),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn build_rows_active(
    catalog: &Catalog,
    derived: &[DerivedSpectrum],
    disclosure: impl Fn(usize) -> Option<bool>,
    filtered: Option<std::sync::Arc<Vec<usize>>>,
    query: &str,
    standalone: Option<&std::path::Path>,
    reveal: &BTreeSet<usize>,
    excluded: &BTreeSet<usize>,
) -> Rows {
    let filtered = filtered.map(|f| {
        if reveal.is_empty() {
            return f;
        }
        std::sync::Arc::new(
            f.iter()
                .copied()
                .chain(reveal.iter().copied().filter(|&g| g < catalog.len()))
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
        )
    });
    let query = query.to_ascii_lowercase();
    let base_len = filtered.as_ref().map_or(catalog.len(), |f| {
        f.partition_point(|&ix| ix < catalog.len())
    });
    let skipped: BTreeSet<_> = excluded
        .iter()
        .copied()
        .filter(|&g| {
            g < catalog.len()
                && filtered
                    .as_ref()
                    .is_none_or(|f| f.binary_search(&g).is_ok())
        })
        .collect();
    let base_len = base_len - skipped.len();
    let mut rows = Rows {
        excluded: skipped,
        scan_scope: None,
        catalog_len: catalog.len(),
        collapsed: BTreeSet::new(),
        context_only: BTreeSet::new(),
        filtered,
        base_len,
        base_start: 0,
        primaries: BTreeMap::new(),
        inserted: Vec::new(),
        inserted_groups: BTreeMap::new(),
    };
    let mut children: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    let mut orphan_sources = BTreeMap::new();
    let mut orphans = Vec::new();
    let mut results = Vec::new();
    if standalone.is_some() {
        orphans.push(super::NO_ENTRY);
    }
    for (i, d) in derived.iter().enumerate() {
        let group = DERIVED_BASE + i;
        if excluded.contains(&group) {
            continue;
        }
        if d.operation
            .as_ref()
            .is_some_and(|o| matches!(o.tool.as_str(), "Live acquisition" | "Live average"))
        {
            results.push(group);
        } else if let Some(path) = &d.source {
            let primary = if standalone == Some(path.as_path()) {
                super::NO_ENTRY
            } else {
                catalog
                    .find_by_canonical_path(path)
                    .filter(|g| !excluded.contains(g))
                    .unwrap_or_else(|| {
                        *orphan_sources.entry(path).or_insert_with(|| {
                            orphans.push(group);
                            group
                        })
                    })
            };
            if primary != group {
                children.entry(primary).or_default().push(group);
            }
        } else {
            results.push(group);
        }
    }
    let matches = |group: usize| {
        let d = &derived[group - DERIVED_BASE];
        reveal.contains(&group)
            || filter_match_lower(&d.label.to_ascii_lowercase(), &query)
            || filter_match_lower(
                &d.params
                    .as_ref()
                    .map(|p| p.import.mode.label())
                    .unwrap_or("")
                    .to_ascii_lowercase(),
                &query,
            )
    };
    let mut inserts: BTreeMap<usize, Vec<Row>> = BTreeMap::new();
    for group in children
        .keys()
        .copied()
        .filter(|&g| g < catalog.len())
        .chain(orphans)
    {
        let source_matches = if group == super::NO_ENTRY {
            reveal.contains(&group)
                || standalone.is_some_and(|path| {
                    filter_match_lower(&path.to_string_lossy().to_ascii_lowercase(), &query)
                })
        } else if group < catalog.len() {
            rows.filtered
                .as_ref()
                .is_none_or(|f| f.binary_search(&group).is_ok())
        } else {
            matches(group)
        };
        let members = children.get(&group).map(Vec::as_slice).unwrap_or_default();
        let matching: Vec<_> = members
            .iter()
            .copied()
            .filter(|&g| source_matches || matches(g))
            .collect();
        if !source_matches && matching.is_empty() {
            continue;
        }
        if !source_matches {
            rows.context_only.insert(group);
        }
        let expanded = disclosure(group).unwrap_or(!source_matches && !matching.is_empty())
            || members.iter().any(|g| reveal.contains(g));
        let primary = Row::Primary {
            group,
            expanded,
            extra_channels: members.len(),
        };
        let anchor = if group < catalog.len() {
            let base = rows
                .filtered
                .as_ref()
                .map_or(group, |f| f.partition_point(|&g| g < group))
                - rows.excluded.range(..group).count();
            if source_matches {
                rows.primaries.insert(group, primary);
                base + 1
            } else {
                inserts.entry(base).or_default().push(primary);
                base
            }
        } else {
            let anchor = group
                .checked_sub(DERIVED_BASE)
                .and_then(|i| derived.get(i))
                .and_then(|d| d.source.as_ref())
                .and_then(|p| catalog.find_by_canonical_path(p))
                .map(|g| {
                    rows.filtered
                        .as_ref()
                        .map_or(g, |f| f.partition_point(|&i| i < g))
                        - rows.excluded.range(..g).count()
                })
                .unwrap_or(base_len);
            inserts.entry(anchor).or_default().push(primary);
            anchor
        };
        if !expanded {
            rows.collapsed.extend(matching.iter().copied());
        }
        if expanded {
            inserts
                .entry(anchor)
                .or_default()
                .extend(matching.into_iter().map(|child| Row::Child {
                    group: child,
                    parent: group,
                }));
        }
    }
    let mut results = results.into_iter().filter(|&g| matches(g)).peekable();
    if results.peek().is_some() {
        let tail = inserts.entry(base_len).or_default();
        tail.push(Row::Header(Section::Results));
        tail.extend(results.map(|group| Row::Result { group }));
    }
    for (base, values) in inserts {
        for value in values {
            let row = base + rows.inserted.len();
            if let Some(group) = value.group() {
                rows.inserted_groups.insert(group, row);
            }
            rows.inserted.push((row, value));
        }
    }
    rows
}

/// Provenance is historical; missing inputs never resolve by a reused slot/path.
pub fn input_missing(
    group: &DerivedSpectrum,
    missing: impl Fn(&crate::group_identity::GroupId) -> bool,
) -> Option<String> {
    let labels: Vec<_> = group
        .operation
        .as_ref()?
        .inputs
        .iter()
        .filter(|i| i.group_id.as_ref().is_some_and(&missing))
        .map(|i| i.label.as_str())
        .collect();
    (!labels.is_empty()).then(|| format!("input missing: {}", labels.join(", ")))
}

/// Group colours remain stable even when traces share a swatch.
pub fn overlay_colors(defaults: &[usize]) -> Vec<usize> {
    defaults.to_vec()
}

/// Compact visible kind; operation names use the persisted tool vocabulary.
pub fn kind(mode: crate::params::DetectionMode, result: Option<&DerivedSpectrum>) -> &'static str {
    use crate::params::{DetectionMode, Quantity};
    if let Some(result) = result {
        return match result.operation.as_ref().map(|op| op.tool.as_str()) {
            Some("merge") => "Merge",
            Some("Difference spectrum") => "Diff",
            Some("Align to reference") => "Align",
            Some("Calibrate energy") => "Calib",
            Some("Deglitch") => "Deglitch",
            Some("Truncate") => "Trunc",
            Some("Rebin") => "Rebin",
            Some("Smooth") => "Smooth",
            _ if result.quantity == Quantity::NormalizedDifference => "Diff",
            _ => result.quantity.label(),
        };
    }
    match mode {
        DetectionMode::Transmission => "Trans",
        DetectionMode::Fluorescence => "Fluo",
        DetectionMode::Reference => "Ref",
        DetectionMode::MuColumn => "μ",
        DetectionMode::Auto => "",
    }
}

/// Height of every row in the virtualized group list, in logical pixels.
/// `uniform_list` measures one item and assumes every item matches, so the
/// two-line group rows and the "Results" header must share this value.
pub const ROW_HEIGHT: f32 = 38.;

/// Quantity text for a measured source group. Every import mode yields the
/// absorption coefficient μ(E) versus photon energy E; the prefix names how it
/// was formed (ln(I0/It), fluorescence/I0, ln(It/Ir) or a stored μ column).
pub fn measured_quantity(mode: crate::params::DetectionMode) -> &'static str {
    use crate::params::DetectionMode;
    match mode {
        DetectionMode::Transmission => "transmission μ(E)",
        DetectionMode::Fluorescence => "fluorescence μ(E)",
        DetectionMode::Reference => "reference μ(E)",
        DetectionMode::MuColumn | DetectionMode::Auto => "measured μ(E)",
    }
}

/// Second, muted line of a group row: "source · quantity · E₀".
///
/// `source` is the file stem or "derived"; `quantity` is the stored or measured
/// quantity; `e0_ev` is the absorption-edge energy E₀ in electronvolts, shown
/// with 0.1 eV resolution only when already known (a processed spectrum or a
/// fixed E₀ setting). Empty parts are omitted; nothing is computed here.
pub fn row_detail(source: &str, quantity: &str, e0_ev: Option<f64>) -> String {
    let e0 = e0_ev
        .filter(|e| e.is_finite())
        .map(|e| format!("{e:.1} eV"));
    [Some(source), Some(quantity), e0.as_deref()]
        .into_iter()
        .flatten()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" · ")
}

/// Deterministic eight-swatch default derived from durable group metadata.
pub fn color_index(id: &crate::group_identity::GroupId) -> usize {
    let text = serde_json::to_string(id).unwrap_or_default();
    text.bytes().fold(0usize, |hash, b| {
        hash.wrapping_mul(31).wrapping_add(b as usize)
    }) % 8
}

/// Current processing diagnostics are independent of the Problems history.
/// Tickets stop older completions from replacing a newer attempt's status.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct DiagnosticTicket {
    id: crate::group_identity::GroupId,
    fingerprint: u64,
    generation: u64,
}

#[derive(Default)]
pub(super) struct Diagnostics {
    generation: u64,
    current: BTreeMap<crate::group_identity::GroupId, (DiagnosticTicket, Vec<super::JobError>)>,
}

impl Diagnostics {
    pub fn begin(
        &mut self,
        id: crate::group_identity::GroupId,
        fingerprint: u64,
    ) -> DiagnosticTicket {
        self.generation += 1;
        let ticket = DiagnosticTicket {
            id: id.clone(),
            fingerprint,
            generation: self.generation,
        };
        self.current.insert(id, (ticket.clone(), Vec::new()));
        ticket
    }

    pub fn finish(&mut self, ticket: &DiagnosticTicket, problems: Vec<super::JobError>) {
        if let Some((current, values)) = self.current.get_mut(&ticket.id)
            && current == ticket
        {
            *values = problems;
        }
    }

    /// A current import preview refreshes parser warnings without erasing a
    /// processing error or superseding an in-flight attempt of the same state.
    pub fn set_warnings(
        &mut self,
        id: crate::group_identity::GroupId,
        fingerprint: u64,
        warnings: Vec<super::JobError>,
    ) {
        if self
            .current
            .get(&id)
            .is_none_or(|(ticket, _)| ticket.fingerprint != fingerprint)
        {
            self.begin(id.clone(), fingerprint);
        }
        if let Some((_, values)) = self.current.get_mut(&id) {
            values.retain(|p| p.severity == super::ProblemSeverity::Error);
            values.extend(warnings);
        }
    }

    pub fn get(&self, id: &crate::group_identity::GroupId, fingerprint: u64) -> &[super::JobError] {
        self.current
            .get(id)
            .filter(|(ticket, _)| ticket.fingerprint == fingerprint)
            .map(|(_, values)| values.as_slice())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn row_detail_separates_similar_references_and_omits_unknown_parts() {
        use crate::params::DetectionMode;
        let quantity = measured_quantity(DetectionMode::MuColumn);
        assert_eq!(
            row_detail("CuO", quantity, Some(8984.04)),
            "CuO · measured μ(E) · 8984.0 eV"
        );
        assert_eq!(row_detail("Cu", quantity, None), "Cu · measured μ(E)");
        assert_eq!(
            row_detail("derived", "μnorm", Some(f64::NAN)),
            "derived · μnorm"
        );
        assert_eq!(row_detail("", "", None), "");
        assert_eq!(
            measured_quantity(DetectionMode::Reference),
            "reference μ(E)"
        );
    }

    #[test]
    fn removal_survivor_does_not_register_hidden_catalog_candidates() {
        use crate::group_identity::GroupRegistry;
        use crate::params::DetectionMode;
        let registry = GroupRegistry::default();
        let removed = registry.register_source(
            Some(1),
            "/data/1.dat".into(),
            DetectionMode::Auto,
            &Default::default(),
        );
        let previous = registry.clone();
        registry.set_excluded(&BTreeSet::from([removed]));
        let before = Rows::catalog_range(100_000, 0, 100_000);
        let after = Rows::catalog_range(100_000, 0, 1);
        let survivor = removal_survivor(&before, &after, 1, &previous, &registry, false);
        assert_eq!(survivor, Some(0));
        assert_eq!(registry.sources().len(), 1);
        registry.register_source(
            survivor,
            "/data/0.dat".into(),
            DetectionMode::Auto,
            &Default::default(),
        );
        assert_eq!(registry.sources().len(), 2);
    }

    use super::*;
    use crate::{
        catalog::FileMeta,
        params::{DetectionMode, PipelineParams},
    };
    fn build_rows(
        catalog: &Catalog,
        derived: &[DerivedSpectrum],
        expanded: &BTreeSet<usize>,
        filtered: Option<&[usize]>,
        query: &str,
    ) -> Vec<Row> {
        let rows = super::build_rows(
            catalog,
            derived,
            expanded,
            filtered.map(|f| std::sync::Arc::new(f.to_vec())),
            query,
            None,
        );
        (0..rows.row_count())
            .filter_map(|i| rows.row_at(i))
            .collect()
    }
    fn catalog() -> Catalog {
        let mut c = Catalog::default();
        c.extend(
            ["a.dat", "b.dat"]
                .into_iter()
                .map(|name| FileMeta {
                    dir: "/data".into(),
                    name: name.into(),
                    size: 0,
                })
                .collect(),
        );
        c
    }
    fn group(source: Option<&str>, mode: DetectionMode) -> DerivedSpectrum {
        let mut params = PipelineParams::default();
        params.import.mode = mode;
        DerivedSpectrum {
            declared_edge: None,
            group_id: None,
            id: 0,
            label: "output".into(),
            energy: vec![],
            mu: vec![],
            source: source.map(Into::into),
            params: Some(params),
            quantity: Default::default(),
            quantity_unconfirmed: false,
            operation: None,
            ..Default::default()
        }
    }
    fn channels() -> Vec<DerivedSpectrum> {
        ["/data/a.dat", "/data/b.dat"]
            .into_iter()
            .flat_map(|p| {
                [
                    group(Some(p), DetectionMode::Fluorescence),
                    group(Some(p), DetectionMode::Reference),
                ]
            })
            .collect()
    }
    #[test]
    fn live_outputs_keep_named_result_rows_when_cache_is_in_catalog() {
        let c = catalog();
        let mut average = group(Some("/data/a.dat"), DetectionMode::MuColumn);
        average.label = "Transmission average · 2 scans".into();
        average.operation = Some(crate::params::Operation {
            tool: "Live average".into(),
            parameters: serde_json::json!({}),
            inputs: vec![],
            applied_energy_shift_ev: 0.,
        });
        let rows = build_rows_active(
            &c,
            &[average],
            |_| Some(false),
            None,
            "",
            None,
            &BTreeSet::new(),
            &BTreeSet::from([0]),
        );
        assert_eq!(rows.row_index(0), None);
        let index = rows.row_index(DERIVED_BASE).unwrap();
        assert_eq!(
            rows.row_at(index),
            Some(Row::Result {
                group: DERIVED_BASE
            })
        );
    }
    #[test]
    fn hidden_live_contributors_do_not_render_or_take_keyboard_rows() {
        let c = catalog();
        let d = vec![
            group(Some("/cache/scan.dat"), DetectionMode::MuColumn),
            group(Some("/cache/average.dat"), DetectionMode::MuColumn),
        ];
        let hidden = BTreeSet::from([DERIVED_BASE]);
        let rows = build_rows_active(&c, &d, |_| None, None, "", None, &BTreeSet::new(), &hidden);
        assert_eq!(rows.row_index(DERIVED_BASE), None);
        assert!(rows.row_index(DERIVED_BASE + 1).is_some());
        assert!(
            !(0..rows.row_count())
                .any(|i| rows.row_at(i).and_then(|r| r.group()) == Some(DERIVED_BASE))
        );
        let revealed = build_rows_active(
            &c,
            &d,
            |_| None,
            None,
            "",
            None,
            &BTreeSet::new(),
            &BTreeSet::new(),
        );
        assert!(revealed.row_index(DERIVED_BASE).is_some());
    }
    #[test]
    fn removal_rows_remain_sparse_filtered_and_promote_in_source_order() {
        let c = catalog();
        let d = channels();
        for excluded in [
            BTreeSet::from([0]),
            BTreeSet::from([1]),
            BTreeSet::from([0, 1]),
        ] {
            for filtered in [
                None,
                Some(std::sync::Arc::new(vec![0])),
                Some(std::sync::Arc::new(vec![1])),
                Some(std::sync::Arc::new(vec![])),
            ] {
                for query in ["", "reference", "absent"] {
                    for expand in [false, true] {
                        let rows = build_rows_active(
                            &c,
                            &d,
                            |_| Some(expand),
                            filtered.clone(),
                            query,
                            None,
                            &BTreeSet::new(),
                            &excluded,
                        );
                        for (i, g) in (0..rows.row_count())
                            .filter_map(|i| rows.row_at(i)?.group().map(|g| (i, g)))
                        {
                            assert!(!excluded.contains(&g));
                            assert_eq!(rows.row_index(g), Some(i));
                        }
                        assert_eq!(rows.row_at(rows.row_count()), None);
                        if let Some(f) = &filtered {
                            assert!(std::sync::Arc::ptr_eq(rows.filtered.as_ref().unwrap(), f));
                        }
                    }
                }
            }
        }
        let before = super::build_rows(&c, &d, &BTreeSet::from([0, 1]), None, "", None);
        let after = build_rows_active(
            &c,
            &d,
            |_| Some(false),
            None,
            "",
            None,
            &BTreeSet::new(),
            &BTreeSet::from([0]),
        );
        assert_eq!(
            before.after_removal(0, |g| after.row_index(g).map(|_| g)),
            Some(DERIVED_BASE)
        );
        assert_eq!(
            after.row_at(0),
            Some(Row::Primary {
                group: DERIVED_BASE,
                expanded: false,
                extra_channels: 1
            })
        );
        assert_eq!(
            after.mark_counts(&BTreeSet::from([DERIVED_BASE, DERIVED_BASE + 1])),
            (2, 0, 1)
        );
        let filtered = build_rows_active(
            &c,
            &[],
            |_| None,
            Some(std::sync::Arc::new(vec![0])),
            "a.dat",
            None,
            &BTreeSet::new(),
            &BTreeSet::from([0]),
        );
        assert_eq!(
            before.after_removal(0, |g| filtered.row_index(g).map(|_| g)),
            None
        );
    }

    #[test]
    fn scan_filter_drives_render_navigation_space_marks_counts_and_scroll() {
        let rows = super::build_rows(
            &catalog(),
            &[],
            &BTreeSet::new(),
            Some(std::sync::Arc::new(vec![1])),
            "b.dat",
            None,
        )
        .in_catalog_range(0, 2);
        assert_eq!(rows.shown().collect::<Vec<_>>(), vec![1]);
        assert_eq!(rows.neighbor(None, 1), Some(1));
        assert_eq!(rows.row_index(1), Some(0)); // Space guard
        assert_eq!(rows.scroll_row(1, Some(3)), Some(4));
        assert_eq!(rows.row_index(0), None);
        let mut marks = BTreeSet::new();
        rows.mark_shown(&mut marks);
        assert_eq!(marks, BTreeSet::from([1]));
        marks.insert(0);
        assert_eq!(rows.mark_counts(&marks), (2, 1, 0));
        rows.invert_shown(&mut marks);
        assert_eq!(marks, BTreeSet::from([0]));
        let collapsed = super::build_rows(
            &catalog(),
            &[],
            &BTreeSet::new(),
            Some(std::sync::Arc::new(vec![1])),
            "b.dat",
            None,
        )
        .in_catalog_range(0, 0);
        assert_eq!(collapsed.mark_counts(&BTreeSet::from([0, 1])), (2, 1, 1));
        let files = super::build_rows(&catalog(), &[], &BTreeSet::new(), None, "", None);
        assert_eq!(files.scroll_row(1, Some(3)), Some(1));
        let second_run = super::build_rows(
            &catalog(),
            &[],
            &BTreeSet::new(),
            Some(std::sync::Arc::new(vec![0, 1])),
            "",
            None,
        )
        .in_catalog_range(1, 1);
        assert_eq!(second_run.row_index(1), Some(0));
        assert_eq!(second_run.row_index(0), None);
        assert_eq!(second_run.shown().collect::<Vec<_>>(), vec![1]);
    }

    #[test]
    fn standalone_mark_focus_anchor_reveal_migrate_by_identity_without_aliases() {
        use super::super::NO_ENTRY;
        use crate::group_identity::GroupRegistry;
        let registry = GroupRegistry::default();
        let id = registry.register_source(
            None,
            "/data/a.dat".into(),
            DetectionMode::Auto,
            &BTreeMap::new(),
        );
        registry.append_catalog(&catalog(), 0);
        let ix = registry.index(&id).unwrap();
        let mut marks = BTreeSet::from([NO_ENTRY, ix]);
        let (mut focus, mut anchor, mut reveal) = (Some(NO_ENTRY), Some(NO_ENTRY), Some(NO_ENTRY));
        migrate_standalone(&mut marks, &mut focus, &mut anchor, &mut reveal, ix);
        assert_eq!((focus, anchor, reveal), (Some(ix), Some(ix), Some(ix)));
        assert_eq!(marks, BTreeSet::from([ix]));
        assert_eq!(compare_set(Some(ix), &marks), marks);
        let mut unmarked = BTreeSet::new();
        migrate_standalone(&mut unmarked, &mut focus, &mut anchor, &mut reveal, ix);
        assert!(unmarked.is_empty());
    }

    #[test]
    fn catalog_refresh_rekeys_focus_anchor_and_reveal_before_space_and_range() {
        use crate::group_identity::GroupRegistry;
        let registry = GroupRegistry::default();
        for (ix, name) in ["a.dat", "b.dat"].into_iter().enumerate() {
            registry.register_source(
                Some(ix),
                format!("/data/{name}").into(),
                DetectionMode::Auto,
                &BTreeMap::new(),
            );
        }
        let saved = InteractionIds::capture([Some(1), Some(0), Some(1)], |ix| registry.id(ix));
        let mut refreshed = Catalog::default();
        refreshed.extend(
            ["inserted.dat", "a.dat", "b.dat"]
                .into_iter()
                .map(|name| FileMeta {
                    dir: "/data".into(),
                    name: name.into(),
                    size: 0,
                })
                .collect(),
        );
        registry.replace_catalog(GroupRegistry::prepare_catalog(
            &refreshed,
            &registry.sources(),
        ));
        let [mut focus, mut anchor, reveal] = saved.resolve(|id| registry.index(id));
        assert_eq!((focus, anchor, reveal), (Some(2), Some(1), Some(2)));
        let rows = super::build_rows(&refreshed, &[], &BTreeSet::new(), None, "", None);
        assert_eq!(rows.range(anchor, focus.unwrap()), vec![1, 2]);
        let mut marks = BTreeSet::new();
        let endpoint = focus.unwrap();
        interact(
            &rows,
            &mut focus,
            &mut anchor,
            &mut marks,
            endpoint,
            Gesture::Toggle,
        );
        assert_eq!(marks, BTreeSet::from([2]));
        let saved = InteractionIds::capture([Some(2); 3], |ix| registry.id(ix));
        registry.replace_catalog(Default::default());
        assert_eq!(saved.resolve(|id| registry.index(id)), [None; 3]);
    }

    #[test]
    fn undo_filtered_out_current_result_chooses_displayed_source_survivor() {
        let derived = vec![group(None, DetectionMode::Auto)];
        let order = super::build_rows_revealing(
            &catalog(),
            &derived,
            |_| Some(true),
            None,
            "",
            None,
            &BTreeSet::new(),
        );
        let displayed_before = super::build_rows(
            &catalog(),
            &derived,
            &BTreeSet::new(),
            Some(std::sync::Arc::new(vec![0, 1])),
            "dat",
            None,
        );
        assert_eq!(displayed_before.row_index(DERIVED_BASE), None);
        let after = super::build_rows(
            &catalog(),
            &[],
            &BTreeSet::new(),
            Some(std::sync::Arc::new(vec![0, 1])),
            "dat",
            None,
        );
        assert_eq!(
            order.after_removal(DERIVED_BASE, |ix| after.row_index(ix).map(|_| ix)),
            Some(1)
        );
    }

    #[test]
    fn navigation_and_range_keep_current_marks_and_anchor_independent() {
        let mut d = channels();
        d.push(group(None, DetectionMode::Auto));
        let rows = super::build_rows(&catalog(), &d, &BTreeSet::from([0, 1]), None, "", None);
        let (mut focus, mut anchor, mut current) = (None, None, None);
        let mut marks = BTreeSet::from([1]);
        let mut act = |endpoint, gesture| {
            if interact(
                &rows,
                &mut focus,
                &mut anchor,
                &mut marks,
                endpoint,
                gesture,
            ) {
                current = Some(endpoint);
            }
            (focus, anchor, current, marks.clone())
        };
        assert_eq!(
            act(0, Gesture::Current),
            (Some(0), Some(0), Some(0), BTreeSet::from([1]))
        );
        assert_eq!(
            act(DERIVED_BASE, Gesture::Toggle),
            (
                Some(DERIVED_BASE),
                Some(DERIVED_BASE),
                Some(0),
                BTreeSet::from([1, DERIVED_BASE])
            )
        );
        let (_, a, c, m) = act(DERIVED_BASE + 4, Gesture::Range);
        assert_eq!(a, Some(DERIVED_BASE));
        assert_eq!(c, Some(0));
        assert_eq!(
            m,
            BTreeSet::from([
                1,
                DERIVED_BASE,
                DERIVED_BASE + 1,
                DERIVED_BASE + 2,
                DERIVED_BASE + 3,
                DERIVED_BASE + 4
            ])
        );
        assert_eq!(act(1, Gesture::Range).3, m, "shrinking a range is additive");
        assert_eq!(rows.neighbor(Some(0), 1), Some(DERIVED_BASE));
        assert_eq!(
            rows.neighbor(Some(DERIVED_BASE + 3), 1),
            Some(DERIVED_BASE + 4),
            "skip Results header"
        );
    }

    #[test]
    fn explicit_collapse_overrides_filter_expansion_and_reveal_is_temporary() {
        let c = catalog();
        let d = channels();
        let filtered = Some(std::sync::Arc::new(vec![]));
        let collapsed = super::build_rows_revealing(
            &c,
            &d,
            |_| Some(false),
            filtered.clone(),
            "reference",
            None,
            &BTreeSet::new(),
        );
        let marks = BTreeSet::from([DERIVED_BASE + 1]);
        assert_eq!(collapsed.mark_counts(&marks), (1, 0, 1));
        let revealed = super::build_rows_revealing(
            &c,
            &d,
            |_| Some(false),
            filtered,
            "reference",
            None,
            &marks,
        );
        assert_eq!(revealed.mark_counts(&marks), (1, 0, 0));
        let mut shown_marks = BTreeSet::new();
        revealed.mark_shown(&mut shown_marks);
        assert_eq!(
            shown_marks, marks,
            "context ancestors are not filter matches"
        );
        revealed.invert_shown(&mut shown_marks);
        assert!(shown_marks.is_empty());
        assert_eq!(collapsed.mark_counts(&marks), (1, 0, 1));
        let run = Rows::catalog_range(100, 40, 5);
        assert_eq!(run.shown().collect::<Vec<_>>(), vec![40, 41, 42, 43, 44]);
        assert_eq!(run.row_index(39), None);
        assert_eq!(run.row_index(45), None);
    }

    #[test]
    fn hidden_or_removed_anchor_restarts_at_endpoint() {
        let rows = super::build_rows(
            &catalog(),
            &channels(),
            &BTreeSet::new(),
            Some(std::sync::Arc::new(vec![1])),
            "b.dat",
            None,
        );
        for hidden in [0, DERIVED_BASE, DERIVED_BASE + 99] {
            let (mut focus, mut anchor) = (Some(hidden), Some(hidden));
            let mut marks = BTreeSet::from([hidden]);
            assert!(!interact(
                &rows,
                &mut focus,
                &mut anchor,
                &mut marks,
                1,
                Gesture::Range
            ));
            assert_eq!(anchor, Some(1));
            assert_eq!(marks, BTreeSet::from([hidden, 1]));
            assert_eq!(rows.neighbor(Some(hidden), 1), Some(1));
        }
    }

    #[test]
    fn shown_commands_include_offscreen_rows_but_preserve_hidden_marks() {
        let mut c = catalog();
        c.extend(
            (0..1000)
                .map(|i| FileMeta {
                    dir: "/data".into(),
                    name: format!("scan{i}.dat").into(),
                    size: 0,
                })
                .collect(),
        );
        let rows = super::build_rows(&c, &channels(), &BTreeSet::new(), None, "", None);
        let mut marks = BTreeSet::from([DERIVED_BASE]);
        rows.mark_shown(&mut marks);
        assert_eq!(marks.len(), c.len() + 1);
        assert!(!marks.contains(&(DERIVED_BASE + 1)));
        rows.invert_shown(&mut marks);
        assert_eq!(marks, BTreeSet::from([DERIVED_BASE]));
        marks.clear();
        assert!(marks.is_empty());
    }

    #[test]
    fn acceptance_navigation_filter_collapse_and_merge_scope_agree() {
        let c = catalog();
        let d = channels();
        let expanded = BTreeSet::from([0, 1]);
        let rows = super::build_rows(&c, &d, &expanded, None, "", None);
        let mut marks = BTreeSet::from([0, 1, DERIVED_BASE]);
        let (mut focus, mut anchor, mut current) = (Some(0), Some(0), Some(0));
        for _ in 0..3 {
            let next = rows.neighbor(focus, 1).unwrap();
            if interact(
                &rows,
                &mut focus,
                &mut anchor,
                &mut marks,
                next,
                Gesture::Current,
            ) {
                current = Some(next);
            }
        }
        assert_eq!(marks, BTreeSet::from([0, 1, DERIVED_BASE]));
        let filtered = Some(std::sync::Arc::new(vec![0]));
        let collapsed =
            super::build_rows(&c, &d, &BTreeSet::new(), filtered.clone(), "a.dat", None);
        assert_eq!(collapsed.mark_counts(&marks), (3, 1, 1));
        assert_eq!(compare_set(current, &marks), marks);
        let revealed =
            super::build_rows_revealing(&c, &d, |_| None, filtered.clone(), "a.dat", None, &marks);
        assert!(marks.iter().all(|&g| revealed.row_index(g).is_some()));
        let restored = super::build_rows(&c, &d, &BTreeSet::new(), filtered, "a.dat", None);
        assert_eq!(restored.mark_counts(&marks), (3, 1, 1));
        marks.retain(|&g| !restored.hidden_by_filter(g));
        assert_eq!(
            marks,
            BTreeSet::from([0, DERIVED_BASE]),
            "clear hidden retains collapsed marks"
        );
        assert_eq!(
            compare_set(current, &marks),
            BTreeSet::from([0, 1, DERIVED_BASE]),
            "current is deduplicated and never silently dropped"
        );
    }

    #[test]
    fn removal_prefers_next_then_previous_and_never_hidden_groups() {
        let mut d = channels();
        d.push(group(None, DetectionMode::Auto));
        let rows = super::build_rows(&catalog(), &d, &BTreeSet::from([0, 1]), None, "", None);
        assert_eq!(
            rows.after_removal(DERIVED_BASE + 3, Some),
            Some(DERIVED_BASE + 4)
        );
        assert_eq!(
            rows.after_removal(DERIVED_BASE + 4, Some),
            Some(DERIVED_BASE + 3)
        );
        let filtered = super::build_rows(
            &catalog(),
            &d,
            &BTreeSet::new(),
            Some(std::sync::Arc::new(vec![1])),
            "b.dat",
            None,
        );
        assert_eq!(filtered.after_removal(1, Some), None);
        assert_eq!(filtered.after_removal(0, Some), None);
        let before = super::build_rows(
            &catalog(),
            &[group(Some("/data/a.dat"), DetectionMode::Reference)],
            &BTreeSet::new(),
            Some(std::sync::Arc::new(vec![])),
            "reference",
            None,
        );
        let after = super::build_rows(
            &catalog(),
            &[],
            &BTreeSet::new(),
            Some(std::sync::Arc::new(vec![])),
            "reference",
            None,
        );
        assert_eq!(
            before.after_removal(DERIVED_BASE, |g| after.row_index(g).map(|_| g)),
            None,
            "removing the only match also hides its context ancestor"
        );
    }

    #[test]
    fn kinds_distinguish_channels_and_persisted_tool_outputs() {
        use crate::params::{Operation, Quantity};
        assert_eq!(kind(DetectionMode::Auto, None), "");
        assert_eq!(kind(DetectionMode::Transmission, None), "Trans");
        assert_eq!(kind(DetectionMode::Fluorescence, None), "Fluo");
        assert_eq!(kind(DetectionMode::Reference, None), "Ref");
        assert_eq!(kind(DetectionMode::MuColumn, None), "μ");
        let mut result = group(None, DetectionMode::Auto);
        result.quantity = Quantity::NormalizedDifference;
        assert_eq!(kind(DetectionMode::Auto, Some(&result)), "Diff");
        for (tool, expected) in [
            ("merge", "Merge"),
            ("Difference spectrum", "Diff"),
            ("Align to reference", "Align"),
            ("Calibrate energy", "Calib"),
        ] {
            result.operation = Some(Operation {
                tool: tool.into(),
                parameters: serde_json::Value::Null,
                inputs: vec![],
                applied_energy_shift_ev: 0.,
            });
            assert_eq!(kind(DetectionMode::Auto, Some(&result)), expected);
        }
    }

    #[test]
    fn colour_survives_identity_serialization_and_display_reordering() {
        let id = crate::group_identity::GroupId::source(
            std::path::Path::new("/data/a.dat"),
            DetectionMode::Reference,
        );
        let restored = serde_json::from_str(&serde_json::to_string(&id).unwrap()).unwrap();
        assert_eq!(color_index(&id), color_index(&restored));
        assert!(color_index(&id) < 8);
    }
    #[test]
    fn two_source_stacks_and_more_than_six_results_share_one_order() {
        let mut d = channels();
        d.extend((0..9).map(|_| group(None, DetectionMode::Auto)));
        let rows = build_rows(&catalog(), &d, &BTreeSet::from([0, 1]), None, "");
        assert_eq!(rows.len(), 16);
        assert_eq!(
            rows[1],
            Row::Child {
                group: DERIVED_BASE,
                parent: 0
            }
        );
        assert_eq!(
            rows[5],
            Row::Child {
                group: DERIVED_BASE + 3,
                parent: 1
            }
        );
        assert_eq!(rows[6], Row::Header(Section::Results));
        assert_eq!(
            rows[15],
            Row::Result {
                group: DERIVED_BASE + 12
            }
        );
    }
    #[test]
    fn collapse_counts_children_and_does_not_emit_them() {
        let rows = build_rows(&catalog(), &channels(), &BTreeSet::new(), None, "");
        assert_eq!(
            rows,
            vec![
                Row::Primary {
                    group: 0,
                    expanded: false,
                    extra_channels: 2
                },
                Row::Primary {
                    group: 1,
                    expanded: false,
                    extra_channels: 2
                }
            ]
        );
    }
    #[test]
    fn filtering_reveals_matching_children_and_ignores_stale_indices() {
        let rows = build_rows(
            &catalog(),
            &channels(),
            &BTreeSet::new(),
            Some(&[999]),
            "reference",
        );
        assert_eq!(rows.len(), 4);
        assert_eq!(
            rows[1],
            Row::Child {
                group: DERIVED_BASE + 1,
                parent: 0
            }
        );
        assert!(
            build_rows(
                &catalog(),
                &channels(),
                &BTreeSet::new(),
                Some(&[]),
                "absent"
            )
            .is_empty()
        );
        let rows = build_rows(
            &catalog(),
            &channels(),
            &BTreeSet::from([0]),
            Some(&[0]),
            "a.dat",
        );
        assert_eq!(rows.len(), 3);
    }
    #[test]
    fn orphan_channels_form_a_source_stack_not_results() {
        let d = channels();
        let rows = build_rows(
            &Catalog::default(),
            &d,
            &BTreeSet::from([DERIVED_BASE]),
            None,
            "",
        );
        assert_eq!(rows.len(), 3);
        assert_eq!(
            rows[1],
            Row::Child {
                group: DERIVED_BASE + 1,
                parent: DERIVED_BASE
            }
        );
    }
    #[test]
    fn navigation_and_ranges_follow_display_order_and_skip_headers() {
        let rows = build_rows(
            &catalog(),
            &[group(None, DetectionMode::Auto)],
            &BTreeSet::new(),
            None,
            "",
        );
        assert_eq!(rows.len(), 4);
        let rows = super::build_rows(
            &catalog(),
            &[group(None, DetectionMode::Auto)],
            &BTreeSet::new(),
            None,
            "",
            None,
        );
        assert_eq!(rows.neighbor(Some(1), 1), Some(DERIVED_BASE));
        assert_eq!(rows.neighbor(Some(DERIVED_BASE), -1), Some(1));
        assert_eq!(rows.neighbor(Some(0), -1), None);
        assert_eq!(rows.range(Some(0), DERIVED_BASE), &[0, 1, DERIVED_BASE]);
        assert_eq!(rows.range(Some(999), 1), &[1]);
    }
    #[test]
    fn million_catalog_primaries_remain_virtual_and_filters_are_shared() {
        use std::sync::Arc;
        let mut catalog = Catalog::default();
        catalog.extend(
            (0..1_000_000)
                .map(|i| FileMeta {
                    dir: Arc::from("/scan"),
                    name: format!("{i:07}.dat").into_boxed_str(),
                    size: 1,
                })
                .collect(),
        );
        let derived = [
            group(Some("/scan/0000000.dat"), DetectionMode::Reference),
            group(Some("/scan/0999998.dat"), DetectionMode::Reference),
            group(None, DetectionMode::Auto),
        ];
        let expanded = BTreeSet::from([0, 999_998]);
        let rows = super::build_rows(&catalog, &derived, &expanded, None, "", None);
        assert_eq!(rows.row_count(), 1_000_004);
        assert_eq!(rows.primaries.len(), 2);
        assert_eq!(rows.inserted.len(), 4);
        assert_eq!(rows.row_index(999_999), Some(1_000_001));
        assert_eq!(rows.row_at(1_000_001).and_then(Row::group), Some(999_999));
        assert_eq!(rows.neighbor(Some(999_999), 1), Some(DERIVED_BASE + 2));
        assert_eq!(
            rows.range(Some(999_997), 999_999),
            vec![999_997, 999_998, DERIVED_BASE + 1, 999_999]
        );
        assert_eq!(rows.row_at(rows.row_count()), None);
        let filtered = Arc::new((0..1_000_000).step_by(2).collect::<Vec<_>>());
        let rows = super::build_rows(
            &catalog,
            &derived,
            &expanded,
            Some(filtered.clone()),
            "",
            None,
        );
        assert!(Arc::ptr_eq(rows.filtered.as_ref().unwrap(), &filtered));
        assert_eq!(rows.row_count(), 500_004);
        assert_eq!(rows.inserted.len(), 4);
        assert_eq!(rows.row_index(999_999), None);
        assert_eq!(rows.row_index(999_998), Some(500_000));
        assert_eq!(rows.neighbor(Some(999_998), 1), Some(DERIVED_BASE + 1));
    }

    #[test]
    fn sparse_mapping_round_trips_filters_stacks_orphans_and_results() {
        let mut derived = channels();
        derived.push(group(Some("/elsewhere/a.dat"), DetectionMode::Transmission));
        derived.push(group(Some("/elsewhere/a.dat"), DetectionMode::Reference));
        derived.push(group(None, DetectionMode::Auto));
        for filter in [
            None,
            Some(vec![]),
            Some(vec![0]),
            Some(vec![1]),
            Some(vec![0, 1, 999]),
        ] {
            for query in ["", "reference", "absent"] {
                for expanded in [BTreeSet::new(), BTreeSet::from([0, 1, DERIVED_BASE + 4])] {
                    let rows = super::build_rows(
                        &catalog(),
                        &derived,
                        &expanded,
                        filter.clone().map(std::sync::Arc::new),
                        query,
                        None,
                    );
                    let groups = (0..rows.row_count())
                        .filter_map(|i| {
                            let row = rows.row_at(i).expect("every visible row maps");
                            row.group().inspect(|&g| {
                                assert_eq!(rows.row_index(g), Some(i));
                            })
                        })
                        .collect::<Vec<_>>();
                    for (i, &g) in groups.iter().enumerate() {
                        assert_eq!(rows.neighbor(Some(g), 1), groups.get(i + 1).copied());
                        assert_eq!(
                            rows.neighbor(Some(g), -1),
                            i.checked_sub(1).map(|i| groups[i])
                        );
                    }
                    if let Some(&last) = groups.last() {
                        assert_eq!(rows.range(groups.first().copied(), last), groups);
                    }
                }
            }
        }
    }

    #[test]
    fn standalone_primary_survives_results_and_owns_its_channels() {
        use super::super::NO_ENTRY;
        let mut derived = vec![group(Some("/solo/scan.dat"), DetectionMode::Reference)];
        derived.extend((0..9).map(|_| group(None, DetectionMode::Auto)));
        let catalog = Catalog::default();
        let path = std::path::Path::new("/solo/scan.dat");
        let rows = super::build_rows(
            &catalog,
            &derived,
            &BTreeSet::from([NO_ENTRY]),
            None,
            "",
            Some(path),
        );
        assert_eq!(rows.row_count(), 12);
        assert_eq!(
            rows.row_at(0),
            Some(Row::Primary {
                group: NO_ENTRY,
                expanded: true,
                extra_channels: 1
            })
        );
        assert_eq!(
            rows.row_at(1),
            Some(Row::Child {
                group: DERIVED_BASE,
                parent: NO_ENTRY
            })
        );
        assert_eq!(rows.row_at(2), Some(Row::Header(Section::Results)));
        assert_eq!(rows.row_index(NO_ENTRY), Some(0));
        assert_eq!(rows.neighbor(Some(DERIVED_BASE), -1), Some(NO_ENTRY));
        assert_eq!(rows.neighbor(Some(NO_ENTRY), 1), Some(DERIVED_BASE));
        let rows = super::build_rows(&catalog, &derived, &BTreeSet::new(), None, "", Some(path));
        assert_eq!(rows.row_count(), 11);
        assert_eq!(rows.neighbor(Some(DERIVED_BASE + 1), -1), Some(NO_ENTRY));
        let rows = super::build_rows(
            &catalog,
            &derived,
            &BTreeSet::new(),
            None,
            "reference",
            Some(path),
        );
        assert_eq!(rows.row_count(), 2);
        assert_eq!(
            rows.row_at(1),
            Some(Row::Child {
                group: DERIVED_BASE,
                parent: NO_ENTRY
            })
        );
        let rows = super::build_rows(
            &catalog,
            &derived,
            &BTreeSet::new(),
            None,
            "absent",
            Some(path),
        );
        assert_eq!(
            rows.row_count(),
            0,
            "a filter may hide current without changing it"
        );
        assert_eq!(rows.row_index(NO_ENTRY), None);
    }

    #[test]
    fn overlay_collisions_preserve_group_colors() {
        assert_eq!(overlay_colors(&[3, 6, 1]), vec![3, 6, 1]);
        assert_eq!(overlay_colors(&[3, 3]), vec![3, 3]);
        assert_eq!(overlay_colors(&[7; 8]), vec![7; 8]);
        assert_eq!(overlay_colors(&[7; 9]), vec![7; 9]);
        assert!(overlay_colors(&[]).is_empty());
    }

    #[test]
    fn diagnostics_use_identity_fingerprint_and_attempt_not_historical_labels() {
        use super::super::{JobError, ProblemSeverity, push_problem};
        use crate::group_identity::GroupId;
        let first = GroupId::source(std::path::Path::new("/a/scan.dat"), DetectionMode::Auto);
        let second = GroupId::source(std::path::Path::new("/b/scan.dat"), DetectionMode::Auto);
        let channel = GroupId::source(
            std::path::Path::new("/a/scan.dat"),
            DetectionMode::Reference,
        );
        let error = JobError {
            batch: None,
            severity: ProblemSeverity::Error,
            label: "scan.dat".into(),
            message: "failed".into(),
        };
        let mut history = Vec::new();
        push_problem(&mut history, error.clone());
        let mut diagnostics = Diagnostics::default();
        let failed = diagnostics.begin(first.clone(), 10);
        diagnostics.finish(&failed, vec![error.clone()]);
        assert_eq!(diagnostics.get(&first, 10), std::slice::from_ref(&error));
        let parser_warning =
            JobError::warning(std::path::Path::new("/a/scan.dat"), "parser warning".into());
        diagnostics.set_warnings(first.clone(), 10, vec![parser_warning.clone()]);
        assert_eq!(
            diagnostics.get(&first, 10),
            &[error.clone(), parser_warning]
        );
        diagnostics.set_warnings(first.clone(), 10, vec![]);
        assert_eq!(diagnostics.get(&first, 10), std::slice::from_ref(&error));
        assert!(diagnostics.get(&second, 10).is_empty());
        assert!(diagnostics.get(&channel, 10).is_empty());
        assert!(diagnostics.get(&first, 11).is_empty());
        let success = diagnostics.begin(first.clone(), 11);
        diagnostics.finish(&success, vec![]);
        diagnostics.finish(&failed, vec![error.clone()]);
        assert!(diagnostics.get(&first, 11).is_empty());
        let older = diagnostics.begin(first.clone(), 11);
        let newer = diagnostics.begin(first.clone(), 11);
        let warning =
            JobError::warning(std::path::Path::new("/a/scan.dat"), "parser warning".into());
        diagnostics.finish(&newer, vec![warning.clone()]);
        diagnostics.finish(&older, vec![error.clone()]);
        assert_eq!(diagnostics.get(&first, 11), &[warning]);
        let retry = diagnostics.begin(first.clone(), 11);
        diagnostics.finish(&retry, vec![]);
        assert!(diagnostics.get(&first, 11).is_empty());
        assert_eq!(
            history,
            vec![error],
            "Problems history survives successful processing"
        );
    }
}
