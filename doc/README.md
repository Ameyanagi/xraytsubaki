# rexafs documentation

Start with the [project README](../README.md), [API guide](api.md),
[Rust guide](../crates/rexafs/README.md), [Python guide](../py-rexafs/README.md) or
[JavaScript guide](../js-rexafs/README.md).

## Release and migration

- [Rebranding plan](rebranding-plan.md): decisions, implementation order and release gates.
- [Migration guide](migration.md): imports, packages and existing desktop data.
- [Release runbook](releasing.md): builds, verification and publication.
- [Dependency update record](dependencies.md): latest stable Rust and compatibility constraints.
- [Distribution license review](distribution-notices.md): observed dependency terms and remaining release work.

## Desktop workflows

- [XDI import](xdi-import.md)
- [Multiple spectra and independent fitting](joint-fitting.md)
- [Structure slices and depth cues](structure-depth-view.md)
- [Project compatibility and recovery](project-compatibility.md)
- [Publication editor and captions](publication.md)
- [Publication export](publication-export.md)
- [Experimental assistant](experimental-assistant.md)
- [Desktop workflow validation](gui-workflow-validation.md)

## Design and scientific history

These records describe the implementation at their stated dates. Validation counts
and benchmark timings are historical, not assertions about the current build.

- [Fitting workspace redesign](fitting-workspace-redesign.md)
- [Structure database design](structure-db-design.md)
- [GUI design](gui-ux-design.md) and [v2 proposal](gui-ux-design-v2.md)
- [Original FEFF fitting scope](../crates/rexafs/doc/feff-fitting-mvp.md)
- [Performance and logic migration](../crates/rexafs/doc/migration-performance-logic-hardening.md)
- [Profiling](profiling.md) and [extended core profiling](../crates/rexafs/doc/profiling.md)
- [Larch/rexafs benchmark matrices, output agreement and CPU profiles (2026-09-06)](benchmarks/2026-09-06-larch/README.md)
- [FEFF/Larch comparisons](plots/feff_vs_larch_index.md)
- [FEFF10 card comparison](plots/feff10_card_comparison_2026-03-03/report.md)
- [Uncertainty notes](../supportinginfo/uncertainty.md) and [additional notes](../supportinginfo/uncertainty2.md)
- [Larch fixture generation](../crates/rexafs/tests/pythonscript/README.md) and [fit reference provenance](../crates/rexafs/tests/testfiles/larch_fit_refs/README.md)
