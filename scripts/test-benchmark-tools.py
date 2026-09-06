#!/usr/bin/env python3
"""Exercise profiling worker failures with real, bounded subprocesses."""

import importlib.util
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

spec = importlib.util.spec_from_file_location(
    "profile_benchmark", Path(__file__).with_name("profile-larch-benchmark.py")
)
if spec is None or spec.loader is None:
    raise RuntimeError("Cannot load profile-larch-benchmark.py")
profile = importlib.util.module_from_spec(spec)
spec.loader.exec_module(profile)

READY = "import os,json; print(json.dumps({'ready_pid':os.getpid()}), flush=True); "


class WorkerTests(unittest.TestCase):
    def worker(self, code, **kwargs):
        return profile.ready_worker(
            [sys.executable, "-u", "-c", code], subprocess.DEVNULL, **kwargs
        )

    def test_valid_handshake(self):
        with self.worker(READY) as process:
            self.assertEqual(process.wait(timeout=5), 0)

    def test_early_exit(self):
        with (
            self.assertRaisesRegex(RuntimeError, "exited before readiness"),
            self.worker("raise SystemExit(3)"),
        ):
            self.fail("An exited worker must not be sampled")

    def test_invalid_handshake(self):
        for payload, message in (
            ("not json", "invalid readiness JSON"),
            ('{"ready_pid": -1}', "PID does not match"),
            ("[]", "PID does not match"),
        ):
            with (
                self.subTest(payload=payload),
                self.assertRaisesRegex(RuntimeError, message),
                self.worker(f"print({payload!r})"),
            ):
                self.fail("An invalid worker must not be sampled")

    def test_exception_reaps_worker(self):
        with (
            self.assertRaisesRegex(ValueError, "sampler failed"),
            self.worker(READY + "import time; time.sleep(60)") as process,
        ):
            raise ValueError("sampler failed")
        self.assertIsNotNone(process.poll())

    @unittest.skipIf(os.name == "nt", "POSIX signal and PID probe")
    def test_partial_line_times_out_and_reaps_worker(self):
        with tempfile.TemporaryDirectory() as directory:
            pid_file = Path(directory) / "pid"
            code = (
                "import os,time,pathlib; "
                f"pathlib.Path({str(pid_file)!r}).write_text(str(os.getpid())); "
                "print('{', end='', flush=True); time.sleep(60)"
            )
            with (
                self.assertRaisesRegex(RuntimeError, "readiness timed out"),
                self.worker(code, startup_timeout=2),
            ):
                self.fail("A partial handshake must not be accepted")
            pid = int(pid_file.read_text())
            with self.assertRaises(ProcessLookupError):
                os.kill(pid, 0)

    @unittest.skipIf(os.name == "nt", "POSIX SIGTERM handling")
    def test_ignores_terminate(self):
        code = (
            "import signal; signal.signal(signal.SIGTERM, signal.SIG_IGN); "
            + READY
            + "import time; time.sleep(60)"
        )
        with self.worker(code, shutdown_timeout=0.5) as process:
            self.assertIsNone(process.poll())
        self.assertEqual(process.returncode, -9)


if __name__ == "__main__":
    unittest.main()
