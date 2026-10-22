---
title: "Projects, sharing and recovery"
description: "Save portable inputs and recover the previous project save."
audience: user
---

Use **Save project** and **Open project** for `.rxs` files. Format 1 is the first
released project format; unreleased codename formats have no compatibility
loader or file association. `.rxs` is the only project suffix.

Projects store processing settings, overrides, derived spectra, fit models/history,
joint assignments and publication settings, including figure sizes and captions.
Materials Project credentials remain in machine-local settings.

Save explicitly. `.rxs.bak` contains the previous completed save, not later
edits. Opening a project resets undo/redo and the action journal; export an
analysis folder before closing to retain the journal. Saved derived-operation
provenance, fit history and completed Assistant conversations have separate
retention rules.

## Raw data: paths or embedded originals

Choose **Raw: paths / Raw: embedded** beside **Save project**. This choice is
remembered in the project and also applies to **Export analysis folder**.

| Mode | Contents | Moving or sharing |
|---|---|---|
| **Paths (default)** | Source references and metadata; no original file payloads | Move the project and source folders together, preserving their relative layout |
| **Embedded** | Losslessly compressed original spectra and referenced FEFF inputs | Reopen the `.rxs` file without its original input files |

Relative paths resolve against the **project's directory**. For example,
`run.rxs` beside a `data` folder stores `data/cu.xmu`; a project inside `projects`
stores `../data/cu.xmu`. Save As recalculates references to the same files.
Different Windows drives require absolute paths. The save dialog starts in the
project's directory, or the source folder for a new project.

Embedded mode includes the imported spectrum folder, standalone sources, override
and joint-fit inputs, current/historical FEFF references, and available
`feff.inp`, `crystal.json` and `engine.txt` metadata. It waits for catalog scanning
and fails if a required source is missing or unreadable. Original bytes,
including comments, line endings and non-text bytes, survive unchanged.
Identical files share one compressed payload. Extraction checks byte counts and
SHA-256 hashes and writes only inside the private rexafs cache.

Paths mode permits unavailable sources, without size/checksum metadata, so
settings can still be saved. Linked files can change independently; their hashes
record the bytes present at save time. Switching an embedded project to paths
references extracted cache files. Keep embedded mode for a portable snapshot.

Opening a paths project does not compare linked files against their saved hashes;
processing reads their current content. Embedded mode checks payload hashes
during extraction. See the [project
loader](https://github.com/Ameyanagi/rexafs/blob/v0.2.4/crates/rexafs-gui/src/project/storage.rs#L547).

Processed spectra are recomputed on reopening. Derived spectra retain full
energy and μ arrays in both modes. Historical fit statistics, models, result
curves and exact transform settings (k/R windows, window families and taper
widths) are saved, so **Publish** can export a reopened project's fits without
a refit. The fit that was current when you saved is shown again and counts as
up to date while its spectrum, processing settings and model are unchanged. Projects saved by older releases still open; their fits keep
statistics and models, and their curves return after a refit.
Embedded mode excludes unrelated workspace files and executables/backends.
[**Export analysis folder**](/docs/desktop/publication/#analysis-folder-contents)
adds processed arrays, available fit arrays and captioned figures/tables beside
`project.rxs`.

## File information and limits

Projects record software/format versions, source references, timestamps and
checksums. Comment-header previews are bounded; embedded payloads retain the
complete original bytes. Limits are 512 MiB per project file and 1 GiB for
expanded inputs. Missing/corrupt embedded payloads and unsafe archive paths
produce errors.

## Compatibility from the first release onward

- Application and project-format versions are independent. Optional additions can
  retain format 1 only if their interpretation stays compatible. Existing defaults
  are part of the format contract.
- Loading never rewrites the source. Saved values, bounds, expressions, derived
  arrays and assignments must survive load/save/reopen. Unknown top-level metadata
  is retained; undocumented nested extension fields are outside this contract.
- Future formats are rejected with an actionable error, and cannot be overwritten.
  Incompatible formats require explicit migration. Failed loading leaves the
  session intact. Missing versions, missing headers and malformed input are errors.

Backward compatibility means **new releases read projects from earlier released
versions**. An older executable cannot be guaranteed to retain later features
when saving a newer file. Keep the original/backup when switching versions.

## Safe saving and recovery

The project is validated before the destination changes, then written to a
temporary file, flushed and renamed in the same directory. Failed writes, syncs
and renames leave the previous project intact. `project.rxs.bak` retains the
exact bytes of the immediately preceding save.

To recover, copy `project.rxs.bak` to `recovered.rxs` and open the copy. This
backup is on the same disk; keep separate archival copies of valuable experiments.



[![Full save-project dialog with embedded source files selected](/screenshots/0.2.11/save-project.jpg)](/screenshots/0.2.11/save-project.jpg)

*Saving embedded sources. Full window, rexafs 0.2.11 on macOS; select to enlarge.*
