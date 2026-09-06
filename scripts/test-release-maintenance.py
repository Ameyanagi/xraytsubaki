"""Regression checks for source identity and post-build release corrections."""
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest.mock import patch
from zipfile import ZipFile


def module(name):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(name + ".py"))
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


source = module("check-release-source")
macos = module("sign-macos-release")
sdist = module("repair-python-sdist")
licenses = module("check-python-sdist")


class ReleaseMaintenanceTests(unittest.TestCase):
    def test_pull_request_or_other_commit_cannot_be_promoted(self):
        run = dict(conclusion="success", head_sha="abc", head_branch="v0.1.0",
                   event="workflow_dispatch", path=".github/workflows/release-build.yml")
        source.validate(run, "v0.1.0", "abc", "0.1.0")
        for key, value in [("head_sha", "other"), ("head_branch", "main"), ("event", "pull_request"),
                           ("conclusion", "failure"), ("path", ".github/workflows/other.yml")]:
            with self.subTest(key=key), self.assertRaises(ValueError):
                source.validate({**run, key: value}, "v0.1.0", "abc", "0.1.0")

    def test_signing_rejects_tampered_archive_and_wrong_build_metadata(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            archive = root / "rexafs-0.1.0-aarch64-apple-darwin.zip"
            with ZipFile(archive, "w") as zipped:
                zipped.writestr(archive.stem + "/build.json", json.dumps(dict(
                    version="0.1.0", target="aarch64-apple-darwin", commit="abc",
                    github_run_id="123", dirty=False, signed=False, notarized=False)))
            manifest = root / "SHA256SUMS"
            manifest.write_text(f"{macos.digest(archive)}  {archive.name}\n")
            args = [archive, manifest, "0.1.0", "abc", "123", "aarch64-apple-darwin"]
            macos.check_source(*args)
            with self.assertRaises(ValueError):
                macos.check_source(*args[:3], "other", *args[4:])
            archive.write_bytes(archive.read_bytes() + b"tampered")
            with self.assertRaises(ValueError):
                macos.check_source(*args)

    def test_license_repair_preserves_all_existing_source_and_metadata(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            archive = root / "rexafs-0.1.0.tar.gz"
            files = {"PKG-INFO": b"Name: rexafs\nVersion: 0.1.0\nLicense-File: LICENSE-MIT\n\n",
                     "src/lib.rs": b"// original Rust source\n"}
            with tarfile.open(archive, "w:gz") as tar:
                for name, data in files.items():
                    member = tarfile.TarInfo("rexafs-0.1.0/" + name)
                    member.size = len(data)
                    tar.addfile(member, io.BytesIO(data))
            manifest = root / "SHA256SUMS"
            manifest.write_text(f"{hashlib.sha256(archive.read_bytes()).hexdigest()}  {archive.name}\n")
            with self.assertRaises(KeyError):
                licenses.check(archive)
            with patch.object(sdist.subprocess, "check_output", return_value=b"Original license\n"):
                final = sdist.repair(archive, manifest, root / "out", "abc")
            licenses.check(final)
            with tarfile.open(final) as tar:
                for name, data in files.items():
                    self.assertEqual(tar.extractfile("rexafs-0.1.0/" + name).read(), data)
                self.assertEqual(tar.extractfile("rexafs-0.1.0/LICENSE-MIT").read(), b"Original license\n")
            self.assertTrue(Path(str(final) + ".sha256").is_file())


if __name__ == "__main__":
    unittest.main()
