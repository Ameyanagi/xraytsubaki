# Retained project fixtures

The first release project format is `.rxs` version 1. Unreleased codename formats
are not supported or retained as compatibility fixtures.

| File | Coverage |
|---|---|
| `minimal-v1.rxs` | Required header and defaults for omitted optional state |
| `format-v1-defaults.json` | Frozen meaning of omitted format-1 fields; additive fields may be introduced without changing existing defaults |
| `rexafs-0.1.0-links.rxs` | Relative sources, metadata, overrides, bounds, derived data, joint assignments, history and publication settings/captions |
| `rexafs-0.1.0-embedded.rxs` | The same state with compressed originals and duplicate-payload deduplication |
| `rexafs-0.1.1-links.rxs`, `rexafs-0.1.1-embedded.rxs` | Saved and reopened through the 0.1.1 writer from the 0.1.0 linked fixture; same format and complete state in both storage modes |
| `future-version.rxs` | Future format: reject without modification |
| `truncated.rxs` | Corrupt/incomplete input: reject without modification |
| `data/*.xmu`, `feff/*.dat` | Real inputs for relocation, byte recovery and processing checks |

Settings, the derived example and recorded fit statistics are synthetic persistence
examples, not scientific fit-reference results. Raw data is copied unchanged from
`crates/rexafs/tests/testfiles/xraylarch_d867/xafsdata/cu_150k.xmu`;
`second.xmu` deliberately duplicates it. The path is copied unchanged from
`feffit/Feff_Cu/feff0001.dat` in the same collection. See the original
[provenance](../../../../rexafs/tests/testfiles/xraylarch_d867/README.md) and
retained comments for attribution. The manifest checksums every project and input.

For **each release**, save small linked and embedded projects with its fields,
review their contents, add new files and append a release entry and hashes.
Keep every previously released sample. Rust discovers all project samples from
the manifest and checks full state after load/save/reopen. Intentionally invalid
samples must appear in `invalid_projects`. Add targeted assertions for new
migrations/defaults; do not regenerate previously released fixtures.

Run `python scripts/check-compatibility-fixtures.py` with Python 3.12+ and the
desktop tests in the [compatibility policy](../../../../../doc/project-compatibility.md).
GitHub runs these checks across release platforms. A version bump without both
fixture modes fails the release gate.
