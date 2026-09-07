"""Build drag-to-Applications DMGs and check installation from a mounted image."""
from contextlib import contextmanager
import hashlib
import json
import os
from pathlib import Path
import plistlib
import re
import shutil
import subprocess
import tempfile
from zipfile import ZipFile

from desktop_channels import app_name

TARGETS = {"aarch64-apple-darwin", "x86_64-apple-darwin"}
NOTICES = ("LICENSE-MIT", "LICENSE-APACHE", "dependencies.json", "licenses")


def sha256(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def installer_name(metadata):
    channel = metadata.get("channel", "stable")
    app_name(channel)
    version, target = metadata["version"], metadata["target"]
    if not re.fullmatch(r"\d+\.\d+\.\d+(?:-(?:alpha|beta|rc)\.\d+)?", version) or target not in TARGETS:
        raise ValueError("Unsupported installer version or target")
    return f"rexafs-{version}-{target}.dmg"


def include_notices(bundle, metadata):
    """Call BEFORE signing: copying only the app must retain all its notices."""
    app = bundle / app_name(metadata.get("channel", "stable"))
    destination = app / "Contents/Resources/notices"
    destination.mkdir(parents=True, exist_ok=True)
    for name in NOTICES:
        source = bundle / name
        if source.is_dir():
            shutil.copytree(source, destination / name, dirs_exist_ok=True)
        else:
            shutil.copy2(source, destination / name)
    return app


def check_payload(root, metadata):
    """Reject a wrong app identity, lost notices, or a broken Applications link."""
    expected_name = app_name(metadata.get("channel", "stable"))
    app = root / expected_name
    if sorted(p.name for p in root.glob("*.app")) != [expected_name]:
        raise ValueError("Installer must contain exactly the intended channel's app")
    applications = root / "Applications"
    if not applications.is_symlink() or os.readlink(applications) != "/Applications":
        raise ValueError("Installer Applications link must point to /Applications")
    with (app / "Contents/Info.plist").open("rb") as stream:
        info = plistlib.load(stream)
    expected_id = "com.rexafs.nightly" if metadata.get("channel") == "nightly" else "com.rexafs.desktop"
    if (info.get("CFBundleIdentifier") != expected_id
            or info.get("CFBundleShortVersionString") != metadata["version"].split("-")[0]
            or info.get("CFBundleExecutable") != "rexafs"):
        raise ValueError("Installed app identity does not match the qualified build")
    for name in NOTICES:
        path = app / "Contents/Resources/notices" / name
        if not path.exists() or (name == "licenses" and not any(path.iterdir())):
            raise ValueError("Installed app is missing license notices: " + name)
    return app


def build_installer(bundle, output, metadata):
    # Imported lazily: source/provenance tests run on Linux without macOS tools.
    import dmgbuild

    name = app_name(metadata.get("channel", "stable"))
    app = bundle / name
    label = name.removesuffix(".app")
    architecture = "Apple Silicon" if metadata["target"] == "aarch64-apple-darwin" else "Intel"
    output.parent.mkdir(parents=True, exist_ok=True)
    if output.exists():
        raise ValueError("Refusing to replace an existing installer")
    with tempfile.TemporaryDirectory(prefix="rexafs-dmg-content-") as temporary:
        instructions = Path(temporary) / "Install.txt"
        instructions.write_text(
            f"{label} {metadata['version']} for {architecture}\n\n"
            f"1. Drag {name} onto Applications.\n"
            "2. Eject this disk image.\n"
            f"3. Open {label} from Applications.\n\n"
            "Before replacing an older copy, save your project and quit that app.\n"
            "Stable and Nightly have different names and can be installed together.\n"
            "Nightly contains recent changes and is intended for testing.\n\n"
            "Licenses are included inside the application: Show Package Contents,\n"
            "then Contents/Resources/notices. Your project files are separate.\n"
            "https://rexafs.com\n", encoding="utf-8")
        provenance = Path(temporary) / "build.json"
        provenance.write_text(json.dumps(metadata, indent=2) + "\n")
        dmgbuild.build_dmg(str(output), f"{label} {metadata['version']} ({architecture})", settings={
            "format": "UDZO", "filesystem": "HFS+", "compression_level": 9,
            "files": [str(app), str(instructions), str(provenance)],
            "symlinks": {"Applications": "/Applications"},
            "icon": str(app / "Contents/Resources/rexafs.icns"),
            "background": None, "window_rect": ((160, 160), (640, 440)),
            "default_view": "icon-view", "icon_size": 96, "text_size": 14,
            "icon_locations": {name: (160, 125), "Applications": (480, 125), "Install.txt": (320, 245)},
            "hide": ["build.json"], "hide_extensions": [name, "Install.txt"],
            "show_toolbar": False, "show_status_bar": False, "show_pathbar": False,
            "show_tab_view": False, "show_sidebar": False,
            "include_icon_view_settings": True, "include_list_view_settings": False,
        })


@contextmanager
def mounted_image(image):
    """Use an owned mountpoint; never detach someone else's mounted volume."""
    with tempfile.TemporaryDirectory(prefix="rexafs-dmg-mount-") as temporary:
        mount = Path(temporary) / "volume"
        mount.mkdir()
        subprocess.run(["hdiutil", "attach", "-readonly", "-nobrowse", "-noautoopen",
                        "-mountpoint", str(mount), str(image)], check=True)
        try:
            yield mount
        finally:
            subprocess.run(["hdiutil", "detach", str(mount)], check=True)


def verify_installation(image, metadata, verify_app=None):
    """Exercise the installed app in a temporary folder, not /Applications."""
    subprocess.run(["hdiutil", "verify", str(image)], check=True)
    with mounted_image(image) as mounted:
        if json.loads((mounted / "build.json").read_text()) != metadata:
            raise ValueError("Mounted installer build provenance differs")
        source = check_payload(mounted, metadata)
        if verify_app:
            verify_app(source, metadata["apple_team_id"])
        with tempfile.TemporaryDirectory(prefix="rexafs-installed-") as temporary:
            installed = Path(temporary) / source.name
            subprocess.run(["ditto", str(source), str(installed)], check=True)
            if verify_app:
                verify_app(installed, metadata["apple_team_id"])
            executable = installed / "Contents/MacOS/rexafs"
            actual = json.loads(subprocess.check_output([str(executable), "--build-info"], text=True))
            for key in ("version", "commit", "channel", "release_tag"):
                if actual.get(key) != metadata.get(key):
                    raise ValueError("Installed binary has incorrect " + key)
            subprocess.run(["lipo", str(executable), "-verify_arch",
                            "arm64" if metadata["target"] == "aarch64-apple-darwin" else "x86_64"], check=True)
            subprocess.run([str(executable), "--self-check"], cwd=temporary, check=True)
            return sha256(executable)


def qualify_installer(image, metadata, archive):
    """Check CI signing/install evidence before an installer may be uploaded."""
    if image.name != installer_name(metadata):
        raise ValueError("Unexpected installer name")
    evidence_file = Path(str(image) + ".json")
    evidence = json.loads(evidence_file.read_text())
    if evidence.get("schema_version") != 1 or evidence.get("build") != metadata:
        raise ValueError("Installer does not match the qualified archive's provenance")
    expected = {"signed": True, "notarized": True, "stapled": True,
                "gatekeeper_accepted": True, "installation_verified": True,
                "archive_sha256": sha256(archive), "sha256": sha256(image),
                "apple_team_id": metadata.get("apple_team_id")}
    if any(evidence.get(key) != value for key, value in expected.items()):
        raise ValueError("Installer qualification or checksum does not match")
    if (not metadata.get("signed") or not metadata.get("notarized") or not metadata.get("apple_team_id")
            or not evidence.get("notarization_id")
            or not re.fullmatch(r"[0-9a-f]{64}", evidence.get("executable_sha256", ""))):
        raise ValueError("Installer is missing signing or installation provenance")
    with ZipFile(archive) as zipped:
        executable = archive.stem + "/" + app_name(metadata.get("channel", "stable")) + "/Contents/MacOS/rexafs"
        with zipped.open(executable) as stream:
            if hashlib.file_digest(stream, "sha256").hexdigest() != evidence["executable_sha256"]:
                raise ValueError("Installer executable differs from the qualified archive")
    sidecar = Path(str(image) + ".sha256")
    if sidecar.read_text().strip() != f"{expected['sha256']}  {image.name}":
        raise ValueError("Installer checksum sidecar does not match")
    return {p.name: sha256(p) for p in (image, sidecar, evidence_file)}
