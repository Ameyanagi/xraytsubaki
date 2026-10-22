---
title: "Processing controls"
description: "Use normalization, AUTOBK, forward and inverse transforms."
audience: user
---

This guide describes **rexafs 0.2.14**. Screenshots identify the version that
produced them and use public Cu measurements. They are
full, unedited windows; select an image for full resolution. See
[capture provenance](/licenses/#desktop-0211-workflow-captures) and
[available downloads](/docs/getting-started/install/).

Loading an absorption spectrum or changing its settings runs **normalization →
AUTOBK → forward Fourier transform → inverse transform**. Selecting a stage
changes the display and controls. The inverse transform runs even with its
controls collapsed, so invalid back-transform settings can fail processing.
See the [0.2.10 desktop pipeline](https://github.com/Ameyanagi/rexafs/blob/v0.2.10/crates/rexafs-gui/src/params.rs).

The inspector edits the **current** group. **Apply to N** copies only the selected
processing stage to eligible marked groups, excluding the current group and
processing-locked groups. **Reset** restores that stage from the project's
defaults. Copying all processing settings does not also copy column mappings or
reference calibration; copying a column mapping is a separate guarded action.
An Auto request is copied as Auto, so its resolved value can differ by spectrum.

Since 0.2.12, automatic values appear below their input boxes, with
units, so the full value or range description remains readable. Invalid numeric
input stays editable and shows an explanation beside the field. The last
committed value remains in use until a valid entry is committed. Choose
**Restore previous value** to cancel the edit, or **Use Auto** when an automatic
value is available. This behavior is shared by the
[numeric field widget](https://github.com/Ameyanagi/rexafs/blob/v0.2.13/crates/rexafs-gui/src/widgets/numeric_field.rs).

[![Normalization controls with full automatic values displayed below their fields](/screenshots/0.2.12/numeric-auto.jpg)](/screenshots/0.2.12/numeric-auto.jpg)

*Signed 0.2.12 release: automatic range values remain readable below their fields.
This full unedited window uses the public room-temperature Cu measurement;
[capture provenance](/licenses/#desktop-0212-release-captures).*

<a id="plot-axis-ranges-unreleased"></a>

## Plot axis ranges

From 0.2.14, right-click a plot and choose **Axis range…**. X and Y each have independent
minimum and maximum fields. Leave a field blank or type **Auto** to let that
endpoint follow the data. For a zero baseline, enter `0` for **Y minimum** and
leave **Y maximum** automatic, then choose **Apply**.

**All Auto** clears the fields; **Reset axes to Auto** in the plot menu restores
natural bounds immediately. Values use the displayed axis units. Invalid
ranges show an explanation and leave the previous view unchanged. Axis ranges
change the display only; processing ranges, fits and source data are unaffected.
Live and Series fit plots retain ranges across frame updates in the current
application session. See the
[axis range guide](https://github.com/Ameyanagi/rexafs/blob/v0.2.14/doc/plot-axis-ranges.md).

[![Axis range dialog in signed rexafs 0.2.14 with Y minimum zero and the other endpoints set to Auto](/screenshots/0.2.14/axis-editor-dialog.png)](/screenshots/0.2.14/axis-editor-dialog.png)

*Complete native dialog from signed 0.2.14. This example fixes only the lower
Y endpoint. [Capture provenance](/licenses/#desktop-0214-release-captures).*

## Alignment

In **Data → Align to reference**, choose a standard. The preview shows dμ/dE
over the alignment window, initially −50 to +100 eV relative to the reference
E₀. Original, aligned and reference derivatives retain their own energy grids;
each is divided by its largest absolute derivative in that window for visual
comparison. This display scaling does not change the absorption arrays or fit.

**Manual shift (eV)** adds to the automatic correction. Use the arrows for
0.1 eV steps, or enter a value; positive values move the aligned spectrum toward
higher energy. Zero keeps the automatic result. The preview updates after each
committed edit. **Apply offset** saves the total energy offset on the current
group, retaining the original arrays and recording the reference, fit window,
automatic correction and manual adjustment.

In **Data → Source**, **Offset (eV)** is the active, editable energy correction:
the displayed energy is the original energy plus this constant. **Zero** removes
it; entering the same value twice does not accumulate shifts. Explicit
normalization and background E₀ settings move with the change in offset, and
processing caches are invalidated. Undo restores the previous setting. Project
files retain the offset and alignment record, including when the offset is reset.
An older group's correction already stored in its arrays, or separate
reference-channel calibration, is not removed by this control.

The automatic alignment uses the core derivative-matching implementation in
[`align_to`](https://github.com/Ameyanagi/rexafs/blob/v0.2.10/crates/rexafs/src/xafs/xasspectrum.rs),
with the manual correction applied afterward. See the
[processing theory](/docs/science/processing/) for algorithm details.

Rust users can set and read the correction directly on `Spectrum` since 0.2.10:

```rust
spectrum.set_energy_offset(3.5)?; // total offset in eV
let offset_ev = spectrum.energy_offset();
spectrum.set_energy_offset(0.0)?; // remove the recorded correction
```

No settings or history struct is required for this operation. The setter uses
the existing `energy_shift` state, adjusts both energy grids and E₀ settings,
and invalidates derived results when the offset changes. `shift_energy(delta)`
remains available for incremental corrections. The GUI uses the same checked
core setter on an owned source-axis buffer, retaining original project data.
In a standalone core spectrum, zero removes recorded constant shifts to
floating-point precision; it does not undo truncation, smoothing or other data
edits. Replacing arrays with `set_spectrum` starts a new baseline at zero offset.
Python and TypeScript exposure of these energy-offset methods remains pending.

## Normalization

Choose **Polynomial** or **MBACK** in the Normalize sidebar.
MBACK shows absorber, edge and optional erfc-background
settings in that same panel. A declared source absorber/edge lets it calculate
on selection; otherwise enter them and choose **Apply**. The ordinary plot and
range handles stay visible. **Atomic match…** and **Compare methods…** open
optional diagnostics within Normalize. See the [MBACK guide](https://github.com/Ameyanagi/rexafs/blob/v0.2.10/doc/mback-normalization.md)
for the model, assumptions and retained results.
The fit toggle follows the selected method: **Pre/post** shows polynomial
baselines; **MBACK fit** shows the complete fitted atomic model over the
measured μ(E), without auxiliary pre/post lines. Enabling the toggle selects
μ(E), so the fitted curve shares the original absorption units. Plot/data
exports include the fit while visible.
These fits are limited to Normalize; Background shows only its **Spline**
overlay. Switching tabs preserves each toggle's preference.

[![Polynomial normalization with the Pre/post overlay enabled](/screenshots/0.2.11/polynomial.jpg)](/screenshots/0.2.11/polynomial.jpg)

*Polynomial: Pre/post shows the two fitted baselines over measured μ(E).*

[![MBACK normalization with automatically detected Cu and K edge and the complete fitted curve](/screenshots/0.2.11/mback.jpg)](/screenshots/0.2.11/mback.jpg)

*MBACK: the orange curve is the complete fitted atomic model, including its
background terms. Cu and K were read from this XDI file's header; check these
choices for your own measurement. A fit overlay is a diagnostic, not proof of
an appropriate normalization model.*

The range icon between **Colors** and **Overview plots**
in the plot toolbar toggles shaded
selection windows and their drag handles across processing, fitting and analysis
plots. Hiding them preserves the numerical ranges and processing results. The
icon stays in place; its tooltip changes between **Hide plot ranges** and
**Show plot ranges**. The separate **Window** control in Transform displays the
Fourier taper curve.

Set $E_0$ or leave it automatic. Pre/post-edge range controls are offsets from $E_0$
in eV. Choose a meaningful baseline on either side of the edge. **norm** subtracts
the pre-edge line and divides by the edge step; **flat** also removes the fitted
post-edge trend. Inspect the curve rather than treating automatic settings as
proof of a valid baseline.

Blank fields select Auto; resolved values appear below the input boxes. New
desktop analyses start with these choices:

| Setting | Starting value |
|---|---|
| Pre-edge range, relative to $E_0$ | −200 to −30 eV |
| Post-edge range | $E_0+150$ eV to the measured energy endpoint |
| Post-edge polynomial order | 2 |
| Victoreen exponent | 0 |
| Edge step | Fitted, unless a positive measured override is entered |

Ranges adapt to available data; short spectra can need manual choices. Desktop
Auto applies these choices before calling the core library, whose unset fields
can resolve differently. Retain resolved settings when comparing interfaces or
other software.

*Unreleased (planned for 0.2.15):* a range that reaches beyond a short scan is
limited to the measured data when processing runs, following Larch: the pre-edge
start is raised to the first energy, the post-edge end lowered to the last, and a
window left without data falls back to Larch's automatic value. An entered value
is never rewritten, so longer scans keep it. The used value appears in amber under
an entered field, with one warning per group in the Problems drawer; an Auto field
shows the used value in its placeholder. Background k max and the Wavelet k range
are limited the same way. A reversed range, or one with fewer than two measured
points, stops processing with an error naming the setting. See
[processing theory](https://github.com/Ameyanagi/rexafs/blob/dev/doc/processing-theory.md)
for the exact rules.

*Unreleased (planned for 0.2.15):* the Problems drawer, opened from the status
bar or **Show problems** in the command palette, also lists a degenerate
normalization, a failed Wavelet map and XANES peak-fit failures or warnings. Each
kind appears at most once per group, keyed by its full file path, and clears when
a later calculation succeeds. Normalization is degenerate when the post-edge and
pre-edge lines meet at E₀, so the edge step is about zero and normalized data
are meaningless; move E₀ onto the edge or widen the pre-edge and normalization
ranges.

## Background

AUTOBK estimates the smooth background and produces $\chi(k)$. Begin with
$R_\mathrm{bkg}=1$ Å, the fixed endpoint penalty 0.001 and the direct solver.
The background objective uses $k$ weight 1; display/forward-transform weighting is
separate. **Link to FFT** is an explicit choice to relate weights.

The default background grid spacing is 0.05 Å⁻¹ and its FFT length is 2048.
The background uses a Hanning window with $dk=0.1$ Å⁻¹; this is separate from the
forward-transform window. The fixed endpoint penalty is a rexafs choice described
in the [AUTOBK objective](/docs/science/autobk/). Older projects preserve their
saved clamp policy, so check it before comparing their results with a new project.

**bkg k min** and **bkg k max** set the photoelectron wave-number range, in Å⁻¹,
over which AUTOBK fits its spline. Drag their handles on χ(k), or, in an
unreleased build after 0.2.14, on the Background **μ(E)** view. The μ(E) handles
sit at the photon energy

$$E = E_0 + \frac{\hbar^2 k^2}{2 m_e} \approx E_0 + 3.81\ \text{eV·Å}^2 \times k^2,$$

where $E_0$ is the edge energy AUTOBK used to define $k$, $\hbar$ is the reduced
Planck constant and $m_e$ is the electron mass
([Newville 2014](https://doi.org/10.2138/rmg.2014.78.2)). The drag readout shows
both units, for example `k 13.5 Å⁻¹ · 9030 eV`. Use this view to stop the
background before a feature that is visible in energy, such as a second
absorption edge inside the scan. The stored value remains $k$, so the energy of
the cut moves when $E_0$ changes. Handles cannot go below $k=0$ or beyond the
last measured energy. See the [handle
conversion](https://github.com/Ameyanagi/rexafs/blob/dev/crates/rexafs-gui/src/app/shell/handles.rs),
which uses the core crate's `KTOE`/`ETOK` constants.

**Background options → Load standard…** accepts an optional χ(k) reference.
Supply exactly two numeric columns: increasing, nonnegative k in Å⁻¹ and matching
finite, unweighted, dimensionless χ(k). Separate columns with whitespace or
commas; begin comments with `#` or `*`. At least two points are required, and
extra columns or malformed rows produce an error. This input is a reference for
the background objective, not a new μ(E) spectrum or an already-windowed Fourier
curve. Its arrays are stored inside the project. Leave it unset for the normal
starting workflow; use **Clear standard** to remove it. See the [standard
reader](https://github.com/Ameyanagi/rexafs/blob/v0.2.10/crates/rexafs-gui/src/params.rs)
and [AUTOBK objective](/docs/science/autobk/).


[![Background showing measured Cu absorption, one AUTOBK spline and the Fourier magnitude below](/screenshots/0.2.11/background.jpg)](/screenshots/0.2.11/background.jpg)

*Spline controls the orange AUTOBK overlay. Normalize's polynomial and MBACK
fit overlays are not drawn in this stage.*


## Forward transform

The Transform view selector contains
**k**, **R**, **k + R**, **q**, and **Wavelet**. It remains visible in all five
views. In Wavelet, **Magnitude**, **Real**, **Imaginary** and **Phase** appear
immediately to the right of that selector. Switching back to Wavelet preserves
its map when the spectrum and processing settings are unchanged.
**Wavelet settings** sits between Forward
FT and Back FT in the parameter panel. It is collapsed initially and opens when
you select Wavelet. The map updates automatically after committed parameter edits
and stepper clicks; calculations run in the background after a short pause.
**Colors** and **Export** are at the top right. Region measurements live in
[**Series → Add trend → Wavelet**](/docs/desktop/series/#measure-a-wavelet-region).
The wavelet map has the k spectrum below
it and the R spectrum to its left. Their plotting areas align, and zooming or
panning shares the matching k or R axis. **Spectra** shows the measured χ and
Fourier magnitude; **Slices** shows wavelet magnitude at a selected coordinate.
See the [wavelet guide](https://github.com/Ameyanagi/rexafs/blob/v0.2.10/doc/wavelet-analysis.md)
for interpretation, settings, retained maps and region integration.

[![Wavelet magnitude with linked R spectrum on the left, k spectrum below and settings in the Transform sidebar](/screenshots/0.2.11/wavelet.jpg)](/screenshots/0.2.11/wavelet.jpg)

*Cu foil wavelet magnitude, k = 2–12 Å⁻¹, k weight 2 and R maximum 6 Å.
R is not phase-corrected; a map maximum is not directly a bond distance.
Changing R maximum to 5 Å updated the map automatically during the GUI check.*

The recommended starting window is 2–15 Å⁻¹ with $k$ weight 2, taper parameter
$dk=1$ Å⁻¹ and a Kaiser–Bessel window. Shorten the range for a noisier or narrower
measurement. View **k**, **R**, or **k + R**, and enable real/imaginary components
where available. Keep enough samples in the fitted window for a meaningful fit.

The forward transform uses 2048 points and infers its k step from the background
grid unless you override it. Its explicit amplitude multiplier is
$\delta k/\sqrt{\pi}$ after an unnormalized forward FFT; increasing padding
refines the displayed R grid without increasing experimental resolution. For
Kaiser–Bessel, $dk$ also affects the window shape, so it is not only a taper width.
The [processing equations](/docs/science/processing/) define the units and scaling.

**Advanced → Larch grid** selects an explicit alternative sampling convention.
It does not change the physical model. See
[Fourier compatibility](/docs/science/fourier-compatibility/).

## Back transform

Expand **Back FT R → q** and choose an R window to isolate a contribution.
The automatic q step follows the R sampling and inverse FFT length. Changing the
maximum q display extent does not independently set its spacing.

The filtered signal retains the weighting/window applied before the forward
transform; it is not generally the original unweighted $\chi(k)$.
[Processing theory](/docs/science/processing/) defines the normalization and
Fourier signs, scales, units and limitations.

## Fluorescence correction

Select uncorrected μ(E), then **Data → Fluorescence correction…**. Enter the full
sample composition, absorber, edge, emission line and measured incident/exit
angles in degrees from the sample surface. For example, `Ka1` selects one line;
`family:Ka` explicitly selects an unresolved family. Geometry has no defaults.

Confirm the fluorescence input and select **Preview** to compare original and
corrected μ(E). **Factor** shows the amplification. **Internal normalization**
exposes optional E₀ and pre/post intervals in eV from E₀; **Calculation notes**
contains the applicability and numerical diagnostics. The default internal
post-edge polynomial is linear. It is separate from final normalization.

**Add corrected spectrum →** retains the original and creates a new group, then
opens **Normalize** for independent polynomial or MBACK processing. Reopen the
correction tool on that group to inspect or export its historical calculation.
Choose **Include source files** when saving a portable project to include the
correction history and original arrays.

This homogeneous, optically thick model is limited to XANES, following the
[Larch applicability guidance](https://xraypy.github.io/xraylarch/xafs_preedge.html#over-absorption-corrections).
Known transmission imports and repeated correction are rejected. Corrected groups
and their calculated descendants retain this limit: use the original spectrum
for EXAFS background removal, transforms, fitting or wavelets. Energy-space
LCF/PCA/MCR remain available. See the
[Python](/docs/reference/stable/python/fluorescencecorrection/) and
[TypeScript](/docs/reference/stable/typescript/fluorescencecorrection/) references
for the same native calculation outside the desktop.

## Exporting a displayed plot

Use **Export** on a processing or comparison plot, an LCF/PCA/MCR result, a Series
heatmap/frame/trend, or the Wavelet toolbar. Choose **Data · CSV**, **Image · PNG**,
or **Vector image · SVG**. Plot images retain the current axis bounds.

Comparison CSVs contain every displayed spectrum, with its complete label, axis
labels and independent x/y coordinates. Different grids are not interpolated to
a shared grid. Zoom does not trim the exported curves. A sampled preview exports
its plotted subset; choose **Plot all** before exporting the full comparison.
CSV uses a long table (`series_index`, `series`, `point`, `x_label`, `y_label`, `x`, `y`), with blank
cells for missing values. Display offsets in waterfall and residual plots are
retained. PCA and MCR diagnostic values retain true zeros rather than the small
positive floor used to display them on a logarithmic axis.

Series heatmap CSVs contain the overview's sampled rows, identified by their
one-based frame labels. Difference mode exports the differences and identifies
the reference in the value label. Trend coordinates retain the plot's zero-based
frame axis. Use the existing Series result export for complete per-frame
measurement tables. Wavelet CSVs contain the native grid and complex values; the
additional JSON option includes original arrays and processing provenance.
