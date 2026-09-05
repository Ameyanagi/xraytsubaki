//! Structure module: CIF parsing / symmetry expansion parity with pymatgen,
//! clusters, feff.inp, databases and FEFF path geometry.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::Value;
use xraytsubaki::xafs::fitting::{parse_feff_path_file, FeffFlavor};
use xraytsubaki::xafs::structure::{
    absorber_sites, build_cluster, read_cif, structure_from_cif, structure_to_cif, write_feff_inp,
    AbsorberSelection, ClusterOptions, Edge, FeffInputOptions, FeffInputStyle, LocalCifLibrary,
    PathGeometry, StructureQuery, StructureSource,
};

fn testfiles() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/testfiles")
}

fn fixture(name: &str) -> (xraytsubaki::xafs::structure::Structure, Value) {
    let dir = testfiles().join("cif");
    let s = read_cif(dir.join(format!("{name}.cif"))).expect("cif parses");
    let json: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join(format!("{name}.json"))).unwrap())
            .unwrap();
    (s, json)
}

/// Compare our expanded sites with pymatgen's (order-independent, periodic).
fn assert_sites_match(s: &xraytsubaki::xafs::structure::Structure, json: &Value, tol: f64) {
    let expected = json["sites"].as_array().unwrap();
    assert_eq!(s.num_sites(), expected.len(), "site count");
    for e in expected {
        let sym = e["species"].as_str().unwrap();
        let f: Vec<f64> = e["frac"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_f64().unwrap())
            .collect();
        let found = s.sites.iter().any(|site| {
            site.majority().map(|sp| sp.symbol.as_str()) == Some(sym)
                && (0..3).all(|i| {
                    let d = (site.frac[i] - f[i]).abs();
                    d.min((1.0 - d).abs()) < tol
                })
        });
        assert!(found, "site {sym} at {f:?} missing from expansion");
    }
}

fn neighbor_map(json: &Value, key: &str) -> BTreeMap<String, u64> {
    json[key]
        .as_object()
        .unwrap()
        .iter()
        .map(|(k, v)| (k.clone(), v.as_u64().unwrap()))
        .collect()
}

#[test]
fn structure_cif_expansion_matches_pymatgen() {
    for name in ["ru_hcp", "ruo2_rutile", "fes2_pyrite", "zro2_baddeleyite"] {
        for variant in ["", "_nosymops", "_p1"] {
            let dir = testfiles().join("cif");
            let s = read_cif(dir.join(format!("{name}{variant}.cif"))).expect("cif parses");
            let (_, json) = fixture(name);
            let lat = &json["lattice"];
            assert!(
                (s.lattice.a - lat["a"].as_f64().unwrap()).abs() < 1e-6,
                "{name}{variant} a"
            );
            assert!(
                (s.lattice.c - lat["c"].as_f64().unwrap()).abs() < 1e-6,
                "{name}{variant} c"
            );
            assert!(
                (s.lattice.beta - lat["beta"].as_f64().unwrap()).abs() < 1e-6,
                "{name}{variant} beta"
            );
            assert!((s.lattice.volume() - lat["volume"].as_f64().unwrap()).abs() < 1e-3);
            assert_eq!(
                s.num_sites(),
                json["num_sites"].as_u64().unwrap() as usize,
                "{name}{variant}"
            );
            assert_sites_match(&s, &json, 1e-4);
            assert_eq!(
                s.formula(),
                json["formula"].as_str().unwrap(),
                "{name}{variant} formula"
            );
            if variant != "_p1" {
                assert_eq!(
                    s.space_group.number,
                    Some(json["spacegroup_number"].as_u64().unwrap() as u16),
                    "{name}{variant} space group number"
                );
            }
        }
    }
}

#[test]
fn structure_no_symops_uses_bundled_table() {
    let dir = testfiles().join("cif");
    let text = std::fs::read_to_string(dir.join("zro2_baddeleyite_nosymops.cif")).unwrap();
    assert!(
        !text.contains("_symmetry_equiv_pos_as_xyz"),
        "fixture must lack symops"
    );
    let s = structure_from_cif(&text).unwrap();
    assert_eq!(s.space_group.operations.len(), 4);
    assert_eq!(s.num_sites(), 12);
    assert_eq!(s.asymmetric_sites.len(), 3);
    assert_eq!(s.sites[0].multiplicity, Some(4));
}

#[test]
fn structure_cif_round_trip_through_writer() {
    let (s, json) = fixture("fes2_pyrite");
    let text = structure_to_cif(&s);
    let back = structure_from_cif(&text).unwrap();
    assert_eq!(back.num_sites(), 12);
    assert_sites_match(&back, &json, 1e-5);
    assert_eq!(back.formula(), "FeS2");
}

#[test]
fn structure_cluster_shells_match_pymatgen_neighbors() {
    for name in ["ru_hcp", "ruo2_rutile", "fes2_pyrite", "zro2_baddeleyite"] {
        let (s, json) = fixture(name);
        // pymatgen's site 0 is the first expanded site of the first asymmetric
        // site; ours too, but match by species + position to be safe.
        let site0 = json["sites"][0].clone();
        let sym0 = site0["species"].as_str().unwrap();
        let f0: Vec<f64> = site0["frac"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_f64().unwrap())
            .collect();
        let idx = s
            .sites
            .iter()
            .position(|site| {
                site.majority().map(|sp| sp.symbol.as_str()) == Some(sym0)
                    && (0..3).all(|i| {
                        let d = (site.frac[i] - f0[i]).abs();
                        d.min((1.0 - d).abs()) < 1e-4
                    })
            })
            .expect("site 0 present");
        for (key, radius) in [("neighbors_site0_3A", 3.0), ("neighbors_site0_8A", 8.0)] {
            let cluster = build_cluster(
                &s,
                &AbsorberSelection::SiteIndex(idx),
                &ClusterOptions {
                    radius,
                    ..Default::default()
                },
            )
            .unwrap();
            // Compare the sorted neighbour distances per element pairwise;
            // pymatgen's keys are rounded to 3 decimals, so allow 0.002 Å.
            let mut ours: BTreeMap<String, Vec<f64>> = BTreeMap::new();
            for atom in cluster.atoms.iter().skip(1) {
                ours.entry(atom.symbol.clone())
                    .or_default()
                    .push(atom.distance);
            }
            let mut expected: BTreeMap<String, Vec<f64>> = BTreeMap::new();
            for (k, n) in neighbor_map(&json, key) {
                let sym = k.split('@').next().unwrap().to_string();
                let d: f64 = k.split('@').nth(1).unwrap().parse().unwrap();
                for _ in 0..n {
                    expected.entry(sym.clone()).or_default().push(d);
                }
            }
            assert_eq!(
                ours.keys().collect::<Vec<_>>(),
                expected.keys().collect::<Vec<_>>(),
                "{name} {key}: elements"
            );
            for (sym, exp) in &expected {
                let mut got = ours[sym].clone();
                got.sort_by(|a, b| a.total_cmp(b));
                let mut exp = exp.clone();
                exp.sort_by(|a, b| a.total_cmp(b));
                assert_eq!(got.len(), exp.len(), "{name} {key}: {sym} neighbour count");
                for (g, e) in got.iter().zip(exp.iter()) {
                    assert!(
                        (g - e).abs() <= 0.002,
                        "{name} {key}: {sym} distance {g} vs {e}"
                    );
                }
            }
        }
    }
    // Ru hcp first shell: 6 + 6 neighbours within 2.71 Å.
    let (s, _) = fixture("ru_hcp");
    let cluster = build_cluster(
        &s,
        &AbsorberSelection::Element("Ru".into()),
        &ClusterOptions::default(),
    )
    .unwrap();
    let first: usize = cluster
        .atoms
        .iter()
        .skip(1)
        .filter(|a| a.distance < 2.71)
        .count();
    assert_eq!(first, 12);
    let shells = cluster.shells(0.01);
    assert_eq!(shells[0].count + shells[1].count, 12);
    assert_eq!(cluster.potentials.len(), 2);
    assert_eq!(cluster.atoms[0].ipot, 0);
    assert!(cluster.atoms.iter().skip(1).all(|a| a.ipot == 1));
    assert_eq!(absorber_sites(&s, "Ru"), vec![0]);
    assert!(absorber_sites(&s, "O").is_empty());
}

#[test]
fn structure_feff_inp_has_expected_cards() {
    let (s, _) = fixture("ruo2_rutile");
    let sites = absorber_sites(&s, "Ru");
    assert_eq!(sites.len(), 1);
    let cluster = build_cluster(
        &s,
        &AbsorberSelection::Element("Ru".into()),
        &ClusterOptions {
            radius: 6.0,
            ..Default::default()
        },
    )
    .unwrap();
    let inp = write_feff_inp(&cluster, &FeffInputOptions::default());
    assert!(inp.contains("HOLE 1"));
    assert!(inp.contains("POTENTIALS"));
    assert!(inp.contains("    0   44   Ru0"));
    assert!(inp.contains("ATOMS"));
    assert!(inp.trim_end().ends_with("END"));
    // Potentials: absorber, then O (nearest), then Ru.
    assert_eq!(cluster.potentials[1].symbol, "O");
    assert_eq!(cluster.potentials[2].symbol, "Ru");
    let feff8 = write_feff_inp(
        &cluster,
        &FeffInputOptions {
            edge: Edge::L3,
            style: FeffInputStyle::Feff8,
            scf: true,
            ..Default::default()
        },
    );
    assert!(
        feff8.contains("EDGE      L3")
            && feff8.contains("SCF       5.0")
            && feff8.contains("RPATH")
    );
}

#[cfg(feature = "refeff-runner")]
#[test]
fn structure_feff_inp_runs_through_refeff() {
    use xraytsubaki::prelude::{run_feff_and_load_paths, FeffExecutionMode, FeffRunRequest};
    let (s, _) = fixture("ru_hcp");
    let cluster = build_cluster(
        &s,
        &AbsorberSelection::Element("Ru".into()),
        &ClusterOptions {
            radius: 5.0,
            ..Default::default()
        },
    )
    .unwrap();
    let inp = write_feff_inp(&cluster, &FeffInputOptions::default());
    let dir = std::env::temp_dir().join(format!("xts-structure-refeff-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let inp_path = dir.join("feff.inp");
    std::fs::write(&inp_path, &inp).unwrap();
    let request = FeffRunRequest {
        executable_path: PathBuf::new(),
        workspace_dir: dir.clone(),
        feffinp: Some(inp_path),
        mode: FeffExecutionMode::RefeffPipeline,
        timeout_sec: None,
        use_sfconv: false,
        keep_all_outputs: false,
    };
    let paths = run_feff_and_load_paths(&request, FeffFlavor::Feff85L).expect("refeff runs");
    assert!(!paths.is_empty());
    let first = paths
        .iter()
        .map(|p| p.feff.reff)
        .fold(f64::INFINITY, f64::min);
    // Ru hcp first shell 2.65 / 2.706 Å.
    assert!(
        (first - 2.65).abs() < 0.03 || (first - 2.706).abs() < 0.03,
        "first Reff {first}"
    );
    // Path geometry maps back onto the cluster.
    let dat = &paths[0].feff;
    let mut geom = PathGeometry::from_feffdat(dat).expect("geometry present");
    assert!((geom.half_length() - dat.reff).abs() < 0.02);
    let matched = geom.map_to_cluster(&cluster, 0.05);
    assert_eq!(matched, geom.legs.len());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn structure_path_geometry_from_feff_dat() {
    let dat = parse_feff_path_file(
        testfiles().join("feffcu01.dat").to_str().unwrap(),
        FeffFlavor::Feff85L,
    )
    .unwrap();
    assert_eq!(dat.geometry_atoms.len(), 2);
    let geom = PathGeometry::from_feffdat(&dat).unwrap();
    assert_eq!(geom.legs[0].symbol, "Cu");
    assert_eq!(geom.legs[0].cart, [0.0, 0.0, 0.0]);
    assert!((geom.legs[1].cart[1] + 1.8016).abs() < 1e-6);
    assert!((geom.half_length() - dat.reff).abs() < 1e-3);
    assert_eq!(geom.polyline().len(), 3);
}

#[test]
fn structure_local_library_indexes_and_searches() {
    let lib = LocalCifLibrary::scan(testfiles().join("cif")).unwrap();
    assert!(lib.len() >= 12, "{} entries", lib.len());
    assert!(lib.failures.is_empty(), "{:?}", lib.failures);
    let hits = lib.search(&StructureQuery::text("RuO2")).unwrap();
    assert!(hits.iter().all(|h| h.formula == "RuO2"));
    assert_eq!(hits.len(), 3);
    let hits = lib
        .search(&StructureQuery::default().with_elements(["Zr", "O"]))
        .unwrap();
    assert_eq!(hits.len(), 3);
    let s = lib.fetch(&hits[0]).unwrap();
    assert_eq!(s.formula(), "ZrO2");
    assert!(s.source.starts_with("cif:"));
}

#[cfg(feature = "amcsd")]
#[test]
fn structure_amcsd_subset_search_and_fetch() {
    use xraytsubaki::xafs::structure::db::amcsd::Amcsd;
    let db = Amcsd::open(testfiles().join("amcsd_subset.db")).unwrap();
    assert_eq!(db.len().unwrap(), 5);
    let hits = db.search(&StructureQuery::text("pyrite")).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].name.as_deref(), Some("Pyrite"));
    let s = db.fetch(&hits[0]).unwrap();
    assert_eq!(s.formula(), "FeS2");
    assert_eq!(s.num_sites(), 12);
    let cluster = build_cluster(
        &s,
        &AbsorberSelection::Element("Fe".into()),
        &ClusterOptions {
            radius: 3.0,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(cluster.atoms.len() - 1, 6, "octahedral Fe–S");
    let hits = db
        .search(&StructureQuery::default().with_elements(["Ru", "O"]))
        .unwrap();
    assert!(hits
        .iter()
        .any(|h| h.formula.contains("Ru") && h.formula.contains("O")));
    let ruo2 = hits
        .iter()
        .find(|h| h.formula == "RuO2")
        .expect("RuO2 record");
    let s = db.fetch(ruo2).unwrap();
    assert_eq!(s.num_sites(), 6);
    let hits = db.search(&StructureQuery::text("baddeleyite")).unwrap();
    let s = db.fetch(&hits[0]).unwrap();
    assert_eq!(s.formula(), "ZrO2");
    assert_eq!(s.num_sites(), 12);
}

#[cfg(feature = "materials-project")]
#[test]
fn structure_materials_project_fixture_converts() {
    use xraytsubaki::xafs::structure::db::mp::{
        hit_from_doc, structure_from_doc, summary_docs, MaterialsProject,
    };
    let v: Value = serde_json::from_str(
        &std::fs::read_to_string(testfiles().join("mp_summary_ru.json")).unwrap(),
    )
    .unwrap();
    let docs = summary_docs(&v).unwrap();
    let hit = hit_from_doc(&docs[0]).unwrap();
    assert_eq!(hit.id, "mp-33");
    assert_eq!(hit.elements, vec!["Ru"]);
    assert_eq!(hit.space_group.as_deref(), Some("P6_3/mmc"));
    let s = structure_from_doc(&docs[0]).unwrap();
    assert_eq!(s.num_sites(), 2);
    assert_eq!(s.source, "mp:mp-33");
    assert!((s.lattice.c - 4.282).abs() < 1e-6);
    let cluster = build_cluster(
        &s,
        &AbsorberSelection::Element("Ru".into()),
        &ClusterOptions::default(),
    )
    .unwrap();
    assert_eq!(
        cluster
            .atoms
            .iter()
            .skip(1)
            .filter(|a| a.distance < 2.71)
            .count(),
        12
    );
    let q = MaterialsProject::summary_query(&StructureQuery::text("RuO2"));
    assert!(q.starts_with("materials/summary/?formula=RuO2"));
    let q = MaterialsProject::summary_query(&StructureQuery::text("Ru-O"));
    assert!(q.contains("chemsys=Ru-O"));
}

#[cfg(feature = "materials-project")]
#[test]
#[ignore = "needs MP_API_KEY and network"]
fn structure_materials_project_live() {
    use xraytsubaki::xafs::structure::db::mp::{MaterialsProject, MaterialsProjectConfig};
    let key = std::env::var("MP_API_KEY").expect("MP_API_KEY");
    let mp = MaterialsProject::new(MaterialsProjectConfig::new(&key));
    let hits = mp.search(&StructureQuery::text("RuO2")).unwrap();
    assert!(!hits.is_empty());
    let s = mp.fetch(&hits[0]).unwrap();
    assert_eq!(s.formula(), "RuO2");
}

/// Parity of the ATOMS block with pymatgen's `FeffAtoms` (Larch/pymatgen
/// cluster convention): same sorted (distance, symbol) list.
#[test]
#[ignore = "runs pymatgen through uv; slow"]
fn structure_atoms_block_matches_pymatgen_feff_atoms() {
    let (s, _) = fixture("ruo2_rutile");
    let cluster = build_cluster(
        &s,
        &AbsorberSelection::Element("Ru".into()),
        &ClusterOptions {
            radius: 6.0,
            ..Default::default()
        },
    )
    .unwrap();
    let ours: Vec<(f64, String)> = cluster
        .atoms
        .iter()
        .skip(1)
        .map(|a| ((a.distance * 1000.0).round() / 1000.0, a.symbol.clone()))
        .collect();
    let project = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/pythonscript");
    let cif = testfiles().join("cif/ruo2_rutile.cif");
    let script = format!(
        "from pymatgen.core import Structure\nfrom pymatgen.io.feff.inputs import Atoms\n\
         s = Structure.from_file(r'{}')\natoms = Atoms(s, 'Ru', 6.0)\n\
         for line in atoms.get_lines()[1:]:\n    print(line[5], line[4])\n",
        cif.display()
    );
    let out = std::process::Command::new("uv")
        .args([
            "run",
            "--project",
            project.to_str().unwrap(),
            "python",
            "-c",
            &script,
        ])
        .output()
        .expect("uv runs");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let mut theirs: Vec<(f64, String)> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| {
            let mut w = l.split_whitespace();
            let d: f64 = w.next()?.parse().ok()?;
            Some(((d * 1000.0).round() / 1000.0, w.next()?.to_string()))
        })
        .collect();
    theirs.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    let mut ours_sorted = ours.clone();
    ours_sorted.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    assert_eq!(ours_sorted.len(), theirs.len(), "atom count");
    for (o, t) in ours_sorted.iter().zip(theirs.iter()) {
        assert!((o.0 - t.0).abs() <= 0.002 && o.1 == t.1, "{o:?} vs {t:?}");
    }
}
