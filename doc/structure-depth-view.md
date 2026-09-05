# Structure slices and depth cues

Open **Slice & depth** above the structure canvas. Its controls replace the
library/setup panel temporarily; **Back** returns to the normal panel while
keeping the visual effect. In Paths, they temporarily replace the path table.

## Inspect a coordination center

Click a visible atom from the FEFF cluster to make it the inspection center.
**Use absorber** returns to the calculation's center. **Through center** sets
the slice position to zero relative to that atom. Inspection never changes the
calculation absorber, cluster, scattering paths, or fit parameters.

Choose **Around center**, then set **Clear radius** to encompass the coordination
shell of interest. Atoms within that sphere retain the overall opacity; more
distant atoms fade smoothly. The transition width is 35% of the clear radius,
with a 0.5 Å minimum. Fade strength controls how faint the distant context becomes.
For rutile, a 2.5 Å clear radius includes the six oxygen neighbours of Ti.

## Slice through the structure

- **Slab:** retain a thickness centered on Position, both in Å. The initial
  thickness is 4 Å; the FEFF calculation radius remains 8 Å by default.
- **Cutaway:** remove the side beyond Position along the positive normal.
  With the View normal, this removes the foreground to expose the interior.
- **View:** the normal follows the camera. Negative positions move farther
  from the viewer; positive positions move nearer.
- **X / Y / Z:** fixed Cartesian normals. These stay fixed to the structure
  during rotation; they are not fractional crystallographic axes or Miller planes.
- **Faint outside context:** show removed geometry at 6% of its otherwise
  computed opacity. It cannot intercept atom picking.

Drag the sliders or use − / + for 0.1 Å adjustments. The legend reports the
retained atom-center count and slice interval. A slice with no retained atom
centers has an explicit message. Empty-space clicks preserve the active center.

Bonds, unit-cell edges, radius guides, path arrows, and polyhedron faces are
clipped at the planes. A crossing bond is retained even if both end atoms are
outside the slab. Atom visibility is determined by its center; this does not
cut a sphere into a flat cap. Cut polyhedra remain open surfaces rather than
inventing additional coordination faces. Faint context is partitioned from the
retained geometry to avoid drawing the retained part twice.

## Transparency and gradient

**Opacity** sets the overall alpha, from 10% to 100%. It multiplies the existing
polyhedron face opacity and the faded context outside the FEFF cluster.
**Back → front** makes distant atoms faint and nearby atoms clear, following
the camera even when the slicing axis is fixed. **Around center** instead fades
by radial distance from the inspection center. Both modes have a strength slider.

The effects are independent: a slab can use uniform alpha, a depth gradient,
or a radial fade. **Reset view effects** restores the full structure with no
slicing or extra fading, preserving camera orientation and zoom. The controls
are session display settings, not serialized fit/project parameters.

Translucent balls use a single gradient primitive so layered highlights do not
accumulate alpha incorrectly. Bonds interpolate the fade along their lengths;
polyhedron faces use the clipped face's centroid for the fade. Rendering remains
the depth-sorted native canvas, with its existing limitations for intersecting
transparent surfaces.

## Release verification — 2026-09-05

The full release GUI suite passes 87 tests, with two local diagnostics ignored.
Seven slicing regressions cover crossing bonds, clipped faces, coplanar boundary
faces in faint-context mode, cutaway direction, fade direction/radius, hidden
atom picking, and the camera/fixed-axis normal distinction. The fade transition
was tightened after desktop inspection and the seven regressions rerun.

Native computer-use checks on the 209-atom, 8 Å rutile cluster:

- A 4 Å View slab retains 75 centers; narrowing it to 2 Å retains 41. Moving
  the 2 Å slab to +2.9 Å retains 30, without changing the 144% zoom.
- Clicking visible Ti atom 23 changes the inspection center. Use absorber
  restores Ti atom 0. The calculation retains all 209 atoms.
- The View cutaway through the absorber retains 105 centers. A Cartesian Z
  cutaway retains 119 before and after rotation, with zoom unchanged.
- Polyhedron faces visibly clip at the slab planes. Outside-context opacity,
  overall opacity, both fade modes, radius adjustment and reset were exercised.

Screenshots: [2 Å polyhedron slab](validation/structure-polyhedron-slice.jpg),
[fixed-Z cutaway](validation/structure-cutaway-z.jpg),
[radial fade around the absorber](validation/structure-center-fade.jpg), and
[isolated coordination with faint context](validation/structure-coordination-focus.jpg).

## Bond density and absorber controls — 2026-09-06

Ball-and-stick and wireframe now default to **Auto** bonds: covalent-radius
contacts are filtered against the nearest heavy-atom neighborhood at both ends.
Covalent bonds to hydrogen in complete molecules are preserved by the molecular reconstruction.
This is a display heuristic, not a chemical bond-order assignment. **All contacts**
restores the broad cutoff view; **Absorber bonds** keeps only nearest bonds incident
to the actual FEFF absorber; **None** hides bonds. Polyhedron topology is unchanged.
In rutile this reduces 572 contacts to 340 Ti–O bonds, or six absorber bonds.
Cu/Ni/Ru nearest-shell checks retain twelve neighbors, and Urea/Aspirin retain
all seven/twenty-one molecular bonds. Sticks end at their visible atom surfaces;
wireframe lines are thinner. Hidden slice endpoints do not incorrectly trim
crossing bonds at invisible sphere boundaries.

**Highlight absorber** changes the actual absorber's atom color to bright cyan.
It is an opaque color overlay at the atom's projected position, so foreground
atoms cannot hide it. Slice and atom-style visibility still apply.
Other atoms keep their element colors. **Find absorber** restores its slice origin
and a fitted zoom, selects the absorber, and adds a labeled pointer above the scene.
Clicking to inspect an atom dismisses the pointer without changing absorber identity.
A slice excluding the absorber explains how to find it. Opacity presets are directly
beside the display controls, with continuous adjustment in Slice & depth.

The native release GUI was checked in cluster-only, ball-and-stick, wireframe,
Auto / All contacts / Absorber bonds, and 50% opacity modes. The FEFF cluster
remains 209 atoms at 8 Å; these controls change display only.

Native release check: [rutile at 50% opacity with color-only absorber](validation/structure-absorber-color.jpg).
