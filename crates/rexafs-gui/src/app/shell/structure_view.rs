//! Fit stage · structure database, cluster / path visualizer, path selection.
//!
//! A native molecular canvas shows shaded CPK atoms, periodic crystal
//! context, the FEFF cluster, and directed scattering legs at a stable scale.
//! The coordination histogram uses ruviz.
//! Atoms come from the workspace `feff.inp`, legs from every path's
//! `feffNNNN.dat` (see [`crate::structure`]). The structure panel talks to
//! [`crate::structure::StructureProvider`] — the seam the core crate's
//! `xafs::structure` sources plug into.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use super::molecule_view::{
    AtomStyle, CrystalContext, MoleculeScene, PolyAtoms, PolyhedronOptions, ViewCamera,
    crystal_context,
};
use gpui::{
    ClickEvent, Context, Entity, IntoElement, ParentElement, SharedString, Styled, div, prelude::*,
    px,
};
use ruviz::render::Color as PlotColor;
use ruviz_gpui::{RuvizPlot, plot_builder};

use super::fit_workspace::FitStep;
use super::{MONO, button, chip, section_label, segment, segmented};
use crate::app::StudioApp;
use crate::settings::{UserSettings, default_amcsd_path};
use crate::structure::{
    Cluster, PathGeometry, SourceConfig, StructureHit, StructureSourceKind, StructureSummary,
    cpk_color, import_cif, load_cluster, load_path_geometry, normalise_importance, provider_for,
};
use crate::theme::Theme;
use crate::widgets::text_input::{InputEvent, InputStyle, TextInput};
use gpui::PathPromptOptions;
use rexafs::xafs::structure as core;

/// Camera presets offered above the 3D view.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum CameraPreset {
    Isometric,
    DownC,
    DownA,
    DownB,
}

impl CameraPreset {
    const ALL: [CameraPreset; 4] = [
        CameraPreset::Isometric,
        CameraPreset::DownC,
        CameraPreset::DownA,
        CameraPreset::DownB,
    ];
    fn label(self) -> &'static str {
        match self {
            CameraPreset::Isometric => "3/4",
            CameraPreset::DownC => "↓c",
            CameraPreset::DownA => "↓a",
            CameraPreset::DownB => "↓b",
        }
    }
}

/// Info card for the picked atom.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AtomPick {
    pub atom: usize,
}

/// Path table filters.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PathFilters {
    pub single_scattering: bool,
    pub max_legs: Option<usize>,
    pub max_reff: Option<f64>,
    /// Minimum relative amplitude (0..1).
    pub min_importance: f64,
}

impl Default for PathFilters {
    fn default() -> Self {
        Self {
            single_scattering: false,
            max_legs: None,
            max_reff: None,
            min_importance: 0.0,
        }
    }
}

/// Everything the structure feature keeps on the app.
pub(crate) struct StructureState {
    pub diagnostics: super::path_diagnostics::PathDiagnostics,
    pub filter_to_spectrum: bool,
    // ---- cluster + paths ----
    pub cluster: Option<Cluster>,
    /// Candidate preview is separate from the calculated model.
    pub preview_cluster: Option<Cluster>,
    pub category: Option<&'static str>,
    cluster_workspace: Option<PathBuf>,
    /// Parallel to `StudioApp::fit_paths`.
    pub paths: Vec<Option<PathGeometry>>,
    pub selected: Option<usize>,
    pub hovered: Option<usize>,
    pub multi: BTreeSet<usize>,
    pub filters: PathFilters,
    pub max_reff_input: Entity<TextInput>,
    // ---- 3D view ----
    pub show: bool,
    pub scene: Option<Arc<MoleculeScene>>,
    pub depth: super::depth_controls::DepthControls,
    pub camera: ViewCamera,
    pub drag: Option<(gpui::Point<gpui::Pixels>, gpui::Point<gpui::Pixels>, bool)>,
    pub view_bounds: Option<gpui::Bounds<gpui::Pixels>>,
    pub atom_style: AtomStyle,
    pub bond_mode: super::bond_geometry::BondMode,
    pub highlight_absorber: bool,
    pub absorber_label: bool,
    pub shading: bool,
    pub crystal_visible: bool,
    pub molecule_only: bool,
    pub show_hydrogens: bool,
    pub atom_labels: bool,
    pub poly_options: PolyhedronOptions,
    pub poly_cutoff_input: Entity<TextInput>,
    pub preview_context: Option<CrystalContext>,
    pub preview_radius: f64,
    pub source_clusters: BTreeMap<PathBuf, Cluster>,
    pub source_contexts: BTreeMap<PathBuf, CrystalContext>,
    pub source_filter: Option<PathBuf>,
    pub path_leg: Option<usize>,
    pub hist: Option<Entity<RuvizPlot>>,
    pub show_shell_hist: bool,
    pub color_by_shell: bool,
    pub pick: Option<AtomPick>,
    pub plot_error: Option<String>,
    // ---- structure panel ----
    pub source: StructureSourceKind,
    pub search: Entity<TextInput>,
    pub hits: Vec<StructureHit>,
    pub search_error: Option<String>,
    pub summary: Option<StructureSummary>,
    pub absorber: Option<String>,
    /// Chosen crystallographic site of the absorber element (index into
    /// `summary.sites`); `None` = every site of the element.
    pub absorber_site: Option<usize>,
    pub edge: String,
    pub backend: rexafs::prelude::FeffExecutionMode,
    pub radius: Entity<TextInput>,
    pub mp_key: Entity<TextInput>,
    pub mp_key_editing: bool,
    /// The cluster the current FEFF workspace was generated from.
    pub core_cluster: Option<Arc<core::Cluster>>,
    pub(crate) core_cluster_workspace: Option<PathBuf>,
    pub settings: UserSettings,
    pub cif_library: Option<Arc<core::LocalCifLibrary>>,
    pub cif_scanning: bool,
    search_gen: u64,
    fetch_gen: u64,
    pub search_running: bool,
    pub fetch_running: bool,
    pub download_cancel: Option<Arc<AtomicBool>>,
    pub download_progress: Option<(u64, Option<u64>)>,
    /// Text filter of the path picker.
    pub path_filter: Entity<TextInput>,
    /// Shells whose multiple-scattering paths are unfolded in the picker.
    pub ms_open: BTreeSet<usize>,
}

impl StructureState {
    pub(crate) fn new(theme: Theme, cx: &mut Context<StudioApp>) -> Self {
        let mono = InputStyle {
            mono: true,
            align_right: true,
            ..Default::default()
        };
        let search = cx.new(|cx| TextInput::new("formula, mineral or id…", "", theme, cx));
        cx.subscribe(&search, |this: &mut StudioApp, _f, event, cx| {
            if let InputEvent::Committed(_) = event {
                this.structure_search(cx);
            }
        })
        .detach();
        let radius = cx.new(|cx| TextInput::new("rmax", "8.0", theme, cx).with_style(mono));
        cx.subscribe(&radius, |this: &mut StudioApp, _f, event, cx| {
            if let InputEvent::Committed(_) = event {
                this.preview_structure(cx);
            }
        })
        .detach();
        let max_reff_input = cx.new(|cx| TextInput::new("max R", "", theme, cx).with_style(mono));
        let poly_cutoff_input = cx.new(|cx| TextInput::new("auto", "", theme, cx).with_style(mono));
        cx.subscribe(&poly_cutoff_input, |this: &mut StudioApp, _, event, cx| {
            if let InputEvent::Committed(text) = event {
                let text = text.trim();
                if text.is_empty() || text.eq_ignore_ascii_case("auto") {
                    this.structure.poly_options.cutoff = None;
                } else if let Some(value) = text
                    .parse::<f64>()
                    .ok()
                    .filter(|value| value.is_finite() && (0.5..=6.0).contains(value))
                {
                    this.structure.poly_options.cutoff = Some(value);
                } else {
                    this.structure.plot_error =
                        Some("Polyhedron bond limit must be auto or 0.5–6 Å.".into());
                    cx.notify();
                    return;
                }
                this.structure.plot_error = None;
                this.rebuild_structure_plot(cx);
                cx.notify();
            }
        })
        .detach();
        cx.subscribe(&max_reff_input, |this: &mut StudioApp, _f, event, cx| {
            if let InputEvent::Committed(text) = event {
                this.structure.filters.max_reff = text.trim().parse().ok();
                cx.notify();
            }
        })
        .detach();
        let mp_key = cx.new(|cx| {
            TextInput::new("Materials Project API key", "", theme, cx).with_style(InputStyle {
                mono: true,
                ..Default::default()
            })
        });
        cx.subscribe(&mp_key, |this: &mut StudioApp, _f, event, cx| {
            if let InputEvent::Committed(text) = event {
                this.structure.mp_key_editing = false;
                this.structure.settings.mp_api_key = text.trim().to_string();
                this.save_user_settings();
                cx.notify();
            }
        })
        .detach();
        let settings = UserSettings::load();
        mp_key.update(cx, |input, cx| {
            input.set_text(settings.mp_api_key.clone(), cx)
        });
        Self {
            cluster: None,
            preview_cluster: None,
            category: None,
            filter_to_spectrum: true,
            diagnostics: Default::default(),
            cluster_workspace: None,
            paths: Vec::new(),
            selected: None,
            hovered: None,
            multi: BTreeSet::new(),
            filters: PathFilters::default(),
            max_reff_input,
            show: false,
            scene: None,
            depth: Default::default(),
            camera: ViewCamera::default(),
            drag: None,
            view_bounds: None,
            atom_style: AtomStyle::BallStick,
            bond_mode: Default::default(),
            highlight_absorber: true,
            absorber_label: false,
            shading: true,
            crystal_visible: true,
            molecule_only: false,
            show_hydrogens: true,
            atom_labels: false,
            poly_options: PolyhedronOptions::default(),
            poly_cutoff_input,
            preview_context: None,
            preview_radius: 8.,
            source_clusters: BTreeMap::new(),
            source_contexts: BTreeMap::new(),
            source_filter: None,
            path_leg: None,
            hist: None,
            show_shell_hist: false,
            color_by_shell: false,
            pick: None,
            plot_error: None,
            source: StructureSourceKind::Builtin,
            search,
            hits: Vec::new(),
            search_error: None,
            summary: None,
            absorber: None,
            absorber_site: None,
            edge: "K".to_string(),
            backend: crate::feffgen::selected_feff_mode().unwrap_or_default(),
            radius,
            mp_key,
            mp_key_editing: false,
            core_cluster: None,
            core_cluster_workspace: None,
            settings,
            cif_library: None,
            cif_scanning: false,
            search_gen: 0,
            fetch_gen: 0,
            search_running: false,
            fetch_running: false,
            download_cancel: None,
            download_progress: None,
            path_filter: cx.new(|cx| {
                TextInput::new("filter paths…", "", theme, cx).with_style(InputStyle {
                    mono: false,
                    ..Default::default()
                })
            }),
            ms_open: BTreeSet::new(),
        }
    }

    /// Configuration handed to a provider (cheap to clone into a job).
    pub(crate) fn source_config(&self) -> SourceConfig {
        SourceConfig {
            cif_library: self.cif_library.clone(),
            amcsd_db: self.settings.amcsd_db.clone(),
            mp_api_key: self.settings.mp_api_key.clone(),
        }
    }

    pub(crate) fn mp_key_text(&self, cx: &gpui::App) -> String {
        self.mp_key.read(cx).text().trim().to_string()
    }
}

fn theme_rgb(c: gpui::Rgba) -> PlotColor {
    PlotColor::from_rgb(
        (c.r * 255.0) as u8,
        (c.g * 255.0) as u8,
        (c.b * 255.0) as u8,
    )
}

impl StudioApp {
    // ---- data ------------------------------------------------------------

    /// Reload the cluster / path geometries when the FEFF workspace or the
    /// path list changed, then refresh the 3D view. Cheap when nothing moved.
    pub(crate) fn refresh_structure(&mut self, cx: &mut Context<Self>) {
        let ws = self.feff_workspace.clone();
        let mut changed = false;
        if ws != self.structure.cluster_workspace {
            // A workspace generated from a structure keeps its core cluster;
            // anything else (Choose feff.inp…, a template) is parsed back.
            if ws.is_some() && ws != self.structure.core_cluster_workspace {
                self.structure.core_cluster = None;
                self.structure.core_cluster_workspace = None;
            }
            self.structure.cluster = match &self.structure.core_cluster {
                Some(c) => Some(Cluster::from_core(c)),
                None => ws.as_deref().and_then(load_cluster),
            };
            self.structure.cluster_workspace = ws;
            changed = true;
        }
        let files: Vec<PathBuf> = self.fit_paths.iter().map(|r| r.spec.file.clone()).collect();
        let mut sources = self.path_sources();
        if let Some(ws) = &self.feff_workspace {
            if !sources.contains(ws) {
                sources.push(ws.clone());
            }
        }
        for source in sources {
            if !self.structure.source_clusters.contains_key(&source) {
                if let Some(cluster) = load_cluster(&source) {
                    self.structure
                        .source_clusters
                        .insert(source.clone(), cluster);
                }
                if let Ok(json) = std::fs::read(source.join("crystal.json")) {
                    if let Ok((s, c)) =
                        serde_json::from_slice::<(core::Structure, core::Cluster)>(&json)
                    {
                        self.structure
                            .source_contexts
                            .insert(source.clone(), crystal_context(&s, &c));
                    }
                }
                changed = true;
            }
        }
        if self
            .structure
            .source_filter
            .as_ref()
            .is_none_or(|s| !files.iter().any(|f| f.parent() == Some(s.as_path())))
        {
            self.structure.source_filter = files
                .first()
                .and_then(|f| f.parent().map(|p| p.to_path_buf()));
        }
        let same = self.structure.paths.len() == files.len()
            && self
                .structure
                .paths
                .iter()
                .zip(files.iter())
                .all(|(p, f)| p.as_ref().is_some_and(|p| &p.file == f));
        if !same {
            let core_cluster = self.structure.core_cluster.clone();
            let mut paths: Vec<Option<PathGeometry>> = files
                .iter()
                .map(|f| {
                    load_path_geometry(
                        f,
                        core_cluster.as_deref().filter(|_| {
                            f.parent() == self.structure.core_cluster_workspace.as_deref()
                        }),
                    )
                })
                .collect();
            normalise_importance(&mut paths);
            for source in self.path_sources() {
                if !self.structure.source_clusters.contains_key(&source) {
                    let local: Vec<_> = paths
                        .iter()
                        .zip(&files)
                        .filter(|(_, f)| f.parent() == Some(source.as_path()))
                        .map(|(p, _)| p.clone())
                        .collect();
                    if let Some(cluster) = cluster_from_paths(&local) {
                        self.structure.source_clusters.insert(source, cluster);
                    }
                }
            }
            // A cluster may be missing (paths added by file): synthesise one
            // from the union of path legs so the view still works.
            if self.structure.cluster.is_none() {
                self.structure.cluster = cluster_from_paths(&paths);
            }
            self.structure.paths = paths;
            self.structure.selected = self
                .structure
                .selected
                .filter(|&i| i < self.fit_paths.len());
            self.structure.multi.retain(|&i| i < self.fit_paths.len());
            changed = true;
        }
        if changed {
            self.rebuild_structure_plot(cx);
        }
    }

    pub(crate) fn select_structure_path(
        &mut self,
        i: usize,
        modifiers: gpui::Modifiers,
        cx: &mut Context<Self>,
    ) {
        if modifiers.platform {
            if !self.structure.multi.remove(&i) {
                self.structure.multi.insert(i);
            }
        } else if modifiers.shift {
            let anchor = self.structure.selected.unwrap_or(i);
            let (lo, hi) = (anchor.min(i), anchor.max(i));
            for j in lo..=hi {
                self.structure.multi.insert(j);
            }
        } else {
            self.structure.multi.clear();
            self.structure.multi.insert(i);
        }
        self.structure.selected = Some(i);
        self.structure.path_leg = None;
        self.structure.pick = None;
        self.rebuild_structure_plot(cx);
        cx.notify();
    }

    pub(crate) fn hover_structure_path(&mut self, i: Option<usize>, cx: &mut Context<Self>) {
        if self.structure.hovered == i {
            return;
        }
        self.structure.hovered = i;
        if self.fit_result.is_some() {
            self.rebuild_fit_plots(cx);
        }
        cx.notify();
    }

    #[allow(dead_code)]
    fn set_paths_enabled(&mut self, indices: &[usize], enabled: bool, cx: &mut Context<Self>) {
        self.set_paths_selected(indices, enabled, cx);
    }

    // ---- 3D plot ---------------------------------------------------------

    pub(crate) fn rebuild_structure_plot(&mut self, cx: &mut Context<Self>) {
        let Some(cluster) = self.displayed_cluster() else {
            self.structure.scene = None;
            self.structure.hist = None;
            return;
        };
        let path = self
            .structure
            .selected
            .filter(|_| !self.viewing_candidate())
            .and_then(|i| self.structure.paths.get(i))
            .and_then(|p| p.as_ref());
        let context = if self.viewing_candidate() {
            self.structure.preview_context.as_ref()
        } else {
            self.displayed_source()
                .and_then(|s| self.structure.source_contexts.get(&s))
        };
        let radius = if self.viewing_candidate() {
            self.structure.preview_radius
        } else {
            self.displayed_source()
                .and_then(|s| self.structure.source_contexts.get(&s))
                .map(|c| c.radius)
                .unwrap_or_else(|| cluster.atoms.iter().map(|a| a.dist).fold(0., f64::max))
        };
        let mut scene = if self.structure.molecule_only
            && let Some(Ok(molecule)) = context.and_then(|c| c.molecule.as_ref())
        {
            MoleculeScene::molecule(
                molecule,
                &cluster,
                radius,
                self.structure.show_hydrogens,
                self.structure.atom_style,
            )
        } else {
            MoleculeScene::new(
                &cluster,
                context.filter(|_| self.structure.crystal_visible),
                radius,
                self.structure.atom_style,
                path,
                self.structure.pick.as_ref().map(|p| p.atom),
                self.structure.poly_options,
            )
        };
        if self.structure.molecule_only
            && let Some(Err(error)) = context.and_then(|c| c.molecule.as_ref())
        {
            scene.message = Some(error.clone());
        }
        scene.labels = self.structure.atom_labels;
        if matches!(
            self.structure.atom_style,
            AtomStyle::BallStick | AtomStyle::Wireframe
        ) {
            scene.apply_bond_mode(self.structure.bond_mode);
        }
        self.structure.scene = Some(Arc::new(scene));
        self.rebuild_structure_hist(&cluster, cx);
    }

    fn rebuild_structure_hist(&mut self, cluster: &Cluster, cx: &mut Context<Self>) {
        let labels: Vec<String> = cluster
            .shells
            .iter()
            .skip(1)
            .map(|(d, _)| format!("{d:.2}"))
            .collect();
        let counts: Vec<f64> = cluster
            .shells
            .iter()
            .skip(1)
            .map(|(_, n)| *n as f64)
            .collect();
        if labels.is_empty() {
            self.structure.hist = None;
            return;
        }
        let plot: ruviz::core::Plot = ruviz::core::Plot::new()
            .theme(self.theme.plot_theme())
            .bar(&labels, &counts)
            .color(theme_rgb(self.theme.accent))
            .xlabel("R (Å)")
            .ylabel("atoms")
            .into();
        let plot = plot.size_px(720, 150);
        match &self.structure.hist {
            Some(h) => h.update(cx, |rp, cx| rp.set_plot(plot, cx)),
            None => {
                self.structure.hist = Some(
                    plot_builder(plot)
                        .presentation(ruviz_gpui::PresentationMode::Image)
                        .build(cx),
                )
            }
        }
    }

    pub(crate) fn set_structure_camera(&mut self, preset: CameraPreset, cx: &mut Context<Self>) {
        let (az, el) = match preset {
            CameraPreset::Isometric => (-0.6, 0.45),
            CameraPreset::DownC => (0., std::f64::consts::FRAC_PI_2),
            CameraPreset::DownA => (std::f64::consts::FRAC_PI_2, 0.),
            CameraPreset::DownB => (0., 0.),
        };
        self.structure.camera.az = az;
        self.structure.camera.el = el;
        cx.notify();
    }

    // ---- structure panel actions -------------------------------------------

    pub(crate) fn save_user_settings(&mut self) {
        if let Err(e) = self.structure.settings.save() {
            self.record_job_error("settings", e);
        }
    }

    /// Run a provider call on the background executor; the result lands
    /// through `apply` unless a newer job of the same kind superseded it
    /// (searches supersede searches, fetches/imports supersede each other).
    fn structure_job<T: Send + 'static>(
        &mut self,
        cx: &mut Context<Self>,
        fetch: bool,
        work: impl FnOnce() -> T + Send + 'static,
        apply: impl FnOnce(&mut Self, T, &mut Context<Self>) + 'static,
    ) {
        let counter = if fetch {
            &mut self.structure.fetch_gen
        } else {
            &mut self.structure.search_gen
        };
        *counter += 1;
        let generation = *counter;
        let job = cx.background_executor().spawn(async move { work() });
        cx.spawn(async move |this, cx| {
            let result = job.await;
            this.update(cx, |app, cx| {
                let current = if fetch {
                    app.structure.fetch_gen
                } else {
                    app.structure.search_gen
                };
                if current == generation {
                    apply(app, result, cx);
                }
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn structure_search(&mut self, cx: &mut Context<Self>) {
        let query = self.structure.search.read(cx).text().to_string();
        let kind = self.structure.source;
        // The CIF library is scanned lazily, once per folder.
        if kind == StructureSourceKind::LocalCif && self.structure.cif_library.is_none() {
            match self.structure.settings.cif_library.clone() {
                Some(root) => {
                    self.structure_scan_cif_library(root, cx);
                    return;
                }
                None => {
                    self.structure.hits.clear();
                    self.structure.search_error =
                        Some("CIF library: choose a folder of .cif files first".into());
                    cx.notify();
                    return;
                }
            }
        }
        // Online sources need a query; switching to their tab must not fire
        // a request (or record an error) for an empty box.
        if query.trim().is_empty()
            && matches!(
                kind,
                StructureSourceKind::MaterialsProject | StructureSourceKind::Cod
            )
        {
            self.structure.hits.clear();
            self.structure.search_error = Some(match kind {
                StructureSourceKind::Cod => {
                    "COD: enter a mineral or compound name, a formula (Fe S2), elements, or a COD id"
                        .to_string()
                }
                _ => "Materials Project: enter a formula (e.g. RuO2) or an mp-id".to_string(),
            });
            cx.notify();
            return;
        }
        let cfg = self.structure.source_config();
        self.structure.search_running = true;
        self.structure.search_error = None;
        cx.notify();
        self.structure_job(
            cx,
            false,
            move || provider_for(kind, &cfg).and_then(|p| p.search(&query)),
            move |app, result, cx| {
                app.structure.search_running = false;
                match result {
                    Ok(hits) => {
                        app.structure.hits = hits;
                        app.structure.search_error = None;
                    }
                    Err(e) => {
                        app.structure.hits.clear();
                        app.structure.search_error = Some(e.clone());
                        app.record_job_error(format!("structure search ({})", kind.badge()), e);
                    }
                }
                cx.notify();
            },
        );
    }

    fn structure_scan_cif_library(&mut self, root: PathBuf, cx: &mut Context<Self>) {
        if self.structure.cif_scanning {
            return;
        }
        self.structure.cif_scanning = true;
        self.structure.search_error = None;
        self.status = format!("scanning {} for CIF files…", root.display()).into();
        cx.notify();
        // Not guarded by the search generation: a scan result is valid
        // whatever was searched meanwhile (it only refreshes the search).
        let job = cx
            .background_executor()
            .spawn(async move { core::LocalCifLibrary::scan(&root).map_err(|e| e.to_string()) });
        cx.spawn(async move |this, cx| {
            let result = job.await;
            this.update(cx, |app, cx| {
                app.structure.cif_scanning = false;
                match result {
                    Ok(lib) => {
                        let n = lib.len();
                        let failed = lib.failures.len();
                        for (path, err) in &lib.failures {
                            app.record_job_error(format!("CIF {}", path.display()), err.clone());
                        }
                        app.structure.cif_library = Some(Arc::new(lib));
                        app.status = if failed == 0 {
                            format!("CIF library: {n} structures").into()
                        } else {
                            format!("CIF library: {n} structures, {failed} files failed to parse")
                                .into()
                        };
                        if app.structure.source == StructureSourceKind::LocalCif {
                            app.structure_search(cx);
                        } else {
                            cx.notify();
                        }
                    }
                    Err(e) => {
                        app.structure.search_error = Some(e.clone());
                        app.record_job_error("CIF library scan", e);
                        cx.notify();
                    }
                }
            })
            .ok();
        })
        .detach();
    }

    /// Apply a fetched/imported structure to the panel.
    fn structure_set_summary(&mut self, summary: StructureSummary, cx: &mut Context<Self>) {
        let el = summary.elements();
        if let Some(interest) = self.spectrum_interest()
            && el.iter().any(|(s, _)| s == &interest.element)
        {
            self.structure.absorber = Some(interest.element);
            if let Some(edge) = interest.edge.filter(|e| {
                ["K", "L1", "L2", "L3", "M1", "M2", "M3", "M4", "M5"].contains(&e.as_str())
            }) {
                self.structure.edge = edge;
            }
        }
        if !self
            .structure
            .absorber
            .as_ref()
            .is_some_and(|a| el.iter().any(|(s, _)| s == a))
        {
            self.structure.absorber = el.first().map(|(s, _)| s.clone());
        }
        self.structure.absorber_site = None;
        for w in &summary.structure.warnings {
            self.record_job_error(format!("structure {}", summary.hit.formula), w.clone());
        }
        self.structure.molecule_only =
            crate::structure::matches_category(&summary.hit, Some("molecule"));
        if self.structure.molecule_only {
            self.structure.atom_style = AtomStyle::BallStick;
        }
        self.structure.poly_options.ligand = None;
        self.structure.summary = Some(summary);
        self.structure.search_error = None;
        self.preview_structure(cx);
        cx.notify();
    }

    pub(crate) fn structure_choose(&mut self, i: usize, cx: &mut Context<Self>) {
        let Some(hit) = self.structure.hits.get(i).cloned() else {
            return;
        };
        let cfg = self.structure.source_config();
        self.structure.fetch_running = true;
        cx.notify();
        let kind = hit.source;
        self.structure_job(
            cx,
            true,
            move || provider_for(kind, &cfg).and_then(|p| p.fetch(&hit)),
            |app, result, cx| {
                app.structure.fetch_running = false;
                match result {
                    Ok(summary) => app.structure_set_summary(summary, cx),
                    Err(e) => {
                        app.structure.search_error = Some(e.clone());
                        app.record_job_error("structure fetch", e);
                        cx.notify();
                    }
                }
            },
        );
    }

    /// "Import CIF…": pick a file, parse it off the UI thread.
    pub(crate) fn structure_import_cif(&mut self, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: None,
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await
                && let Some(file) = paths.first().cloned()
            {
                this.update(cx, |app, cx| app.structure_import_cif_path(file, cx))
                    .ok();
            }
        })
        .detach();
    }

    /// Import one CIF file (also used by the `REXAFS_IMPORT_CIF` launch hook).
    pub(crate) fn structure_import_cif_path(&mut self, file: PathBuf, cx: &mut Context<Self>) {
        self.structure.fetch_running = true;
        self.status = format!("reading {}", file.display()).into();
        cx.notify();
        self.structure_job(
            cx,
            true,
            move || {
                if file
                    .extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| e.eq_ignore_ascii_case("xyz"))
                {
                    crate::structure::import_xyz(&file)
                } else {
                    import_cif(&file)
                }
            },
            |app, result, cx| {
                app.structure.fetch_running = false;
                match result {
                    Ok(summary) => {
                        app.structure.source = StructureSourceKind::LocalCif;
                        app.status =
                            format!("imported {} ({})", summary.hit.name, summary.hit.formula)
                                .into();
                        app.structure_set_summary(summary, cx);
                    }
                    Err(e) => {
                        app.structure.search_error = Some(e.clone());
                        app.record_job_error("CIF import", e);
                        cx.notify();
                    }
                }
            },
        );
    }

    /// "Choose folder…" for the CIF library.
    pub(crate) fn structure_choose_cif_folder(&mut self, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: None,
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await
                && let Some(dir) = paths.first().cloned()
            {
                this.update(cx, |app, cx| {
                    app.structure.settings.cif_library = Some(dir.clone());
                    app.structure.cif_library = None;
                    app.structure.source = StructureSourceKind::LocalCif;
                    app.save_user_settings();
                    app.structure_scan_cif_library(dir, cx);
                })
                .ok();
            }
        })
        .detach();
    }

    /// "Choose…" for an existing AMCSD database file.
    pub(crate) fn structure_choose_amcsd(&mut self, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: None,
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await
                && let Some(file) = paths.first().cloned()
            {
                this.update(cx, |app, cx| {
                    app.structure.settings.amcsd_db = Some(file);
                    app.structure.source = StructureSourceKind::Amcsd;
                    app.save_user_settings();
                    app.structure_search(cx);
                })
                .ok();
            }
        })
        .detach();
    }

    /// Download the AMCSD database to `~/.rexafs` (cancellable; progress
    /// in the status bar).
    pub(crate) fn structure_download_amcsd(&mut self, cx: &mut Context<Self>) {
        if self.structure.download_cancel.is_some() {
            return;
        }
        let Some(dest) = default_amcsd_path() else {
            self.record_job_error("AMCSD download", "HOME not set");
            return;
        };
        let cancel = Arc::new(AtomicBool::new(false));
        let progress = Arc::new(std::sync::Mutex::new((0u64, None::<u64>)));
        let done = Arc::new(AtomicBool::new(false));
        let mirror = Arc::new(std::sync::Mutex::new(String::new()));
        self.structure.download_cancel = Some(cancel.clone());
        self.structure.download_progress = Some((0, None));
        self.status = "downloading AMCSD…".into();
        cx.notify();
        let job = {
            let cancel = cancel.clone();
            let progress = progress.clone();
            let mirror = mirror.clone();
            let dest = dest.clone();
            cx.background_executor().spawn(async move {
                core::db::amcsd::download_amcsd_with(
                    &dest,
                    |received, total| {
                        if let Ok(mut p) = progress.lock() {
                            *p = (received, total);
                        }
                    },
                    &cancel,
                    |url| {
                        if let Ok(mut m) = mirror.lock() {
                            *m = mirror_host(url);
                        }
                    },
                )
                .map_err(|e| e.to_string())
            })
        };
        // Progress ticker.
        {
            let done = done.clone();
            let progress = progress.clone();
            let mirror = mirror.clone();
            cx.spawn(async move |this, cx| {
                while !done.load(Ordering::Relaxed) {
                    cx.background_executor()
                        .timer(Duration::from_millis(250))
                        .await;
                    let p = progress.lock().map(|p| *p).unwrap_or((0, None));
                    let from = mirror
                        .lock()
                        .map(|m| {
                            if m.is_empty() {
                                String::new()
                            } else {
                                format!(" from {m}")
                            }
                        })
                        .unwrap_or_default();
                    if this
                        .update(cx, |app, cx| {
                            if app.structure.download_cancel.is_some() {
                                app.structure.download_progress = Some(p);
                                app.status = match p.1 {
                                    Some(total) if total > 0 => format!(
                                        "downloading AMCSD{from}… {:.0} / {:.0} MB",
                                        p.0 as f64 / 1e6,
                                        total as f64 / 1e6
                                    ),
                                    _ => format!(
                                        "downloading AMCSD{from}… {:.0} MB",
                                        p.0 as f64 / 1e6
                                    ),
                                }
                                .into();
                                cx.notify();
                            }
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            })
            .detach();
        }
        cx.spawn(async move |this, cx| {
            let result = job.await;
            done.store(true, Ordering::Relaxed);
            this.update(cx, |app, cx| {
                app.structure.download_cancel = None;
                app.structure.download_progress = None;
                match result {
                    Ok(path) => {
                        app.status = format!("AMCSD database ready: {}", path.display()).into();
                        app.structure.settings.amcsd_db = Some(path);
                        app.save_user_settings();
                        app.structure.source = StructureSourceKind::Amcsd;
                        app.structure_search(cx);
                    }
                    Err(e) if e.contains("cancelled") => {
                        app.status = "AMCSD download cancelled".into();
                    }
                    Err(e) => {
                        app.status = format!("AMCSD download failed: {e}").into();
                        app.record_job_error("AMCSD download", e);
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn structure_cancel_download(&mut self, cx: &mut Context<Self>) {
        if let Some(flag) = &self.structure.download_cancel {
            flag.store(true, Ordering::Relaxed);
            self.status = "cancelling AMCSD download…".into();
            cx.notify();
        }
    }

    /// Build the cluster and feff.inp for the chosen structure, then run FEFF.
    fn build_candidate_cluster(&self, cx: &gpui::App) -> Result<(core::Cluster, f64), String> {
        let summary = self
            .structure
            .summary
            .as_ref()
            .ok_or("Choose a structure first.")?;
        let absorber = self
            .structure
            .absorber
            .as_ref()
            .ok_or("Choose an absorbing element.")?;
        let radius = self
            .structure
            .radius
            .read(cx)
            .text()
            .trim()
            .parse::<f64>()
            .map_err(|_| "Enter a cluster radius from 2 to 12 Å.")?;
        if !radius.is_finite() || !(2.0..=12.0).contains(&radius) {
            return Err("Enter a cluster radius from 2 to 12 Å.".into());
        }
        let selection = match self
            .structure
            .absorber_site
            .and_then(|i| summary.sites.get(i))
            .filter(|s| s.symbol == *absorber)
        {
            Some(site) => core::AbsorberSelection::SiteIndex(site.site_index),
            None => core::AbsorberSelection::Element(absorber.clone()),
        };
        let built = match &summary.xyz {
            Some(xyz) => {
                let sel = match self
                    .structure
                    .absorber_site
                    .and_then(|i| summary.sites.get(i))
                    .filter(|s| s.symbol == *absorber)
                {
                    Some(site) => core::XyzAbsorber::Index(site.site_index),
                    None => core::XyzAbsorber::CentralOf(absorber.clone()),
                };
                xyz.to_cluster(&sel, Some(radius))
            }
            None => core::build_cluster(
                &summary.structure,
                &selection,
                &core::ClusterOptions {
                    radius,
                    ..Default::default()
                },
            ),
        };
        built.map(|c| (c, radius)).map_err(|e| e.to_string())
    }

    pub(crate) fn preview_structure(&mut self, cx: &mut Context<Self>) {
        self.structure.pick = None;
        match self.build_candidate_cluster(cx) {
            Ok((cluster, radius)) => {
                self.structure.preview_radius = radius;
                self.structure.preview_context = self
                    .structure
                    .summary
                    .as_ref()
                    .filter(|s| s.xyz.is_none())
                    .map(|s| crystal_context(&s.structure, &cluster));
                self.structure.preview_cluster = Some(Cluster::from_core(&cluster));
                self.structure.search_error = None;
            }
            Err(e) => {
                self.structure.preview_cluster = None;
                self.structure.preview_context = None;
                self.structure.search_error = Some(e);
            }
        }
        self.rebuild_structure_plot(cx);
        cx.notify();
    }

    fn viewing_candidate(&self) -> bool {
        matches!(
            self.stage_view.fit_step,
            FitStep::Structure | FitStep::Calculate
        ) && self.structure.summary.is_some()
    }
    fn displayed_cluster(&self) -> Option<Cluster> {
        if self.viewing_candidate() {
            self.structure.preview_cluster.clone()
        } else {
            self.displayed_source()
                .and_then(|s| self.structure.source_clusters.get(&s).cloned())
                .or_else(|| self.structure.cluster.clone())
        }
    }

    pub(crate) fn displayed_source(&self) -> Option<PathBuf> {
        self.structure
            .selected
            .and_then(|i| self.fit_paths.get(i))
            .and_then(|p| p.spec.file.parent().map(|p| p.to_path_buf()))
            .or_else(|| self.structure.source_filter.clone())
            .or_else(|| self.feff_workspace.clone())
    }
    pub(crate) fn path_sources(&self) -> Vec<PathBuf> {
        let mut sources = Vec::new();
        for p in &self.fit_paths {
            if let Some(parent) = p.spec.file.parent() {
                let parent = parent.to_path_buf();
                if !sources.contains(&parent) {
                    sources.push(parent);
                }
            }
        }
        sources
    }
    pub(crate) fn source_label(&self, source: &std::path::Path) -> String {
        self.structure
            .source_clusters
            .get(source)
            .map(|c| c.title.lines().next().unwrap_or("Structure").to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| {
                source
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            })
    }

    pub(crate) fn structure_generate_paths(&mut self, cx: &mut Context<Self>) {
        if self.feff_running {
            return;
        }
        let built = self.build_candidate_cluster(cx);
        let (cluster, radius) = match built {
            Ok(value) => value,
            Err(e) => {
                self.structure.search_error = Some(e.clone());
                self.status = e.into();
                cx.notify();
                return;
            }
        };
        let summary = self.structure.summary.clone().expect("validated structure");
        let absorber = self.structure.absorber.clone().expect("validated absorber");
        let Some(edge) = core::Edge::parse(&self.structure.edge) else {
            self.status = format!("unknown edge {}", self.structure.edge).into();
            cx.notify();
            return;
        };
        for w in &cluster.warnings {
            self.record_job_error("cluster", w.clone());
        }
        let opts = core::FeffInputOptions {
            edge,
            rmax: Some(radius),
            rpath: Some(radius),
            titles: vec![format!(
                "{} · {} · absorber {}",
                summary.hit.formula, summary.hit.name, absorber
            )],
            ..Default::default()
        };
        let inp = core::write_feff_inp(&cluster, &opts);
        match crate::feffgen::new_workspace_with(&inp) {
            Ok(dir) => {
                if summary.xyz.is_none() {
                    if let Ok(json) = serde_json::to_vec(&(&*summary.structure, &cluster)) {
                        if let Err(e) = std::fs::write(dir.join("crystal.json"), json) {
                            self.record_job_error("save crystal context", e.to_string());
                        }
                    }
                }
                self.structure.core_cluster = Some(Arc::new(cluster));
                self.structure.core_cluster_workspace = Some(dir.clone());
                self.set_feff_workspace(Some(dir));
                self.refresh_structure(cx);
                self.run_feff10_now(cx);
            }
            Err(e) => {
                self.status = format!("feff.inp generation failed: {e}").into();
                self.record_job_error("feff.inp generation", e.to_string());
                cx.notify();
            }
        }
    }

    /// One-line summary of the generated cluster for the panel.
    pub(crate) fn structure_cluster_summary(&self) -> Option<String> {
        if self.viewing_candidate() {
            let c = self.structure.preview_cluster.as_ref()?;
            return Some(format!(
                "Preview: {} atoms · {} shells · {:.1} Å cutoff",
                c.atoms.len(),
                c.shells.len().saturating_sub(1),
                self.structure.preview_radius
            ));
        }
        let c = self.structure.core_cluster.as_ref()?;
        let shells = c.shells(0.01);
        let nearest = shells
            .get(1)
            .map(|s| {
                format!(
                    " · nearest {} × {} at {:.3} Å",
                    s.count, s.symbol, s.distance
                )
            })
            .unwrap_or_default();
        Some(format!(
            "cluster: {} atoms · {} shells within {:.1} Å{}{}",
            c.atoms.len(),
            shells.len().saturating_sub(1),
            c.radius,
            nearest,
            if c.warnings.is_empty() {
                String::new()
            } else {
                format!(" · {} warning(s)", c.warnings.len())
            }
        ))
    }

    // ---- center: 3D view + docked table -------------------------------------

    pub(crate) fn structure_center(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        if self.stage_view.fit_step == FitStep::Paths && self.structure.diagnostics.open {
            return self.path_diagnostics_center(cx);
        }
        let t = self.theme;
        if self.structure.scene.is_none() && self.structure.cluster.is_some() {
            self.rebuild_structure_plot(cx);
        }
        let cluster = self.displayed_cluster();
        let toolbar = self.structure_toolbar(cx);
        let mut left = div()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .gap_2()
            .when(self.stage_view.fit_step == FitStep::Paths, |d| {
                d.child(self.path_view_tabs(cx))
            })
            .child(toolbar);
        match (&cluster, &self.structure.scene) {
            (Some(cluster), Some(_scene)) => {
                left = left.child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .relative()
                        .rounded_lg()
                        .border_1()
                        .border_color(t.border)
                        .bg(t.raised)
                        .overflow_hidden()
                        .child(self.molecule_canvas(cx))
                        .child(self.structure_legend(cluster))
                        .children(self.structure_pick_card(cluster)),
                );
                if self.structure.show_shell_hist
                    && let Some(hist) = &self.structure.hist
                {
                    left = left.child(
                        div()
                            .h(px(150.))
                            .flex_none()
                            .rounded_lg()
                            .border_1()
                            .border_color(t.border)
                            .bg(t.raised)
                            .overflow_hidden()
                            .child(
                                div()
                                    .absolute()
                                    .left_3()
                                    .top_1p5()
                                    .text_size(px(11.))
                                    .text_color(t.text_muted)
                                    .child("coordination shells · atoms per distance"),
                            )
                            .child(div().size_full().child(hist.clone())),
                    );
                }
            }
            _ => {
                left = left.child(
                    div()
                        .flex_1()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(t.text_muted)
                        .child(match &self.structure.plot_error {
                            Some(e) => format!("3D view unavailable: {e}"),
                            None => {
                                "Select a reference material to preview its atomic structure here."
                                    .to_string()
                            }
                        }),
                );
            }
        }
        div()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .flex()
            .gap_2()
            .px_3()
            .pt_2()
            .pb_3()
            .child(left)
            .when(self.stage_view.fit_step == FitStep::Paths, |d| {
                d.child(
                    div()
                        .w(px(430.))
                        .flex_none()
                        .min_h_0()
                        .flex()
                        .flex_col()
                        .rounded_lg()
                        .border_1()
                        .border_color(t.border)
                        .bg(t.surface)
                        .overflow_hidden()
                        .child(if self.structure.depth.open {
                            div()
                                .id("path-depth-scroll")
                                .flex_1()
                                .min_h_0()
                                .overflow_y_scroll()
                                .child(self.structure_depth_panel(cx))
                                .into_any_element()
                        } else {
                            self.structure_paths_table(true, cx).into_any_element()
                        }),
                )
            })
            .into_any_element()
    }

    fn structure_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let mut presets = div().flex().items_center().gap_1();
        for p in CameraPreset::ALL {
            presets = presets.child(
                button(
                    &t,
                    SharedString::from(format!("cam-{}", p.label())),
                    p.label(),
                    false,
                )
                .on_click(cx.listener(move |this, _: &ClickEvent, _w, cx| {
                    this.set_structure_camera(p, cx);
                })),
            );
        }
        let mut styles = div().flex().flex_wrap().items_center().gap_1();
        for (i, style) in AtomStyle::ALL.into_iter().enumerate() {
            if self.structure.molecule_only && style == AtomStyle::Polyhedra {
                continue;
            }
            styles = styles.child(
                chip(
                    &t,
                    SharedString::from(format!("atom-style-{i}")),
                    style.label(),
                    self.structure.atom_style == style,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.structure.atom_style = style;
                    this.rebuild_structure_plot(cx);
                    cx.notify();
                })),
            );
        }
        let has_context = if self.viewing_candidate() {
            self.structure.preview_context.is_some()
        } else {
            self.displayed_source()
                .is_some_and(|s| self.structure.source_contexts.contains_key(&s))
        };
        let mut modes = div().flex().flex_wrap().items_center().gap_1();
        if has_context {
            for (i, label) in ["Crystal + cluster", "Cluster only", "Complete molecule"]
                .into_iter()
                .enumerate()
            {
                modes = modes.child(
                    chip(
                        &t,
                        SharedString::from(format!("context-{i}")),
                        label,
                        if i == 2 {
                            self.structure.molecule_only
                        } else {
                            !self.structure.molecule_only
                                && self.structure.crystal_visible == (i == 0)
                        },
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.structure.crystal_visible = i == 0;
                        this.structure.molecule_only = i == 2;
                        if i == 2 && this.structure.atom_style == AtomStyle::Polyhedra {
                            this.structure.atom_style = AtomStyle::BallStick;
                        }
                        this.structure.camera.zoom = 1.;
                        this.rebuild_structure_plot(cx);
                        cx.notify();
                    })),
                );
            }
        }
        if self.stage_view.fit_step == FitStep::Structure && self.structure.summary.is_some() {
            modes = modes
                .child(div().ml_2().text_size(px(11.)).child("Cluster radius"))
                .child(div().w(px(56.)).child(self.structure.radius.clone()))
                .child("Å");
        }
        styles = styles.child(
            chip(&t, "atom-shading", "Shading", self.structure.shading).on_click(cx.listener(
                |this, _, _, cx| {
                    this.structure.shading = !this.structure.shading;
                    cx.notify();
                },
            )),
        );
        styles = styles.child(
            chip(&t, "atom-labels", "Labels", self.structure.atom_labels).on_click(cx.listener(
                |this, _, _, cx| {
                    this.structure.atom_labels = !this.structure.atom_labels;
                    this.rebuild_structure_plot(cx);
                    cx.notify();
                },
            )),
        );
        styles = styles.child(
            chip(
                &t,
                "slice-depth-controls",
                "Slice & depth",
                self.structure.depth.open || self.structure.depth.options.active(),
            )
            .on_click(cx.listener(|this, _, _, cx| {
                this.structure.depth.open = !this.structure.depth.open;
                cx.notify();
            })),
        );
        if self.structure.molecule_only {
            styles = styles.child(
                chip(
                    &t,
                    "view-hydrogens",
                    "Hydrogens",
                    self.structure.show_hydrogens,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.structure.show_hydrogens = !this.structure.show_hydrogens;
                    this.rebuild_structure_plot(cx);
                    cx.notify();
                })),
            );
        }
        let mut poly_controls = div().flex().flex_col().gap_1();
        let mut display_controls = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_1()
            .text_size(px(11.))
            .child(
                chip(
                    &t,
                    "highlight-absorber",
                    "Highlight absorber",
                    self.structure.highlight_absorber,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.structure.highlight_absorber = !this.structure.highlight_absorber;
                    this.structure.absorber_label = false;
                    cx.notify();
                })),
            )
            .child(
                button(&t, "find-absorber", "Find absorber", false)
                    .on_click(cx.listener(|this, _, _, cx| this.find_structure_absorber(cx))),
            );
        if matches!(
            self.structure.atom_style,
            AtomStyle::BallStick | AtomStyle::Wireframe
        ) {
            use super::bond_geometry::BondMode;
            display_controls = display_controls.child(div().ml_2().child("Bonds"));
            for mode in [
                BondMode::Auto,
                BondMode::Absorber,
                BondMode::AllContacts,
                BondMode::None,
            ] {
                display_controls = display_controls.child(
                    chip(
                        &t,
                        SharedString::from(format!("bond-mode-{}", mode.label())),
                        mode.label(),
                        self.structure.bond_mode == mode,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.structure.bond_mode = mode;
                        this.rebuild_structure_plot(cx);
                        cx.notify();
                    })),
                );
            }
        }
        let mut opacity_controls = div().flex().items_center().gap_1().child(format!(
            "Opacity {:.0}%",
            self.structure.depth.options.opacity * 100.
        ));
        for percent in [100, 75, 50, 25] {
            let value = percent as f64 / 100.;
            opacity_controls = opacity_controls.child(
                chip(
                    &t,
                    ("structure-opacity", percent as usize),
                    format!("{percent}%"),
                    (self.structure.depth.options.opacity - value).abs() < 0.005,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.structure.depth.options.opacity = value;
                    cx.notify();
                })),
            );
        }
        display_controls = display_controls.child(opacity_controls);
        if self.structure.atom_style == AtomStyle::Polyhedra && !self.structure.molecule_only {
            let mut row = div().flex().flex_wrap().items_center().gap_1();
            for (network, label) in [(true, "Repeat by element"), (false, "Selected center")] {
                row = row.child(
                    chip(
                        &t,
                        SharedString::from(format!("poly-network-{network}")),
                        label,
                        self.structure.poly_options.network == network,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.structure.poly_options.network = network;
                        this.rebuild_structure_plot(cx);
                        cx.notify();
                    })),
                );
            }
            for (value, label) in [(0.35, "35%"), (0.65, "65%"), (0.9, "90%"), (1.0, "Solid")] {
                row = row.child(
                    chip(
                        &t,
                        SharedString::from(format!("poly-alpha-{label}")),
                        label,
                        self.structure.poly_options.opacity == value,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.structure.poly_options.opacity = value;
                        this.rebuild_structure_plot(cx);
                        cx.notify();
                    })),
                );
            }
            row = row.child(
                chip(&t, "poly-edges", "Edges", self.structure.poly_options.edges).on_click(
                    cx.listener(|this, _, _, cx| {
                        this.structure.poly_options.edges = !this.structure.poly_options.edges;
                        this.rebuild_structure_plot(cx);
                        cx.notify();
                    }),
                ),
            );
            for (mode, label) in [
                (PolyAtoms::Centers, "Centers"),
                (PolyAtoms::All, "All atoms"),
                (PolyAtoms::None, "No atoms"),
            ] {
                row = row.child(
                    chip(
                        &t,
                        SharedString::from(format!("poly-atoms-{label}")),
                        label,
                        self.structure.poly_options.atoms == mode,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.structure.poly_options.atoms = mode;
                        this.rebuild_structure_plot(cx);
                        cx.notify();
                    })),
                );
            }
            let mut neighbors = div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_1()
                .text_size(px(11.))
                .child("Neighbours");
            let mut elements = vec![(None, "Auto".to_string())];
            if let Some(cluster) = self.displayed_cluster() {
                elements.extend(
                    cluster
                        .element_counts()
                        .into_iter()
                        .map(|(symbol, z, _)| (Some(z), symbol.to_string())),
                );
            }
            for (element, label) in elements {
                neighbors = neighbors.child(
                    chip(
                        &t,
                        SharedString::from(format!("poly-ligand-{label}")),
                        label,
                        self.structure.poly_options.ligand == element,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.structure.poly_options.ligand = element;
                        this.rebuild_structure_plot(cx);
                        cx.notify();
                    })),
                );
            }
            neighbors = neighbors
                .child(div().ml_2().child("Bond limit"))
                .child(
                    div()
                        .w(px(52.))
                        .child(self.structure.poly_cutoff_input.clone()),
                )
                .child("Å");
            neighbors = neighbors.child(div().ml_2().child("Faces"));
            for (color, label) in [(Some(0x70a9ee), "Blue"), (None, "Element")] {
                neighbors = neighbors.child(
                    chip(
                        &t,
                        SharedString::from(format!("poly-color-{label}")),
                        label,
                        self.structure.poly_options.color == color,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.structure.poly_options.color = color;
                        this.rebuild_structure_plot(cx);
                        cx.notify();
                    })),
                );
            }
            poly_controls = poly_controls.child(row).child(neighbors);
        }
        let mut route = div().flex().items_center().flex_wrap().gap_1();
        if !self.viewing_candidate()
            && let Some(p) = self
                .structure
                .selected
                .and_then(|i| self.structure.paths.get(i))
                .and_then(|p| p.as_ref())
        {
            route = route.child(
                chip(
                    &t,
                    "all-path-legs",
                    "All legs",
                    self.structure.path_leg.is_none(),
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.structure.path_leg = None;
                    cx.notify();
                })),
            );
            for (i, pair) in p.polyline().windows(2).enumerate() {
                let label = format!("{} → {}", i + 1, if i + 1 == p.nleg { 1 } else { i + 2 });
                let _ = pair;
                route = route.child(
                    chip(
                        &t,
                        SharedString::from(format!("path-leg-{i}")),
                        label,
                        self.structure.path_leg == Some(i),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.structure.path_leg = Some(i);
                        cx.notify();
                    })),
                );
            }
            route = route.child(button(&t, "focus-path", "Focus path", false).on_click(
                cx.listener(|this, _, _, cx| {
                    if let Some(scene) = &this.structure.scene {
                        let radius = scene
                            .route
                            .iter()
                            .map(|p| p.iter().map(|v| v * v).sum::<f64>().sqrt())
                            .fold(1., f64::max);
                        this.structure.camera.zoom = (scene.extent / radius * 0.85).clamp(0.25, 5.);
                        cx.notify();
                    }
                }),
            ));
        }
        div()
            .flex_none()
            .flex()
            .flex_col()
            .gap_1()
            .child(modes)
            .child(styles)
            .child(display_controls)
            .child(poly_controls)
            .child(route)
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .flex_wrap()
                    .gap_2()
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(t.text_muted)
                            .child("View"),
                    )
                    .child(presets)
                    .child(button(&t, "cam-zoom-out", "−", false).on_click(cx.listener(
                        |this, _: &ClickEvent, _, cx| {
                            this.structure.camera.zoom_by(-1.2_f64.ln());
                            cx.notify();
                        },
                    )))
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(t.text_muted)
                            .child(format!("{:.0}%", self.structure.camera.zoom * 100.)),
                    )
                    .child(button(&t, "cam-zoom-in", "+", false).on_click(cx.listener(
                        |this, _: &ClickEvent, _, cx| {
                            this.structure.camera.zoom_by(1.2_f64.ln());
                            cx.notify();
                        },
                    )))
                    .child(
                        button(&t, "cam-reset", "reset", false).on_click(cx.listener(
                            |this, _: &ClickEvent, _w, cx| {
                                this.structure.camera = ViewCamera::default();
                                cx.notify();
                            },
                        )),
                    )
                    .child(div().w(px(1.)).h(px(18.)).bg(t.border))
                    .child(
                        chip(
                            &t,
                            "st-histogram",
                            "Shell counts",
                            self.structure.show_shell_hist,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.structure.show_shell_hist = !this.structure.show_shell_hist;
                            cx.notify();
                        })),
                    )
                    .child(
                        chip(
                            &t,
                            "st-shells",
                            "colour by shell",
                            self.structure.color_by_shell,
                        )
                        .on_click(cx.listener(
                            |this, _: &ClickEvent, _w, cx| {
                                this.structure.color_by_shell = !this.structure.color_by_shell;
                                this.rebuild_structure_plot(cx);
                                cx.notify();
                            },
                        )),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(t.text_muted)
                            .whitespace_nowrap()
                            .child("Drag rotates · wheel zooms · click inspects"),
                    ),
            )
    }

    fn structure_legend(&self, cluster: &Cluster) -> impl IntoElement + use<> {
        let t = self.theme;
        let mut legend = div()
            .absolute()
            .top_2()
            .left_3()
            .flex()
            .flex_col()
            .gap_0p5()
            .text_size(px(11.))
            .text_color(t.text_muted);
        let mut elements = BTreeMap::new();
        if self.structure.molecule_only
            && let Some(scene) = &self.structure.scene
        {
            for atom in &scene.atoms {
                *elements.entry(atom.z).or_insert(0usize) += 1;
            }
        } else {
            for (_, z, n) in cluster.element_counts() {
                elements.insert(z, n);
            }
        }
        let mut swatches = div().flex().flex_wrap().gap_3();
        for (z, n) in elements {
            let symbol = crate::structure::element_symbol(z);
            let c = cpk_color(z);
            swatches = swatches.child(
                div()
                    .flex()
                    .items_center()
                    .gap_1p5()
                    .child(
                        div()
                            .w(px(9.))
                            .h(px(9.))
                            .rounded_full()
                            .bg(gpui::rgb(c))
                            .border_1()
                            .border_color(t.border),
                    )
                    .child(SharedString::from(format!("{symbol} × {n}"))),
            );
        }
        legend = legend.child(swatches);
        if let Some(frame) = self.structure_depth_frame() {
            use super::structure_depth::{FadeMode, SliceMode};
            if frame.options.slice != SliceMode::Off {
                let [lo, hi] = frame.limits();
                let counts = self
                    .structure
                    .scene
                    .as_ref()
                    .map(|s| {
                        let atoms: Vec<_> = s.atoms.iter().filter(|a| !a.faded).collect();
                        (
                            atoms.iter().filter(|a| frame.contains(a.pos)).count(),
                            atoms.len(),
                        )
                    })
                    .unwrap_or_default();
                legend = legend.child(div().text_color(t.accent).child(format!(
                    "{} · {} {} · retains {} / {} atom centers",
                    if frame.options.slice == SliceMode::Slab {
                        "Slab"
                    } else {
                        "Cutaway"
                    },
                    frame.options.axis.label(),
                    if lo.is_finite() {
                        format!("{lo:.1} to {hi:.1} Å")
                    } else {
                        format!("≤ {hi:.1} Å")
                    },
                    counts.0,
                    counts.1
                )));
                if counts.0 == 0 {
                    legend =
                        legend.child(div().text_color(t.warn).child(
                            "No atom centers in slice · move Position or use Through center.",
                        ));
                }
            }
            if frame.options.fade != FadeMode::Off || frame.options.opacity < 0.999 {
                legend = legend.child(format!(
                    "Opacity {:.0}% · {}",
                    frame.options.opacity * 100.,
                    match frame.options.fade {
                        FadeMode::Off => "uniform transparency",
                        FadeMode::Depth => "far → near: faint → clear",
                        FadeMode::Center => "clear around selected center",
                    }
                ));
            }
        }
        let context = if self.viewing_candidate() {
            self.structure.preview_context.as_ref()
        } else {
            self.displayed_source()
                .and_then(|s| self.structure.source_contexts.get(&s))
        };
        if self.structure.crystal_visible
            && !self.structure.molecule_only
            && let Some(context) = context
        {
            legend = legend.child(format!(
                "{} × {} × {} unit cells · faded atoms outside cluster",
                context.cells[0], context.cells[1], context.cells[2]
            ));
            if context.truncated {
                legend = legend.child("Context limited to 12,000 atoms; FEFF cluster is complete.");
            }
        }
        if let Some(scene) = &self.structure.scene {
            if self.structure.highlight_absorber {
                legend = legend.child(div().text_color(gpui::rgb(0x67e8f9)).child("● Absorber"));
            }
            if matches!(
                self.structure.atom_style,
                AtomStyle::BallStick | AtomStyle::Wireframe
            ) {
                legend = legend.child(format!(
                    "{} · {} displayed bonds",
                    self.structure.bond_mode.label(),
                    scene.bonds.len()
                ));
            }
            if self.structure.highlight_absorber
                && let Some(absorber) = scene.atoms.iter().find(|a| a.absorber)
                && let Some(frame) = self.structure_depth_frame()
                && !frame.contains(absorber.pos)
            {
                legend = legend.child(
                    div()
                        .text_color(gpui::rgb(0x67e8f9))
                        .child("Absorber is outside the slice · use Find absorber."),
                );
            }
            if self.structure.atom_style == AtomStyle::Polyhedra {
                let z = cluster
                    .atoms
                    .get(self.structure.pick.as_ref().map(|p| p.atom).unwrap_or(0))
                    .map(|a| a.z)
                    .unwrap_or(0);
                legend = legend.child(format!(
                    "{} {} coordination polyhedra · click an atom to change center",
                    scene.poly_count,
                    crate::structure::element_symbol(z)
                ));
            }
            if self.structure.molecule_only && scene.message.is_none() {
                legend = legend
                    .child(format!(
                        "Complete molecule · {} displayed atoms · {} bonds",
                        scene.atoms.len(),
                        scene.bonds.len()
                    ))
                    .child("Hydrogen display does not change the FEFF cluster.");
            }
            if let Some(message) = &scene.message {
                legend = legend.child(
                    div()
                        .max_w(px(380.))
                        .text_color(t.warn)
                        .child(message.clone()),
                );
            }
        }
        legend.child(div().mt_1().child(SharedString::from(format!(
            "FEFF cluster · {} atoms · {} shells · {} {:.1} Å",
            cluster.atoms.len(),
            cluster.shells.len().saturating_sub(1),
            if context.is_some() { "cutoff" } else { "outermost atom" },
            self.structure.scene.as_ref().map(|s|s.radius).unwrap_or(0.)
        ))))
    }

    fn structure_pick_card(&self, cluster: &Cluster) -> Option<impl IntoElement + use<>> {
        let t = self.theme;
        let pick = self.structure.pick.as_ref()?;
        let atom = cluster.atoms.get(pick.atom)?;
        let in_paths: Vec<String> = self
            .structure
            .paths
            .iter()
            .enumerate()
            .filter_map(|(i, p)| p.as_ref().map(|p| (i, p)))
            .filter(|_| !self.viewing_candidate())
            .filter(|(_, p)| {
                p.atom_indices(cluster)
                    .iter()
                    .flatten()
                    .any(|&a| a == pick.atom)
            })
            .map(|(i, p)| format!("{} {} ({:.2} Å)", i + 1, p.label(), p.reff))
            .collect();
        let mut card = div()
            .absolute()
            .bottom_2()
            .left_3()
            .px_2()
            .py_1()
            .rounded_md()
            .bg(t.surface)
            .border_1()
            .border_color(t.border)
            .text_size(px(11.))
            .text_color(t.text)
            .flex()
            .flex_col()
            .gap_0p5()
            .child(SharedString::from(format!(
                "{} · {:.3} Å · shell {} · ipot {}",
                atom.symbol, atom.dist, atom.shell, atom.ipot
            )))
            .child(
                div()
                    .text_color(t.text_muted)
                    .child(SharedString::from(format!(
                        "({:.3}, {:.3}, {:.3})",
                        atom.pos[0], atom.pos[1], atom.pos[2]
                    ))),
            );
        if in_paths.is_empty() {
            card = card.child(div().text_color(t.text_muted).child("in no imported path"));
        } else {
            let shown: Vec<String> = in_paths.iter().take(4).cloned().collect();
            let more = in_paths.len().saturating_sub(4);
            card = card.child(
                div()
                    .text_color(t.text_muted)
                    .child(SharedString::from(format!(
                        "paths: {}{}",
                        shown.join(" · "),
                        if more > 0 {
                            format!(" · +{more}")
                        } else {
                            String::new()
                        }
                    ))),
            );
        }
        Some(card)
    }

    // ---- path table ---------------------------------------------------------

    /// Path table with columns, filters and multi-select. `docked` = the
    /// full-height variant beside the 3D view; otherwise the compact form
    /// used at the top of the inspector's Paths section.
    /// The path table beside the 3D view (and the compact one in the
    /// inspector): the shell-grouped picker.
    pub(crate) fn structure_paths_table(
        &self,
        docked: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        self.path_picker(docked, cx)
    }

    // ---- inspector: structure panel ------------------------------------------

    fn spectrum_interest(&self) -> Option<crate::spectrum_interest::SpectrumInterest> {
        let header = self.import_preview.as_ref().and_then(|p| p.xdi.as_ref());
        let e0 = (self.spectrum_path == self.current_path)
            .then(|| self.spectrum.as_ref().and_then(|s| s.get_e0()))
            .flatten();
        crate::spectrum_interest::SpectrumInterest::infer(header, e0)
    }

    pub(crate) fn structure_library_panel(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let t = self.theme;
        let st = &self.structure;
        let interest = self.spectrum_interest();
        let label_w = px(64.);
        let row_label = |text: &'static str| {
            div()
                .w(label_w)
                .flex_none()
                .text_size(px(12.))
                .text_color(t.text_muted)
                .child(text)
        };
        let muted_mono = |text: String| {
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .font_family(MONO)
                .text_size(px(11.))
                .text_color(t.text_muted)
                .child(SharedString::from(text))
        };
        let mut sources = segmented(&t);
        for (i, k) in StructureSourceKind::ALL.into_iter().enumerate() {
            sources = sources.child(
                segment(
                    &t,
                    SharedString::from(format!("src-{}", k.badge())),
                    k.label(),
                    st.source == k,
                    i == 0,
                )
                .on_click(cx.listener(move |this, _: &ClickEvent, _w, cx| {
                    this.structure.source = k;
                    this.structure.hits.clear();
                    this.structure.search_error = None;
                    this.structure_search(cx);
                })),
            );
        }
        let mut panel = div()
            .flex()
            .flex_col()
            .gap_1p5()
            .px_3()
            .child(div().flex().flex_wrap().child(sources));

        // Per-source configuration.
        match st.source {
            StructureSourceKind::Builtin => {
                panel = panel.child(
                    div()
                        .text_size(px(11.))
                        .text_color(t.text_muted)
                        .child("Curated reference structures · available offline"),
                );
                if let Some(hint) = &interest {
                    panel = panel
                        .child(
                            div()
                                .text_size(px(11.))
                                .text_color(t.text_muted)
                                .child(hint.label()),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_wrap()
                                .gap_1()
                                .child(
                                    chip(
                                        &t,
                                        "library-interest",
                                        format!("Contains {}", hint.element),
                                        st.filter_to_spectrum,
                                    )
                                    .on_click(cx.listener(
                                        |this, _, _, cx| {
                                            this.structure.filter_to_spectrum = true;
                                            cx.notify();
                                        },
                                    )),
                                )
                                .child(
                                    chip(
                                        &t,
                                        "library-all-elements",
                                        "All materials",
                                        !st.filter_to_spectrum,
                                    )
                                    .on_click(cx.listener(
                                        |this, _, _, cx| {
                                            this.structure.filter_to_spectrum = false;
                                            cx.notify();
                                        },
                                    )),
                                ),
                        );
                }
                let mut categories = div().flex().flex_wrap().gap_1();
                for (category, label) in [
                    (None, "All types"),
                    (Some("metal"), "Metals"),
                    (Some("oxide"), "Oxides"),
                    (Some("sulfide"), "Sulfides"),
                    (Some("molecule"), "Molecules"),
                    (Some("other"), "Other"),
                ] {
                    categories = categories.child(
                        chip(
                            &t,
                            SharedString::from(format!("catalog-{label}")),
                            label,
                            st.category == category,
                        )
                        .on_click(cx.listener(
                            move |this, _: &ClickEvent, _w, cx| {
                                this.structure.category = category;
                                cx.notify();
                            },
                        )),
                    );
                }
                panel = panel.child(categories);
            }
            StructureSourceKind::LocalCif => {
                let folder = st
                    .settings
                    .cif_library
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "no folder chosen".to_string());
                let count = st
                    .cif_library
                    .as_ref()
                    .map(|l| format!(" · {} files", l.len()))
                    .unwrap_or_default();
                panel = panel.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .child(row_label("folder"))
                        .child(muted_mono(format!("{folder}{count}")))
                        .child(button(&t, "cif-folder", "Choose folder…", false).on_click(
                            cx.listener(|this, _: &ClickEvent, _w, cx| {
                                this.structure_choose_cif_folder(cx)
                            }),
                        )),
                );
            }
            StructureSourceKind::MaterialsProject => {
                let key = st.mp_key_text(cx);
                let masked: SharedString = if key.is_empty() {
                    "not set — free key at materialsproject.org/api".into()
                } else {
                    format!(
                        "{}••••••••{}",
                        &key[..key.len().min(3)],
                        &key[key.len().saturating_sub(2)..]
                    )
                    .into()
                };
                let mut key_row = div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(row_label("API key"));
                if st.mp_key_editing {
                    key_row = key_row.child(div().flex_1().child(st.mp_key.clone()));
                } else {
                    key_row = key_row.child(muted_mono(masked.to_string())).child(
                        button(&t, "mp-key-edit", "change", false).on_click(cx.listener(
                            |this, _: &ClickEvent, _w, cx| {
                                this.structure.mp_key_editing = true;
                                cx.notify();
                            },
                        )),
                    );
                }
                panel = panel.child(key_row);
            }
            StructureSourceKind::Amcsd => {
                let db = st
                    .settings
                    .amcsd_db
                    .as_ref()
                    .filter(|p| p.is_file())
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "database not downloaded".to_string());
                let mut row = div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(row_label("database"))
                    .child(muted_mono(db));
                if st.download_cancel.is_some() {
                    let text = match st.download_progress {
                        Some((got, Some(total))) if total > 0 => {
                            format!("{:.0} %", 100.0 * got as f64 / total as f64)
                        }
                        Some((got, _)) => format!("{:.0} MB", got as f64 / 1e6),
                        None => "starting…".to_string(),
                    };
                    row = row
                        .child(
                            div()
                                .font_family(MONO)
                                .text_size(px(11.))
                                .text_color(t.accent)
                                .child(SharedString::from(text)),
                        )
                        .child(
                            button(&t, "amcsd-cancel", "Cancel", false).on_click(cx.listener(
                                |this, _: &ClickEvent, _w, cx| this.structure_cancel_download(cx),
                            )),
                        );
                } else {
                    row = row
                        .child(
                            button(&t, "amcsd-choose", "Choose…", false).on_click(cx.listener(
                                |this, _: &ClickEvent, _w, cx| this.structure_choose_amcsd(cx),
                            )),
                        )
                        .child(button(&t, "amcsd-download", "Download…", false).on_click(
                            cx.listener(|this, _: &ClickEvent, _w, cx| {
                                this.structure_download_amcsd(cx)
                            }),
                        ));
                }
                panel = panel.child(row);
                panel = panel.child(credit_line(
                    &t,
                    "American Mineralogist Crystal Structure Database (MSA / MAC, NSF). Cite \
                     Downs & Hall-Wallace (2003) Am. Mineral. 88, 247–250 · rruff.geo.arizona.edu/AMS \
                     · SQLite packaging by larixite (xraypy)",
                ));
            }
            StructureSourceKind::Cod => {
                panel = panel.child(credit_line(
                    &t,
                    "Crystallography Open Database · crystallography.net/cod · data in the public \
                     domain (CC0). Cite Gražulis et al. (2012) Nucleic Acids Res. 40, D420 and \
                     Vaitkus et al. (2021) J. Appl. Cryst. 54, 661",
                ));
            }
        }

        // Search row (Import CIF… works for every source).
        let busy = st.search_running || st.cif_scanning || st.fetch_running;
        panel = panel.child(
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(div().flex_1().min_w_0().child(st.search.clone()))
                .child(
                    button(
                        &t,
                        "st-search",
                        if st.cif_scanning {
                            "scanning…"
                        } else if st.search_running {
                            "searching…"
                        } else {
                            "Search"
                        },
                        !busy,
                    )
                    .on_click(
                        cx.listener(|this, _: &ClickEvent, _w, cx| this.structure_search(cx)),
                    ),
                )
                .child(
                    button(&t, "st-import", "Import CIF / XYZ…", false).on_click(
                        cx.listener(|this, _: &ClickEvent, _w, cx| this.structure_import_cif(cx)),
                    ),
                ),
        );
        if let Some(err) = &st.search_error {
            panel = panel.child(
                div()
                    .text_size(px(11.))
                    .text_color(t.warn)
                    .child(SharedString::from(err.clone())),
            );
        }
        if st.hits.is_empty() && !busy && st.search_error.is_none() {
            panel = panel.child(
                div()
                    .py_3()
                    .text_color(t.text_muted)
                    .child("No structures found. Try an element, formula, or mineral name."),
            );
        }
        if !st.hits.is_empty() {
            let mut list = div()
                .id("st-hits")
                .max_h(px(520.))
                .overflow_y_scroll()
                .rounded_md()
                .border_1()
                .border_color(t.border)
                .flex()
                .flex_col();
            let mut shown = 0;
            for (i, hit) in st.hits.iter().enumerate() {
                if st.source == StructureSourceKind::Builtin
                    && (!crate::structure::matches_category(hit, st.category)
                        || (st.filter_to_spectrum
                            && interest.as_ref().is_some_and(|hint| {
                                !crate::spectrum_interest::contains_element(hit, &hint.element)
                            })))
                {
                    continue;
                }
                shown += 1;
                let chosen = st
                    .summary
                    .as_ref()
                    .is_some_and(|s| s.hit.id == hit.id && s.hit.source == hit.source);
                list = list.child(
                    div()
                        .id(("st-hit", i))
                        .h(px(46.))
                        .border_b_1()
                        .border_color(t.border)
                        .flex_none()
                        .px_2()
                        .flex()
                        .items_center()
                        .gap_2()
                        .cursor_pointer()
                        .when(chosen, |d| {
                            d.bg(t.raised).border_l_2().border_color(t.accent)
                        })
                        .hover(|d| d.bg(t.raised))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _w, cx| {
                            this.structure_choose(i, cx)
                        }))
                        .child(
                            div()
                                .w(px(72.))
                                .flex_none()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .child(SharedString::from(hit.formula.clone())),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .overflow_hidden()
                                .text_ellipsis()
                                .whitespace_nowrap()
                                .text_color(t.text_muted)
                                .child(SharedString::from(
                                    [hit.name.as_str(), hit.space_group.as_str()]
                                        .iter()
                                        .filter(|x| !x.is_empty())
                                        .copied()
                                        .collect::<Vec<_>>()
                                        .join(" · "),
                                )),
                        )
                        .child(
                            div()
                                .px_1()
                                .rounded_sm()
                                .border_1()
                                .border_color(t.border)
                                .text_size(px(10.))
                                .text_color(t.text_muted)
                                .child(hit.source.badge()),
                        ),
                );
            }
            let count = format!("{} result{}", shown, if shown == 1 { "" } else { "s" });
            if shown == 0 {
                panel = panel.child(div().py_2().text_color(t.text_muted).child("No matches for these filters. Choose All materials or All types, or import your own CIF / XYZ."));
            }
            panel = panel.child(list).child(
                div()
                    .text_size(px(10.5))
                    .text_color(t.text_muted)
                    .child(SharedString::from(count)),
            );
        }
        panel
    }

    pub(crate) fn structure_panel(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        if self.stage_view.fit_step == FitStep::Structure {
            self.structure_library_panel(cx).into_any_element()
        } else {
            self.structure_calculation_panel(cx).into_any_element()
        }
    }

    fn structure_calculation_panel(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let st = &self.structure;
        let row_label = |text: &'static str| {
            div()
                .w(px(64.))
                .flex_none()
                .text_size(px(12.))
                .text_color(t.text_muted)
                .child(text)
        };
        let mut panel = div()
            .flex()
            .flex_col()
            .gap_3()
            .px_3()
            .child(section_label(&t, crate::feffgen::backend_name(st.backend)))
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(t.text_muted)
                    .child("Runs locally. Generated paths are grouped into shells for selection."),
            );
        let mut engines = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_1()
            .child(row_label("engine"));
        let available = [
            #[cfg(feature = "refeff-runner")]
            rexafs::prelude::FeffExecutionMode::RefeffPipeline,
            #[cfg(feature = "feff10-runner")]
            rexafs::prelude::FeffExecutionMode::Feff10Pipeline,
        ];
        for (i, mode) in available.into_iter().enumerate() {
            engines = engines.child(
                chip(
                    &t,
                    SharedString::from(format!("feff-engine-{i}")),
                    crate::feffgen::backend_name(mode),
                    st.backend == mode,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    if !this.feff_running {
                        this.structure.backend = mode;
                        cx.notify();
                    }
                })),
            );
        }
        panel = panel.child(engines);
        if !self.fit_paths.is_empty() {
            panel=panel.child(div().text_size(px(11.)).text_color(t.text_muted)
            .child(format!("New calculations add a source. {} existing paths and their parameter edits are kept.",self.fit_paths.len())));
        }
        if let Some(error) = &st.search_error {
            panel = panel.child(div().text_color(t.warn).child(error.clone()));
        }
        if let Some(s) = &st.summary {
            let l = s.lattice;
            let sites = s
                .sites
                .iter()
                .map(|x| format!("{} ×{}", x.label, x.multiplicity))
                .collect::<Vec<_>>()
                .join("  ");
            let n_atoms: usize = s.sites.iter().map(|x| x.multiplicity).sum();
            panel = panel.child(
                div()
                    .rounded_md()
                    .border_1()
                    .border_color(t.border)
                    .bg(t.raised)
                    .px_2()
                    .py_1p5()
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .text_size(px(11.5))
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .items_baseline()
                            .child(
                                div()
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .child(SharedString::from(s.formula())),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .text_color(t.text_muted)
                                    .child(SharedString::from(s.hit.name.clone())),
                            )
                            .child(
                                div()
                                    .px_1()
                                    .rounded_sm()
                                    .border_1()
                                    .border_color(t.border)
                                    .text_size(px(10.))
                                    .text_color(t.text_muted)
                                    .child(s.hit.source.badge()),
                            ),
                    )
                    .child(
                        div()
                            .text_color(t.text_muted)
                            .child(SharedString::from(s.space_group())),
                    )
                    .child(mono_line(
                        &t,
                        format!("a {:.4}  b {:.4}  c {:.4} Å", l[0], l[1], l[2]),
                    ))
                    .child(mono_line(
                        &t,
                        format!("α {:.2}°  β {:.2}°  γ {:.2}°", l[3], l[4], l[5]),
                    ))
                    .child(mono_line(
                        &t,
                        format!("{n_atoms} atoms / cell · sites  {sites}"),
                    )),
            );
            // Absorber: element chips, then the element's distinct sites.
            let mut abs_row = div()
                .flex()
                .items_center()
                .flex_wrap()
                .gap_1()
                .child(row_label("absorber"));
            for (sym, z) in s.elements() {
                let on = st.absorber.as_deref() == Some(sym.as_str());
                let mult: usize = s.sites_of(&sym).iter().map(|x| x.multiplicity).sum();
                let sym2 = sym.clone();
                abs_row = abs_row.child(
                    chip(
                        &t,
                        SharedString::from(format!("abs-{sym}")),
                        format!("{sym} (Z {z}, ×{mult})"),
                        on,
                    )
                    .on_click(cx.listener(
                        move |this, _: &ClickEvent, _w, cx| {
                            this.structure.absorber = Some(sym2.clone());
                            this.structure.absorber_site = None;
                            this.preview_structure(cx);
                            cx.notify();
                        },
                    )),
                );
            }
            panel = panel.child(abs_row);
            if let Some(abs) = &st.absorber {
                let indexed: Vec<(usize, &crate::structure::SiteSummary)> = s
                    .sites
                    .iter()
                    .enumerate()
                    .filter(|(_, x)| &x.symbol == abs)
                    .collect();
                if indexed.len() > 1 {
                    let mut site_row = div()
                        .flex()
                        .items_center()
                        .flex_wrap()
                        .gap_1()
                        .child(row_label("site"));
                    site_row = site_row.child(
                        chip(&t, "abs-site-all", "first site", st.absorber_site.is_none())
                            .on_click(cx.listener(|this, _: &ClickEvent, _w, cx| {
                                this.structure.absorber_site = None;
                                this.preview_structure(cx);
                                cx.notify();
                            })),
                    );
                    for (i, site) in indexed {
                        site_row = site_row.child(
                            chip(
                                &t,
                                SharedString::from(format!("abs-site-{i}")),
                                format!("{} ×{}", site.label, site.multiplicity),
                                st.absorber_site == Some(i),
                            )
                            .on_click(cx.listener(
                                move |this, _: &ClickEvent, _w, cx| {
                                    this.structure.absorber_site = Some(i);
                                    this.preview_structure(cx);
                                    cx.notify();
                                },
                            )),
                        );
                    }
                    panel = panel.child(site_row);
                }
            }
            let mut edge_row = div().flex().items_center().gap_1().child(row_label("edge"));
            for e in ["K", "L1", "L2", "L3"] {
                edge_row = edge_row.child(
                    chip(&t, SharedString::from(format!("edge-{e}")), e, st.edge == e).on_click(
                        cx.listener(move |this, _: &ClickEvent, _w, cx| {
                            this.structure.edge = e.to_string();
                            cx.notify();
                        }),
                    ),
                );
            }
            panel = panel.child(edge_row).child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(row_label("cluster"))
                    .child(div().w(px(72.)).flex_none().child(st.radius.clone()))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .text_size(px(11.))
                            .text_color(t.text_muted)
                            .child("Å radius · 2–12 Å"),
                    ),
            );
        }
        if st.summary.is_none() {
            panel = panel.child(
                div()
                    .text_color(t.text_muted)
                    .child("Choose a structure in step 1, or open a custom feff.inp below."),
            );
        }
        if let Some(summary) = self.structure_cluster_summary() {
            panel = panel.child(
                div()
                    .text_size(px(10.5))
                    .text_color(t.text_muted)
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .child(SharedString::from(summary)),
            );
        }
        panel = panel.child(
            div()
                .text_size(px(10.5))
                .text_color(t.text_muted)
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .child(SharedString::from(match &self.feff_workspace {
                    Some(ws) => format!("workspace: {}", ws.display()),
                    None => "no FEFF workspace yet".to_string(),
                })),
        );
        panel
    }
}

/// Build a synthetic cluster from path legs when no feff.inp is available.
fn cluster_from_paths(paths: &[Option<PathGeometry>]) -> Option<Cluster> {
    let mut text = String::from("TITLE paths\nPOTENTIALS\n");
    let mut pots: Vec<(usize, u32, String)> = Vec::new();
    let mut atoms: Vec<([f64; 3], usize, String)> = Vec::new();
    for p in paths.iter().flatten() {
        for leg in &p.legs {
            if !pots.iter().any(|(i, _, _)| *i == leg.ipot) {
                pots.push((leg.ipot, leg.z, leg.symbol.clone()));
            }
            let dup = atoms.iter().any(|(pos, _, _)| {
                (pos[0] - leg.pos[0]).abs() < 0.02
                    && (pos[1] - leg.pos[1]).abs() < 0.02
                    && (pos[2] - leg.pos[2]).abs() < 0.02
            });
            if !dup {
                atoms.push((leg.pos, leg.ipot, leg.symbol.clone()));
            }
        }
    }
    if atoms.is_empty() {
        return None;
    }
    pots.sort_by_key(|p| p.0);
    for (i, z, s) in &pots {
        text.push_str(&format!(" {i} {z} {s}\n"));
    }
    text.push_str("ATOMS\n");
    for (pos, ipot, sym) in &atoms {
        text.push_str(&format!(
            " {:.5} {:.5} {:.5} {} {}\n",
            pos[0], pos[1], pos[2], ipot, sym
        ));
    }
    text.push_str("END\n");
    let c = crate::structure::parse_feff_inp(&text);
    (!c.is_empty()).then_some(c)
}

fn mono_line(t: &Theme, text: String) -> gpui::Div {
    div()
        .font_family(MONO)
        .text_size(px(11.))
        .text_color(t.text)
        .child(SharedString::from(text))
}

/// `https://docs.xrayabsorption.org/databases/amcsd_cif2.db` → `docs.xrayabsorption.org`.
fn mirror_host(url: &str) -> String {
    url.split("://")
        .nth(1)
        .unwrap_or(url)
        .split('/')
        .next()
        .unwrap_or(url)
        .to_string()
}

/// One-line attribution shown under an online source.
fn credit_line(t: &Theme, text: &'static str) -> gpui::Div {
    div()
        .text_size(px(10.5))
        .text_color(t.text_muted)
        .child(SharedString::from(text))
}
