# Distribution license review

The project's own source files retain their existing MIT OR Apache-2.0 license.
That declaration does not describe every dependency linked into a wheel, Wasm
module or desktop binary. The rebrand does not change upstream licenses.

The dependency graph was updated and checked on 2026-09-06:

| Previous dependency | Replacement | Distribution affected |
|---|---|---|
| rusty-fitpack 0.1.2 (GPL-3.0) | Local B-spline basis, direct QR coefficient solve using existing nalgebra, evaluation and coefficient Jacobian | Rust, Python, Wasm and desktop |
| zlog, ztracing, ztracing_macro (GPL-3.0-or-later) | Apache-2.0 sum_tree patch using MIT tracing directly | Desktop |

No spline library was added. Cubic interpolation retains not-a-knot boundaries;
AUTOBK clamps evaluation and FEFF path resampling extrapolates its end polynomial
pieces. AUTOBK's existing LinearDirect optimizer remains unchanged. The new
implementation is checked against independent SciPy values and existing scientific
fixtures. No GPL source was copied. The desktop patch preserves the Apache source
and attribution; its exact changes and upstream tests are recorded in
[vendor/sum_tree/README.rexafs.md](../vendor/sum_tree/README.rexafs.md).

`cargo deny --locked check licenses` passes with cargo-deny 0.20.2. The checked-in
[policy](../deny.toml) covers the whole workspace, all features, every platform
branch, and build/development dependencies. GPL, AGPL and LGPL are not accepted
license choices. Dependencies offering a permissive OR alternative are accepted
under that alternative. CI requires this gate before accepting release artifacts.

This does not mean every dependency is MIT/Apache: allowed terms also include
MPL-2.0 and CDDL-1.0, among others listed in the policy. `gpui_util` lacks a Cargo
license field; the scanner identifies its existing `LICENSE-APACHE`. Upstream
licenses and required notices are preserved rather than rewritten.

The desktop packaging script includes `dependencies.json` with resolved versions,
repository URLs and declared licenses, and copies available LICENSE, COPYING and
NOTICE files into `licenses/`. The inventory includes build dependencies; it is
evidence for review, not proof that every listed crate is linked into the binary.
Inspect notices and applicable source requirements for every advertised platform.
The inventory is a collection aid, not a change to upstream distribution terms.

The desktop archive uses ReFEFF and excludes the optional FEFF10 runner. Imported
XrayLarch examples retain their upstream commit and measurement provenance in
`resources/examples/PROVENANCE.md` (or the macOS app's Resources directory).
Source packages retain their existing fixture provenance. Qualify the fixture
redistribution notices alongside dependency notices before public publication.
