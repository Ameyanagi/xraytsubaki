"""Package and smoke-test a native release binary. Does not publish or sign."""
import hashlib
import json
import os
import platform
import plistlib
import shutil
import subprocess
import tempfile
from pathlib import Path

root = Path(__file__).resolve().parents[1]
metadata = json.loads(subprocess.check_output(
    ["cargo", "metadata", "--locked", "--no-deps", "--format-version", "1"], cwd=root))
version = next(p["version"] for p in metadata["packages"] if p["name"] == "rexafs-gui")
target = subprocess.check_output(["rustc", "-vV"], text=True).split("host: ")[1].splitlines()[0]
system = platform.system()
if system not in {"Darwin", "Linux", "Windows"}:
    raise SystemExit(f"Unsupported desktop host: {system}")
stem = f"rexafs-{version}-{target}"
out = root / "target/distributions"
bundle = out / stem
if bundle.exists():
    shutil.rmtree(bundle)
binary_name = "rexafs.exe" if system == "Windows" else "rexafs"
binary_relative = Path("rexafs.app/Contents/MacOS/rexafs") if system == "Darwin" else Path(binary_name)
binary = bundle / binary_relative
binary.parent.mkdir(parents=True, exist_ok=True)
shutil.copy2(Path(metadata["target_directory"]) / "release" / binary_name, binary)
resources = bundle / ("rexafs.app/Contents/Resources" if system == "Darwin" else "resources")
resources.mkdir(parents=True, exist_ok=True)
shutil.copy2(root / "assets/brand/rexafs-icon.png", resources / "rexafs-icon.png")
if system == "Darwin":
    shutil.copy2(root / "assets/brand/rexafs.icns", resources / "rexafs.icns")
elif system == "Windows":
    shutil.copy2(root / "assets/brand/rexafs.ico", resources / "rexafs.ico")
examples = resources / "examples"
examples.mkdir(parents=True)
fixtures = root / "crates/rexafs/tests/testfiles/xraylarch_d867"
shutil.copy2(fixtures / "xafsdata/cu_150k.xmu", examples / "cu_150k.xmu")
shutil.copy2(fixtures / "README.md", examples / "PROVENANCE.md")
if system == "Darwin":
    with (bundle / "rexafs.app/Contents/Info.plist").open("wb") as f:
        plistlib.dump({
            "CFBundleExecutable": "rexafs", "CFBundleIdentifier": "com.rexafs.desktop",
            "CFBundleName": "rexafs", "CFBundleDisplayName": "rexafs", "CFBundlePackageType": "APPL",
            "CFBundleIconFile": "rexafs.icns",
            "CFBundleShortVersionString": version, "CFBundleVersion": version.split("-")[0],
            "NSHighResolutionCapable": True,
            "CFBundleDocumentTypes": [{"CFBundleTypeName": "rexafs project",
                "CFBundleTypeExtensions": ["rxs"], "CFBundleTypeRole": "Editor"}],
        }, f)
for name in ["LICENSE-MIT", "LICENSE-APACHE"]:
    shutil.copy2(root / name, bundle / name)
# Include exact resolved third-party license declarations for release review.
dependencies = json.loads(subprocess.check_output([
    "cargo", "metadata", "--locked", "--format-version", "1", "--filter-platform", target,
    "--manifest-path", "crates/rexafs-gui/Cargo.toml", "--no-default-features", "--features", "refeff-runner",
], cwd=root))
packages = {p["id"]: p for p in dependencies["packages"]}
nodes = {n["id"]: n for n in dependencies["resolve"]["nodes"]}
pending = [p["id"] for p in dependencies["packages"] if p["name"] == "rexafs-gui"]
included = set()
while pending:
    package_id = pending.pop()
    if package_id in included:
        continue
    included.add(package_id)
    for dep in nodes[package_id]["deps"]:
        if any(k["kind"] != "dev" for k in dep["dep_kinds"]):
            pending.append(dep["pkg"])
notices = bundle / "licenses"
notices.mkdir()
inventory = []
for package_id in sorted(included):
    p = packages[package_id]
    inventory.append({k: p.get(k) for k in ["name", "version", "license", "repository", "source"]})
    source_dir = Path(p["manifest_path"]).parent
    candidates = set(source_dir.glob("LICENSE*")) | set(source_dir.glob("COPYING*")) | set(source_dir.glob("NOTICE*"))
    if p.get("license_file"):
        candidates.add(source_dir / p["license_file"])
    for source in candidates:
        if source.is_file():
            dest = notices / f"{p['name']}-{p['version']}" / source.name
            dest.parent.mkdir(exist_ok=True)
            shutil.copy2(source, dest)
(bundle / "dependencies.json").write_text(json.dumps(inventory, indent=2) + "\n")
(bundle / "README.txt").write_text(
    f"rexafs {version}\nRust-powered X-ray absorption analysis\nhttps://rexafs.com\n\n"
    "Built with ReFEFF. Keep the extracted directory together; it contains the example and notices.\n"
    "Open rexafs.app on macOS or run rexafs / rexafs.exe on Linux / Windows.\n"
    "This archive has no publisher code signature; macOS notarization is not included.\n"
    "Linux requires a graphical session, Vulkan-capable driver, fontconfig and xkbcommon.\n"
    "Save .rxs projects with relative source paths (default), or select Raw: embedded for portable originals.\n"
    "Run the executable with --self-check for a display-free packaged example check.\n"
)
if system in {"Darwin", "Linux"}:
    command = ["otool", "-L", str(binary)] if system == "Darwin" else ["ldd", str(binary)]
    linked = subprocess.check_output(command, text=True)
    if system == "Linux" and "not found" in linked:
        raise SystemExit(f"Missing dynamic library:\n{linked}")
    if system == "Darwin" and any(prefix in linked for prefix in ["/opt/homebrew/", "/usr/local/", "/Users/"]):
        # The first otool line names the executable itself; inspect only dependencies.
        if any(prefix in "\n".join(linked.splitlines()[1:]) for prefix in ["/opt/homebrew/", "/usr/local/", "/Users/"]):
            raise SystemExit(f"Non-system dynamic library:\n{linked}")
    (bundle / "linked-libraries.txt").write_text(linked)
(bundle / "build.json").write_text(json.dumps({
    "version": version, "target": target, "features": ["refeff-runner"],
    "rustc": subprocess.check_output(["rustc", "--version"], text=True).strip(),
    "commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip(),
    "dirty": bool(subprocess.check_output(["git", "status", "--porcelain"], cwd=root)),
    "github_run_id": os.environ.get("GITHUB_RUN_ID"),
    "signed": False, "notarized": False,
}, indent=2) + "\n")
if system == "Darwin":
    archive = out / f"{stem}.zip"
    subprocess.run(["ditto", "-c", "-k", "--sequesterRsrc", "--keepParent", str(bundle), str(archive)], check=True)
else:
    archive = Path(shutil.make_archive(str(out / stem), "gztar" if system == "Linux" else "zip", out, stem))
# Check the extracted archive in a fresh location, not just the build tree.
with tempfile.TemporaryDirectory(prefix="rexafs-package-") as directory:
    if system == "Darwin":
        subprocess.run(["ditto", "-x", "-k", str(archive), directory], check=True)
    else:
        shutil.unpack_archive(archive, directory)
    extracted = Path(directory) / stem / binary_relative
    subprocess.run([str(extracted), "--version"], cwd=directory, check=True)
    subprocess.run([str(extracted), "--self-check"], cwd=directory, check=True)
checksum = hashlib.sha256(archive.read_bytes()).hexdigest()
Path(str(archive) + ".sha256").write_text(f"{checksum}  {archive.name}\n")
print(archive)
