# `xraytsubaki-gui` (Deprecated)

This Dioxus-based GUI is deprecated and no longer the primary desktop client.

Use the Tauri + React application instead:

- App root: `xraytsubaki-app/`
- Dev server: `cd xraytsubaki-app && npm run dev`
- Desktop app: `cd xraytsubaki-app && npm run tauri dev`

Migration status:

- New feature work goes to `xraytsubaki-app`.
- CI now validates the Tauri app path instead of this crate.
- This crate remains in the repository only for legacy reference during transition.
