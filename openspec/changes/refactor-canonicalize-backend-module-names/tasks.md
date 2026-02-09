## 1. Rename and Routing Refactor
- [x] 1.1 Rename nalgebra-default files to canonical names:
  - `background_nalgebra.rs` -> `background.rs`
  - `mathutils_nalgebra.rs` -> `mathutils.rs`
  - `normalization_nalgebra.rs` -> `normalization.rs`
  - `xafsutils_nalgebra.rs` -> `xafsutils.rs`
  - `xrayfft_nalgebra.rs` -> `xrayfft.rs`
- [x] 1.2 Rename ndarray compatibility files to explicit suffix names:
  - `background.rs` -> `background_ndarray.rs`
  - `mathutils.rs` -> `mathutils_ndarray.rs`
  - `normalization.rs` -> `normalization_ndarray.rs`
  - `xafsutils.rs` -> `xafsutils_ndarray.rs`
  - `xrayfft.rs` -> `xrayfft_ndarray.rs`
- [x] 1.3 Update `crates/xraytsubaki/src/xafs/mod.rs` cfg routing to match new file names.

## 2. Reference Cleanup
- [x] 2.1 Search for stale `_nalgebra.rs` or old unsuffixed-ndarray references and update docs/notes where needed.

## 3. Validation
- [x] 3.1 Run `cargo check -p xraytsubaki`.
- [x] 3.2 Run `cargo check -p xraytsubaki --features ndarray-compat`.
- [x] 3.3 Run strict gate suite:
  - `cargo test -p xraytsubaki`
  - `cargo clippy -p xraytsubaki --all-targets -- -D warnings`
  - `cargo fmt --all -- --check`
  - `cargo check --manifest-path py-xraytsubaki/Cargo.toml`
- [x] 3.4 Run `openspec validate refactor-canonicalize-backend-module-names --strict`.
