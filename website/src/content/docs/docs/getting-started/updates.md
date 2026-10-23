---
title: "Updates and offline use"
description: "Keep analyses portable and install rexafs without a live connection."
audience: user
---

This guide describes **rexafs 0.2.14**.

## Update the desktop

Save your project, then open **Help → Updates**. You can also search for
**Check for updates** with Cmd+K on macOS or Ctrl+K on Windows/Linux. Use
**Stable** for routine work; **Nightly** is opt-in and includes unreleased changes.

**Check on startup** is enabled by default. Turn it off in this panel for offline
use. Checks do not install software or upload spectra; the channel and startup
preference are saved on this computer.

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

Desktop versions through 0.2.7 need one manual installation from the
[download page](/download/) to acquire the new updater. On macOS, quit rexafs
before moving the updated app into Applications; on Windows run the installer;
on Linux replace the complete portable folder. Nightly has a separate app name
and can coexist with Stable.

Cached downloads are verified again before reuse. If a size or SHA-256 check
fails, remove the file named in the error and retry. Failed new downloads discard
their temporary files. After a network error, retry or check the release page;
the error does not establish that your version is current.

### Update and restart

**Upgrading from 0.2.10 or 0.2.11 on macOS:** install 0.2.14 manually once from
the [download page](/download/). The older signed updater loses its app-bundle
layout and cannot start the replacement helper. Version 0.2.12 copies the complete
signed helper bundle, and signed ZIP/DMG qualification checks its startup.
The existing app and saved analysis are preserved if the older helper fails.
Windows and Linux use different helpers.

From 0.2.12, macOS desktop and Python releases require Apple Silicon. Intel Mac
users can retain the archived 0.2.11 downloads; no newer Intel asset is provided.


On macOS, Windows and Linux, choose **Update and restart** to download, verify and
install a newer build of the current channel. rexafs saves an embedded recovery
copy of the analysis, then reopens that copy after restarting. Your original
project is unchanged. Save the reopened project to your preferred location;
the session undo stack and running calculations are not restored.

Progress and **Cancel** remain available until the restart begins. Finish active
imports, calculations and Assistant turns before updating. A recovery-save or
verification error keeps the current app open. A failed replacement or launch
restores the previous app. The previous bundle and recovery file remain in a
private `.rexafs-update-*` folder beside the application.

Windows installations run the matching installer automatically, preserving
shortcuts and the uninstall entry. Portable Windows and Linux copies update the
complete extracted folder. User files inside that folder are preserved; a file
collision stops the update before replacement. x64 and ARM64 use their matching
packages.

Run the app from a writable installation folder. Source executables, macOS disk
images, channel changes and system-managed Linux packages retain manual updates.
**Preferences** also offers a download-only action. An update appears only when
a matching release asset has been published for your platform.

Keep a backup when moving between versions. New releases read earlier released
project formats; an older executable may not preserve newer features when saving.

## Remove old installers and updater copies

Open **Help → Storage** to inspect managed downloads and inactive updater app
copies. Review the listed files and combined size before choosing **Delete listed
files**. The view also opens the hidden storage locations in your file manager.
Current applications, recovery projects and scientific calculation files are
retained. See [storage and cleanup](/docs/desktop/storage/) for the scope and
lock checks.

## Install offline

Copy a desktop installer or complete portable folder to another computer with
the same OS and architecture. The bundled Cu example and built-in structures work
offline. Online structure databases and the optional assistant require connectivity.

For Python, prepare uv and the Python interpreter on the offline computer first.
Download wheels on a connected computer with the same OS, architecture and
Python version. Replace 3.12 below with the destination version. Use pip in a
temporary uv environment because uv has no `pip download` command:

```sh
uv run --no-project --python 3.12 --with pip python -m pip download --only-binary=:all: rexafs==0.2.14 --dest wheelhouse
```

On the offline computer, create the project:

```sh
uv init --offline --python 3.12 rexafs-analysis
cd rexafs-analysis
```

Copy `wheelhouse` into the project, then run:

```sh
uv add --offline --no-index --find-links wheelhouse rexafs==0.2.14
uv run --offline python -c "import rexafs; print(rexafs.__version__)"
```

Keep `pyproject.toml`, `uv.lock`, `.python-version` and the wheelhouse to recreate
the environment. These wheels target only the OS, architecture and Python
version used for the download. See [uv's project
workflow](https://docs.astral.sh/uv/guides/projects/).

For TypeScript/JavaScript, install Bun on the destination first. Download the
[published 0.2.14 package archive](https://registry.npmjs.org/rexafs/-/rexafs-0.2.14.tgz)
on the connected computer, copy it to your project on the offline computer, and run:

```sh
bun add ./rexafs-0.2.14.tgz
```

The rexafs 0.2.14 archive includes its WebAssembly binaries and has no runtime
package dependencies. Your application may have other dependencies that need to
be prepared separately. Bun supports [installing local
tarballs](https://bun.sh/docs/pm/cli/add). For Rust, use `cargo vendor` in your
consuming project and retain its generated Cargo configuration before building
offline.

Use an embedded project to move original spectra and FEFF inputs together.
[Project portability](/docs/desktop/projects/) explains what is included.
