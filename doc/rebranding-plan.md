# rexafs rebranding and release plan

Prepared 2026-09-06. Public identity: **rexafs**. Working codename: **xraytsubaki**.
This is the migration plan; implementation and validation status are recorded at
the end. Registry publication, domain deployment and the remote repository rename
are separate release operations, not implied by local build success.

## Brand and naming

Use lowercase `rexafs` in prose, imports, commands and package names. Describe it
as **Rust-powered X-ray absorption analysis**. The `r` can evoke Rust and the
ReFEFF relationship without prescribing an artificial expansion of every letter.
Credit **ReFEFF** as the optional Rust scattering calculation engine and keep its
upstream name, citations and license notices. rexafs also processes measured XAS,
loads existing FEFF paths and performs fitting independently of that engine.

Retain one origin note: “Developed under the codename xraytsubaki, inspired by the
camellia.” Preserve historical benchmarks, scientific references and third-party
fixture provenance. Rebranding does not change numerical claims or attribution.

| Surface | Release name | Source location after migration |
|---|---|---|
| Rust library / crates.io | `rexafs` | `crates/rexafs` |
| Python distribution / import | `rexafs` / `import rexafs` | `py-rexafs` |
| Python extension | `rexafs._core` (private implementation) | `py-rexafs/src` |
| JavaScript / TypeScript / npm | `rexafs` | `js-rexafs` |
| Wasm binding crate | `rexafs-wasm` (not published to crates.io) | `crates/rexafs-wasm` |
| Desktop app / executable | `rexafs` | `crates/rexafs-gui` |
| Desktop build package | `rexafs-gui` (not published to crates.io) | `crates/rexafs-gui` |
| Website | `https://rexafs.com` | Domain owned; site deployment pending |
| GitHub repository, final step | `ameyanagi/rexafs` | Rename existing repository; preserve history |

On 2026-09-06, the public JSON endpoints for `rexafs` on crates.io, PyPI and npm
returned HTTP 404. The old `xraytsubaki` name also returned 404 on all three.
This is a point-in-time lookup, not ownership or guaranteed name availability.

## Audit findings

- The core already includes normalization, AUTOBK, transforms, tools, LCF/PCA,
  structures, joint and independent fitting, and optional plotting/FEFF runners.
  The two READMEs disagree and incorrectly describe several features as future work.
- The Python distribution and library say `xraytsubaki`, but the PyO3 initializer
  says `py_xraytsubaki`. Fix the import contract and test an installed wheel.
  The two registered functions are a limited binding, not full Rust API parity.
- Python input validation currently happens after an unchecked setter; batch
  “processed count” can include failed spectra. Expose clear result semantics.
- Rust has a large prelude, acronym-heavy type names, cloning getters and deep
  module paths. Add a small facade without destabilizing the numerical engine.
- No npm manifest, Wasm binding or release workflow exists. Wasm is a real port:
  audit RNG, filesystem, process, threading and serialization dependencies.
- The desktop uses git-pinned GPUI. Distribute application builds from GitHub;
  do not promise that its workspace manifest is crates.io-publishable.
- Desktop state used `~/.xraytsubaki`, `XTS_*` environment variables and an
  unreleased project suffix. Retain settings/environment fallbacks; establish
  `.rxs` as the first release format with relative links or embedded data.
- Existing CI covers core builds, tests, formatting, clippy, a binding compile
  check and benchmarks. It does not establish wheel/npm installation or desktop
  portability. Existing macOS workflow notes are historical evidence only.

## Implementation order

### 1. Rename the local product and repair documentation

Rename workspace directories and Cargo packages; update imports, examples, CI,
scripts, package metadata, app strings and generated export text. Use
`https://rexafs.com` as homepage. Keep the existing GitHub repository URL until
the final remote rename, then update it everywhere in one release change.

Rewrite the root README as the product entry point, the core README as the Rust
guide and the Python README as its actual binding contract. Add a documentation
index, an API guide, migration notes and a release runbook. Update all maintained
documentation references and mark superseded design/benchmark records clearly.
Do not relabel third-party source text or rewrite historical measurement values.

### 2. Establish a small API before first publication

The shared entry point is `process(energy, mu)` with an optional E0 override. The
result contains `e0`, `k`, `chi`, `r`, `chir_mag`, `chir_re` and `chir_im`, with
documented units and paired array lengths. Rust also exposes `Spectrum`, `Group`,
`Error`, `Result`, and shallow `io`, `fitting`, `structure`, `analysis` modules.
Keep `XASSpectrum`, `XASGroup` and deep modules usable during migration.

- Rust: checked `Spectrum::from_arrays`, `process`, `ProcessOptions` and
  `ProcessedSpectrum`; borrowed getters can follow without changing old getters.
- Python: `rexafs.process` returns a typed result with NumPy arrays. Keep the old
  function names as compatibility entry points in the new module. Validate
  dimensionality, finiteness and lengths before entering the core.
- JavaScript: `await init()` then `process(Float64Array, Float64Array, options)`;
  generated TypeScript declarations, browser and Node entry points, explicit
  initialization and ownership. No filesystem/FEFF runner promise in Wasm v0.1.
- Avoid global state, dozens of flat keyword arguments and language-specific
  algorithm copies. Advanced normalization, background and fit configuration
  remain on the Rust types initially; binding parity must be documented honestly.
- Test the shared pipeline against the existing staged Rust pipeline and reference
  fixtures. Cover invalid arrays, error propagation and batch partial failures.

### 3. Package and test each distribution

| Channel | First release contract | Required evidence |
|---|---|---|
| crates.io | Native Rust numerical library, optional integrations | Packaged crate builds outside workspace patches; metadata, licenses and assets present |
| PyPI | CPython wheels + source distribution | Clean wheel install/import and real pipeline test; rebuild wheel from sdist |
| npm | Browser/Node Wasm numerical pipeline | Build, type check, tarball install and numerical/error tests in Node and browser |
| GitHub | Optimized desktop app | Launch on each advertised OS/architecture; sample import, processing, project reopen and ReFEFF calculation |

Start local desktop qualification with Apple Silicon macOS (the development platform).
The final release must be built through GitHub Actions for Apple Silicon/Intel macOS,
Linux x86_64 and Windows x86_64, with build and launch evidence for advertised targets. Python wheels should cover the supported CPython range on macOS,
manylinux and Windows. Do not advertise a target merely because a CI matrix lists
it. Prefer serial Wasm initially; retain native parallel batch processing.

Use one `0.1.0` release line across distributions. For release candidates map
Cargo/npm `0.1.0-rc.1` to Python `0.1.0rc1`, use npm's `next` dist-tag and mark the
GitHub release as a prerelease. Declare support from tested toolchains, not old
classifiers. ReFEFF and optional FEFF10 artifacts need their own attribution and
redistribution review before bundling them in a public desktop archive.

### 4. Release automation and artifacts

Build and test locally first; the final distributions must come from a successful
GitHub multi-platform build of the reviewed version tag. Keep release builds
separate from registry writes and promote the same build run across channels. Produce versioned app archives, checksums,
license notices and release notes; preserve the commit and feature set used.
On macOS create an `.app` bundle; signing/notarization requires the maintainer's
Apple credentials and must be completed before calling the download a signed app.

Use Cargo's package dry run and inspect its contents before uploading
([Cargo publishing guide](https://doc.rust-lang.org/cargo/reference/publishing.html)).
Use maturin for wheels/sdist and platform compatibility checks
([maturin distribution guide](https://www.maturin.rs/distribution.html)).
Use Wasm target-specific JS glue and package it behind a deliberate npm export
map ([wasm-pack build guide](https://wasm-bindgen.github.io/wasm-pack/book/commands/build.html)).

Configure registry authentication against the **final** GitHub repository name:

- crates.io: bootstrap the first actual release with maintainer authentication,
  then configure trusted publishing; confirm the current bootstrap requirements
  before execution ([Rust trusted publishing announcement](https://blog.rust-lang.org/2025/07/11/crates-io-development-update-2025-07/)).
- PyPI: configure a pending trusted publisher for `rexafs`, workflow and environment;
  it creates the project on first use and does not reserve the name
  ([PyPI pending publishers](https://docs.pypi.org/trusted-publishers/creating-a-project-through-oidc/)).
- npm: bootstrap the package if required, then configure repository/workflow-bound
  trusted publishing. Use Node >=22.14 and npm >=11.5.1; repository metadata must
  match the real repo ([npm trusted publishing](https://docs.npmjs.com/trusted-publishers/)).

A failed partial release is resumed using the same tested artifacts for missing
channels; never try to overwrite a published version. A code fix gets a new
version. Announce a coordinated release only when all advertised downloads install.

### 5. Rename the GitHub repository last, then publish

After local migration and artifact qualification, rename the existing repository
to `ameyanagi/rexafs`. Update `origin`, workspace/package repository URLs, badges,
workflow references and website links. Confirm redirects and the final repo's
Actions configuration. Configure trusted publishers after this rename, or update
their repository binding before use. Push the reviewed release commit and tag only
after this check; GitHub redirects do not substitute for publisher configuration.

Connect `rexafs.com` to the selected documentation host, verify HTTPS, provide
language quickstarts and desktop downloads, then publish the coordinated release.
Do not put unverified install badges or a download button on the site prematurely.

## Completion checklist

- [x] Local package, source and UI rebrand; legacy projects/settings preserved.
- [x] Documentation revised; current and planned APIs distinguished.
- [x] Shared simple API and Python import contract tested.
- [x] npm/Wasm package builds and installs; browser behavior verified.
- [x] Core package and Python wheel/sdist verified outside the source tree.
- [x] GPL dependencies removed; direct spline solve and dependency license gate added.
- [ ] Desktop release build packaged and launched on each advertised target.
- [ ] Release automation checked; signing and dependency notices qualified.
- [ ] Remote repository renamed, URLs and trusted publishers reconciled.
- [ ] Domain documentation deployed, packages published and downloads verified.

Implementation status and measured local results are recorded in the
[release runbook](releasing.md#validation-record). Public distribution remains
pending the GitHub platform matrix and final distribution qualification; the
[dependency license gate](distribution-notices.md) now passes locally.
