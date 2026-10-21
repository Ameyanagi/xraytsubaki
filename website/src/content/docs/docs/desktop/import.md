---
title: "Import and groups"
description: "Map detector signals, confirm energy units and organize spectra."
audience: user
---

This guide describes **rexafs 0.2.13**, including confirmation for matching
files and dismissal of pending imports. Screenshot captions identify their
actual versions and capture builds.

## Import and review

Choose **Import…** or drop a measurement file into the workspace. The plotted
preview handles beamline text, numeric tables, Athena projects, Larix sessions,
XTUNES saves and supported HDF5 arrays. Choose **Scan** for a file with several
records. File content determines the reader; an extension alone does not
establish the units or detector arithmetic.

1. Inspect the plotted signal and **Energy axis**. The reader preserves declared
   eV/keV, relative-energy and monochromator-angle calibration. Confirm an
   assumed unit or provide the missing crystal-plane spacing for an angular axis.
2. Check the signals to import: stored **μ**, **Transmission**, **Fluorescence**
   or **Reference**, when the source provides those channels. Each row shows its
   source-column formula. **Preview** changes the displayed curve without
   changing the checked outputs.
3. Use **Columns** to correct detector roles or **Custom mapping…** for a manual
   output. **Source details** reveals original headers and rows. Read any parser
   warnings before accepting the data.
4. Choose **Import N spectra**. Every checked conversion must succeed before
   groups are added. Each signal becomes a separate spectrum, the Data view
   starts on raw μ(E), and one undo removes the entire import.

[![rexafs 0.2.11 import preview with the stored Cu absorption selected](/screenshots/0.2.11/import-preview.jpg)](/screenshots/0.2.11/import-preview.jpg)

Captured through computer use from the signed 0.2.11 macOS release.
This public Cu measurement contains 408 points and a stored `mutrans` signal;
it does not need detector arithmetic. See
[data and capture provenance](/licenses/#desktop-0211-workflow-captures).

For transmission, rexafs computes `ln(I0 / It)` from incident and transmitted
intensities in matching units. Reference uses `ln(It / Ir)`, with `Ir` measured
after the reference foil. Fluorescence uses the selected detector sum divided by
`I0`. Choose stored μ when absorption is already calculated. The reader does
not repeat dark-current, dead-time or other detector corrections.

Angles are Bragg angles θ, not full scattering angles 2θ. Conversion uses
`E = hc / (2 d sin θ)`, where energy E is in eV, reflecting-plane spacing d is
in Å and `hc = 12398.419843320026 eV Å`. Use the spacing for the actual
monochromator reflection. This applies first-order Bragg diffraction; it does
not fit an instrument offset. The [reader guide](/docs/reference/stable/measurement-reading/#units-and-signals)
explains units, intensity requirements, defaults and scientific references.

For HDF5, **Scan → Select datasets…** combines explicitly chosen, equal-length
real vectors. Images need a detector reduction; warnings identify partial group
recovery. A readable container does not mean every channel is available. The
[format guide](/docs/reference/stable/measurement-reading/) describes Athena,
Larix, XTUNES, 9809/KEK `.qd` and other supported formats and their limits.

Released versions through 0.2.7 retain the older batch workflow for folders and
multiple-file selections; import containers individually in these versions.
Source files are unchanged; saved desktop projects
retain original measurement bytes and accepted signal mappings.

### Confirm matching files together

Drop several `.qd` files, drop a folder, or select several files in **Import…**.
In the measurement preview, **Apply to N matching files in this import** is
initially checked when compatible files are available. Review the plotted
spectrum and adjust the mapping once, then choose **Import N spectra**.
Uncheck the option to import only the previewed file.

Matching uses the content-detected format, column names and units, detector
roles, reader warnings and energy conversion, including the monochromator
spacing. Different point counts, acquisition timestamps and numeric values are
allowed. Each imported spectrum retains its own original data and header.
Files with different layouts, unreadable data, unnamed columns or multiple
scans remain under **Pending import** for separate review. **Source details**
lists files needing that review.

This confirmation applies only to the current drop or file selection; it does
not create a persistent recipe or approve a later drop. Every selected file is
validated before any groups are added. If a matching file changes or its chosen
arithmetic is invalid, no groups are added; the error identifies that file.
Reopen the preview after correcting the source, or uncheck the batch option to
review files individually. One undo removes the accepted batch.

Implemented by the desktop's
[`measurement_import::batch` module](https://github.com/Ameyanagi/rexafs/blob/v0.2.10/crates/rexafs-gui/src/app/shell/measurement_import/batch.rs).
This option was added in 0.2.10.

### Skip unwanted pending files

Under **Pending**, choose the **× beside a filename** to skip that file.
The **Skip ▾** menu offers **Skip .prj**, **Skip .xts**, and the other extensions
currently waiting, or **Skip all pending**. Extension matching ignores letter
case, so `.prj` and `.PRJ` are grouped together. Hover over a filename to read
its review reason.

**Undo skip** restores the most recent skipped selection in this session.
Skipping applies only to the pending entries captured when you opened the menu;
it does not exclude that extension from future imports. Original files and
already imported groups remain intact. To reconsider a file later, import it again.
Saved projects retain its skipped status and original review evidence.

The **× on the import summary bar** only hides that summary. It does not skip
pending files. Use the row's × or the Skip menu to remove them from the queue.

[![Skip pending files individually or by extension in rexafs 0.2.11](/screenshots/0.2.11/pending-import-skip.jpg)](/screenshots/0.2.11/pending-import-skip.jpg)

Captured through computer use from the signed 0.2.11 Mac app. The three pending
files contain generated software-test signals; the background plot is the public
Cu example. Skipping `.dat` left the `.txt` entry pending; **Undo skip** restored both `.dat` files.

<a id="import-a-whole-project-unreleased"></a>

### Import a whole project

Use **Import…** to select an Athena `.prj`, an Athena-compatible `.prj` saved by
Larch, or a Larix `.larix` session. Every scan is initially
selected; a project group appears as a scan in the preview.

1. Open the **Scan** menu to inspect the list. **All scans** selects the complete
   list; individual checkboxes choose a subset.
2. Use **Preview** beside a scan to inspect its plot, source header and signals.
   Each scan keeps its own column mapping, units and checked signals when you
   switch previews.
3. Choose **Import N spectra** to add every checked output from the selected
   scans. The number counts spectra, so one scan can contribute several signals.

[![rexafs 0.2.11 import dialog with all four Athena project records selected](/screenshots/0.2.11/import-project-scans.jpg)](/screenshots/0.2.11/import-project-scans.jpg)

Captured through computer use from the signed 0.2.11 Mac app. All four records
were imported together successfully. This example uses Larch's
MIT-distributed `json_unzipped.prj`; see the [screenshot attribution](/licenses/#documentation-screenshots).

Selected scans that need mapping block import until you resolve them or explicitly
uncheck them. The app validates the complete selection before adding any groups.
It preserves the stored absorption and original evidence; saved Athena/Larch
processing results and commands remain provenance rather than active rexafs
processing settings.

This selection also applies to XTUNES `.xtsp` projects and multi-scan SPEC or
HDF5/NeXus files. HDF5 tables containing only detectors or images are not
automatically absorption spectra: choose compatible one-dimensional datasets and
their axis/signal mapping. Historical pixel-axis Athena groups require calibration.
MDA binary files remain unsupported. Versions through 0.2.8 import the selected
scan's signals; selection across scans is introduced in 0.2.9.

### File extensions and folder discovery

The file picker and single-file drops have no extension restriction. The shared
reader checks the contents, including Athena `.prj`, Larix `.larix`, XTUNES
saves, text measurements and supported HDF5/NeXus containers. KEK `.qd` files
using the supported 9809 layout contain measurements; `.qc` condition files
are not spectra.

Folder discovery includes the following names, without
regard to letter case. Beamline formats and containers appear under **Pending
import**; select one to review its scans and signals in the plotted preview.
Plain text and XDI retain the recipe-based batch workflow.

Projects containing accepted measurement imports reopen their saved groups and
explicit source list. They do not rescan the original folder and create duplicate
groups; import the folder again when you want to review newly added files.

| Files | Extensions |
| --- | --- |
| Numeric tables and absorption exports | `.dat`, `.txt`, `.xmu`, `.chi`, `.xdi`, `.csv`, `.tsv`, `.asc`, `.ascii`, `.raw`, `.xas` |
| Beamline measurements | `.qd`, `.ex3`, `.spec`, `.fio`, `.tey`, `.pfy`, `.pey`, `.cey`, `.xes`, `.sdat` |
| HDF5 and NeXus | `.h5`, `.hdf5`, `.hdf`, `.nxs`, `.nx` |
| Saved measurements | `.prj`, `.larix`, `.xtunes`, `.xts`, `.xtsp` |
| Numbered scans and compressed measurements | Numeric suffixes such as `.001` and `.0001`; known measurement suffixes followed by `.gz` |

Extensionless files and `.json` files are also discovered when a bounded content
check recognizes measurement data. Select an unusual filename individually if
the folder scan misses it. Discovery does not guarantee conversion: the preview
reports unsupported layouts, damaged files, missing units and ambiguous signals.
Saved rexafs `.rxs` projects use **Open project…**.

**Since 0.2.7:** duplicate detector-role headings require
explicit column selection, and each stored absorption column appears as its own
signal choice. Malformed leading numeric rows produce an error even when comments
separate them from later data.

## Recipes and groups

Batch imports use a column review and saved recipes. Check the plotted edge,
units and parser diagnostics; **Revalidate** checks an edited mapping and
**Review later** keeps the source pending.

**Recipe** saves import choices for matching layouts. Reopening a project keeps
its accepted mappings; later computer recipes apply to new imports. Source files
are unchanged.

Recipes match column names/order, units, parser dialect and conversion metadata
within their saved folder or instrument scope.
Matching project recipes take priority over computer recipes. Conflicting
interpretations need review. Unnamed columns require one representative
confirmation in each new batch, even when a previous recipe supplies a suggested
mapping. A familiar layout with changed units also requires a new review.

Editing a named recipe creates a revision; existing groups keep their original
interpretation. Disabling reuse stops automatic selection for future imports
without deleting earlier groups. See the
[recipe dispatcher](https://github.com/Ameyanagi/rexafs/blob/v0.2.4/crates/rexafs-gui/src/import_recipes.rs#L317).

Each group row has two lines. The first shows the group name, shortened at the
end when the panel is narrow. The second, in muted text, shows the source file
name (or **derived** for a calculated result), the quantity, such as
**measured μ(E)** for an absorption coefficient read from the file, and the
absorption-edge energy E₀ in eV once processing or a fixed E₀ setting provides
it. Hover a row for its full name and path.

Select a group to make it **current** and edit its settings in the inspector;
the current group has a solid accent background. **Marks** select groups for
comparisons and bulk actions without changing the current group; a mark is
shown only by the row's checkbox. Filtering or collapsing rows can hide marked groups without
unmarking them; check the marked/hidden count before bulk actions. Comparison
plots offer **Preview 12**, **Plot all** and a **Gradient** color option. Sampling
changes only the display; it does not exclude inputs from PCA or MCR. Use
[Series](/docs/desktop/series/) to inspect individual scan frames.

Right-click a group to rename it, duplicate it, add an available channel, or
**Lock processing**. A duplicate has independent settings but can read the same
source file. A processing lock prevents processing edits; it does not copy source
data. **Remove group** removes the group without deleting its source file and can
be undone in the current session. Derived results keep their saved arrays when an
input group is removed; their input notice records the missing source.

Use **Re-map columns…** in the Data parameters to review an existing mapping.
Inspect derived spectra after changing source interpretation or processing inputs.

Derived results are labelled by their stored quantity. Calculated norm/flat
spectra from MCR and LCF can continue through background subtraction, transforms
and fitting while preserving their prepared scale by default. **Refit
pre/post-edge** explicitly enables baseline subtraction and edge-step rescaling.
Differences and residuals remain differences; they cannot be processed as raw
absorption. Older projects ask you to confirm unlabelled arrays' quantity
before processing. Choose what the values represent; a label does not convert
the arrays.

## Data treatment

See [data treatment](/docs/science/analysis/) for alignment, energy calibration,
deglitching, truncation, rebinning, smoothing, merging and difference spectra
before applying these tools across a collection.
