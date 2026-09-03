//! GUI-side structure model for the Fit stage's cluster / path visualizer.
//!
//! Everything here is derived from files the FEFF runner already produces:
//! the `ATOMS` block of `feff.inp` (the cluster) and the leg geometry table
//! at the top of every `feffNNNN.dat` (the scattering paths). The core crate's
//! `xafs::structure` module (CIF reader, symmetry expansion, cluster builder,
//! databases) will replace [`StructureProvider`]'s stub implementation — see
//! the seam note on that trait.

use std::path::{Path, PathBuf};

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

    /// Cluster atom index for every scatterer leg (skipping the absorber).
    pub fn atom_indices(&self, cluster: &Cluster) -> Vec<Option<usize>> {
        self.legs
            .iter()
            .skip(1)
            .map(|leg| cluster.nearest(leg.pos, 0.05))
            .collect()
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

/// Load the geometry of a path file.
pub fn load_path_geometry(file: &Path) -> Option<PathGeometry> {
    let text = std::fs::read_to_string(file).ok()?;
    parse_path_dat(&text, file)
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
// Structure sources (seam for `xafs::structure::StructureSource`)
// ---------------------------------------------------------------------------

/// Where a structure hit came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StructureSourceKind {
    Builtin,
    LocalCif,
    MaterialsProject,
    Amcsd,
}

impl StructureSourceKind {
    pub const ALL: [StructureSourceKind; 4] = [
        StructureSourceKind::Builtin,
        StructureSourceKind::LocalCif,
        StructureSourceKind::MaterialsProject,
        StructureSourceKind::Amcsd,
    ];

    pub fn label(self) -> &'static str {
        match self {
            StructureSourceKind::Builtin => "Built-in",
            StructureSourceKind::LocalCif => "CIF library",
            StructureSourceKind::MaterialsProject => "Materials Project",
            StructureSourceKind::Amcsd => "AMCSD",
        }
    }

    pub fn badge(self) -> &'static str {
        match self {
            StructureSourceKind::Builtin => "built-in",
            StructureSourceKind::LocalCif => "cif",
            StructureSourceKind::MaterialsProject => "mp",
            StructureSourceKind::Amcsd => "amcsd",
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
}

/// A loaded structure, summarised for the panel and the cluster generator.
#[derive(Clone, Debug, PartialEq)]
pub struct StructureSummary {
    pub hit: StructureHit,
    pub lattice: [f64; 6],
    /// (symbol, Z, multiplicity) per crystallographic site.
    pub sites: Vec<(String, u32, usize)>,
    /// The generator recipe, when the structure is one of the built-ins.
    pub builtin: Option<crate::feffgen::CrystalSpec>,
}

impl StructureSummary {
    /// Distinct elements, in site order.
    pub fn elements(&self) -> Vec<(String, u32)> {
        let mut out: Vec<(String, u32)> = Vec::new();
        for (s, z, _) in &self.sites {
            if !out.iter().any(|(e, _)| e == s) {
                out.push((s.clone(), *z));
            }
        }
        out
    }
}

/// Seam: the Fit stage's structure panel talks to this trait only. Today the
/// only implementation is [`BuiltinStructures`]; swapping in the core crate's
/// `xafs::structure::StructureSource` (CIF library, Materials Project, AMCSD)
/// means adding one adapter type here and returning it from
/// [`provider_for`].
pub trait StructureProvider: Send + Sync {
    /// Substring search over formula / name / id.
    fn search(&self, query: &str) -> Result<Vec<StructureHit>, String>;
    fn fetch(&self, hit: &StructureHit) -> Result<StructureSummary, String>;
}

/// The simple structures `feffgen` can generate, exposed as a database.
pub struct BuiltinStructures;

/// id, element(s), structure, space group, a, c, mineral/name
type BuiltinRow = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    f64,
    Option<f64>,
    &'static str,
);

const BUILTINS: &[BuiltinRow] = &[
    // id, element(s), structure, space group, a, c, mineral/name
    (
        "ru-hcp",
        "Ru",
        "hcp",
        "P6₃/mmc",
        2.706,
        Some(4.282),
        "ruthenium",
    ),
    ("cu-fcc", "Cu", "fcc", "Fm-3m", 3.615, None, "copper"),
    ("fe-bcc", "Fe", "bcc", "Im-3m", 2.866, None, "α-iron"),
    ("ni-fcc", "Ni", "fcc", "Fm-3m", 3.524, None, "nickel"),
    ("pt-fcc", "Pt", "fcc", "Fm-3m", 3.924, None, "platinum"),
    ("au-fcc", "Au", "fcc", "Fm-3m", 4.078, None, "gold"),
    ("pd-fcc", "Pd", "fcc", "Fm-3m", 3.891, None, "palladium"),
    (
        "co-hcp",
        "Co",
        "hcp",
        "P6₃/mmc",
        2.507,
        Some(4.069),
        "cobalt",
    ),
    ("zn-hcp", "Zn", "hcp", "P6₃/mmc", 2.665, Some(4.947), "zinc"),
    (
        "si-diamond",
        "Si",
        "diamond",
        "Fd-3m",
        5.431,
        None,
        "silicon",
    ),
    ("nio", "Ni,O", "rocksalt", "Fm-3m", 4.177, None, "bunsenite"),
    ("mgo", "Mg,O", "rocksalt", "Fm-3m", 4.212, None, "periclase"),
    ("nacl", "Na,Cl", "rocksalt", "Fm-3m", 5.640, None, "halite"),
    (
        "zns",
        "Zn,S",
        "zincblende",
        "F-43m",
        5.409,
        None,
        "sphalerite",
    ),
    (
        "cscl",
        "Cs,Cl",
        "cscl",
        "Pm-3m",
        4.123,
        None,
        "caesium chloride",
    ),
];

impl BuiltinStructures {
    fn hit(row: &BuiltinRow) -> StructureHit {
        let formula = row.1.replace(',', "");
        StructureHit {
            id: row.0.to_string(),
            formula,
            name: format!("{} ({})", row.6, row.2),
            space_group: row.3.to_string(),
            source: StructureSourceKind::Builtin,
        }
    }
}

impl StructureProvider for BuiltinStructures {
    fn search(&self, query: &str) -> Result<Vec<StructureHit>, String> {
        let q = query.trim().to_ascii_lowercase();
        Ok(BUILTINS
            .iter()
            .map(Self::hit)
            .filter(|h| {
                q.is_empty()
                    || h.formula.to_ascii_lowercase().contains(&q)
                    || h.name.to_ascii_lowercase().contains(&q)
                    || h.id.contains(&q)
            })
            .collect())
    }

    fn fetch(&self, hit: &StructureHit) -> Result<StructureSummary, String> {
        let row = BUILTINS
            .iter()
            .find(|r| r.0 == hit.id)
            .ok_or_else(|| format!("unknown built-in structure '{}'", hit.id))?;
        let elements: Vec<&str> = row.1.split(',').collect();
        let (el1, el2) = (elements[0], elements.get(1).copied());
        let a = row.4;
        let c = row.5.unwrap_or(a);
        let hex = row.2 == "hcp";
        let lattice = if hex {
            [a, a, c, 90.0, 90.0, 120.0]
        } else {
            [a, a, a, 90.0, 90.0, 90.0]
        };
        let mult = |structure: &str| match structure {
            "fcc" | "rocksalt" | "zincblende" => 4,
            "bcc" => 2,
            "hcp" => 2,
            "diamond" => 8,
            "cscl" => 1,
            _ => 1,
        };
        let mut sites = vec![(
            el1.to_string(),
            atomic_number(el1).unwrap_or(0),
            mult(row.2),
        )];
        if let Some(e2) = el2 {
            sites.push((e2.to_string(), atomic_number(e2).unwrap_or(0), mult(row.2)));
        }
        Ok(StructureSummary {
            hit: hit.clone(),
            lattice,
            sites,
            builtin: Some(crate::feffgen::CrystalSpec {
                element: el1.to_string(),
                element2: el2.map(|s| s.to_string()),
                structure: row.2.to_string(),
                a,
                c: row.5,
                edge: "K".to_string(),
                rmax: 6.0,
            }),
        })
    }
}

/// Provider for a source kind. Only built-ins are wired today; the others
/// return an explanatory error until the core `xafs::structure` module
/// lands (see the [`StructureProvider`] note).
pub fn provider_for(kind: StructureSourceKind) -> Result<Box<dyn StructureProvider>, String> {
    match kind {
        StructureSourceKind::Builtin => Ok(Box::new(BuiltinStructures)),
        StructureSourceKind::LocalCif => {
            Err("CIF library: waiting for the core structure module (xafs::structure)".into())
        }
        StructureSourceKind::MaterialsProject => {
            Err("Materials Project: waiting for the core structure module (xafs::structure)".into())
        }
        StructureSourceKind::Amcsd => {
            Err("AMCSD: waiting for the core structure module (xafs::structure)".into())
        }
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
        let p = BuiltinStructures;
        let hits = p.search("ru").unwrap();
        assert_eq!(hits.len(), 1);
        let s = p.fetch(&hits[0]).unwrap();
        assert_eq!(s.hit.formula, "Ru");
        assert_eq!(s.lattice[5], 120.0);
        assert_eq!(s.sites[0].2, 2);
        assert!(s.builtin.is_some());
        assert!(p.search("").unwrap().len() >= 10);
        assert!(provider_for(StructureSourceKind::Amcsd).is_err());
    }

    #[test]
    fn element_table_is_consistent() {
        assert_eq!(element_symbol(44), "Ru");
        assert_eq!(atomic_number("ru"), Some(44));
        assert_eq!(cpk_color(8), 0xff0d0d);
        assert!((covalent_radius(29) - 1.32).abs() < 1e-6);
    }
}
