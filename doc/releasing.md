# Releasing rexafs

The repository is preparing its first coordinated 0.1.0 release. Nothing in this
runbook means the packages, website or GitHub downloads have already been published.
The [rebranding plan](rebranding-plan.md) defines scope; [dependency notes](dependencies.md)
record the Rust 1.98.1 toolchain and compatibility constraints.

## Current launch status

Checked 2026-09-06:

- The public repository is now [`Ameyanagi/rexafs`](https://github.com/Ameyanagi/rexafs),
  and this checkout's `origin` and current package metadata use that name.
- [Release build 34012430217](https://github.com/Ameyanagi/rexafs/actions/runs/34012430217)
  passed for PR #16 at `a88599ee382fb7a4ed89dce48b094b160bc5cc35`. All build jobs
  succeeded, including 20 Python wheel targets and four desktop targets; all 28
  uploaded artifacts remain available. This was a pull-request run, so it cannot
  be promoted by `publish.yml`.
- [Main CI 34013687691](https://github.com/Ameyanagi/rexafs/actions/runs/34013687691)
  passed at merge commit `df7a2d698ba1e4d39848b6a452c05bc180423937`.
- No GitHub release or version tag exists. The crates.io, PyPI and npm package
  endpoints for `rexafs` returned HTTP 404 at this check.
- The GitHub `release` environment is configured for tags matching `v*`.
  Its `CRATES_IO_TOKEN` and `NPM_TOKEN` secret names were verified through GitHub;
  their values were not read. The maintainer confirmed the PyPI pending publisher
  for `rexafs`, repository `Ameyanagi/rexafs`, workflow `publish.yml`, environment
  `release`. Registry authentication will be exercised by the first tagged upload.

Next, review the final metadata and [draft release notes](release-notes-0.1.0.md),
tag the reviewed commit, and dispatch a new release build for that tag. Use its
successful run ID for the initial registry uploads and GitHub draft. Complete
platform launch qualification before public promotion of the desktop downloads.

## Local checks

```bash
python scripts/check-compatibility-fixtures.py
cargo fmt --all -- --check
cargo deny --locked check licenses
cargo test --locked --manifest-path vendor/sum_tree/Cargo.toml
cargo test --locked -p rexafs
cargo test --locked -p rexafs --features ndarray-compat
cargo test --locked -p rexafs --features trust-region
cargo clippy --locked -p rexafs --all-targets -- -D warnings
cargo check --locked -p rexafs --features ndarray-compat
cargo check --locked -p rexafs --features plotting,refeff-runner,feff10-runner,amcsd,materials-project,cod
cargo doc --locked -p rexafs --no-deps
cargo package --locked -p rexafs
```

Inspect the `.crate` contents, licenses and required compressed structure assets.
Cargo verifies the extracted package without relying on the workspace's GPUI patch.
Do not use `--no-verify` as the final publishing gate. A dirty checkout may be
packaged locally with `--allow-dirty` for review; releases require a reviewed commit.

Python:

```bash
maturin build --release --locked --manifest-path py-rexafs/Cargo.toml --out dist
maturin sdist --manifest-path py-rexafs/Cargo.toml --out dist
python -m pip install dist/rexafs-*.whl
python py-rexafs/tests/test_api.py
```

Use a fresh environment to install the wheel, and another to rebuild/install the
sdist. Test each supported CPython minor (3.10–3.14), OS and architecture. Linux
wheels must meet the declared manylinux policy; a local Linux wheel is insufficient.

JavaScript:

```bash
npm --prefix js-rexafs run build
npm --prefix js-rexafs test
cd js-rexafs
npm pack
```

Install the tarball in a fresh consumer project, type-check its public imports,
and exercise both the Node and browser entry points with a real spectrum. Include
the `.wasm` files and license notices; no compiler is required on the consumer side.

Desktop (run on the target platform):

```bash
cargo test --locked --release -p rexafs-gui --no-default-features --features refeff-runner
cargo build --locked --release -p rexafs-gui --no-default-features --features refeff-runner
python scripts/test-release-archive.py
python scripts/package-desktop.py
```

Use Python 3.12+ for the release scripts. `package-macos.sh` remains a macOS build
convenience wrapper. Archives go to `target/distributions/` with version and Rust
host triple in the filename: macOS `.app` ZIP, Linux `.tar.gz`, Windows ZIP.
Each contains ReFEFF, an example, license files, a dependency inventory and build
metadata, with an adjacent SHA-256 checksum. The script extracts the archive into
a fresh directory and runs its executable's `--version` and `--self-check`.
The latter processes the packaged example without relying on the source checkout.
It does not test GPU rendering or replace an interactive launch check.
Windows ZIP packaging clamps upstream file dates to ZIP's supported range
(1980–2107), preserving file contents and leaving source timestamps untouched.

The macOS package rejects Homebrew/local dynamic libraries; Linux rejects unresolved
linked libraries. Inspect `linked-libraries.txt` and qualify a clean installation.
The Linux runner installs GPUI's X11/Wayland build dependencies; the release needs
a graphical session, GTK 3, fontconfig, xkbcommon and a Vulkan-capable driver. Windows
uses its native MSVC/Windows SDK toolchain. Test the actual minimum OS before
advertising compatibility beyond the runner image.

The packages have no publisher code signature. Developer ID signing/notarization
and Windows signing require the maintainer's credentials and an extension to the
GitHub packaging job. Sign before the final archive and checksum are generated;
then qualify the signed download on a clean machine. Preserve the
[dependency notices](distribution-notices.md) for each distribution.

## GitHub is the release build authority

Local artifacts are verification outputs, not public release uploads.
`release-build.yml` runs on pull requests and supports manual/reusable execution.
It builds and tests:

- the non-GPL dependency license policy across all features and platform branches,
  plus the Apache sum_tree patch's upstream tests;
- the Rust source crate on Ubuntu;
- CPython 3.10–3.14 wheels on Ubuntu, Apple Silicon macOS, Intel macOS and Windows;
- the Python sdist, including an installation rebuilt from that source archive;
- the npm tarball, Node/browser numerical tests and an installed TypeScript consumer;
- desktop archives on macOS 15 ARM64/Intel, Ubuntu 24.04 x86_64 and Windows 2025 x86_64.

The final manifest job requires **every** build job to succeed. `SHA256SUMS` uses
flat asset names so it also works after downloading all GitHub Release assets into
one directory. A matrix entry is a qualification target, not evidence of support;
record the actual successful run and perform GUI launch/import/process/project
reopen/ReFEFF checks on each advertised target.

After the final repository rename and reviewed version tag, dispatch the build:

```bash
gh workflow run release-build.yml --ref v0.1.0
gh run list --workflow release-build.yml
# After the selected build succeeds:
gh workflow run publish.yml --ref v0.1.0 -f channel=github-draft -f build_run_id=RUN_ID
```

`publish.yml` requires the matching version tag and a successful, manually dispatched
`release-build.yml` run for that exact commit. It downloads that run's artifacts and
verifies every checksum. The Rust publishing step additionally reproduces the
`.crate` in GitHub and compares its bytes before using Cargo's registry uploader.
Python and npm upload the downloaded distributions directly. Local artifact paths
are never accepted as publication inputs.

The `release` GitHub environment is configured for the final repository. Channels:

- `crates-io`: select `registry_auth=token` to use `CRATES_IO_TOKEN` for the first
  upload. After configuring the crate's trusted publisher, use the default
  `registry_auth=trusted-publishing` to obtain a temporary token through
  `rust-lang/crates-io-auth-action`. Package verification runs before authentication.
- `pypi`: pending/existing trusted publisher for `publish.yml`, environment `release`.
  This channel always uses trusted publishing and needs no PyPI API token.
- `npm`: select `registry_auth=token` to use `NPM_TOKEN` for the first upload.
  After configuring the package's trusted publisher, use the default
  `registry_auth=trusted-publishing`; it does not pass the bootstrap token to npm.
- `github-draft`: creates a draft with GitHub-built assets and checksums. Review
  platform qualification, notices, signing status and notes before making it public.

All registry publisher forms use these exact values:

| Field | Value |
|---|---|
| Package/project | `rexafs` |
| GitHub owner | `Ameyanagi` |
| Repository | `rexafs` |
| Workflow filename | `publish.yml` |
| Environment | `release` |

For the first coordinated upload, replace `RUN_ID` with the successful manual
release build of the `v0.1.0` commit:

```bash
gh workflow run publish.yml --ref v0.1.0 -f channel=crates-io -f registry_auth=token -f build_run_id=RUN_ID
gh workflow run publish.yml --ref v0.1.0 -f channel=npm -f registry_auth=token -f build_run_id=RUN_ID
gh workflow run publish.yml --ref v0.1.0 -f channel=pypi -f registry_auth=trusted-publishing -f build_run_id=RUN_ID
```

PyPI's pending publisher creates the project on first upload. crates.io and npm
require an existing package before their trusted publisher can be configured.
After those first uploads, add the publishers using the table above. For npm,
allow direct `npm publish`, which is the operation used by this workflow. Remove
the bootstrap secrets and revoke their registry tokens after switching to and
verifying trusted publishing. See the [PyPI setup guide](https://docs.pypi.org/trusted-publishers/creating-a-project-through-oidc/),
[crates.io guidance](https://blog.rust-lang.org/2025/07/11/crates-io-development-update-2025-07/)
and [npm trusted publishing guide](https://docs.npmjs.com/trusted-publishers/).

Repeat publication with the **same build_run_id and tag** for missing channels;
no rebuild is necessary after a registry-only failure. Keep the run's artifacts
until every channel completes. Never overwrite a published package version. A code
change requires a new coordinated version/tag and a new successful build. Release
candidates use Cargo/npm `0.1.0-rc.1`, Python `0.1.0rc1`, npm `next` and GitHub's
prerelease marker. A resumed draft upload may report already-present assets;
compare their hashes before replacing anything.

## Final repository rename and launch

1. Finish local rebranding and artifact qualification.
2. The existing GitHub repository has been renamed from `ameyanagi/xraytsubaki`
   to `Ameyanagi/rexafs`, preserving its history and issue/PR context.
3. `origin` and current workspace, Python/npm and documentation URLs now use
   `https://github.com/Ameyanagi/rexafs`. Update other existing clones as needed.
   Retain old URLs only where they explain migration history.
4. Configure/reconcile registry trusted-publisher repository/workflow/environment
   bindings with the final name. Verify Actions access and tag protection.
5. Review version, changelog and package contents; push the release commit and tag.
   Run the GitHub multi-platform build, qualify its downloads, then publish each
   channel using that successful build run ID.
6. Deploy documentation to `rexafs.com` on the selected host, verify DNS/HTTPS and
   download links, then announce only the channels/targets that passed installation.

Public registry lookups on 2026-09-06 found no rexafs package; they do not reserve
names. Recheck before the first publish. Registry guidance is linked from the
[plan](rebranding-plan.md); authentication setup and final remote actions are still
maintainer release steps.

## Validation record

Verified locally on Apple Silicon macOS with Rust 1.98.1, 2026-09-06:

| Check | Result |
|---|---|
| Core default test suite | 213 passed, 3 ignored |
| Core ndarray compatibility suite, including doctests | 250 passed, 3 ignored |
| Core trust-region suite | 213 passed, 3 ignored |
| Direct spline solve | Independent SciPy knot/coefficient/evaluation/Jacobian references passed; Ru/Cu/Ni χ differs from the previous pipeline by <4e-14 |
| Dependency licenses | cargo-deny 0.20.2 passed across all features/platform branches; standalone sum_tree license check passed |
| Apache sum_tree patch | 10 upstream tests passed |
| Core clippy, all targets, `-D warnings` | Passed |
| Rustdoc, `RUSTDOCFLAGS="-D warnings"` | Passed |
| Optional plotting/ReFEFF/FEFF10/database integrations | Compile checks passed |
| Extracted crates.io package | Cargo verification passed; source, licenses and compressed structure assets present |
| CPython 3.12 wheel | Fresh installation; 5 API tests passed |
| CPython 3.14 sdist rebuild | Fresh installation with NumPy 2.5.2; 5 API tests passed |
| npm/Wasm | 3 tests passed, Chromium fetch/processing passed, tarball installation and Node-only TypeScript compilation passed |
| Desktop optimized tests, ReFEFF build | 124 passed, 2 ignored; includes project fixtures, captioned exports, credential permissions and stale FEFF jobs |
| macOS app archive | Extracted `--version` / `--self-check` passed; example E0=8977.493 eV |
| macOS interactive app | Bundled Cu example rendered; custom 4 × 3 inch / 300 DPI PNG saved at 1200 × 900; project reopened with saved plot settings; opaque cluster correctly occluded the central absorber; .rxs controls saved both modes and a portable embedded copy restored the selected spectrum/overrides without adjacent inputs |
| Publication report | Five vector figures rendered in Chromium; numbered figure/table captions and resolved processing values verified visually |
| Project compatibility | Five .rxs fixtures, three raw inputs and a frozen defaults snapshot checksummed; linked relocation/Save As, lossless embedded recovery without originals, repeat saves, unchanged processing, metadata, backup/failure and exact-float tests passed |
| Release automation | actionlint passed; checksum roundtrip and tamper rejection passed; coordinated version check passed |
| Documentation | Relative-link audit passed; formatting and diff whitespace checks passed |

Upstream future-incompatibility notices remain for binrw 0.12.1, block 0.1.6 and
proc-macro-error2 2.0.1. The GUI also has existing unused-code warnings; these did
not fail its build/tests. Ignored scientific/performance tests are not counted as
passing. The ndarray suite exposed an overlength FFT input panic, now fixed to
match the default implementation's truncation.

Spline replacement exposed a joint-fit stopping issue near floating-point
resolution. The FEFF DogLeg parameter tolerance now matches its forward-difference
Jacobian step, `sqrt(f64::EPSILON)`; cost and gradient tolerances are unchanged.
The joint-fit regression still requires convergence, known parameter recovery and
finite positive uncertainties. AUTOBK's direct solver is unchanged.

The table above records **local** results. The subsequent GitHub pull-request
matrix passed, as recorded in the current launch status, and the repository rename
is complete. Automated archive self-checks do not qualify interactive Intel macOS,
Linux or Windows launches. Registries remain unpublished and domain deployment
is pending. The first public release still requires a successful manual build of
the final tag, credentials/publisher setup for registry channels, and final
download qualification, including each platform's notices.
