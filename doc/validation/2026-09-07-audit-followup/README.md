# Overlay and palette follow-up audit

[Open the seven-capture gallery](index.html). Tracking issue: [#28](https://github.com/Ameyanagi/rexafs/issues/28).

The findings review identified a shared-axis error when overlaid groups had different FT k weights and missing pointer isolation in the command palette. This audit uses a copied public Cu fixture with identical background processing and FT weights 1 and 3, in a separate application.

- Both χ(k) traces coincide when displayed with the active group's weight 1. Switching the active group to weight 3 makes both curves follow k³χ(k), still coinciding.
- Fourier curves retain their original group-specific values. Mixed-weight R/q axes do not claim one group's physical units, and legends identify each FT weight. A visible note explains the display behavior.
- Open the palette and scroll twice over the exposed R plot. The R axis remains 0–10 Å. Cmd+5 does not change the underlying Transform stage while the modal is open.
- Escape closes the palette; Cmd+3 then reaches Background. Executing Updates from the palette focuses the update dialog correctly, and closing it restores normal shortcuts.

The numerical regression checks both active-group choices, exact common-weight χ(k) arrays, unchanged Fourier arrays and unchanged processing weights. All eight plotting tests and all 15 project tests pass. The retained 0.1.2 fixtures also give the Ru primary channel its own defaults so it does not inherit the Cu fixture's explicit E0.

These checks address B14/B16 from the findings review. The broader project-lifecycle, parser/resource-limit and profiling proposals remain separate follow-up work.
