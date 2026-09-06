# Advanced processing audit — 7 September 2026

[Open the screenshot gallery](index.html). This isolated macOS ARM development app uses the public QAS Ru sample/reference fixture and the project from the [multi-channel audit](../2026-09-07-multichannel-audit/README.md).

## Verified in computer use

- Load a standard χ(k) file. Reject malformed rows with a line-specific error; accept a 401-point, two-column unweighted standard. Show its filename and point count, clear it, and restore it with Undo.
- Change background k-origin E₀ to 22114 eV while normalization stays at 22113 eV.
- Switch between Fixed λ and legacy Fixed. Show the applicable solver controls; keep the fixed default as one linear solve. Widen enum controls so their names remain readable.
- Set inverse R range 1–3 Å, high-R taper 0.25 Å, R weight 2, q output limit 10.13 Å⁻¹ and NFFT 4096. Render χ(q).
- Reject incompatible explicit q step 0.05. The existing error banner identifies the displayed curve as the previous result. Clear the field to restore Auto and clear the banner.
- Save and reopen the project; retain the embedded standard and all advanced overrides on the fluorescence group. The reference keeps its own Rbkg 1.3 Å, automatic E₀ and no standard.

The synthetic standard is a UI fixture, not a physically recommended standard for this sample. Its signal is 0.003 sin(4k), sampled every 0.05 Å⁻¹ from 0 to 20 Å⁻¹.

## Numerical correction (issue #26)

The default backend previously applied inverse-window samples to shifted bins, left bins beyond the displayed R extent unfiltered, and ignored R weighting. The compatibility backend retained unfiltered DC. Resizing the inverse FFT could allocate the wrong number of bins or retain the wrong transform size. The q grid was stretched to the requested maximum instead of following the FFT spacing.

Both backends now window the complete spectrum at the actual R-bin positions, including DC and Nyquist, apply R weighting, rebuild the requested FFT length, and return an exact q grid capped at the available output. Invalid R grids and incompatible explicit step/NFFT settings return errors. This intentionally changes affected χ(q) results; projects retain their requested settings but recompute with the corrected algorithm.

## Validation

- Explicit inverse-DFT oracle: 3 tests pass on each array backend, covering shifted/out-of-window bins, R weight, truncated display arrays, even/odd/growing/shrinking NFFT, exact q spacing, oversized output limits, and invalid settings.
- Five stored Larch fit-space comparisons pass, including k, q, R, multiple weights and noise estimation.
- Full GUI suite: 144 passed, 3 existing ignored tests, no failures. Includes standard parsing, persistence/cache invalidation, independent normalization/background E₀, and actual core transform settings/output.
- Core strict Clippy passes. GUI Clippy retains the existing baseline (34 binary warnings, 31 test warnings). Build, formatting and diff checks pass.

## Further polish

- Advanced panels are long; collapsible sections would make routine processing more compact while keeping every input reachable.
- Numeric Auto placeholders can still truncate in the narrow value cells; enum menus now use the full panel width.
- Add a resolved q-step display next to Auto, so users need not infer it from NFFT and the R-grid spacing.
- The saved-project indicator still relies on the status bar; a persistent filename/dirty-state indicator would be clearer.

Stable/Nightly updating and release automation remain tracked in issue #23.
