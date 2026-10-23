---
title: "Your first analysis"
description: "Process a public Cu foil spectrum and save a reproducible project."
audience: user
---

Download the [room-temperature Cu foil spectrum](/examples/cu_metal_rt.xdi)
from the X-ray Absorption Data Library. It was measured at APS 13-ID-C.
This is a teaching example; use a suitable reference for experimental calibration.
See [data provenance](/licenses/). Select a screenshot for the full application
window.

## 1. Open and verify the data

Choose **Import…** and select `cu_metal_rt.xdi`. In the preview, keep
**Stored signal · mutrans** checked and inspect **Detected energy**. The XDI
header declares energy in eV. Choose **Import 1 spectrum**. This file already
contains absorption: do not apply a second logarithm.

**Open Cu example** in the empty workspace offers a separate bundled 150 K Cu
measurement. The same workflow applies, but its measured arrays and fit results differ.

[![Import preview showing the stored Cu absorption and detected energy axis](/screenshots/0.2.11/import-preview.jpg)](/screenshots/0.2.11/import-preview.jpg)

*Check the Cu edge and column mapping before importing. rexafs 0.2.11, macOS.*

## 2. Inspect normalization

Open **Process** (Cmd/Ctrl+2). Its four linked plots show measured μ(E),
normalized μ(E), $k^w\chi(k)$ (χ(k) multiplied by $k$ to the power $w$) and
the Fourier magnitude $|\chi(R)|$; they update together after every edit. On the μ(E) plots, inspect the estimated $E_0$ and the
blue pre-edge and yellow post-edge fit windows; the **Normalize** section of the
inspector holds their settings. In 0.2.14 and earlier, these steps use the
separate **Normalize**, **Background** and **Transform** stages shown in the
screenshots. A blank numeric field means **Auto**; its placeholder shows
the current resolved value. Select **norm** for edge-step-normalized absorption,
or **flat** to remove the fitted post-edge trend from that display.

Start with the defaults. When processing your own sample, choose baseline windows
that exclude the edge structure and remain inside the measured range. Automatic
$E_0$ is an estimate, not an energy calibration.

[![Full normalization window showing the Cu edge and baseline fitting regions](/screenshots/0.2.11/polynomial.jpg)](/screenshots/0.2.11/polynomial.jpg)

*The Pre/post toggle displays the baseline fits over the original absorption. rexafs 0.2.11, macOS.*

## 3. Extract EXAFS and transform it

Inspect the oscillations on the χ(k) plot; enlarge it with the button in its
header, or double-click its title, and press Esc to return to the grid. The
**Background** section holds the AUTOBK settings. The recommended
AUTOBK cutoff is $R_\mathrm{bkg}=1$ Å; the background fit uses $k$ weight 1 by default.
A higher cutoff can remove structural signal as well as background.

The **Transform** section holds the forward Fourier transform settings, and its
k window is drawn on the χ(k) plot. The default forward window is 2–15 Å⁻¹,
with weight 2 and a Kaiser–Bessel window. Choose a useful upper limit for the
signal-to-noise range of your actual spectrum; this example uses 12 Å⁻¹. The $|\chi(R)|$ plot displays the
magnitude of the Fourier transform. Its peak locations are not phase corrected.

[![Full transform window showing weighted EXAFS and Fourier magnitude together](/screenshots/0.2.11/transform.jpg)](/screenshots/0.2.11/transform.jpg)

*The Cu spectrum in k and R, with forward-transform controls, in the 0.2.11
Transform stage. rexafs 0.2.11, macOS.*

## 4. Save and export

Choose **Save project**. **Link source files** keeps references to your inputs;
**Include source files** makes a portable project with embedded original bytes.
Choose a new `.rxs` filename. See [projects and recovery](/docs/desktop/projects/).

Open **Publish**, choose a figure and PNG or SVG, then **Export…**. Choose
**Analysis folder** as the format to export figures, data, captions, methods and
references together.
Review the figure labels and experimental details before sharing them.

Next, [fit a first coordination shell](/docs/desktop/fitting/) or read
[what each calculation means](/docs/science/processing/).
