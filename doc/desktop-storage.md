# Desktop storage and installer cleanup

**From 0.2.13:** Help → Storage, About rexafs → Storage, and the command palette
provide a storage view. The view scans in the background, lists managed installer
downloads and inactive updater app copies, and shows their combined logical file
size before deletion. Filesystem compression or shared blocks can make the actual
change in free disk space different from the displayed size.

**Unreleased (source builds after 0.2.14):** **About rexafs** opens an About
panel instead of the Help menu, so use Help → Storage or the command palette.

**Open storage folder** opens the hidden `~/.rexafs` directory in the system file
manager. **Open installer downloads** opens its `updates` folder. On macOS, the
view also provides buttons for retained `.rexafs-update-*` transaction folders
beside installed applications. These are separate locations; opening the main
storage directory alone does not reveal old updater copies in Applications.

**Delete listed files** permanently removes the displayed, unchanged candidates.
Downloaded installers can be fetched again. On macOS, inactive previous apps,
staged apps, helper copies and extraction leftovers can also be removed when
their recorded target app still exists. This removes the old app rollback copy;
the current installed app is kept. Cleanup never recursively deletes an entire
update transaction folder, because it can also contain an unsaved recovery project.

The following remain untouched:

- `Update recovery.rxs`, update logs and transaction records.
- Saved projects, extracted project inputs and normalization/wavelet histories.
- RMC checkpoints, FEFF calculations, structures, settings and credentials.
- Partial downloads, unrecognized files, symbolic-link candidates and app copies
  that are running or locked by an updater.

Cleanup is unavailable during a download or installation in this app. It takes
an installer-cache lock shared with this version's download workers and the
existing macOS installation lock before removing updater copies. It rechecks
the candidate list and file metadata so newly created or changed files are not
silently added to a cleanup. Errors are shown; unreadable files are kept.

The view manages files created by rexafs's updater. It does not search the user's
Downloads folder or remove manually installed applications. Other data in the
storage folder may still be needed by a linked project; opening that folder is
for deliberate inspection, not an instruction to delete its whole contents.

Implementation: [`storage_cleanup.rs`](../crates/rexafs-gui/src/storage_cleanup.rs)
and the [Storage view](../crates/rexafs-gui/src/app/shell/help/storage.rs).
