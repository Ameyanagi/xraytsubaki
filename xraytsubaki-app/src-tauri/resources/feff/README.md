# Bundled FEFF Modules

Place FEFF85EXAFS module executables in the platform directory for packaging.

Required module names:
- `feff8l_rdinp`
- `feff8l_pot`
- `feff8l_xsph`
- `feff8l_pathfinder`
- `feff8l_genfmt`
- `feff8l_ff2x`

Windows requires `.exe` suffix for each module.

Platform directories:
- `macos-aarch64/`
- `macos-x86_64/`
- `linux-aarch64/`
- `linux-x86_64/`
- `windows-x86_64/`

Build source and attribution:
- Source: https://github.com/xraypy/feff85exafs
- Keep `THIRD_PARTY_NOTICES.md` included with distributions.
