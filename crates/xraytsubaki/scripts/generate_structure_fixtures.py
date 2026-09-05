"""Generate CIF fixtures + pymatgen parity JSON for the structure module tests.

Run:  uv run --project crates/xraytsubaki/tests/pythonscript python crates/xraytsubaki/scripts/generate_structure_fixtures.py
"""
import json, re
from pathlib import Path
from pymatgen.core import Lattice, Structure
from pymatgen.io.cif import CifWriter
from pymatgen.symmetry.analyzer import SpacegroupAnalyzer

OUT = Path(__file__).resolve().parents[1] / "tests" / "testfiles" / "cif"
OUT.mkdir(parents=True, exist_ok=True)

def hcp_ru():
    return Structure.from_spacegroup("P6_3/mmc", Lattice.hexagonal(2.706, 4.282), ["Ru"], [[1/3, 2/3, 1/4]])
def rutile_ruo2():
    return Structure.from_spacegroup("P4_2/mnm", Lattice.tetragonal(4.4919, 3.1066), ["Ru", "O"], [[0, 0, 0], [0.3058, 0.3058, 0]])
def pyrite_fes2():
    return Structure.from_spacegroup("Pa-3", Lattice.cubic(5.418), ["Fe", "S"], [[0, 0, 0], [0.385, 0.385, 0.385]])
def baddeleyite_zro2():
    lat = Lattice.from_parameters(5.1505, 5.2116, 5.3173, 90, 99.23, 90)
    return Structure.from_spacegroup("P2_1/c", lat, ["Zr", "O", "O"], [[0.2754, 0.0395, 0.2083], [0.070, 0.3317, 0.3447], [0.4416, 0.7569, 0.4792]])

def neighbors(struct, site_index, r):
    out = {}
    for n in struct.get_neighbors(struct[site_index], r):
        d = round(n.nn_distance, 3)
        key = f"{n.specie.symbol}@{d:.3f}"
        out[key] = out.get(key, 0) + 1
    return out

fixtures = {
    "ru_hcp": hcp_ru(),
    "ruo2_rutile": rutile_ruo2(),
    "fes2_pyrite": pyrite_fes2(),
    "zro2_baddeleyite": baddeleyite_zro2(),
}
for name, s in fixtures.items():
    sga = SpacegroupAnalyzer(s, symprec=1e-3)
    conv = sga.get_conventional_standard_structure()
    # CIF with explicit symmetry operations (symprec) — the common database form.
    CifWriter(s, symprec=1e-3).write_file(OUT / f"{name}.cif")
    # Variant without the symop loop: only the H-M symbol / IT number survive.
    text = (OUT / f"{name}.cif").read_text()
    text_nosym = re.sub(r"loop_\n_symmetry_equiv_pos_site_id\n_symmetry_equiv_pos_as_xyz\n(?:.*\n)*?(?=\n|loop_|_)", "", text)
    text_nosym = re.sub(r"loop_\n\s*_symmetry_equiv_pos_site_id\n\s*_symmetry_equiv_pos_as_xyz\n(?:\s*\d+\s+'[^']*'\n)+", "", text)
    (OUT / f"{name}_nosymops.cif").write_text(text_nosym)
    # P1 export (fully expanded cell, no symmetry) for a third parser path.
    CifWriter(s).write_file(OUT / f"{name}_p1.cif")
    data = {
        "formula": s.composition.reduced_formula,
        "formula_sum": s.composition.formula,
        "spacegroup_number": sga.get_space_group_number(),
        "spacegroup_symbol": sga.get_space_group_symbol(),
        "lattice": {"a": s.lattice.a, "b": s.lattice.b, "c": s.lattice.c,
                    "alpha": s.lattice.alpha, "beta": s.lattice.beta, "gamma": s.lattice.gamma,
                    "matrix": s.lattice.matrix.tolist(), "volume": s.lattice.volume},
        "num_sites": len(s),
        "sites": [{"species": site.specie.symbol, "frac": [float(x) for x in site.frac_coords]} for site in s],
        "neighbors_site0_8A": neighbors(s, 0, 8.0),
        "neighbors_site0_3A": neighbors(s, 0, 3.0),
    }
    (OUT / f"{name}.json").write_text(json.dumps(data, indent=1))
    print(name, len(s), "sites", sga.get_space_group_symbol())
