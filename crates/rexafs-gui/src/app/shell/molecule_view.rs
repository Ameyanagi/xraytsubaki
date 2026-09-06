//! Molecular rendering with a fixed orthographic scale. Rotation never fits
//! the projected bounding box; only the wheel and explicit reset change zoom.
use super::bond_geometry::{BondMode, contacts, nearest_bonds};
use super::molecular_geometry::{MolecularComponent, complete_molecule};
use super::structure_depth::{DepthFrame, FadeMode};
use super::structure_view::AtomPick;
use crate::{
    app::StudioApp,
    structure::{Cluster, PathGeometry, covalent_radius, cpk_color},
    theme::Theme,
};
use gpui::{
    Bounds, Context, IntoElement, MouseButton, Pixels, Point, Rgba, Styled, Window, canvas, div,
    point, prelude::*, px, size,
};
use rexafs::xafs::structure as core;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum AtomStyle {
    Balls,
    BallStick,
    Wireframe,
    Polyhedra,
}
impl AtomStyle {
    pub const ALL: [Self; 4] = [
        Self::Balls,
        Self::BallStick,
        Self::Wireframe,
        Self::Polyhedra,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Balls => "Balls",
            Self::BallStick => "Ball + stick",
            Self::Wireframe => "Wireframe",
            Self::Polyhedra => "Polyhedron",
        }
    }
}

#[derive(Clone)]
pub(crate) struct SceneAtom {
    pub pos: [f64; 3],
    pub z: u32,
    pub index: Option<usize>,
    pub shell: usize,
    pub absorber: bool,
    pub faded: bool,
    pub label: String,
}
#[derive(Clone, Copy)]
pub(crate) struct PolyhedronOptions {
    pub network: bool,
    pub ligand: Option<u32>,
    pub cutoff: Option<f64>,
    pub opacity: f32,
    pub color: Option<u32>,
    pub edges: bool,
    pub atoms: PolyAtoms,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum PolyAtoms {
    All,
    Centers,
    None,
}
impl Default for PolyhedronOptions {
    fn default() -> Self {
        Self {
            network: true,
            ligand: None,
            cutoff: None,
            opacity: 0.65,
            color: Some(0x70a9ee),
            edges: true,
            atoms: PolyAtoms::Centers,
        }
    }
}
#[derive(Clone)]
pub(crate) struct PolyFace {
    vertices: Vec<[f64; 3]>,
    normal: [f64; 3],
    z: u32,
}
#[derive(Clone, Default)]
pub(crate) struct CrystalContext {
    pub atoms: Vec<SceneAtom>,
    pub edges: Vec<[[f64; 3]; 2]>,
    pub cells: [usize; 3],
    pub truncated: bool,
    pub radius: f64,
    pub molecule: Option<Result<MolecularComponent, String>>,
}
#[derive(Clone, Default)]
pub(crate) struct MoleculeScene {
    pub atoms: Vec<SceneAtom>,
    pub bonds: Vec<[usize; 2]>,
    pub all_bonds: Vec<[usize; 2]>,
    pub edges: Vec<[[f64; 3]; 2]>,
    pub faces: Vec<PolyFace>,
    pub poly_atoms: Vec<usize>,
    pub poly_centers: Vec<usize>,
    pub poly_count: usize,
    pub poly_options: PolyhedronOptions,
    pub message: Option<String>,
    pub labels: bool,
    pub route: Vec<[f64; 3]>,
    pub radius: f64,
    pub extent: f64,
    pub center: [f64; 3],
}
#[derive(Clone, Copy)]
pub(crate) struct ViewCamera {
    pub az: f64,
    pub el: f64,
    pub zoom: f64,
}
impl Default for ViewCamera {
    fn default() -> Self {
        Self {
            az: -0.6,
            el: 0.45,
            zoom: 1.,
        }
    }
}
impl ViewCamera {
    pub(crate) fn zoom_by(&mut self, log_delta: f64) {
        if log_delta.is_finite() {
            self.zoom = (self.zoom * log_delta.exp()).clamp(0.25, 5.);
        }
    }

    fn scroll_zoom(&mut self, delta: gpui::ScrollDelta) {
        let log_delta = match delta {
            gpui::ScrollDelta::Pixels(p) => -f32::from(p.y) as f64 * 0.0015,
            gpui::ScrollDelta::Lines(p) => -p.y as f64 * 0.12,
        };
        // High-resolution wheels and synthetic scroll events can report large
        // deltas. One event must not jump from a fitted view to maximum zoom.
        self.zoom_by(log_delta.clamp(-0.15, 0.15));
    }

    pub(crate) fn rotate(self, p: [f64; 3]) -> [f64; 3] {
        let (sa, ca) = self.az.sin_cos();
        let (se, ce) = self.el.sin_cos();
        let x = ca * p[0] - sa * p[1];
        let y = sa * p[0] + ca * p[1];
        [x, ce * p[2] - se * y, se * p[2] + ce * y]
    }
    fn project(self, p: [f64; 3], bounds: Bounds<Pixels>, extent: f64) -> [f32; 3] {
        let q = self.rotate(p);
        let scale = self.scale(bounds, extent);
        [
            f32::from(bounds.center().x) + q[0] as f32 * scale,
            f32::from(bounds.center().y) - q[1] as f32 * scale,
            q[2] as f32,
        ]
    }
    fn scale(self, b: Bounds<Pixels>, extent: f64) -> f32 {
        f32::from(b.size.width.min(b.size.height)) * 0.42 * self.zoom as f32 / extent.max(1.) as f32
    }
}
fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| a[i] - b[i])
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| a[i] * b[i]).sum()
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn norm(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}

/// Whole periodic unit cells covering the calculated sphere, with at least three
/// cells along each axis for context. The FEFF cluster itself is never truncated.
pub(crate) fn crystal_context(s: &core::Structure, c: &core::Cluster) -> CrystalContext {
    let origin = s.lattice.to_cart(s.sites[c.absorber_site].frac);
    let mut lo = [0; 3];
    let mut hi = [0; 3];
    for atom in &c.atoms {
        for k in 0..3 {
            lo[k] = lo[k].min(atom.image[k]);
            hi[k] = hi[k].max(atom.image[k]);
        }
    }
    for k in 0..3 {
        lo[k] = lo[k].min(-1);
        hi[k] = hi[k].max(1);
    }
    let mut out = CrystalContext {
        radius: c.radius,
        molecule: Some(complete_molecule(s, c.absorber_site)),
        cells: std::array::from_fn(|i| (hi[i] - lo[i] + 1) as usize),
        ..Default::default()
    };
    let cart = |f| sub(s.lattice.to_cart(f), origin);
    'cells: for a in lo[0]..=hi[0] {
        for b in lo[1]..=hi[1] {
            for z in lo[2]..=hi[2] {
                for site in &s.sites {
                    let Some(el) = site.element() else {
                        continue;
                    };
                    if el.z == 1 {
                        continue;
                    }
                    let p = cart([
                        site.frac[0] + a as f64,
                        site.frac[1] + b as f64,
                        site.frac[2] + z as f64,
                    ]);
                    if norm(p) <= c.radius + 1e-6 {
                        continue;
                    }
                    if out.atoms.len() >= 12000 {
                        out.truncated = true;
                        break 'cells;
                    }
                    out.atoms.push(SceneAtom {
                        pos: p,
                        z: el.z as u32,
                        index: None,
                        shell: 0,
                        absorber: false,
                        faded: true,
                        label: site.label.clone(),
                    });
                }
            }
        }
    }
    // A lattice grid along each axis, drawn once per edge.
    for axis in 0..3 {
        let j = (axis + 1) % 3;
        let k = (axis + 2) % 3;
        for u in lo[j]..=hi[j] + 1 {
            for v in lo[k]..=hi[k] + 1 {
                let mut p = [0.; 3];
                p[j] = u as f64;
                p[k] = v as f64;
                p[axis] = lo[axis] as f64;
                let mut q = p;
                q[axis] = (hi[axis] + 1) as f64;
                out.edges.push([cart(p), cart(q)]);
            }
        }
    }
    out
}

/// Convex coordination faces, including coplanar polygons as one face.
fn hull_faces(points: &[[f64; 3]]) -> Vec<Vec<[f64; 3]>> {
    let mut faces: Vec<Vec<usize>> = Vec::new();
    for i in 0..points.len() {
        for j in i + 1..points.len() {
            for k in j + 1..points.len() {
                let normal = cross(sub(points[j], points[i]), sub(points[k], points[i]));
                if norm(normal) < 1e-8 {
                    continue;
                }
                let ds: Vec<_> = points
                    .iter()
                    .map(|p| dot(sub(*p, points[i]), normal) / norm(normal))
                    .collect();
                if ds.iter().any(|d| *d > 1e-5) && ds.iter().any(|d| *d < -1e-5) {
                    continue;
                }
                let ids: Vec<_> = ds
                    .iter()
                    .enumerate()
                    .filter(|(_, d)| d.abs() < 1e-5)
                    .map(|(n, _)| n)
                    .collect();
                if !faces.contains(&ids) {
                    faces.push(ids);
                }
            }
        }
    }
    faces
        .into_iter()
        .map(|mut ids| {
            let center = std::array::from_fn(|i| {
                ids.iter().map(|&n| points[n][i]).sum::<f64>() / ids.len() as f64
            });
            let u = sub(points[ids[0]], center);
            let normal = cross(
                sub(points[ids[1]], points[ids[0]]),
                sub(points[ids[2]], points[ids[0]]),
            );
            let v = cross(normal, u);
            ids.sort_by(|&a, &b| {
                let pa = sub(points[a], center);
                let pb = sub(points[b], center);
                (dot(pa, v) / norm(v))
                    .atan2(dot(pa, u) / norm(u))
                    .total_cmp(&(dot(pb, v) / norm(v)).atan2(dot(pb, u) / norm(u)))
            });
            ids.into_iter().map(|i| points[i]).collect()
        })
        .collect()
}

impl MoleculeScene {
    pub fn new(
        cluster: &Cluster,
        context: Option<&CrystalContext>,
        radius: f64,
        style: AtomStyle,
        path: Option<&PathGeometry>,
        picked: Option<usize>,
        poly: PolyhedronOptions,
    ) -> Self {
        let mut scene = Self {
            radius,
            extent: radius.max(1.),
            ..Default::default()
        };
        scene.atoms = cluster
            .atoms
            .iter()
            .enumerate()
            .map(|(i, a)| SceneAtom {
                pos: a.pos,
                z: a.z,
                index: Some(i),
                shell: a.shell,
                absorber: a.ipot == 0,
                faded: false,
                label: a.tag.clone(),
            })
            .collect();
        if style != AtomStyle::Balls {
            scene.all_bonds = contacts(&scene.atoms);
            scene.bonds = nearest_bonds(&scene.atoms, &scene.all_bonds);
        }
        if let Some(context) = context {
            scene.atoms.extend(context.atoms.clone());
            scene.edges = context.edges.clone();
        }
        if style == AtomStyle::Polyhedra {
            scene.build_polyhedra(picked.unwrap_or(0), poly);
        }
        for a in &scene.atoms {
            scene.extent = scene.extent.max(norm(a.pos));
        }
        if let Some(p) = path {
            scene.route = p.polyline();
        }
        scene
    }
    pub fn apply_bond_mode(&mut self, mode: BondMode) {
        match mode {
            BondMode::Auto => (),
            BondMode::Absorber => self
                .bonds
                .retain(|&[a, b]| self.atoms[a].absorber || self.atoms[b].absorber),
            BondMode::AllContacts => self.bonds = self.all_bonds.clone(),
            BondMode::None => self.bonds.clear(),
        }
    }
    fn build_polyhedra(&mut self, center: usize, options: PolyhedronOptions) {
        self.poly_options = options;
        let Some(seed) = self.atoms.get(center) else {
            return;
        };
        let z = seed.z;
        let centers = self
            .atoms
            .iter()
            .enumerate()
            .filter(|(i, a)| !a.faded && a.z == z && (options.network || *i == center))
            .map(|(i, _)| i)
            .collect::<Vec<_>>();
        let mut selected = std::collections::BTreeSet::new();
        let mut bonds = std::collections::BTreeSet::new();
        let mut oversized = 0;
        for center in centers {
            let origin = self.atoms[center].pos;
            // Auto follows the nearest unlike-element coordination shell when
            // one lies within a plausible bond range, otherwise the metal shell.
            let unlike = self.atoms.iter().any(|a| {
                a.z != z && a.z != 1 && {
                    let d = norm(sub(a.pos, origin));
                    d > 0.4 && d <= 1.4 * (covalent_radius(a.z) + covalent_radius(z)) as f64
                }
            });
            let candidates = self
                .atoms
                .iter()
                .enumerate()
                .filter(|(_, a)| {
                    options
                        .ligand
                        .map(|el| a.z == el)
                        .unwrap_or(a.z != 1 && (!unlike || a.z != z))
                })
                .map(|(i, a)| (i, norm(sub(a.pos, origin))))
                .filter(|(_, d)| *d > 0.4)
                .collect::<Vec<_>>();
            let nearest = candidates
                .iter()
                .map(|(_, d)| *d)
                .fold(f64::INFINITY, f64::min);
            let cutoff = options.cutoff.unwrap_or(nearest * 1.25);
            let neighbors = candidates
                .into_iter()
                .filter(|(_, d)| *d <= cutoff + 1e-6)
                .map(|(i, _)| i)
                .collect::<Vec<_>>();
            if neighbors.len() > 24 {
                oversized += 1;
                continue;
            }
            if neighbors.len() < 3 {
                continue;
            }
            let points = neighbors
                .iter()
                .map(|&i| self.atoms[i].pos)
                .collect::<Vec<_>>();
            let faces = hull_faces(&points);
            if faces.is_empty() {
                continue;
            }
            self.poly_count += 1;
            self.poly_centers.push(center);
            selected.insert(center);
            for &n in &neighbors {
                selected.insert(n);
                bonds.insert([center.min(n), center.max(n)]);
            }
            for vertices in faces {
                let mut normal =
                    cross(sub(vertices[1], vertices[0]), sub(vertices[2], vertices[0]));
                let midpoint = std::array::from_fn(|i| {
                    vertices.iter().map(|p| p[i]).sum::<f64>() / vertices.len() as f64
                });
                if dot(normal, sub(midpoint, origin)) < 0. {
                    normal = normal.map(|v| -v);
                }
                let length = norm(normal);
                if length <= 1e-8 {
                    continue;
                }
                self.faces.push(PolyFace {
                    vertices,
                    normal: normal.map(|v| v / length),
                    z,
                });
            }
        }
        self.poly_atoms = selected.into_iter().collect();
        self.bonds = if options.atoms == PolyAtoms::All {
            bonds.into_iter().collect()
        } else {
            Vec::new()
        };
        if self.poly_count == 0 {
            self.message = Some(
                "No coordination faces. Choose another center, neighbour element or bond limit."
                    .into(),
            );
        } else if oversized > 0 {
            self.message = Some(format!(
                "{oversized} centers exceed 24 neighbours; reduce the bond limit to display them."
            ));
        }
    }

    pub fn molecule(
        component: &MolecularComponent,
        cluster: &Cluster,
        radius: f64,
        hydrogens: bool,
        style: AtomStyle,
    ) -> Self {
        let mut scene = Self {
            radius,
            ..Default::default()
        };
        for axis in 0..3 {
            let lo = component
                .atoms
                .iter()
                .map(|a| a.pos[axis])
                .fold(f64::INFINITY, f64::min);
            let hi = component
                .atoms
                .iter()
                .map(|a| a.pos[axis])
                .fold(f64::NEG_INFINITY, f64::max);
            scene.center[axis] = (lo + hi) * 0.5;
        }
        scene.extent = component
            .atoms
            .iter()
            .map(|a| norm(sub(a.pos, scene.center)))
            .fold(1.5, f64::max);
        let mut indices = std::collections::BTreeMap::new();
        for (i, a) in component.atoms.iter().enumerate() {
            if a.z == 1 && !hydrogens {
                continue;
            }
            let index = cluster
                .atoms
                .iter()
                .position(|b| b.z == a.z && norm(sub(a.pos, b.pos)) < 1e-4);
            indices.insert(i, scene.atoms.len());
            scene.atoms.push(SceneAtom {
                pos: a.pos,
                z: a.z,
                index,
                shell: index.map(|i| cluster.atoms[i].shell).unwrap_or(0),
                absorber: norm(a.pos) < 1e-6,
                faded: false,
                label: a.label.clone(),
            });
        }
        if style != AtomStyle::Balls {
            for [a, b] in &component.bonds {
                if let (Some(a), Some(b)) = (indices.get(a), indices.get(b)) {
                    scene.bonds.push([*a, *b]);
                }
            }
        }
        scene.extent = scene.extent.max(1.5);
        // Complete-molecule connectivity is already reconstructed across CIF
        // boundaries. Preserve its C–C, C–H etc. bonds in Auto mode.
        scene.all_bonds = scene.bonds.clone();
        scene
    }
}

fn alpha(mut color: Rgba, a: f32) -> Rgba {
    color.a = a;
    color
}
fn tint(c: Rgba, light: f32) -> Rgba {
    Rgba {
        r: (c.r * light).min(1.),
        g: (c.g * light).min(1.),
        b: (c.b * light).min(1.),
        ..c
    }
}
fn line(window: &mut Window, pts: &[[f32; 3]], color: Rgba, width: f32, closed: bool) {
    if pts.is_empty() {
        return;
    }
    let mut b = gpui::PathBuilder::stroke(px(width));
    b.move_to(point(px(pts[0][0]), px(pts[0][1])));
    for p in &pts[1..] {
        b.line_to(point(px(p[0]), px(p[1])));
    }
    if closed {
        b.close();
    }
    if let Ok(p) = b.build() {
        window.paint_path(p, color);
    }
}
fn disk(window: &mut Window, p: [f32; 3], r: f32, c: Rgba) {
    window.paint_quad(gpui::quad(
        Bounds::new(
            point(px(p[0] - r), px(p[1] - r)),
            size(px(2. * r), px(2. * r)),
        ),
        gpui::Corners::all(px(r)),
        c,
        gpui::Edges::all(px(0.)),
        c,
        gpui::BorderStyle::Solid,
    ));
}

fn atom_in_style(scene: &MoleculeScene, style: AtomStyle, index: usize) -> bool {
    style != AtomStyle::Polyhedra
        || match scene.poly_options.atoms {
            PolyAtoms::None => false,
            PolyAtoms::Centers => scene.poly_centers.contains(&index),
            PolyAtoms::All => true,
        }
}

fn atom_radius(z: u32, style: AtomStyle, scale: f32) -> f32 {
    if style == AtomStyle::Wireframe {
        2.
    } else {
        (covalent_radius(z)
            * scale
            * if style == AtomStyle::Balls {
                0.62
            } else {
                0.31
            })
        .clamp(3., 24.)
    }
}

// A single gradient quad keeps translucent spheres at their requested alpha.
// Layering twelve translucent highlight disks would incorrectly make them opaque.
fn translucent_ball(w: &mut Window, p: [f32; 3], r: f32, color: Rgba, shading: bool) {
    let background = if shading {
        gpui::linear_gradient(
            135.,
            gpui::linear_color_stop(tint(color, 1.25), 0.),
            gpui::linear_color_stop(tint(color, 0.4), 1.),
        )
    } else {
        color.into()
    };
    w.paint_quad(gpui::quad(
        Bounds::new(
            point(px(p[0] - r), px(p[1] - r)),
            size(px(2. * r), px(2. * r)),
        ),
        gpui::Corners::all(px(r)),
        background,
        gpui::Edges::all(px(0.)),
        color,
        gpui::BorderStyle::Solid,
    ));
}

fn depth_line(
    w: &mut Window,
    edge: [[f64; 3]; 2],
    depth: DepthFrame,
    project: &impl Fn([f64; 3]) -> [f32; 3],
    color: Rgba,
    width: f32,
) {
    for (part, inside) in depth.segments(edge) {
        let steps = if depth.options.fade == FadeMode::Off {
            1
        } else {
            8
        };
        for n in 0..steps {
            let at = |t: f64| std::array::from_fn(|a| part[0][a] + (part[1][a] - part[0][a]) * t);
            let opacity = depth.alpha(at((n as f64 + 0.5) / steps as f64), inside) * color.a;
            if opacity > 0.002 {
                line(
                    w,
                    &[
                        project(at(n as f64 / steps as f64)),
                        project(at((n + 1) as f64 / steps as f64)),
                    ],
                    alpha(color, opacity),
                    width,
                    false,
                );
            }
        }
    }
}

impl StudioApp {
    pub(crate) fn find_structure_absorber(&mut self, cx: &mut Context<Self>) {
        let index = self
            .structure
            .scene
            .as_ref()
            .and_then(|s| s.atoms.iter().find(|a| a.absorber))
            .and_then(|a| a.index);
        if let Some(atom) = index {
            self.structure.pick = Some(AtomPick { atom });
            self.structure.highlight_absorber = true;
            self.structure.absorber_label = true;
            self.structure.depth.options.offset = 0.;
            self.structure.camera.zoom = 1.;
            self.rebuild_structure_plot(cx);
            cx.notify();
        }
    }
    pub(crate) fn molecule_canvas(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let scene = self.structure.scene.clone().unwrap_or_default();
        let camera = self.structure.camera;
        let style = self.structure.atom_style;
        let shading = self.structure.shading;
        let shells = self.structure.color_by_shell;
        let step = self.structure.path_leg;
        let highlight_absorber = self.structure.highlight_absorber;
        let absorber_label = self.structure.absorber_label;
        let depth = DepthFrame::new(
            self.structure.depth.options,
            &scene,
            camera,
            self.structure.pick.as_ref().map(|p| p.atom),
        );
        let theme = self.theme;
        let weak = cx.entity().downgrade();
        let view = canvas(
            move |bounds, _, cx| {
                weak.update(cx, |this, _| this.structure.view_bounds = Some(bounds))
                    .ok();
            },
            move |bounds, _, window, cx| {
                paint_scene(
                    &scene,
                    camera,
                    style,
                    shading,
                    shells,
                    step,
                    depth,
                    highlight_absorber,
                    absorber_label,
                    theme,
                    bounds,
                    window,
                    cx,
                )
            },
        )
        .size_full();
        div()
            .id("molecular-canvas")
            .size_full()
            .cursor_grab()
            .child(view)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, ev: &gpui::MouseDownEvent, _, cx| {
                    this.structure.drag = Some((ev.position, ev.position, false));
                    cx.stop_propagation();
                }),
            )
            .on_mouse_move(cx.listener(|this, ev: &gpui::MouseMoveEvent, _, cx| {
                if ev.pressed_button != Some(MouseButton::Left) {
                    return;
                }
                if let Some((start, last, moved)) = this.structure.drag {
                    let dx = f32::from(ev.position.x - last.x);
                    let dy = f32::from(ev.position.y - last.y);
                    let distance = f32::from(ev.position.x - start.x)
                        .hypot(f32::from(ev.position.y - start.y));
                    if moved || distance > 4. {
                        this.structure.camera.az += dx as f64 * 0.008;
                        this.structure.camera.el =
                            (this.structure.camera.el + dy as f64 * 0.008).clamp(-1.55, 1.55);
                        this.structure.drag = Some((start, ev.position, true));
                        cx.notify();
                    }
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, ev: &gpui::MouseUpEvent, _, cx| {
                    if let Some((_, _, false)) = this.structure.drag.take() {
                        this.pick_molecule_atom(ev.position, cx);
                    }
                    cx.stop_propagation();
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, _| {
                    this.structure.drag = None;
                }),
            )
            .on_scroll_wheel(cx.listener(|this, ev: &gpui::ScrollWheelEvent, _, cx| {
                if this.structure.drag.is_some() {
                    cx.stop_propagation();
                    return;
                }
                this.structure.camera.scroll_zoom(ev.delta);
                cx.stop_propagation();
                cx.notify();
            }))
    }
    fn pick_molecule_atom(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let (Some(scene), Some(bounds)) = (&self.structure.scene, self.structure.view_bounds)
        else {
            return;
        };
        let depth = DepthFrame::new(
            self.structure.depth.options,
            scene,
            self.structure.camera,
            self.structure.pick.as_ref().map(|p| p.atom),
        );
        let mut hits: Vec<_> = scene
            .atoms
            .iter()
            .enumerate()
            .filter_map(|(index, a)| {
                if !depth.pickable(a.pos) || !atom_in_style(scene, self.structure.atom_style, index)
                {
                    return None;
                }
                let i = a.index?;
                let p =
                    self.structure
                        .camera
                        .project(sub(a.pos, scene.center), bounds, scene.extent);
                let d = (p[0] - f32::from(position.x)).hypot(p[1] - f32::from(position.y));
                let radius = atom_radius(
                    a.z,
                    self.structure.atom_style,
                    self.structure.camera.scale(bounds, scene.extent),
                );
                (d <= radius.max(6.)).then_some((i, p[2]))
            })
            .collect();
        hits.sort_by(|a, b| b.1.total_cmp(&a.1));
        // Empty space must not silently move an active slice back to the absorber.
        if hits.is_empty() && depth.options.active() {
            return;
        }
        self.structure.pick = hits.first().map(|(atom, _)| AtomPick { atom: *atom });
        self.structure.absorber_label = false;
        if self.structure.atom_style == AtomStyle::Polyhedra {
            self.rebuild_structure_plot(cx);
        }
        cx.notify();
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_scene(
    scene: &MoleculeScene,
    camera: ViewCamera,
    style: AtomStyle,
    shading: bool,
    shells: bool,
    leg: Option<usize>,
    depth: DepthFrame,
    highlight_absorber: bool,
    absorber_label: bool,
    t: Theme,
    b: Bounds<Pixels>,
    w: &mut Window,
    cx: &mut gpui::App,
) {
    let project = |p| camera.project(sub(p, scene.center), b, scene.extent);
    let scale = camera.scale(b, scene.extent);
    for edge in &scene.edges {
        depth_line(w, *edge, depth, &project, alpha(t.text_muted, 0.12), 0.65);
    }
    // Radius guides are a true sphere cut through the absorber, not a fitted box.
    if !scene.edges.is_empty() {
        for axis in 0..3 {
            let ring: Vec<_> = (0..=96)
                .map(|n| {
                    let ang = n as f64 * std::f64::consts::TAU / 96.;
                    let mut p = [0.; 3];
                    p[(axis + 1) % 3] = scene.radius * ang.cos();
                    p[(axis + 2) % 3] = scene.radius * ang.sin();
                    p
                })
                .collect();
            for points in ring.windows(2) {
                depth_line(
                    w,
                    [points[0], points[1]],
                    depth,
                    &project,
                    alpha(t.accent, 0.28),
                    1.,
                );
            }
        }
    }
    enum Primitive {
        Atom(usize),
        Bond(usize),
        Face(usize, Vec<[f64; 3]>, bool),
    }
    let mut draw = Vec::new();
    for (i, a) in scene.atoms.iter().enumerate() {
        if atom_in_style(scene, style, i) && depth.atom_alpha(a.pos) > 0.002 {
            // Highlight changes the material color, never the geometric depth.
            draw.push((project(a.pos)[2], Primitive::Atom(i)));
        }
    }
    for (i, ids) in scene.bonds.iter().enumerate() {
        draw.push((
            (project(scene.atoms[ids[0]].pos)[2] + project(scene.atoms[ids[1]].pos)[2]) * 0.5,
            Primitive::Bond(i),
        ));
    }
    for (i, f) in scene.faces.iter().enumerate() {
        for (vertices, inside) in depth.polygons(&f.vertices) {
            draw.push((
                vertices.iter().map(|p| project(*p)[2]).sum::<f32>() / vertices.len() as f32,
                Primitive::Face(i, vertices, inside),
            ));
        }
    }
    draw.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (_, p) in draw {
        match p {
            Primitive::Bond(i) => {
                let ids = scene.bonds[i];
                let centers = ids.map(|i| scene.atoms[i].pos);
                let projected = centers.map(project);
                let length =
                    (projected[1][0] - projected[0][0]).hypot(projected[1][1] - projected[0][1]);
                let radii = ids.map(|i| {
                    if depth.contains(scene.atoms[i].pos) && atom_in_style(scene, style, i) {
                        atom_radius(scene.atoms[i].z, style, scale)
                    } else {
                        0.
                    }
                });
                if length <= radii[0] + radii[1] {
                    continue;
                }
                // Trim to the projected sphere boundaries, so sticks cannot
                // stripe their own endpoint balls when depth sorting interleaves them.
                let at = |t: f64| {
                    std::array::from_fn(|a| centers[0][a] + (centers[1][a] - centers[0][a]) * t)
                };
                let pts = [
                    at((radii[0] / length) as f64),
                    at(1. - (radii[1] / length) as f64),
                ];
                let midpoint =
                    at(0.5_f64.clamp((radii[0] / length) as f64, 1. - (radii[1] / length) as f64));
                let width = if style == AtomStyle::Wireframe {
                    1.0
                } else {
                    (scale * 0.11).clamp(2.5, 7.)
                };
                for n in 0..2 {
                    let atom = &scene.atoms[ids[n]];
                    let color = alpha(
                        gpui::rgb(cpk_color(atom.z)),
                        if atom.faded { 0.14 } else { 1. },
                    );
                    let edge = [pts[n], midpoint];
                    if depth.options.active() || color.a < 0.99 {
                        depth_line(w, edge, depth, &project, color, width * 0.8);
                    } else {
                        depth_line(w, edge, depth, &project, tint(color, 0.5), width);
                        depth_line(w, edge, depth, &project, color, width * 0.67);
                        if shading && style != AtomStyle::Wireframe {
                            depth_line(w, edge, depth, &project, tint(color, 1.35), width * 0.22);
                        }
                    }
                }
            }
            Primitive::Face(i, vertices, inside) => {
                let face = &scene.faces[i];
                let center: [f64; 3] = std::array::from_fn(|axis| {
                    vertices.iter().map(|p| p[axis]).sum::<f64>() / vertices.len() as f64
                });
                let opacity = depth.alpha(center, inside);
                if opacity < 0.002 {
                    continue;
                }
                let pts: Vec<_> = vertices.iter().copied().map(project).collect();
                let mut path = gpui::PathBuilder::fill();
                path.move_to(point(px(pts[0][0]), px(pts[0][1])));
                for p in &pts[1..] {
                    path.line_to(point(px(p[0]), px(p[1])));
                }
                path.close();
                let normal = camera.rotate(face.normal);
                let light = if shading {
                    0.48 + 0.52 * dot(normal, [-0.35, 0.45, 0.822]).abs() as f32
                } else {
                    0.85
                };
                let base = gpui::rgb(
                    scene
                        .poly_options
                        .color
                        .unwrap_or_else(|| cpk_color(face.z)),
                );
                let color = tint(base, light);
                if let Ok(path) = path.build() {
                    w.paint_path(path, alpha(color, scene.poly_options.opacity * opacity));
                }
                if scene.poly_options.edges {
                    line(w, &pts, alpha(tint(base, 0.3), 0.85 * opacity), 1.25, true);
                }
            }
            Primitive::Atom(i) => {
                if style == AtomStyle::Polyhedra {
                    match scene.poly_options.atoms {
                        PolyAtoms::None => continue,
                        PolyAtoms::Centers if !scene.poly_centers.contains(&i) => continue,
                        _ => (),
                    }
                }
                let a = &scene.atoms[i];
                let p = project(a.pos);
                let outside = a.faded
                    || (style == AtomStyle::Polyhedra
                        && scene.poly_count > 0
                        && !scene.poly_atoms.contains(&i));
                let mut color = if a.absorber && highlight_absorber {
                    gpui::rgb(0x67e8f9)
                } else if shells && !outside {
                    crate::plotting::trace_rgba(&t, a.shell % 8)
                } else {
                    gpui::rgb(cpk_color(a.z))
                };
                color.a = (if outside { 0.14 } else { 1. }) * depth.atom_alpha(a.pos);
                let radius = atom_radius(a.z, style, scale);
                if !a.absorber && depth.options.active() && norm(sub(a.pos, depth.origin)) < 1e-6 {
                    disk(w, p, radius + 2., alpha(t.accent, 0.4 * color.a));
                }
                if color.a < 0.99 {
                    translucent_ball(
                        w,
                        p,
                        radius,
                        color,
                        shading && !outside && style != AtomStyle::Wireframe,
                    );
                    continue;
                }
                disk(w, p, radius, tint(color, 0.48));
                if shading && !outside && style != AtomStyle::Wireframe {
                    for layer in 0..12 {
                        let f = layer as f32 / 12.;
                        let q = [p[0] - radius * 0.23 * f, p[1] - radius * 0.26 * f, p[2]];
                        disk(
                            w,
                            q,
                            radius * (0.94 - 0.65 * f),
                            tint(color, 0.55 + 0.85 * f),
                        );
                    }
                } else {
                    disk(w, p, radius * 0.9, color);
                }
            }
        }
    }
    if scene.labels {
        for (_, atom) in scene.atoms.iter().enumerate().filter(|(i, a)| {
            !a.faded
                && atom_in_style(scene, style, *i)
                && depth.contains(a.pos)
                && depth.atom_alpha(a.pos) >= 0.2
        }) {
            let p = project(atom.pos);
            let text = gpui::SharedString::from(atom.label.clone());
            let run = gpui::TextRun {
                len: text.len(),
                font: w.text_style().font(),
                color: alpha(t.text, depth.atom_alpha(atom.pos)).into(),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let shaped = w.text_system().shape_line(text, px(11.), &[run], None);
            shaped
                .paint(
                    point(px(p[0] + 8.), px(p[1] - 15.)),
                    px(13.),
                    gpui::TextAlign::Left,
                    None,
                    w,
                    cx,
                )
                .ok();
        }
    }
    // Each traversal gets a parallel lane. Opposite legs remain distinguishable
    // even when they connect the same two atoms. All coordinates remain exact.
    for (i, pair) in scene.route.windows(2).enumerate() {
        for (part, inside) in depth.segments([pair[0], pair[1]]) {
            let p = project(part[0]);
            let q = project(part[1]);
            let dx = q[0] - p[0];
            let dy = q[1] - p[1];
            let len = dx.hypot(dy);
            if len < 1. {
                continue;
            }
            let active = leg.is_none_or(|n| n == i);
            let midpoint = std::array::from_fn(|a| (part[0][a] + part[1][a]) * 0.5);
            let color = alpha(
                crate::plotting::trace_rgba(&t, i % 8),
                (if active { 1. } else { 0.16 }) * depth.alpha(midpoint, inside),
            );
            let (ux, uy) = (dx / len, dy / len);
            let repeated = scene
                .route
                .windows(2)
                .take(i)
                .filter(|edge| {
                    norm(sub(edge[0], pair[0])) < 1e-6 && norm(sub(edge[1], pair[1])) < 1e-6
                })
                .count();
            let offset = 4. + repeated as f32 * 6.;
            let a = [p[0] - uy * offset, p[1] + ux * offset, p[2]];
            let z = [q[0] - uy * offset, q[1] + ux * offset, q[2]];
            line(w, &[a, z], color, if active { 2.5 } else { 1. }, false);
            let tip = [a[0] + dx * 0.67, a[1] + dy * 0.67, 0.];
            let head = 7_f32.min(len * 0.2);
            line(
                w,
                &[
                    [
                        tip[0] - ux * head - uy * head * 0.55,
                        tip[1] - uy * head + ux * head * 0.55,
                        0.,
                    ],
                    tip,
                    [
                        tip[0] - ux * head + uy * head * 0.55,
                        tip[1] - uy * head - ux * head * 0.55,
                        0.,
                    ],
                ],
                color,
                2.5,
                false,
            );
            if active && inside && color.a > 0.2 {
                let text = gpui::SharedString::from((i + 1).to_string());
                let style = w.text_style();
                let run = gpui::TextRun {
                    len: text.len(),
                    font: style.font(),
                    color: color.into(),
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                };
                let text = w.text_system().shape_line(text, px(11.), &[run], None);
                text.paint(
                    point(px(tip[0] - uy * 10.), px(tip[1] + ux * 10. - 6.)),
                    px(13.),
                    gpui::TextAlign::Left,
                    None,
                    w,
                    cx,
                )
                .ok();
            }
        }
    }
    // Only the explicit "Find absorber" annotation overlays the scene. The
    // highlighted sphere itself keeps its geometric depth and requested opacity.
    if highlight_absorber
        && absorber_label
        && let Some(atom) = scene.atoms.iter().find(|a| a.absorber)
        && depth.contains(atom.pos)
    {
        let p = project(atom.pos);
        if p[0] >= f32::from(b.left())
            && p[0] <= f32::from(b.right())
            && p[1] >= f32::from(b.top())
            && p[1] <= f32::from(b.bottom())
        {
            paint_absorber_marker(w, cx, p, atom, style, scale, t, b);
        }
    }
}

fn paint_absorber_marker(
    w: &mut Window,
    cx: &mut gpui::App,
    p: [f32; 3],
    atom: &SceneAtom,
    style: AtomStyle,
    scale: f32,
    t: Theme,
    b: Bounds<Pixels>,
) {
    let gold = gpui::rgb(0x67e8f9);
    let radius = if style == AtomStyle::Wireframe {
        8.
    } else {
        (covalent_radius(atom.z)
            * scale
            * if style == AtomStyle::Balls {
                0.62
            } else {
                0.31
            })
        .clamp(3., 24.)
            + 5.
    };
    let ring: Vec<_> = (0..=48)
        .map(|i| {
            let a = i as f32 * std::f32::consts::TAU / 48.;
            [p[0] + radius * a.cos(), p[1] + radius * a.sin(), p[2]]
        })
        .collect();
    line(w, &ring, gpui::rgb(0x151b23), 4.5, false);
    line(w, &ring, gold, 2.2, false);
    let text = gpui::SharedString::from(format!(
        "Absorber · {}",
        crate::structure::element_symbol(atom.z)
    ));
    let run = gpui::TextRun {
        len: text.len(),
        font: w.text_style().font(),
        color: gold.into(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let shaped = w.text_system().shape_line(text, px(12.), &[run], None);
    let width = f32::from(shaped.width) + 14.;
    let x = (p[0] + radius + 15.).clamp(
        f32::from(b.left()) + 6.,
        (f32::from(b.right()) - width - 6.).max(f32::from(b.left()) + 6.),
    );
    let y = (p[1] - radius - 35.).clamp(
        f32::from(b.top()) + 6.,
        (f32::from(b.bottom()) - 28.).max(f32::from(b.top()) + 6.),
    );
    let leader = [
        [p[0] + radius * 0.7, p[1] - radius * 0.7, p[2]],
        [x, y + 22., p[2]],
    ];
    line(w, &leader, gpui::rgb(0x151b23), 3., false);
    line(w, &leader, gold, 1.4, false);
    w.paint_quad(gpui::quad(
        Bounds::new(point(px(x), px(y)), size(px(width), px(24.))),
        gpui::Corners::all(px(5.)),
        alpha(t.surface, 0.96),
        gpui::Edges::all(px(1.)),
        gold,
        gpui::BorderStyle::Solid,
    ));
    shaped
        .paint(
            point(px(x + 7.), px(y + 4.)),
            px(16.),
            gpui::TextAlign::Left,
            None,
            w,
            cx,
        )
        .ok();
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rutile_repeats_complete_titanium_oxygen_octahedra() {
        let s = core::read_cif(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../rexafs/data/builtin_cifs/tio2_rutile.cif"),
        )
        .unwrap();
        let c = core::build_cluster(
            &s,
            &core::AbsorberSelection::Element("Ti".into()),
            &core::ClusterOptions::default(),
        )
        .unwrap();
        let context = crystal_context(&s, &c);
        let cluster = Cluster::from_core(&c);
        let scene = MoleculeScene::new(
            &cluster,
            Some(&context),
            8.,
            AtomStyle::Polyhedra,
            None,
            None,
            PolyhedronOptions::default(),
        );
        let centers = c.atoms.iter().filter(|a| a.symbol == "Ti").count();
        assert_eq!(scene.poly_count, centers);
        assert_eq!(scene.faces.len(), centers * 8);
        assert!(
            scene
                .faces
                .iter()
                .all(|f| f.vertices.len() == 3 && f.z == 22)
        );
        let single = MoleculeScene::new(
            &cluster,
            Some(&context),
            8.,
            AtomStyle::Polyhedra,
            None,
            None,
            PolyhedronOptions {
                network: false,
                ligand: Some(8),
                atoms: PolyAtoms::All,
                ..Default::default()
            },
        );
        assert_eq!(single.poly_count, 1);
        assert_eq!(single.bonds.len(), 6);
        assert_eq!(single.poly_atoms.len(), 7);
        assert!(
            single
                .faces
                .iter()
                .all(|f| (norm(f.normal) - 1.).abs() < 1e-8)
        );
    }
    #[test]
    fn coordination_hulls_keep_coplanar_faces() {
        let cube: Vec<_> = (0..8)
            .map(|n| std::array::from_fn(|i| if n & (1 << i) == 0 { -1. } else { 1. }))
            .collect();
        let faces = hull_faces(&cube);
        assert_eq!(faces.len(), 6);
        assert!(faces.iter().all(|f| f.len() == 4));
        assert_eq!(
            hull_faces(&[
                [1., 0., 0.],
                [-1., 0., 0.],
                [0., 1., 0.],
                [0., -1., 0.],
                [0., 0., 1.],
                [0., 0., -1.]
            ])
            .len(),
            8
        );
    }
    #[test]
    fn large_scroll_event_keeps_cluster_in_view() {
        let mut camera = ViewCamera::default();
        camera.scroll_zoom(gpui::ScrollDelta::Lines(point(0., -120.)));
        assert!((1.0..1.17).contains(&camera.zoom));
        camera.scroll_zoom(gpui::ScrollDelta::Lines(point(0., 120.)));
        assert!((camera.zoom - 1.).abs() < 1e-12);
        let before = camera;
        camera.zoom_by(1.2_f64.ln());
        assert!((camera.zoom - 1.2).abs() < 1e-12);
        assert_eq!((camera.az, camera.el), (before.az, before.el));
    }

    #[test]
    fn orbit_preserves_scale_and_distance() {
        let b = Bounds::new(point(px(0.), px(0.)), size(px(600.), px(400.)));
        let a = ViewCamera::default();
        let c = ViewCamera {
            az: 2.,
            el: 0.9,
            ..a
        };
        assert_eq!(a.scale(b, 8.), c.scale(b, 8.));
        assert!((norm(a.rotate([1., 2., 3.])) - norm(c.rotate([1., 2., 3.]))).abs() < 1e-10);
    }
}
