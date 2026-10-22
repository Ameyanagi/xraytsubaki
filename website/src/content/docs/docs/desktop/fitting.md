---
title: "Fit your first coordination shell"
description: "Build and inspect a one-shell Cu EXAFS model."
audience: user
---

Continue the [Cu processing walkthrough](/docs/getting-started/first-analysis/)
with a one-shell model. For your own samples, choose appropriate ranges and
constraints; this example does not establish a unique structural interpretation.

## 1. Choose a structure and calculate paths

Open **Fit → Structure → Curated**, select **Cu / Copper**, then
**Use structure →**. In Calculate, select Cu as absorber, the K edge, an 8 Å
radius and **ReFEFF**. Choose **Calculate paths** and wait for completion.

Every 0.2.11 desktop package offers ReFEFF and FEFF10; see the
[Windows ARM64 requirement](/docs/getting-started/install/#arm64-availability).
Record the engine used.


[![Full structure and calculation setup for the built-in Cu model](/screenshots/0.2.11/structure.jpg)](/screenshots/0.2.11/structure.jpg)

*Cu scattering setup. Full window, rexafs 0.2.11 on macOS; select to enlarge.*


## 2. Select a model

Choose **First shell** in Paths, then **Set up model →**. The Cu example selects
the first Cu–Cu single-scattering path. The calculated reference distance and path
degeneracy describe the reference structure, not fitted results.


[![Full Paths stage with first-shell Cu scattering selected](/screenshots/0.2.11/paths.jpg)](/screenshots/0.2.11/paths.jpg)

*First-shell Cu path. Full window, rexafs 0.2.11 on macOS; select to enlarge.*


Inspect the model's amplitude, energy shift, distance change and disorder
parameters. Their meanings and units are described in
[structures and paths](/docs/desktop/structures/). Keep the initial template for
this example. In **Fit ranges**, inspect $k=2$–12 Å⁻¹ and $R=1$–3 Å. The transform
weight is 2. The fit is in R space; the visible k-space curves can extend beyond
the selected fitting range.


[![Full model view with k and R fitting limits](/screenshots/0.2.11/fit-ranges.jpg)](/screenshots/0.2.11/fit-ranges.jpg)

*R-space fit limits. Full window, rexafs 0.2.11 on macOS; select to enlarge.*


## 3. Run and inspect

Choose **Run fit**. In Results, inspect the measured/model curves, residuals,
parameter uncertainties, correlations where reported, and physical path distances.
A variable that is the whole expression of one path-parameter column is labelled
with that parameter's symbol and unit, for example `dr_1` appears as
"ΔR (Å) · Cu–Cu 2.553 Å" with the variable name beside it. The path is named by its
absorber–scatterer chain and FEFF half path length R_eff (the reference geometry,
not the fitted distance). Shared variables such as S₀² and ΔE₀ carry no path, and
variables used only inside expressions keep the name you typed. Copied Markdown
reports keep the variable names. A small R-factor and a converged optimizer do not establish that the model is unique
or physically complete.


[![Full Cu first-shell fit result with curves, residuals, parameters and statistics](/screenshots/0.2.11/fit-result.jpg)](/screenshots/0.2.11/fit-result.jpg)

*ReFEFF first-shell fit; higher shells lie outside the selected R range. Full window, rexafs 0.2.11 on macOS; select to enlarge.*


The screenshot's noise scale is 1. Its uncertainties therefore depend on that
scale and the local covariance approximation; they are not a full experimental
error budget. Without **ε from noise**, Results shows reduced χ² as
**n/a (noise not estimated)**, because the statistic needs a measurement-noise
estimate to mean anything. Read [fitting statistics](/docs/science/fitting-statistics/) before
interpreting the reported standard errors.

Save the project to retain the model/history, and use
[publication exports](/docs/desktop/publication/) to preserve data and methods.
For shared variables across spectra, continue with
[multiple-spectrum fitting](/docs/desktop/multiple-spectra/).


## RMC fitting mode

Version 0.2.10 adds **Fit mode: RMC** at the upper right of Fitting (labelled
**Method: RMC** in the unreleased development build).
Choose a structure, edit its supercell with a live 3D preview, inspect the initial exact ReFEFF
calculation and run coordinate refinement against the processed spectrum.
The initial objective is R real plus imaginary, with R minimum at or above
the saved AUTOBK Rbkg. Adaptive scattering remains experimental and is not
exposed by these controls.

Results update during a run with initial/best curves, coordinates, residual
history and calculation statistics. Pause retains a warm calculator; Stop and
save or an automatic checkpoint supports later resume. **Recover latest run**
loads the most recent durable checkpoint after reopening the application.
Resume uses its original inputs and random-number state. Wait for Pause or
Stop and save to finish before closing if the latest state is needed; an
unexpected exit can lose moves since the previous checkpoint.

The current desktop supports one spectrum and one structure. Hybrid EA and
mixture controls are later additions. Reaching a step limit does not establish
convergence: the display separately reports the shared residual-trend criterion.
Follow the [RMC workflow](/docs/desktop/rmc/) for setup, saved preprocessing,
resume and convergence diagnostics.

The builder updates after a short typing delay without running scattering.
Invalid edits keep the last valid preview and block a new calculation. Suggested
cell size uses the cluster radius and displacement margin; starting fit ranges
respect measured k coverage and begin 0.15 Å above the spectrum's saved Rbkg.
These are editable starting values, not a claim of structural convergence.
