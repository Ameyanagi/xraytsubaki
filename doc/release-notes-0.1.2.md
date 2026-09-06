# rexafs 0.1.2

This release improves selecting, importing, processing and publishing spectra in
the desktop application. Select all, Deselect all and Invert operate on spectrum
groups. Import multiple files and folders into the existing session; named QAS
reference channels become independent groups with their own processing settings.
Sample and reference groups retain their identities in projects, joint fits and
publication exports.

Comparison overlays use the active group's display k weight for every χ(k)
curve. R/q curves retain their individual Fourier weights; mixed weights are
explicitly identified instead of labeling all curves with one group's units.
The command palette also isolates plot input and restores keyboard focus on close.

Drag the AUTOBK k-range handles on χ(k) to set the background window, including
k max. Fit R min must be at least the actual AUTOBK Rbkg for each dataset; this
is enforced for numeric edits, plot handles, restored models, batch fits and joint
fits. The inspector now exposes the remaining background and inverse-transform
inputs, including a separate background k-origin E0, an optional χ standard,
solver controls and the inverse q grid. Fixed λ remains the default single-solve
background model. Imported standards and group-specific settings survive project
save/reopen.

Publish includes **flattened normalized μ(E)** as a separate figure using the
library's flattening result. Export PNG, SVG and full-resolution CSV data, with
independent sample/reference figures, captions and channel provenance. Figure
choices persist when switching spectra, and stale fit figures are identified.

The core inverse FFT now applies the R window and R weighting to the correct
frequency bins, including DC and the undisplayed high-R tail. It handles changed
FFT sizes and generates an exact, bounded q grid. **Filtered χ(q) results can
therefore differ from earlier releases.** Direct-DFT oracle tests cover both array
backends, padding/truncation and odd/even FFT sizes. The fit-space regression
comparisons with Larch continue to pass.

**Updates** adds Stable (the default) and opt-in Nightly channels. Downloads are
verified against GitHub's size and SHA-256; installation is explicit through the
downloaded ZIP. Nightly builds use `rexafs Nightly.app` and a separate bundle
identifier, allowing both channels to coexist. The daily workflow builds, tests,
signs and notarizes both Mac architectures before publishing a GitHub prerelease.
Nightlies do not publish packages to crates.io, PyPI or npm. See
[desktop updates](desktop-updates.md) for operation and installation.

The screenshot audits record the interactions checked and the defects corrected:
[selection, ranges and Publish](validation/2026-09-07-ux-audit/README.md),
[multi-channel import and export](validation/2026-09-07-multichannel-audit/README.md),
[advanced processing](validation/2026-09-07-advanced-processing-audit/README.md),
[update channels](validation/2026-09-07-update-audit/README.md),
and [overlay/palette follow-up](validation/2026-09-07-audit-followup/README.md).
Linked and embedded 0.1.2 compatibility fixtures retain the new group identities,
background standard and inverse-transform settings. Earlier released fixtures
remain unchanged; the project format is still version 1.
