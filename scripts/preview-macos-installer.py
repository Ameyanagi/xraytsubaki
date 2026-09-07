"""Build an UNSIGNED DMG preview from a packaged app; never for publication."""
import argparse
import json
from pathlib import Path
import platform
import subprocess
import tempfile

from macos_installer import build_installer, include_notices, installer_name, verify_installation


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("bundle", type=Path, help="Packaged directory containing build.json and the app")
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    if platform.system() != "Darwin":
        raise SystemExit("Installer previews require macOS")
    metadata = json.loads((args.bundle / "build.json").read_text())
    # A preview cannot inherit a release's signing claim, and changes only a copy.
    metadata.update(signed=False, notarized=False, installer_preview=True)
    image = args.output.resolve() / installer_name(metadata).replace(".dmg", "-preview.dmg")
    with tempfile.TemporaryDirectory(prefix="rexafs-dmg-preview-") as temporary:
        bundle = Path(temporary) / "bundle"
        subprocess.run(["ditto", str(args.bundle.resolve()), str(bundle)], check=True)
        app = include_notices(bundle, metadata)
        subprocess.run(["codesign", "--force", "--sign", "-", str(app)], check=True)
        build_installer(bundle, image, metadata)
        verify_installation(image, metadata)
    print(image)
