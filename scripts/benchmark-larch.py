#!/usr/bin/env python3
"""Matched XAFS pipeline benchmarks; run in an isolated, pinned environment.

Every timed spectrum is fresh. Repeated Spectrum.fft() calls would measure
rexafs' result cache, not its processing speed. Input loading, output copying,
profiling and plotting are deliberately outside the stage timing intervals.
"""

from __future__ import annotations

import argparse
import cProfile
import hashlib
import importlib.metadata
import json
import os
import platform
import pstats
import random
import resource
import subprocess
import sys
import time
from pathlib import Path

# Set these before importing either numerical stack. Record actual loaded
# libraries with threadpoolctl as well; environment variables alone are not proof.
for _key in (
    "OPENBLAS_NUM_THREADS",
    "OMP_NUM_THREADS",
    "MKL_NUM_THREADS",
    "VECLIB_MAXIMUM_THREADS",
    "NUMEXPR_NUM_THREADS",
    "RAYON_NUM_THREADS",
):
    os.environ[_key] = "1"

import larch
import numpy as np
import rexafs
from larch.xafs import autobk, find_e0, pre_edge, xftf
from threadpoolctl import threadpool_info, threadpool_limits

ROOT = Path(__file__).resolve().parents[1]
DATA = ROOT / "crates/rexafs/tests/testfiles"
DATASETS = {
    "cu": ("xraylarch_d867/xafsdata/cu_150k.xmu", 0),
    "ni": ("xraylarch_d867/xafsdata/ni_metal_rt.xdi", 0),
    "ru": ("Ru_QAS.dat", 0),
    "cu_dense_8192": ("xraylarch_d867/xafsdata/cu_150k.xmu", 8192),
    "cu_dense_32768": ("xraylarch_d867/xafsdata/cu_150k.xmu", 32768),
}
CONFIGS = {
    "larch": None,
    "shipped_default": ("LinearDirect", True, "Fixed", True),
    "direct_cached": ("LinearDirect", True, "Fixed", False),
    "direct_uncached": ("LinearDirect", False, "Fixed", False),
    "direct_two_pass": ("LinearDirect", True, "TwoPass", False),
    "legacy_lm": ("LegacyLm", True, "Fixed", False),
    "dogleg": ("TrustRegionDogLeg", True, "Fixed", False),
}
SETUPS = {
    "standard": {"nfft": 2048, "kstep": 0.05, "rbkg": 1.0},
    "fine_fft": {"nfft": 8192, "kstep": 0.025, "rbkg": 1.0},
    # Ablations test numerical differences without silently changing defaults.
    "no_clamp": {"nfft": 2048, "kstep": 0.05, "rbkg": 1.0, "nclamp": 0},
    "aligned_rounding": {
        "nfft": 2048,
        "kstep": 0.05,
        "rbkg": 1.0,
        "kmax": 12.8,
        "nclamp": 0,
    },
}


def load_input(name):
    filename, dense = DATASETS[name]
    path = DATA / filename
    raw = np.loadtxt(path)
    energy = raw[:, 0].copy()
    mu = np.log(raw[:, 1] / raw[:, 2]) if name == "ru" else raw[:, 1].copy()
    if dense:
        target = np.linspace(energy[0], energy[-1], dense)
        mu, energy = np.interp(target, energy, mu), target
    assert np.isfinite(energy).all() and np.isfinite(mu).all()
    assert (np.diff(energy) > 0).all()
    return (
        energy,
        mu,
        {
            "file": path.relative_to(ROOT).as_posix(),
            "file_sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
            "input_sha256": hashlib.sha256(energy.tobytes() + mu.tobytes()).hexdigest(),
            "points": len(energy),
            "preprocessing": "linear interpolation to synthetic dense grid"
            if dense
            else "log(I0/It)"
            if name == "ru"
            else "original measured mu",
        },
    )


def parameters(energy, mu, setup):
    # E0 detection is deliberately shared, not silently counted for just one
    # implementation. Own-default E0 is recorded separately in the metadata.
    result = {
        "e0": float(find_e0(energy, mu)),
        "pre1": -150.0,
        "pre2": -30.0,
        "norm1": 150.0,
        "norm2": 700.0,
        "nnorm": 2,
        "nvict": 0,
        "kmin": 0.0,
        "kmax": 12.0,
        "kweight": 1,
        "dk": 0.1,
        "nclamp": 3,
        "clamp_lo": 0,
        "clamp_hi": 1,
    }
    result.update(SETUPS[setup])
    return result


def configure_rexafs(p, config):
    norm = rexafs.PrePostEdge()
    for name, key in (
        ("e0", "e0"),
        ("pre_edge_start", "pre1"),
        ("pre_edge_end", "pre2"),
        ("norm_start", "norm1"),
        ("norm_end", "norm2"),
        ("norm_polyorder", "nnorm"),
        ("n_victoreen", "nvict"),
    ):
        setattr(norm, name, p[key])
    bkg = rexafs.AUTOBK()
    bkg.ek0 = p["e0"]
    for key in (
        "rbkg",
        "kmin",
        "kmax",
        "kweight",
        "dk",
        "nfft",
        "kstep",
        "nclamp",
        "clamp_lo",
        "clamp_hi",
    ):
        setattr(bkg, key, p[key])
    bkg.window = "Hanning"
    (
        bkg.solver,
        bkg.linear_workspace_cache,
        bkg.clamp_scale_policy,
        bkg.linear_fallback_to_lm,
    ) = CONFIGS[config]
    # A direct-only result must not conceal an iterative fallback. The shipped
    # default keeps fallback enabled; report this diagnostic difference explicitly.
    ft = rexafs.XrayFFTF()
    ft.kmin, ft.kmax, ft.kweight = 2.0, p["kmax"], 2.0
    ft.dk, ft.dk2, ft.window = 1.0, 1.0, "KaiserBessel"
    ft.nfft, ft.kstep, ft.rmax_out = p["nfft"], p["kstep"], 10.0
    return (
        rexafs.NormalizationMethod.PrePostEdge(norm),
        rexafs.BackgroundMethod.AUTOBK(bkg),
        ft,
    )


class Pipeline:
    def __init__(self, energy, mu, p, config):
        self.energy, self.mu, self.p, self.config = energy, mu, p, config
        self.rx = configure_rexafs(p, config) if config != "larch" else None

    def create(self):
        if self.rx is None:
            return larch.Group(energy=self.energy, mu=self.mu)
        n, b, f = self.rx
        return (
            rexafs.Spectrum(self.energy, self.mu)
            .set_e0(self.p["e0"])
            .set_normalization_method(n)
            .set_background_method(b)
            .set_fft(f)
        )

    def normalize(self, s):
        if self.rx is not None:
            s.normalize()
        else:
            pre_edge(
                self.energy,
                self.mu,
                group=s,
                make_flat=True,
                **{
                    k: self.p[k]
                    for k in ("e0", "pre1", "pre2", "norm1", "norm2", "nnorm", "nvict")
                },
            )

    def background(self, s):
        if self.rx is not None:
            s.calc_background()
        else:
            autobk(
                self.energy,
                self.mu,
                group=s,
                edge_step=s.edge_step,
                win="hanning",
                calc_uncertainties=False,
                **{
                    k: self.p[k]
                    for k in (
                        "e0",
                        "rbkg",
                        "kmin",
                        "kmax",
                        "kweight",
                        "dk",
                        "nfft",
                        "kstep",
                        "nclamp",
                        "clamp_lo",
                        "clamp_hi",
                    )
                },
            )

    def transform(self, s):
        if self.rx is not None:
            s.fft()
        else:
            self.larch_fft(s.k, s.chi, s)

    def larch_fft(self, k, chi, group):
        xftf(
            k,
            chi,
            group=group,
            kmin=2.0,
            kmax=self.p["kmax"],
            kweight=2,
            dk=1.0,
            dk2=1.0,
            window="kaiser",
            nfft=self.p["nfft"],
            kstep=self.p["kstep"],
            rmax_out=10.0,
            with_phase=False,
        )

    def run(self, timed=False):
        stamps = [time.perf_counter_ns()]
        s = self.create()
        stamps.append(time.perf_counter_ns())
        self.normalize(s)
        stamps.append(time.perf_counter_ns())
        self.background(s)
        stamps.append(time.perf_counter_ns())
        self.transform(s)
        stamps.append(time.perf_counter_ns())
        if timed:
            return s, dict(
                zip(
                    ("construct", "normalize", "autobk", "fft", "pipeline"),
                    [*(np.diff(stamps) * 1e-6), (stamps[-1] - stamps[0]) * 1e-6],
                )
            )
        return s

    def output(self, s):
        fields = ("norm", "flat", "k", "chi", "r", "chir_mag", "chir_real", "chir_imag")
        if self.rx is not None:
            result = {k: np.asarray(getattr(s, k)()).copy() for k in fields}
            same = larch.Group()
            self.larch_fft(result["k"], result["chi"], same)
            result["same_chi_larch_real"] = same.chir.real.copy()
            result["same_chi_larch_imag"] = same.chir.imag.copy()
        else:
            result = {k: np.asarray(getattr(s, k)).copy() for k in fields[:-2]}
            result.update(chir_real=s.chir.real.copy(), chir_imag=s.chir.imag.copy())
        assert all(np.isfinite(v).all() for v in result.values())
        return result


def environment():
    def command(args):
        p = subprocess.run(args, text=True, capture_output=True, check=False)
        return p.stdout.strip() if p.returncode == 0 else None

    return {
        "utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "python": sys.version,
        "platform": platform.platform(),
        "machine": platform.machine(),
        "cpu": command(["sysctl", "-n", "machdep.cpu.brand_string"]),
        "physical_cores": command(["sysctl", "-n", "hw.physicalcpu"]),
        "logical_cores": os.cpu_count(),
        "memory_bytes": command(["sysctl", "-n", "hw.memsize"]),
        "versions": {
            p: importlib.metadata.version(p)
            for p in ("rexafs", "xraylarch", "numpy", "scipy", "lmfit", "threadpoolctl")
        },
        "threadpools": threadpool_info(),
        "thread_environment": {
            k: v
            for k, v in os.environ.items()
            if k
            in (
                "OPENBLAS_NUM_THREADS",
                "OMP_NUM_THREADS",
                "MKL_NUM_THREADS",
                "VECLIB_MAXIMUM_THREADS",
                "NUMEXPR_NUM_THREADS",
                "RAYON_NUM_THREADS",
            )
        },
        "checkout_commit": command(["git", "rev-parse", "HEAD"]),
        "rexafs_binary_sha256": hashlib.sha256(
            Path(rexafs._core.__file__).read_bytes()
        ).hexdigest(),
        "larch_autobk_source_sha256": hashlib.sha256(
            (Path(larch.__file__).parent / "xafs/autobk.py").read_bytes()
        ).hexdigest(),
        "script_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
    }


def worker(args):
    energy, mu, source = load_input(args.dataset)
    p = parameters(energy, mu, args.setup)
    auto_e0 = float(rexafs.Spectrum(energy, mu).find_e0().e0())
    pipeline = Pipeline(energy, mu, p, args.config)
    with threadpool_limits(limits=1):
        meta = environment()
        rss_before = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
        first, cold = pipeline.run(timed=True)
        if args.profile_seconds:
            # Import/setup/warm-up are outside the profile. New spectra inside
            # the loop prevent the terminal-stage result cache hiding work.
            profiler = cProfile.Profile()
            count = 0
            print(json.dumps({"ready_pid": os.getpid()}), flush=True)
            begin = time.perf_counter()
            profiler.enable()
            while time.perf_counter() - begin < args.profile_seconds:
                pipeline.run()
                count += 1
            profiler.disable()
            args.output.parent.mkdir(parents=True, exist_ok=True)
            profiler.dump_stats(str(args.output.with_suffix(".pstats")))
            with args.output.with_suffix(".txt").open("w") as f:
                pstats.Stats(profiler, stream=f).strip_dirs().sort_stats(
                    "cumulative"
                ).print_stats(60)
            result = {
                "environment": meta,
                "dataset": args.dataset,
                "config": args.config,
                "setup": args.setup,
                "iterations": count,
                "profile_seconds": args.profile_seconds,
            }
        else:
            for _ in range(3):
                pipeline.run()
            # Fixed batches of fresh spectra. Per-stage and total samples are
            # retained, not just a best result; report median and p95.
            repeats = max(3, min(50, int(100.0 / max(cold["pipeline"], 0.01))))
            samples = {k: [] for k in cold}
            for _ in range(args.rounds):
                batch = {k: [] for k in cold}
                for _ in range(repeats):
                    _, timing = pipeline.run(timed=True)
                    for k, v in timing.items():
                        batch[k].append(float(v))
                for k, values in batch.items():
                    samples[k].append(float(np.mean(values)))
            result = {
                "environment": meta,
                "dataset": args.dataset,
                "source": source,
                "config": args.config,
                "setup": args.setup,
                "parameters": p,
                "rexafs_auto_e0": auto_e0,
                "first_call_ms": cold,
                "samples_ms_per_spectrum": samples,
                "repeats_per_round": repeats,
                "rounds": args.rounds,
                "timing_ms": {
                    k: {
                        "median": float(np.median(v)),
                        "p95": float(np.percentile(v, 95)),
                        "min": min(v),
                        "max": max(v),
                    }
                    for k, v in samples.items()
                },
                "peak_rss_bytes": resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
                * (1 if sys.platform == "darwin" else 1024),
                "rss_before_bytes": rss_before
                * (1 if sys.platform == "darwin" else 1024),
                "incremental_peak_rss_bytes": (
                    resource.getrusage(resource.RUSAGE_SELF).ru_maxrss - rss_before
                )
                * (1 if sys.platform == "darwin" else 1024),
                "rss_scope": "Process imports both libraries; incremental peak excludes imports and shared E0 selection",
                "load_average": os.getloadavg(),
            }
            if args.config == "larch":
                d = first.autobk_details
                result["autobk_details"] = {
                    k: getattr(d, k) for k in ("nknots", "irbkg", "chisqr", "redchi")
                }
            args.output.parent.mkdir(parents=True, exist_ok=True)
            np.savez_compressed(
                args.output.with_suffix(".npz"), energy=energy, **pipeline.output(first)
            )
        args.output.write_text(
            json.dumps(result, indent=2, default=lambda o: o.item()) + "\n"
        )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--worker", action="store_true")
    parser.add_argument("--dataset", choices=DATASETS, default="cu")
    parser.add_argument("--config", choices=CONFIGS, default="larch")
    parser.add_argument("--setup", choices=SETUPS, default="standard")
    parser.add_argument("--rounds", type=int, default=7)
    parser.add_argument("--profile-seconds", type=float, default=0)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument(
        "--quick", action="store_true", help="Only original Cu / standard setup"
    )
    args = parser.parse_args()
    if args.worker:
        worker(args)
        return
    args.output.mkdir(parents=True, exist_ok=True)
    jobs = [
        (d, s, c)
        for d in (["cu"] if args.quick else DATASETS)
        for s in (["standard"] if args.quick else ["standard", "fine_fft"])
        for c in CONFIGS
    ]
    if not args.quick:
        jobs += [
            (d, s, c)
            for d in ("cu", "ni", "ru")
            for s in ("no_clamp", "aligned_rounding")
            for c in CONFIGS
        ]
    random.Random(20260906).shuffle(jobs)
    failures = []
    for index, (d, s, c) in enumerate(jobs):
        name = f"{d}--{s}--{c}"
        print(f"[{index + 1}/{len(jobs)}] {name}", flush=True)
        output = args.output / f"{name}.json"
        command = [
            sys.executable,
            str(Path(__file__).resolve()),
            "--worker",
            "--dataset",
            d,
            "--setup",
            s,
            "--config",
            c,
            "--rounds",
            str(args.rounds),
            "--output",
            str(output),
        ]
        proc = subprocess.run(command, text=True, capture_output=True, check=False)
        if proc.returncode:
            failures.append(
                {"case": name, "returncode": proc.returncode, "stderr": proc.stderr}
            )
            print(proc.stderr, file=sys.stderr)
        elif proc.stderr:
            output.with_suffix(".stderr.txt").write_text(proc.stderr)
    (args.output / "failures.json").write_text(json.dumps(failures, indent=2) + "\n")
    if failures:
        raise SystemExit(f"{len(failures)} benchmark cases failed; see failures.json")


if __name__ == "__main__":
    main()
