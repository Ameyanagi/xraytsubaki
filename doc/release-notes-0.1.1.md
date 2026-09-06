# rexafs 0.1.1

Fast plot dragging no longer leaves a stationary copy of a spectrum behind the
moving preview. This release consumes the ruviz-gpui 0.13.1 fix, which clears the
plot interior before drawing the translated layer and handles transparent panel
backgrounds without blending the old data through the preview.

The upstream regression test reproduces the old duplicate-spectrum pixels and
checks large horizontal, vertical and diagonal pans, including transparency.
The existing pan-anchor, asynchronous-render and final-frame tests remain in
place. Linked and embedded `.rxs` compatibility samples were saved and reopened
with the 0.1.1 writer; the project format is unchanged.

The [benchmark and profiling report](benchmarks/2026-09-06-larch/README.md) records
112 comparisons of published rexafs 0.1.0 and XrayLarch 2026.3.1, with several
rexafs solvers, measured and synthetic dense inputs, output arrays, CPU profiles
and reproducible scripts. It explains substantial timing and numerical gaps.
Automatic AUTOBK spline parameter counts now use floor rather than round, as in
Larch. For rbkg=1 and kmax=12 this changes nine parameters to eight. The separate
[clamp and signal-recovery study](benchmarks/2026-09-07-clamp-study/README.md)
records 322 configurations, measured scans, known synthetic backgrounds and
single-solve penalty prototypes. Repeating the original standard comparison
reduced the Ru χ(k) difference from 37.51% to 4.72% before the clamp-model change.
New analyses now use the study's [fixed-λ model](autobk-fixed-penalty.md), with a
configurable default λ=0.001, cubic interpolation, a floor low-R cutoff, and the
final three endpoint samples. One column-scaled linear solve replaces the
initial-scale/ridge-retry path. Older projects retain their legacy clamp model.
Faster processing or agreement with Larch does not by itself establish scientific
accuracy; the retained synthetic study measures signal recovery separately.

See the [changelog](../CHANGELOG.md) and [release runbook](releasing.md) for
distribution and validation records.
