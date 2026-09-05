# Multiple spectra and independent batches

**Model & fit → Fit multiple spectra** fits several spectra simultaneously.
The browser groups paths under their spectrum. Selecting a spectrum shows its
range and every assigned path's parameter values; selecting a path focuses it.

## Workflow

1. Configure the needed paths in **Paths**, including paths from different
   calculated structures where needed.
2. Choose **Fit multiple spectra**. Add the current file, or mark several files
   in the file browser and choose **+ Marked**.
3. Use **± Paths** beside a spectrum to change its assignments. Each path shows
   its calculation directory, reference distance, and number of legs.
4. Edit the displayed initial values. **Global** uses one variable across spectra;
   **This spectrum** gives that spectrum its own value and Fit toggle. Paths
   referencing the same name within a spectrum still share that variable.
   New multi-spectrum setups start distance/disorder variables per spectrum.
5. Set each spectrum's k/R range. **Transform (k)** follows that file's processing
   k-weight, including per-file overrides. Number buttons select manual fit
   weights. New models follow the transform; old projects keep saved weights.
6. **Advanced** exposes expressions for each path in the selected spectrum.
   Editing these does not change other spectra or the source path template.
   Undefined variable names have an **Add parameter** action. A numeric constant
   can become a variable with **Fit this value**.
7. Run the fit. Select a spectrum above the result plots to inspect its model,
   residuals, contributions, and R-factor. The result panel includes global
   statistics and each fitted variable's uncertainty. Re/Im χ(R) are selectable.

Assignments use path file identities rather than catalog indices. Projects and
history preserve scopes, local starting values, fit/fixed choices, per-spectrum
ranges, and per-path expressions. History includes physical path distances with
uncertainties propagated through the full covariance matrix.

A global constrained parameter cannot depend on a local parameter. Undefined
variables, invalid ranges, missing paths, empty assignments, and duplicate spectra
prevent fitting. Failed preprocessing reports the file name; it never silently
omits a dataset.

## Independent batches

**Results & batch → Batch** fits each frame independently, using its effective
processing parameters. Automatic fit weights also follow each frame's transform.
Select **Single spectrum** before starting a batch. Each row has solver status
and uncertainties; failures appear in Problems. CSV exports values and errors.

## Validation, 2026-09-06

- Synthetic simultaneous fit: one path in A, two in B, different k/R ranges,
  transform weights 1 and 3, and B-specific amplitude/distance expressions.
  Recovers shared amplitude 0.82 and local distances −0.013/+0.024 Å with finite
  uncertainties; verifies every dataset's fitted ranges/weights and contributions.
- Regression coverage includes local initial values, independent fit/fixed choices,
  mixed global/local scopes, constrained dependencies, missing/duplicate paths,
  project round trips, and preservation of old manual weights.
- Initial release GUI validation: two copies of the measured Cu150K foil (QA
  duplicates, not independent physical measurements) converge with six parameters
  and R-factor 0.00813. Project saved through the native dialog retains assignments,
  fitted values, and physical distance uncertainties.
- Final browser release GUI: independent k = 2–11 / 3–12 Å⁻¹, transform
  weights 1 / 3, and a spectrum-B-only ΔR expression converge with six variables
  and combined R-factor 0.01208. Global value edits propagate; making S₀² local
  to B raises the free count to seven and keeps its value independent. Returning
  it to Global restores six. Dragging B's k-min handle from 3 to 5 changes B alone;
  A remains at 2. The range was restored after this interaction check.
- Reopened the native saved project and verified its independent settings.
  Final saved history contains the resolved weights 1 / 3, both range sets,
  all six fitted variables, and positive physical-distance uncertainties.
- Full GUI release suite: 101 passed, 2 ignored; release build succeeds.
- Batch release GUI: three Cu150K copies converge; an invalid fourth reports
  “no numeric data.” Native CSV export matches single-fit values/errors to its
  six-decimal precision.

Screenshots: [setup](validation/joint-fit-setup.jpg),
[result](validation/joint-fit-results.jpg),
[batch](validation/batch-fit-validation.jpg).
