---
title: "Concepts and glossary"
description: "The measurements, quantities, symbols and file types used throughout rexafs."
audience: user
---

Symbols and units follow the [processing guide](/docs/science/processing/).
Each entry links to the relevant workflow or calculation.

## The measurement

**X-ray absorption spectroscopy (XAS)** measures how strongly a sample absorbs
X-rays as a function of photon energy. The absorption rises sharply at an
**absorption edge**, the energy at which a core electron of one element can be
excited. Because each element has its own edge energies, XAS is element-specific.
Energies in rexafs are photon energies in electronvolts (eV).

**$\mu(E)$** is the absorption spectrum. In transmission, it is the natural
logarithm of the incident intensity divided by the transmitted intensity,
$\mu(E)=\ln[I_0(E)/I_t(E)]$. In fluorescence, a common estimate is the
fluorescence intensity divided by $I_0$. The desktop import assigns the detector
columns to these roles; the libraries accept already constructed absorption.
See [import and groups](/docs/desktop/import/).

**XANES**, the X-ray absorption near-edge structure, is the region within a few
tens of eV of the edge. Its shape reflects the oxidation state and local
symmetry of the absorbing element. rexafs normalizes XANES for comparison and
supports [linear combination fitting and principal component
analysis](/docs/science/analysis/) of XANES collections.

**EXAFS**, the extended X-ray absorption fine structure, is the weak
oscillation that continues for hundreds of eV above the edge. It is caused by
the outgoing photoelectron scattering from neighboring atoms. Its frequency
encodes interatomic distances and its amplitude encodes the number and disorder
of neighbors. Extracting and fitting EXAFS is the main purpose of rexafs.

## Processing quantities

| Term | Meaning | Where it is used |
|---|---|---|
| **$E_0$** | The edge energy used as the energy origin. rexafs estimates it from the absorption derivative; it is an estimate, not a calibration. | [Process › Normalize](/docs/desktop/processing/#normalization) |
| **Pre-edge and post-edge lines** | Baselines fitted below and above the edge. The pre-edge line is subtracted; the post-edge fit defines the edge step. | [Process › Normalize](/docs/desktop/processing/#normalization) |
| **Edge step** | The size of the absorption jump at $E_0$. Dividing by it produces a spectrum that rises from 0 to 1. | [Processing equations](/docs/science/processing/#2-find-the-edge-and-normalize-the-absorption) |
| **norm / flat** | Normalized absorption; the flattened version also removes the fitted post-edge trend so the region above the edge is level. | [Process › Normalize](/docs/desktop/processing/#normalization) |
| **k** | The photoelectron wave number in Å⁻¹, computed from the energy above $E_0$. EXAFS is analyzed on this axis. | [Process › Background](/docs/desktop/processing/#background) |
| **$\chi(k)$** | The EXAFS oscillations: the normalized absorption minus the smooth background, as a function of k. It is dimensionless. | [Process › Background](/docs/desktop/processing/#background) |
| **AUTOBK** | The algorithm that estimates the smooth background with a spline chosen so that the low-R part of the Fourier transform is small. | [AUTOBK objective](/docs/science/autobk/) |
| **$R_{\mathrm{bkg}}$** | The AUTOBK cutoff distance in Å. Fourier components below it are treated as background. The starting value is 1 Å. | [AUTOBK objective](/docs/science/autobk/) |
| **k weight** | The power of k multiplied into $\chi(k)$ before transforming or plotting. It compensates for the decay of the oscillations at high k. The forward transform starts with weight 2. | [Process › Transform](/docs/desktop/processing/#forward-transform) |
| **Window** | A taper applied to $\chi(k)$ before the transform to reduce truncation ripples. rexafs starts with a Kaiser–Bessel window from 2 to 15 Å⁻¹. | [Process › Transform](/docs/desktop/processing/#forward-transform) |
| **$\chi(R)$** | The Fourier transform of weighted, windowed $\chi(k)$, as a function of distance R in Å. Its magnitude has peaks near neighbor distances, shifted by a scattering phase. | [Processing equations](/docs/science/processing/#4-transform-from-k-to-r) |
| **Phase correction** | The shift between a peak in $\chi(R)$ and the actual interatomic distance. rexafs does not phase-correct plots; fitted distances come from the path model. | [Fit statistics](/docs/science/fitting-statistics/) |
| **q and $\chi(q)$** | q is the back-transform coordinate in Å⁻¹; $\chi(q)$ is the signal reconstructed from a selected R range. | [Back transform](/docs/desktop/processing/#back-transform) |

## Structures and fitting

| Term | Meaning | Where it is used |
|---|---|---|
| **Structure** | A reference geometry: a built-in model, a CIF or XYZ file, or an entry from Materials Project, AMCSD or COD. It defines where neighbors are expected. | [Structures and paths](/docs/desktop/structures/) |
| **Absorber** | The atom whose edge was measured. The scattering calculation is centered on it. | [Structures and paths](/docs/desktop/structures/#find-a-structure) |
| **Cluster radius** | The distance in Å around the absorber within which atoms are included in the calculation. The desktop starts at 8 Å. | [Structures and paths](/docs/desktop/structures/#find-a-structure) |
| **FEFF, ReFEFF and FEFF10** | Programs that compute scattering amplitudes and phases for each path. Every 0.2.11 desktop package includes ReFEFF and FEFF10. Record the engine you used. | [Calculate and select paths](/docs/desktop/structures/#calculate-and-select-paths) |
| **Scattering path** | One route the photoelectron can take from the absorber to one or more neighbors and back. Single-scattering paths visit one neighbor; multiple-scattering paths visit several. | [Structures and paths](/docs/desktop/structures/#path-parameters) |
| **Shell** | A group of neighbors at about the same distance from the absorber. The **first shell** is the nearest one. | [First fit](/docs/desktop/fitting/) |
| **Degeneracy (N)** | The number of equivalent paths in the reference structure, equal to the coordination number for a single-scattering shell. | [Path parameters](/docs/desktop/structures/#path-parameters) |
| **$S_0^2$** | The amplitude reduction factor, a dimensionless scale usually between 0.7 and 1. It multiplies N, so the two cannot be separated from one fit. | [Path parameters](/docs/desktop/structures/#path-parameters) |
| **$\Delta E_0$** | A fitted shift in eV between the experimental energy origin and the calculation's. | [Path parameters](/docs/desktop/structures/#path-parameters) |
| **$\Delta R$** | A fitted change in Å from the path's reference half-length. The fitted distance is the reference length plus $\Delta R$. | [Path parameters](/docs/desktop/structures/#path-parameters) |
| **$\sigma^2$** | The mean-square disorder of the path length in Å², from thermal motion and structural variation. Larger values damp high-k oscillations. | [Path parameters](/docs/desktop/structures/#path-parameters) |
| **Fit range** | The k and R limits inside which the model is compared with the data. They determine how much independent information the fit has. | [Fit statistics](/docs/science/fitting-statistics/#independent-information-and-reported-chi-square) |
| **R-factor** | The squared misfit divided by the squared data, a dimensionless measure of how closely the model follows the data. It does not prove the model is correct. | [Fit statistics](/docs/science/fitting-statistics/#r-factor) |
| **Joint fit** | Fitting several spectra at once with some variables shared and others local to each spectrum. | [Multiple spectra](/docs/desktop/multiple-spectra/) |
| **Batch** | Fitting every frame of a series independently with the same model, to follow a trend. | [Independent batches](/docs/desktop/multiple-spectra/#independent-batches) |
| **LCF** | Linear combination fitting: modeling an unknown spectrum as a weighted sum of reference spectra. | [LCF, PCA and data treatment](/docs/science/analysis/) |
| **PCA** | Principal component analysis: describing spectral variation with orthogonal components. Their number does not by itself identify chemical species. | [LCF, PCA and data treatment](/docs/science/analysis/#principal-component-analysis) |

## Desktop objects

| Term | Meaning | Where it is used |
|---|---|---|
| **Group** | One imported spectrum with its own processing settings, or a derived result. Groups are listed in the side panel. | [Import and groups](/docs/desktop/import/#recipes-and-groups) |
| **Current group and marks** | The current group is the one you edit and inspect. Marked groups receive comparison plots and bulk actions. | [Import and groups](/docs/desktop/import/#recipes-and-groups) |
| **Recipe** | A saved column mapping that is reused for files with the same layout. | [Import and groups](/docs/desktop/import/#recipes-and-groups) |
| **Stage** | One step of the desktop workflow: Data, Process, Fit, Series or Publish. Process combines normalization, background removal and transforms; releases up to 0.2.14 show these as separate Normalize, Background and Transform stages. | [Desktop overview](/docs/desktop/) |
| **Series** | A folder of scans treated as ordered frames, for browsing and trend plots. | [Series and trends](/docs/desktop/series/) |
| **Project (`.rxs`)** | The saved analysis: settings, models, fit history and publication choices, with linked or embedded source data. The previous save is kept as `.rxs.bak`. | [Projects and recovery](/docs/desktop/projects/) |
| **Analysis folder** | An export containing figures, data tables, captions, a printable report, a methods draft, references and the project. | [Publication and exports](/docs/desktop/publication/#analysis-folder-contents) |

## File formats

| Format | Description |
|---|---|
| **Plain-text columns** | Whitespace- or comma-separated numeric columns with `#` comments. The import review assigns energy, intensities or μ. Most beamline exports fit this description. |
| **XDI** | The XAS Data Interchange format, with named columns, units and metadata in the header. See [import XDI data](/docs/desktop/xdi/). |
| **QAS transmission** | Files with energy, $I_0$ and $I_t$ in the first three columns, read by the Python `read_qas_transmission` function. |
| **CIF and XYZ** | Crystal and molecular structure files used to build scattering clusters. |
| **`.rxs`** | The rexafs project format. |
| **Athena `.prj`** | Readable through the Rust `io` module for interchange with Demeter/Athena projects. |
