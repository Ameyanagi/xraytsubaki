## Context
The current repository has three interacting concerns:
1. Build reliability is blocked by a transitive legacy FFT path (`fftconvolve` -> older `easyfft` -> `anymap` incompatibility).
2. Core pipeline runtime includes panic-prone behavior and a confirmed logic bug in FFT channel access.
3. Existing hot paths include avoidable allocation/conversion churn that limits throughput and scale.

This change is intentionally staged so measurement and safety gates are in place before deeper optimization work.

## Goals / Non-Goals
- Goals:
- Restore reliable build/test on stable toolchains.
- Remove panic-prone batch behavior and enforce typed, structured failures.
- Improve end-to-end throughput while preserving scientific output tolerances.
- Make validation repeatable in CI across core, GUI, and Python-adjacent workflows.

- Non-Goals:
- Major algorithm replacements.
- Broad redesign of GUI/Python feature sets.

## Decisions
- Decision: Compatibility unblock is mandatory before optimization.
  - Rationale: Performance claims are not actionable without stable, repeatable compile/test paths.

- Decision: Batch methods return aggregate typed failures instead of panicking.
  - Rationale: Parallel and batch contexts require failure reporting that is deterministic and index-addressable.

- Decision: One FFT stack only.
  - Rationale: Mixed legacy/new FFT dependencies increase incompatibility risk and maintenance cost.

- Decision: Internal numeric representation should be consistent through the hot pipeline with adapters at boundaries.
  - Rationale: Repeated representation churn in loops is a dominant avoidable overhead.

- Decision: CI starts with informational perf regression signals, then upgrades to blocking thresholds.
  - Rationale: Allows baseline stabilization before hard gate enforcement.

## Architecture and Sequencing
- Slice A: Dependency unblock + CI split/matrix.
- Slice B: Panic elimination + FFT correctness fix + invariants.
- Slice C: Conversion/allocation refactor in AUTOBK/normalize/FFT path.
- Slice D: API cleanup and interface docs.

Each slice includes tests and benchmark comparison artifacts before moving forward.

## Risks / Trade-offs
- Risk: Small API break in batch interfaces may require downstream updates.
  - Mitigation: Keep signature changes minimal and document migration examples.

- Risk: Perf optimization might alter numerical output.
  - Mitigation: Require seq/par parity tests and tolerance-bound equivalence checks.

- Risk: CI duration increase from benchmark gates.
  - Mitigation: Separate fast compile/test jobs from heavier benchmark jobs and phase gating from informational to blocking.

## Open Questions
- Whether benchmark allocation metrics in CI should use DHAT, heaptrack, or criterion-integrated allocator hooks by default across all runners.
- Whether Python zero-copy interop guarantees should initially target only contiguous `f64` buffers or include broader dtype/layout support.
