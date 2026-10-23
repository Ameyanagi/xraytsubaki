---
title: "LCF, PCA and MCR-ALS"
description: "Analyze collections while keeping preprocessing assumptions explicit."
audience: user
---

This guide describes **rexafs 0.2.13**; older figures retain their capture versions.
LCF, PCA and native MCR-ALS are available in the desktop and Rust. Python and
TypeScript expose spectrum processing; their collection-analysis bindings remain
planned. In the desktop, use **Data → Parameters** or action search.

Missing normalization or background arrays are prepared on temporary copies
using each input's settings. Existing selected arrays are reused and the source
spectra remain unchanged. The desktop starts on **flat**; **norm** remains
selectable. Rust defaults to normalized absorption; choose
`AnalysisSpace::Flat` explicitly for flattened absorption.

## Linear combination fitting

Linear combination fitting (LCF) models an unknown spectrum as a weighted sum of references:

$$\min_{\mathbf w}\sum_{j=1}^{M}\left[U(x_j)-\sum_{i=1}^{S}w_iS_i(x_j)\right]^2.$$

$U$ is the unknown, $S_i$ the $i$th standard, $w_i$ its dimensionless weight,
$M$ the number of fitted samples and $S$ the number of standards. The axis $x$
is energy in eV or k in Å⁻¹, depending on the analysis space. All spectral arrays
must use the same units and normalization. Standards are interpolated onto the
unknown's grid in the selected range. Bounds must be finite and increasing, and
every input must cover the entire interval. References also need coverage for
the permitted shift margin when shifts are fitted. An incomplete interval
produces an error rather than extending endpoint values.

The default bounds are $0\le w_i\le1$ with $\sum_iw_i=1$. Energy-space ranges
start at −20 to +30 eV relative to $E_0$; χ-space ranges start at 3–12 Å⁻¹.
Optional shifts move each standard's axis before fitting; they are off by default.
The shift is added to the standard's axis: a positive value moves a feature to
higher energy or k. `max_e0_shift=5` means 5 eV for energy spaces but 5 Å⁻¹ for
χ-space, despite its historical name; choose a meaningful bound for the selected
axis. These are constant axis shifts, not a full physical recalibration model.
Weights only support composition claims when the standards and measurement model
justify that interpretation. Similar standards can give ambiguous weights.

rexafs solves the bounded linear subproblem to numerical tolerances with an
active-set method: coefficients on bounds are held fixed while the free
coefficients are solved, then bounds are activated or released as needed.
When shifts are allowed, an outer nonlinear solve varies the shifts and solves
the bounded weights again at each step. See
[Nocedal and Wright, *Numerical Optimization*](https://doi.org/10.1007/978-0-387-40065-5)
for active-set constrained optimization. This description is traced to
[lcf.rs](https://github.com/Ameyanagi/rexafs/blob/v0.2.9/crates/rexafs/src/xafs/analysis/lcf.rs).
The generated [Rust API](/api/rust/rexafs/xafs/analysis/index.html) documents
`LcfConfig`, `lcf` and combination searches. For the general XANES use of reference
mixtures, see [Larch's linear-analysis guide](https://xraypy.github.io/xraylarch/xafs_xanes.html).

The reported `chi_square` is the unweighted squared residual sum, in squared
spectral units. `reduced_chi_square` divides it by
`max(n_data - n_vary, 1)`. Here `n_vary` counts one coefficient per standard,
subtracts one for the sum constraint, and adds each fitted shift; coefficients
on bounds do not reduce that reporting count. This differs from the
[EXAFS path fitter's statistics](/docs/science/fitting-statistics/).
Coefficient errors use a local linear covariance with the fitted shifts held
fixed; shift errors come from the projected nonlinear problem. They are not a
joint coefficient/shift covariance or a confidence interval. Similar references,
active bounds, interpolation, and an uncertain reference set limit interpretation.
The R-factor is the squared discrepancy divided by the data's squared norm;
it is NaN for an all-zero signal.

## Principal component analysis

PCA arranges $n$ spectra on a common grid of $m$ points in a matrix
$D\in\mathbb R^{n\times m}$ and computes a singular value decomposition:

$$D=U\Sigma V^\mathsf T.$$

Rows of $V^\mathsf T$ are orthonormal spectral components; the diagonal singular
values $\sigma_i$ indicate their magnitudes. The fraction attributed to component
$i$ is $\sigma_i^2/\sum_j\sigma_j^2$. Matrix entries have the chosen spectral
units; components are normalized and scores carry the spectral scale.

rexafs defaults to **no mean subtraction** (`center=false`). If centering is
enabled, $D$ in this equation is the mean-subtracted matrix. Centering changes the
question answered by the decomposition and the interpretation of rank; record it
when comparing software. A component is a mathematical direction, not necessarily
a pure chemical species. Noise and inconsistent normalization can create
additional apparent components. Target transformation tests reconstruction using
selected components, not chemical uniqueness.

See the [Rust PCA API](/api/rust/rexafs/xafs/analysis/pca/index.html) for configuration
and [Larch's PCA explanation](https://xraypy.github.io/xraylarch/xafs_xanes.html#principal-component-analysis)
for background; its default centering differs from rexafs.

The API's `eigenvalues` are $\lambda_i=\sigma_i^2/n$, in squared spectral units.
Without centering these are eigenvalues of the second-moment matrix
$D^\mathsf T D/n$, not a covariance about the mean. With centering they use a
population denominator $n$, not the sample-variance denominator $n-1$.
The reported variance fraction is therefore a fraction of squared signal when
uncentered, or of squared deviations when centered. It is zero for every
component of an all-zero input matrix.

To reconstruct a target spectrum $\mathbf y$ on the model grid using $c$
components, rexafs computes

$$
\mathbf a=C_c(\mathbf y-\boldsymbol\mu),\qquad
\widehat{\mathbf y}=\boldsymbol\mu+C_c^\mathsf T\mathbf a.
$$

$C_c$ contains the first $c$ orthonormal component rows, $\boldsymbol\mu$ is
the stored mean (zero without centering), and $\mathbf a$ contains projection
scores in the input spectral units. The reconstructed $\widehat{\mathbf y}$
has those same units. Scores can be negative and are not composition fractions.
Zero retained components reconstructs the mean alone. `target_transform()`
interpolates within the target's measured coverage onto the training grid and
rejects incomplete coverage; `reconstruct()` instead requires values already on
that grid.

The indicator used to suggest a retained count $c$ is

$$
\operatorname{IND}(c)=
\frac{1}{(n-c)^2}
\sqrt{\frac{\sum_{j=c+1}^{\min(n,m)}\lambda_j}{m(n-c)}},
\qquad 0\le c<n.
$$

Here $n$ is the number of training spectra, $m$ the number of common grid
points, and the eigenvalue index starts at one. This is the precise scaling
implemented in [pca.rs](https://github.com/Ameyanagi/rexafs/blob/v0.2.9/crates/rexafs/src/xafs/analysis/pca.rs).
The method is based on [Malinowski (1977)](https://doi.org/10.1021/ac50012a027);
its minimum suggests a numerical rank rather than proving a number of chemical
species. The recommended `component_count_suggestion()` first recognizes exact
low-rank data using a floating-point tolerance. Otherwise it requires a finite,
positive interior IND minimum below both boundary values. Zero data and an
unsupported boundary minimum return no suggestion. This is a rexafs diagnostic,
not an experimental noise threshold. When $n>m$, missing tail eigenvalues count
as zero and can make IND uninformative. The older
`suggested_components_ind()` helper retains its historical minimum search for
compatibility. Inspect spectrum shapes, noise and reconstruction error along
with any count suggestion.

PCA reconstruction `chi_square` is also a raw squared residual sum; its reduced
value divides by `max(m - c, 1)`. No experimental noise variance is supplied to
this calculation, so this number is not a calibrated chi-square test.

<a id="unreleased-collection-analysis"></a>

## Collection workflow

LCF, PCA and MCR use the same input-preparation and coverage rules. Energy bounds
are offsets from E₀: the target's E₀ for LCF, and the first input's E₀ for PCA
and MCR. View presets change the plot only; calculation bounds select the data
used by an analysis. From 0.2.13, desktop MCR-ALS has an independent **Auto**
range covering the full common energy interval of the selected spectra. It does
not inherit PCA's default −20 to +30 eV XANES interval. Enter explicit MCR bounds
to select a narrower interval; saved results retain their calculation settings.

The [synthetic copper tutorial](/docs/desktop/synthetic-copper/) follows 50
known raw-absorption mixtures through default PCA, blind MCR-ALS, known-reference
LCF and Series differences. It distinguishes normalized coefficients from raw
mixing weights and retains the numerical results used in its figures.

`lcf_batch` keeps each target's result or error in input order, and
`lcf_batch_with_progress` supports cancellation after completed rows. The desktop
Series workflow uses this core API. PCA's `component_count_suggestion`,
`numerical_rank` and `reconstruction_errors` now supply the desktop's count
suggestions and error curves. Python and TypeScript analysis bindings are
explicit follow-up tasks, including parity tests for this shared behavior.

The collection plot offers **Plot all**, **Preview 12** and **Gradient**. Preview
sampling affects the display only. PCA and MCR use every marked spectrum,
including the current group. Plot range presets change the view; separate
analysis presets and dashed bounds specify the calculation interval.

PCA adds scree, cumulative contribution, **Error vs count**, score projections,
loadings and similarity views. Error versus count measures reconstruction,
not cross-validation. For closed three-standard mixtures, mean-centered PCA can
have two varying directions plus the mean; that does not imply two species.
Numerical-rank and interior IND suggestions are labeled diagnostics. Scree,
Error vs count and IND offer **Y axis → Linear / Log**. From 0.2.13, Linear is
the default and preserves zeros. Log remains available with a display-only floor
of 10⁻³². Cumulative contribution remains linear. Switching scales does not
recalculate PCA or change exported values.

[![Fresh default PCA of 50 synthetic copper spectra on a linear scale in rexafs 0.2.13](/screenshots/0.2.13/copper-pca.jpg)](/screenshots/0.2.13/copper-pca.jpg)

This signed 0.2.13 capture uses the bundled 50-frame example, **flat**, no mean
subtraction and the default −20 to +30 eV calculation interval. Three components
retain 99.9998568% of the squared signal. See the
[tutorial](/docs/desktop/synthetic-copper/) for the known recipe and interpretation.

The [historical 0.2.11 capture](/screenshots/0.2.11/pca-error-linear.jpg) reopens
a saved 0.2.9 analysis of all 100 synthetic Cu mixtures in **flat**, with
mean subtraction enabled and a −29 to +171 eV interval relative to E₀. Two
varying directions explain 75.91% and 24.09% of the centered squared signal.
The mean retains the shared part of the three-standard mixture; two directions
do not imply two chemical species. Select any screenshot to view it at full size.

MCR fits the bilinear model $D = C S + R$, with spectra as rows of $D$,
nonnegative coefficient rows in $C$, component spectra as rows of $S$, and
residuals $R$. Closure and spectral nonnegativity are separate constraints.
Missing norm or flat arrays are prepared on temporary copies using input
settings. Every input must provide complete common measured coverage.
Inspect convergence, per-sample residuals and component spectra; a small residual
does not establish unique chemical factors. See the
[NIST pyMCR paper](https://doi.org/10.6028/jres.124.018) for the model and the
[Rust API](/api/rust/rexafs/xafs/analysis/mcr/index.html) for
rexafs's independent implementation, defaults and error conditions.

[![Fresh default MCR component spectra from 50 flattened copper mixtures in rexafs 0.2.13](/screenshots/0.2.13/copper-mcr-spectra.jpg)](/screenshots/0.2.13/copper-mcr-spectra.jpg)

The signed 0.2.13 run uses all 517 shared points, three components, coefficient
closure, seed zero and the default 500-iteration limit. Spectral nonnegativity is
disabled. It converged after 110 iterations. These factors require reference
comparison before assigning chemical labels.

The [historical 100-mixture run](/screenshots/0.2.11/mcr-components.jpg) uses
the common full interval, 8780.2–9768.2 eV, three components,
coefficient closure, seed zero and at most 2,000 iterations. Spectral
nonnegativity is disabled to retain small negative baselines. It converged
after 870 iterations. These are estimated factors, not independently identified
pure compounds; reference comparison is still necessary.

These are retained example settings. The current desktop default is 500 maximum
iterations; the [50-frame copper tutorial](/docs/desktop/synthetic-copper/)
demonstrates the current defaults and their recovered fractions.

After fitting, mark standards and use **Compare marked standards** for a matched
spectral overlay without rescaling.

[![Recovered MCR components overlaid with their matched Cu foil, Cu₂O and CuO reference spectra](/screenshots/0.2.11/mcr-reference-comparison.jpg)](/screenshots/0.2.11/mcr-reference-comparison.jpg)

*Solid curves are recovered factors; dashed curves are the matched references.
Matching labels the comparison and does not change the blind fit.*

**Add to Groups** retains each MCR component,
or an LCF fit, residual and weighted contributions, as calculated norm/flat
groups. They can be compared with references using the usual group plot without
normalizing the calculated arrays again. Operation metadata records the source
identities and calculation settings. Portable projects retain owned analysis
results; Publish exports add calculated data files with metadata headers and
companion provenance JSON. Derivative and χ-space LCF arrays remain available
in analysis JSON, but currently cannot be added as calculated groups.

Recovered norm/flat component groups can continue through background removal
and transforms in **Process**, then **Fit**. Their values and unit edge step are
preserved by default. The Normalize section offers E₀ and an optional **Refit pre/post-edge** action, which enables
the ordinary baseline and edge-step controls even for flattened input. The
original recovered arrays remain stored, and disabling the option restores
the default path. Residual groups remain
differences. A component only covers the retained MCR interval, so repeat MCR
over wider measured coverage before doing broad-range EXAFS fitting. The Rust
`McrResult::component_spectrum(index)` method provides this conversion; these
analysis APIs are not yet exposed by the Python or TypeScript bindings.

[![A recovered MCR component processed through background subtraction and Fourier transformation in rexafs 0.2.11](/screenshots/0.2.11/mcr-component-fft.jpg)](/screenshots/0.2.11/mcr-component-fft.jpg)

The component was added with **Add to Groups** and opened in the 0.2.11
**Transform** stage, now part of **Process**.
The plots demonstrate subsequent processing over its retained coverage; they
do not establish a physically unique component or validate a structural model.
These captures were made through computer use from the signed 0.2.11 Mac app.
PCA and MCR show retained results from the earlier project; the reference overlay
and component-to-group processing were checked again in 0.2.11.
See [input and screenshot provenance](/licenses/#documentation-screenshots).

The Series view separately offers norm and flat absorption, with flat initially
selected. Cursor plots follow the selected frame in energy, k and R spaces.
Batch LCF coefficients and settings are retained in projects and exported as
`data/lcf-series.csv` and JSON; inspect failed/cancelled frame records before
interpreting an incomplete batch.

## Data-treatment choices

| Tool | What changes | What to inspect |
|---|---|---|
| Calibration / alignment | Energy origin or relative energy shift | A suitable reference and common interval |
| Deglitch | Selected bad samples | Real features must not be mistaken for glitches |
| Truncate | Available energy extent | Enough pre/post-edge and EXAFS range remains |
| Rebin | Sampling on pre-edge/XANES/k regions | Bin widths and lost resolution |
| Smooth | Local spectral variation | Noise reduction can also suppress signal |
| Merge | Several spectra become a mean/combined spectrum | Alignment and physically comparable measurements |
| Difference | A reference is subtracted | Common units, normalization and energy registration |

Keep original inputs and record the transformations. More interpolated or
zero-padded points do not create independent physical information. The
[generated tools documentation](/api/rust/rexafs/xafs/tools/index.html) defines the
implemented methods and numerical defaults.

### Rebinning and the spread of merged spectra

The default rebin grid uses 10 eV pre-edge steps up to $E_0-30$ eV,
0.5 eV steps through the near-edge region up to $E_0+50$ eV, then
0.05 Å⁻¹ steps in k. Boxcar bins average absorption and retain the nominal grid
energy. Centroid bins also replace that energy with the mean measured energy in
the bin. Empty bins use linear interpolation. The finite outer bin boundaries
can omit points near the ends of the measured interval; inspect the returned
grid before interpreting endpoint features. Within-bin standard deviations
measure variation among the included samples, not propagated measurement errors.
See [`rebin` in the source](https://github.com/Ameyanagi/rexafs/blob/v0.2.4/crates/rexafs/src/xafs/tools.rs).

For a merge, rexafs first interpolates each spectrum onto the chosen grid within
the shared measured energy interval. At each energy it calculates

$$
\bar\mu=\frac{\sum_{i=1}^{N}w_i\mu_i}{W},\qquad
s=\sqrt{\frac{\sum_{i=1}^{N}w_i(\mu_i-\bar\mu)^2}{W(N-1)/N}},\qquad
W=\sum_{i=1}^{N}w_i.
$$

Here $N$ is the number of selected spectra, $\mu_i$ is interpolated absorption,
and $w_i$ is its finite, nonnegative relative weight, with $W>0$. The mean
$\bar\mu$ and reported spread $s$ retain absorption units. Equal weights give
the usual sample standard deviation; see the
[NIST definition of sample spread](https://www.itl.nist.gov/div898/handbook/eda/section3/eda356.htm).
For unequal weights, the denominator is a rexafs choice, not a general unbiased
weighted variance estimator. Zero-weight members still count in $N$; a single
member receives zero spread. This is the formula implemented by
[`merge_spectra`](https://github.com/Ameyanagi/rexafs/blob/v0.2.4/crates/rexafs/src/xafs/tools.rs),
not the standard error of the merged mean. It also does not estimate correlations
introduced by interpolation. Keep the individual scans when assessing whether
variation reflects noise, energy drift or an actual change in the sample.


## XANES peak fitting

Version 0.2.10 adds **Data → XANES peak fit** and the shared `PeakFit` API in
Rust, Python and TypeScript. Build a sum of named peaks, steps and baseline
terms, choose the signal representation and energy interval, and inspect the
component curves and residual. Fits can be applied to marked groups or a Series;
saved models can also run during experimental Live acquisition.

Peak area is the whole-axis model integral in signal units × eV, not peak
height. Widths are full widths at half maximum in eV; absorption steps use a
separate positive energy scale. Bounds, fixed parameters and mathematical ties
encode assumptions. The optimizer does not choose a peak count or identify
chemical species. Report its termination status and inspect local uncertainty
diagnostics; a small residual does not establish a unique decomposition.

See the [model and uncertainty guide](https://github.com/Ameyanagi/rexafs/blob/v0.2.10/doc/xanes-peak-fitting.md),
[Python API](/docs/reference/stable/python/peakfit/) and
[TypeScript API](/docs/reference/stable/typescript/peakfit/).
