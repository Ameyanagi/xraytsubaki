# rexafs sum_tree patch

Source: Zed `crates/sum_tree` at commit
`3060e4170ea5ef0e6886b9ac1853aaead9ddd59f`:
https://github.com/zed-industries/zed/tree/3060e4170ea5ef0e6886b9ac1853aaead9ddd59f/crates/sum_tree

This crate is Apache-2.0. The original copyright and license text are preserved
in `LICENSE-APACHE`. No code from the GPL-licensed ztracing, ztracing_macro or
zlog crates is included here.

Changes made for rexafs on 2026-09-06:

- Replace the two `ztracing::instrument` imports with `tracing::instrument`.
  The existing seven span annotations use the standard tracing macro directly.
- Remove the test-only zlog logger initializer and its ctor dependency.
- Make workspace-inherited manifest values explicit for this standalone patch.
  Preserve the GPUI revision's heapless/proptest/rand compatibility lines.

The root Cargo manifest patches the Zed git source to this directory. The tree
and cursor algorithms are unchanged. Keep the upstream tests with the patch:

```bash
cargo test --locked --manifest-path vendor/sum_tree/Cargo.toml
```

Remove the patch when the selected upstream GPUI graph no longer needs it and
the dependency license gate passes for every target.
