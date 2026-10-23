---
title: "Multiple spectra and batches"
description: "Share parameters across spectra or fit a collection independently."
audience: user
---

**Fit → Model → Fit multiple spectra** fits spectra simultaneously. Select a
spectrum in the browser to see its range and assigned path parameters; select
a path to focus it.

A joint fit combines residuals under shared parameter constraints. See
[fitting statistics](/docs/science/fitting-statistics/) for the objective,
chi-square, covariance and R-factor. Duplicating a spectrum does not create
independent physical measurements.

## Workflow

1. Configure **Paths**. You can combine paths from different calculated structures.
2. Choose **Fit multiple spectra**. Add the current file, or mark several files
   in the file browser and choose **+ Marked**.
3. Use **± Paths** to change a spectrum's assignments. Each path shows its
   calculation directory, reference distance and number of legs.
4. Edit the displayed initial values. **Global** uses one variable across spectra;
   **This spectrum** gives that spectrum its own value and Fit toggle. Paths
   referencing the same name within a spectrum still share that variable.
   New multi-spectrum setups start distance/disorder variables per spectrum.
5. Set each spectrum's k/R range. **Transform (k)** follows that file's Process › Transform
   k-weight, including per-file overrides. Number buttons select manual fit
   weights. New models follow the transform; old projects keep saved weights.
6. **Advanced** exposes expressions for each path in the selected spectrum.
   Editing these does not change other spectra or the source path template.
   Undefined variable names have an **Add parameter** action. A numeric constant
   can become a variable with **Fit this value**.
7. Run the fit. Select a spectrum above the plots to inspect its model, residuals,
   contributions and R-factor. Real/imaginary χ(R) views are available. The panel
   also shows global statistics and each fitted variable's uncertainty.

Assignments use path file identities rather than catalog indices. Projects and
history preserve scopes, local starting values, fit/fixed choices, per-spectrum
ranges, and per-path expressions. History includes physical path distances with
uncertainties propagated through the full covariance matrix.

A global constrained parameter cannot depend on a local parameter. Undefined
variables, invalid ranges, missing paths, empty assignments, and duplicate spectra
prevent fitting. Failed preprocessing reports the file name; it never silently
omits a dataset.

## Independent batches

**Fit → Results → Batch** fits each frame independently, using its effective
processing parameters. Automatic fit weights also follow each frame's transform.
Select **Single spectrum** before starting a batch. Each row has solver status
and uncertainties; failures appear in Problems. CSV exports values and errors.

In [Series](/docs/desktop/series/), choose **Sampled frames** for a preview or
**All frames** for the full scan before running the calculation. The heatmap
remains a sampled overview in either mode. A batch fits independent parameter
sets; use the joint workflow above when spectra must constrain the same variable.
