//! Resolve saved composer samples against the workspace without changing marks.
use super::*;
use crate::group_identity::GroupId;
use crate::params::Quantity;
use crate::publication::composed::{ComposerInput, SampleInput};
use crate::publication::figures::{
    composer::{CompositeRecipe, CompositeSample},
    quantity_figures,
};
use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};

/// Reuse generated arrays until the processed spectrum, label or quantity changes.
struct CachedSample {
    spectrum: Arc<rexafs::prelude::XASSpectrum>,
    fingerprint: u64,
    label: String,
    quantity: Quantity,
    figures: Vec<Arc<FigureData>>,
}

/// Session caches; project settings contain only durable sample identifiers.
#[derive(Default)]
pub(super) struct ComposerSamplesState {
    figures: BTreeMap<GroupId, CachedSample>,
    /// One attempt per processing revision. Failed sources
    /// wait for a settings change or a successful manual reload, not a redraw.
    attempted: BTreeSet<(GroupId, u64)>,
    /// Only interrupted requests become eligible again when the queue changes.
    pending: Option<(u64, Vec<(GroupId, u64)>)>,
}

impl ComposerSamplesState {
    /// Retain completed attempts independently of later batches, while allowing
    /// requests discarded by the shared queue's generation guard to run again.
    fn settle_requests(&mut self, generation: u64, running: bool) {
        let Some((pending_generation, _)) = &self.pending else {
            return;
        };
        if *pending_generation != generation {
            for key in self.pending.take().unwrap().1 {
                self.attempted.remove(&key);
            }
        } else if !running {
            self.pending = None;
        }
    }
}

/// Sort only marked groups through the sparse workspace row lookup.
fn composer_sample_indices(
    current: Option<usize>,
    marks: &BTreeSet<usize>,
    rows: &crate::app::group_rows::Rows,
) -> Vec<usize> {
    let mut marked: Vec<_> = marks
        .iter()
        .copied()
        .filter(|ix| Some(*ix) != current)
        .filter_map(|ix| rows.row_index(ix).map(|row| (row, ix)))
        .collect();
    marked.sort_unstable();
    current
        .into_iter()
        .chain(marked.into_iter().map(|(_, ix)| ix))
        .collect()
}

/// Unique sample dependencies of the requested recipes. Panel overrides replace
/// the shared selection, and fit panels always use the current fitted dataset.
/// The implicit current sample is included only when a recipe gives it a
/// figure label; otherwise empty selections keep the ordinary current path.
fn recipe_sample_ids<'a>(
    recipes: impl IntoIterator<Item = &'a CompositeRecipe>,
    current: Option<GroupId>,
) -> BTreeSet<GroupId> {
    let mut ids = BTreeSet::new();
    for recipe in recipes {
        let selections = recipe
            .panels
            .iter()
            .filter(|panel| !panel.uses_current_fit())
            .map(|panel| panel.samples.as_ref().unwrap_or(&recipe.samples));
        let mut uses_current = false;
        for selection in selections {
            uses_current |= selection.is_empty();
            ids.extend(selection.iter().cloned());
        }
        if uses_current
            && let Some(current) = &current
            && recipe
                .figure_labels
                .get(current)
                .is_some_and(|label| !label.trim().is_empty())
        {
            ids.insert(current.clone());
        }
    }
    ids
}

/// Identify metadata separately from the stable figure array pointers.
fn sample_metadata_revision(sample: &CompositeSample) -> usize {
    let mut metadata = std::collections::hash_map::DefaultHasher::new();
    (&sample.id, &sample.label, &sample.color, sample.is_current).hash(&mut metadata);
    metadata.finish() as usize
}

impl StudioApp {
    /// Capture every recipe's workspace sources before export. Resolving IDs here
    /// preserves unmarked samples and sources outside the saved project's scope;
    /// processing missing cache entries runs later on the export worker.
    pub(super) fn composer_export_input(&self) -> ComposerInput {
        let current = self.current_group_index();
        let samples = self
            .composer_export_sample_ids()
            .into_iter()
            .filter_map(|id| {
                let ix = self.menu_index(&id)?;
                let target = self.tool_target(ix)?;
                let color = self.group_plot_color(ix).unwrap_or_else(|| {
                    self.theme
                        .plot_theme()
                        .get_color(self.group_color_index(ix))
                });
                Some(SampleInput {
                    id,
                    is_current: current == Some(ix),
                    color: format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b),
                    spectrum: SpectrumInput {
                        label: target.label,
                        path: target.path,
                        params: self.effective_params(ix).clone(),
                        group: ix
                            .checked_sub(DERIVED_BASE)
                            .and_then(|index| self.derived.get(index))
                            .cloned(),
                        data: self.composer_sample_spectrum(ix),
                        source_error: None,
                    },
                })
            })
            .collect();
        ComposerInput {
            current: self.publication_figures(),
            samples,
        }
    }

    /// Dependencies of active and inactive recipes for export and revision checks.
    pub(super) fn composer_export_sample_ids(&self) -> BTreeSet<GroupId> {
        let current = self.current_group_index().and_then(|ix| self.group_id(ix));
        recipe_sample_ids(
            self.publish
                .settings
                .recipes()
                .iter()
                .map(|named| &named.recipe),
            current,
        )
    }

    /// Offer the current group first, followed by marked groups in workspace order.
    pub(super) fn composer_sample_choices(&self) -> Vec<(GroupId, String)> {
        // Marks remain eligible when their rows are filtered out or folded.
        // Expanding this unfiltered model retains source-channel adjacency;
        // catalog primaries remain virtual even for very large workspaces.
        let rows = crate::app::group_rows::build_rows_active(
            &self.catalog,
            &self.derived,
            |_| Some(true),
            None,
            "",
            self.standalone_path(),
            &BTreeSet::new(),
            &self.group_registry.excluded_indices(),
        );
        composer_sample_indices(self.current_group_index(), &self.selection, &rows)
            .into_iter()
            .filter_map(|ix| self.group_id(ix).map(|id| (id, self.entry_label(ix))))
            .collect()
    }

    /// Gather only the IDs needed by non-fit panels; rendering retains list order.
    fn composer_sample_ids(&self) -> BTreeSet<GroupId> {
        let current = self.current_group_index().and_then(|ix| self.group_id(ix));
        recipe_sample_ids([&self.publish.settings.active_recipe().recipe], current)
    }

    /// The same processed revision used by workspace overlays. A standalone
    /// current group can live outside the shared LRU cache, so use its ready data.
    fn composer_sample_spectrum(&self, ix: usize) -> Option<Arc<rexafs::prelude::XASSpectrum>> {
        let fingerprint = self.effective_fingerprint(ix);
        self.cache
            .peek(&(ix, fingerprint))
            .cloned()
            .or_else(|| {
                (self.current_group_index() == Some(ix) && self.publication_ready())
                    .then(|| self.spectrum.clone())
                    .flatten()
            })
            .or_else(|| {
                self.publish
                    .samples
                    .figures
                    .get(&self.group_id(ix)?)
                    .filter(|cached| cached.fingerprint == fingerprint)
                    .map(|cached| cached.spectrum.clone())
            })
    }

    /// Queue missing saved samples through the existing background batch path.
    /// The queue also accepts unmarked samples restored from a project.
    pub(super) fn ensure_composer_samples(&mut self, cx: &mut Context<Self>) {
        self.publish
            .samples
            .settle_requests(self.compare_gen, self.compare_running);
        if self.compare_running {
            return;
        }
        // Keep completed results before another batch can evict them from the
        // workspace LRU. Saved figures must not change as other groups load.
        let _ = self.composer_samples();
        let ids = self.composer_sample_ids();
        self.publish
            .samples
            .attempted
            .retain(|(id, _)| ids.contains(id));
        let mut requests = Vec::new();
        for id in ids {
            let Some(ix) = self.menu_index(&id) else {
                continue;
            };
            let key = (id, self.effective_fingerprint(ix));
            if self.composer_sample_spectrum(ix).is_some() {
                self.publish.samples.attempted.remove(&key);
                continue;
            }
            if (self.load_running && self.current_group_index() == Some(ix))
                || self.publish.samples.attempted.contains(&key)
            {
                continue;
            }
            requests.push((key, ix));
        }
        if requests.is_empty() {
            return;
        }
        // At most one cacheful per batch lets the next redraw retain every
        // result, even when the saved selection exceeds the workspace cache.
        requests.truncate(crate::app::PROCESSED_CACHE_CAPACITY);
        self.ensure_groups_loaded(requests.iter().map(|(_, ix)| *ix).collect(), cx);
        let keys: Vec<_> = requests.into_iter().map(|(key, _)| key).collect();
        self.publish.samples.attempted.extend(keys.iter().cloned());
        self.publish.samples.pending = self.compare_running.then_some((self.compare_gen, keys));
    }

    /// Resolve labels and colours at render time, retaining stable figure Arcs
    /// between redraws. Missing IDs are omitted; unprocessed groups keep their
    /// label with an empty figure list for the composer's missing-data message.
    pub(super) fn composer_samples(&mut self) -> Vec<CompositeSample> {
        let ids = self.composer_sample_ids();
        self.publish
            .samples
            .figures
            .retain(|id, _| ids.contains(id));
        let mut samples = Vec::new();
        for id in ids {
            let Some(ix) = self.menu_index(&id) else {
                continue;
            };
            let label = self.entry_label(ix);
            let color = self.group_plot_color(ix).unwrap_or_else(|| {
                self.theme
                    .plot_theme()
                    .get_color(self.group_color_index(ix))
            });
            let color = format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b);
            let figures = if let Some(spectrum) = self.composer_sample_spectrum(ix) {
                self.cache_composer_sample(
                    id.clone(),
                    ix,
                    self.effective_fingerprint(ix),
                    spectrum,
                );
                self.publish.samples.figures[&id].figures.clone()
            } else {
                self.publish.samples.figures.remove(&id);
                Vec::new()
            };
            samples.push(CompositeSample {
                id,
                label,
                color,
                figures,
                is_current: self.current_group_index() == Some(ix),
            });
        }
        samples
    }

    /// Pin selected batch results before an unrelated load can evict them from
    /// the workspace LRU, including when a batch fills that cache completely.
    pub(crate) fn retain_composer_sample(
        &mut self,
        ix: usize,
        fingerprint: u64,
        spectrum: Arc<rexafs::prelude::XASSpectrum>,
    ) {
        if !self.publish.composer.open {
            return;
        }
        if let Some(id) = self.group_id(ix)
            && self.composer_sample_ids().contains(&id)
        {
            self.cache_composer_sample(id, ix, fingerprint, spectrum);
        }
    }

    /// Rebuild only for a new revision or changed display metadata.
    fn cache_composer_sample(
        &mut self,
        id: GroupId,
        ix: usize,
        fingerprint: u64,
        spectrum: Arc<rexafs::prelude::XASSpectrum>,
    ) {
        let label = self.entry_label(ix);
        let quantity = ix
            .checked_sub(DERIVED_BASE)
            .and_then(|index| self.derived.get(index))
            .map(|group| group.quantity)
            .unwrap_or_default();
        if self.publish.samples.figures.get(&id).is_none_or(|cached| {
            !Arc::ptr_eq(&cached.spectrum, &spectrum)
                || cached.fingerprint != fingerprint
                || cached.label != label
                || cached.quantity != quantity
        }) {
            let figures = quantity_figures(spectrum.clone(), &label, Some(quantity))
                .into_iter()
                .map(Arc::new)
                .collect();
            self.publish.samples.figures.insert(
                id,
                CachedSample {
                    spectrum,
                    fingerprint,
                    label,
                    quantity,
                    figures,
                },
            );
        }
    }

    /// Track data, renames, removals, the current sample and workspace palette
    /// changes in previews.
    pub(super) fn composer_sample_source(&self, samples: &[CompositeSample]) -> Vec<usize> {
        let mut source = Vec::new();
        for sample in samples {
            source.push(sample_metadata_revision(sample));
            source.extend(
                sample
                    .figures
                    .iter()
                    .map(|figure| Arc::as_ptr(figure) as usize),
            );
        }
        source
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app::group_rows,
        catalog::{Catalog, FileMeta},
        params::DerivedSpectrum,
    };

    #[test]
    fn publication_export_collects_inactive_recipe_samples_and_panel_overrides() {
        use crate::publication::figures::composer::CompositePanel;
        let id = GroupId::legacy_result;
        let recipes = [
            CompositeRecipe {
                samples: vec![id(1), id(2)],
                panels: vec!["xanes".into(), "fit-k".into()],
                ..Default::default()
            },
            CompositeRecipe {
                samples: vec![id(99)],
                panels: vec![
                    CompositePanel {
                        samples: Some(vec![id(3), id(2)]),
                        ..CompositePanel::from("chi-k")
                    },
                    CompositePanel {
                        samples: Some(vec![]),
                        ..CompositePanel::from("xanes")
                    },
                    CompositePanel {
                        samples: Some(vec![id(98)]),
                        ..CompositePanel::from("fit-r")
                    },
                ],
                ..Default::default()
            },
        ];
        assert_eq!(
            recipe_sample_ids(&recipes, None),
            BTreeSet::from([id(1), id(2), id(3)])
        );
        assert_eq!(
            recipe_sample_ids([&recipes[1]], None),
            BTreeSet::from([id(2), id(3)])
        );
    }

    fn sample_catalog() -> Catalog {
        let mut catalog = Catalog::default();
        catalog.extend(
            ["a.dat", "b.dat", "c.dat"]
                .into_iter()
                .map(|name| FileMeta {
                    dir: "/data".into(),
                    name: name.into(),
                    size: 0,
                })
                .collect(),
        );
        catalog
    }

    #[test]
    fn implicit_current_sample_is_retained_only_for_a_nonblank_figure_label() {
        let current = GroupId::legacy_result(1);
        let selected = GroupId::legacy_result(2);
        let mut recipe = CompositeRecipe {
            panels: vec!["xanes".into()],
            ..Default::default()
        };
        assert!(recipe_sample_ids([&recipe], Some(current.clone())).is_empty());
        recipe.figure_labels.insert(current.clone(), " \t ".into());
        assert!(recipe_sample_ids([&recipe], Some(current.clone())).is_empty());
        recipe
            .figure_labels
            .insert(current.clone(), "Reference".into());
        assert_eq!(
            recipe_sample_ids([&recipe], Some(current.clone())),
            BTreeSet::from([current.clone()])
        );
        assert!(recipe_sample_ids([&recipe], None).is_empty());

        recipe.samples = vec![selected.clone()];
        assert_eq!(
            recipe_sample_ids([&recipe], Some(current.clone())),
            BTreeSet::from([selected.clone()]),
            "a selected sample does not need the implicit current sample"
        );
        recipe.panels.push("chi-k".into());
        recipe.panels[0].samples = Some(Vec::new());
        assert_eq!(
            recipe_sample_ids([&recipe], Some(current.clone())),
            BTreeSet::from([current.clone(), selected]),
            "an independently empty panel still needs the current label"
        );
        recipe.panels = vec!["fit-k".into()];
        recipe.samples.clear();
        assert!(recipe_sample_ids([&recipe], Some(current)).is_empty());
    }

    #[test]
    fn composer_preview_revision_tracks_current_sample_without_changing_arrays() {
        let mut sample = CompositeSample {
            id: GroupId::legacy_result(1),
            label: "Original workspace name".into(),
            color: "#123456".into(),
            figures: Vec::new(),
            is_current: false,
        };
        let selected_revision = sample_metadata_revision(&sample);
        sample.is_current = true;
        assert_ne!(sample_metadata_revision(&sample), selected_revision);
        sample.is_current = false;
        assert_eq!(sample_metadata_revision(&sample), selected_revision);
    }

    #[test]
    fn composer_choices_put_current_before_marked_catalog_then_results() {
        let derived = [DerivedSpectrum {
            label: "D".into(),
            ..Default::default()
        }];
        let rows = group_rows::build_rows_active(
            &sample_catalog(),
            &derived,
            |_| Some(true),
            None,
            "",
            None,
            &BTreeSet::new(),
            &BTreeSet::new(),
        );
        let marks = BTreeSet::from([0, 2, DERIVED_BASE]);
        assert_eq!(
            composer_sample_indices(Some(2), &marks, &rows),
            vec![2, 0, DERIVED_BASE],
            "current C appears once, followed by marked catalog A and result D"
        );
        assert_eq!(marks, BTreeSet::from([0, 2, DERIVED_BASE]));
    }

    #[test]
    fn composer_choices_keep_hidden_channels_beside_their_source() {
        // Creation order differs from workspace order: D, B's channel, A's channel.
        let derived = [
            DerivedSpectrum {
                label: "D".into(),
                ..Default::default()
            },
            DerivedSpectrum {
                source: Some("/data/b.dat".into()),
                ..Default::default()
            },
            DerivedSpectrum {
                source: Some("/data/a.dat".into()),
                ..Default::default()
            },
        ];
        let catalog = sample_catalog();
        let marks = BTreeSet::from([0, 1, DERIVED_BASE, DERIVED_BASE + 1, DERIVED_BASE + 2]);
        let filtered = group_rows::build_rows(
            &catalog,
            &derived,
            &BTreeSet::new(),
            Some(Arc::new(vec![2])),
            "c.dat",
            None,
        );
        assert!(marks.iter().all(|&ix| filtered.row_index(ix).is_none()));
        let collapsed =
            group_rows::build_rows(&catalog, &derived, &BTreeSet::new(), None, "", None);
        assert!(collapsed.row_index(DERIVED_BASE + 1).is_none());
        assert!(collapsed.row_index(DERIVED_BASE + 2).is_none());

        let rows = group_rows::build_rows_active(
            &catalog,
            &derived,
            |_| Some(true),
            None,
            "",
            None,
            &BTreeSet::new(),
            &BTreeSet::new(),
        );
        assert_eq!(
            composer_sample_indices(Some(2), &marks, &rows),
            vec![2, 0, DERIVED_BASE + 2, 1, DERIVED_BASE + 1, DERIVED_BASE],
            "filtering and folding retain marks in their expanded workspace order"
        );
    }

    #[test]
    fn composer_catalog_rescan_clears_pinned_spectra_and_failed_attempts() {
        let id = GroupId::legacy_result(1);
        let mut state = PublishState::default();
        state.samples.figures.insert(
            id.clone(),
            CachedSample {
                spectrum: Arc::new(rexafs::prelude::XASSpectrum::default()),
                fingerprint: 10,
                label: "Sample".into(),
                quantity: Quantity::RawMu,
                figures: Vec::new(),
            },
        );
        state.samples.attempted.insert((id, 10));
        assert_eq!(state.samples.figures.len(), 1);
        assert_eq!(state.samples.attempted.len(), 1);
        state.invalidate_composer_samples();
        assert!(state.samples.figures.is_empty());
        assert!(state.samples.attempted.is_empty());
        assert!(state.samples.pending.is_none());
    }

    #[test]
    fn composer_sample_failures_do_not_retry_each_other_in_later_batches() {
        let a = (GroupId::legacy_result(1), 10);
        let b = (GroupId::legacy_result(2), 20);
        let mut state = ComposerSamplesState::default();
        state.attempted.insert(a.clone());
        state.pending = Some((1, vec![a.clone()]));
        state.settle_requests(1, false);
        state.attempted.insert(b.clone());
        state.pending = Some((2, vec![b.clone()]));
        state.settle_requests(2, false);
        state.settle_requests(3, false);
        assert_eq!(state.attempted, BTreeSet::from([a, b]));
        assert!(state.pending.is_none());
    }

    #[test]
    fn composer_sample_cancellation_retries_only_interrupted_requests() {
        let completed = (GroupId::legacy_result(1), 10);
        let interrupted = (GroupId::legacy_result(2), 20);
        let mut state = ComposerSamplesState {
            attempted: BTreeSet::from([completed.clone(), interrupted.clone()]),
            pending: Some((2, vec![interrupted.clone()])),
            ..Default::default()
        };
        state.settle_requests(2, true);
        assert!(state.attempted.contains(&interrupted));
        state.settle_requests(3, true);
        assert_eq!(state.attempted, BTreeSet::from([completed]));
        assert!(state.pending.is_none());
    }
}
