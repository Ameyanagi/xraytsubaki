# Update-channel audit — 7 September 2026

[Open the screenshot gallery](index.html). Tests use isolated development apps and machine settings with the public sample/reference project from the earlier audits. The Nightly QA tag `nightly-20260907-0` is a local test identity, not a published release.

## Verified in computer use

- Fresh preferences select Stable and allow an asynchronous startup release check. Opening Updates discovers the actual GitHub v0.1.1 release and the ARM Mac archive.
- Download the 22.7 MB archive, verify its published size and SHA-256, and reveal it in Finder. The existing app and project are not replaced.
- Opt into Nightly; before its first publication, the UI correctly reports no available nightly releases.
- Disable startup checking, restart the app, and verify both preferences persist. Manual checking still works.
- Open Updates through the command palette. The update dialog blocks pointer input, selection shortcuts and stage shortcuts from reaching the underlying project; Escape closes it and normal keyboard shortcuts work afterward.
- Compile a separate Nightly QA app. Its title, brand and build tag identify it clearly; an explicit switch to Stable offers the stable download even when the library versions match.

The audit caught and corrected click-through and focus-restoration defects in the new dialog. Intermediate screenshots are retained and captioned as such. Finder was switched to list view before recording the release folder.

## Automated validation

- Update tests cover channel isolation, semantic stable-version ordering, no downgrade within Stable, explicit return from Nightly, matching architecture/official URLs, SHA-256 requirements, and corrupt/truncated/oversized downloads.
- Four settings tests pass, including preference round trips, legacy defaults and owner-only storage.
- Five nightly/channel tests verify immutable build identities, main-only publication, both required Mac architectures, matching build/signing provenance, stable identities across reruns, notarization metadata and checksum rejection. Existing release-maintenance and archive tests also pass.
- Stable and Nightly development builds pass. Actionlint 1.7.12 validates both the nightly and amended release-build workflows. GUI Clippy retains its existing warning baseline.

## Update behavior

Checks use the public GitHub API without uploading project data. Stable uses the latest-release endpoint so it stays discoverable after many nightly builds. Downloads are streamed to temporary files, checked against the GitHub SHA-256/size, then made available in the private application update directory. Installation is explicit through the downloaded ZIP; the running app is not replaced automatically.

Nightly packaging uses `rexafs Nightly.app` and a distinct bundle identifier. The scheduled workflow runs daily at 18:23 UTC, builds both Mac architectures, requires core and desktop regressions, signs/notarizes/staples both, checks the final extracted archives, and publishes an immutable GitHub prerelease after verifying uploaded digests. It never moves Stable's latest marker and never publishes nightly packages to the registries.

The first real scheduled/dispatch run and final signed release qualification must be recorded after this workflow reaches main.
