# Desktop UX audit status

The September 2026 audit retains 130 screenshots, including intermediate defects
and their corrected states. Each gallery has captions and recorded observations:

- [Selection, processing ranges and Publish — 55 captures](2026-09-07-ux-audit/index.html)
- [Multi-file and multi-channel import — 30 captures](2026-09-07-multichannel-audit/index.html)
- [Advanced processing controls — 22 captures](2026-09-07-advanced-processing-audit/index.html)
- [Stable/Nightly updates — 16 captures](2026-09-07-update-audit/index.html)

- [Mixed-weight overlays and palette isolation — 7 captures](2026-09-07-audit-followup/index.html)

## Addressed for 0.1.2

- Bulk select, deselect and invert, visible group counts and clearer overlay scope.
- Append multiple files/folders without replacing the session; deduplicate sources.
- Preserve named reference, transmission and fluorescence channels independently
  across processing, projects, joint fits and publication exports.
- Restore project selections and per-spectrum parameters across catalog refreshes.
- Edit AUTOBK k max and other background/transform ranges directly on plots.
- Enforce each spectrum's Rbkg as the lower bound of its fitting R range.
- Expose background standards, solver controls and inverse-transform parameters.
- Export flattened spectra, full-resolution CSV data and separate channel figures.
- Correct mixed-weight comparison labels and isolate palette scroll/keyboard input.
- Add Stable/Nightly checks, verified downloads and signed nightly builds.

## Remaining polish

These items remain follow-up work; they are not claimed as fixed by this release.

| Priority | Observation | Improvement |
|---|---|---|
| Medium | A directory can contain spectra from different absorption edges | Make scan/series grouping explicit and flag incompatible edges before treating files as one series |
| Medium | Additional channels do not share every primary-row filter | Apply consistent search/filter behavior across group types |
| Low | Project identity and unsaved state are not prominent | Show project filename and a dirty indicator |
| Low | Marked/frozen group selections are session state | Consider saving those selections in the project |
| Low | Publish and advanced controls require substantial scrolling | Group controls into clear collapsible sections and keep export scope visible |

The galleries document functional computer-use checks on macOS ARM development
apps. Final signed-release checks are recorded separately. Native Intel hardware,
Windows/Linux graphical qualification and exhaustive Series workflows are outside
the completed local audit.
