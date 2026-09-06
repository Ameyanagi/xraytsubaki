"""Require a fixture for this release and verify all retained project samples."""
import hashlib
import json
import tomllib
from pathlib import Path

root = Path(__file__).resolve().parents[1]
version = tomllib.loads((root / "Cargo.toml").read_text())["workspace"]["package"]["version"]
fixtures = root / "crates/rexafs-gui/tests/fixtures/projects"
manifest = json.loads((fixtures / "manifest.json").read_text())
assert version in manifest["releases"], f"Record a compatibility fixture for release {version}"
retained = {p.relative_to(fixtures).as_posix() for p in fixtures.rglob("*")
            if p.is_file() and p.name not in {"README.md", "manifest.json"}}
assert set(manifest["sha256"]) == retained, "Every project and source sample must be checksummed"
assert set(manifest["invalid_projects"]) <= set(manifest["sha256"])
for name, digest in manifest["sha256"].items():
    assert hashlib.sha256((fixtures / name).read_bytes()).hexdigest() == digest, f"Historical fixture changed: {name}"
for release, record in manifest["releases"].items():
    for field, mode in [("project", "paths"), ("embedded_project", "embedded")]:
        name = record[field]
        assert name.endswith(".rxs") and name in manifest["sha256"], f"Missing {mode} fixture for {release}"
        project = json.loads((fixtures / name).read_text())
        assert project["version"] == record["format_version"] == project["header"]["format_version"]
        assert project["header"]["software_version"] == release
        assert project["header"]["storage"] == mode
        assert project["header"]["path_base"] == "project_directory"
        assert project["header"]["format"] == "rxs"
print(f"Compatibility fixtures verified: {len(manifest['sha256'])} samples; release {version}")
