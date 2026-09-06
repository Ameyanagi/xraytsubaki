"""Channel isolation, source/signing identity and nightly publication regressions."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from zipfile import ZipFile
from desktop_channels import app_name, identity


def module(name):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(name + ".py"))
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


nightly = module("nightly-release")
macos = module("sign-macos-release")
TAG = "nightly-20260907-123"


class DesktopChannels(unittest.TestCase):
    def test_reruns_keep_the_original_date_and_source_identity(self):
        run = dict(id=123, head_sha="abc", head_branch="main", event="workflow_dispatch", created_at="2026-09-07T23:59:59Z")
        expected = dict(version="0.1.2", commit="abc", tag=TAG, built_at="2026-09-07T23:59:59Z")
        self.assertEqual(nightly.build_plan(run, "0.1.2", "abc", "123"), expected)
        with self.assertRaises(ValueError):
            nightly.build_plan({**run, "head_sha": "other"}, "0.1.2", "abc", "123")

    def test_build_identity_and_app_names_keep_channels_separate(self):
        self.assertEqual(identity("0.1.2", {})["release_tag"], "v0.1.2")
        self.assertEqual(app_name("stable"), "rexafs.app")
        self.assertEqual(app_name("nightly"), "rexafs Nightly.app")
        env = dict(REXAFS_BUILD_CHANNEL="nightly", REXAFS_BUILD_TAG=TAG, REXAFS_BUILD_UTC="2026-09-07T00:00:00Z")
        self.assertEqual(identity("0.1.2", env)["channel"], "nightly")
        for invalid in [dict(REXAFS_BUILD_CHANNEL="unexpected"), dict(REXAFS_BUILD_CHANNEL="nightly"),
                        {**env, "REXAFS_BUILD_TAG": "v0.1.2"}, {**env, "REXAFS_BUILD_CHANNEL": "stable"}]:
            with self.subTest(invalid=invalid), self.assertRaises(ValueError):
                identity("0.1.2", invalid)

    def test_only_main_schedule_or_manual_run_can_publish(self):
        env = dict(GITHUB_REF="refs/heads/main", GITHUB_REPOSITORY="Ameyanagi/rexafs", GITHUB_EVENT_NAME="schedule")
        nightly.require_main(env)
        for key, value in [("GITHUB_REF", "refs/pull/12/merge"), ("GITHUB_REPOSITORY", "someone/rexafs"),
                           ("GITHUB_EVENT_NAME", "pull_request"), ("GITHUB_REF", "refs/tags/v0.1.2")]:
            with self.subTest(key=key), self.assertRaises(ValueError):
                nightly.require_main({**env, key: value})

    def archive(self, root, target, overrides=None, signed=True):
        file = root / f"rexafs-0.1.2-{target}.zip"
        metadata = dict(version="0.1.2", commit="abc", target=target, github_run_id="123", signing_run_id="123",
                        signing_tools_commit="abc", channel="nightly", release_tag=TAG, dirty=False,
                        signed=signed, notarized=signed, notarization_id="test-only", apple_team_id="TEST")
        metadata.update(overrides or {})
        with ZipFile(file, "w") as zip:
            zip.writestr(file.stem + "/build.json", json.dumps(metadata))
            zip.writestr(file.stem + "/rexafs Nightly.app/Contents/MacOS/rexafs", b"test-only")
        digest = nightly.sha256(file)
        Path(str(file) + ".sha256").write_text(f"{digest}  {file.name}\n")
        return file

    def test_publication_requires_both_signed_architectures_and_matching_run(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            files = [self.archive(root, target) for target in sorted(nightly.TARGETS)]
            self.assertEqual(len(nightly.qualify(files, "0.1.2", "abc", "123", TAG)), 4)
            with self.assertRaises(ValueError):
                nightly.qualify(files[:1], "0.1.2", "abc", "123", TAG)
            for overrides in [dict(signed=False), dict(notarized=False), dict(dirty=True), dict(commit="wrong"),
                              dict(signing_tools_commit="wrong"), dict(release_tag="nightly-20260906-123"),
                              dict(channel="stable"), dict(github_run_id="122"), dict(notarization_id="")]:
                files[0] = self.archive(root, sorted(nightly.TARGETS)[0], overrides)
                with self.subTest(overrides=overrides), self.assertRaises(ValueError):
                    nightly.qualify(files, "0.1.2", "abc", "123", TAG)

    def test_signing_does_not_accept_nightly_as_stable_or_another_tag(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            target = "aarch64-apple-darwin"
            file = self.archive(root, target, signed=False)
            args = [file, Path(str(file) + ".sha256"), "0.1.2", "abc", "123", target]
            macos.check_source(*args, channel="nightly", release_tag=TAG)
            with self.assertRaises(ValueError):
                macos.check_source(*args)
            with self.assertRaises(ValueError):
                macos.check_source(*args, channel="nightly", release_tag="nightly-20260906-123")
            file.write_bytes(file.read_bytes() + b"tampered")
            with self.assertRaises(ValueError):
                macos.check_source(*args, channel="nightly", release_tag=TAG)


if __name__ == "__main__":
    unittest.main()
