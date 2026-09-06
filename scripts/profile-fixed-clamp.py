#!/usr/bin/env python3
"""Profile the actual fixed-lambda and Larch full pipelines on the same Cu scan."""

import argparse
import gzip
import importlib.util
import json
import subprocess
import sys
from pathlib import Path


def module(name, filename):
    spec = importlib.util.spec_from_file_location(
        name, Path(__file__).with_name(filename)
    )
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--worker", choices=["larch", "fixed_cached", "fixed_uncached"])
    args = parser.parse_args()
    if args.worker:
        bench = module("benchmark", "benchmark-larch.py")
        bench.CONFIGS.update(
            fixed_cached=("LinearDirect", True, "FixedPenalty", False),
            fixed_uncached=("LinearDirect", False, "FixedPenalty", False),
        )
        bench.worker(
            argparse.Namespace(
                dataset="cu",
                setup="standard",
                config=args.worker,
                profile_seconds=15,
                rounds=7,
                output=args.output,
            )
        )
        return
    args.output.mkdir(parents=True, exist_ok=True)
    profiler = module("profile", "profile-larch-benchmark.py")
    for config in ("larch", "fixed_cached", "fixed_uncached"):
        output = args.output / f"{config}.json"
        command = [
            sys.executable,
            __file__,
            "--worker",
            config,
            "--output",
            str(output),
        ]
        with (
            output.with_suffix(".stderr.txt").open("w") as stderr,
            profiler.ready_worker(command, stderr, startup_timeout=120) as worker,
        ):
            sample = output.with_suffix(".native.txt")
            result = subprocess.run(
                ["/usr/bin/sample", str(worker.pid), "8", "1", "-file", str(sample)],
                capture_output=True,
                text=True,
                timeout=30,
                check=False,
            )
            status = {
                "returncode": result.returncode,
                "stderr": result.stderr,
                "scope": "full pipeline on fresh spectra; 15 seconds, native sample 8 seconds",
            }
            output.with_suffix(".status.json").write_text(
                json.dumps(status, indent=2) + "\n"
            )
            if worker.wait(timeout=45) or result.returncode:
                raise RuntimeError(f"Native profile failed: {config}")
            sample.with_suffix(".txt.gz").write_bytes(
                gzip.compress(sample.read_bytes(), mtime=0)
            )
            sample.unlink()
        print(f"Profiled {config}", flush=True)


if __name__ == "__main__":
    main()
