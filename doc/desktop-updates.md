# Desktop updates and release channels

## Preferences and About (unreleased)

**Unreleased (source builds after 0.2.14):** the application menu has
**Settings…** (Cmd+, on macOS, Ctrl+, on Windows/Linux), which opens
Preferences. It gathers the settings already saved in `~/.rexafs/settings.json`
(or the file named by `REXAFS_SETTINGS`): the Dark/Light theme, the update
channel and **Check for updates on startup**. It also lists the structure
sources and Assistant choices saved there, which you still change where they are
used, and shows the settings file location. Changes apply immediately and are
saved at once, exactly as from **Switch theme** or the Updates panel.
**About rexafs** now opens an About panel with the version, the CI commit (local
builds show "local build"), the MIT OR Apache-2.0 licence, and links to
**Licenses** and the source repository, instead of the Help menu.

## Update and restart (0.2.12)

On macOS, Windows and Linux, **Update and restart** downloads the selected
current-channel release, verifies its SHA-256 and size, checks the architecture
and compiled release identity, and runs the packaged-example check. macOS also
requires the official Developer ID signature and Gatekeeper acceptance. It saves and reloads an embedded `.rxs` recovery copy
before arming a helper. Active calculations, imports and Assistant turns must
finish first. Cancel remains available until the helper handoff.

The helper waits for the parent process to exit. A filesystem lock excludes
concurrent updates of the same app. Staging and the old-app backup are in a
private directory beside the destination, so replacement uses local renames.
Replacement or detected startup failures trigger rollback; the previous app and recovery
project remain available. The reopened analysis is a recovery copy, not a write
to the original project. Saved project data survive; the undo stack and running
jobs are not serialized. Large or unavailable embedded inputs can prevent the
recovery save, in which case rexafs stays open and reports the error.

Implementation: [`updates/install`](../crates/rexafs-gui/src/updates/install.rs)
and the [update dialog](../crates/rexafs-gui/src/app/shell/updates_view.rs).
Signature checks follow Apple's [Code Signing Tasks](https://developer.apple.com/library/archive/documentation/Security/Conceptual/CodeSigningGuide/Procedures/Procedures.html)
and [code requirements](https://developer.apple.com/documentation/technotes/tn3127-inside-code-signing-requirements).
The updater additionally pins the rexafs Developer ID team and channel bundle
identifier; that trust policy and the recovery workflow are rexafs choices.

Windows installations use the matching per-user Setup installer automatically,
preserving their shortcuts and uninstall registration. The GUI downloads and
checks the ZIP payload and installer, then closes before Setup runs silently.
Setup cannot reboot Windows or force-close other applications. A failed install
restores the previous folder and the saved rexafs uninstall registration.
Portable Windows ZIP copies and Linux tar.gz copies replace their extracted app
folder. No terminal commands or manual extraction are needed for the update.
The Windows flags follow the official
[Inno Setup command-line reference](https://jrsoftware.org/ishelp/topic_setupcmdline.htm).

Windows and Linux packages include an inventory of package-owned files, beginning
with 0.2.8. User files in the app folder are preserved. A collision with a new
package file, a modified package file, links or special files stops the update
before installation. The inventory is not a signature: Windows and Linux releases
are currently unsigned and rely on the official HTTPS release metadata and
SHA-256 checks. Both x64 and ARM64 use their matching native packages.

No administrator helper is installed. Source builds, read-only disk images,
translocated apps, unwritable folders and channel changes retain manual
installation. System-managed Linux installations should use their package
manager. Automatic updates require a published matching asset; nightlies that
publish only macOS assets do not offer a Windows/Linux update. From 0.2.12,
macOS releases require Apple Silicon. The signed 0.2.10/0.2.11 Mac helper cannot
start because it loses its bundle layout: install 0.2.12 manually once. The
corrected helper retains the full signed bundle and is checked in both signed
ZIP and installed-DMG qualification. Intel Mac users can retain 0.2.11.

Desktop versions through 0.2.7 need one manual installation to acquire the updater;
their historical download workflow is described below.

## Historical download workflow (through 0.2.7)

Open **Help → Updates**, or search for **Check for updates** with Cmd+K
on macOS or Ctrl+K on Windows/Linux.

**Stable** is the default for routine analysis. **Nightly** is opt-in and follows builds of `dev`. The selected channel and the **Check for updates on startup** preference belong to this computer, not the project. Automatic checks do not install software or upload spectra.

Choose **Download** to fetch the matching Mac archive and verify its size and SHA-256. **Show download in Finder** reveals the completed ZIP. Save the project, quit the app, extract the archive, and move the application into Applications. Nightly is named `rexafs Nightly.app`, so it can coexist with Stable. Choosing Stable from a Nightly app explicitly offers the stable release, even if its library version is older.

The built-in verified-download action supports macOS in 0.2.4. Windows and Linux
users should follow the release link and download the matching installer or
portable archive manually; see [installation](installing.md).

An already downloaded archive is checked again before reuse. If that cached file
fails its size or SHA-256 check, the error identifies its path; remove the damaged
download and retry. rexafs does not open it or replace it silently. A failed new
download discards its temporary file. A network error during **Check for updates**
does not establish that the installed version is current; retry when GitHub is
reachable or check the release page manually.

A Nightly label includes the immutable build tag. `rexafs --build-info` reports the library version, channel, release tag, source commit and optional nightly build time. Each packaged archive contains the same identity and signing/notarization provenance in `build.json`.

## Updating to 0.2.4

Version 0.2.4 fits the initial window to the display, adds import/project/example
actions to the empty workspace, and keeps long text and its caret inside fields.
Folder scans use bounded batches and respond to cancellation. Existing format-1
projects, processing settings and numerical defaults are unchanged. Save the
current project before replacing the application.

## Updating to 0.2.3

Version 0.2.3 fixes Windows console launches, Ctrl shortcuts and multi-selection,
and reduces accessibility update overhead. macOS keeps its Cmd shortcuts.
Project format, saved processing settings and numerical defaults are unchanged.
Save the current project before replacing the application.

## Updating to 0.2.1

Save the project before updating. Version 0.2.1 improves startup, import choices,
processing controls, publication figures, and structure interaction. Spectrum
palettes and the optional AUTOBK/FFT weight link are saved with the project.
Existing format-1 projects remain readable; keep their backups when switching
between application versions.

## Updating to 0.2.0

Save the project before updating. Version 0.2.0 adds persistent group identities,
marks, labels, locks, import recipes and applications, pending sources, and parser
evidence to format-1 projects. Existing mappings remain authoritative on reopen;
computer recipes apply only to new imports. Original source files are unchanged.
Keep the previous-save backup when moving an ongoing analysis to this version.

## Updating to 0.1.4

Save the project before quitting for an update. Version 0.1.4 adds optional
Assistant conversations to format-1 projects; older projects begin with an empty
history. The Assistant's host, width, model preferences and conversation-save
limit belong to this computer. Conversation contents belong to the project and
are written on Save. The default limit is five; zero disables saving. Existing
previous-save backups remain available.

## Maintainer operation

`.github/workflows/nightly.yml` builds pushes to `dev` and accepts manual dispatch on `dev`. At 18:23 UTC daily, its scheduler on `main` dispatches a separate run on `dev`, whose exact source commit is retained for every job and rerun. A manual dispatch on `main` also delegates to `dev`. Publication rejects main, feature branches and pull-request refs. See the [branch workflow](development-branches.md). A date plus GitHub run ID forms each tag; published nightlies are never overwritten and are never marked as the latest stable release.

From 0.2.12 preparation onward, the workflow builds Apple Silicon macOS packages with ReFEFF, runs core and desktop checks, and uses the existing `macos-signing` environment for Developer ID signing and Apple notarization. No signing secrets are available in build jobs. It produces ZIP archives and [drag-to-Applications DMG installers](macos-installers.md), with installation checks and notices inside the app. The final `nightly` environment publishes only after both formats pass source, target, checksum and notarization-provenance checks. Uploaded GitHub digests are checked before the draft becomes public. A failed draft can be resumed by rerunning the workflow; an already-public nightly requires a new dispatch/run ID.

The separate reviewed stable workflow remains `publish.yml`; its crates.io, PyPI and npm trusted publishers are unchanged. Windows/Linux portable desktop previews and a Windows setup installer are available for platform testing. Native Windows development-build interaction checks are recorded in the [Windows review](validation/2026-09-09-windows-gui/README.md). Linux packaged GUI checks use X11 software rendering. Final-download native Windows interaction, physical Linux GPUs and native Wayland remain unqualified; automated archive and installation checks are recorded with the releases.
