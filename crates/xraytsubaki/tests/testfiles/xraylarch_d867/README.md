XrayLarch fixture subset imported from:
https://github.com/xraypy/xraylarch/tree/d8678dd666fd95839fe9dc71b4dbe8bedec278ff/examples

Included fixtures:
- FEFF run inputs: feff8l/Co, feff8l/FeO_withPb, feff8l/MnO2, feffit/Feff_ZnSe/feff.inp
- Fit paths: feffit/Feff_Cu/*.dat, feffit/Feff_ZnSe/*.dat
- Fit data: xafsdata/cu_150k.xmu, xafsdata/znse_zn_xafs.001, xafsdata/ni_metal_rt.xdi

Metal foil validation uses the original, unmodified measured data and headers:
- Cu: `cu_150k.xmu`, 99.999% metal foil, 150 K, NSLS X-11A (September 1992).
- Ni: `ni_metal_rt.xdi`, standard foil (Joe Wong boxed set), room temperature,
  APS 13-ID-C (2001-06-26). Columns are energy in eV, precomputed mutrans, and I0.
  Added from the same pinned upstream commit above; no numeric transformation.
