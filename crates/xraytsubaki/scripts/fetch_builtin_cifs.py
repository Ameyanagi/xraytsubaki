#!/usr/bin/env python3
"""Fetch the built-in EXAFS standard structures from the COD (CC0) into
crates/xraytsubaki/data/builtin_cifs/ and write catalog.json.

Selection per target: COD entries whose Hill formula matches exactly and whose
space-group number matches; prefer entries with coordinates, no partial
occupancy hints, most recent year. Re-run to refresh; existing files are
kept unless --force.
"""
import json, os, sys, time, urllib.parse, urllib.request, re
BASE = "https://www.crystallography.net/cod"
UA = "xraytsubaki-builtin-fetch/1.0 (+https://github.com/Ameyanagi/xraytsubaki)"
HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.normpath(os.path.join(HERE, "..", "data", "builtin_cifs"))
# key, display name, Hill formula (COD 'formula' field, without dashes), sg number, category, note
TARGETS = [
 ("cu","Copper","Cu",225,"metal",""),("fe_bcc","α-Iron (bcc)","Fe",229,"metal",""),("ni","Nickel","Ni",225,"metal",""),
 ("co_hcp","Cobalt (hcp)","Co",194,"metal",""),("pt","Platinum","Pt",225,"metal",""),("pd","Palladium","Pd",225,"metal",""),
 ("au","Gold","Au",225,"metal",""),("ag","Silver","Ag",225,"metal",""),("ru_hcp","Ruthenium (hcp)","Ru",194,"metal",""),
 ("rh","Rhodium","Rh",225,"metal",""),("ir","Iridium","Ir",225,"metal",""),("mo","Molybdenum","Mo",229,"metal",""),
 ("w","Tungsten","W",229,"metal",""),("ti_hcp","Titanium (hcp)","Ti",194,"metal",""),("zn","Zinc","Zn",194,"metal",""),
 ("sn_beta","β-Tin","Sn",141,"metal",""),("al","Aluminium","Al",225,"metal",""),("cr","Chromium","Cr",229,"metal",""),
 ("v","Vanadium","V",229,"metal",""),("mn_alpha","α-Manganese","Mn",217,"metal",""),("nb","Niobium","Nb",229,"metal",""),
 ("ta","Tantalum","Ta",229,"metal",""),("re","Rhenium","Re",194,"metal",""),("os","Osmium","Os",194,"metal",""),
 ("ruo2","RuO₂ rutile","O2 Ru",136,"oxide",""),("tio2_rutile","TiO₂ rutile","O2 Ti",136,"oxide",""),("tio2_anatase","TiO₂ anatase","O2 Ti",141,"oxide",""),
 ("fe2o3_hematite","α-Fe₂O₃ hematite","Fe2 O3",167,"oxide",""),("fe3o4_magnetite","Fe₃O₄ magnetite","Fe3 O4",227,"oxide",""),
 ("feo_wustite","FeO wüstite","Fe O",225,"oxide",""),("nio","NiO bunsenite","Ni O",225,"oxide",""),("coo","CoO","Co O",225,"oxide",""),
 ("co3o4","Co₃O₄ spinel","Co3 O4",227,"oxide",""),("cuo_tenorite","CuO tenorite","Cu O",15,"oxide",""),("cu2o_cuprite","Cu₂O cuprite","Cu2 O",224,"oxide",""),
 ("zno_wurtzite","ZnO wurtzite","O Zn",186,"oxide",""),("mno","MnO manganosite","Mn O",225,"oxide",""),("mn2o3_bixbyite","Mn₂O₃ bixbyite","Mn2 O3",206,"oxide",""),
 ("mno2_pyrolusite","β-MnO₂ pyrolusite","Mn O2",136,"oxide",""),("moo3","α-MoO₃","Mo O3",62,"oxide",""),("wo3_monoclinic","WO₃ (monoclinic)","O3 W",14,"oxide",""),
 ("ceo2","CeO₂ fluorite","Ce O2",225,"oxide",""),("zro2_monoclinic","m-ZrO₂ baddeleyite","O2 Zr",14,"oxide",""),("sno2_cassiterite","SnO₂ cassiterite","O2 Sn",136,"oxide",""),
 ("v2o5","V₂O₅ shcherbinaite","O5 V2",59,"oxide",""),("cr2o3_eskolaite","Cr₂O₃ eskolaite","Cr2 O3",167,"oxide",""),("al2o3_corundum","α-Al₂O₃ corundum","Al2 O3",167,"oxide",""),
 ("pdo","PdO","O Pd",131,"oxide",""),("ag2o","Ag₂O","Ag2 O",224,"oxide",""),("pto2_alpha","α-PtO₂","O2 Pt",164,"oxide",""),
 ("fes2_pyrite","FeS₂ pyrite","Fe S2",205,"sulfide",""),("mos2_2h","2H-MoS₂ molybdenite","Mo S2",194,"sulfide",""),("zns_sphalerite","ZnS sphalerite","S Zn",216,"sulfide",""),
 ("cufes2_chalcopyrite","CuFeS₂ chalcopyrite","Cu Fe S2",122,"sulfide",""),("nis_millerite","NiS millerite","Ni S",160,"sulfide",""),("cos2_cattierite","CoS₂ cattierite","Co S2",205,"sulfide",""),
 ("nacl_halite","NaCl halite","Cl Na",225,"other",""),("caf2_fluorite","CaF₂ fluorite","Ca F2",225,"other",""),("licoo2","LiCoO₂","Co Li O2",166,"other",""),
 ("lifepo4_triphylite","LiFePO₄ triphylite","Fe Li O4 P",62,"other",""),
]
def get(url, retries=3):
    for i in range(retries):
        try:
            req = urllib.request.Request(url, headers={"User-Agent": UA})
            with urllib.request.urlopen(req, timeout=60) as r:
                return r.read().decode("utf-8", "replace")
        except Exception as e:
            err = e; time.sleep(2.0 * (i + 1))
    raise err
def main():
    force = "--force" in sys.argv
    os.makedirs(OUT, exist_ok=True)
    catalog = []
    for key, name, formula, sgn, cat, note in TARGETS:
        dest = os.path.join(OUT, key + ".cif")
        meta_path = os.path.join(OUT, key + ".json")
        if os.path.exists(dest) and os.path.exists(meta_path) and not force:
            catalog.append(json.load(open(meta_path))); print(f"{key}: cached"); continue
        q = urllib.parse.urlencode({"format": "json", "formula": formula})
        time.sleep(0.6)
        try:
            rows = json.loads(get(f"{BASE}/result?{q}"))
        except Exception as e:
            print(f"{key}: query failed {e}"); continue
        cands = []
        for r in rows:
            try: n = int(r.get("sgNumber") or 0)
            except ValueError: n = 0
            if n != sgn: continue
            if (r.get("formula") or "").strip("- ") != formula: continue
            title = (r.get("title") or "").lower()
            bad = any(w in title for w in ("pressure", "gpa", "nano", "thin film", "amorph", "hydrogen", "deuter", "high-temperature", "melt"))
            year = int(r.get("year") or 0)
            cands.append((bad, -year, r))
        if not cands:
            print(f"{key}: no candidate (rows={len(rows)})"); continue
        cands.sort(key=lambda t: (t[0], t[1]))
        r = cands[0][2]; cid = str(r["file"])
        time.sleep(0.6)
        try:
            cif = get(f"{BASE}/{cid}.cif")
        except Exception as e:
            print(f"{key}: cif download failed {e}"); continue
        if "_atom_site_fract_x" not in cif:
            print(f"{key}: {cid} has no coordinates, trying next"); 
            ok = False
            for _, _, r2 in cands[1:6]:
                time.sleep(0.6); cid = str(r2["file"]); cif = get(f"{BASE}/{cid}.cif")
                if "_atom_site_fract_x" in cif: r = r2; ok = True; break
            if not ok: print(f"{key}: no coordinates in any candidate"); continue
        open(dest, "w").write(cif)
        meta = {"key": key, "name": name, "formula": formula, "sg_number": sgn, "sg": r.get("sg"), "category": cat,
                "source": "COD", "id": f"cod-{cid}", "url": f"{BASE}/{cid}.html", "year": r.get("year"), "journal": r.get("journal"),
                "authors": r.get("authors"), "doi": r.get("doi"), "license": "CC0 1.0 (Crystallography Open Database)",
                "citation": "Gražulis et al. (2012) Nucleic Acids Res. 40, D420–D427", "note": note}
        json.dump(meta, open(meta_path, "w"), indent=1, ensure_ascii=False)
        catalog.append(meta); print(f"{key}: {cid} ({r.get('sg')}, {r.get('year')})")
    # Keep the neutron-diffraction molecule examples pinned: their complete H
    # positions are intentional, so a newest-formula-match query is unsuitable.
    for key in ("urea", "aspirin"):
        with open(os.path.join(OUT, key + ".json"), encoding="utf-8") as f:
            meta = json.load(f)
        dest = os.path.join(OUT, key + ".cif")
        if force or not os.path.exists(dest):
            cid = meta["id"].removeprefix("cod-")
            cif = get(f"{BASE}/{cid}.cif")
            if "_atom_site_fract_x" not in cif:
                raise ValueError(f"{key}: pinned COD entry has no coordinates")
            with open(dest, "w", encoding="utf-8") as f:
                f.write(cif)
        catalog.append(meta)
        print(f"{key}: pinned {meta['id']}")
    json.dump(catalog, open(os.path.join(OUT, "catalog.json"), "w"), indent=1, ensure_ascii=False)
    print("catalog entries:", len(catalog))
if __name__ == "__main__":
    main()
