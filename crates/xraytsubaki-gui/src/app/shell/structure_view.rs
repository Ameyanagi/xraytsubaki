//! Fit stage · structure database, cluster / path visualizer, path selection.
//!
//! The 3D view (ruviz-gpui `RuvizPlot3D`) shows the FEFF cluster: atoms as
//! CPK-coloured markers sized by covalent radius, the absorber at the
//! origin, shells by distance, and the selected path as a closed polyline.
//! Atoms come from the workspace `feff.inp`, legs from every path's
//! `feffNNNN.dat` (see [`crate::structure`]). The structure panel talks to
//! [`crate::structure::StructureProvider`] — the seam the core crate's
//! `xafs::structure` sources plug into.

use std::collections::BTreeSet;
use std::path::PathBuf;

use gpui::{
    ClickEvent, Context, Entity, IntoElement, ParentElement, SharedString, Styled, div, prelude::*,
    px,
};
use ruviz::core::{Camera3D, CameraView3D, PickHit3D, PickPrimitive3D, Point3D};
use ruviz::render::Color as PlotColor;
use ruviz_gpui::{Plot3DEvent, RuvizPlot, RuvizPlot3D, plot_builder, plot3d_builder};

use super::{MONO, button, chip, section_label, segment, segmented};
use crate::app::StudioApp;
use crate::structure::{
    Cluster, PathGeometry, StructureHit, StructureSourceKind, StructureSummary, covalent_radius,
    cpk_color, load_cluster, load_path_geometry, normalise_importance, provider_for,
};
use crate::theme::Theme;
use crate::widgets::text_input::{InputEvent, InputStyle, TextInput};

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
    fn view(self) -> CameraView3D {
        match self {
            CameraPreset::Isometric => CameraView3D::Isometric,
            CameraPreset::DownC => CameraView3D::Top,
            CameraPreset::DownA => CameraView3D::Left,
            CameraPreset::DownB => CameraView3D::Front,
        }
    }
}

/// A 3D polyline: x, y, z, colour, width, legend label.
type LineSpec = (Vec<f64>, Vec<f64>, Vec<f64>, PlotColor, f32, Option<String>);

/// What a 3D series maps back to, for picking.
enum SeriesTarget {
    /// Scatter series: point index → cluster atom index.
    Atoms(Vec<usize>),
    /// Line series (bonds, path polyline): no pick target.
    Decor,
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
    // ---- cluster + paths ----
    pub cluster: Option<Cluster>,
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
    pub plot3d: Option<Entity<RuvizPlot3D>>,
    series_map: Vec<SeriesTarget>,
    pub hist: Option<Entity<RuvizPlot>>,
    pub color_by_shell: bool,
    pub show_bonds: bool,
    pub pick: Option<AtomPick>,
    pub plot_error: Option<String>,
    // ---- structure panel ----
    pub source: StructureSourceKind,
    pub search: Entity<TextInput>,
    pub hits: Vec<StructureHit>,
    pub search_error: Option<String>,
    pub summary: Option<StructureSummary>,
    pub absorber: Option<String>,
    pub edge: String,
    pub radius: Entity<TextInput>,
    pub mp_key: Entity<TextInput>,
    pub mp_key_editing: bool,
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
        let radius = cx.new(|cx| TextInput::new("rmax", "6.0", theme, cx).with_style(mono));
        let max_reff_input = cx.new(|cx| TextInput::new("max R", "", theme, cx).with_style(mono));
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
            if let InputEvent::Committed(_) = event {
                this.structure.mp_key_editing = false;
                cx.notify();
            }
        })
        .detach();
        Self {
            cluster: None,
            cluster_workspace: None,
            paths: Vec::new(),
            selected: None,
            hovered: None,
            multi: BTreeSet::new(),
            filters: PathFilters::default(),
            max_reff_input,
            show: false,
            plot3d: None,
            series_map: Vec::new(),
            hist: None,
            color_by_shell: false,
            show_bonds: false,
            pick: None,
            plot_error: None,
            source: StructureSourceKind::Builtin,
            search,
            hits: Vec::new(),
            search_error: None,
            summary: None,
            absorber: None,
            edge: "K".to_string(),
            radius,
            mp_key,
            mp_key_editing: false,
        }
    }

    pub(crate) fn mp_key_text(&self, cx: &gpui::App) -> String {
        self.mp_key.read(cx).text().trim().to_string()
    }
}

/// Shell colour ramp: near shells warm, far shells cool.
fn shell_color(shell: usize, n_shells: usize) -> PlotColor {
    let t = if n_shells <= 1 {
        0.0
    } else {
        shell as f64 / (n_shells - 1) as f64
    };
    // amber → teal → violet
    let (r, g, b) = if t < 0.5 {
        let u = t * 2.0;
        (
            (232.0 + (25.0 - 232.0) * u) as u8,
            (160.0 + (158.0 - 160.0) * u) as u8,
            (60.0 + (112.0 - 60.0) * u) as u8,
        )
    } else {
        let u = (t - 0.5) * 2.0;
        (
            (25.0 + (144.0 - 25.0) * u) as u8,
            (158.0 + (133.0 - 158.0) * u) as u8,
            (112.0 + (233.0 - 112.0) * u) as u8,
        )
    };
    PlotColor::from_rgb(r, g, b)
}

fn rgb(hex: u32) -> PlotColor {
    PlotColor::from_rgb(
        ((hex >> 16) & 0xff) as u8,
        ((hex >> 8) & 0xff) as u8,
        (hex & 0xff) as u8,
    )
}

fn theme_rgb(c: gpui::Rgba) -> PlotColor {
    PlotColor::from_rgb(
        (c.r * 255.0) as u8,
        (c.g * 255.0) as u8,
        (c.b * 255.0) as u8,
    )
}

fn marker_px(z: u32) -> f32 {
    5.0 + covalent_radius(z) * 5.0
}

impl StudioApp {
    // ---- data ------------------------------------------------------------

    /// Reload the cluster / path geometries when the FEFF workspace or the
    /// path list changed, then refresh the 3D view. Cheap when nothing moved.
    pub(crate) fn refresh_structure(&mut self, cx: &mut Context<Self>) {
        let ws = self.feff_workspace.clone();
        let mut changed = false;
        if ws != self.structure.cluster_workspace {
            self.structure.cluster = ws.as_deref().and_then(load_cluster);
            self.structure.cluster_workspace = ws;
            changed = true;
        }
        let files: Vec<PathBuf> = self.fit_paths.iter().map(|r| r.spec.file.clone()).collect();
        let same = self.structure.paths.len() == files.len()
            && self
                .structure
                .paths
                .iter()
                .zip(files.iter())
                .all(|(p, f)| p.as_ref().is_some_and(|p| &p.file == f));
        if !same {
            let mut paths: Vec<Option<PathGeometry>> =
                files.iter().map(|f| load_path_geometry(f)).collect();
            normalise_importance(&mut paths);
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

    /// Indices of `fit_paths` that pass the filters, in table order.
    pub(crate) fn structure_filtered_paths(&self) -> Vec<usize> {
        let f = &self.structure.filters;
        (0..self.fit_paths.len())
            .filter(|&i| {
                let Some(Some(p)) = self.structure.paths.get(i) else {
                    return true;
                };
                if f.single_scattering && !p.is_single_scattering() {
                    return false;
                }
                if f.max_legs.is_some_and(|m| p.nleg > m) {
                    return false;
                }
                if f.max_reff.is_some_and(|m| p.reff > m) {
                    return false;
                }
                p.importance >= f.min_importance
            })
            .collect()
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

    fn set_paths_enabled(&mut self, indices: &[usize], enabled: bool, cx: &mut Context<Self>) {
        for &i in indices {
            if let Some(row) = self.fit_paths.get_mut(i) {
                row.spec.enabled = enabled;
            }
        }
        self.fit_model_changed(cx);
    }

    // ---- 3D plot ---------------------------------------------------------

    pub(crate) fn rebuild_structure_plot(&mut self, cx: &mut Context<Self>) {
        let Some(cluster) = self.structure.cluster.clone() else {
            self.structure.plot3d = None;
            self.structure.hist = None;
            self.structure.series_map.clear();
            return;
        };
        let theme = self.theme;
        let n_shells = cluster.shells.len().max(1);
        let selected = self
            .structure
            .selected
            .and_then(|i| self.structure.paths.get(i).cloned().flatten());
        let highlight_atoms: BTreeSet<usize> = selected
            .as_ref()
            .map(|p| p.atom_indices(&cluster).into_iter().flatten().collect())
            .unwrap_or_default();

        // Group atoms into series: by element (CPK) or by shell (ramp).
        struct Group {
            label: String,
            color: PlotColor,
            size: f32,
            atoms: Vec<usize>,
        }
        let mut groups: Vec<Group> = Vec::new();
        for (i, a) in cluster.atoms.iter().enumerate() {
            if a.ipot == 0 || highlight_atoms.contains(&i) {
                continue;
            }
            let (key, color, size) = if self.structure.color_by_shell {
                (
                    format!("shell {} · {:.2} Å", a.shell, cluster.shells[a.shell].0),
                    shell_color(a.shell, n_shells),
                    marker_px(a.z) * 0.9,
                )
            } else {
                (a.symbol.clone(), rgb(cpk_color(a.z)), marker_px(a.z))
            };
            match groups.iter_mut().find(|g| g.label == key) {
                Some(g) => g.atoms.push(i),
                None => groups.push(Group {
                    label: key,
                    color,
                    size,
                    atoms: vec![i],
                }),
            }
        }
        let mut series_map: Vec<SeriesTarget> = Vec::new();
        let xyz = |ids: &[usize]| -> (Vec<f64>, Vec<f64>, Vec<f64>) {
            let mut x = Vec::with_capacity(ids.len());
            let mut y = Vec::with_capacity(ids.len());
            let mut z = Vec::with_capacity(ids.len());
            for &i in ids {
                let p = cluster.atoms[i].pos;
                x.push(p[0]);
                y.push(p[1]);
                z.push(p[2]);
            }
            (x, y, z)
        };

        // Absorber first so it is always series 0.
        let absorber_ids: Vec<usize> = cluster
            .atoms
            .iter()
            .enumerate()
            .filter(|(_, a)| a.ipot == 0)
            .map(|(i, _)| i)
            .collect();
        let (ax, ay, az) = xyz(&absorber_ids);
        let absorber_label = cluster
            .absorber()
            .map(|a| format!("{} (absorber)", a.symbol))
            .unwrap_or_else(|| "absorber".into());
        let mut sb = ruviz::scatter3d(&ax, &ay, &az)
            .color(theme_rgb(theme.accent))
            .marker_size(
                cluster
                    .absorber()
                    .map(|a| marker_px(a.z) * 1.5)
                    .unwrap_or(10.0),
            )
            .label(absorber_label)
            .theme(theme.plot_theme())
            .xlabel("x (Å)")
            .ylabel("y (Å)")
            .zlabel("z (Å)");
        series_map.push(SeriesTarget::Atoms(absorber_ids));

        for g in &groups {
            let (x, y, z) = xyz(&g.atoms);
            sb = sb
                .scatter3d(&x, &y, &z)
                .color(g.color)
                .marker_size(g.size)
                .label(g.label.clone());
            series_map.push(SeriesTarget::Atoms(g.atoms.clone()));
        }
        if !highlight_atoms.is_empty() {
            let ids: Vec<usize> = highlight_atoms.iter().copied().collect();
            let (x, y, z) = xyz(&ids);
            let size = ids
                .iter()
                .map(|&i| marker_px(cluster.atoms[i].z))
                .fold(0.0_f32, f32::max)
                * 1.6;
            sb = sb
                .scatter3d(&x, &y, &z)
                .color(theme_rgb(theme.warn))
                .marker_size(size)
                .label("selected path");
            series_map.push(SeriesTarget::Atoms(ids));
        }

        // Lines: bonds (first shell only, optional) then the path polyline.
        let mut lines: Vec<LineSpec> = Vec::new();
        if self.structure.show_bonds {
            let cutoff = cluster.shells.get(1).map(|s| s.0 * 1.15).unwrap_or(0.0);
            let mut count = 0usize;
            'outer: for (i, a) in cluster.atoms.iter().enumerate() {
                for b in cluster.atoms.iter().skip(i + 1) {
                    let d = ((a.pos[0] - b.pos[0]).powi(2)
                        + (a.pos[1] - b.pos[1]).powi(2)
                        + (a.pos[2] - b.pos[2]).powi(2))
                    .sqrt();
                    if d > 0.5 && d <= cutoff {
                        lines.push((
                            vec![a.pos[0], b.pos[0]],
                            vec![a.pos[1], b.pos[1]],
                            vec![a.pos[2], b.pos[2]],
                            theme_rgb(theme.text_muted),
                            1.0,
                            None,
                        ));
                        count += 1;
                        if count >= 240 {
                            break 'outer;
                        }
                    }
                }
            }
        }
        if let Some(p) = &selected {
            let pts = p.polyline();
            lines.push((
                pts.iter().map(|q| q[0]).collect(),
                pts.iter().map(|q| q[1]).collect(),
                pts.iter().map(|q| q[2]).collect(),
                theme_rgb(theme.warn),
                3.0,
                Some(format!("{} · {:.3} Å", p.label(), p.reff)),
            ));
        }

        let plot = if lines.is_empty() {
            sb
        } else {
            let mut lb = {
                let (x, y, z, c, w, label) = lines.remove(0);
                let b = sb.line3d(&x, &y, &z).color(c).line_width(w);
                let b = match label {
                    Some(l) => b.label(l),
                    None => b,
                };
                series_map.push(SeriesTarget::Decor);
                b
            };
            for (x, y, z, c, w, label) in lines {
                lb = lb.line3d(&x, &y, &z).color(c).line_width(w);
                if let Some(l) = label {
                    lb = lb.label(l);
                }
                series_map.push(SeriesTarget::Decor);
            }
            // Return to a scatter builder type by appending an empty absorber
            // marker series is not possible; keep the line builder instead.
            self.structure.series_map = series_map;
            self.install_structure_plot(lb, cx);
            self.rebuild_structure_hist(&cluster, cx);
            return;
        };
        self.structure.series_map = series_map;
        self.install_structure_plot(plot, cx);
        self.rebuild_structure_hist(&cluster, cx);
    }

    fn install_structure_plot<P>(&mut self, plot: P, cx: &mut Context<Self>)
    where
        P: ruviz::core::TryIntoPlot3DSession + 'static,
    {
        match &self.structure.plot3d {
            Some(entity) => {
                let res = entity.update(cx, |p, cx| p.set_plot_keep_view(plot, cx));
                if let Err(e) = res {
                    self.structure.plot_error = Some(e.to_string());
                }
            }
            None => {
                let entity = plot3d_builder(plot).interactive().fill().build(cx);
                cx.subscribe(
                    &entity,
                    |this: &mut Self, _e, event: &Plot3DEvent, cx| match event {
                        Plot3DEvent::Pick(hit) => this.structure_pick(*hit, cx),
                        Plot3DEvent::Error(err) => {
                            this.structure.plot_error = Some(err.to_string());
                            cx.notify();
                        }
                        Plot3DEvent::CameraChanged(_) => {}
                    },
                )
                .detach();
                self.structure.plot3d = Some(entity);
            }
        }
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

    fn structure_pick(&mut self, hit: PickHit3D, cx: &mut Context<Self>) {
        if hit.primitive != PickPrimitive3D::Point {
            return;
        }
        let Some(SeriesTarget::Atoms(ids)) =
            self.structure.series_map.get(hit.series_index as usize)
        else {
            return;
        };
        let Some(&atom) = ids.get(hit.primitive_index as usize) else {
            return;
        };
        self.structure.pick = Some(AtomPick { atom });
        cx.notify();
    }

    pub(crate) fn set_structure_camera(&mut self, preset: CameraPreset, cx: &mut Context<Self>) {
        if let Some(p) = &self.structure.plot3d {
            let _ = p.update(cx, |p, cx| {
                p.set_camera(
                    Camera3D::default()
                        .camera_view(preset.view())
                        .look_at(Point3D {
                            x: 0.0,
                            y: 0.0,
                            z: 0.0,
                        }),
                    cx,
                )
            });
        }
    }

    // ---- structure panel actions -------------------------------------------

    pub(crate) fn structure_search(&mut self, cx: &mut Context<Self>) {
        let query = self.structure.search.read(cx).text().to_string();
        match provider_for(self.structure.source) {
            Ok(p) => match p.search(&query) {
                Ok(hits) => {
                    self.structure.hits = hits;
                    self.structure.search_error = None;
                }
                Err(e) => {
                    self.structure.hits.clear();
                    self.structure.search_error = Some(e);
                }
            },
            Err(e) => {
                self.structure.hits.clear();
                self.structure.search_error = Some(e);
            }
        }
        cx.notify();
    }

    pub(crate) fn structure_choose(&mut self, i: usize, cx: &mut Context<Self>) {
        let Some(hit) = self.structure.hits.get(i).cloned() else {
            return;
        };
        match provider_for(hit.source).and_then(|p| p.fetch(&hit)) {
            Ok(summary) => {
                let el = summary.elements();
                if !self
                    .structure
                    .absorber
                    .as_ref()
                    .is_some_and(|a| el.iter().any(|(s, _)| s == a))
                {
                    self.structure.absorber = el.first().map(|(s, _)| s.clone());
                }
                self.structure.summary = Some(summary);
                self.structure.search_error = None;
            }
            Err(e) => self.structure.search_error = Some(e),
        }
        cx.notify();
    }

    /// Build the feff.inp for the chosen structure and run FEFF.
    pub(crate) fn structure_generate_paths(&mut self, cx: &mut Context<Self>) {
        let Some(summary) = self.structure.summary.clone() else {
            self.status = "choose a structure first".into();
            cx.notify();
            return;
        };
        let Some(mut spec) = summary.builtin.clone() else {
            self.status =
                "this structure needs the core CIF → cluster generator (xafs::structure)".into();
            cx.notify();
            return;
        };
        if let Some(abs) = &self.structure.absorber
            && abs != &spec.element
        {
            // Swap roles so the absorber is the first element.
            let other = spec.element.clone();
            spec.element = abs.clone();
            spec.element2 = Some(other);
        }
        spec.edge = self.structure.edge.clone();
        spec.rmax = self
            .structure
            .radius
            .read(cx)
            .text()
            .trim()
            .parse()
            .unwrap_or(6.0);
        match crate::feffgen::new_workspace_from_spec(&spec) {
            Ok(dir) => {
                self.feff_workspace = Some(dir);
                self.fit_paths.clear();
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

    // ---- center: 3D view + docked table -------------------------------------

    pub(crate) fn structure_center(&mut self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        if self.structure.plot3d.is_none() && self.structure.cluster.is_some() {
            self.rebuild_structure_plot(cx);
        }
        let cluster = self.structure.cluster.clone();
        let toolbar = self.structure_toolbar(cx);
        let mut left = div()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .gap_2()
            .child(toolbar);
        match (&cluster, &self.structure.plot3d) {
            (Some(cluster), Some(plot)) => {
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
                        .child(div().size_full().child(plot.clone()))
                        .child(self.structure_legend(cluster))
                        .children(self.structure_pick_card(cluster)),
                );
                if let Some(hist) = &self.structure.hist {
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
                            None => "No cluster yet — choose a structure and Generate paths, or Run FEFF on a feff.inp".to_string(),
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
            .child(
                div()
                    .w(px(360.))
                    .flex_none()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .rounded_lg()
                    .border_1()
                    .border_color(t.border)
                    .bg(t.surface)
                    .overflow_hidden()
                    .child(self.structure_paths_table(true, cx)),
            )
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
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap_2()
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(t.text_muted)
                    .child("view"),
            )
            .child(presets)
            .child(
                button(&t, "cam-reset", "reset", false).on_click(cx.listener(
                    |this, _: &ClickEvent, _w, cx| {
                        if let Some(p) = &this.structure.plot3d {
                            let _ = p.update(cx, |p, cx| p.reset_view(cx));
                        }
                    },
                )),
            )
            .child(div().w(px(1.)).h(px(18.)).bg(t.border))
            .child(
                chip(
                    &t,
                    "st-shells",
                    "colour by shell",
                    self.structure.color_by_shell,
                )
                .on_click(cx.listener(|this, _: &ClickEvent, _w, cx| {
                    this.structure.color_by_shell = !this.structure.color_by_shell;
                    this.rebuild_structure_plot(cx);
                    cx.notify();
                })),
            )
            .child(
                chip(&t, "st-bonds", "bonds", self.structure.show_bonds).on_click(cx.listener(
                    |this, _: &ClickEvent, _w, cx| {
                        this.structure.show_bonds = !this.structure.show_bonds;
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
                    .child("drag to orbit · wheel to zoom · click an atom"),
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
        for (symbol, z, n) in cluster.element_counts() {
            let c = cpk_color(z);
            legend = legend.child(
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
        legend.child(div().mt_1().child(SharedString::from(format!(
            "{} atoms · {} shells · rmax {:.1} Å",
            cluster.atoms.len(),
            cluster.shells.len().saturating_sub(1),
            cluster.atoms.iter().map(|a| a.dist).fold(0.0_f64, f64::max)
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
    pub(crate) fn structure_paths_table(
        &self,
        docked: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let t = self.theme;
        let f = &self.structure.filters;
        let filtered = self.structure_filtered_paths();
        let n_multi = self.structure.multi.len();
        let mut filters = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_1()
            .px_2()
            .py_1()
            .child(
                chip(&t, "pf-ss", "single scattering", f.single_scattering).on_click(cx.listener(
                    |this, _: &ClickEvent, _w, cx| {
                        this.structure.filters.single_scattering =
                            !this.structure.filters.single_scattering;
                        cx.notify();
                    },
                )),
            )
            .child(
                chip(&t, "pf-3legs", "≤ 3 legs", f.max_legs == Some(3)).on_click(cx.listener(
                    |this, _: &ClickEvent, _w, cx| {
                        let f = &mut this.structure.filters;
                        f.max_legs = if f.max_legs == Some(3) { None } else { Some(3) };
                        cx.notify();
                    },
                )),
            )
            .child(
                chip(&t, "pf-amp", "amp ≥ 5 %", f.min_importance > 0.0).on_click(cx.listener(
                    |this, _: &ClickEvent, _w, cx| {
                        let f = &mut this.structure.filters;
                        f.min_importance = if f.min_importance > 0.0 { 0.0 } else { 0.05 };
                        cx.notify();
                    },
                )),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(t.text_muted)
                            .child("R ≤"),
                    )
                    .child(
                        div()
                            .w(px(56.))
                            .child(self.structure.max_reff_input.clone()),
                    ),
            );
        if n_multi > 0 {
            let sel: Vec<usize> = self.structure.multi.iter().copied().collect();
            let sel2 = sel.clone();
            filters = filters
                .child(
                    button(&t, "pf-enable", format!("enable {n_multi}"), true).on_click(
                        cx.listener(move |this, _: &ClickEvent, _w, cx| {
                            this.set_paths_enabled(&sel, true, cx);
                        }),
                    ),
                )
                .child(
                    button(&t, "pf-disable", "disable", false).on_click(cx.listener(
                        move |this, _: &ClickEvent, _w, cx| {
                            this.set_paths_enabled(&sel2, false, cx);
                        },
                    )),
                );
        }
        let header = div()
            .px_2()
            .py_0p5()
            .flex()
            .items_center()
            .gap_1()
            .text_size(px(10.5))
            .text_color(t.text_muted)
            .child(div().w(px(16.)))
            .child(div().flex_1().child("path"))
            .child(div().w(px(52.)).child("Reff"))
            .child(div().w(px(28.)).child("N"))
            .child(div().w(px(30.)).child("legs"))
            .child(div().w(px(56.)).child("amp"));
        let mut list = div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .id(if docked {
                "st-paths-docked"
            } else {
                "st-paths-insp"
            })
            .overflow_y_scroll();
        if self.fit_paths.is_empty() {
            list = list.child(
                div()
                    .p_2()
                    .text_size(px(11.))
                    .text_color(t.text_muted)
                    .child("no paths yet"),
            );
        }
        for i in filtered {
            let row = &self.fit_paths[i];
            let geom = self.structure.paths.get(i).and_then(|p| p.as_ref());
            let enabled = row.spec.enabled;
            let selected = self.structure.multi.contains(&i);
            let focused = self.structure.selected == Some(i);
            let label: SharedString = match geom {
                Some(g) => format!("{} {}", i + 1, g.label()).into(),
                None => format!("{} {}", i + 1, row.spec.label).into(),
            };
            let (reff, degen, nleg, amp) = match (geom, &row.meta) {
                (Some(g), _) => (g.reff, g.degen, g.nleg, g.importance),
                (None, Some(m)) => (m.reff, m.degen, m.nleg, 0.0),
                _ => (0.0, 0.0, 0, 0.0),
            };
            let bar_w = (amp * 40.0).round().max(1.0) as f32;
            list = list.child(
                div()
                    .id(("st-path", i))
                    .h(px(24.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_1()
                    .text_size(px(11.5))
                    .cursor_pointer()
                    .when(selected, |d| d.bg(t.raised))
                    .when(focused, |d| d.border_l_2().border_color(t.accent))
                    .hover(|d| d.bg(t.raised))
                    .on_hover(cx.listener(move |this, hovered: &bool, _w, cx| {
                        this.hover_structure_path(if *hovered { Some(i) } else { None }, cx);
                    }))
                    .on_click(cx.listener(move |this, ev: &ClickEvent, _w, cx| {
                        this.select_structure_path(i, ev.modifiers(), cx);
                    }))
                    .child(
                        div()
                            .id(("st-path-en", i))
                            .flex_none()
                            .w(px(12.))
                            .h(px(12.))
                            .rounded_sm()
                            .border_1()
                            .when(enabled, |d| d.bg(t.accent).border_color(t.accent))
                            .when(!enabled, |d| d.bg(t.raised).border_color(t.border))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _w, cx| {
                                cx.stop_propagation();
                                let now = this
                                    .fit_paths
                                    .get(i)
                                    .map(|r| r.spec.enabled)
                                    .unwrap_or(false);
                                this.set_paths_enabled(&[i], !now, cx);
                            })),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .when(!enabled, |d| d.text_color(t.text_muted))
                            .child(label),
                    )
                    .child(mono(&t, format!("{reff:.3}"), 52.))
                    .child(mono(&t, format!("{degen:.0}"), 28.))
                    .child(mono(&t, format!("{nleg}"), 30.))
                    .child(
                        div()
                            .w(px(56.))
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(div().w(px(bar_w)).h(px(6.)).rounded_sm().bg(t.accent))
                            .child(
                                div()
                                    .text_size(px(10.))
                                    .text_color(t.text_muted)
                                    .child(SharedString::from(format!("{:.0}", amp * 100.0))),
                            ),
                    ),
            );
        }
        let mut col = div().flex().flex_col().min_h_0().min_w_0();
        if docked {
            col = col.child(
                div()
                    .px_2()
                    .pt_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(section_label(&t, "paths"))
                    .child(div().text_size(px(10.5)).text_color(t.text_muted).child(
                        SharedString::from(format!(
                            "{} imported · click to focus · ⌘/⇧ multi-select",
                            self.fit_paths.len()
                        )),
                    )),
            );
        }
        col.child(filters).child(header).child(list).flex_1()
    }

    // ---- inspector: structure panel ------------------------------------------

    pub(crate) fn structure_panel(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = self.theme;
        let st = &self.structure;
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
                    this.structure_search(cx);
                })),
            );
        }
        let mut panel = div()
            .flex()
            .flex_col()
            .gap_1p5()
            .px_3()
            .child(div().flex().flex_wrap().child(sources))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(div().flex_1().child(st.search.clone()))
                    .child(
                        button(&t, "st-search", "Search", false)
                            .on_click(cx.listener(|this, _: &ClickEvent, _w, cx| {
                                this.structure_search(cx)
                            })),
                    )
                    .child(
                        button(&t, "st-import", "Import CIF…", false).on_click(cx.listener(
                            |this, _: &ClickEvent, _w, cx| {
                                this.status = "CIF import arrives with the core structure module (xafs::structure)".into();
                                cx.notify();
                            },
                        )),
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
        if !st.hits.is_empty() {
            let mut list = div()
                .id("st-hits")
                .max_h(px(168.))
                .overflow_y_scroll()
                .rounded_md()
                .border_1()
                .border_color(t.border)
                .flex()
                .flex_col();
            for (i, hit) in st.hits.iter().enumerate() {
                let chosen = st
                    .summary
                    .as_ref()
                    .is_some_and(|s| s.hit.id == hit.id && s.hit.source == hit.source);
                list = list.child(
                    div()
                        .id(("st-hit", i))
                        .h(px(26.))
                        .px_2()
                        .flex()
                        .items_center()
                        .gap_2()
                        .cursor_pointer()
                        .when(chosen, |d| d.bg(t.raised))
                        .hover(|d| d.bg(t.raised))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _w, cx| {
                            this.structure_choose(i, cx)
                        }))
                        .child(
                            div()
                                .w(px(64.))
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
                                .child(SharedString::from(format!(
                                    "{} · {}",
                                    hit.name, hit.space_group
                                ))),
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
            panel = panel.child(list);
        }
        if let Some(s) = &st.summary {
            let l = s.lattice;
            let sites = s
                .sites
                .iter()
                .map(|(sym, _, m)| format!("{sym}×{m}"))
                .collect::<Vec<_>>()
                .join("  ");
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
                            .child(
                                div()
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .child(SharedString::from(s.hit.formula.clone())),
                            )
                            .child(
                                div()
                                    .text_color(t.text_muted)
                                    .child(SharedString::from(s.hit.name.clone())),
                            ),
                    )
                    .child(
                        div()
                            .text_color(t.text_muted)
                            .child(SharedString::from(s.hit.space_group.clone())),
                    )
                    .child(mono_line(
                        &t,
                        format!("a {:.3}  b {:.3}  c {:.3} Å", l[0], l[1], l[2]),
                    ))
                    .child(mono_line(
                        &t,
                        format!("α {:.1}°  β {:.1}°  γ {:.1}°", l[3], l[4], l[5]),
                    ))
                    .child(mono_line(&t, format!("sites  {sites}"))),
            );
            // Absorber picker
            let mut abs_row = div().flex().items_center().flex_wrap().gap_1().child(
                div()
                    .w(px(64.))
                    .text_size(px(12.))
                    .text_color(t.text_muted)
                    .child("absorber"),
            );
            for (sym, z) in s.elements() {
                let on = st.absorber.as_deref() == Some(sym.as_str());
                let mult = s
                    .sites
                    .iter()
                    .filter(|(e, _, _)| *e == sym)
                    .map(|(_, _, m)| *m)
                    .sum::<usize>();
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
                            cx.notify();
                        },
                    )),
                );
            }
            panel = panel.child(abs_row);
            let mut edge_row = div().flex().items_center().gap_1().child(
                div()
                    .w(px(64.))
                    .text_size(px(12.))
                    .text_color(t.text_muted)
                    .child("edge"),
            );
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
                    .child(
                        div()
                            .w(px(64.))
                            .text_size(px(12.))
                            .text_color(t.text_muted)
                            .child("cluster"),
                    )
                    .child(div().w(px(64.)).child(st.radius.clone()))
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(t.text_muted)
                            .child("Å radius / RMAX"),
                    )
                    .child(div().flex_1())
                    .child(
                        button(
                            &t,
                            "st-generate",
                            if self.feff_running {
                                "running FEFF…"
                            } else {
                                "Generate paths"
                            },
                            !self.feff_running,
                        )
                        .on_click(cx.listener(
                            |this, _: &ClickEvent, _w, cx| this.structure_generate_paths(cx),
                        )),
                    ),
            );
        }
        // Materials Project key (masked)
        let key = st.mp_key_text(cx);
        let masked: SharedString = if key.is_empty() {
            "not set".into()
        } else {
            format!(
                "{}••••••••{}",
                &key[..key.len().min(3)],
                &key[key.len().saturating_sub(2)..]
            )
            .into()
        };
        let mut key_row = div().flex().items_center().gap_1().child(
            div()
                .w(px(64.))
                .text_size(px(12.))
                .text_color(t.text_muted)
                .child("MP key"),
        );
        if st.mp_key_editing {
            key_row = key_row.child(div().flex_1().child(st.mp_key.clone()));
        } else {
            key_row = key_row
                .child(
                    div()
                        .flex_1()
                        .font_family(MONO)
                        .text_size(px(11.5))
                        .text_color(t.text_muted)
                        .child(masked),
                )
                .child(
                    button(&t, "mp-key-edit", "change", false).on_click(cx.listener(
                        |this, _: &ClickEvent, _w, cx| {
                            this.structure.mp_key_editing = true;
                            cx.notify();
                        },
                    )),
                );
        }
        panel = panel.child(key_row).child(
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

fn mono(t: &Theme, text: String, w: f32) -> gpui::Div {
    div()
        .w(px(w))
        .font_family(MONO)
        .text_size(px(11.))
        .text_color(t.text_muted)
        .child(SharedString::from(text))
}

fn mono_line(t: &Theme, text: String) -> gpui::Div {
    div()
        .font_family(MONO)
        .text_size(px(11.))
        .text_color(t.text)
        .child(SharedString::from(text))
}
