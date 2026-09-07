"""Installer regressions: installed notices, channel identity and publication evidence."""
import hashlib
import json
from pathlib import Path
import plistlib
import tempfile
import unittest
from zipfile import ZipFile

import macos_installer as installer
from desktop_channels import app_name


class MacInstallerTests(unittest.TestCase):
    def test_app_only_install_retains_notices_and_channel_identity(self):
        for channel in ("stable", "nightly"):
            with self.subTest(channel=channel), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                metadata = dict(channel=channel, version="0.1.2", target="aarch64-apple-darwin")
                app = root / app_name(channel)
                (app / "Contents").mkdir(parents=True)
                (app / "Contents/Info.plist").write_bytes(plistlib.dumps(dict(
                    CFBundleIdentifier="com.rexafs.nightly" if channel == "nightly" else "com.rexafs.desktop",
                    CFBundleShortVersionString="0.1.2", CFBundleExecutable="rexafs")))
                for name in installer.NOTICES:
                    if name == "licenses":
                        (root / name).mkdir()
                        (root / name / "third-party.txt").write_text("Third-party notice")
                    else:
                        (root / name).write_text("Original " + name)
                installer.include_notices(root, metadata)
                (root / "Applications").symlink_to("/Applications", target_is_directory=True)
                self.assertEqual(installer.check_payload(root, metadata), app)
                self.assertEqual((app / "Contents/Resources/notices/licenses/third-party.txt").read_text(), "Third-party notice")
                for name in installer.NOTICES[:-1]:
                    self.assertEqual((app / "Contents/Resources/notices" / name).read_bytes(), (root / name).read_bytes())
                with self.assertRaises(ValueError):
                    installer.check_payload(root, {**metadata, "channel": "nightly" if channel == "stable" else "stable"})
                with self.assertRaises(ValueError):
                    installer.check_payload(root, {**metadata, "version": "0.1.3"})
                (root / "Applications").unlink()
                (root / "Applications").symlink_to("/tmp")
                with self.assertRaises(ValueError):
                    installer.check_payload(root, metadata)
                (root / "Applications").unlink()
                (root / "Applications").symlink_to("/Applications")
                (app / "Contents/Resources/notices/LICENSE-MIT").unlink()
                with self.assertRaises(ValueError):
                    installer.check_payload(root, metadata)

    def test_installer_requires_exact_archive_binary_digest_and_signing_evidence(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            metadata = dict(channel="stable", version="0.1.2", target="aarch64-apple-darwin",
                            signed=True, notarized=True, apple_team_id="TEST", commit="source")
            image = root / installer.installer_name(metadata)
            image.write_bytes(b"test-only image")
            archive = image.with_suffix(".zip")
            with ZipFile(archive, "w") as zipped:
                zipped.writestr(archive.stem + "/rexafs.app/Contents/MacOS/rexafs", b"test-only binary")
            evidence = dict(schema_version=1, build=metadata, signed=True, notarized=True, stapled=True,
                            gatekeeper_accepted=True, installation_verified=True, apple_team_id="TEST",
                            notarization_id="test-only", archive_sha256=installer.sha256(archive),
                            sha256=installer.sha256(image), executable_sha256=hashlib.sha256(b"test-only binary").hexdigest())
            evidence_file = Path(str(image) + ".json")
            sidecar = Path(str(image) + ".sha256")
            sidecar.write_text(f"{installer.sha256(image)}  {image.name}\n")
            for invalid in [None, dict(build={**metadata, "commit": "other"}), dict(build={**metadata, "channel": "nightly"}),
                            dict(build={**metadata, "target": "x86_64-apple-darwin"}), dict(signed=False),
                            dict(notarized=False), dict(stapled=False), dict(gatekeeper_accepted=False),
                            dict(installation_verified=False), dict(notarization_id=""), dict(apple_team_id="OTHER"),
                            dict(archive_sha256="wrong"), dict(sha256="wrong"), dict(executable_sha256="0" * 64)]:
                evidence_file.write_text(json.dumps({**evidence, **(invalid or {})}))
                with self.subTest(invalid=invalid):
                    if invalid is None:
                        self.assertEqual(len(installer.qualify_installer(image, metadata, archive)), 3)
                    else:
                        with self.assertRaises(ValueError):
                            installer.qualify_installer(image, metadata, archive)
            evidence_file.write_text(json.dumps(evidence))
            sidecar.write_text("wrong checksum")
            with self.assertRaises(ValueError):
                installer.qualify_installer(image, metadata, archive)
            sidecar.write_text(f"{installer.sha256(image)}  {image.name}\n")
            image.write_bytes(b"modified image")
            with self.assertRaises(ValueError):
                installer.qualify_installer(image, metadata, archive)
            image.unlink()
            with self.assertRaises(FileNotFoundError):
                installer.qualify_installer(image, metadata, archive)


if __name__ == "__main__":
    unittest.main()
