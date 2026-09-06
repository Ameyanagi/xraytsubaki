"""Check the coordinated release version, optionally against a v-prefixed tag."""
import json
import re
import sys
import tomllib
from pathlib import Path

root = Path(__file__).resolve().parents[1]
workspace = tomllib.loads((root / "Cargo.toml").read_text())
version = workspace["workspace"]["package"]["version"]
assert json.loads((root / "js-rexafs/package.json").read_text())["version"] == version
for name in ["crates/rexafs", "crates/rexafs-gui", "crates/rexafs-wasm", "py-rexafs"]:
    package = tomllib.loads((root / name / "Cargo.toml").read_text())["package"]
    assert package["version"] == {"workspace": True}, f"{name}: version must inherit workspace"
assert re.fullmatch(r"\d+\.\d+\.\d+(?:-(?:alpha|beta|rc)\.\d+)?", version), version
if len(sys.argv) > 1:
    assert sys.argv[1] == f"v{version}", f"tag {sys.argv[1]} does not match v{version}"
print(version)
