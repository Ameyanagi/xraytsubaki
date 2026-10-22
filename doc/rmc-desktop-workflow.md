# Desktop RMC workflow (0.2.11)

For the 0.2.12 adaptive cache, Structural evolution view, and genetic/hybrid
controls, see [the follow-up guide](rmc-structural-evolution.md). The versioned
sections below describe their original 0.2.11 behavior.

## Setup and range corrections in 0.2.11

The next action stays at the upper right, matching ordinary path fitting:
**Use structure →**, **Next: fit settings →**, **Run RMC →**, then **Edit settings →**.
**Preview initial fit** sits beside Run RMC. Validation messages remain beside
these actions, above the scrollable controls; advancing does not require scrolling
to a footer. Run controls still prevent duplicate jobs and reject invalid inputs.

Version 0.2.11 copies the spectrum's forward Transform k bounds, k weight,
window, `dk`/`dk high`, FFT length and spacing when initializing RMC or choosing
**Use spectrum ranges**. Explicit Back FT R bounds, window and widths are copied
as well. Without explicit R bounds, RMC starts at Rbkg + 0.15 Å and ends at
max(4 Å, Rbkg + 1.15 Å). The forward **R max out** is a plotting extent, not a fit
bound. RMC still rejects R minimum below Rbkg and R maximum above its supported
10 Å output extent. Fit transforms retain the native fitting convention of a
uniform grid starting at zero; the processing plot's Input/Larch grid selector
does not select a different RMC objective.

The copied bounds remain editable. This action copies settings once; subsequent
processing edits require **Use spectrum ranges** again. It never changes fixed
S₀² or ΔE₀, and saved/running jobs retain their original transform and inputs.
Historical projects keep their original fit-window defaults until explicitly
reset. k bounds are clipped to measured support; they are no longer shortened to
fit the default ReFEFF calculation extent.

ReFEFF calculation coverage expands to include the measured Fourier taper and
the theoretical wave numbers needed after ΔE₀. The required upper bound is
rounded upward to a whole Å⁻¹, with a minimum of the configured calculator limit.
The rexafs adapter allows at most 30 Å⁻¹ and also checks the actual returned
scattering tables. It never extrapolates missing theory. The previous desktop
used a fixed 16 Å⁻¹ calculation limit and the fitting default `dk = 4` instead of
the spectrum's window; this could reject or shorten otherwise usable ranges.

Desktop preparation now reserves a context for every distinct combination of
structure, absorbing atom, edge and ReFEFF settings. All selected absorbers are
still averaged; none are silently sampled. This fixes the previous 128-context
ceiling for a 256-absorber cell. The one-million-path catalogue budget, per-site
search limits and 256 MiB numerical cache budget remain in force. The cache
budget does not bound total process memory: phase tensors and catalogues also
consume memory. Resource errors identify the required count and relevant limit.
Core API callers can set `AccelerationSettings.max_contexts` explicitly; its
default remains 128.

New desktop jobs also reuse electronic setup for matching local inputs. Scatterer
rows are sorted at the existing twelve-decimal FEFF input precision, retaining the
absorber first. Only complete, identical input cards and options share immutable
potentials and phase tables. Each absorber keeps its own atom identities, path
catalogue and coordinate updates. Different orientations, species, edges and
polarizations remain distinct; no approximate geometry matching or site sampling
is introduced. Larger disordered inputs may offer little reuse.

Rust callers enable this with `AccelerationSettings { reuse_electronic_inputs:
true, ..Default::default() }`. The core default remains false for historical
checkpoint compatibility. New desktop requests record true; old saved requests
keep false, their original ordering and the original 128-context minimum. Sorting
can change numerical summation and which atom represents a potential when nearest
sites tie. Bitwise agreement with the old ordering is not guaranteed; the selected
mode is part of calculator identity. Do not switch it during resume. **Run details**
reports the number of electronic preparations and shared contexts. See the
[startup profiling method](rmc-startup-profiling.md) for reproducible timing and
spectral-agreement checks; startup speed does not establish fit convergence.

These corrections were introduced in 0.2.11. Implementation:
[`Draft::use_spectrum_ranges`, `Request::new` and the worker settings](../crates/rexafs-gui/src/rmc_fitting.rs),
with [core resource checks](../crates/rexafs/src/xafs/rmc/accelerated.rs).

## CPU controls (0.2.11)

**Fit settings → CPU workers** selects the total thread budget. New drafts use
**Auto**, which detects available logical CPUs up to 64. Enter 1–64 to override
it, or clear the field to return to Auto. The adjacent label shows available CPUs.
Parallel execution first distributes absorbing sites. **Advanced settings →
Parallel paths** lets spare workers evaluate paths within an absorber, using the
same bounded pool. It is enabled for new drafts and also benefits single-site jobs.

Older drafts keep one worker and their historical path scheduling until explicitly
changed. Existing saved/running jobs retain their captured settings. **Run details**
shows the actual saved worker count, path fallback and backend preparation threads.
Checkpoint and exported job settings retain these choices; form edits apply to a
new run. Backend electronic preparation remains serial across contexts, and the
backend's internal thread setting remains one by default. See the
[core parallelism guide](rmc.md#unreleased-absorber-first-cpu-parallelism) for the
scope, deterministic reductions and resource limits.

<a id="unreleased-fitting-controls"></a>

## Fitting controls (0.2.11)

New jobs start with **Auto moves** and a 0.05 Å Cartesian move width. The width
adapts within 0.1–2 times its starting value, freezes after 80% of the original
attempt budget, and the numerical Metropolis tolerance cools linearly to zero.
**Fixed moves** retains manual control. Existing checkpoints and older saved
drafts keep their historical policy. These are starting heuristics, not an
optimal step size for every material. Continuing a run does not restart cooling.

**Estimate calibration…** searches theoretical ΔE₀ and bounded S₀² with the
starting geometry held fixed. Inspect the estimate and any bound warning, then
choose **Use calibration** explicitly. Changed inputs invalidate the preview.
Advanced settings exposes the search interval (default −15…+15 eV in 0.5 eV
increments) and amplitude bounds (default 0.5…1.2). A poor structural reference
can bias either value. S₀² remains fixed; ΔE₀ stays fixed unless refinement is selected.

**ΔE₀: Fixed / Refine** selects optional energy refinement for a new run. **Fixed**
is the default and preserves existing projects. **Refine** keeps S₀² fixed,
searches ΔE₀ before the first coordinate move and then updates it periodically.
**Advanced → Energy refinement** exposes inclusive bounds in eV and the update
interval (250 attempts initially). The initial grid uses 0.5 eV spacing; subsequent
local grids cover ±1 eV at 0.1 eV spacing. These are numerical settings, not
uncertainties. Bounds must preserve the full Fourier taper. The measured spectrum's
energy alignment and normalization E₀ are unchanged.

Results display the best structure's matching ΔE₀ and fixed S₀², with a warning if
the shift touches a bound. Only decreases verified by the full objective are kept.
Each state, energy-update record and sampled trajectory retains its own shifts;
failed searches leave the checkpoint and random sequence unchanged. **Export
result…** includes `fit-parameters.json` with initial/best shifts, fixed amplitudes
and the refinement policy, matching the exported curves and structures. Full
checkpoints preserve all current/best values for exact continuation.

After pausing or finishing, **Refine best…** performs a bounded local search using
numerical coordinate derivatives of the full configured scattering calculator.
Reference electronic potentials, calibration and mixture fractions remain fixed.
All trials obey the original displacement and distance constraints; only fully
verified decreases are accepted. Defaults are three passes, 0.001 Å derivative
probes, at most 0.02 Å trial moves and 5,000 geometry evaluations. This work can
be cancelled and does not change the RMC checkpoint or its random sequence.
**Refinement** compares experiment, the previous best and the refined result.
**Export result…** adds `local-refinement.json`, `refined.xyz` and
`refined-fit-k.csv` alongside the original run. The JSON retains the constraints,
calibration, preprocessing, coordinates, arrays and accepted-step history.

Local descent is not proof of structural uniqueness or convergence. The
[refinement design and Rust API](rmc-refinement-plan.md) explain the policy,
scientific assumptions and validation. Local refinement uses numerical derivatives
and requires no automatic-differentiation toolchain.

## Released workflow

The first desktop implementation adds **Fit mode: Path fitting / RMC** at the
upper right of Fitting (renamed **Method** after 0.2.11). It supports one processed spectrum and one explicit
periodic structure. The numerical engine is the existing `RmcSession`; ordinary
path fitting remains the default for older projects. This workflow is introduced in 0.2.10; it is absent from 0.2.9.

## Prepare and run

1. Import and process a spectrum through Background. RMC captures the processed
   `Spectrum` with `RmcDataset::from_spectrum`, including its preprocessing state,
   source identity and the desktop processing recipe. It does not run AUTOBK again.
2. Open Fitting, select **RMC**, choose a structure from the existing library or
   structure import tools, and press **Use structure**.
3. In **Supercell**, edit the supercell repeat counts. The 3D preview rebuilds in a
   background task after a 150 ms typing delay, without scattering calculations.
   Obsolete preview tasks cannot overwrite newer input. Invalid edits retain the
   last valid preview and block preparation/run. **Use suggested size** derives
   repeat counts from twice the cluster radius plus displacement margin. The core
   rejects allocations above 10,000 atoms. The preview shows coordinates, atom/
   absorber counts and periodic cell dimensions. Atom indices are
   zero based; fixed atom `0` is the default anchor. Blank absorber indices select
   every atom of the chosen element. Mixed/partial occupancy requires an explicit
   fully occupied realization and is rejected by the core.
4. Check cluster/path radii, maximum path legs, displacement envelope and minimum
   atom distance. Exact cached ReFEFF uses zero amplitude cutoffs and a fixed
   reference electronic potential. A changed geometry receives exact typed-path
   calculations within that reference model. Adaptive approximation is experimental
   and is not exposed by these desktop controls.
5. In **Fit settings**, inspect S₀², fit ΔE₀, k/R ranges, integer k weight, move size,
   numerical Metropolis tolerance, seed and attempt budget. Pair constraints use
   entries such as `Cu-O=1.5, Cu-Cu=2.0`, in Å. **Preview initial fit** to inspect
   its curves before starting moves, or select **Run RMC** directly.

On first entering RMC with a processed spectrum, starting fit ranges copy its
Transform settings, including explicit Back FT bounds, as described above.
Without an explicit lower R bound, R minimum starts at saved Rbkg + 0.15 Å.
**Use spectrum ranges** copies these settings again without changing S₀² or ΔE₀.
The numbered pages guide Structure → Supercell → Fit settings → Results.
The Supercell page offers **Next: fit settings** at the upper right; Fit settings
keeps preview/run actions above its scrolling form. Advanced controls are
collapsed initially. Field-specific input errors and missing prerequisites appear
next to the run actions. Structure choices support keyboard activation and expose
their material names to assistive technology.

New 0.2.11 jobs use 10,000 attempts, Auto moves starting at 0.05 Å, numerical
tolerance 0.001, a 0.2 Å displacement envelope, a 1 Å minimum distance and a fixed
anchor at atom 0. These are starting values; calibration, constraints, cell size
and convergence need sample-specific assessment. Larger suggested cells can
substantially increase computation cost. Historically, 0.2.10 started with fixed
0.03 Å moves and placed run actions below the form. Older saved runs retain their
captured move policy.

The initial desktop objective is the native R-space **real plus imaginary**
residual, normalized by experimental power, with any configured structural penalty
added separately. The k/R magnitude plots are diagnostics. R minimum must be at
least the saved AUTOBK Rbkg: the adapter rejects an incompatible range rather than
quietly changing it. One integer k weight from 0 through 3 is supported. Automatic
noise estimation and multiple k weights are not connected here. S₀² remains
fixed. ΔE₀ stays fixed during each coordinate move and can be updated between
moves when **Refine** is selected; each saved state's value matches its curves.

## Live results, pause and recovery

Run opens Results. A dedicated background thread owns the calculator and optimizer.
The UI checks its bounded event queue every 250 ms; completed-state updates are
produced approximately every 500 ms. A single long scattering evaluation can delay
an update. Results include experimental/initial/best curves in k, R magnitude,
R real and R imaginary; the best configuration and its cell; current/best objective
history; acceptance, hard-constraint rejections, elapsed time and active-path reuse.
The default plot view shows k and R magnitude at full panel height. A second
view shows R real and imaginary, and an optional grid shows all four plots.
A labelled color legend identifies experimental, initial and best traces.
**Hide initial** rescales the view for comparing experiment with the best fit;
this changes only the display. Ordinary live updates preserve manual zoom/pan.
**Run details** holds timing, cache definitions, objective components and recovery
provenance. Compact cards keep best objective, improvement and convergence visible.
The live bar separates preparation, running, pausing and stopping, shows progress
through the attempt budget, and disables duplicate pending control actions.
Unchanged best curves reuse their existing plots while the residual history updates;
this avoids repeating Fourier transforms for every rejected move.
Best curves and coordinates come from the same saved state. Metropolis may accept
a worse current state, so the final current state is not necessarily the best fit.

**Pause** finishes the current move, saves a checkpoint, and retains the prepared
calculator for a fast warm resume. **Stop and save** requests calculator
cancellation and saves the last complete transactional state. A kernel that does
not inspect cancellation immediately may delay stopping. Wait for the stopped or
paused state before closing the application if the latest state is important.
When the run stops or finishes, messages that applied only to the active run (for
example, a request to stop it before starting another) are cleared, and the
window footer reports the RMC outcome and completed attempts or generations
instead of an earlier, unrelated message such as a path-fit R-factor.

Checkpoints are written atomically after preparation, about every 30 seconds at
move boundaries, and on pause, stop, budget completion or a calculation failure.
They live in `~/.rexafs/rmc/run-*/checkpoint.json` and contain current, initial and
best states, RNG state, frozen inputs and settings. An unexpected application exit
can lose work since the last completed checkpoint. **Recover latest run** discovers
the newest recovery file; **Open checkpoint…** selects a particular run. Opening
loads results without automatically launching a calculation. **Resume saved run**
rebuilds the same exact calculator and resumes its saved sequence and unfinished
budget. Completing the budget exposes an explicit **Additional attempts** field
(default 10,000) and **Continue optimization**. This field changes only the total
attempt limit; it does not reuse edits made to a new-run draft.

Changing the active spectrum, processing or draft settings does not retarget a
saved/running job. Resume always uses the saved problem. Stop the existing worker
before starting a new problem or a path fit. An input mismatch is shown above the
results. Paused workers still own their calculator and must be stopped before a
second fitting job starts.

Saving an `.rxs` project embeds the latest completed checkpoint with the RMC draft
and selected mode. This first implementation retains one run in the project;
recovery files remain independently available. Large trajectory collections and
linked checkpoint archives are later work. Existing project size limits apply.

## What convergence means

The desktop calls the shared `residual_trend` diagnostic. It requires at least
3,000 attempts, compares nonoverlapping 500-attempt windows and requires three
consecutive passing comparisons. For each comparison:

- Best-score improvement must be at most `1e-5 + 0.005 × |previous best|`.
- Absolute mean-score change must be at most `1e-5 + 0.01 × |previous mean|`.

The display distinguishes insufficient history, still changing and residual
plateau. Its default recent-trend plot shows the latest four diagnostic windows
(2,000 attempts), so a large early improvement does not hide later changes.
**All retained attempts** shows the available history, up to 10,000 records.
Changing this view does not change the convergence calculation. This is an empirical numerical criterion, not proof of a unique or
physically complete structure. It does not automatically stop this desktop run.
Reaching the attempt limit is reported separately. Short software checks are not
final scientific refinements. History retains the most recent 10,000 attempts.

Elapsed time includes setup and active execution but excludes time paused; the
reported seconds per attempt is inclusive and is not an isolated scattering
benchmark. Cache reuse is `reused active paths / (reused active paths + exact path
calculations)` for the current calculator, which resets after a cold resume.

## Export and Transform wavelets

Pause or finish before **Export result…**. A new result folder contains the full
checkpoint, initial/best XYZ, configuration JSON preserving periodic cells,
experimental/initial/best k and R CSV files, convergence settings/report and a
Markdown summary. Ordinary XYZ alone does not preserve a periodic cell.

Wavelet display is under **Transform → Wavelet**. After integration with the
current `dev` branch, the desktop uses its retained Cauchy-wavelet workspace,
including linked plots, region measurements and saved map history; see
[Wavelet analysis](wavelet-analysis.md). This display does not change the RMC
objective. The RMC-compatible Morlet and Gaussian STFT implementation remains in
`rexafs::transform::LocalSpectrumTransform`, with direct and FFT evaluation.
These are distinct wavelet conventions; no numerical equivalence is implied.
The earlier Morlet desktop preview is superseded by the Cauchy workspace.

## Scope and implementation

Hybrid EA–RMC, multiple weighted structures, joint datasets, automatic copying of
path-fit calibration, advanced proposal/annealing controls, structural distributions
and path-report inspection remain desktop follow-up work. The underlying Rust
engine already supports many of these. Do not read the two-option desktop selector
as implying an EA calculation.

Implementation and regression evidence are in:

- [Desktop adapter and worker](../crates/rexafs-gui/src/rmc_fitting.rs).
- [RMC workspace](../crates/rexafs-gui/src/app/shell/rmc.rs).
- [Wavelet view adapter](../crates/rexafs-gui/src/app/shell/wavelet.rs).
- [Shared transform](../crates/rexafs/src/xafs/transform/local_spectrum.rs).
- [Transform compatibility tests](../crates/rexafs/tests/rmc_transforms.rs).
- [Original plan and later slices](rmc-desktop-workflow-plan.md).

Automated worker tests use a synthetic Cu–O dimer to verify frozen Spectrum
inputs, prepare/pause, exact cold continuation against an uninterrupted native
session, project round trips, invalid checkpoint rejection, live progress, stop
recovery and exports. This is software verification, not a Cu₂O fit-quality claim.

## Local verification, 18 September 2026

- Default core suite: 405 passed, 3 ignored; strict core Clippy passed.
- Desktop release suite: 562 passed, 6 ignored, including live worker/stop,
  exact continuation, Spectrum snapshots, default ranges and bounded Cu₂O
  supercell previews.
- Stable and Next Rust API references regenerated successfully.
- Desktop compilation without optional calculation backends also passed.
- A release desktop build was launched with a copy of `Cu oxides.prj`.
- The computer-use connector was unavailable. Native macOS accessibility and
  window capture were used instead for the desktop interaction review; see the
  updated verification record below.

The desktop guide, project defaults and live builder are development work;
no new converged Cu₂O refinement is claimed by these software checks.

### Issues found during the native interaction review

The review found two scientific display/setup issues in the initial desktop
implementation. Selecting a full `dk` margin requested unnecessary low-k points
below the valid threshold for a positive ΔE₀. The desktop now selects the native
window support (`dk/2` on each side) plus one interpolation point, and validates
the shifted grid before enabling a run. A larger shift produces an actionable
minimum-k message rather than failing after submission. The starting fit-range
suggestions retain their conservative full-width margin.

R-space plots and CSV exports now call
`rexafs::rmc::transform_spectrum_fourier`, which shares the RMC objective
interpolation and the existing fitting transform. k weighting occurs before
interpolation, exactly as in the objective. This corrects the original direct
transform call for a selected grid beginning above zero; the corresponding path
Fourier helper also uses this mapping. A regression compares the displayed
complex residual against the objective on an offset, irregular grid for all
four supported k weights. The numerical optimizer and historical checkpoints
are unchanged; corrected displays can differ from earlier desktop previews.

The periodic viewer now frames the entire unit-cell box, clips drawing to its
viewport, removes absorber-centred cluster guides, and provides a reset action
and an atom legend. These changes affect presentation only.

### Native Cu₂O workflow qualification

The release app was driven through macOS accessibility using a copy of the
`cu2o_abs` group from `Cu oxides.prj`. Real window captures are available in the
[desktop gallery](https://cu2o-rmc-results.ameyanagi.chatgpt.site/desktop.html).
The interaction review covered structure selection, live 48/162-atom previews,
Rbkg validation, initial preparation, budget continuation, live k/R/complex plots,
pause at 1,023 attempts, warm resume, stop at 1,476, recovery in a fresh app,
and cold continuation. It also checked the 1,280 × 800 light-theme layout.

The verification run was stopped and saved at **5,396 / 10,100 attempts**, with
best objective **0.03488986888294703** and diagnosis **StillChanging**. Its newest
500-attempt window improved the best residual by about 6.02%, so it does not meet
the plateau criterion. A fresh app recovered the same state; native result export
and a project save with embedded source data both succeeded. The saved project
and export contain the same complete checkpoint.

This run used a 2 × 2 × 2 Cu₂O cell (48 atoms, 32 Cu absorbers), exact cached
ReFEFF, k = 4.0–10.4 Å⁻¹, R = 1.15–4.0 Å, k weight 2 and the native Kaiser–Bessel
window with dk = 4.0 Å⁻¹. S₀² = 0.9840213777547031 and
ΔE₀ = 8.762386660183811 eV were fixed during moves. Its objective/window differ
from the archived scientific comparison, so the scores are not directly
comparable. This is a native workflow qualification, not a converged refinement.

### Integration with current dev

Before merging PR 89, the branch was integrated with PR 88. Its retained Cauchy
wavelet workspace, fluorescence/normalization tools and Live acquisition remain
available. The earlier standalone Morlet desktop adapter was removed; its shared
Rust numerical API is unchanged. Numeric fields retain both live-preview events
and the newer precision-preserving display formatting. The native screenshots
above document the pre-integration RMC review; automated checks also cover the
combined branch.

## Interpreting an improving but poor fit

The percentage reduction compares the current best objective with the initial
objective; it is not the percentage of the experimental signal explained. A very
poor initial model can improve by 99% and still have a large residual. Inspect
the best objective, complex-R curves and residual trend together. The desktop
now labels this metric **Reduction from initial**.

The setup and result views flag an R fit upper bound beyond the scattering-path
radius. Review the path catalogue before interpreting outer-shell discrepancies.
Fourier R includes scattering phase shifts and is not an exact bond distance, so
absence of this message does not establish adequate model coverage. Increasing
the path radius can substantially increase calculation cost.

S₀² and theoretical ΔE₀ remain fixed during coordinate-only RMC. Calibrate them
with an appropriate reference or a justified path fit before structural
refinement; the core calibration workflows are described in
[the search guide](rmc-search-upgrade.md#four-parameter-first-shell-calibration).
A completed attempt budget or numerical plateau does not establish physical
accuracy or a unique structure. These messages are display changes; existing
runs retain their captured settings and numerical objective.
