//! Complete a finite molecule across periodic CIF boundaries. Display only:
//! this never changes the absorber-centered spherical FEFF calculation.
use rexafs::xafs::structure::Structure;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Clone, Debug)]
pub(crate) struct MolecularAtom {
    pub pos: [f64; 3],
    pub z: u32,
    pub label: String,
}
#[derive(Clone, Debug, Default)]
pub(crate) struct MolecularComponent {
    pub atoms: Vec<MolecularAtom>,
    pub bonds: Vec<[usize; 2]>,
}

/// Covalent-radius connectivity is a visualization heuristic, not bond-order
/// assignment. A cycle with nonzero lattice translation identifies an extended
/// solid, preventing display of a cut fragment as a complete molecule.
pub(crate) fn complete_molecule(s: &Structure, seed: usize) -> Result<MolecularComponent, String> {
    if s.sites.len() > 512 {
        return Err("Molecule view supports up to 512 sites per unit cell.".into());
    }
    let origin = s.lattice.to_cart(
        s.sites
            .get(seed)
            .ok_or("No molecular center selected")?
            .frac,
    );
    let elements = s
        .sites
        .iter()
        .map(|site| {
            site.element()
                .ok_or_else(|| format!("Unknown element at {}", site.label))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut graph = vec![Vec::new(); s.sites.len()];
    for i in 0..s.sites.len() {
        for j in i..s.sites.len() {
            let cutoff = 1.2 * (elements[i].covalent_radius + elements[j].covalent_radius);
            let delta: [f64; 3] = std::array::from_fn(|a| s.sites[j].frac[a] - s.sites[i].frac[a]);
            let axes: [[f64; 3]; 3] = std::array::from_fn(|axis| {
                let mut cart = [0.; 3];
                cart[axis] = cutoff;
                s.lattice.to_frac(cart)
            });
            let reach: [f64; 3] =
                std::array::from_fn(|a| axes.iter().map(|v| v[a] * v[a]).sum::<f64>().sqrt());
            let lo: [i32; 3] = std::array::from_fn(|a| (-delta[a] - reach[a]).ceil() as i32);
            let hi: [i32; 3] = std::array::from_fn(|a| (-delta[a] + reach[a]).floor() as i32);
            if (0..3).any(|a| hi[a] - lo[a] > 8) {
                return Err("Unit cell is too small or oblique for molecule connectivity.".into());
            }
            for a in lo[0]..=hi[0] {
                for b in lo[1]..=hi[1] {
                    for c in lo[2]..=hi[2] {
                        let image = [a, b, c];
                        let d = s
                            .lattice
                            .to_cart(std::array::from_fn(|k| delta[k] + image[k] as f64));
                        let distance = d.iter().map(|v| v * v).sum::<f64>().sqrt();
                        if distance > 0.4 && distance <= cutoff {
                            graph[i].push((j, image));
                            graph[j].push((i, image.map(|v| -v)));
                        }
                    }
                }
            }
        }
    }
    let mut images = BTreeMap::from([(seed, [0i32; 3])]);
    let mut todo = VecDeque::from([seed]);
    while let Some(i) = todo.pop_front() {
        for &(j, shift) in &graph[i] {
            let image = std::array::from_fn(|a| images[&i][a] + shift[a]);
            if let Some(old) = images.get(&j) {
                if *old != image {
                    return Err("This structure has an extended bonded network; use Crystal + cluster or Cluster only.".into());
                }
            } else {
                images.insert(j, image);
                todo.push_back(j);
            }
        }
    }
    let mut out = MolecularComponent::default();
    let mut indices = BTreeMap::new();
    for (&i, image) in &images {
        let pos = s.lattice.to_cart(std::array::from_fn(|a| {
            s.sites[i].frac[a] + image[a] as f64
        }));
        indices.insert(i, out.atoms.len());
        out.atoms.push(MolecularAtom {
            pos: std::array::from_fn(|a| pos[a] - origin[a]),
            z: elements[i].z as u32,
            label: s.sites[i].label.clone(),
        });
    }
    let mut bonds = BTreeSet::new();
    for (&i, &a) in &indices {
        for &(j, _) in &graph[i] {
            if let Some(&b) = indices.get(&j) {
                bonds.insert([a.min(b), a.max(b)]);
            }
        }
    }
    out.bonds = bonds.into_iter().collect();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rexafs::xafs::structure::read_cif;
    fn crystal(name: &str) -> Structure {
        read_cif(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../rexafs/data/builtin_cifs")
                .join(format!("{name}.cif")),
        )
        .unwrap()
    }
    #[test]
    fn molecular_cifs_keep_hydrogens_and_cross_cell_bonds() {
        for (name, atoms, hydrogens, bonds) in [("urea", 8, 4, 7), ("aspirin", 21, 8, 21)] {
            let s = crystal(name);
            for seed in 0..s.sites.len() {
                let molecule = complete_molecule(&s, seed).unwrap();
                assert_eq!(molecule.atoms.len(), atoms, "{name} site {seed}");
                assert_eq!(
                    molecule.atoms.iter().filter(|a| a.z == 1).count(),
                    hydrogens
                );
                assert_eq!(molecule.bonds.len(), bonds, "{name}");
                assert!(molecule.bonds.iter().all(|[a, b]| {
                    molecule.atoms[*a]
                        .pos
                        .iter()
                        .zip(molecule.atoms[*b].pos)
                        .map(|(a, b)| (a - b).powi(2))
                        .sum::<f64>()
                        .sqrt()
                        < 1.7
                }));
            }
        }
    }
    #[test]
    fn extended_solids_are_not_presented_as_finite_molecules() {
        for name in ["cu", "tio2_rutile"] {
            assert!(
                complete_molecule(&crystal(name), 0)
                    .unwrap_err()
                    .contains("extended bonded network")
            );
        }
    }
}
