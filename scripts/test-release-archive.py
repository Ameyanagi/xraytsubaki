"""Regression for Windows packaging of epoch-dated dependency notices."""
import os
from pathlib import Path
import tempfile
import unittest
from zipfile import ZipFile

from release_archive import zip_bundle


class ZipBundleTests(unittest.TestCase):
    def test_old_notice_dates_preserve_bytes_and_bundle_layout(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            bundle = root / "rexafs-test-windows"
            notices = bundle / "licenses" / "example-1.0"
            notices.mkdir(parents=True)
            empty = bundle / "resources" / "empty"
            empty.mkdir(parents=True)
            original = b"License fixture\r\n\x00original bytes\n"
            notice = notices / "LICENSE"
            notice.write_bytes(original)
            # This reproduces the timestamp that broke the GitHub Windows ZIP.
            os.utime(notice, (86400, 86400))
            os.utime(empty, (86400, 86400))
            before = notice.stat().st_mtime_ns
            archive = root / "rexafs-test-windows.zip"
            zip_bundle(bundle, archive)
            with ZipFile(archive) as result:
                self.assertIsNone(result.testzip())
                name = "rexafs-test-windows/licenses/example-1.0/LICENSE"
                self.assertEqual(result.read(name), original)
                self.assertEqual(result.getinfo(name).date_time, (1980, 1, 1, 0, 0, 0))
                self.assertIn("rexafs-test-windows/resources/empty/", result.namelist())
                self.assertTrue(all(n.startswith("rexafs-test-windows/") for n in result.namelist()))
                result.extractall(root / "unpacked")
            self.assertEqual((root / "unpacked" / name).read_bytes(), original)
            self.assertEqual(notice.stat().st_mtime_ns, before)


if __name__ == "__main__":
    unittest.main()
