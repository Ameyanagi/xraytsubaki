"""Generate the space-group operation table used by xafs::structure::symmetry.

Run:  uv run --project crates/xraytsubaki/tests/pythonscript python crates/xraytsubaki/scripts/generate_spacegroup_table.py
Writes src/xafs/structure/spacegroups.json.gz (one entry per spglib Hall number 1..530).
"""
import gzip, json
from fractions import Fraction
from pathlib import Path
import spglib

OUT = Path(__file__).resolve().parents[1] / "src" / "xafs" / "structure" / "spacegroups.json.gz"
OUT.parent.mkdir(parents=True, exist_ok=True)

def op_string(rot, trans):
    parts = []
    for row, t in zip(rot, trans):
        s = ""
        for coef, name in zip(row, "xyz"):
            if coef == 1: s += f"+{name}"
            elif coef == -1: s += f"-{name}"
            elif coef != 0: raise ValueError(coef)
        f = Fraction(float(t)).limit_denominator(12)
        if f != 0:
            s += f"+{f.numerator}/{f.denominator}" if f > 0 else f"-{abs(f.numerator)}/{f.denominator}"
        parts.append(s.lstrip("+"))
    return ",".join(parts)

entries = []
for hall in range(1, 531):
    t = spglib.get_spacegroup_type(hall)
    sym = spglib.get_symmetry_from_database(hall)
    ops = [op_string(r, tr) for r, tr in zip(sym["rotations"], sym["translations"])]
    entries.append({
        "hall_number": hall,
        "number": t.number,
        "hall": t.hall_symbol,
        "hm_short": t.international_short,
        "hm_full": t.international_full,
        "hm": t.international,
        "choice": t.choice,
        "ops": ops,
    })
with gzip.open(OUT, "wt", compresslevel=9) as fh:
    json.dump(entries, fh, separators=(",", ":"))
print(OUT, OUT.stat().st_size, "bytes,", len(entries), "entries")
