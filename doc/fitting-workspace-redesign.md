# Fitting workspace redesign

## Research and design rationale

Reviewed primary documentation on 2026-09-05. These are design references, not claims of comparative user-study results.

| Reference | Useful concept | Decision for rexafs |
| --- | --- | --- |
| [Athena introduction](https://bruceravel.github.io/demeter/documents/Athena/intro.html) | Current data and marked groups are distinct; processing controls follow the scientific workflow. | Keep the existing spectrum browser and show the fitted spectrum in a persistent summary. Batch scope remains explicit. |
| [Artemis main window](https://bruceravel.github.io/demeter/documents/Artemis/startup/main.html) | Data, FEFF calculations, model, and fitting actions have distinct roles. | Give fitting its own full workspace, with five freely navigable steps and a persistent primary action. |
| [Artemis path browser](https://bruceravel.github.io/demeter/documents/Artemis/feff/paths.html) | Geometry, degeneracy, effective distance, and relative importance help select paths. Rank is an estimate, not proof that a path belongs in the fit. | Put the shell-grouped path table beside the 3D geometry. A row focuses a path; its checkbox controls inclusion. Keep first-shell and importance presets. |
| [Artemis parameters](https://bruceravel.github.io/demeter/documents/Artemis/gds.html) | Fit variables and path-parameter expressions are different layers of the model. | Separate Parameters and Path expressions; use the visible terms vary, fixed, and expression while retaining the variable/expression distinction. A separate parser defect discovered during validation is corrected below. |
| [Artemis model checks](https://bruceravel.github.io/demeter/documents/Artemis/fit/sanity.html) | Catch missing paths and invalid fit ranges before optimization. | Disable Run fit with a visible reason when data, enabled paths, or valid ranges are missing. Numerical fitting retains core validation. |
| [Artemis history](https://bruceravel.github.io/demeter/documents/Artemis/history.html) | Previous models and fit statistics support iteration and recovery. | Give History its own tab beside results and batch, retaining the existing restore mechanism. |
| [Larix overview](https://millenia.cars.aps.anl.gov/xraylarch/larix/overview.html) | Integrated XAS analysis includes structure/CIF tools and FEFF browsing; operations are recorded for reproducibility. Several tasks use separate windows. | Bring the relevant controls and viewer together at each step and retain the existing journal and project persistence. This change does not add script export. |
| [SIXPACK](https://www.sams-xrays.com/sixpack) | An element-oriented FEFF entry point makes starting from known materials approachable. | Expose the existing offline curated library immediately, with Metals, Oxides, Sulfides, and Other filters, plus database and file import options. |
| [VESTA object properties](https://jp-minerals.org/vesta/en/doc/VESTAch12.html) | Atom, bond, and polyhedron styles and opacity serve different inspection tasks. | Offer shaded balls, ball-and-stick, wireframe, and a coordination polyhedron; keep periodic context translucent outside the FEFF sphere. |

## Workspace

1. **Structure:** searchable curated standards, CIF folder, Materials Project, AMCSD, COD, and CIF/XYZ import. A candidate structure is previewed before calculation. Crystal + cluster shows complete surrounding unit cells, faded atoms outside the cutoff, and sphere guides; Cluster only isolates the FEFF atoms. Balls, ball-and-stick, wireframe, and polyhedron modes share a stable orthographic camera. Shading is implemented in the native GPUI canvas; scientific plots retain ruviz. Candidate geometry is separate from the calculated model so browsing cannot silently replace fitted paths.
2. **Calculate:** absorbing element/site, edge, cluster radius, actual embedded engine, custom input, and calculation feedback. Radius errors are reported rather than silently substituted or clamped. The default cluster radius is 8 Å. Both ReFEFF and FEFF-RS (bundled FEFF10 through the feff10 wrapper) are available in the default GUI build.
3. **Paths:** source-specific shell lists beside the structure. Multiple calculations coexist and can contribute to the same fit. Numbered arrows distinguish traversal direction; leg buttons isolate one traversal and Focus path zooms explicitly. Source selection changes the displayed cluster without changing path inclusion.
4. **Model & fit:** processed data before the first fit; Parameters, Fit ranges, and Path expressions in a focused panel. Existing k/R/q views, residuals, path contributions, templates, constraints, and bounds remain available.
5. **Results & batch:** statistics, fitted values and uncertainties, correlations, fit history, and explicit batch scope with progress/cancellation and CSV export.

The top navigation is reversible, including direct entry for imported paths. Completion marks describe available artifacts; a stale-fit message remains visible when inputs change. Calculation and fitting actions have a fixed location. Project loading retains compatibility with older files and now records standalone spectrum paths.

## Verification

Desktop checks use the native app in Cargo release mode, evaluated through computer use:

- The 8 Å Ru hcp preview has 147 atoms; ReFEFF calculates 69 paths. Dragging rotates without zooming or opening an atom card. Ball shading and cluster/crystal toggles render correctly.
- The original single-center polyhedron was obscured by the full cluster. This first inspection led to the repeated-polyhedra implementation described below; the earlier nearest-neighbor-only prototype has been replaced.
- Multiple-scattering legs have arrows, numbers, individual-leg controls, and an explicit Focus path button. Repeated traversals use separate lanes, including outward and return legs along the same bond.
- A second RuO₂ calculation using FEFF-RS adds 240 paths. The metal's 69 paths and existing variables remain. The combined model has 309 paths, five enabled paths, three selected single-scattering shells, and ten variables.
- New source parameters are independent (`p2_`, `p3_` prefixes). Templates explain sharing, and Custom expressions preserves existing edits. Phase fractions are not inferred or inserted automatically.
- Saving and reloading the combined model restores both sources, crystal context, ten variables, the standalone spectrum, and edited ranges k = 3.1–12 Å⁻¹ and R = 1.7–3 Å. Automated persistence checks also cover non-default k weights.
- Invalid ranges disable Run fit with an explanation. Failed fits show a persistent error near the action; large residuals are called out in Results. Missing uncertainties display “unavailable.”
- Resizing the native window from about 1187 to 1022 displayed pixels wraps the viewer controls and keeps source selection and fit actions reachable.
- Release checks completed a single fit and a 12-frame batch (12 completed, zero failed), including a final desktop batch run after the parser fix, using copies of the repository spectrum as a workflow fixture. Batch results now occupy the central table area; they retain CSV export and selection-to-spectrum navigation. Explicit column navigation was verified to reveal the rightmost fitted parameters.

Initial validation: `cargo build --release -p rexafs-gui`; 73 GUI tests passed, with two explicitly invoked local diagnostics ignored in the ordinary suite. All 64 core fitting tests passed with the default trust-region solver enabled; 60 passed with default features disabled, exercising the Levenberg–Marquardt fallback. Later foil, XDI and molecule checks are recorded below. Online database requests requiring service access were not exercised live.

## Upstream rendering request

[ruviz issue #182](https://github.com/Ameyanagi/ruviz/issues/182) requests optional shaded sphere markers for molecular views. It documents the current flat scatter-marker API, the application's layered-circle approximation, the intended crystal/cluster/path workflow, depth and transparency requirements, and release-mode performance checks. GPU sphere impostors are proposed for evaluation; their performance has not yet been measured. The request preserves flat markers as the default and separates ordinary diffuse/specular lighting from later shadow or ambient-occlusion work.

## Fitting defect and engine comparison

The default template named its energy-shift variable `e0`, but the expression grammar matched the constant `e` first and accepted an unparsed suffix. Consequently, `e0` evaluated to 2.718281828 instead of the variable. Names such as `ei` and `pi_scale` were also affected. Constants now require a complete token, and the parser consumes the complete expression. Regression tests cover variable prefixes, malformed suffixes, valid scientific notation, and synthetic recovery with the actual GUI template.

The initial engine comparison used identical Ru hcp 6 Å inputs, the same repository Ru_QAS.dat spectrum, saved GUI processing settings, paths, and fit ranges. **Ru_QAS.dat is not a metal foil.** This reproduced the earlier saved model to diagnose its behavior; it must not be treated as validation against a standard foil. The old loose Ru fitting regression has now been replaced by the measured Cu/Ni foil tests below.

| Check | ReFEFF 0.2.0 | FEFF-RS / feff10 0.2.2 |
| --- | ---: | ---: |
| Generated paths | 28 | 28 |
| Saved GUI model before parser fix: R factor | 0.991883318 | 0.991883054 |
| Saved GUI model after parser fix: R factor | 1.197708053 | 1.197736688 |
| Energy shift after fix (eV) | 9.99509 | 9.99474 |
| Legacy restricted Ru workflow test (removed): R factor | 0.10286 | 0.10285 |

The generators agree closely for this input: for the first two paths, relative RMS differences in the FEFF scattering-amplitude column over k = 3–12 Å⁻¹ are about 5.4×10⁻⁶ and 3.1×10⁻⁶. The parser fix restores the energy-shift degree of freedom and uncertainties; it does **not** make this saved broad-range hcp model a good fit. The resulting objective can remain in a poor local solution with an inadequate model or starting values. This comparison does not establish general engine parity or scientific validity of the selected structures. The synthetic regression recovers a known 3.2 eV shift within 0.01 eV with R factor below 10⁻⁶.

Raw comparison summaries are in [validation/feff-backend-comparison.json](validation/feff-backend-comparison.json). Full calculation artifacts were retained in the local temporary comparison directories. Reproduce with both engine features (now enabled by default):

```sh
REXAFS_COMPARE_PROJECT=/path/to/project.rxs cargo test --release -p rexafs-gui compare_saved_project_backends -- --ignored --nocapture
cargo run --release -p rexafs-gui
```

The comparison helper expects a single calculated source and its standalone spectrum. Source-specific physical constraints and initial values still need to be chosen for a real multi-phase analysis. Calculation inputs are copied into unique workspaces when rerunning an existing source, keeping earlier model path files intact.

## Follow-up: convergence and interaction

The core previously discarded the optimizer's termination report. Fit results now retain the method, stopping reason, convergence flag, iteration/evaluation count when available, and initial/final objective (half the squared weighted residual norm). Older serialized results remain readable with an absent report. Results distinguishes numerical convergence from a low residual, identifies parameters at bounds, and reports insufficient independent information. History retains the report; the batch table and CSV distinguish converged and stopped runs.

A diagnostic with the saved hcp fixture reduced the objective from 2.09274 to 0.15762 and stopped at parameter tolerance after 48 iterations, with R factor 1.19771. Repeating from those fitted values reached R factor 0.96336 with S₀² at its lower bound and ΔR at its upper bound. Another repeat made no material improvement. This is not an iteration-limit failure and is not evidence that the sample is bulk hcp Ru. A real scientific fit requires the sample identity and a defensible model. The runs are preserved in [validation/fit-convergence-diagnostic.json](validation/fit-convergence-diagnostic.json); reproduce with the `diagnose_saved_fit` ignored test and `REXAFS_COMPARE_PROJECT`.

The covariance calculation previously used a forward difference that was clamped to zero at an upper parameter bound, creating a spurious zero Jacobian column. It now uses the available backward direction, sharing the bounded finite-difference rule with the trust-region factor. A regression compares the boundary derivative to the unrestricted derivative of the same FEFF amplitude model. Uncertainties near active bounds still require careful interpretation.

Molecular wheel zoom now limits each event's logarithmic change and uses separate pixel/line sensitivities. Explicit − / + controls change zoom by a factor of 1.2 and display its percentage. Desktop checks confirmed + gives 120%, − returns to 100%, and a large wheel event gives about 116% instead of jumping to 500%. Orbiting preserves that percentage without opening an atom card. Results visibly shows the 48-iteration stopping reason, objective reduction, R ≥ 1 explanation, and S₀² lower-bound warning. Legacy imported geometry without saved cluster-cutoff metadata labels its measured extent “outermost atom” rather than claiming it is the calculation cutoff.

History was exercised through the desktop UI: its entry shows the termination reason and restores the original starting values and model. Parameter-field uncertainties are now shown only when the field still matches the fitted value, avoiding a misleading uncertainty beside a restored starting value. [Result-panel screenshot](validation/fitting-result-diagnostics.jpg).

## Standard metal foils and XDI

The regression now uses measured Cu and Ni metal foils from the pinned XrayLarch fixture revision, with fresh calculations from the curated fcc CIFs. Cu is a 99.999% rolled/annealed 12 µm foil measured at 150 K (NSLS X11A, September 1992). Ni is a room-temperature standard foil from the Joe Wong boxed set (APS 13-ID-C, June 2001). The app's initial example is now Cu, replacing the unidentified Ru workflow fixture.

Both engines receive identical 8 Å inputs. The first single-scattering shell is fitted with four freely varying parameters: S₀², ΔE₀, ΔR and σ². Processing uses the GUI defaults, with k = 2–12 Å⁻¹, R = 1–3 Å and k weights 1, 2 and 3. No fixed-amplitude workaround is used.

| Foil | ReFEFF R-factor | FEFF-RS R-factor | Result |
| --- | ---: | ---: | --- |
| Cu, 150 K | 0.00813466 | 0.00814827 | Both converged in 8 iterations |
| Ni, room temperature | 0.00287188 | 0.00288708 | Both converged in 9 iterations |

All four parameters have finite positive uncertainty estimates and remain inside their bounds. The regression requires R < 0.02, an engine R-factor difference below 0.00005, and each engine parameter difference below 5% of the smaller uncertainty estimate. The largest Ni ΔE₀ difference is about 0.0069 eV versus an uncertainty of about 0.40 eV. These are useful first-shell checks, not a validation of every FEFF input or a complete scientific analysis of each foil. The curated Ni CIF has a = 3.5451 Å; its fitted negative ΔR must be interpreted relative to that starting structure.

The [machine-readable comparison](validation/metal-foil-backend-comparison.json) records provenance, engine versions, geometry, ranges, solver reports, parameters and acceptance criteria. Reproduce with:

```sh
cargo test --release -p rexafs-gui metal_foils_fit_with_both_backends -- --nocapture
```

The Ni measurement is an unmodified XDI file. [XDI support](xdi-import.md) now reads the official format, metadata, declared columns, original units and precomputed μ correctly. Folder discovery includes `.xdi`; keV and supported monochromator angles convert to eV. Malformed tables produce errors. Native release checks verified the [import preview](validation/xdi-ni-import.jpg), reopening the project with its ranges intact, and [Run fit → Results](validation/ni-foil-release-fit.jpg), yielding R = 0.00287 in nine iterations. Opening an `.rxs` from the command line now uses the project loader.

## VESTA polyhedra and Mercury-style molecule inspection

The implementation follows the visual conventions documented in [VESTA's display styles](https://jp-minerals.org/vesta/en/doc/VESTAch5.html) and [object properties](https://jp-minerals.org/vesta/en/doc/VESTAch12.html). The official [quartz polyhedron example](https://jp-minerals.org/vesta/en/doc/images/quartz-3.png) was inspected directly: filled shaded faces, outlined edges and visible center atoms make the coordination network readable.

- Polyhedra repeat around the selected element throughout the FEFF cluster. Selected center isolates one coordination environment. Periodic context supplies complete ligands at the cluster boundary.
- Convex faces merge coplanar triangles into polygons, receive orientation-dependent shading, and have crisp outlines. Blue faces are the default; element colors are optional. Opacity, edges and center/all/no atom display can be changed independently.
- Auto neighbors use a nearby unlike-element shell when plausible, otherwise the nearest shell, with a 25% distance tolerance. Element and absolute bond-distance controls allow correction. Fewer than three or more than 24 neighbors do not silently generate misleading geometry. These connectivity rules are visualization heuristics.
- The native rutile check has 69 Ti centers and complete TiO₆ octahedra at an 8 Å cutoff. The geometry regression verifies six oxygen vertices and eight triangular faces per Ti, including boundary centers. Shell counts are now an optional histogram so the structure gets the main display area.

For molecules, the references were [Mercury's official overview](https://www.ccdc.cam.ac.uk/solutions/software/mercury/), the [CCDC crystal API description](https://downloads.ccdc.cam.ac.uk/documentation/API/descriptive_docs/crystal.html), and the [official Mercury workshop](https://info.ccdc.cam.ac.uk/hubfs/CCDC_Workshop_Mercury_Exercises-1.pdf?hsLang=en), especially asymmetric-unit versus complete-molecule display, packing, ball-and-stick styles and hydrogen visibility. Mercury itself was not installed or exercised live.

Complete molecule reconstructs a finite bonded component across periodic cell boundaries. It displays CIF hydrogen positions, optionally labels atoms, and uses shaded balls and element-colored half-bonds. It does not change the spherical FEFF cluster. Extended periodic networks produce a clear message and retain the cluster view. Bonds use covalent-radius connectivity; bond orders, aromatic rendering and hydrogen-bond analysis are not assigned.

The offline library now contains 62 structures, including a Molecules filter with pinned neutron-diffraction examples: [Urea, COD 9017316](https://www.crystallography.net/cod/9017316.html), and [Aspirin, COD 7050899](https://www.crystallography.net/cod/7050899.html). Every crystallographic starting site reconstructs Urea to 8 atoms (4 H, 7 bonds) and Aspirin to 21 atoms (8 H, 21 bonds). Tests reject extended Cu and rutile networks. The catalog refresh script retains the pinned molecular examples.

Rendering remains a native GPUI canvas with depth-sorted primitives and inexpensive shading approximations. It is not VESTA's rendering engine or a GPU depth-buffered molecular renderer. The optional ruviz feature request remains separate.

The subsequent [slice and depth controls](structure-depth-view.md) add movable slabs, foreground cutaways, fixed Cartesian or camera-following normals, overall opacity, near/far fading, and radial fading around the selected coordination center. They clip displayed geometry and preserve all calculation inputs.

The complete release GUI suite passes 80 tests, with two local diagnostics ignored; the core XDI suite passes five tests. Geometry and interaction tests cover complete molecular connectivity, rutile polyhedra, camera rotation, wheel zoom and click/drag separation.

Final native checks used the rebuilt release executable after the layout polish. [Translucent rutile polyhedra](validation/vesta-polyhedra-rutile.jpg) and [opaque faces](validation/vesta-polyhedra-rutile-solid.jpg) were inspected at 144% zoom with the crystal context, repeated centers and isolated-center controls. [Urea](validation/molecule-urea.jpg) and [Aspirin](validation/molecule-aspirin.jpg) open directly from the Molecules filter with atom labels and CIF hydrogens. Dragging Urea and Aspirin changes orientation while retaining 100% zoom and does not open an atom card. Hiding Urea's hydrogens changes the display from 8 atoms / 7 bonds to 4 atoms / 3 bonds; its 108-atom, 8 Å FEFF cluster remains unchanged. Shell counts can be expanded and collapsed. The final geometry and molecular regression subsets pass after the polish, and `git diff --check` is clean.

## Follow-up usability changes — 2026-09-06

- The curated catalog defaults to **Contains [element]**, using `Element.symbol`
  and `Element.edge` from XDI, otherwise an explicitly estimated nearest edge to
  the selected spectrum's E₀ (within 100 eV). Tabulated edges come from the embedded
  `xraydb` 0.4.1 database. **All materials**, category filters, and CIF/XYZ import
  remain available. Unknown edges show the full catalog. Old preview metadata is
  cleared during file changes; processed E₀ is used only for the current file.
  Selecting a structure containing the spectrum's element defaults its absorber
  accordingly, without changing an existing calculated model merely by browsing.
- **Im χ(R)** is available beside Re in Model & fit and Results, for data previews
  and fitted curves. Path selection has **Geometry / FEFF curves** views, retaining
  the selector alongside the plot. Curves include effective amplitude, raw
  scattering magnitude, total phase, mean free path and reference k²χ(k).
  Effective amplitude is magnitude times reduction factor; total phase is the
  sum of central and scattering phases, as used by the core FEFF path parser.
  Reference χ uses a dense grid, FEFF degeneracy, S₀²=1 and zero shifts/disorder;
  it is labeled separately from fitted contributions. Inspecting a row never
  changes its inclusion checkbox.
- Result and History include R_eff, fitted R_eff+ΔR, ΔR, σ², S₀², ΔE₀, degeneracy,
  and leg count. Multiple-scattering distance is explicitly the half-path length.
  Uncertainties propagate the full fit covariance through expressions; missing
  covariance is “unavailable,” while constants are fixed. Snapshots are serialized
  with history so distances do not depend on reopening the path file.
- [Joint fitting](joint-fitting.md) provides explicit spectrum/path assignments,
  Shared / Per spectrum parameter scopes, and per-spectrum result inspection.

The reference for path diagnostics is the [Larch FEFF-path documentation](https://xraypy.github.io/xraylarch/xafs_feffpaths.html);
computed arrays follow the repository's FEFF equations (the documented amplitude
addition in one upstream paragraph is inconsistent with the actual product).
See [fit and history](validation/fit-imaginary-history.jpg) and
[FEFF amplitude inspection](validation/path-feff-amplitude.jpg).

### Ruviz issue status

[Issue #182](https://github.com/Ameyanagi/ruviz/issues/182) was closed by merged
[PR #183](https://github.com/Ameyanagi/ruviz/pull/183) on 2026-09-05. It provides the
first milestone for molecular spheres, surface depth, picking and stable scale.
Translucent faces and cylinder bonds remain outside that milestone. The current
molecular canvas uses GPUI directly and does not require this change; scientific
plots use ruviz 0.12.2. Migrating the molecular viewer is a separate integration,
not part of these usability changes. No additional upstream issue was filed.

The final multi-spectrum browser and per-spectrum ranges are documented in
[joint-fitting.md](joint-fitting.md). Native checks also confirm
[Ni filtering from XDI metadata](validation/library-nickel-filter.jpg) and the
[dense reference path curve](validation/path-reference-chi.jpg).
