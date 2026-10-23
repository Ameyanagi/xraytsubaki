# Cauchy wavelet analysis

Introduced in rexafs 0.2.10. The native core implements `cauchy_v1`;
the source desktop provides a map workspace and full-frame Series region trends.
Python and TypeScript expose the same native calculation and region statistics.
Live acquisition qualification remains pending. This page does
not claim that a released GUI already provides the complete workflow.

A wavelet map localizes EXAFS oscillations jointly in photoelectron wave number
k (Å⁻¹) and Fourier-like distance R (Å). It helps compare contributions with
different k dependence. The scientific motivation is
[Muñoz, Argoul and Farges (2003), *Continuous Cauchy wavelet transform analyses of EXAFS spectra: a qualitative approach*](https://doi.org/10.2138/am-2003-0423).
R features are not phase-corrected bond lengths; color intensity is not a
concentration or coordination number. [Larch's documentation](https://xraypy.github.io/xraylarch/xafs_wavelets.html)
provides a related Cauchy implementation and examples.

## Common API

```rust,no_run
use rexafs::{Spectrum, Wavelet};
# fn example(spectrum: &Spectrum) -> Result<(), Box<dyn std::error::Error>> {
let map = spectrum.wavelet(&Wavelet::new(2.0..=12.0))?;
let magnitude = map.magnitude();
let region = map.integral(4.0..=10.0, 1.0..=3.0)?;
let mean = map.mean(4.0..=10.0, 1.0..=3.0)?;
let maximum = map.maximum(4.0..=10.0, 1.0..=3.0)?;
# Ok(()) }
```

The selected k interval must be fully measured. Missing normalization and AUTOBK
run automatically on a copy, using the spectrum's settings; the source remains
unchanged. Existing unweighted χ(k) is reused. The ordinary Fourier transform
and its display window do not run implicitly. A native fluorescence-corrected
XANES-only branch is rejected as unqualified for EXAFS.

`Wavelet::new` defaults to weight 2, order 100, k sampling 0.05 Å⁻¹, R up to 6 Å
and no taper. These are rexafs starting choices, not universal optimal settings.
Use `.kweight(0)`, `.order(50)`, `.kstep(0.025)`, `.rmax(4.0)` or `.taper(1.0)` to
change named settings. Larger order narrows the frequency response and broadens
localization in k. Weighting emphasizes high-k oscillations and noise. The taper
uses half-cosine ramps of the supplied width **inside** both selected endpoints;
it is explicitly distinct from the historical FFT window-width convention.

The array API is `settings.calculate(k, chi)`, where χ is unweighted and
dimensionless. Original arrays, requested settings, prepared grid, support mask,
window and complex result remain available. `map.real()` and `map.imaginary()`
borrow immutable flattened arrays, with **rows=R and columns=k**. Cell (j,i) is at
`j * map.k().len() + i`. `map.magnitude()` returns an independent array.
`map.phase(0.01)` returns radians, masking cells below 1% of the map maximum;
zero-magnitude cells are always masked. This changes neither complex data nor
region measurements. Magnitude slices use `slice_at_k(k)` and `slice_at_r(r)`.
Serde JSON preserves the scientific map; deserialization checks method, dimensions,
axes, finite values and resource limits. It does not independently prove that an
external producer's numerical values are correct.

## Python and TypeScript (0.2.10)

The common call is `spectrum.wavelet(model)` in each language. Missing
normalization/background stages run on a private copy; users do not need to
prepare intermediate arrays. Specify the measured k interval in Å⁻¹, rather than
an energy interval relative to E₀.

```python
from rexafs import Wavelet

model = Wavelet((2, 12))
wavelet_map = spectrum.wavelet(model)
image = wavelet_map.magnitude       # NumPy matrix: R rows, k columns
region = wavelet_map.integral((4, 10), (1, 3))
print(region.value, region.unit)
```

Python options are named: `Wavelet((2, 12), kweight=2, rmax=4)`.
Array properties return independent NumPy copies. The transform and spectrum
preparation release the Python global interpreter lock. `phase()` returns a
matrix in radians, using `NaN` for zero/low-amplitude cells; its default floor is
1% of maximum magnitude. `wavelet_map.definition` returns independent settings,
and `preparation` describes automatic processing or is `None` for direct arrays.

```ts
import { Wavelet } from "rexafs/node";

const model = new Wavelet([2, 12]);
const map = spectrum.wavelet(model);
try {
  const image = map.magnitude;      // Flat Float64Array: R rows, k columns
  const [rows, columns] = map.shape;
  const region = map.integral([4, 10], [1, 3]);
  console.log(rows, columns, image[0], region.value, region.unit);
} finally {
  map.free();
  model.free();
}
```

TypeScript uses named options, for example `new Wavelet([2, 12], {rmax: 4})`.
Cell `(row, column)` is at `row * map.shape[1] + column`. Arrays are independent
copies and remain valid after the map is freed. Release every owned map and model
with `free()`, including a model obtained from `map.definition`. `preparation` is
`null` for array calculations. Browser applications import from `rexafs`, await
`init()` first, and should run synchronous calculations in a Worker.

Both bindings also provide `model.calculate(k, chi)` for original unweighted χ(k),
`model.estimate(k)` for dimensions/storage, `map.slice_at_k(k)` and
`map.slice_at_r(r)` for magnitude slices, and `to_json()`/`from_json()` for retained
definitions and maps. Python accepts one-dimensional numeric sequences or NumPy
arrays; TypeScript expects `Float64Array` inputs. Invalid coverage, settings,
dimensions or excessive resource requests fail with an error. Display sampling
and colors never enter a native region integral. All advanced settings have the
same meanings and defaults as the Rust API described above.

The adapters live in [`py-rexafs/src/wavelet.rs`](../py-rexafs/src/wavelet.rs),
[`crates/rexafs-wasm/src/wavelet.rs`](../crates/rexafs-wasm/src/wavelet.rs) and
[`js-rexafs/wavelet.js`](../js-rexafs/wavelet.js). They use the core calculation;
no separate Python or JavaScript numerical algorithm is introduced.

## Desktop workflow (0.2.10)

*Builds after 0.2.14:* Transform is a section of the **Process** stage. Its view
selector offers **Linked plots**, **χ(q)** and **Wavelet**; **Wavelet settings**
sits in the Transform section of the inspector. The rest of this section
describes the 0.2.10 layout.

Select a spectrum, then **Transform → Wavelet** in the shared
**k · R · k + R · q · Wavelet** view selector. The selector stays visible in
every Transform view. In Wavelet, choose **Magnitude**, **Real**, **Imaginary** or
**Phase** immediately to the right of the view selector. These display choices
preserve the calculated map. **Wavelet settings** is a collapsed section between
**Forward FT** and **Back FT** in the ordinary parameter panel. Selecting Wavelet
opens that section; its number fields behave like the Fourier settings.
Switching views preserves the current wavelet map and display settings while the spectrum and processing settings stay
unchanged; returning does not recalculate the map. Opening Wavelet calculates a
missing map automatically. Committing a parameter edit (Enter, Tab or leaving
the field), or clicking a number stepper, updates the map after a 200 ms pause.
Calculation runs in the background; further edits cancel obsolete work, and only
the latest result can replace the displayed map. Spectrum and processing changes
also update the open Wavelet view. Set a fully measured k interval.
Weight 2 and R up to 6 Å are starting values;
**Advanced** exposes order, sampling and taper. Missing normalization/background
processing uses the current spectrum settings on a copy. Confirmed unweighted
χ(k) groups reuse their original arrays without normalization or AUTOBK. Unknown
quantities must be confirmed first. A failed calculation
leaves the previous retained result intact; its fields must not be mistaken for
newly calculated data. Progress and errors appear beside **Spectra / Slices**;
hover to read a long message. A completed calculation adds no status header.

The k–R map sits above the k spectrum and to the right of the R spectrum.
**Spectra** shows the original k-weighted χ below the map and the ordinary
Fourier magnitude to its left, with R vertical and zero amplitude toward the
map. The plotting areas share the same k and R coordinates, including when
resizing the workspace. Panning or zooming any panel updates the matching
physical axis in its neighbor; intensity axes remain independent. **Reset View**
on the map resets both shared axes. These are display changes and do not change
the calculated map. The linked Fourier calculation shares the
selected k interval and weight, and retains the current Fourier window/settings;
its window name is shown. That window does not change the wavelet's own taper.
**Slices**, or a click on the map, shows magnitude versus k at the selected R and
magnitude versus R at the selected k. The readout gives physical coordinates and
bilinearly interpolated native magnitude. Slices use the same aligned layout
and linked axes. R values are not phase-corrected. The layout and viewport links
are implemented in [`wavelet/layout.rs`](../crates/rexafs-gui/src/app/shell/wavelet/layout.rs);
[`wavelet/plots.rs`](../crates/rexafs-gui/src/app/shell/wavelet/plots.rs) constructs
the spectra and map with matching margins.

Region measurements belong to **Series → Add trend**, described below. Transform
contains only map settings and inspection controls.

**Colors**, at the top right beside **Export**, chooses a palette and direction.
Its menu also contains **Lock scale**. Magnitude has a zero-based scale;
real/imaginary views have symmetric scales. Phase uses radians and hides low
amplitude (below 1% of the native maximum); an entirely masked map has empty axes.
The display texture has at most 512 k columns and 256 R rows, interpolated in
physical coordinates, including nonuniform retained R grids. Magnitude uses
bilinear native magnitudes; phase uses the interpolated complex value. Neither
that texture nor display colors enter the region integral.

**Lock scale** keeps the numerical color limits across maps with identical
processing and wavelet definitions. A change of component, processing or transform
definition resets the lock. This is a color-limit control, not a qualified batch
comparison workflow. Recalculating a map resets its plot bounds; changing colors
preserves an interactive zoom. Switching between spectra and slices resets the
lower plots because their amplitude units differ.

The toolbar no longer has a History menu. Each group's latest automatic preview
is retained for project saving, without accumulating an entry for every edit.
Maps loaded from older projects and historical region references are preserved.
The refresh icon explicitly rereads the source, for example after an external
file changes. A reused saved map records its original calculation inputs; reopening
it does not assert that a linked source file is unchanged.

**Export → Full map and provenance · JSON** writes the complete complex map,
original k/χ, preparation settings and linked Fourier result. Historical saved
regions remain in project artifacts and JSON exports. **Data · CSV** streams the
full native k–R grid, real and imaginary values, magnitude, support flag and
k-weight. Complex values and magnitude have units of k^weight χ. Padding is
identified by the support flag; R is not phase corrected. CSV uses the numerical
map, independent of the display texture or palette. **Image · PNG** and
**Vector image · SVG** export the current map panel. JSON serialization is
streamed without constructing a duplicate value tree.

Project saves retain small receipts; **Include source files** embeds their
checksum-verified compressed artifacts too. Reopening restores the numerical map
even if the original artifact cache is unavailable. The desktop keeps one
scientific map resident; full-series map retention is not implemented. Desktop
rendering requires at least two R rows.

### Series region trends

1. In Transform, set the measured k range, weight and other Wavelet settings.
2. Open a series and choose **Add trend → Wavelet**. This copies the current
   Wavelet settings into the trend definition. Choose **Integral**, **Maximum**
   or **Mean**; each measures the map's magnitude.
3. Drag any of the four rectangle edges in the preview, or enter the k bounds
   (Å⁻¹) and R bounds (Å). The value updates from the native map. Use the frame
   arrows to inspect the same region on another spectrum.
4. Choose **Calculate all … frames**. Results include every requested frame,
   with explicit failed rows when data do not cover the requested range.

**Use Transform settings** explicitly refreshes the copied settings. Existing
region bounds are kept when they remain inside the new extent; otherwise the
editor initializes a new region inside it. Later Transform edits never change a
saved trend. Its definition retains the transform, statistic and both intervals.
Each result retains source identity, preparation choices, units and numerical
method. A recipe additionally freezes processing choices for replay. Missing
normalization/AUTOBK runs on a copy; confirmed χ(k) groups reuse their original
arrays. Workers calculate one frame map at a time, retain scalar outcomes, and
recalculate only the selected preview on demand.

CSV exports contain the k bounds in `range_start`/`range_end`, explicit R bounds,
the statistic, method and serialized transform definition. Project saves retain
these outcomes and revisions. Historical scalar-only runs remain readable.
**Hide plot ranges** hides the preview rectangle without changing its bounds or
results. In ordinary processing plots this icon sits between **Colors** and
**Overview plots**; Add trend places it beside the preview controls.

The retained data model and worker are implemented in
[`series_measurements/wavelet.rs`](../crates/rexafs-gui/src/series_measurements/wavelet.rs)
and [`series_measurements.rs`](../crates/rexafs-gui/src/series_measurements.rs).
This Series workflow is introduced in 0.2.10; Live acquisition and native Windows/Linux
qualification require further checks.

See the [updated computer-use validation](validation/2026-09-18-wavelet-trends/README.md).
The [original map check](validation/2026-09-17-wavelet/README.md) is historical.

## Numerical convention

The native implementation is
[`wavelet::calculate`](../crates/rexafs/src/xafs/wavelet/calculate.rs).
It linearly resamples onto a uniform zero-origin k grid with spacing δk. Only the
selected measured support contributes; values outside it are recorded zero
padding, not extrapolated observations. The grid ends at the last represented
uniform point within the original k extent. Define

```text
u_j = k_j^w χ(k_j) window(k_j)
U_l = sum_j u_j exp(−2π i j l / L)
ω_l = 2π l / (L δk)
a_R = m / (2R)
H_l(R) = [2π / Γ(m+1)] (a_R ω_l)^m exp(−a_R ω_l),  0 < l < L/2
W_j(R) = (1/L) sum_l U_l H_l(R) exp(+2π i j l / L)
```

Here j indexes k samples, l indexes discrete frequency, L is FFT length, w is k
weight and m is positive integer order. Γ(m+1)=m!. δk and a_R have Å⁻¹ units;
ω and R have Å units, so a_Rω is dimensionless. DC, Nyquist and negative-frequency
filter bins are zero. The filter peaks at ω=2R, matching the EXAFS 2kR coordinate.
The FFT has no forward multiplier and the inverse has factor 1/L. W therefore
has numerical units of k^wχ, unlike the ordinary XAFS Fourier transform's scaling.
The filter is evaluated in the log domain to avoid factorial/power overflow.
The existing easyfft/RustFFT dependency supplies Fourier transforms; no Python
runtime or new numerical dependency is required.

This discrete filter, explicit support and fixed order define rexafs
`cauchy_v1`. R=0 is excluded as singular. The default FFT is the smallest power
of two at least twice the prepared grid length. A requested shorter FFT fails;
measured data are never silently truncated. Default R spacing is π/(Lδk),
numerical sampling rather than independent spatial resolution. `.rstep(...)`
changes it; `.radii(vec![...])` supplies exact positive increasing coordinates
for comparisons. R must remain below the k-sampling Nyquist radius π/(2δk).
Extending R at fixed order/grid leaves overlapping values identical. Cropping,
color scaling and image resolution must operate on a separate display view.
Finite support and padding still produce boundary artifacts; inspect sensitivity
to the chosen support and taper.

The [pinned Larch implementation](https://github.com/xraypy/xraylarch/blob/e3c93284fed358c2c8979cba4c139430527433c6/larch/xafs/cauchy_wavelet.py)
couples order to the number of R rows and uses an actual FFT length twice its
`nfft` argument. Thus reference tests supply its exact order, coordinates and
actual length explicitly. They establish matched-convention agreement, not
identity between the two applications' defaults. The reference source preserves
historical Univ. Marne la Vallee/Hans–Argoul/Argoul–Muñoz/Farges attribution;
this is recorded with the fixture and upstream license.

## Region measurements and bounded calculation

`map.integral(k_range, r_range)` integrates the bilinear surface through **native
magnitude samples** over a fully covered rectangle. It inserts exact requested
boundaries and sums the integrals of clipped bilinear cells. k must also lie
within selected measured support; phase is not an integral metric. The value
has units of |W| times Å⁻¹ times Å, which reduce to those of k^wχ. No uncertainty
is supplied. [`wavelet::map`](../crates/rexafs/src/xafs/wavelet/map.rs) implements
these statistics, slices, immutable access and deserialization validation.

`map.mean(k_range, r_range)` divides that integral by the rectangle's area,
Δk × ΔR. It is an area-weighted mean, not an average of array cells, so unequal
R spacing does not bias it. `map.maximum(k_range, r_range)` finds the largest
value on the same clipped bilinear surface, including exact region boundaries.
A bilinear cell reaches its maximum at a corner; the implementation checks the
corners of every intersected cell. Both operations retain the same bounds and
units, with distinct method identifiers. These are rexafs measurement conventions,
not concentration estimates or uncertainty calculations. All three methods are
available with the same arguments in Rust, Python and TypeScript.

`settings.estimate(k)` checks dimensions and estimates buffer size before any
transform. Limits are 1 million original samples, 32768 prepared k samples/R rows,
262144 FFT points, 4 million complex cells and 64 million R-rows-times-FFT-points
of filtering work. Invalid grids fail before large allocation. Original-input
copies, optional preparation metadata, FFT scratch and serialization add overhead
to the reported scientific buffer size. A map is calculated one R row at a time;
`calculate_with_cancel` checks a callback between rows and discards partial output
on cancellation. `spectrum.wavelet_with_cancel` checks before/after prerequisite
processing too; it does not interrupt an ongoing AUTOBK solve.

The Series worker follows this bounded-residency contract: every requested frame
is processed, scalar results and inputs/settings are retained, and selected maps
are recomputed on demand. This does not qualify Live acquisition by itself.

## Qualification

The [experimental Larch comparisons](../crates/rexafs/tests/fixtures/analysis/experimental-larch/README.md)
use two measured Cu foils and an Aichi RuO₂ spectrum. Both transforms receive
the same independently Larch-prepared χ(k); all 36,530 complex cells in each
map are compared. This isolates wavelet conventions from background-removal
differences and does not establish AUTOBK equivalence. Source licenses, numerical
settings, generator hashes, measured differences and tolerances are retained.

Tests compare Fourier sign, axes and amplitude with independent direct DFT sums;
localize a synthetic packet near its known k/R position; verify identical values
under fixed-order R extension; and exercise irregular interpolation, explicit
padding/taper, cancellation, zero amplitude, invalid grids and memory limits.
An analytic bilinear surface checks region integration with non-grid boundaries.
The high-level spectrum test verifies automatic preparation, unchanged input,
retained preparation settings and replay from the original k/χ arrays.

The [synthetic Larch fixture](../crates/rexafs/tests/fixtures/analysis/wavelet/README.md)
contains two matched-grid cases with distinct weights/orders, original source
hash and package version. Test fixtures stay in Git and are excluded from crate
archives. No unpublished experimental data are used. Experimental interpretation,
platform GUI workflows and display/series integration require their own checks.
