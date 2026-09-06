//! Display connectivity, independent of FEFF paths and polyhedron construction.
use super::molecule_view::SceneAtom;
use crate::structure::covalent_radius;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum BondMode {
    #[default]
    Auto,
    Absorber,
    AllContacts,
    None,
}
impl BondMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::Absorber => "Absorber bonds",
            Self::AllContacts => "All contacts",
            Self::None => "None",
        }
    }
}
fn distance(a: &SceneAtom, b: &SceneAtom) -> f64 {
    (0..3)
        .map(|i| (a.pos[i] - b.pos[i]).powi(2))
        .sum::<f64>()
        .sqrt()
}
pub(crate) fn contacts(atoms: &[SceneAtom]) -> Vec<[usize; 2]> {
    let mut out = Vec::new();
    for i in 0..atoms.len() {
        for j in i + 1..atoms.len() {
            let d = distance(&atoms[i], &atoms[j]);
            if d > 0.4
                && d <= 1.2 * (covalent_radius(atoms[i].z) + covalent_radius(atoms[j].z)) as f64
            {
                out.push([i, j]);
            }
        }
    }
    out
}
/// Keep the nearby heavy-atom coordination shell at both ends of a contact.
/// Hydrogen does not set a heavy atom's shell distance (which would prune C–C
/// bonds just because C–H is shorter). This is a viewing heuristic, not bond
/// order/valence assignment; All contacts exposes the broader radius rule.
pub(crate) fn nearest_bonds(atoms: &[SceneAtom], candidates: &[[usize; 2]]) -> Vec<[usize; 2]> {
    let mut nearest = vec![f64::INFINITY; atoms.len()];
    for &[i, j] in candidates {
        if atoms[i].z == 1 || atoms[j].z == 1 {
            continue;
        }
        let d = distance(&atoms[i], &atoms[j]);
        nearest[i] = nearest[i].min(d);
        nearest[j] = nearest[j].min(d);
    }
    candidates
        .iter()
        .copied()
        .filter(|&[i, j]| {
            atoms[i].z == 1
                || atoms[j].z == 1
                || distance(&atoms[i], &atoms[j]) <= 1.25 * nearest[i].min(nearest[j]) + 1e-6
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::molecule_view::{AtomStyle, MoleculeScene, PolyhedronOptions};
    use super::*;
    use crate::structure::Cluster;
    use rexafs::xafs::structure as core;
    fn scene(key: &str, element: &str) -> MoleculeScene {
        let s = core::read_cif(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../rexafs/data/builtin_cifs")
                .join(format!("{key}.cif")),
        )
        .unwrap();
        let c = core::build_cluster(
            &s,
            &core::AbsorberSelection::Element(element.into()),
            &Default::default(),
        )
        .unwrap();
        MoleculeScene::new(
            &Cluster::from_core(&c),
            None,
            8.,
            AtomStyle::BallStick,
            None,
            None,
            PolyhedronOptions::default(),
        )
    }
    #[test]
    fn rutile_retains_tio6_without_titanium_titanium_contacts() {
        let mut s = scene("tio2_rutile", "Ti");
        assert!(
            s.all_bonds
                .iter()
                .any(|&[i, j]| s.atoms[i].z == 22 && s.atoms[j].z == 22)
        );
        assert!(s.bonds.iter().all(|&[i, j]| s.atoms[i].z != s.atoms[j].z));
        assert!(s.bonds.len() < s.all_bonds.len());
        let auto = s.bonds.len();
        let all = s.all_bonds.len();
        s.apply_bond_mode(BondMode::Absorber);
        assert_eq!(s.bonds.len(), 6);
        assert!(
            s.bonds
                .iter()
                .all(|&[i, j]| s.atoms[i].absorber || s.atoms[j].absorber)
        );
        eprintln!(
            "Rutile bonds: {all} broad contacts → {auto} automatic bonds → {} absorber bonds",
            s.bonds.len()
        );
    }
    #[test]
    fn elemental_metals_keep_the_first_coordination_shell() {
        for (key, element) in [("cu", "Cu"), ("ni", "Ni"), ("ru_hcp", "Ru")] {
            let mut s = scene(key, element);
            s.apply_bond_mode(BondMode::Absorber);
            assert_eq!(s.bonds.len(), 12, "{element} first shell");
        }
    }
    #[test]
    fn explicit_contact_and_no_bond_modes_do_not_change_atoms() {
        let mut s = scene("tio2_rutile", "Ti");
        let n = s.atoms.len();
        s.apply_bond_mode(BondMode::AllContacts);
        assert_eq!(s.bonds, s.all_bonds);
        s.apply_bond_mode(BondMode::None);
        assert!(s.bonds.is_empty());
        assert_eq!(s.atoms.len(), n);
    }
    #[test]
    fn complete_molecules_preserve_covalent_connectivity() {
        for (key, atoms, bonds) in [("urea", 8, 7), ("aspirin", 21, 21)] {
            let crystal = core::read_cif(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../rexafs/data/builtin_cifs")
                    .join(format!("{key}.cif")),
            )
            .unwrap();
            let cluster = core::build_cluster(
                &crystal,
                &core::AbsorberSelection::Element("C".into()),
                &Default::default(),
            )
            .unwrap();
            let molecule = super::super::molecular_geometry::complete_molecule(
                &crystal,
                cluster.absorber_site,
            )
            .unwrap();
            let mut scene = MoleculeScene::molecule(
                &molecule,
                &Cluster::from_core(&cluster),
                8.,
                true,
                AtomStyle::BallStick,
            );
            scene.apply_bond_mode(BondMode::Auto);
            assert_eq!(scene.atoms.len(), atoms);
            assert_eq!(scene.bonds.len(), bonds, "{key}");
        }
    }
}
