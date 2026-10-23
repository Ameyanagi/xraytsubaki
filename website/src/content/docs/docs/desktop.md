---
title: "Desktop analysis"
description: "Follow the measurement-to-fit workflow in the rexafs desktop."
audience: user
---

Import spectra, process and fit them, and export results from one project.
The desktop runs on macOS, Windows and Linux without Python or Rust installed.
Start with [your first analysis](/docs/getting-started/first-analysis/).

## The workspace

The **Groups** panel on the left lists spectra and derived results. The center plots the
selected stage; **Parameters** on the right edits that stage for the current
group. The **stage bar** follows the analysis order:

| Stage | What it does |
|---|---|
| [**Data**](/docs/desktop/import/) | Import files, review columns, organize groups, and apply alignment, rebinning or merging. |
| [**Process**](/docs/desktop/processing/) | Normalize absorption to a unit edge step, remove the smooth background with AUTOBK to obtain $\chi(k)$, and Fourier-transform it to $\chi(R)$. Optionally back-transform a selected R range to $\chi(q)$ or inspect a Cauchy wavelet map. Four linked plots show the steps together. |
| [**Fit**](/docs/desktop/fitting/) | Build a structural model and run single or joint fits: **Structure → Calculate → Paths → Model → Results**, or choose [RMC](/docs/desktop/rmc/) for coordinate refinement. |
| [**Series**](/docs/desktop/series/) | Browse scans as ordered frames and inspect trends. |
| [**Publish**](/docs/desktop/publication/) | Set figure size, labels, limits and captions; export PNG, SVG, CSV or an analysis folder. |

Normalization, background removal and transforms run automatically when an
absorption spectrum loads or its processing settings change. Selecting a stage
changes the display and controls.

*Unreleased (after 0.2.14):* the former **Normalize**, **Background** and
**Transform** stages are merged into one **Process** stage, so the stage bar has
five stages. Releases up to 0.2.14 show seven stages, and the screenshots on
these pages were made with those releases. Selecting a thumbnail in the Data
overview opens Process with that plot enlarged.

| Task | Guide |
|---|---|
| Choose an absorber, structure and scattering paths | [Structures and paths](/docs/desktop/structures/) |
| Refine a periodic structure and resume a long run | [RMC fitting](/docs/desktop/rmc/) |
| Share parameters or process a batch | [Multiple spectra](/docs/desktop/multiple-spectra/) |
| Move or recover a project | [Projects](/docs/desktop/projects/) |
| Use the optional analysis assistant | [Assistant](/docs/desktop/assistant/) |

**Groups** and **Parameters** toggle the side panels. Use the action search to
find a stage or processing tool. Per-spectrum processing settings and group
assignments belong to the project; application layout preferences belong to
your computer.

## Keyboard navigation

Use **Cmd** on macOS and **Ctrl** on Windows/Linux for the modifier below. These
are the shortcuts in [rexafs
0.2.4](https://github.com/Ameyanagi/rexafs/blob/v0.2.4/crates/rexafs-gui/src/app.rs#L484).
Most workspace shortcuts yield while the Assistant has focus, and text fields
retain their normal editing shortcuts.

| Action | Shortcut |
|---|---|
| Open a project | Cmd/Ctrl+O |
| Import files or folders | Cmd/Ctrl+Shift+O |
| Save the project | Cmd/Ctrl+S |
| Search actions | Cmd/Ctrl+K |
| Data through Publish, in stage-bar order | Cmd/Ctrl+1 through Cmd/Ctrl+5 (Cmd/Ctrl+7 in 0.2.14 and earlier) |
| Toggle Groups / Parameters | Cmd/Ctrl+B / Cmd/Ctrl+J |
| Focus the group filter | Cmd/Ctrl+P |
| Undo / redo an analysis edit | Cmd/Ctrl+Z / Cmd/Ctrl+Shift+Z |
| Show the action journal | Cmd/Ctrl+Shift+J |

When the Groups list has focus, use Up/Down to navigate, Space to toggle a mark,
and F2 or Enter to rename a group. Cmd/Ctrl+A marks the displayed groups;
Cmd/Ctrl+Shift+A clears marks. **Help** opens the bundled Cu example, the offline
license reader and Updates. Undo history belongs to the current session; save a
project or export an analysis record before closing it.

[![The complete Data stage displaying the measured Cu absorption spectrum](/screenshots/0.2.11/data.jpg)](/screenshots/0.2.11/data.jpg)

*Data stage with the selected Cu spectrum. rexafs 0.2.11 on macOS; select any screenshot for full resolution.*
