"""Resolve a release tag and require its successful, manually dispatched build."""
import json
import os
import re
import subprocess
import sys
import tomllib


def validate(run, tag, commit, version):
    if not re.fullmatch(r"v\d+\.\d+\.\d+(?:-(?:alpha|beta|rc)\.\d+)?", tag):
        raise ValueError("Expected a coordinated release tag")
    if tag != f"v{version}":
        raise ValueError("Tag does not match the source version")
    expected = {"conclusion": "success", "head_sha": commit,
                "head_branch": tag, "event": "workflow_dispatch",
                "path": ".github/workflows/release-build.yml"}
    for key, value in expected.items():
        if run.get(key) != value:
            raise ValueError(f"Release build has an unexpected {key}")


if __name__ == "__main__":
    tag, run_id = sys.argv[1:]
    if not re.fullmatch(r"v\d+\.\d+\.\d+(?:-(?:alpha|beta|rc)\.\d+)?", tag) or not run_id.isdecimal():
        raise SystemExit("Invalid release tag or build run ID")
    commit = subprocess.check_output(["git", "rev-parse", f"refs/tags/{tag}^{{commit}}"], text=True).strip()
    cargo = subprocess.check_output(["git", "show", f"{commit}:Cargo.toml"], text=True)
    version = tomllib.loads(cargo)["workspace"]["package"]["version"]
    run = json.loads(subprocess.check_output([
        "gh", "api", f"repos/{os.environ['GITHUB_REPOSITORY']}/actions/runs/{run_id}"]))
    validate(run, tag, commit, version)
    with open(os.environ["GITHUB_OUTPUT"], "a") as output:
        output.write(f"commit={commit}\nversion={version}\n")
    print(f"Qualified {tag} at {commit}, build {run_id}")
