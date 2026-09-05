//! Display-only slicing and depth cues. Distances are Cartesian Å relative to
//! the inspected atom; clipping never changes cluster atoms or FEFF paths.
use super::molecule_view::{MoleculeScene, ViewCamera};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum SliceMode {
    #[default]
    Off,
    Slab,
    Cutaway,
}
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum DepthAxis {
    #[default]
    View,
    X,
    Y,
    Z,
}
impl DepthAxis {
    pub fn label(self) -> &'static str {
        match self {
            Self::View => "View",
            Self::X => "X",
            Self::Y => "Y",
            Self::Z => "Z",
        }
    }
    pub fn normal(self, camera: ViewCamera) -> [f64; 3] {
        match self {
            Self::View => [
                camera.el.cos() * camera.az.sin(),
                camera.el.cos() * camera.az.cos(),
                camera.el.sin(),
            ],
            Self::X => [1., 0., 0.],
            Self::Y => [0., 1., 0.],
            Self::Z => [0., 0., 1.],
        }
    }
}
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum FadeMode {
    #[default]
    Off,
    Depth,
    Center,
}
#[derive(Clone, Copy)]
pub(crate) struct DepthOptions {
    pub slice: SliceMode,
    pub axis: DepthAxis,
    pub offset: f64,
    pub thickness: f64,
    pub ghost: bool,
    pub fade: FadeMode,
    pub opacity: f64,
    pub strength: f64,
    pub focus_radius: f64,
}
impl Default for DepthOptions {
    fn default() -> Self {
        Self {
            slice: SliceMode::Off,
            axis: DepthAxis::View,
            offset: 0.,
            thickness: 4.,
            ghost: false,
            fade: FadeMode::Off,
            opacity: 1.,
            strength: 0.85,
            focus_radius: 3.,
        }
    }
}
impl DepthOptions {
    pub fn active(self) -> bool {
        self.slice != SliceMode::Off || self.fade != FadeMode::Off || self.opacity < 0.999
    }
}
#[derive(Clone, Copy)]
pub(crate) struct DepthFrame {
    pub options: DepthOptions,
    pub origin: [f64; 3],
    pub normal: [f64; 3],
    pub view_normal: [f64; 3],
    pub span: [f64; 2],
}
pub(crate) fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| a[i] * b[i]).sum()
}
pub(crate) fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| a[i] - b[i])
}
fn lerp(a: [f64; 3], b: [f64; 3], t: f64) -> [f64; 3] {
    std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t)
}
impl DepthFrame {
    pub fn new(
        options: DepthOptions,
        scene: &MoleculeScene,
        camera: ViewCamera,
        picked: Option<usize>,
    ) -> Self {
        let origin = scene
            .atoms
            .iter()
            .find(|a| a.index.is_some() && a.index == picked)
            .or_else(|| scene.atoms.iter().find(|a| a.absorber))
            .map(|a| a.pos)
            .unwrap_or(scene.center);
        let view_normal = DepthAxis::View.normal(camera);
        let mut span = [f64::INFINITY, f64::NEG_INFINITY];
        for a in scene.atoms.iter().filter(|a| !a.faded) {
            let d = dot(sub(a.pos, origin), view_normal);
            span[0] = span[0].min(d);
            span[1] = span[1].max(d);
        }
        if !span[0].is_finite() || span[1] - span[0] < 1e-6 {
            span = [-1., 1.];
        }
        Self {
            options,
            origin,
            normal: options.axis.normal(camera),
            view_normal,
            span,
        }
    }
    pub fn depth(self, p: [f64; 3]) -> f64 {
        dot(sub(p, self.origin), self.normal)
    }
    pub fn limits(self) -> [f64; 2] {
        match self.options.slice {
            SliceMode::Off => [f64::NEG_INFINITY, f64::INFINITY],
            SliceMode::Slab => [
                self.options.offset - self.options.thickness * 0.5,
                self.options.offset + self.options.thickness * 0.5,
            ],
            SliceMode::Cutaway => [f64::NEG_INFINITY, self.options.offset],
        }
    }
    pub fn contains(self, p: [f64; 3]) -> bool {
        let d = self.depth(p);
        let [lo, hi] = self.limits();
        d >= lo - 1e-8 && d <= hi + 1e-8
    }
    /// Fade uses camera depth even when a Cartesian slicing axis is selected.
    pub fn alpha(self, p: [f64; 3], inside: bool) -> f32 {
        if !inside && !self.options.ghost {
            return 0.;
        }
        let cue = match self.options.fade {
            FadeMode::Off => 1.,
            FadeMode::Depth => ((dot(sub(p, self.origin), self.view_normal) - self.span[0])
                / (self.span[1] - self.span[0]))
                .clamp(0., 1.),
            FadeMode::Center => {
                let d = dot(sub(p, self.origin), sub(p, self.origin)).sqrt();
                let r = self.options.focus_radius.max(0.1);
                // Keep the requested coordination sphere clear, then fade over
                // a narrow transition instead of leaving several outer shells
                // almost opaque. The transition stays smooth at the boundary.
                let feather = (r * 0.35).max(0.5);
                (-((d - r).max(0.) / feather).powi(2)).exp()
            }
        };
        (self.options.opacity
            * (1. - self.options.strength * (1. - cue))
            * if inside { 1. } else { 0.06 }) as f32
    }
    pub fn atom_alpha(self, p: [f64; 3]) -> f32 {
        self.alpha(p, self.contains(p))
    }
    /// Picking follows visible geometry, never the hidden/ghost context.
    pub fn pickable(self, p: [f64; 3]) -> bool {
        self.contains(p) && self.atom_alpha(p) >= 0.08
    }
    /// Partition a segment at the planes. Crossing bonds remain visible even
    /// if both end atoms lie outside a thin slab.
    pub fn segments(self, edge: [[f64; 3]; 2]) -> Vec<([[f64; 3]; 2], bool)> {
        let [a, b] = edge;
        let da = self.depth(a);
        let db = self.depth(b);
        let mut ts = vec![0., 1.];
        if (db - da).abs() > 1e-12 {
            for limit in self.limits() {
                let t = (limit - da) / (db - da);
                if t > 0. && t < 1. {
                    ts.push(t);
                }
            }
        }
        ts.sort_by(f64::total_cmp);
        ts.dedup_by(|a, b| (*a - *b).abs() < 1e-12);
        ts.windows(2)
            .filter_map(|t| {
                let points = [lerp(a, b, t[0]), lerp(a, b, t[1])];
                let inside = self.contains(lerp(a, b, (t[0] + t[1]) * 0.5));
                (inside || self.options.ghost).then_some((points, inside))
            })
            .collect()
    }
    /// Split convex faces, retaining disjoint outside pieces for ghost mode.
    /// Cut faces are open surfaces, not newly invented coordination faces.
    pub fn polygons(self, vertices: &[[f64; 3]]) -> Vec<(Vec<[f64; 3]>, bool)> {
        let mut kept = vertices.to_vec();
        let mut parts = Vec::new();
        for (limit, sign) in [(self.limits()[0], 1.), (self.limits()[1], -1.)] {
            if !limit.is_finite() || kept.len() < 3 {
                continue;
            }
            let clip = |positive: bool| {
                let mut out = Vec::new();
                for i in 0..kept.len() {
                    let a = kept[i];
                    let b = kept[(i + 1) % kept.len()];
                    let da = (self.depth(a) - limit) * sign * if positive { 1. } else { -1. };
                    let db = (self.depth(b) - limit) * sign * if positive { 1. } else { -1. };
                    let a_inside = if positive { da >= 0. } else { da > 0. };
                    let b_inside = if positive { db >= 0. } else { db > 0. };
                    if a_inside {
                        out.push(a);
                    }
                    if a_inside != b_inside {
                        out.push(lerp(a, b, da / (da - db)));
                    }
                }
                out
            };
            if self.options.ghost {
                let outside = clip(false);
                if outside.len() >= 3 {
                    parts.push((outside, false));
                }
            }
            kept = clip(true);
        }
        if kept.len() >= 3 {
            parts.push((kept, true));
        }
        parts
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn slab() -> DepthFrame {
        DepthFrame {
            options: DepthOptions {
                slice: SliceMode::Slab,
                thickness: 2.,
                ..Default::default()
            },
            origin: [0.; 3],
            normal: [0., 0., 1.],
            view_normal: [0., 0., 1.],
            span: [-4., 4.],
        }
    }
    #[test]
    fn crossing_bonds_are_clipped_with_both_atoms_outside() {
        let f = slab();
        let parts = f.segments([[0., 0., -3.], [0., 0., 3.]]);
        assert_eq!(parts, vec![([[0., 0., -1.], [0., 0., 1.]], true)]);
        assert!(!f.pickable([0., 0., 3.]));
    }
    #[test]
    fn polyhedron_faces_are_intersected_not_removed_by_centroid() {
        let f = slab();
        let parts = f.polygons(&[[-2., 0., -3.], [2., 0., -3.], [2., 0., 3.], [-2., 0., 3.]]);
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].0.len(), 4);
        for v in &parts[0].0 {
            assert!(f.contains(*v));
        }
        assert!(
            (parts[0]
                .0
                .iter()
                .map(|v| v[2])
                .fold(f64::NEG_INFINITY, f64::max)
                - 1.)
                .abs()
                < 1e-8
        );
    }
    #[test]
    fn ghost_parts_do_not_duplicate_inside_geometry() {
        let mut f = slab();
        f.options.ghost = true;
        let parts = f.segments([[0., 0., -3.], [0., 0., 3.]]);
        assert_eq!(parts.iter().filter(|p| p.1).count(), 1);
        assert_eq!(parts.len(), 3);
        assert!((f.atom_alpha([0., 0., 3.]) - 0.06).abs() < 1e-6);
        assert!(!f.pickable([0., 0., 3.]));
    }
    #[test]
    fn coplanar_boundary_faces_are_not_drawn_twice_in_ghost_mode() {
        let mut f = slab();
        f.options.ghost = true;
        let parts = f.polygons(&[[0., 0., 1.], [1., 0., 1.], [0., 1., 1.]]);
        assert_eq!(parts.len(), 1);
        assert!(parts[0].1);
    }
    #[test]
    fn cutaway_removes_foreground_relative_to_selected_center() {
        let mut f = slab();
        f.options.slice = SliceMode::Cutaway;
        f.origin = [0., 0., 5.];
        assert!(f.contains([0., 0., 4.]));
        assert!(f.contains([0., 0., 5.]));
        assert!(!f.contains([0., 0., 6.]));
    }
    #[test]
    fn depth_and_coordination_fades_have_defined_direction() {
        let mut f = slab();
        f.options.slice = SliceMode::Off;
        f.options.fade = FadeMode::Depth;
        assert!(f.atom_alpha([0., 0., -4.]) < f.atom_alpha([0., 0., 4.]));
        f.options.fade = FadeMode::Center;
        f.options.focus_radius = 2.;
        assert_eq!(f.atom_alpha([0., 0., 0.]), 1.);
        assert_eq!(f.atom_alpha([2., 0., 0.]), 1.);
        assert!(f.atom_alpha([3.2, 0., 0.]) < 0.3);
        assert!(f.atom_alpha([6., 0., 0.]) < 0.2);
    }
    #[test]
    fn view_axis_matches_camera_depth_and_cartesian_axis_stays_fixed() {
        let camera = ViewCamera {
            az: 0.7,
            el: -0.4,
            zoom: 1.,
        };
        let p = [1., 2., 3.];
        let n = DepthAxis::View.normal(camera);
        assert!((dot(n, n) - 1.).abs() < 1e-12);
        assert!((dot(p, n) - camera.rotate(p)[2]).abs() < 1e-12);
        assert_eq!(DepthAxis::Z.normal(camera), [0., 0., 1.]);
    }
}
