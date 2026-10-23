---
title: "Series and trends"
description: "Browse scan frames and distinguish sampled overviews from complete calculations."
audience: user
---

This guide describes **rexafs 0.2.14**. The retained 0.2.11 screenshots were captured
through computer use from the signed macOS release, using two
public Cu foil measurements. These are a small workflow example, **not a time series** or a
controlled temperature experiment. See
[data and capture provenance](/licenses/#desktop-0211-workflow-captures).

## Start with the overview

Open **Series**, then choose a folder scan or **Use loaded groups** for spectra
already in your project. The source menu lists folder scans and named series.
The heatmap, selected spectrum and trend share the frame cursor.

Choose **flat μ(E)**, **norm μ(E)**, weighted **χ(k)** or **|χ(R)|**.
Click a heatmap row or use the scrubber to select a frame; **Left/Right** moves
between frames when the plot has focus. K-space plots center zero with symmetric
vertical limits; large glitches remain visible.

## Add a measured trend

1. Choose **Add trend…**, then a metric: Point, Maximum, Integral, Mean or Edge energy.
2. Choose the signal and its interval. New desktop trends start with **Flat**,
   **0–30 eV from each frame's E₀**; the core API defaults to Norm.
3. Drag the shaded boundaries or type **From** and **To**. Use the range icon
   to show or hide these handles without changing the interval.
4. Review several frames with **Left/Right**, then choose **Calculate all N frames**.

[![Add trend showing flattened Cu absorption and draggable interval boundaries](/screenshots/0.2.11/series-trend.jpg)](/screenshots/0.2.11/series-trend.jpg)

*Maximum over 0–30 eV from E₀. Changing the preview frame does not change the
interval. Arrow keys in a focused numeric field still edit that field.*

**Results…** holds saved runs, per-frame failures and CSV/JSON export.
**Advanced** in the trend editor holds ordering, coordinates, presets, recipes
and recovery. Saved trends retain their settings: changing the overview's
representation or difference display does not recalculate them.

<a id="exafs-parameter-trends-unreleased"></a>

## EXAFS parameter trends

Version 0.2.14 adds **Fit** beside the spectral views. Choose
**Add trend… → EXAFS fit…**, select a reviewed single-spectrum fit, and choose
**Fit all frames**. Each frame uses the saved model and starting values.
Processing can follow each spectrum or copy the current spectrum's settings.
The comparison and frame controls remain available while fits run in the background.

Select any model variable or path distance in the trend selector. **Expression…**
adds a derived quantity such as `reff + dr_1`, using the selected path's FEFF
reference half-path length. Supply its display unit; units are not inferred for
arbitrary variables or expressions. Error bars show ±1 local standard error,
propagated with the fit covariance, including parameter correlations. FEFF
geometry is treated as exact. Missing uncertainty is not replaced with zero;
failed or unconverged frames leave gaps.

Changing the displayed expression does not rerun the fits. Save the project to
retain full results and custom trends. Runs cover the series membership at start;
incoming Live frames require a new Series run. See the
[source guide](https://github.com/Ameyanagi/rexafs/blob/v0.2.14/doc/series-fitting.md)
for assumptions and error-bar interpretation. The earlier release screenshots
above retain their historical build identities.

[![Signed rexafs 0.2.14 showing the reopened Cu fit and a distance trend with error bars](/screenshots/0.2.14/series-restored.png)](/screenshots/0.2.14/series-restored.png)

*Signed 0.2.14: the saved `reff + dr_1` expression and fit survive reopening.
These two frames repeat a prepared Cu reference; they do not demonstrate a
physical time trend. [Input and capture provenance](/licenses/#desktop-0214-release-captures).*

## Measure a wavelet region

Set up the map in [**Process → Wavelet**](/docs/desktop/processing/#forward-transform)
(**Transform → Wavelet** in 0.2.14 and earlier),
then open **Series → Add trend… → Wavelet**. Choose **Use Transform settings**
to copy that setup from the Transform section. Select **Integral**, **Maximum**
or **Mean**, then drag the k–R rectangle or enter its four bounds. k is in Å⁻¹ and R is in Å.

[![Series wavelet integral with a selected k–R rectangle and Calculate all frames](/screenshots/0.2.11/series-wavelet.jpg)](/screenshots/0.2.11/series-wavelet.jpg)

*The example integrates wavelet magnitude over k = 4–10 Å⁻¹ and R = 1–3 Å.
Both frames completed. A saved trend retains the transform settings and rectangle.*

These measurements use native wavelet magnitude, not the rendered colors.
Integral measures the area under the bilinearly interpolated magnitude surface;
Mean divides it by the rectangle area; Maximum finds its largest value in the
region. R is not phase-corrected. These are signal metrics, not direct coordination
numbers or concentrations. See the [method and units](https://github.com/Ameyanagi/rexafs/blob/v0.2.10/doc/wavelet-analysis.md#region-measurements-and-bounded-calculation).

## Compare against a reference frame

**Difference** subtracts one fixed reference from the heatmap and selected
spectrum. **Reference… / Ref: N ▾** chooses it. **Colors ▾** selects a palette or
reverses it; Auto uses blue–red with a scale centered on zero for differences.
These display controls leave original groups and saved calculations unchanged.

[![Difference heatmap and selected frame beside the unchanged saved wavelet integral trend](/screenshots/0.2.11/series-difference.jpg)](/screenshots/0.2.11/series-difference.jpg)

*Frame 2 minus frame 1 in flattened absorption. The saved wavelet trend remains
the original per-frame integral. The two inputs have different acquisition
conditions and energy estimates; this illustration is not a calibrated physical difference.*

The overview samples at most 192 available frames, while custom trends calculate
every member. The older batch-fit and LCF controls still require a folder scan.
See the [full-frame measurement workflow](https://github.com/Ameyanagi/rexafs/blob/v0.2.10/doc/full-frame-measurements.md)
for storage, recipes and recovery.

## Frames and overview sampling

Scans follow filename order within each directory. Frame numbers are indices,
not elapsed seconds. Use zero-padded filenames for acquisition order, retain
timestamps separately, and check the scan and file names before interpreting a
time series.

The overview samples at most **192 active frames** across the scan, each using
its effective settings, including per-file overrides. The heatmap interpolates
linearly onto 256 energy, k or R display points, filling zeros outside each
frame's range. An empty end region need not be a measured zero, and sampling can
miss short-lived features. Failed sampled frames appear in **Problems**.

Display ranges are 0–15 Å⁻¹ for k and 0–6 Å for R. The energy axis runs from
$E_0-40$ eV to the smaller of the measured endpoint and $E_0+200$ eV, using the
first successfully processed frame. These ranges do not set normalization or
transform limits. Use consistent processing and k weights to compare amplitudes;
the header shows the project k weight, which per-file overrides can differ from.

Select a frame with the heatmap or scrubber. With the plot focused, Left/Right
moves one frame, Shift+Left/Right moves approximately one percent of the scan, and
Home/End selects the first/last position. The plot can show the nearest sampled
frame while the selected frame loads; wait for processing before interpreting it.
Use **Refresh overview** after changes.

These display choices are rexafs-specific, implemented in the [overview and
frame loading](https://github.com/Ameyanagi/rexafs/blob/v0.2.10/crates/rexafs-gui/src/app.rs)
and [Series controls](https://github.com/Ameyanagi/rexafs/blob/v0.2.10/crates/rexafs-gui/src/app/shell/series.rs).

## What the trends mean

The **Edge energy trend** reports the absolute estimated or requested $E_0$ in eV.
Older versions called it “E₀ shift.” It does not subtract a reference or calibrate the
beamline. Noise and normalization choices can move automatic estimates, as can
sample changes.

The **white-line trend** is the maximum processed absorption from $E_0$ to
$E_0+30$ eV, inclusive. This trend uses **flattened absorption when available**,
falling back to normalized absorption, independently of the energy-plot selector.
Its historical UI label is “white line (norm. μ).” The height is dimensionless after
edge-step normalization. It is a local maximum, not an integrated peak area,
concentration or oxidation-state calibration. A glitch can dominate it. These
definitions come from `frame_sample` and `trend_snapshot` in the
[Series implementation](https://github.com/Ameyanagi/rexafs/blob/v0.2.10/crates/rexafs-gui/src/app.rs).
See [processing theory](/docs/science/processing/) for the meaning of normalized
and flattened absorption.

The $E_0$ and white-line trends cover sampled overview frames. Independent fits
can supply fitted-parameter trends; linear-combination fitting (LCF) supplies
standard weights. Choose **Sampled frames / All frames** for those calculations.
**All frames** calculates every surviving frame without increasing the heatmap's
192-frame limit. Missing or failed results appear as gaps, not zero-valued
parameters.

For LCF, mark at least two standards and review their processing settings, the
LCF range and its constraints in the data tools; Series uses those settings.
The core prepares missing selected arrays on copies. Canceling keeps completed
rows, so check completion status and per-frame errors before treating an exported
trend as the whole scan. Project saves retain the batch coefficients and settings;
publication exports include `data/lcf-series.csv` and JSON.

[![All 100 Cu-mixture LCF results and the selected frame 52 in rexafs 0.2.11](/screenshots/0.2.11/series-lcf.jpg)](/screenshots/0.2.11/series-lcf.jpg)

Captured through computer use from the signed 0.2.11 Mac app after all 100
synthetic mixtures completed LCF against the Cu foil, Cu₂O and CuO references.
The plot uses flattened absorption. Selecting the heatmap and pressing Right
changed the selected frame, spectrum and trend cursor together; the capture
shows frame 52. See [data and capture provenance](/licenses/#documentation-screenshots).

Inspect a subset before a complete calculation, then review failures and
per-frame uncertainties. See [multiple spectra and batches](/docs/desktop/multiple-spectra/)
for fit setup and [LCF theory](/docs/science/analysis/) for standard weights.
Processing choices and model inadequacy can create apparent trends; smoothness
alone does not validate a model.


## Experimental Live acquisition

In 0.2.10, **Series → Live…** watches a selected folder and retains completed
spectra and trends. Preview the filename filter, detector mapping and frozen
processing or saved recipe before starting. The default quiet-file policy uses
three matching observations one second apart. A writer pause can look complete;
choose a policy suitable for the acquisition and inspect retained revisions.
**Pause**, retry and **Open paused** preserve committed results and permit review
before continuation. A saved XANES peak model can run as frames arrive.

Live acquisition remains experimental. Physical Windows/Linux acquisition and
network-share behavior are not qualified by the current local review. See the
[Live guide](https://github.com/Ameyanagi/rexafs/blob/v0.2.10/doc/live-acquisition.md)
for completion policies, recovery and source-revision handling.

<span id="frame-navigation-and-smaller-windows-unreleased"></span>

## Frame navigation and smaller windows (0.2.12)

The Series overview uses frame numbers starting at **1** in its heatmap, trend,
cursor readout and trend CSV export. Internal array indices and saved project
identities remain unchanged. Recorded physical coordinates, such as time or
temperature in saved measured trends, retain their own labels and units.

Use **Previous** and **Next**, enter a whole number in **Frame** and press Enter,
or select a heatmap row. Out-of-range entries show an inline explanation without
changing the selected frame. Arrow-key navigation still works when the plot has
focus. The controls and plot-coordinate conversion are implemented in
[Series](https://github.com/Ameyanagi/rexafs/blob/v0.2.13/crates/rexafs-gui/src/app/shell/series.rs)
and [plotting](https://github.com/Ameyanagi/rexafs/blob/v0.2.13/crates/rexafs-gui/src/plotting.rs).

When the central workspace is narrower than 700 logical pixels, the heatmap and
detail plots stack vertically in a scrollable area. Heatmap controls wrap to keep
Difference, Reference, Colors and Export accessible with the side panels open.

<a id="unreleased-live-monitor-and-averages"></a>

### Live monitor and averages

Version 0.2.14 adds a compact monitor. Open it from the top-bar Live indicator;
use Live options to dock it in the sidebar.
Acquisition continues when the monitor is hidden or another stage/application is
active. Its Latest scan/Last 5 scans/Average selection is independent of the main analysis.

[![Signed rexafs 0.2.14 with a three-scan fluorescence average in the Live sidebar while Cu stays selected in Data](/screenshots/0.2.14/live-sidebar.png)](/screenshots/0.2.14/live-sidebar.png)

*The monitor follows synthetic arrivals while the analyst's Cu view stays in place.
The same monitor can open in a [separate window](/screenshots/0.2.14/live-separate-monitor.png).
[Input and capture provenance](/licenses/#desktop-0214-release-captures).*

Preview a sample, choose transmission, fluorescence and reference under Signals,
and choose Individual scans, Running averages, or Average every N scans. Channels stay
separate. Running mode maintains three outputs for three selected signals;
set mode maintains three per set. Original scans remain available through Show
source scans. Pause/Resume reconciles arrivals without importing committed
revisions twice. Rewritten files replace their contribution to the raw absorption
mean and retain their original batch position.

A reviewed single-spectrum fit can also be selected under EXAFS fit. Preview
checks the frozen paths and model; Live fits individual scans or updated averages
according to the Output choice. EXAFS · k and EXAFS · R show data and fitted
curves in the monitor. XANES peaks is a separately named option.

Averaging assumes repeated scans of the same state, uses equal weights and common
energy coverage, and does not infer alignment or a signal-to-noise stopping rule.
See the [Live guide](https://github.com/Ameyanagi/rexafs/blob/v0.2.14/doc/live-acquisition.md)
for processing, retention and qualification limits. These additions extend the
original 0.2.10 workflow described above.

<a id="live-fit-parameters-and-reference-processing-unreleased"></a>

### Live fit parameters and reference processing

In Live, **Processing → Reference: …** copies the settings of a named spectrum.
The default **Copy selected spectrum’s settings** copies the current selection.
Preview captures normalization, background removal, and transform settings;
Start freezes them for the session. Auto values still resolve for each incoming
spectrum. The EXAFS model is selected separately.

Choose **EXAFS · k** or **EXAFS · R** to show a **Parameter** selector and
error-bar trend. **Expression…** supports derived quantities such as
`reff + dr_1`. Selecting a parameter does not refit the scans. The side monitor
shows this trend first, with an expandable fit comparison. In averaging modes,
each point is a fit of an updated average, so the number of updates can differ
from the number of source files. These averages share scans and are not
independent measurements.
