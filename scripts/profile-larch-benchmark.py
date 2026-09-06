#!/usr/bin/env python3
"""Collect Python call profiles and macOS native samples of the same workload."""

import argparse
import json
import queue
import subprocess
import sys
import threading
from contextlib import contextmanager
from pathlib import Path


def stop_worker(process, timeout=10):
    """Reap the worker even if it ignores termination."""
    if process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=timeout)


@contextmanager
def ready_worker(command, stderr, startup_timeout=60, shutdown_timeout=10):
    """Bound the readiness handshake, including a missing newline or early EOF."""
    process = subprocess.Popen(
        command, text=True, stdout=subprocess.PIPE, stderr=stderr
    )
    messages = queue.Queue(maxsize=1)

    def read_ready():
        try:
            messages.put((process.stdout.readline(), None))
        except (OSError, ValueError) as error:
            messages.put((None, error))

    reader = threading.Thread(target=read_ready, daemon=True)
    reader.start()
    try:
        try:
            line, read_error = messages.get(timeout=startup_timeout)
        except queue.Empty as error:
            raise RuntimeError(
                "Profile worker readiness timed out; see stderr"
            ) from error
        if read_error is not None:
            raise RuntimeError("Cannot read profile worker readiness") from read_error
        if not line:
            raise RuntimeError("Profile worker exited before readiness; see stderr")
        try:
            ready = json.loads(line)
        except json.JSONDecodeError as error:
            raise RuntimeError("Profile worker sent invalid readiness JSON") from error
        if not isinstance(ready, dict) or ready.get("ready_pid") != process.pid:
            raise RuntimeError(
                "Profile worker readiness PID does not match its process"
            )
        yield process
    finally:
        stop_worker(process, timeout=shutdown_timeout)
        reader.join(timeout=shutdown_timeout)
        process.stdout.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--dataset", default="cu")
    parser.add_argument("--setup", default="standard")
    parser.add_argument(
        "--configs",
        nargs="+",
        default=["larch", "shipped_default", "direct_uncached", "legacy_lm", "dogleg"],
    )
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    script = Path(__file__).with_name("benchmark-larch.py")
    for config in args.configs:
        output = args.output / f"{args.dataset}--{args.setup}--{config}.json"
        command = [
            sys.executable,
            str(script),
            "--worker",
            "--dataset",
            args.dataset,
            "--setup",
            args.setup,
            "--config",
            config,
            "--profile-seconds",
            "15",
            "--output",
            str(output),
        ]
        print(f"Profiling {config}", flush=True)
        with (
            output.with_suffix(".stderr.txt").open("w") as stderr,
            ready_worker(command, stderr) as process,
        ):
            sample = output.with_suffix(".native-sample.txt")
            if sys.platform == "darwin":
                completed = subprocess.run(
                    [
                        "/usr/bin/sample",
                        str(process.pid),
                        "8",
                        "1",
                        "-file",
                        str(sample),
                    ],
                    capture_output=True,
                    text=True,
                    timeout=30,
                    check=False,
                )
                status = {
                    "command": [
                        "sample",
                        "WORKER_PID",
                        "8",
                        "1",
                        "-file",
                        sample.name,
                    ],
                    "returncode": completed.returncode,
                    "stderr": completed.stderr,
                }
            else:
                status = {
                    "returncode": None,
                    "reason": "Native sample capture currently requires macOS",
                }
            output.with_suffix(".native-status.json").write_text(
                json.dumps(status, indent=2) + "\n"
            )
            code = process.wait(timeout=45)
            if code:
                raise RuntimeError(f"Profile worker {config} failed: {code}")


if __name__ == "__main__":
    main()
