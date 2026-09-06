#!/usr/bin/env python3
"""Collect Python call profiles and macOS native samples of the same workload."""

import argparse
import json
import subprocess
import sys
from pathlib import Path


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
        with output.with_suffix(".stderr.txt").open("w") as stderr:
            process = subprocess.Popen(
                command, text=True, stdout=subprocess.PIPE, stderr=stderr
            )
            try:
                line = process.stdout.readline()
                ready = json.loads(line)
                assert ready["ready_pid"] == process.pid
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
            finally:
                if process.poll() is None:
                    process.terminate()
                    process.wait(timeout=10)


if __name__ == "__main__":
    main()
