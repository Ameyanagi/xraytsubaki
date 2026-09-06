# Rust dependency update record

The release uses **Rust 1.98.1**, the latest stable verified on 2026-09-06 with
`rustup check` and the [Rust release announcements](https://blog.rust-lang.org/).
`rust-toolchain.toml` pins that verified toolchain; update it together with release
CI when adopting a newer stable. Nightly is not required.

Direct dependencies were checked against the crates.io API on that date and
updated across the Rust workspace, including the Python and Wasm binding crates.
The workspace lockfile records the resulting transitive versions. Unused
`serde_arrow` was removed instead of retaining an unused Arrow dependency tree.

## Compatibility constraints

| Dependency | Selected line | Reason |
|---|---|---|
| nalgebra | 0.34.2 | Latest Levenberg–Marquardt 0.15 exposes nalgebra 0.34 types; nalgebra 0.35 is not interchangeable at that trait boundary |
| nalgebra-apex | 0.33.3 | Latest apex-solver 1.4 exposes nalgebra 0.33 types; the solver adapter keeps this separate |
| rusqlite | 0.31 | apex-io 0.3 links SQLite through this version; Cargo rejects a second libsqlite3-sys `links = sqlite3` version |
| faer | 0.24 | Matches the latest apex-solver factor/Jacobian interface |
| GPUI / gpui_platform | Existing shared git revision | Latest registry GPUI remains 0.2.2; the desktop needs the matching platform entry point and macOS font-kit feature |

Other direct dependencies use the newest compatible resolution of the updated
requirements. This does not force every transitive crate to its newest major:
upstream public types, feature contracts and native-link constraints still apply.

## Published XrayDB dependency

The desktop uses [`xraydb` 0.4.1](https://crates.io/crates/xraydb), the published
library from [`Ameyanagi/xraydb-rs`](https://github.com/Ameyanagi/xraydb-rs).
This is the latest stable crates.io release verified on 2026-09-06. Cargo resolves
both `xraydb` and `xraydb-data` from crates.io; there is no local fork or path patch.
The unused local `xraydb-rs/` scaffold has been removed.

`SpectrumInterest` uses its embedded reference database to identify likely
absorption edges from E₀ when XDI metadata is absent. Explicit element/edge
metadata takes priority, and a database estimate never changes an existing model.
The desktop regression suite checks Cu/Ni edge identification, metadata priority
and invalid-energy handling. The crate is MIT OR Apache-2.0 licensed and is covered
by the release dependency inventory and license gate.

## Spline and license changes

`rusty-fitpack` was removed. Spline coefficients now use a direct QR solve with
the existing nalgebra dependency, with local evaluation and basis derivatives.
No replacement spline crate is needed. AUTOBK's direct optimizer is unchanged.
The Apache-licensed GPUI sum_tree component is patched to use tracing directly,
removing its GPL tracing wrappers. Details and the enforced non-GPL license policy
are in the [distribution notices](distribution-notices.md).

## Verification policy

Run core tests, the simple API tests, clippy, documentation builds, optional
feature checks, Python wheel installation, Wasm runtime tests and desktop builds.
Version resolution alone is not evidence of a working release. Record final
results and remaining platform qualification in [the release runbook](releasing.md).
