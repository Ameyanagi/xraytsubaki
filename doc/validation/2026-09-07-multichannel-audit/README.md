# Multi-channel import and publication audit — 7 September 2026

This follows the [first UX audit](../2026-09-07-ux-audit/README.md) and PR #24. Screenshots come from an isolated development app on macOS ARM, using public Cu, Ni and QAS Ru fixtures. User applications and projects were left running.

Open [the screenshot gallery](index.html) or [the recorded observations](steps.json).

## Verified in computer use

- Select two files and a folder together in the native picker. Append three sources to the original three-file project, preserve the active spectrum, and automatically add the named QAS reference channel.
- Change reference Rbkg to 1.3 Å; confirm the primary sample still uses Auto = 1.0 Å.
- Add fluorescence as a third channel from the same file, with independent normalization and background parameters.
- Mark reference and fluorescence; export both while only fluorescence is active. The resulting bundle contains 12 figures, each as PNG/SVG/CSV, including two flattened-spectrum CSV files. The manifest records separate group IDs and channel labels; the reference export retains Rbkg 1.3 Å.
- Save and reopen a linked project. All six files and both additional channels return, the active fluorescence channel is restored, and reference Rbkg remains 1.3 Å.
- Repeat the same mixed selection import: zero duplicate files or channels are added.
- Add both channels to a joint fit despite their shared source path. The reference rejects R min 1.2 Å below its own 1.3 Å Rbkg; fluorescence accepts 1.2 Å above its own 1.0 Å Rbkg. Each preview displays that channel's data.
- Remove the reference group: its fit assignment reports a missing source instead of silently using another channel. Undo restores the group and its fit identity.

## Corrections found during this audit

- Put the channel name first so narrow sidebars distinguish Reference and Fluorescence. Label Data counts as groups, rather than calling channels files.
- Virtualize additional-group rows so importing a long run does not create every row on each render.
- Restore the model value after rejecting an invalid joint-fit range edit, so an uncommitted value cannot be mistaken for the saved range.
- Assign IDs to legacy groups when entering an editable session. Keep archive decoding lossless, including exact floating-point bits and opaque metadata.
- Cache raw arrays for additional channels as well as primary files; processing-parameter edits do not repeatedly read the source file.
- Preserve surviving group marks and frozen indices across removal/undo, and retire in-flight results before indices move.

## Validation

- New tests cover QAS transmission `ln(I0/It)` and reference `ln(It/Ir)`, named-channel detection, no invented references for unnamed columns, source deduplication across catalog runs and name-store boundaries, linked/embedded project round trips with one raw payload, stable identities, and rejection of duplicate joint datasets.
- Full GUI suite: 139 passed, 3 ignored and 2 failures exposed the archive ID-migration problem. After correcting it, all 15 project tests pass. A current-build run excluding the seven unchanged FEFF-related tests passes 135 tests, with 3 pre-existing ignored tests. Those seven tests passed in the full run.
- GUI build, `cargo check --tests`, formatting and diff checks pass. GUI Clippy retains its existing warning baseline; the new warnings found during development were corrected.

## Further work tracked in issue #23

- Expose the remaining advanced library inputs with their applicability explained, including inverse-transform controls and AUTOBK standard/solver settings.
- Add the Stable/Nightly update channel and signed nightly publishing workflow.
- Improve project save/dirty-state feedback and make explicit scan grouping clearer when one directory contains different absorption edges.
- Consider persisting marked/frozen group selections in projects and filtering additional channels together with primary file rows.

Screenshots include intermediate states so detected problems and their corrections remain reviewable. Test outputs use only public fixtures. Native pickers were captured after navigating to the isolated QA directory.
