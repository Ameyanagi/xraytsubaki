---
title: "RMC structure refinement"
description: "Refine atomic coordinates with exact cached ReFEFF, inspect live fits, and resume saved runs."
audience: user
---

In **rexafs 0.2.14**, choose **Fit → Method: RMC** at the upper right of the
fitting workspace. Reverse Monte Carlo (RMC) proposes random coordinate moves,
calculates their spectra and accepts or rejects them against the measured data
and configured constraints. It refines a periodic structure rather than the
path parameters used by ordinary EXAFS fitting.

The desktop uses one processed spectrum and one starting structure, with ordinary
RMC, genetic and hybrid search. The [Rust API](/api/rust/rexafs/xafs/rmc/index.html)
also supports weighted structures and joint datasets. RMC is not exposed by the
Python or TypeScript bindings. Resource controls and structural history are
described [below](#new-in-0212).

## Prepare the spectrum and structure

1. Import the measurement and inspect its normalization and background
   (AUTOBK) result in **Process**. The RMC input captures the processed spectrum and its preprocessing
   information, including AUTOBK Rbkg. It does not rerun AUTOBK for each move.
2. Select **RMC**, choose a fully occupied periodic structure and press
   **Use structure**. Resolve mixed or partial occupancy before refinement.
3. In **Supercell**, edit repeat counts and inspect the live preview. **Use
   suggested size** accounts for cluster radius and the allowed displacement.
   Larger cells cost more to calculate. Invalid edits preserve the last valid
   preview and block a new run; preview updates do not calculate scattering.
4. Check absorber, edge, cluster/path radii, maximum path legs and constraints.
   Blank absorber indices use all atoms of the selected element. Fixed atom
   indices are zero based. Pair minimum distances accept entries such as
   `Cu-O=1.5, Cu-Cu=2.0`, in Å.
5. In **Fit settings**, inspect calibration, k/R ranges, k weight and the
   attempt budget. Select **Preview initial fit** and compare its curves with
   experiment before choosing **Run RMC**.

New jobs start with 10,000 attempts, automatic moves beginning at 0.05 Å,
numerical Metropolis tolerance 0.001, a 0.2 Å displacement envelope, 1 Å global
minimum distance and atom 0 as an anchor. These are editable starting values,
not a physical model or a convergence guarantee. S₀² stays fixed during RMC;
ΔE₀ can remain fixed or be refined periodically. Calibrate both for the sample.
The Metropolis tolerance controls acceptance of worse trial scores; it is not a
measured thermodynamic temperature. Older saved runs retain their settings.

The signed-release example below uses public room-temperature Cu data,
32 periodic Cu sites, k = 2–12 Å⁻¹ and R = 1.5–3.5 Å. Auto resolves to ten
workers on the capture machine. Its short run tests the workflow; it does not
establish a calibrated structure.

## Fit ranges and the background

The next action stays at the upper right on each RMC page, matching path fitting.
Fit settings places **Preview initial fit** beside **Run RMC →**.

New jobs copy the Process › Transform windows, widths and forward sampling as well as k bounds
and weight. Explicit Back FT R bounds are used; otherwise the starting R range
is Rbkg + 0.15 Å to max(4 Å, Rbkg + 1.15 Å). **Use spectrum ranges** copies these
settings again after processing edits. It does not retarget a saved run. ReFEFF
coverage expands for the fit window and ΔE₀, up to the adapter's 30 Å⁻¹ limit,
with actual returned support checked. Missing theory is rejected rather than
extrapolated. The [workflow guide](https://github.com/Ameyanagi/rexafs/blob/v0.2.14/doc/rmc-desktop-workflow.md)
explains these settings.

The default objective minimizes the normalized sum of squared **real and
imaginary R-space residuals**, using the same Fourier mapping as native path
fitting. The magnitude plot is a diagnostic; matching magnitude alone is not
the objective. Configured structural penalties are added separately.

Starting k limits follow the spectrum's Process › Transform settings within valid measured
and calculator support, including the window. Without an explicit Back FT lower
bound, R minimum starts **0.15 Å above the saved AUTOBK Rbkg**. The desktop
rejects R minimum below Rbkg, where background removal can produce artifacts.
**Use spectrum ranges** reapplies the suggested
ranges without changing S₀² or ΔE₀. One integer k weight from 0 through 3 is
supported; automatic noise estimation and multiple k weights are not connected.

Exact cached **ReFEFF 0.4.0** is the default. Changed geometry receives exact
path calculations within fixed reference electronic potentials; unchanged paths
can be reused. This does not recalculate the electronic potential after every
move. Adaptive scattering remains experimental, opt-in in Rust, and absent from
these desktop controls.

The desktop allocates a prepared context for every selected absorbing site,
edge and setting. This removes the previous 128-context default limit for larger
cells without discarding absorbers. Independent path-count and memory-related
limits still apply, and larger cells cost more.

New runs also share electronic preparation for identical local inputs after
deterministic ordering of the scatterer rows. All absorbing sites
and their explicit paths are retained. **Run details** shows calculated and shared
electronic contexts. Sorting can change numerical summation and the representative
atom for a potential when nearest sites tie. Old checkpoints retain their original
ordering; restarting a new job opts into the improved preparation. See the
[startup profiling method](https://github.com/Ameyanagi/rexafs/blob/v0.2.14/doc/rmc-startup-profiling.md)
for reproducible timing and numerical-agreement checks.


<span id="cpu-workers-in-the-source-checkout"></span>

## CPU workers

**Fit settings → CPU workers** starts at **Auto** for
new jobs, using available logical CPUs up to 64. Enter a count from 1 to 64 to
override it, or clear the field to restore Auto. Independent absorbers run first.
**Advanced settings → Parallel paths**, enabled for new jobs, uses spare workers
for paths within an absorber. Both levels share one thread pool.

Older projects keep their existing scheduling. **Run details** shows the captured
worker count; saved jobs resume with that count. Changing the form affects a new
run. More workers may use more temporary memory and are not always faster.

<span id="energy-refinement-in-the-source-checkout"></span>

[![Saved RMC cache details showing 32 snapshots, 320 hits, 32 cold misses and no repeat misses](/screenshots/0.2.12/rmc-run-details.jpg)](/screenshots/0.2.12/rmc-run-details.jpg)

*Signed 0.2.12 release: all 32 absorber snapshots were retained, with no repeated
misses or evictions. One electronic preparation was shared with 31 other contexts.
The ten-attempt check used ten workers; its timings are not a performance benchmark.*

## Energy refinement

In **Fit settings**, choose **ΔE₀ → Refine** to update
the theoretical energy shift while keeping S₀² fixed. **Fixed** remains the default,
including for older projects. Set S₀² from an appropriate reference calibration.

A new run searches the starting shift before moving atoms. Further local searches
run every 250 attempts by default. Change the interval and energy bounds under
**Advanced → Energy refinement**. Bounds are in eV; the default initial search
uses 0.5 eV spacing and local searches use ±1 eV at 0.1 eV spacing. The grid spacing
is resolution, not uncertainty. Only a full-objective improvement is accepted.

Results show ΔE₀ for the best structure and warn when it reaches a bound. Saved
runs keep each structure's matching shift; exports include these values in
`fit-parameters.json` and the report. Experimental energy alignment and spectrum
preprocessing stay unchanged. This is alternating structural/energy optimization;
a small residual alone does not establish a unique structure.

<span id="calibration-and-local-refinement-in-the-source-checkout"></span>

## Calibration and local refinement

New jobs offer **Auto moves** (starting at 0.05 Å) and **Fixed moves**. Auto adjusts
the width during the early part of the original budget and cools the numerical
tolerance to zero. Existing saved runs keep their settings.

**Estimate calibration…** previews amplitude S₀² and theoretical energy shift
ΔE₀ using the starting structure. Review the bounds and choose **Use calibration**
explicitly; changes to the inputs require a new estimate. A suitable experimental
reference is needed to interpret these correlated parameters physically.

After pausing or finishing, choose **Refine best…** to try local coordinate
improvements. This uses numerical gradients, with full scattering verification
and the original constraints. It preserves the RMC checkpoint. **Refinement**
compares the previous and refined spectra; **Export result…** includes both,
refined XYZ coordinates and a separate JSON audit. A smaller residual does not
establish a unique structure. Local refinement uses numerical derivatives.

## Read the live results

Results show experiment, initial calculation and best calculation in k space,
R magnitude, R real and R imaginary. The best coordinates and curves describe
the same saved state. The current state can be worse than the best because the
Metropolis rule can accept uphill moves. **Hide initial** changes the display
scale without changing the objective.

The statistics separate best score, improvement, acceptance, hard-constraint
rejections and residual diagnosis. **Run details** includes timing, cache
statistics and saved-input provenance. Updates arrive between completed
calculations; one long scattering evaluation can delay the display or a pause.

[![Reopened ten-attempt Cu RMC result with fixed amplitude and energy and convergence not assessed](/screenshots/0.2.12/rmc-results.jpg)](/screenshots/0.2.12/rmc-results.jpg)

*Signed 0.2.12 result reopened from its saved project. This ten-attempt check kept
S₀² = 1 and ΔE₀ = 0 eV fixed. The objective fell from 6.4641 to 5.3341, but amplitude
mismatch remains and convergence is not assessed. It is a software check, not a
calibrated scientific fit. [Capture and input provenance](/licenses/#desktop-0212-release-captures).*

## Pause, save and resume

**Pause** finishes the current move, saves a checkpoint and retains the prepared
calculator for a fast resume. **Stop and save** cancels work and retains the
last complete state. Wait for the paused or stopped status before closing when
the latest state is needed. When a run stops or finishes, messages about the
active run are cleared and the window footer reports the RMC outcome and the
number of completed attempts or generations, replacing any earlier message such
as a path-fit R-factor.

Checkpoints are saved after preparation, approximately every 30 seconds at move
boundaries, and on pause, stop, completion or calculation failure. A sudden exit
can lose moves since the preceding checkpoint. **Recover latest run** opens the
latest recovery file; **Open checkpoint…** selects another. Loading results does
not start calculation automatically. **Resume saved run** restores the saved
inputs, settings and random state and rebuilds the calculator when necessary.

Saving an `.rxs` project embeds the latest complete checkpoint and RMC setup.
Recovery files also remain under `~/.rexafs/rmc/run-*/checkpoint.json`. The project
currently retains one RMC run. After the budget finishes, enter **Additional
attempts** and choose **Continue optimization**; the starting increment is 10,000.

Editing preprocessing, changing the active spectrum or editing a new-run draft
does not alter a saved run. Resume uses its original problem, including its
background settings. An input mismatch is shown above the results. To use new
preprocessing, stop the old worker and prepare a new run.

## Decide whether to continue

The budget is separate from convergence. The desktop's empirical diagnostic
requires at least **3,000 attempts**, compares nonoverlapping **500-attempt
windows**, and needs **three consecutive passing comparisons**:

- Best-score improvement ≤ `1e-5 + 0.005 × |previous best|`.
- Absolute mean-score change ≤ `1e-5 + 0.01 × |previous mean|`.

The result is **Insufficient history**, **Still changing** or **Residual plateau**.
The recent view shows 2,000 attempts; **All retained attempts** shows the retained
history. A plateau does not stop the run automatically or prove a unique,
physically complete structure. Inspect both spectral components, constraints and
structural distributions, and compare independent seeds when drawing conclusions.

The [workflow and implementation record](https://github.com/Ameyanagi/rexafs/blob/v0.2.14/doc/rmc-desktop-workflow.md)
explains persistence, numerical conventions and software checks. Its short Cu₂O
verification run remained **StillChanging**, not converged. The
[Rust guide](https://github.com/Ameyanagi/rexafs/blob/v0.2.14/doc/rmc.md) covers
Spectrum inputs and the broader RMC and evolutionary APIs.

<span id="new-in-the-0212-release-candidate"></span>

## New in 0.2.12

The selector is **Method: RMC**. Existing checkpoints keep their saved scientific
settings; editing the new-run form does not retarget them.

### Cache memory and large path catalogues

Start with **Fit settings → Cache memory (MiB) → Auto**. The desktop estimates
available physical memory, leaves headroom, and reassesses the scattering-cache
budget every two seconds. Enter a fixed MiB value to override it; clear the field
to restore Auto. One MiB is 1,048,576 bytes. The policy applies to new runs and
cold resumes; a paused worker keeps its current policy. It bounds retained
scattering snapshots, not total application memory. Electronic tables, path
catalogues and temporary worker results need additional memory.

Preparation reports path enumeration, electronic preparation and scattering.
**Run details** shows the budget, estimated snapshot requirement, hits, cold and
repeated misses, evictions and oversized snapshots. Cold misses are normal on
first use. Repeated misses mean previously calculated snapshots must be rebuilt.
The warning clears after a budget increase or 30 seconds without another repeated
miss. If a fixed limit is too small, try Auto or a larger limit when memory allows.
Neither a larger cache nor more workers guarantees faster initialization.

**Catalogue path limit** controls a different resource: the total retained
geometric paths. Auto chooses and saves a capacity once when a new run starts;
older runs retain their historical limit. The core API still defaults to one
million paths. Enter 1–100,000,000 to override the desktop capacity. Increasing
cache memory alone cannot clear a path-limit error.

When enumeration exceeds the guard, **Path limit…** opens the relevant settings.
The reported count is a lower bound found before stopping, not the final number
of paths. Increasing capacity leaves the modeled scattering unchanged. Reducing
path radius, maximum legs or absorbing sites changes the calculation. All
selected absorbers remain explicit; the program does not silently sample them.
The memory fractions and path-size allowance are empirical resource policies,
not allocation guarantees. See the
[policy and implementation](https://github.com/Ameyanagi/rexafs/blob/v0.2.14/doc/rmc-structural-evolution.md#cache-memory-and-responsive-preparation).

[![Genetic and hybrid search choices beside Auto memory and catalogue controls](/screenshots/0.2.12/rmc-search-memory.jpg)](/screenshots/0.2.12/rmc-search-memory.jpg)

*Signed 0.2.12 release, full unedited window. The public Cu example uses automatic
cache memory and catalogue capacity. All three search choices are visible; this
run uses ordinary RMC. [Capture provenance](/licenses/#desktop-0212-release-captures).*

### Structural evolution

Choose **Results → Structural evolution**, an element pair and a view:
**Distributions**, **Distance vs step**, **Coordination**, **Mean distance** or
**Variance**. Initial/current/best overlays share the same bins. The heatmap
records actual completed attempts or generations; blank gaps are unsampled,
with no interpolation. Steps are optimization progress, not physical time.

Before a new run, **Structural tracking** sets the radial interval and bins.
Defaults are [0, 8) Å, 160 bins, every 100 RMC attempts and at most 256 history
samples. Population searches sample each generation; completion and checkpoint
boundaries can add samples. A bounded background worker can skip intermediate
updates, and the view reports skipped and removed samples. Old checkpoints do
not acquire invented historical distributions.

For periodic cells, the displayed dimensionless pair distribution is
$g_{AB,i}=H_{AB,i}/(\rho_B V_i)$. Here $H_{AB,i}$ is the number of B neighbors per
selected A absorber in bin $i$, $\rho_B=N_B/V_{\mathrm{cell}}$ is the full-cell
species density in Å⁻³, and $V_i=4\pi(r_{i+1}^3-r_i^3)/3$ is the shell volume
in Å³. This uses center, density and shell normalization; see the
[GROMACS normalization reference](https://manual.gromacs.org/current/onlinehelp/gmx-rdf.html).
rexafs uses full-cell density, excludes the central self image, includes other
periodic images and applies no finite-N correction. Finite clusters have no
assumed bulk density and show **Neighbors per absorber per bin** instead.

Coordination sums raw neighbors over the chosen interval. Mean distance is in Å
and population variance in Å²; empty intervals have no mean or variance.
Select a shell-specific interval before interpreting these as first-shell
quantities. Radii above half the smallest cell-plane spacing repeat cell
correlations; finite-cluster boundaries reduce neighbor counts. These histories
are not an equilibrium ensemble, a uniqueness test or an uncertainty estimate.
The [analysis implementation](https://github.com/Ameyanagi/rexafs/blob/v0.2.14/crates/rexafs/src/xafs/rmc/analysis.rs)
defines the counting conventions.

[![Initial, current and best normalized Cu–Cu pair distributions from the periodic Cu example](/screenshots/0.2.12/structural-overlays.jpg)](/screenshots/0.2.12/structural-overlays.jpg)

[![Cu–Cu distance-versus-attempt heatmap with eleven samples from attempt zero through ten](/screenshots/0.2.12/structural-heatmap.jpg)](/screenshots/0.2.12/structural-heatmap.jpg)

[![Cu–Cu distance variance in square angstroms over ten RMC attempts](/screenshots/0.2.12/structural-variance.jpg)](/screenshots/0.2.12/structural-variance.jpg)

*Full unedited signed 0.2.12 windows, reopened from the same ten-attempt Cu run.
The [0, 3.5) Å interval has 70 bins and is sampled every attempt. Initial fcc Cu
has 12 neighbors at 2.55646 Å; periodic curves use full-cell density and shell
normalization. All eleven samples were retained. The variance describes the
selected distances in Å², not fit uncertainty. These plots do not establish
experimental agreement or convergence.*

**Export result…** includes `structural-evolution.json`, `structural-curves.csv`
and `structural-history.csv`, with pair identities, actual steps, bounds and
units. These retain histograms and moments, not every historical coordinate set.

### Genetic and hybrid search

**Fit settings → Search method** offers **RMC**, **Genetic / EA** and
**Hybrid EA–RMC**. The evolutionary algorithm (EA) uses selection, atom-wise
crossover, mutation and elite survivors. Start with 12 individuals, 50 generations
and two elites. Pure genetic search has no local Metropolis attempts; hybrid
starts with one per nonelite child. These are editable rexafs defaults.

Both population modes require fixed ΔE₀; selecting them with energy refinement
enabled shows an error instead of changing calibration silently. They still pay
the scattering cost. Initializing the population can take longer than ordinary
RMC, and larger populations or local budgets increase work and cache pressure.

Pause completes a generation. Stop preserves the last committed population.
Checkpoints retain all individuals and the random state. The convergence view
compares population mean and best objective; the single-chain plateau diagnostic
does not apply. Structural Current and Best both show the best member of the
current elitist population, not averaged coordinates. The Initial curve remains
the input structure. See the
[search description and implementation](https://github.com/Ameyanagi/rexafs/blob/v0.2.14/doc/rmc-structural-evolution.md#genetic-and-hybrid-search).
