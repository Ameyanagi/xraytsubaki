# Fluorescence over-absorption correction

Introduced in rexafs 0.2.10. The native core implements the named
`fluo_elam_v1` profile, with Python and TypeScript APIs using that same calculation.
The source-checkout desktop includes the preview and corrected-group workflow below.
Series/Live correction recipes and native Windows/Linux interaction checks remain
unfinished. These additions are not part of an already released GUI.

Fluorescence intensity is not always proportional to the absorber's absorption.
Attenuation of both the incident and emitted beams can reduce spectral features.
The initial correction assumes a **homogeneous, optically thick sample**, a
specified fluorescence line and measured geometry. It follows the FLUO-style
XANES operation in [pinned Larch fluo_corr](https://github.com/xraypy/xraylarch/blob/e3c93284fed358c2c8979cba4c139430527433c6/larch/xafs/fluo.py).
Larch identifies this method as suitable for XANES and questionable for EXAFS;
rexafs does not qualify its corrected branch for AUTOBK, EXAFS fitting or wavelets.
See the [official applicability discussion](https://xraypy.github.io/xraylarch/xafs_preedge.html#over-absorption-corrections).
Thin films, finite thickness, depth gradients and mixed emission from different
shells need a different qualified model. Detector dead-time correction is a
separate upstream operation.

## A spectrum and a small settings object

```rust,no_run
use rexafs::{FluorescenceCorrection, Spectrum};
# fn example(spectrum: &Spectrum) -> Result<(), Box<dyn std::error::Error>> {
let correction = FluorescenceCorrection::new("CuO", "Cu", "K")
    .line("Ka1")
    .angles(45.0, 45.0);
let mut corrected = spectrum.correct_fluorescence(&correction)?;
corrected.normalize()?;
let record = corrected.fluorescence_correction().unwrap();
# Ok(()) }
```

The angles are an **example**, not defaults: supply the measured incident and
exit angles, in that order, in degrees **from the sample surface**. Normal
incidence is 90°. Both angles must be greater than zero and at most 90°.
The formula describes the entire sample, including matrix/diluent atoms; selecting
Cu as the absorber does not imply that the sample is pure copper. No density is
needed because the common density factor cancels in this thick-sample ratio.

The call borrows the original spectrum and returns a new one. Internal
conventional normalization is automatic; users do not have to prepare `norm` or
`flat` arrays. Calling the operation on unknown acquisition provenance explicitly
interprets that input as fluorescence, and the result records that assumption.
Known transmission and already corrected lineages are rejected. A generic detector
ratio is not automatically labeled fluorescence because it may represent electron
yield. `set_absorption_mode` allows an explicit acquisition declaration. Native
reader transmission arithmetic attaches its known transmission interpretation.

The output contains corrected μ and no final normalization cache. `normalize()`
then uses the ordinary polynomial default, or a separately selected `MBack` model.
This separation avoids using corrected normalization to calculate its own input.
The result retains original energy/μ, internal normalization, geometry, formula,
atomic references, factor, denominator and diagnostics. Data edits preserve this
historical record on its own recorded energy grid; they do not rewrite it.
`record.definition()` returns resolved, version-pinned settings for exact replay.
`correction.apply(energy, mu)` is the equivalent array API for explicitly
interpreted fluorescence input.

## Python and TypeScript (0.2.10)

Use the same two-stage workflow: make a corrected spectrum, then normalize it.
The examples use illustrative 45° surface angles; replace them with the measured
geometry and the complete sample formula.

```python
from rexafs import FluorescenceCorrection

model = FluorescenceCorrection("CuO", "Cu", "K", line="Ka1", angles=(45, 45))
corrected = spectrum.correct_fluorescence(model)
corrected.normalize()
record = corrected.fluorescence_correction()
print(record.maximum_amplification, record.warnings)
```

Python copies settings and arrays, releases the global interpreter lock during
calculation, and returns a separate `Spectrum`. Result array properties are
independent NumPy copies. `record.internal` gives the internal conventional fit,
with copied lists; `record.atomic` gives the edge, emission and compound attenuation
evidence. `record.definition` returns settings pinned to resolved intervals/E₀ and
the atomic dataset. `record.to_json()` preserves the full native history.

```ts
import { FluorescenceCorrection } from "rexafs/node";

const model = new FluorescenceCorrection("CuO", "Cu", "K", {
  line: "Ka1", angles: [45, 45],
});
const corrected = spectrum.correct_fluorescence(model);
try {
  corrected.normalize();
  const record = corrected.fluorescence_correction();
  console.log(record?.maximum_amplification, record?.warnings);
} finally {
  corrected.free();
  model.free();
}
```

The TypeScript result owns ordinary JavaScript data and copied `Float64Array`
values; it needs no `free()`. Editing those copies never changes the retained
native spectrum, JSON history or replay definition. `record.definition` creates
an owned model; free it after use. Browser callers await `init()` first and should
run large synchronous calculations in a Worker.

Both languages expose `model.apply(energy, mu)` for direct arrays and
`spectrum.set_absorption_mode("transmission")` (or `"fluorescence"`/`"unknown"`)
for explicit acquisition interpretation. Python measurement-to-spectrum import
preserves known native transmission evidence. Reconstructing a new spectrum from
bare arrays has unknown provenance; prefer `correct_fluorescence` to retain the
correction record and processing restrictions. Changing the mode or editing an
already corrected spectrum does not erase that record or enable EXAFS processing.

The options `family`, `e0`, `pre_edge`, `post_edge` and `degree` have the same meanings
as the core settings below. Python requires keyword arguments for line/angles;
TypeScript requires them in its options object. Neither infers geometry. Inputs
and reference availability are validated by the native core. The adapters are
[`py-rexafs/src/fluorescence.rs`](../py-rexafs/src/fluorescence.rs),
[`crates/rexafs-wasm/src/fluorescence.rs`](../crates/rexafs-wasm/src/fluorescence.rs)
and [`js-rexafs/fluorescence.js`](../js-rexafs/fluorescence.js).

## Internal normalization and calculation

The default internal fit uses a line for the pre-edge and a degree-one post-edge
polynomial, no Victoreen exponent, and an independently fitted positive edge step.
Measured E₀ is detected before fitting unless supplied by the spectrum or settings.
The tabulated edge remains unshifted. Default intervals run from the first energy
to E₀−30 eV and from E₀+100 eV to the last energy. They must contain sufficient
points; short measurements fail instead of silently changing the interval.
`.pre_edge(-200.0..=-50.0)`, `.post_edge(100.0..=800.0)`, `.degree(2)` and
`.e0(8979.0)` make these choices explicit. Intervals are eV offsets from E₀.
An individual `.line("Ka1")` differs from `.line_family("Ka")`, whose energy is an
intensity-weighted mean of compatible lines originating at one shell.

Let a(E) be the total compound mass attenuation in cm²/g, E_f the emission energy,
E_edge the tabulated edge, n₀(E) the internally normalized uncorrected signal,
and θ_in/θ_out the measured surface angles. The named compatibility profile uses

```text
g = sin(θ_in) / sin(θ_out)
α = [a(E_f) g + a(E_edge − 10 eV)] / [a(E_edge + 10 eV) − a(E_edge − 10 eV)]
d(E) = α + 1 − n₀(E)
μ_corrected(E) = μ(E) α / d(E)
```

Angles are converted to radians for the sine. Both g and α are dimensionless;
α/d is a multiplicative factor preserving the units of μ. The three attenuation
samples, composition mass fractions and Elam total-attenuation provider identity
are saved. This is the constant-α ±10 eV profile, not a general thickness model.
See the [offline atomic-data record](../crates/rexafs/data/atomic/README.md) for
interpolation, dataset checksums and attribution. The implementation is
[`calculate::apply`](../crates/rexafs/src/xafs/fluorescence/calculate.rs);
[`Spectrum::correct_fluorescence`](../crates/rexafs/src/xafs/xasspectrum.rs)
retains the output lineage and protects unqualified downstream stages.

Rexafs requires a positive tabulated attenuation jump and a positive, unfloored
internal fitted edge step. In extremely dilute samples the matrix's attenuation
slope can make the ±10 eV net jump nonpositive: this profile then fails, even
though the physical dilute limit may need little correction. Invalid geometry,
unsupported atomic data, incomplete intervals and nonfinite results also fail.
No negative or tiny denominator is clipped to manufacture an output. The numerical
rejection threshold is `64 * f64::EPSILON * max(1, α+1)`; the complete result records
it. Engineering diagnostics flag a denominator below 5% of α+1, amplification
over 10, or either angle below 5°. These thresholds are tested numerical guardrails,
not confidence intervals or physically universal acceptance criteria. Noise and
uncertain angles/composition can be strongly amplified. Corrected-array uncertainty
is unavailable until dependence on the internal normalization is propagated.

## Desktop workflow (0.2.10)

Select uncorrected μ(E), then **Data → Fluorescence correction…** in the
processing tools. Enter the full sample composition (including its matrix),
absorber, edge, detected emission and measured incident/exit angles. Angles are
measured from the sample surface: normal incidence is 90°. No geometry or
composition is guessed. For emission, enter an individual line such as `Ka1`,
or explicitly select an unresolved family with `family:Ka`.

Confirm **Uncorrected fluorescence μ(E)** and select **Preview**. The main plot
compares the original and corrected arrays; **Factor** shows their multiplicative
correction. Known transmission imports remain rejected even after confirmation.
**Internal normalization** exposes optional measured E₀, pre/post intervals in eV
from E₀ and polynomial degree. Blank intervals use the core defaults above; the
resolved intervals and numerical warnings appear beside the preview.

**Add corrected spectrum →** retains the calculation before adding an independent
group and opens **Process** (the **Normalize** stage in 0.2.14 and earlier) for
final polynomial or MBACK processing. The input
group remains available. Normalization, energy-space LCF/PCA/MCR and peak fitting
can use the corrected group. Background, EXAFS transforms, wavelets and RMC must
use the uncorrected input. This restriction follows calculated LCF/MCR outputs,
merged groups, processing-tool results and duplicates. In builds after 0.2.14,
Process shows only the two μ(E) plots for such a group, with a notice, and its
Background and Transform sections show the XANES-only notice.

Open **Fluorescence correction…** on a corrected descendant to inspect its ancestor
calculation, open the original group when available, or export the numerical
history as JSON. Multiple ancestor records can be browsed individually. The
history contains original and corrected arrays, internal normalization, geometry,
composition, atomic-data identity and source/processing identities. It is not a
claim that later edited arrays are identical to that historical result. Embedded
projects retain compressed, checksum-checked history even when its local cache
is removed. Keep original measurements for independent scientific validation.

The [computer-use validation record](validation/2026-09-17-fluorescence/README.md)
includes synthetic screenshots and explicit test limits.

The implementation is in the [desktop correction controller](../crates/rexafs-gui/src/app/shell/fluorescence.rs),
[history storage](../crates/rexafs-gui/src/fluorescence_history.rs) and
[typed group preparation](../crates/rexafs-gui/src/params.rs).
The advanced Rust `Spectrum::restrict_to_xanes()` marker lets a frontend carry
this domain limit onto a calculated descendant without inventing a direct
correction record. `is_xanes_only()` reports either an inherited limit or a direct
native correction. The marker survives serialization and data edits, clears
EXAFS caches, and does not change arrays or normalization. Ordinary callers of
`correct_fluorescence()` receive the restriction automatically.

## Qualification

The [synthetic reference fixture](../crates/rexafs/tests/fixtures/analysis/fluorescence/larch-reference.json)
records pinned Larch source hashes, Python/XrayDB versions and explicit geometry,
line/family, degree, E₀ and intervals. Its generator loads unchanged numerical
function bodies and replaces only Larch's interactive group adapter/decorator.
CuO, dilute Cu in SiO₂ and Fe₂O₃ cases compare internal curves, corrected μ,
emission energies and attenuation. This is software agreement, not proof of
physical accuracy. Independent tests cover forward/inverse model recovery,
input-unit scaling, the valid weak-correction limit, near-singularity diagnostics,
invalid science, source immutability, repeat-correction rejection, project-style
serialization and separate polynomial/MBACK final normalization. Desktop regression
tests also cover embedded history recovery, transmission evidence in historical
imports and inherited LCF domain limits. Series/Live correction recipes and
matched experimental validation remain future work.
