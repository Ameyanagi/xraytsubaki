//! Spherical cluster of atoms around an absorber, FEFF potential
//! assignment and neighbour-shell summary.

use serde::{Deserialize, Serialize};

use super::element::Element;
use super::model::Structure;
use super::StructureError;

/// Which site is the absorber.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AbsorberSelection {
    /// A site index of `Structure::sites`.
    SiteIndex(usize),
    /// The first site whose majority species is this element.
    Element(String),
    /// The `nth` (0-based) crystallographically distinct site of the element,
    /// counted over the asymmetric unit.
    ElementSite { symbol: String, nth: usize },
}

/// How mixed-occupancy sites are resolved into single atoms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum OccupancyPolicy {
    /// Majority species on every site (deterministic; Larch/pymatgen default
    /// behaviour when a site is not randomised).
    #[default]
    Majority,
    /// Species drawn per atom proportional to occupancy, seeded.
    Random { seed: u64 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClusterOptions {
    /// Sphere radius in Å.
    pub radius: f64,
    pub include_hydrogen: bool,
    pub occupancy: OccupancyPolicy,
}

impl Default for ClusterOptions {
    fn default() -> Self {
        Self {
            radius: 8.0,
            include_hydrogen: false,
            occupancy: OccupancyPolicy::Majority,
        }
    }
}

/// One atom of the cluster, absorber at the origin.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClusterAtom {
    /// Cartesian position relative to the absorber (Å).
    pub cart: [f64; 3],
    pub distance: f64,
    pub symbol: String,
    pub z: u8,
    /// Index into `Structure::sites`.
    pub site_index: usize,
    /// Lattice translation of the periodic image.
    pub image: [i32; 3],
    /// FEFF potential index (0 = absorber).
    pub ipot: u16,
    /// `Ru_1`, `(Fe0.7Ni0.3)_3` — site species + site number, as Larch tags.
    pub label: String,
}

impl ClusterAtom {
    pub fn element(&self) -> &'static Element {
        Element::from_z(self.z).expect("cluster atoms carry a valid Z")
    }
}

/// A FEFF potential.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Potential {
    pub ipot: u16,
    pub symbol: String,
    pub z: u8,
    /// Number of cluster atoms using it.
    pub count: usize,
}

/// A neighbour shell: atoms of one element at one distance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Shell {
    pub distance: f64,
    pub symbol: String,
    pub count: usize,
    /// Indices into `Cluster::atoms`.
    pub atoms: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cluster {
    pub absorber_site: usize,
    /// Atoms sorted by distance; `atoms[0]` is the absorber.
    pub atoms: Vec<ClusterAtom>,
    pub potentials: Vec<Potential>,
    pub radius: f64,
    pub warnings: Vec<String>,
    /// Formula/title of the parent structure, for feff.inp titles.
    pub structure_title: String,
    pub formula: String,
    pub space_group: Option<String>,
}

impl Cluster {
    pub fn absorber(&self) -> &ClusterAtom {
        &self.atoms[0]
    }

    /// Group neighbours (excluding the absorber) by element and distance
    /// within `tol` Å.
    pub fn shells(&self, tol: f64) -> Vec<Shell> {
        let mut shells: Vec<Shell> = Vec::new();
        for (i, atom) in self.atoms.iter().enumerate().skip(1) {
            if let Some(shell) = shells
                .iter_mut()
                .find(|s| s.symbol == atom.symbol && (s.distance - atom.distance).abs() <= tol)
            {
                // Running mean keeps the shell centred.
                shell.distance = (shell.distance * shell.count as f64 + atom.distance)
                    / (shell.count as f64 + 1.0);
                shell.count += 1;
                shell.atoms.push(i);
            } else {
                shells.push(Shell {
                    distance: atom.distance,
                    symbol: atom.symbol.clone(),
                    count: 1,
                    atoms: vec![i],
                });
            }
        }
        shells.sort_by(|a, b| a.distance.total_cmp(&b.distance));
        shells
    }

    /// Nearest cluster atom to a Cartesian point, with its distance.
    pub fn nearest(&self, cart: [f64; 3]) -> Option<(usize, f64)> {
        self.atoms
            .iter()
            .enumerate()
            .map(|(i, a)| {
                let d = ((a.cart[0] - cart[0]).powi(2)
                    + (a.cart[1] - cart[1]).powi(2)
                    + (a.cart[2] - cart[2]).powi(2))
                .sqrt();
                (i, d)
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
    }
}

/// Indices of every site of the structure that can host `symbol` as its
/// majority species, grouped by crystallographic equivalence (one
/// representative per asymmetric site, in order).
pub fn absorber_sites(structure: &Structure, symbol: &str) -> Vec<usize> {
    let mut seen_asym = Vec::new();
    let mut out = Vec::new();
    for (i, site) in structure.sites.iter().enumerate() {
        let is_host = site
            .majority()
            .is_some_and(|sp| sp.symbol.eq_ignore_ascii_case(symbol));
        if !is_host {
            continue;
        }
        match site.asym_index {
            Some(a) if seen_asym.contains(&a) => continue,
            Some(a) => seen_asym.push(a),
            None => {}
        }
        out.push(i);
    }
    out
}

/// Build the cluster.
pub fn build_cluster(
    structure: &Structure,
    absorber: &AbsorberSelection,
    opts: &ClusterOptions,
) -> Result<Cluster, StructureError> {
    if !(opts.radius > 0.5 && opts.radius < 50.0) {
        return Err(StructureError::InvalidCluster {
            reason: format!("radius {} Å out of range", opts.radius),
        });
    }
    if structure.sites.is_empty() {
        return Err(StructureError::InvalidCluster {
            reason: "structure has no sites".into(),
        });
    }
    let absorber_site = match absorber {
        AbsorberSelection::SiteIndex(i) => {
            if *i >= structure.sites.len() {
                return Err(StructureError::AbsorberNotFound {
                    reason: format!("site index {i} ≥ {}", structure.sites.len()),
                });
            }
            *i
        }
        AbsorberSelection::Element(symbol) => *absorber_sites(structure, symbol)
            .first()
            .ok_or_else(|| StructureError::AbsorberNotFound {
                reason: format!("no site with majority species {symbol}"),
            })?,
        AbsorberSelection::ElementSite { symbol, nth } => {
            let sites = absorber_sites(structure, symbol);
            *sites
                .get(*nth)
                .ok_or_else(|| StructureError::AbsorberNotFound {
                    reason: format!(
                        "{symbol} has {} distinct sites, asked for #{nth}",
                        sites.len()
                    ),
                })?
        }
    };
    let mut warnings = Vec::new();
    let absorber_element = structure.sites[absorber_site].element().ok_or_else(|| {
        StructureError::AbsorberNotFound {
            reason: "absorber site has no species".into(),
        }
    })?;

    // Species per site under the occupancy policy.
    let mut rng_state = match opts.occupancy {
        OccupancyPolicy::Random { seed } => seed.wrapping_mul(6364136223846793005).wrapping_add(1),
        OccupancyPolicy::Majority => 0,
    };
    let mut next_random = || {
        rng_state ^= rng_state << 13;
        rng_state ^= rng_state >> 7;
        rng_state ^= rng_state << 17;
        (rng_state >> 11) as f64 / (1u64 << 53) as f64
    };
    for site in &structure.sites {
        if site.species.len() > 1 && matches!(opts.occupancy, OccupancyPolicy::Majority) {
            let maj = site
                .majority()
                .map(|s| s.symbol.clone())
                .unwrap_or_default();
            let dropped: Vec<String> = site
                .species
                .iter()
                .filter(|s| s.symbol != maj)
                .map(|s| format!("{}({:.2})", s.symbol, s.occupancy))
                .collect();
            let msg = format!(
                "site {} uses majority species {maj}; dropped {}",
                site.label,
                dropped.join(", ")
            );
            if !warnings.contains(&msg) {
                warnings.push(msg);
            }
        }
    }

    let lattice = &structure.lattice;
    let center = structure.cart(absorber_site);
    let r2 = opts.radius * opts.radius;
    let ranges: Vec<i32> = (0..3)
        .map(|i| (opts.radius / lattice.interplanar_spacing(i)).ceil() as i32 + 1)
        .collect();

    let mut atoms: Vec<ClusterAtom> = Vec::new();
    for (si, site) in structure.sites.iter().enumerate() {
        let species = match opts.occupancy {
            OccupancyPolicy::Majority => site.majority().cloned(),
            OccupancyPolicy::Random { .. } => None,
        };
        let base = lattice.to_cart(site.frac);
        for na in -ranges[0]..=ranges[0] {
            for nb in -ranges[1]..=ranges[1] {
                for nc in -ranges[2]..=ranges[2] {
                    let shift = lattice.to_cart([na as f64, nb as f64, nc as f64]);
                    let cart = [
                        base[0] + shift[0] - center[0],
                        base[1] + shift[1] - center[1],
                        base[2] + shift[2] - center[2],
                    ];
                    let d2 = cart[0] * cart[0] + cart[1] * cart[1] + cart[2] * cart[2];
                    if d2 > r2 {
                        continue;
                    }
                    let is_absorber = si == absorber_site && na == 0 && nb == 0 && nc == 0;
                    let sp = match (&species, opts.occupancy) {
                        (Some(sp), _) => sp.clone(),
                        (None, OccupancyPolicy::Random { .. }) => {
                            let u = next_random() * site.total_occupancy().max(1e-9);
                            let mut acc = 0.0;
                            let mut chosen = None;
                            for s in &site.species {
                                acc += s.occupancy;
                                if u <= acc {
                                    chosen = Some(s.clone());
                                    break;
                                }
                            }
                            match chosen.or_else(|| site.species.last().cloned()) {
                                Some(s) => s,
                                None => continue,
                            }
                        }
                        (None, OccupancyPolicy::Majority) => continue,
                    };
                    let element = if is_absorber {
                        absorber_element
                    } else {
                        match sp.element() {
                            Some(e) => e,
                            None => {
                                let msg = format!(
                                    "site {} has unknown species symbol {:?}; skipped",
                                    site.label, sp.symbol
                                );
                                if !warnings.contains(&msg) {
                                    warnings.push(msg);
                                }
                                continue;
                            }
                        }
                    };
                    if !opts.include_hydrogen && element.z == 1 && !is_absorber {
                        continue;
                    }
                    atoms.push(ClusterAtom {
                        cart,
                        distance: d2.sqrt(),
                        symbol: element.symbol.to_string(),
                        z: element.z,
                        site_index: si,
                        image: [na, nb, nc],
                        ipot: u16::MAX,
                        label: format!("{}_{}", site.species_string(), si + 1),
                    });
                }
            }
        }
    }
    atoms.sort_by(|a, b| {
        a.distance
            .total_cmp(&b.distance)
            .then_with(|| a.site_index.cmp(&b.site_index))
            .then_with(|| a.image.cmp(&b.image))
    });
    // The absorber (distance 0) must be first.
    if let Some(pos) = atoms
        .iter()
        .position(|a| a.site_index == absorber_site && a.image == [0, 0, 0])
    {
        let abs = atoms.remove(pos);
        atoms.insert(0, abs);
    } else {
        return Err(StructureError::AbsorberNotFound {
            reason: "absorber not inside its own cluster".into(),
        });
    }
    // Potentials: 0 = absorber, then one per species in order of first
    // appearance by distance (Larch convention: every listed ipot is used).
    let mut potentials: Vec<Potential> = vec![Potential {
        ipot: 0,
        symbol: absorber_element.symbol.to_string(),
        z: absorber_element.z,
        count: 1,
    }];
    atoms[0].ipot = 0;
    for atom in atoms.iter_mut().skip(1) {
        let ipot = match potentials.iter_mut().skip(1).find(|p| p.z == atom.z) {
            Some(p) => {
                p.count += 1;
                p.ipot
            }
            None => {
                let ipot = potentials.len() as u16;
                potentials.push(Potential {
                    ipot,
                    symbol: atom.symbol.clone(),
                    z: atom.z,
                    count: 1,
                });
                ipot
            }
        };
        atom.ipot = ipot;
    }
    Ok(Cluster {
        absorber_site,
        atoms,
        potentials,
        radius: opts.radius,
        warnings,
        structure_title: structure.title.clone(),
        formula: structure.formula(),
        space_group: structure
            .space_group
            .hm_symbol
            .clone()
            .or_else(|| structure.space_group.number.map(|n| format!("#{n}"))),
    })
}
