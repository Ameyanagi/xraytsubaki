#!/usr/bin/env python3
"""Pack data/builtin_cifs/*.cif + catalog.json into
src/xafs/structure/builtin_library.json.gz (embedded with include_bytes!).

Run after scripts/fetch_builtin_cifs.py, or after editing a CIF by hand.
"""
import gzip, json, os
HERE = os.path.dirname(os.path.abspath(__file__))
DATA = os.path.normpath(os.path.join(HERE, "..", "data", "builtin_cifs"))
OUT = os.path.normpath(os.path.join(HERE, "..", "src", "xafs", "structure", "builtin_library.json.gz"))

def main():
    catalog = json.load(open(os.path.join(DATA, "catalog.json")))
    entries = []
    for meta in catalog:
        cif_path = os.path.join(DATA, meta["key"] + ".cif")
        if not os.path.exists(cif_path):
            print("missing", cif_path); continue
        cif = open(cif_path, encoding="utf-8", errors="replace").read()
        entry = dict(meta); entry["cif"] = cif
        for k in ("year", "journal", "authors", "doi", "sg", "url"):
            if entry.get(k) is not None:
                entry[k] = str(entry[k])
        entries.append(entry)
    entries.sort(key=lambda e: (e["category"], e["name"]))
    raw = json.dumps(entries, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
    with open(OUT, "wb") as fh:
        with gzip.GzipFile(fileobj=fh, mode="wb", compresslevel=9, mtime=0) as f:
            f.write(raw)
    print(f"{len(entries)} entries, {len(raw)} bytes raw, {os.path.getsize(OUT)} bytes gz -> {OUT}")

if __name__ == "__main__":
    main()
