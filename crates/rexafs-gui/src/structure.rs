//! GUI-side structure model for the Fit stage's cluster / path visualizer.
//!
//! The view model (a [`Cluster`] of positioned atoms and one [`PathGeometry`]
//! per FEFF path) is built from the core crate's `xafs::structure` module:
//! clusters generated from a crystal structure come straight from
//! `build_cluster`, path legs from `PathGeometry::from_feffdat`. Two fallbacks
//! keep imported workspaces working: the `ATOMS` block of a hand-made
//! `feff.inp` and the leg table of a `feffNNNN.dat` are parsed here.
//!
//! Structure *sources* (built-in recipes, a CIF folder, Materials Project,
//! AMCSD) sit behind [`StructureProvider`], a thin adapter over the core
//! `StructureSource` trait that also carries the GUI's source configuration.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use rexafs::xafs::structure::{self as core, StructureSource};

/// Periodic-table row: symbol, Jmol CPK colour, covalent radius (Å).
struct ElementInfo(&'static str, u32, f32);

/// Z = 1..=96. Colours follow the Jmol CPK table; radii are Cordero covalent
/// radii, rounded.
const ELEMENTS: [ElementInfo; 96] = [
    ElementInfo("H", 0xffffff, 0.31),
    ElementInfo("He", 0xd9ffff, 0.28),
    ElementInfo("Li", 0xcc80ff, 1.28),
    ElementInfo("Be", 0xc2ff00, 0.96),
    ElementInfo("B", 0xffb5b5, 0.84),
    ElementInfo("C", 0x909090, 0.76),
    ElementInfo("N", 0x3050f8, 0.71),
    ElementInfo("O", 0xff0d0d, 0.66),
    ElementInfo("F", 0x90e050, 0.57),
    ElementInfo("Ne", 0xb3e3f5, 0.58),
    ElementInfo("Na", 0xab5cf2, 1.66),
    ElementInfo("Mg", 0x8aff00, 1.41),
    ElementInfo("Al", 0xbfa6a6, 1.21),
    ElementInfo("Si", 0xf0c8a0, 1.11),
    ElementInfo("P", 0xff8000, 1.07),
    ElementInfo("S", 0xffff30, 1.05),
    ElementInfo("Cl", 0x1ff01f, 1.02),
    ElementInfo("Ar", 0x80d1e3, 1.06),
    ElementInfo("K", 0x8f40d4, 2.03),
    ElementInfo("Ca", 0x3dff00, 1.76),
    ElementInfo("Sc", 0xe6e6e6, 1.70),
    ElementInfo("Ti", 0xbfc2c7, 1.60),
    ElementInfo("V", 0xa6a6ab, 1.53),
    ElementInfo("Cr", 0x8a99c7, 1.39),
    ElementInfo("Mn", 0x9c7ac7, 1.39),
    ElementInfo("Fe", 0xe06633, 1.32),
    ElementInfo("Co", 0xf090a0, 1.26),
    ElementInfo("Ni", 0x50d050, 1.24),
    ElementInfo("Cu", 0xc88033, 1.32),
    ElementInfo("Zn", 0x7d80b0, 1.22),
    ElementInfo("Ga", 0xc28f8f, 1.22),
    ElementInfo("Ge", 0x668f8f, 1.20),
    ElementInfo("As", 0xbd80e3, 1.19),
    ElementInfo("Se", 0xffa100, 1.20),
    ElementInfo("Br", 0xa62929, 1.20),
    ElementInfo("Kr", 0x5cb8d1, 1.16),
    ElementInfo("Rb", 0x702eb0, 2.20),
    ElementInfo("Sr", 0x00ff00, 1.95),
    ElementInfo("Y", 0x94ffff, 1.90),
    ElementInfo("Zr", 0x94e0e0, 1.75),
    ElementInfo("Nb", 0x73c2c9, 1.64),
    ElementInfo("Mo", 0x54b5b5, 1.54),
    ElementInfo("Tc", 0x3b9e9e, 1.47),
    ElementInfo("Ru", 0x248f8f, 1.46),
    ElementInfo("Rh", 0x0a7d8c, 1.42),
    ElementInfo("Pd", 0x006985, 1.39),
    ElementInfo("Ag", 0xc0c0c0, 1.45),
    ElementInfo("Cd", 0xffd98f, 1.44),
    ElementInfo("In", 0xa67573, 1.42),
    ElementInfo("Sn", 0x668080, 1.39),
    ElementInfo("Sb", 0x9e63b5, 1.39),
    ElementInfo("Te", 0xd47a00, 1.38),
    ElementInfo("I", 0x940094, 1.39),
    ElementInfo("Xe", 0x429eb0, 1.40),
    ElementInfo("Cs", 0x57178f, 2.44),
    ElementInfo("Ba", 0x00c900, 2.15),
    ElementInfo("La", 0x70d4ff, 2.07),
    ElementInfo("Ce", 0xffffc7, 2.04),
    ElementInfo("Pr", 0xd9ffc7, 2.03),
    ElementInfo("Nd", 0xc7ffc7, 2.01),
    ElementInfo("Pm", 0xa3ffc7, 1.99),
    ElementInfo("Sm", 0x8fffc7, 1.98),
    ElementInfo("Eu", 0x61ffc7, 1.98),
    ElementInfo("Gd", 0x45ffc7, 1.96),
    ElementInfo("Tb", 0x30ffc7, 1.94),
    ElementInfo("Dy", 0x1fffc7, 1.92),
    ElementInfo("Ho", 0x00ff9c, 1.92),
    ElementInfo("Er", 0x00e675, 1.89),
    ElementInfo("Tm", 0x00d452, 1.90),
    ElementInfo("Yb", 0x00bf38, 1.87),
    ElementInfo("Lu", 0x00ab24, 1.87),
    ElementInfo("Hf", 0x4dc2ff, 1.75),
    ElementInfo("Ta", 0x4da6ff, 1.70),
    ElementInfo("W", 0x2194d6, 1.62),
    ElementInfo("Re", 0x266696, 1.51),
    ElementInfo("Os", 0x266696, 1.44),
    ElementInfo("Ir", 0x175487, 1.41),
    ElementInfo("Pt", 0xd0d0e0, 1.36),
    ElementInfo("Au", 0xffd123, 1.36),
    ElementInfo("Hg", 0xb8b8d0, 1.32),
    ElementInfo("Tl", 0xa6544d, 1.45),
    ElementInfo("Pb", 0x575961, 1.46),
    ElementInfo("Bi", 0x9e4fb5, 1.48),
    ElementInfo("Po", 0xab5c00, 1.40),
    ElementInfo("At", 0x754f45, 1.50),
    ElementInfo("Rn", 0x428296, 1.50),
    ElementInfo("Fr", 0x420066, 2.60),
    ElementInfo("Ra", 0x007d00, 2.21),
    ElementInfo("Ac", 0x70abfa, 2.15),
    ElementInfo("Th", 0x00baff, 2.06),
    ElementInfo("Pa", 0x00a1ff, 2.00),
    ElementInfo("U", 0x008fff, 1.96),
    ElementInfo("Np", 0x0080ff, 1.90),
    ElementInfo("Pu", 0x006bff, 1.87),
    ElementInfo("Am", 0x545cf2, 1.80),
    ElementInfo("Cm", 0x785ce3, 1.69),
];

/// Element symbol for an atomic number, `"X"` when unknown.
pub fn element_symbol(z: u32) -> &'static str {
    ELEMENTS
        .get(z.wrapping_sub(1) as usize)
        .map(|e| e.0)
        .unwrap_or("X")
}

/// Atomic number for a symbol (case-insensitive), if known.
pub fn atomic_number(symbol: &str) -> Option<u32> {
    let s = symbol.trim();
    ELEMENTS
        .iter()
        .position(|e| e.0.eq_ignore_ascii_case(s))
        .map(|i| i as u32 + 1)
}

/// Jmol CPK colour as 0xRRGGBB.
pub fn cpk_color(z: u32) -> u32 {
    ELEMENTS
        .get(z.wrapping_sub(1) as usize)
        .map(|e| e.1)
        .unwrap_or(0xb0b0b0)
}

/// Covalent radius in Å (1.4 when unknown).
pub fn covalent_radius(z: u32) -> f32 {
    ELEMENTS
        .get(z.wrapping_sub(1) as usize)
        .map(|e| e.2)
        .unwrap_or(1.4)
}

/// A potential index declared in the `POTENTIALS` block.
#[derive(Clone, Debug, PartialEq)]
pub struct Potential {
    pub ipot: usize,
    pub z: u32,
    pub symbol: String,
}

/// One atom of the FEFF cluster (Cartesian Å, absorber at the origin).
#[derive(Clone, Debug, PartialEq)]
pub struct ClusterAtom {
    pub pos: [f64; 3],
    pub ipot: usize,
    pub z: u32,
    pub symbol: String,
    pub tag: String,
    /// Distance from the absorber.
    pub dist: f64,
    /// Index of the coordination shell this atom belongs to (0 = absorber).
    pub shell: usize,
}

/// Parsed cluster.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Cluster {
    pub title: String,
    pub potentials: Vec<Potential>,
    pub atoms: Vec<ClusterAtom>,
    /// Coordination shells: (mean distance, atom count), ascending.
    pub shells: Vec<(f64, usize)>,
}

impl Cluster {
    pub fn is_empty(&self) -> bool {
        self.atoms.is_empty()
    }

    pub fn absorber(&self) -> Option<&ClusterAtom> {
        self.atoms.iter().find(|a| a.ipot == 0)
    }

    /// Element counts (symbol, Z, n) in order of first appearance.
    pub fn element_counts(&self) -> Vec<(String, u32, usize)> {
        let mut out: Vec<(String, u32, usize)> = Vec::new();
        for a in &self.atoms {
            match out.iter_mut().find(|(s, _, _)| *s == a.symbol) {
                Some(e) => e.2 += 1,
                None => out.push((a.symbol.clone(), a.z, 1)),
            }
        }
        out
    }

    /// Nearest atom index to a point, within `tol` Å.
    pub fn nearest(&self, pos: [f64; 3], tol: f64) -> Option<usize> {
        let mut best: Option<(usize, f64)> = None;
        for (i, a) in self.atoms.iter().enumerate() {
            let d = dist3(a.pos, pos);
            if d <= tol && best.is_none_or(|(_, bd)| d < bd) {
                best = Some((i, d));
            }
        }
        best.map(|(i, _)| i)
    }
}

impl Cluster {
    /// View of a cluster generated by the core builder.
    pub fn from_core(c: &core::Cluster) -> Self {
        let potentials = c
            .potentials
            .iter()
            .map(|p| Potential {
                ipot: p.ipot as usize,
                z: p.z as u32,
                symbol: p.symbol.clone(),
            })
            .collect();
        let mut atoms: Vec<ClusterAtom> = c
            .atoms
            .iter()
            .map(|a| ClusterAtom {
                pos: a.cart,
                ipot: a.ipot as usize,
                z: a.z as u32,
                symbol: a.symbol.clone(),
                tag: a.label.clone(),
                dist: a.distance,
                shell: 0,
            })
            .collect();
        let shells = assign_shells(&mut atoms, 0.02);
        Self {
            title: c.structure_title.clone(),
            potentials,
            atoms,
            shells,
        }
    }
}

fn dist3(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// Group atoms into coordination shells: distances within `width` Å of the
/// running shell mean share a shell.
fn assign_shells(atoms: &mut [ClusterAtom], width: f64) -> Vec<(f64, usize)> {
    let mut order: Vec<usize> = (0..atoms.len()).collect();
    order.sort_by(|&a, &b| atoms[a].dist.total_cmp(&atoms[b].dist));
    let mut shells: Vec<(f64, usize)> = Vec::new();
    for i in order {
        let d = atoms[i].dist;
        match shells.last_mut() {
            Some((mean, n)) if (d - *mean).abs() <= width => {
                *mean = (*mean * *n as f64 + d) / (*n as f64 + 1.0);
                *n += 1;
            }
            _ => shells.push((d, 1)),
        }
        atoms[i].shell = shells.len() - 1;
    }
    shells
}

/// Parse the `POTENTIALS` and `ATOMS` blocks of a `feff.inp`.
pub fn parse_feff_inp(text: &str) -> Cluster {
    #[derive(PartialEq)]
    enum Block {
        None,
        Potentials,
        Atoms,
    }
    let mut block = Block::None;
    let mut cluster = Cluster::default();
    for raw in text.lines() {
        let line = raw.split('*').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let upper = line.to_ascii_uppercase();
        let first = upper.split_whitespace().next().unwrap_or("");
        match first {
            "TITLE" => {
                if cluster.title.is_empty() {
                    cluster.title = line[5..].trim().to_string();
                }
                block = Block::None;
                continue;
            }
            "POTENTIALS" => {
                block = Block::Potentials;
                continue;
            }
            "ATOMS" => {
                block = Block::Atoms;
                continue;
            }
            "END" => {
                block = Block::None;
                continue;
            }
            _ => {}
        }
        // Any other card keyword ends a block.
        if first.chars().all(|c| c.is_ascii_uppercase() || c == '_')
            && first.len() > 1
            && first.parse::<f64>().is_err()
        {
            block = Block::None;
            continue;
        }
        let cols: Vec<&str> = line.split_whitespace().collect();
        match block {
            Block::Potentials => {
                if cols.len() >= 2
                    && let (Ok(ipot), Ok(z)) = (cols[0].parse::<usize>(), cols[1].parse::<u32>())
                {
                    let symbol = cols
                        .get(2)
                        .map(|s| s.trim_matches(|c: char| !c.is_ascii_alphabetic()))
                        .filter(|s| !s.is_empty())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| element_symbol(z).to_string());
                    cluster.potentials.push(Potential { ipot, z, symbol });
                }
            }
            Block::Atoms => {
                if cols.len() >= 4
                    && let (Ok(x), Ok(y), Ok(zc), Ok(ipot)) = (
                        cols[0].parse::<f64>(),
                        cols[1].parse::<f64>(),
                        cols[2].parse::<f64>(),
                        cols[3].parse::<usize>(),
                    )
                {
                    let tag = cols.get(4).map(|s| s.to_string()).unwrap_or_default();
                    let (z, symbol) = match cluster.potentials.iter().find(|p| p.ipot == ipot) {
                        Some(p) => (p.z, p.symbol.clone()),
                        None => {
                            let sym: String = tag
                                .chars()
                                .take_while(|c| c.is_ascii_alphabetic())
                                .collect();
                            (atomic_number(&sym).unwrap_or(0), sym)
                        }
                    };
                    let pos = [x, y, zc];
                    cluster.atoms.push(ClusterAtom {
                        pos,
                        ipot,
                        z,
                        symbol,
                        tag,
                        dist: dist3(pos, [0.0; 3]),
                        shell: 0,
                    });
                }
            }
            Block::None => {}
        }
    }
    cluster.shells = assign_shells(&mut cluster.atoms, 0.02);
    cluster
}

/// Read and parse `<workspace>/feff.inp`.
pub fn load_cluster(workspace: &Path) -> Option<Cluster> {
    let text = std::fs::read_to_string(workspace.join("feff.inp")).ok()?;
    let c = parse_feff_inp(&text);
    (!c.is_empty()).then_some(c)
}

/// One leg of a scattering path.
#[derive(Clone, Debug, PartialEq)]
pub struct PathLeg {
    pub pos: [f64; 3],
    pub ipot: usize,
    pub z: u32,
    pub symbol: String,
    /// Cluster atom this leg was mapped onto (core `map_to_cluster`).
    pub atom: Option<usize>,
}

/// Geometry of one FEFF path as listed in its `feffNNNN.dat`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PathGeometry {
    pub file: PathBuf,
    pub nleg: usize,
    pub degen: f64,
    pub reff: f64,
    /// Legs in scattering order; the first entry is the absorber.
    pub legs: Vec<PathLeg>,
    /// Relative importance in [0, 1] (see [`path_importance`]).
    pub importance: f64,
}

impl PathGeometry {
    /// Human label such as `Ru–Ru` or `Ru–O–Ru` (scatterers between the
    /// absorber and the return to it).
    pub fn label(&self) -> String {
        if self.legs.is_empty() {
            return String::new();
        }
        let mut parts: Vec<&str> = vec![self.legs[0].symbol.as_str()];
        for leg in self.legs.iter().skip(1) {
            parts.push(leg.symbol.as_str());
        }
        parts.join("–")
    }

    /// `true` for single scattering (absorber → one atom → absorber).
    pub fn is_single_scattering(&self) -> bool {
        self.nleg <= 2
    }

    /// Cluster atom index for every scatterer leg (skipping the absorber):
    /// the core mapping when the path came from a generated cluster, else
    /// the nearest atom within 0.05 Å.
    pub fn atom_indices(&self, cluster: &Cluster) -> Vec<Option<usize>> {
        self.legs
            .iter()
            .skip(1)
            .map(|leg| leg.atom.or_else(|| cluster.nearest(leg.pos, 0.05)))
            .collect()
    }

    /// Build from the core geometry of a parsed path file.
    pub fn from_core(geo: &core::PathGeometry, file: &Path, importance: f64) -> Self {
        Self {
            file: file.to_path_buf(),
            nleg: geo.nleg,
            degen: geo.degen,
            reff: geo.reff,
            legs: geo
                .legs
                .iter()
                .map(|l| PathLeg {
                    pos: l.cart,
                    ipot: l.ipot as usize,
                    z: l.z as u32,
                    symbol: l.symbol.clone(),
                    atom: l.cluster_atom,
                })
                .collect(),
            importance,
        }
    }

    /// Closed polyline through the absorber and every scatterer.
    pub fn polyline(&self) -> Vec<[f64; 3]> {
        let mut pts: Vec<[f64; 3]> = self.legs.iter().map(|l| l.pos).collect();
        if let Some(first) = pts.first().copied() {
            pts.push(first);
        }
        pts
    }
}

/// Parse the header and geometry table of a `feffNNNN.dat`.
///
/// Returns `None` when the file has no `nleg, deg, reff` line.
pub fn parse_path_dat(text: &str, file: &Path) -> Option<PathGeometry> {
    let mut lines = text.lines();
    let mut header: Option<(usize, f64, f64)> = None;
    for line in lines.by_ref() {
        if line.contains("nleg") && line.contains("reff") {
            let nums: Vec<f64> = line
                .split_whitespace()
                .take_while(|t| t.parse::<f64>().is_ok())
                .filter_map(|t| t.parse().ok())
                .collect();
            if nums.len() >= 3 {
                header = Some((nums[0] as usize, nums[1], nums[2]));
            }
            break;
        }
    }
    let (nleg, degen, reff) = header?;
    let mut legs = Vec::new();
    let mut mag = Vec::new();
    let mut in_table = false;
    for line in lines {
        let l = line.trim();
        if l.is_empty() {
            continue;
        }
        if l.starts_with('x') && l.contains("pot") {
            in_table = true;
            continue;
        }
        if l.starts_with('k') && l.contains("real") {
            in_table = false;
            continue;
        }
        let cols: Vec<&str> = l.split_whitespace().collect();
        if in_table {
            if cols.len() >= 5
                && let (Ok(x), Ok(y), Ok(z), Ok(ipot), Ok(zn)) = (
                    cols[0].parse::<f64>(),
                    cols[1].parse::<f64>(),
                    cols[2].parse::<f64>(),
                    cols[3].parse::<usize>(),
                    cols[4].parse::<u32>(),
                )
            {
                let symbol = cols
                    .get(5)
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| element_symbol(zn).to_string());
                legs.push(PathLeg {
                    pos: [x, y, z],
                    ipot,
                    z: zn,
                    symbol,
                    atom: None,
                });
            }
        } else if cols.len() >= 3
            && let (Ok(k), Ok(m)) = (cols[0].parse::<f64>(), cols[2].parse::<f64>())
        {
            mag.push((k, m));
        }
    }
    let importance = raw_importance(degen, reff, &mag);
    Some(PathGeometry {
        file: file.to_path_buf(),
        nleg,
        degen,
        reff,
        legs,
        importance,
    })
}

/// Un-normalised importance: degeneracy × mean |feff| over k = 3…12 Å⁻¹,
/// divided by R², i.e. the amplitude prefactor of the EXAFS equation.
fn raw_importance(degen: f64, reff: f64, mag: &[(f64, f64)]) -> f64 {
    let (sum, n) = mag
        .iter()
        .filter(|(k, _)| (3.0..=12.0).contains(k))
        .fold((0.0, 0usize), |(s, n), (_, m)| (s + m, n + 1));
    if n == 0 || reff <= 0.0 {
        return 0.0;
    }
    degen * (sum / n as f64) / (reff * reff)
}

/// Load the geometry of a path file through the core FEFF parser, mapping
/// its legs onto `cluster` when one is known; falls back to the local table
/// parser for files the core reader rejects.
pub fn load_path_geometry(file: &Path, cluster: Option<&core::Cluster>) -> Option<PathGeometry> {
    let core_geo = rexafs::prelude::parse_feff_path_file(
        file.to_string_lossy().as_ref(),
        rexafs::prelude::FeffFlavor::Feff85L,
    )
    .ok()
    .and_then(|dat| {
        let mut geo = core::PathGeometry::from_feffdat(&dat).ok()?;
        if let Some(c) = cluster {
            geo.map_to_cluster(c, 0.05);
        }
        let mag: Vec<(f64, f64)> = dat
            .k
            .iter()
            .zip(dat.mag_feff.iter())
            .map(|(k, m)| (*k, *m))
            .collect();
        Some(PathGeometry::from_core(
            &geo,
            file,
            raw_importance(geo.degen, geo.reff, &mag),
        ))
    });
    core_geo.or_else(|| {
        let text = std::fs::read_to_string(file).ok()?;
        parse_path_dat(&text, file)
    })
}

/// Normalise importances so the strongest path is 1.
pub fn normalise_importance(paths: &mut [Option<PathGeometry>]) {
    let max = paths
        .iter()
        .flatten()
        .map(|p| p.importance)
        .fold(0.0_f64, f64::max);
    if max > 0.0 {
        for p in paths.iter_mut().flatten() {
            p.importance /= max;
        }
    }
}

// ---------------------------------------------------------------------------
// Structure sources (adapters over `xafs::structure::StructureSource`)
// ---------------------------------------------------------------------------

/// Where a structure hit came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StructureSourceKind {
    Builtin,
    LocalCif,
    MaterialsProject,
    Amcsd,
    Cod,
}

impl StructureSourceKind {
    pub const ALL: [StructureSourceKind; 5] = [
        StructureSourceKind::Builtin,
        StructureSourceKind::LocalCif,
        StructureSourceKind::MaterialsProject,
        StructureSourceKind::Amcsd,
        StructureSourceKind::Cod,
    ];

    pub fn label(self) -> &'static str {
        match self {
            StructureSourceKind::Builtin => "Curated",
            StructureSourceKind::LocalCif => "CIF folder",
            StructureSourceKind::MaterialsProject => "Materials Project",
            StructureSourceKind::Amcsd => "AMCSD",
            StructureSourceKind::Cod => "COD",
        }
    }

    pub fn badge(self) -> &'static str {
        match self {
            StructureSourceKind::Builtin => "built-in",
            StructureSourceKind::LocalCif => "cif",
            StructureSourceKind::MaterialsProject => "mp",
            StructureSourceKind::Amcsd => "amcsd",
            StructureSourceKind::Cod => "cod",
        }
    }
}

/// A search result.
#[derive(Clone, Debug, PartialEq)]
pub struct StructureHit {
    pub id: String,
    pub formula: String,
    pub name: String,
    pub space_group: String,
    pub source: StructureSourceKind,
    /// The core hit this came from (built-ins have none).
    pub core: Option<core::StructureHit>,
}

impl StructureHit {
    fn from_core(hit: core::StructureHit, source: StructureSourceKind) -> Self {
        Self {
            id: hit.id.clone(),
            formula: hit.formula.clone(),
            name: hit.name.clone().unwrap_or_default(),
            space_group: hit.space_group.clone().unwrap_or_default(),
            source,
            core: Some(hit),
        }
    }
}

pub fn matches_category(hit: &StructureHit, category: Option<&str>) -> bool {
    category.is_none_or(|category| {
        hit.core
            .as_ref()
            .and_then(|h| h.extra.get("category"))
            .is_some_and(|c| c == category)
    })
}

/// One crystallographic site, as listed in the panel.
#[derive(Clone, Debug, PartialEq)]
pub struct SiteSummary {
    pub label: String,
    pub symbol: String,
    pub z: u32,
    pub multiplicity: usize,
    /// Index of the first expanded site with this asymmetric index.
    pub site_index: usize,
}

/// A loaded structure, summarised for the panel and the cluster generator.
#[derive(Clone, Debug)]
pub struct StructureSummary {
    pub hit: StructureHit,
    pub lattice: [f64; 6],
    /// Symmetry-distinct sites (label, element, multiplicity).
    pub sites: Vec<SiteSummary>,
    pub structure: Arc<core::Structure>,
    /// Set when the "structure" is a non-periodic XYZ cluster; the cluster
    /// is then built directly from these atoms.
    pub xyz: Option<Arc<core::Xyz>>,
}

impl StructureSummary {
    pub fn from_structure(hit: StructureHit, structure: core::Structure) -> Self {
        let l = &structure.lattice;
        let lattice = [l.a, l.b, l.c, l.alpha, l.beta, l.gamma];
        // Group the expanded sites by asymmetric-unit index (falling back to
        // the label) so the panel lists each distinct site once.
        let mut sites: Vec<SiteSummary> = Vec::new();
        for (i, site) in structure.sites.iter().enumerate() {
            let Some(sp) = site.majority() else {
                continue;
            };
            let key = site
                .asym_index
                .map(|a| format!("#{a}"))
                .unwrap_or_else(|| site.label.clone());
            match sites.iter_mut().find(|s| s.label == key) {
                Some(s) => s.multiplicity += 1,
                None => sites.push(SiteSummary {
                    label: key,
                    symbol: sp.symbol.clone(),
                    z: sp.element().map(|e| e.z as u32).unwrap_or(0),
                    multiplicity: 1,
                    site_index: i,
                }),
            }
        }
        // Prefer the crystallographic label over the "#n" key.
        for s in &mut sites {
            if s.label.starts_with('#') {
                s.label = structure.sites[s.site_index].label.clone();
            }
        }
        Self {
            hit,
            lattice,
            sites,
            structure: Arc::new(structure),
            xyz: None,
        }
    }

    /// Summary of an XYZ file: one "site" per element (multiplicity = atom
    /// count), no lattice.
    pub fn from_xyz(hit: StructureHit, xyz: core::Xyz) -> Self {
        let mut sites: Vec<SiteSummary> = Vec::new();
        for (i, atom) in xyz.atoms.iter().enumerate() {
            match sites.iter_mut().find(|s| s.symbol == atom.symbol) {
                Some(s) => s.multiplicity += 1,
                None => sites.push(SiteSummary {
                    label: atom.symbol.clone(),
                    symbol: atom.symbol.clone(),
                    z: atom.z as u32,
                    multiplicity: 1,
                    site_index: i,
                }),
            }
        }
        let title = if xyz.comment.is_empty() {
            hit.name.clone()
        } else {
            xyz.comment.clone()
        };
        let mut structure = core::Structure::new(
            &title,
            // A unit cube is always a valid lattice.
            core::Lattice::from_parameters(1.0, 1.0, 1.0, 90.0, 90.0, 90.0).expect("unit lattice"),
            Vec::new(),
        );
        structure.source = "xyz".into();
        Self {
            hit,
            lattice: [0.0; 6],
            sites,
            structure: Arc::new(structure),
            xyz: Some(Arc::new(xyz)),
        }
    }

    /// Distinct elements, in site order.
    pub fn elements(&self) -> Vec<(String, u32)> {
        let mut out: Vec<(String, u32)> = Vec::new();
        for s in &self.sites {
            if !out.iter().any(|(e, _)| *e == s.symbol) {
                out.push((s.symbol.clone(), s.z));
            }
        }
        out
    }

    /// Distinct sites of one element.
    pub fn sites_of(&self, symbol: &str) -> Vec<&SiteSummary> {
        self.sites.iter().filter(|s| s.symbol == symbol).collect()
    }

    pub fn formula(&self) -> String {
        self.structure.formula()
    }

    pub fn space_group(&self) -> String {
        let sg = &self.structure.space_group;
        match (&sg.hm_symbol, sg.number) {
            (Some(hm), Some(n)) => format!("{hm} (#{n})"),
            (Some(hm), None) => hm.clone(),
            (None, Some(n)) => format!("#{n}"),
            (None, None) => self.hit.space_group.clone(),
        }
    }
}

/// Machine-level configuration the sources need.
#[derive(Clone, Default)]
pub struct SourceConfig {
    /// Scanned CIF folder, shared with the search job.
    pub cif_library: Option<Arc<core::LocalCifLibrary>>,
    pub amcsd_db: Option<PathBuf>,
    pub mp_api_key: String,
}

/// The Fit stage's structure panel talks to this trait only. Every method
/// may block (network, SQLite, CIF parsing) and is called on the background
/// executor.
pub trait StructureProvider: Send + Sync {
    /// Free-text search over formula / name / id.
    fn search(&self, query: &str) -> Result<Vec<StructureHit>, String>;
    fn fetch(&self, hit: &StructureHit) -> Result<StructureSummary, String>;
}

fn query_for(text: &str) -> core::StructureQuery {
    // "Fe S" / "Fe,S" → element filter; anything else → free text.
    let toks: Vec<&str> = text
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|t| !t.is_empty())
        .collect();
    let all_elements = !toks.is_empty()
        && toks
            .iter()
            .all(|t| t.len() <= 2 && atomic_number(t).is_some());
    let mut q = core::StructureQuery::default();
    if all_elements {
        q.elements = toks.iter().map(|t| capitalise(t)).collect();
    } else if !text.trim().is_empty() {
        q.text = Some(text.trim().to_string());
    }
    q.limit = 200;
    q
}

fn capitalise(sym: &str) -> String {
    let mut c = sym.chars();
    match c.next() {
        Some(f) => f.to_ascii_uppercase().to_string() + &c.as_str().to_ascii_lowercase(),
        None => String::new(),
    }
}

/// Adapter over a core `StructureSource`.
fn core_search(
    src: &dyn StructureSource,
    kind: StructureSourceKind,
    query: &str,
) -> Result<Vec<StructureHit>, String> {
    src.search(&query_for(query))
        .map(|hits| {
            hits.into_iter()
                .map(|h| StructureHit::from_core(h, kind))
                .collect()
        })
        .map_err(|e| e.to_string())
}

fn core_fetch(src: &dyn StructureSource, hit: &StructureHit) -> Result<StructureSummary, String> {
    let core_hit = hit
        .core
        .as_ref()
        .ok_or_else(|| "hit did not come from this source".to_string())?;
    let structure = src.fetch(core_hit).map_err(|e| e.to_string())?;
    Ok(StructureSummary::from_structure(hit.clone(), structure))
}

/// A scanned folder of CIF files.
pub struct LocalCifProvider(pub Arc<core::LocalCifLibrary>);

impl StructureProvider for LocalCifProvider {
    fn search(&self, query: &str) -> Result<Vec<StructureHit>, String> {
        core_search(&*self.0, StructureSourceKind::LocalCif, query)
    }
    fn fetch(&self, hit: &StructureHit) -> Result<StructureSummary, String> {
        core_fetch(&*self.0, hit)
    }
}

/// AMCSD SQLite database; opened per call (SQLite connections are not `Sync`).
pub struct AmcsdProvider(pub PathBuf);

impl AmcsdProvider {
    fn open(&self) -> Result<core::db::amcsd::Amcsd, String> {
        core::db::amcsd::Amcsd::open(&self.0).map_err(|e| e.to_string())
    }
}

impl StructureProvider for AmcsdProvider {
    fn search(&self, query: &str) -> Result<Vec<StructureHit>, String> {
        core_search(&self.open()?, StructureSourceKind::Amcsd, query)
    }
    fn fetch(&self, hit: &StructureHit) -> Result<StructureSummary, String> {
        core_fetch(&self.open()?, hit)
    }
}

/// Crystallography Open Database REST API (no key needed).
pub struct CodProvider(pub core::db::cod::Cod);

impl StructureProvider for CodProvider {
    fn search(&self, query: &str) -> Result<Vec<StructureHit>, String> {
        if query.trim().is_empty() {
            return Err(
                "COD: enter a mineral or compound name, a formula (Fe S2), elements, or a COD id"
                    .into(),
            );
        }
        core_search(&self.0, StructureSourceKind::Cod, query)
    }
    fn fetch(&self, hit: &StructureHit) -> Result<StructureSummary, String> {
        core_fetch(&self.0, hit)
    }
}

/// Materials Project v2 REST API.
pub struct MaterialsProjectProvider(pub core::db::mp::MaterialsProject);

impl StructureProvider for MaterialsProjectProvider {
    fn search(&self, query: &str) -> Result<Vec<StructureHit>, String> {
        if query.trim().is_empty() {
            return Err("Materials Project: enter a formula (e.g. RuO2) or an mp-id".into());
        }
        core_search(&self.0, StructureSourceKind::MaterialsProject, query)
    }
    fn fetch(&self, hit: &StructureHit) -> Result<StructureSummary, String> {
        core_fetch(&self.0, hit)
    }
}

/// Read one CIF file into a summary (an "Import CIF…" result).
pub fn import_cif(path: &Path) -> Result<StructureSummary, String> {
    let structure = core::read_cif(path).map_err(|e| e.to_string())?;
    let id = path.to_string_lossy().to_string();
    let core_hit = core::db::hit_from_structure(&structure, "cif", &id);
    let mut hit = StructureHit::from_core(core_hit, StructureSourceKind::LocalCif);
    if hit.name.is_empty() {
        hit.name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
    }
    Ok(StructureSummary::from_structure(hit, structure))
}

/// Import an XYZ file as a ready-made cluster.
pub fn import_xyz(path: &Path) -> Result<StructureSummary, String> {
    let xyz = core::read_xyz(path).map_err(|e| e.to_string())?;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let mut counts: std::collections::BTreeMap<String, usize> = Default::default();
    for a in &xyz.atoms {
        *counts.entry(a.symbol.clone()).or_default() += 1;
    }
    let formula = counts
        .iter()
        .map(|(s, n)| {
            if *n == 1 {
                s.clone()
            } else {
                format!("{s}{n}")
            }
        })
        .collect::<Vec<_>>()
        .join("");
    let hit = StructureHit {
        id: path.to_string_lossy().to_string(),
        formula,
        name,
        space_group: "XYZ cluster".into(),
        source: StructureSourceKind::LocalCif,
        core: None,
    };
    Ok(StructureSummary::from_xyz(hit, xyz))
}

/// The bundled library of common standards (COD, CC0) — see the core
/// `BuiltinLibrary`.
pub struct BuiltinProvider;

impl StructureProvider for BuiltinProvider {
    fn search(&self, query: &str) -> Result<Vec<StructureHit>, String> {
        let lib = core::BuiltinLibrary::get().map_err(|e| e.to_string())?;
        core_search(lib, StructureSourceKind::Builtin, query)
    }

    fn fetch(&self, hit: &StructureHit) -> Result<StructureSummary, String> {
        let lib = core::BuiltinLibrary::get().map_err(|e| e.to_string())?;
        core_fetch(lib, hit)
    }
}

/// The simple structures the old generator knew, converted into real
/// `Structure`s so `feffgen` recipes still run through the core cluster
/// builder like everything else.
/// Conventional-cell sites of a simple structure type: (frac, element slot).
fn builtin_basis(structure: &str) -> Option<Vec<([f64; 3], usize)>> {
    let fcc = vec![
        ([0.0, 0.0, 0.0], 0),
        ([0.5, 0.5, 0.0], 0),
        ([0.5, 0.0, 0.5], 0),
        ([0.0, 0.5, 0.5], 0),
    ];
    Some(match structure {
        "fcc" => fcc,
        "bcc" => vec![([0.0, 0.0, 0.0], 0), ([0.5, 0.5, 0.5], 0)],
        "cscl" => vec![([0.0, 0.0, 0.0], 0), ([0.5, 0.5, 0.5], 1)],
        "rocksalt" => {
            let mut b = fcc;
            b.extend([
                ([0.5, 0.0, 0.0], 1),
                ([0.0, 0.5, 0.0], 1),
                ([0.0, 0.0, 0.5], 1),
                ([0.5, 0.5, 0.5], 1),
            ]);
            b
        }
        "zincblende" | "diamond" => {
            let slot = if structure == "diamond" { 0 } else { 1 };
            let mut b = fcc;
            b.extend([
                ([0.25, 0.25, 0.25], slot),
                ([0.75, 0.75, 0.25], slot),
                ([0.75, 0.25, 0.75], slot),
                ([0.25, 0.75, 0.75], slot),
            ]);
            b
        }
        "hcp" => vec![([0.0, 0.0, 0.0], 0), ([1.0 / 3.0, 2.0 / 3.0, 0.5], 0)],
        _ => return None,
    })
}

/// A simple crystal (element(s), structure type, cell) as a core `Structure`.
pub fn builtin_structure(spec: &crate::feffgen::CrystalSpec) -> Result<core::Structure, String> {
    let structure = spec.structure.trim().to_ascii_lowercase();
    let basis = builtin_basis(&structure).ok_or_else(|| {
        format!(
            "unknown structure '{structure}' (fcc, bcc, hcp, diamond, rocksalt, zincblende, cscl)"
        )
    })?;
    let a = spec.a;
    let lattice = if structure == "hcp" {
        let c = spec.c.unwrap_or(a * (8.0f64 / 3.0).sqrt());
        core::Lattice::from_parameters(a, a, c, 90.0, 90.0, 120.0)
    } else {
        core::Lattice::cubic(a)
    }
    .map_err(|e| e.to_string())?;
    let el1 = spec.element.trim().to_string();
    let el2 = spec.element2.clone().unwrap_or_else(|| el1.clone());
    let mut sites = Vec::new();
    let mut counts = [0usize; 2];
    for (frac, slot) in basis {
        let sym = if slot == 0 { &el1 } else { &el2 };
        counts[slot] += 1;
        sites.push(core::Site::new(
            &format!("{sym}{}", counts[slot]),
            sym,
            frac,
        ));
    }
    let title = format!("{} ({structure})", spec.element);
    let mut s = core::Structure::new(&title, lattice, sites);
    s.source = "built-in".into();
    Ok(s)
}

/// Provider for a source kind, given the machine configuration.
pub fn provider_for(
    kind: StructureSourceKind,
    cfg: &SourceConfig,
) -> Result<Box<dyn StructureProvider>, String> {
    match kind {
        StructureSourceKind::Builtin => Ok(Box::new(BuiltinProvider)),
        StructureSourceKind::LocalCif => cfg
            .cif_library
            .clone()
            .map(|lib| Box::new(LocalCifProvider(lib)) as Box<dyn StructureProvider>)
            .ok_or_else(|| "CIF library: choose a folder of .cif files first".to_string()),
        StructureSourceKind::MaterialsProject => {
            let key = cfg.mp_api_key.trim();
            if key.is_empty() {
                return Err(
                    "Materials Project: set your API key (materialsproject.org → API)".into(),
                );
            }
            Ok(Box::new(MaterialsProjectProvider(
                core::db::mp::MaterialsProject::new(core::db::mp::MaterialsProjectConfig::new(key)),
            )))
        }
        StructureSourceKind::Amcsd => {
            let db = cfg
                .amcsd_db
                .clone()
                .filter(|p| p.is_file())
                .ok_or_else(|| {
                    "AMCSD: download the database or choose an existing .db file".to_string()
                })?;
            Ok(Box::new(AmcsdProvider(db)))
        }
        StructureSourceKind::Cod => Ok(Box::new(CodProvider(core::db::cod::Cod::default()))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const INP: &str = "TITLE Ru hcp (generated)\nHOLE 1 1.0\n\nPOTENTIALS\n 0 44 Ru\n 1 44 Ru\n\nATOMS\n 0.0 0.0 0.0 0 Ru0 0.0\n 1.353 -0.78115 -2.141 1 Ru1 2.65041\n -1.353 -2.34346 0.0 1 Ru1 2.706\n 2.706 1.56231 -2.141 1 Ru1 3.78776\nEND\n";

    #[test]
    fn parses_feff_inp_cluster_and_shells() {
        let c = parse_feff_inp(INP);
        assert_eq!(c.title, "Ru hcp (generated)");
        assert_eq!(c.potentials.len(), 2);
        assert_eq!(c.atoms.len(), 4);
        assert_eq!(c.absorber().unwrap().z, 44);
        assert_eq!(c.shells.len(), 4);
        assert!((c.shells[1].0 - 2.65041).abs() < 1e-4);
        assert_eq!(c.element_counts(), vec![("Ru".to_string(), 44, 4)]);
        assert_eq!(c.nearest([2.706, 1.56231, -2.141], 0.05), Some(3));
    }

    const DAT: &str = " PATH  Ru K edge\n     2  12.000   2.6504    2.4900   -6.0 nleg, deg, reff, rnrmav(bohr), edge\n        x         y         z   pot at#\n      .0000     .0000     .0000  0  44 Ru       absorbing atom\n     1.3530    -.7812   -2.1410  1  44 Ru\n    k   real[2*phc]   mag[feff]  phase[feff] red factor   lambda     real[p]@#\n   .000  3.5E+00  0.0E+00 -5.1E+00  9.8E-01  1.5E+01  1.8E+00\n  4.000  3.5E+00  2.0E-01 -5.6E+00  9.8E-01  1.5E+01  1.8E+00\n  8.000  3.5E+00  1.0E-01 -6.1E+00  9.8E-01  1.5E+01  1.8E+00\n";

    #[test]
    fn parses_path_geometry_and_importance() {
        let p = parse_path_dat(DAT, Path::new("feff0001.dat")).unwrap();
        assert_eq!(p.nleg, 2);
        assert_eq!(p.degen, 12.0);
        assert!((p.reff - 2.6504).abs() < 1e-6);
        assert_eq!(p.legs.len(), 2);
        assert_eq!(p.label(), "Ru–Ru");
        assert!(p.is_single_scattering());
        // degen * mean(0.2, 0.1) / reff²
        assert!((p.importance - 12.0 * 0.15 / (2.6504 * 2.6504)).abs() < 1e-9);
        assert_eq!(p.polyline().len(), 3);
        let c = parse_feff_inp(INP);
        assert_eq!(p.atom_indices(&c), vec![Some(1)]);
    }

    #[test]
    fn builtin_provider_searches_and_fetches() {
        let p = BuiltinProvider;
        let hits = p.search("ruthenium").unwrap();
        let hit = hits
            .iter()
            .find(|h| h.id == "ru_hcp")
            .cloned()
            .unwrap_or_else(|| panic!("{hits:?}"));
        let s = p.fetch(&hit).unwrap();
        assert_eq!(s.hit.formula, "Ru");
        assert!((s.lattice[5] - 120.0).abs() < 1e-9);
        // hcp: two Ru sites in the conventional cell.
        assert_eq!(s.sites.iter().map(|x| x.multiplicity).sum::<usize>(), 2);
        assert_eq!(s.elements(), vec![("Ru".to_string(), 44)]);
        let all = p.search("").unwrap();
        assert!(all.len() >= 40);
        assert!(all.iter().all(|h| matches_category(h, None)));
        assert!(matches_category(&hit, Some("metal")));
        assert!(!matches_category(&hit, Some("oxide")));
        for category in ["metal", "oxide", "sulfide", "other"] {
            assert!(
                all.iter().any(|h| matches_category(h, Some(category))),
                "{category}"
            );
        }
        assert_eq!(p.search("RuO2").unwrap()[0].id, "ruo2");
        let cfg = SourceConfig::default();
        assert!(provider_for(StructureSourceKind::Amcsd, &cfg).is_err());
        assert!(provider_for(StructureSourceKind::MaterialsProject, &cfg).is_err());
        assert!(provider_for(StructureSourceKind::LocalCif, &cfg).is_err());
    }

    #[test]
    fn builtin_structures_run_through_the_core_cluster_builder() {
        let p = BuiltinProvider;
        let hit = p
            .search("ru_hcp")
            .unwrap()
            .into_iter()
            .find(|h| h.id == "ru_hcp")
            .unwrap();
        let s = p.fetch(&hit).unwrap();
        let cluster = core::build_cluster(
            &s.structure,
            &core::AbsorberSelection::Element("Ru".into()),
            &core::ClusterOptions {
                radius: 3.0,
                ..Default::default()
            },
        )
        .unwrap();
        let view = Cluster::from_core(&cluster);
        // 12 nearest neighbours (6 × 2.65 + 6 × 2.71) plus the absorber.
        assert_eq!(view.atoms.len(), 13);
        assert!((view.shells[1].0 - 2.65).abs() < 0.03, "{:?}", view.shells);
        assert_eq!(view.shells[1].1 + view.shells[2].1, 12);
        // Binary structures keep both elements.
        let nacl = p.search("nacl").unwrap().remove(0);
        let s = p.fetch(&nacl).unwrap();
        assert_eq!(s.elements().len(), 2);
        // Formula summaries use alphabetical Hill order when carbon is absent.
        assert_eq!(s.formula(), "ClNa");
    }

    #[test]
    fn query_parsing_splits_elements_from_text() {
        let q = query_for("Fe S");
        assert_eq!(q.elements, vec!["Fe".to_string(), "S".to_string()]);
        assert!(q.text.is_none());
        let q = query_for("pyrite");
        assert_eq!(q.text.as_deref(), Some("pyrite"));
        assert!(q.elements.is_empty());
    }

    #[test]
    fn cif_import_and_local_library_provider() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../rexafs/tests/testfiles/cif");
        let lib = Arc::new(core::LocalCifLibrary::scan(&dir).unwrap());
        let cfg = SourceConfig {
            cif_library: Some(lib),
            ..Default::default()
        };
        let p = provider_for(StructureSourceKind::LocalCif, &cfg).unwrap();
        let hits = p.search("Ru").unwrap();
        assert!(!hits.is_empty(), "no Ru hits in {}", dir.display());
        let s = p.fetch(&hits[0]).unwrap();
        assert!(s.elements().iter().any(|(e, _)| e == "Ru"));
        let cif = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .find(|p| p.extension().is_some_and(|x| x == "cif"))
            .unwrap();
        let imported = import_cif(&cif).unwrap();
        assert!(!imported.sites.is_empty());
        assert_eq!(imported.hit.source, StructureSourceKind::LocalCif);
    }

    #[test]
    fn element_table_is_consistent() {
        assert_eq!(element_symbol(44), "Ru");
        assert_eq!(atomic_number("ru"), Some(44));
        assert_eq!(cpk_color(8), 0xff0d0d);
        assert!((covalent_radius(29) - 1.32).abs() < 1e-6);
    }
}
