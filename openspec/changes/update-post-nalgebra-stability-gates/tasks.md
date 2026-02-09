## 1. Baseline and Scope Lock
- [x] 1.1 Confirm and record current failures for strict gates:
  - `cargo clippy -p xraytsubaki --all-targets -- -D warnings`
  - `cargo fmt --all -- --check`
  - `cargo check --manifest-path py-xraytsubaki/Cargo.toml`
- [x] 1.2 Confirm nalgebra-first default remains the active runtime baseline and keep `ndarray-compat` as optional compatibility only.

## 2. Core Quality Gate Compliance
- [x] 2.1 Resolve core crate lint failures that block `clippy -D warnings` (including cfg typos and warning-level issues escalated by `-D warnings`).
- [x] 2.2 Ensure formatting compliance for workspace Rust files with `cargo fmt --all -- --check`.
- [x] 2.3 Re-run `cargo test -p xraytsubaki` and confirm no regression in existing test behavior.

## 3. Python Binding Stabilization
- [x] 3.1 Fix Python binding compile errors in `py-xraytsubaki` while preserving stable API signatures.
- [x] 3.2 Remove binding-level warning debt required for clean `cargo check --manifest-path py-xraytsubaki/Cargo.toml`.
- [x] 3.3 Verify `run_batch_qas_trans` and `run_pipeline_arrays` contract behavior remains aligned with `py-xraytsubaki/README.md`.

## 4. CI Enforcement
- [x] 4.1 Update CI workflow to run and enforce the canonical strict gate command set for core and Python bindings.
- [x] 4.2 Ensure failures in strict gate commands block merges for relevant pull requests.

## 5. Documentation and Final Validation
- [x] 5.1 Update migration or README notes where needed so docs match post-change gate expectations.
- [x] 5.2 Run:
  - `cargo test -p xraytsubaki`
  - `cargo clippy -p xraytsubaki --all-targets -- -D warnings`
  - `cargo fmt --all -- --check`
  - `cargo check --manifest-path py-xraytsubaki/Cargo.toml`
- [x] 5.3 Run `openspec validate update-post-nalgebra-stability-gates --strict`.
