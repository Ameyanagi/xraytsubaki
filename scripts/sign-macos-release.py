"""Sign and notarize an existing GitHub-built desktop ZIP without rebuilding it."""
import argparse
import base64
from contextlib import contextmanager
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import platform
import secrets
import shlex
import subprocess
import tempfile
import time
from zipfile import ZipFile
from desktop_channels import app_name
from macos_installer import build_installer, include_notices, installer_name, verify_installation
from release_downloads import targets_for_version


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def check_source(archive, manifest, version, commit, run_id, target, channel="stable", release_tag=None):
    """Return unsigned archive metadata after matching its source build identity.

    Check the archive's SHA-256 against the supplied manifest, validate member
    paths, and require the expected clean commit/run/version/target/channel.
    release_tag is checked only when supplied. This reads the archive without
    extraction and raises ValueError on mismatches; check-release-source.py
    separately verifies the GitHub run before these artifacts are downloaded.
    """
    app_name(channel)
    if not target.endswith("apple-darwin") or target not in targets_for_version(version):
        raise ValueError("Mac target is not supported by this release version")
    expected_name = f"rexafs-{version}-{target}.zip"
    if archive.name != expected_name:
        raise ValueError("Unexpected desktop archive name")
    entries = dict(line.split("  ", 1)[::-1] for line in manifest.read_text().splitlines())
    if entries.get(archive.name) != digest(archive):
        raise ValueError("Desktop archive differs from the qualified build")
    stem = archive.stem
    with ZipFile(archive) as zipped:
        for name in zipped.namelist():
            path = PurePosixPath(name)
            if path.is_absolute() or ".." in path.parts or path.parts[0] not in {stem, "__MACOSX"}:
                raise ValueError("Unsafe or unexpected archive path")
        metadata = json.loads(zipped.read(f"{stem}/build.json"))
    expected = {"version": version, "commit": commit, "github_run_id": run_id,
                "target": target, "dirty": False, "signed": False, "notarized": False}
    if any(metadata.get(key) != value for key, value in expected.items()):
        raise ValueError("Desktop build metadata does not match the source run")
    if metadata.get("channel", "stable") != channel:
        raise ValueError("Desktop build channel does not match")
    if release_tag is not None and metadata.get("release_tag") != release_tag:
        raise ValueError("Desktop build tag does not match")
    return metadata


def private_run(command):
    """Run a credential-bearing command with captured output and redacted errors.

    Return stdout on success. On failure, raise RuntimeError containing only
    the executable and exit status; callers must not log the returned output
    when it contains credentials.
    """
    # Credential-bearing command lines must never appear in exceptions or logs.
    result = subprocess.run(command, capture_output=True, text=True)
    if result.returncode:
        raise RuntimeError(f"{command[0]} credential setup failed (exit {result.returncode})")
    return result.stdout


@contextmanager
def signing_keychain(directory):
    """Yield a temporary keychain containing the configured Developer ID identity.

    Read the six macos-signing settings named below, import the certificate and
    store a notarytool profile. During use the keychain joins the user's search
    list. The finally block attempts to restore that list and removes the
    temporary keychain/certificate. This mutates signing state on a macOS runner
    and is intended for the signing workflow, not ordinary documentation builds.
    """
    required = ["MACOS_CERTIFICATE_P12_BASE64", "MACOS_CERTIFICATE_PASSWORD",
                "APPLE_ID", "APPLE_APP_SPECIFIC_PASSWORD", "APPLE_TEAM_ID", "MACOS_SIGNING_IDENTITY"]
    missing = [name for name in required if not os.environ.get(name)]
    if missing:
        raise ValueError("Missing macos-signing configuration: " + ", ".join(missing))
    keychain = directory / "signing.keychain-db"
    p12 = directory / "identity.p12"
    p12.write_bytes(base64.b64decode(os.environ[required[0]], validate=True))
    p12.chmod(0o600)
    password = secrets.token_urlsafe(48)
    original = shlex.split(private_run(["security", "list-keychains", "-d", "user"]))
    try:
        private_run(["security", "create-keychain", "-p", password, str(keychain)])
        private_run(["security", "set-keychain-settings", "-lut", "7200", str(keychain)])
        private_run(["security", "unlock-keychain", "-p", password, str(keychain)])
        private_run(["security", "import", str(p12), "-k", str(keychain), "-f", "pkcs12",
                     "-P", os.environ["MACOS_CERTIFICATE_PASSWORD"], "-T", "/usr/bin/codesign"])
        private_run(["security", "set-key-partition-list", "-S", "apple-tool:,apple:,codesign:",
                     "-s", "-k", password, str(keychain)])
        private_run(["security", "list-keychains", "-d", "user", "-s", *original, str(keychain)])
        identities = private_run(["security", "find-identity", "-v", "-p", "codesigning", str(keychain)])
        identity = os.environ["MACOS_SIGNING_IDENTITY"]
        if not identity.startswith("Developer ID Application: ") or f'"{identity}"' not in identities:
            raise ValueError("The certificate does not provide the requested Developer ID identity")
        private_run(["xcrun", "notarytool", "store-credentials", "rexafs-release", "--keychain", str(keychain),
                     "--apple-id", os.environ["APPLE_ID"], "--team-id", os.environ["APPLE_TEAM_ID"],
                     "--password", os.environ["APPLE_APP_SPECIFIC_PASSWORD"]])
        yield keychain
    finally:
        subprocess.run(["security", "list-keychains", "-d", "user", "-s", *original], capture_output=True)
        if keychain.exists():
            subprocess.run(["security", "delete-keychain", str(keychain)], capture_output=True)
        p12.unlink(missing_ok=True)


def run(command):
    """Run a command, retrying Apple's stapler after transient service failures.

    stapler fetches notarization tickets from Apple's CloudKit service, which
    sometimes drops the connection (exit 68) shortly after notarization. Retry
    stapler up to five times with growing waits; other commands run once.
    """
    command = [str(value) for value in command]
    attempts = 5 if command[:2] == ["xcrun", "stapler"] else 1
    for attempt in range(1, attempts + 1):
        try:
            subprocess.run(command, check=True)
            return
        except subprocess.CalledProcessError:
            if attempt == attempts:
                raise
            print(f"stapler failed (attempt {attempt}/{attempts}); retrying", flush=True)
            time.sleep(30 * attempt)


def verify_app(app, team):
    """Require a timestamped Developer ID app signature and accepted notarization.

    Run codesign, stapler and Gatekeeper checks for the expected Apple team.
    This checks the signed app bundle; it does not launch its graphical UI.
    Tool failures propagate and mismatched signature details raise ValueError.
    """
    run(["codesign", "--verify", "--deep", "--strict", "--verbose=2", app])
    details = subprocess.run(["codesign", "-d", "--verbose=4", str(app)], check=True,
                             capture_output=True, text=True).stderr
    if (f"TeamIdentifier={team}\n" not in details or "Authority=Developer ID Application:" not in details
            or "(runtime)" not in details or "Timestamp=" not in details):
        raise ValueError("Expected timestamped Developer ID signature with Hardened Runtime")
    run(["xcrun", "stapler", "validate", app])
    run(["spctl", "--assess", "--type", "execute", "--verbose=2", app])


def notarize(submission, keychain, directory, label):
    """Submit to Apple and return the accepted notarization submission ID.

    Use the keychain's rexafs-release profile, wait up to 45 minutes, and save
    the service log under directory using label. An unaccepted result raises
    ValueError; this step does not staple the ticket or verify Gatekeeper.
    """
    auth = ["--keychain-profile", "rexafs-release", "--keychain", str(keychain)]
    result = json.loads(subprocess.check_output([
        "xcrun", "notarytool", "submit", str(submission), *auth, "--wait", "--timeout", "45m",
        "--output-format", "json"], text=True))
    log = directory / f"{label}-notarization-log.json"
    run(["xcrun", "notarytool", "log", result["id"], *auth, log])
    print(log.read_text())
    if result.get("status") != "Accepted":
        raise ValueError("Apple did not accept the notarization submission")
    return result["id"]


def sign_installer(bundle, archive, output, metadata, keychain, directory):
    """Create, sign, notarize, staple and qualify the bundle's DMG installer.

    Write the image, SHA-256 sidecar and JSON qualification evidence into output.
    Verify a copied installation and record its executable hash alongside the
    signed ZIP hash. Existing installer output is rejected by build_installer.
    Requires Apple's tools and submits the image to Apple's notary service.
    """
    image = output / installer_name(metadata)
    build_installer(bundle, image, metadata)
    run(["codesign", "--sign", metadata["signing_identity"], "--keychain", keychain, "--timestamp", image])
    notarization_id = notarize(image, keychain, directory, "installer")
    run(["xcrun", "stapler", "staple", image])
    run(["codesign", "--verify", "--strict", "--verbose=2", image])
    details = subprocess.run(["codesign", "-d", "--verbose=4", str(image)], check=True,
                             capture_output=True, text=True).stderr
    if (f"TeamIdentifier={metadata['apple_team_id']}\n" not in details
            or "Authority=Developer ID Application:" not in details or "Timestamp=" not in details):
        raise ValueError("Installer is missing its timestamped Developer ID signature")
    run(["xcrun", "stapler", "validate", image])
    run(["spctl", "--assess", "--type", "open", "--context", "context:primary-signature", "--verbose=2", image])
    executable_sha256 = verify_installation(image, metadata, verify_app)
    evidence = dict(schema_version=1, build=metadata, signed=True, notarized=True, stapled=True,
                    gatekeeper_accepted=True, installation_verified=True,
                    apple_team_id=metadata["apple_team_id"], notarization_id=notarization_id,
                    archive_sha256=digest(archive), sha256=digest(image), executable_sha256=executable_sha256)
    Path(str(image) + ".json").write_text(json.dumps(evidence, indent=2) + "\n")
    Path(str(image) + ".sha256").write_text(f"{digest(image)}  {image.name}\n")


def sign_archive(archive, output, metadata, keychain, directory, dmg=False):
    """Sign the already-qualified native app and write a fresh release archive.

    Use check_source's metadata; update that dictionary in place with signing
    provenance. Repackage and re-extract the signed/stapled app, then verify its
    signature and calculation self-checks before writing the checksum. With
    dmg=True also retain in-app notices and produce a qualified DMG. Executables
    are signed but not rebuilt. Existing ZIP/checksum paths may be overwritten;
    callers should supply a fresh workflow output directory.
    """
    unpacked = directory / "unpacked"
    run(["ditto", "-x", "-k", archive, unpacked])
    bundle = unpacked / archive.stem
    application_name = app_name(metadata.get("channel", "stable"))
    app = bundle / application_name
    if dmg:
        include_notices(bundle, metadata)
    identity, team = os.environ["MACOS_SIGNING_IDENTITY"], os.environ["APPLE_TEAM_ID"]
    run(["codesign", "--force", "--sign", identity, "--keychain", keychain,
         "--options", "runtime", "--timestamp", app])
    submission = directory / "notarization.zip"
    run(["ditto", "-c", "-k", "--keepParent", app, submission])
    notarization_id = notarize(submission, keychain, directory, "app")
    run(["xcrun", "stapler", "staple", app])
    verify_app(app, team)
    metadata.update(signed=True, notarized=True, signing_identity=identity, apple_team_id=team,
                    notarization_id=notarization_id, unsigned_archive_sha256=digest(archive),
                    signing_run_id=os.environ["GITHUB_RUN_ID"], signing_tools_commit=os.environ["GITHUB_SHA"])
    (bundle / "build.json").write_text(json.dumps(metadata, indent=2) + "\n")
    readme = bundle / "README.txt"
    readme.write_text(readme.read_text().replace(
        "This archive has no publisher code signature; macOS notarization is not included.",
        "The macOS application is signed with Developer ID and notarized by Apple."))
    output.mkdir(parents=True, exist_ok=True)
    final = output / archive.name
    # Staple the .app before producing the final ZIP and its checksum.
    run(["ditto", "-c", "-k", "--sequesterRsrc", "--keepParent", bundle, final])
    fresh = directory / "verified"
    run(["ditto", "-x", "-k", final, fresh])
    extracted = fresh / archive.stem / application_name
    verify_app(extracted, team)
    options = ["--version", "--self-check", "--self-check-updater"]
    if "feff10-runner" in metadata.get("features", []):
        options.append("--self-check-feff")
    for option in options:
        subprocess.run([str(extracted / "Contents/MacOS/rexafs"), option], cwd=fresh, check=True, timeout=600)
    Path(str(final) + ".sha256").write_text(f"{digest(final)}  {final.name}\n")
    if dmg:
        sign_installer(bundle, final, output, metadata, keychain, directory)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["archive", "manifest", "output"]:
        parser.add_argument(name, type=Path)
    for name in ["version", "commit", "build-run-id", "target"]:
        parser.add_argument("--" + name, required=True)
    parser.add_argument("--channel", choices=["stable", "nightly"], default="stable")
    parser.add_argument("--release-tag")
    parser.add_argument("--dmg", action="store_true", help="Also build, sign, notarize and verify a DMG installer")
    args = parser.parse_args()
    if platform.system() != "Darwin":
        raise SystemExit("Signing requires macOS")
    metadata = check_source(args.archive, args.manifest, args.version, args.commit, args.build_run_id, args.target, args.channel, args.release_tag)
    with tempfile.TemporaryDirectory(prefix="rexafs-signing-") as temporary:
        directory = Path(temporary)
        with signing_keychain(directory) as keychain:
            sign_archive(args.archive.resolve(), args.output.resolve(), metadata, keychain, directory, args.dmg)
