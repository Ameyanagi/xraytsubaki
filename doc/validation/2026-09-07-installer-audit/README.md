# Mac installer preview audit — 7 September 2026

These captures show an unsigned/ad hoc **preview**, not a notarized release installer. The input application is the qualified Stable 0.1.2 binary (source `b4fd8c08d15a1cb62e2315bb64e037f0f0b68c30`). Packaging modifies only a temporary copy to include license notices, leaving the public release archive untouched.

- [Finder with hidden files visible](01-preview-hidden-files-visible.png): the local Finder preference exposes the normally hidden volume icon. This preference was preserved.
- [Normal Finder view](02-preview-standard-finder.png): application, Applications shortcut and installation instructions fit inside the 640 × 440 window even with the user's path/status bars enabled. Finder's hidden-file preference was temporarily toggled for this capture and then restored.

The ARM and Intel previews both passed disk-image verification, mount, Applications-link and bundle-identity checks, installed license checks, a fresh `ditto` installation copy, exact build identity, Mach-O architecture, and `--self-check`. Intel was exercised through Rosetta on the local ARM Mac; the pull request also exercises native Intel CI. The initial 640 × 380 layout clipped the instructions; the final shared layout was enlarged and the instructions moved upward.

The preview does not verify Developer ID or notarization. Those checks belong to the signing workflow and must pass for both architectures before release installers are published.
