## Context
The repository already has two pending structural changes:
- error modernization
- ndarray-to-nalgebra migration

This proposal is intentionally outcome-oriented (performance + logic reliability) and avoids introducing additional major architecture shifts. It defines the minimum required technical decisions to deliver measurable improvements safely.

Current constraints observed in code and tooling:
- Hot path contains repeated vector materialization and conversion work in AUTOBK and normalization routines.
- Batch APIs in `XASGroup` currently call `unwrap()` in both sequential and parallel iterators.
- Current stable build is blocked by a transitive dependency chain involving `anymap`.

## Goals
- Improve end-to-end core pipeline throughput with measurable benchmark targets.
- Eliminate panic-based behavior in production batch processing paths.
- Ensure deterministic compatibility/build verification in CI.

## Non-Goals
- Replacing core scientific methods or changing scientific model semantics.
- Designing new external API surface beyond what is required for reliable error behavior.

## Decisions

### 1. Performance changes are benchmark-gated
Implementation must establish baseline metrics first, then compare post-change metrics for:
- single-spectrum benchmark path
- parallel large-batch benchmark path

No optimization task is considered complete without benchmark comparison artifacts.

### 2. Reliability uses fallible batch aggregation
`XASGroup` processing methods shall not panic when a spectrum fails.
They shall return typed batch failures including at least the failing spectrum index and source error.

Rationale:
- preserves progress semantics for callers
- keeps library behavior predictable in parallel execution

### 3. Logic fixes are included with perf work
Known behavior defects discovered during optimization (for example incorrect channel getter mapping) are in-scope and must be corrected in the same change stream.

Rationale:
- avoids shipping faster but incorrect output behavior

### 4. Compatibility unblock precedes optimization
If core crate does not build on current stable toolchain, compatibility fix is required before throughput optimization tasks begin.

Rationale:
- guarantees repeatable validation environment
- enables CI and benchmark gating

## Trade-offs

### Conservative API evolution vs reliability
Returning richer batch error structures may be a small API break. This is accepted to remove panic behavior and improve operational correctness.

### Minimal change vs deep rewrite
The proposal prefers targeted hot-path optimization (allocation/conversion reduction, indexing improvements) rather than full algorithm rewrite, to control risk and reduce delivery time.

### CI runtime cost vs safety
Adding benchmark/regression checks increases CI cost. This is accepted because performance is a primary product requirement.

## Rollout Plan
1. Build compatibility unblock and reproducible benchmark baseline.
2. Batch reliability and correctness fixes.
3. Hot-path performance optimizations with repeated benchmark validation.
4. CI gating hardening for regressions.

## Risks and Mitigations
- Risk: regression from dependency changes
  - Mitigation: stable+beta CI matrix for core crate
- Risk: scientific drift during optimization
  - Mitigation: existing tolerance-based tests + targeted output parity tests
- Risk: partial migration conflict with pending refactors
  - Mitigation: stage tasks with explicit dependencies on related changes
