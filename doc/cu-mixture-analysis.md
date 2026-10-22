# Copper mixtures: PCA, MCR-ALS and reference fitting

These desktop features and the native MCR-ALS API are available in **rexafs 0.2.9**.
LCF, PCA and MCR-ALS are not yet exposed in the Python or TypeScript bindings.

**Unreleased desktop defaults:** PCA plots start on a linear scale. MCR-ALS
starts with both range endpoints set to Auto, meaning the full measured energy
interval shared by all selected spectra. The overlap is resolved after loading
the selected inputs and recorded in the result. Explicit endpoints still select
a narrower interval; choosing **Common full range** restores automatic overlap.
MCR range fields are separate from PCA/LCF fields. These desktop choices are
implemented in [analysis ranges](../crates/rexafs-gui/src/app/shell/tools/analysis_range.rs)
and the [MCR worker](../crates/rexafs-gui/src/app/shell/tools/mcr.rs); the native
library's `McrConfig::range = None` convention is unchanged.
See the [current API guide](analysis-api.md) for Rust examples, result shapes,
defaults and proposed simplifications.

For the newer 50-frame raw-absorption sequence with automatic processing, see
[the synthetic copper tutorial](synthetic-copper-reduction.md). The historical
100-mixture workflow below retains its original settings and results.

See the [validation record](validation/2026-09-16-cu-mixtures/README.md) for
measured recovery errors, GUI screenshots and export checks.

## Prepare comparable spectra

The retained [fixtures](../crates/rexafs/tests/fixtures/analysis/cu-mixtures/README.md)
contain 100 synthetic mixtures and three prepared Cu foil, Cu₂O and CuO standards,
with generation settings, true fractions and original-source attribution. The
original Athena project is unchanged. These are noise-free linear combinations,
not 100 independent experimental measurements.

For the desktop experiment, import the 100 mixture files and the three standards.
Use E₀ = 8979 eV, edge step = 1, pre-edge offsets −150 to −75 eV, post-edge offsets
+150 to +650 eV, polynomial order 2 and Victoreen power 0 for every group.
Use **flat** for the analyses and a calculation interval of −29 to +171 eV
relative to E₀. This selects 190 measured points from 8950.205 to 9148.198 eV.
The fixed edge step and shared polynomial settings preserve the linear mixture
relation when these already normalized arrays are flattened. Automatic,
sample-dependent preprocessing need not preserve that relation.

The desktop analysis selector starts on flat; norm remains available. The core
functions still default to norm and require prepared arrays. Flat spectra are
dimensionless absorption with the fitted post-edge curvature removed. Changing
analysis space changes the fitted data; it is not merely a plot setting.

## Inspect the collection and choose a count

Mark the 100 mixtures and open Compare. **Plot all 100** displays every marked
spectrum; **Preview 12** samples only the display. **Gradient** colors the traces
in collection order. All marked inputs participate in PCA and MCR regardless of
the plot mode, including the current group. LCF instead treats the current group
as its target and the other marked groups as standards.

The plot toolbar offers XANES (−20 to +80 eV), −200 to +800 eV, and Full spectrum.
These are viewing ranges relative to the current spectrum's E₀. They do not
change calculation settings. The analysis form has separate range presets and
shows its selected interval as dashed lines on the spectrum plot. Common full
range uses the intersection of prepared inputs; prepare them before selecting it.
The dashed lines belong to the open analysis form: switching between PCA and
MCR redraws them at that form's interval (PCA defaults to −20 to +30 eV from E₀;
blank MCR fields use the common measured interval), and closing the form
removes them. Opening a processing tool such as Deglitch or Align returns to the
spectra; reopen the analysis tool to see its stored result again.
Completed result plots show the actual sampled interval retained with that run.

Open **Data → Parameters → Principal components**. Inspect:

- **Scree** and **Cumulative**: the contribution of each principal component.
- **Error vs count**: relative squared reconstruction error against retained
  component count, including zero components. This is training reconstruction,
  not cross-validation. A target curve, when present, uses its retained data.
- **PC1 / PC2**, the other pairwise score views, and **Similarity**: sample
  grouping. Similarity is cosine similarity in the selected score subspace;
  negative values are valid and a zero vector is undefined.
- **Loadings** and **Reconstruction**: spectral directions and target residuals.

Use **Y axis → Linear / Log** on Scree, Error vs count and IND. In the released
0.2.9 workflow, Log is the default and makes small contributions visible; the
unreleased preview defaults to Linear. Linear shows their magnitude relative to
the largest values and preserves exact zeros. Log floors values at
10⁻³² for plotting only. Cumulative contribution stays linear from 0 to 100%.
The choice persists between diagnostic views during the session, changes no
calculation, and does not change the component-count suggestion or exported
values. A numerical-rank suggestion uses
σ > ε max(n,p) ‖D‖F, where ε is double-precision machine epsilon, n is the sample
count, p the energy-point count and D the original prepared matrix. This is a
rexafs precision criterion, not an experimental noise model. Otherwise an
interior IND minimum supplies a labeled heuristic; boundary minima do not.

For these mixtures, uncentered PCA has three independent directions. With mean
subtraction, there are two varying directions plus the mean: closure leaves two
independent fractions for three standards. Neither PCA directions nor an IND
minimum directly identify chemical species. The
[notebook review](validation/2026-09-16-cu-mixtures/notebook-review.md) explains
how these views correspond to the supplied notebooks.

## Resolve components and compare references

With only the 100 mixtures marked, open **MCR-ALS**. Choose flat, three
components, sum-to-one coefficients, signed spectra, seed 0 and up to 2000
iterations for this demonstration. Signed spectra retain small negative values
from baseline subtraction. The core default iteration limit is 500; reaching it
is reported as an iteration limit, not convergence. Inspect Spectra, Fractions,
Convergence and Residuals. Sample order in the fractions plot is not elapsed time.

After fitting, mark exactly the three prepared standards and use **Compare
marked standards**. References are interpolated in the retained analysis space
onto the result grid. Global permutation matching minimizes the sum of squared
relative spectral discrepancies; it does not rescale, shift or refit recovered
components. The overlay uses matched colors, solid recovered curves and dashed
references. A match is a comparison, not proof of chemical identity. The
assignment implementation follows the minimum-cost assignment problem described
by [Kuhn (1955)](https://doi.org/10.1002/nav.3800020109).

**Add to Groups** creates one calculated group for each recovered spectrum.
For LCF, it creates the fitted sum, data-minus-fit residual, and each weighted
reference contribution. These groups retain their calculated norm or flat arrays
without running normalization again. Mark calculated groups with the original
standards to use the normal comparison plot. MCR components are estimated pure
spectra; LCF contributions already include their fitted weights, so their
amplitudes should differ from unweighted standards.

Recovered norm/flat groups also support **Background → Transform → Fit**.
By default their supplied values have a unit edge step and are not normalized
or flattened a second time. Normalize lets you adjust E₀ and offers **Refit
pre/post-edge** to enable the ordinary baseline ranges, polynomial and edge-step
controls. This can correct an offset, slope or imperfect step in a recovered
flattened component. Turning it off returns to the retained component values.
Background and transform settings remain editable. The source arrays and
MCR Operation record remain intact when downstream processing changes. Residual
groups retain difference semantics and cannot enter absorption processing.

Only the resolved energy interval is available. To fit EXAFS on recovered
components, rerun MCR over a sufficiently wide common measured interval before
adding the groups. The 8950–9148 eV result above intentionally covers XANES; it
does not recover the excluded high-energy signal. AUTOBK acts on the declared
flat or norm component directly, rather than reconstructing a raw measurement.

Each group retains a distinct quantity and an Operation metadata record: method,
analysis space, actual energy interval, ordered source identities/fingerprints,
settings and fit statistics. MCR adds component index, initialization, constraints,
termination and coefficients; LCF adds weights and configuration. Known reference
matches are labeled as matches. The portable `.rxs` project also retains the
complete owned MCR/PCA/LCF result arrays. Later preprocessing edits do not rewrite
these historical results.

The Publish analysis export includes calculated two-column `.dat` files with
comment headers and a companion `*-provenance.json` for these groups. Headers
identify the quantity and calculation. Prefer `.rxs` for reopening without losing
quantity semantics; generic text import does not restore a calculated group's
full type automatically. Add to Groups currently supports norm and flat outputs;
derivative and χ-space LCF arrays remain available in analysis JSON.

## Series display and batch coefficients

Use the dropdown above the Series heatmap to choose a scan. A scan is currently
one imported directory, ordered by the catalog's filenames; marks do not define
scan membership or a time axis. Portable projects display the original directory
name rather than the private extraction-cache identifier.

The Series heatmap and cursor plot have separate **norm μ(E)** and **flat μ(E)**
choices; flat is the initial display. Selecting a frame updates the same
observable used by the cursor plot in energy, k and R spaces. If a sampled
overview does not contain that exact frame, the cursor curve stays blank until
its full source loads; a neighboring frame is never labeled as the selection.
Plot sizing follows the actual panels, with fewer energy ticks on narrow cards.

With the three standards marked, select the 100-frame mixture scan and use
**Run LCF trend**. Its analysis space and constraints come from the Data LCF
form; the display choice is independent. The project retains coefficient rows,
input and standard identities, configuration, completion/cancellation flags and
per-frame errors. Publish writes `data/lcf-series.csv` and `data/lcf-series.json`.
CSV frames are one-based; internal JSON frame keys are zero-based. Rows contain
one weight per named standard followed by the R-factor. A finished job can still
contain failed or cancelled frames, so compare row count with the input record.
These are historical results; opening the project does not rerun the trend.

## Native calculation

The Rust implementation uses existing nalgebra and rexafs constrained
least-squares code, with no Python runtime dependency. A prepared collection can
be fitted with:

```rust,ignore
use rexafs::prelude::{AnalysisSpace, McrConfig, mcr_als};
let result = mcr_als(&prepared_spectra, &McrConfig {
    space: AnalysisSpace::Flat,
    range: Some((-29.0, 171.0)),
    components: 3,
    max_iterations: 2000,
    ..McrConfig::default()
})?;
// result.spectra: components × energy points
// result.concentrations: input spectra × components
```

For n spectra, p energies and r components, MCR minimizes ‖D − C S‖²F.
D is n × p prepared dimensionless absorption, C is n × r nonnegative
coefficients, and S is r × p estimated component spectra. Each alternating step
solves one factor while holding the other fixed. Optional closure constrains
each row of C to sum to one; optional spectral nonnegativity constrains S.
No alignment, smoothing, normalization or clipping occurs inside the solver.
All spectra must cover the first spectrum's selected grid; extrapolation and
rank-deficient initial factors are rejected. Returned arrays own the best
complete accepted iteration, with an explicit termination reason.

This bilinear model is described in the
[NIST pyMCR paper](https://doi.org/10.6028/jres.124.018) and
[official documentation](https://pages.nist.gov/pyMCR/). Initialization, numerical
thresholds and defaults are rexafs choices, documented at the
[implementing API](../crates/rexafs/src/xafs/analysis/mcr.rs). pyMCR is used only
for an independent development comparison. A near-zero reconstruction residual
does not prove unique pure spectra: multiple factor pairs can describe the
same mixtures. Known-composition anchors add information and must be identified
as such when reporting recovery.
