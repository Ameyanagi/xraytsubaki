#!/usr/bin/env python3
"""Validate and time production fixed-lambda AUTOBK against retained references."""

from __future__ import annotations

import argparse
import cProfile
import importlib.util
import io
import json
import os
import platform
import pstats
import random
import subprocess
import sys
import time
import zipfile
from pathlib import Path

spec = importlib.util.spec_from_file_location(
    "study", Path(__file__).with_name("study-autobk-clamps.py")
)
study = importlib.util.module_from_spec(spec)
spec.loader.exec_module(study)
bench = study.bench
np = study.np
from threadpoolctl import threadpool_info, threadpool_limits

# Same fixed objective, independent SciPy solve. These bounds leave headroom
# over the measured native/CI roundoff without masking algorithm changes.
FIXED_RTOL = 1e-11
FIXED_ATOL = 1e-12
# Stock Larch updates its clamp scale, so this is a scientific compatibility
# bound on measured scans at the default lambda, not a roundoff assertion.
LARCH_RELATIVE_L2_LIMIT = 5e-5  # 0.005%, over 2 <= k <= kmax


def assert_fixed_reference(actual, expected, label):
    np.testing.assert_equal(np.shape(actual), np.shape(expected), err_msg=label)
    assert np.all(np.isfinite(actual)) and np.all(np.isfinite(expected)), label
    np.testing.assert_allclose(
        actual, expected, rtol=FIXED_RTOL, atol=FIXED_ATOL, err_msg=label
    )


def rx_spectrum(energy, mu, p, step, penalty=0.001, cached=True, legacy=False):
    norm = bench.rexafs.PrePostEdge()
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
    norm.edge_step = step
    bkg = bench.rexafs.AUTOBK()
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
    bkg.ek0 = p["e0"]
    bkg.clamp_scale_policy = "Fixed" if legacy else "FixedPenalty"
    if not legacy:
        bkg.clamp_lambda = penalty
    bkg.linear_workspace_cache = cached
    # Strict results cannot silently use a nonlinear fallback.
    bkg.linear_fallback_to_lm = False
    sp = (
        bench.rexafs.Spectrum(energy, mu)
        .set_e0(p["e0"])
        .set_normalization_method(bench.rexafs.NormalizationMethod.PrePostEdge(norm))
        .set_background_method(bench.rexafs.BackgroundMethod.AUTOBK(bkg))
    )
    sp.normalize()
    return sp


def rx_fit(*args, **kwargs):
    sp = rx_spectrum(*args, **kwargs)
    sp.calc_background()
    return np.asarray(sp.k()), np.asarray(sp.chi())


def validate(output):
    rows = []
    archive = bench.ROOT / "doc/benchmarks/2026-09-07-clamp-study/raw-cases.zip"
    with zipfile.ZipFile(archive) as z:
        for name in sorted(
            n for n in z.namelist() if n.endswith(".json") and n != "study.json"
        ):
            record = json.loads(z.read(name))
            with np.load(io.BytesIO(z.read(name.replace(".json", ".npz")))) as raw:
                for penalty in (0, 0.001, 1):
                    k, chi = rx_fit(
                        raw["energy"],
                        raw["mu"],
                        record["parameters"],
                        record["edge_step"],
                        penalty,
                    )
                    reference = raw[study.fixed_name(penalty)]
                    np.testing.assert_allclose(k, raw["k"], rtol=0, atol=1e-14)
                    error = study.relative(chi, reference)
                    assert_fixed_reference(chi, reference, f"{name} lambda={penalty}")
                    rows.append(
                        {
                            "case": name.removesuffix(".json"),
                            "lambda": penalty,
                            "relative_l2_vs_prototype": error,
                            "max_abs_vs_prototype": float(
                                np.max(np.abs(chi - reference))
                            ),
                            "relative_l2_vs_larch": study.relative(
                                chi, raw["larch_dynamic"]
                            ),
                        }
                    )
    extra = []
    reference_etok = study.ab.ETOK
    rust_etok = 1 / (
        1e20
        * (6.62607015e-34 / (2 * np.pi)) ** 2
        / (2 * 9.1093837139e-31 * 1.602176634e-19)
    )
    for dataset in ("cu", "ni", "ru"):
        energy, mu, _ = bench.load_input(dataset)
        p = bench.parameters(energy, mu, "standard")
        pipeline = bench.Pipeline(energy, mu, p, "larch")
        group = pipeline.create()
        pipeline.normalize(group)
        step = float(group.edge_step)
        for kmin, lo, hi in ((0, 0, 0), (0, 2, 0), (0, 2, 5), (2, 0, 1), (2, 2, 5)):
            p.update(kmin=kmin, clamp_lo=lo, clamp_hi=hi)
            np.testing.assert_allclose(rust_etok, reference_etok, rtol=0, atol=1e-16)
            problem = study.Problem(energy, mu, p, step)
            for penalty in (0.001, 1):
                expected, _ = problem.fixed(penalty=penalty)
                _, actual = rx_fit(energy, mu, p, step, penalty, cached=False)
                assert_fixed_reference(
                    actual,
                    expected,
                    f"{dataset} kmin={kmin} lo={lo} hi={hi} lambda={penalty}",
                )
                extra.append(
                    {
                        "dataset": dataset,
                        "kmin": kmin,
                        "lo": lo,
                        "hi": hi,
                        "penalty": penalty,
                        "relative_l2": study.relative(actual, expected),
                        "max_abs": float(np.max(np.abs(actual - expected))),
                    }
                )
    result = {
        "reference_archive_sha256": study.sha(archive),
        "fits": len(rows),
        "fixed_reference_tolerance": {"rtol": FIXED_RTOL, "atol": FIXED_ATOL},
        "extra_endpoint_checks": extra,
        "larch_etok": reference_etok,
        "rexafs_etok": rust_etok,
        "constants_revision": "CODATA 2022 in both packages; Larch unmodified",
        "rows": rows,
    }
    (output / "validation.json").write_text(json.dumps(result, indent=2) + "\n")
    print(
        f"Validated {len(rows)} production fits; worst prototype relative error {max(r['relative_l2_vs_prototype'] for r in rows):.3g}",
        flush=True,
    )


def modes(old=False):
    if old:
        return {
            "larch": None,
            "published_0.1.0_cached": {"legacy": True},
            "published_0.1.0_uncached": {"legacy": True, "cached": False},
        }
    return {
        "larch": None,
        "fixed_0.001_cached": {},
        "fixed_0.001_uncached": {"cached": False},
        "fixed_0_cached": {"penalty": 0},
        "fixed_1_cached": {"penalty": 1},
        "legacy_fixed_cached": {"legacy": True},
    }


def timing(output, old=False, quick=False):
    rows, arrays, host_loads, compatibility = [], {}, [], []
    rng = random.Random(20260907)
    datasets = ["cu"] if quick else list(bench.DATASETS)
    for dataset in datasets:
        energy, mu, source = bench.load_input(dataset)
        for setup in ("standard", "fine_fft"):
            p = bench.parameters(energy, mu, setup)
            pipeline = bench.Pipeline(energy, mu, p, "larch")
            group = pipeline.create()
            pipeline.normalize(group)
            step = float(group.edge_step)
            reference = study.call_larch(energy, mu, p, step)
            options = modes(old)
            times = {mode: [] for mode in options}
            full = {mode: [] for mode in options}
            for round_index in range(9):
                host_loads.append(
                    {
                        "dataset": dataset,
                        "setup": setup,
                        "round": round_index,
                        "load_average": os.getloadavg(),
                    }
                )
                order = list(options)
                rng.shuffle(order)
                for mode in order:
                    config = options[mode]
                    for _ in range(5 if quick else 20):
                        if config is None:
                            start = time.perf_counter_ns()
                            result = study.call_larch(energy, mu, p, step)
                            elapsed = (time.perf_counter_ns() - start) / 1e6
                            total = elapsed
                            chi = result.chi
                        else:
                            begin = time.perf_counter_ns()
                            result = rx_spectrum(energy, mu, p, step, **config)
                            start = time.perf_counter_ns()
                            result.calc_background()
                            end = time.perf_counter_ns()
                            elapsed, total = (end - start) / 1e6, (end - begin) / 1e6
                            chi = np.asarray(result.chi())
                        if round_index >= 2:
                            times[mode].append(elapsed)
                            full[mode].append(total)
                    arrays[f"{dataset}__{setup}__{mode}"] = chi.copy()
            arrays[f"{dataset}__{setup}__k"] = np.asarray(reference.k)
            if not old and dataset in ("cu", "ni", "ru"):
                k = np.asarray(reference.k)
                mask = (k >= 2) & (k <= p["kmax"])
                assert mask.any(), f"empty Larch comparison range: {dataset}/{setup}"
                chi = arrays[f"{dataset}__{setup}__fixed_0.001_cached"]
                error = study.relative(chi[mask], reference.chi[mask])
                assert np.isfinite(error) and error <= LARCH_RELATIVE_L2_LIMIT, (
                    f"{dataset}/{setup}: Larch relative L2 {error} exceeds "
                    f"{LARCH_RELATIVE_L2_LIMIT} at lambda=0.001"
                )
                compatibility.append(
                    {"dataset": dataset, "setup": setup, "relative_l2": error}
                )
            for mode in options:
                chi = arrays[f"{dataset}__{setup}__{mode}"]
                rows.append(
                    {
                        "dataset": dataset,
                        "setup": setup,
                        "mode": mode,
                        "parameters": p,
                        "edge_step": step,
                        "source": source,
                        "background_ms": {
                            "median": float(np.median(times[mode])),
                            "p10": float(np.quantile(times[mode], 0.1)),
                            "p90": float(np.quantile(times[mode], 0.9)),
                            "samples": times[mode],
                        },
                        "construct_normalize_background_ms": full[mode]
                        if mode != "larch"
                        else None,
                        "chi_relative_l2_vs_larch": study.relative(chi, reference.chi),
                    }
                )
            print(f"Timed {dataset}/{setup}", flush=True)
    suffix = "published" if old else "candidate"
    if not old:
        (output / "larch-compatibility.json").write_text(
            json.dumps(
                {
                    "lambda": 0.001,
                    "relative_l2_limit": LARCH_RELATIVE_L2_LIMIT,
                    "k_range": "2 <= k <= kmax",
                    "rows": compatibility,
                },
                indent=2,
            )
            + "\n"
        )
    (output / f"timing-{suffix}.json").write_text(
        json.dumps(
            {"rows": rows, "host_loads": host_loads, "logical_cpus": os.cpu_count()},
            indent=2,
        )
        + "\n"
    )
    np.savez_compressed(output / f"chi-{suffix}.npz", **arrays)


def profile(output):
    energy, mu, _ = bench.load_input("cu")
    p = bench.parameters(energy, mu, "standard")
    pipeline = bench.Pipeline(energy, mu, p, "larch")
    group = pipeline.create()
    pipeline.normalize(group)
    step = float(group.edge_step)
    for mode in ("larch", "fixed_0.001_cached", "fixed_0.001_uncached"):
        config = modes()[mode]

        def run(config=config):
            if config is None:
                return study.call_larch(energy, mu, p, step)
            sp = rx_spectrum(energy, mu, p, step, **config)
            sp.calc_background()
            return sp

        for _ in range(3):
            run()
        profiler = cProfile.Profile()
        profiler.enable()
        for _ in range(300):
            run()
        profiler.disable()
        profiler.dump_stats(str(output / f"{mode}.pstats"))
        with (output / f"{mode}-profile.txt").open("w") as stream:
            pstats.Stats(profiler, stream=stream).strip_dirs().sort_stats(
                "cumulative"
            ).print_stats(45)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--validate-only", action="store_true")
    parser.add_argument("--timing-only", action="store_true")
    parser.add_argument("--published", action="store_true")
    parser.add_argument("--quick", action="store_true")
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    with threadpool_limits(limits=1):
        if not args.published and not args.timing_only:
            validate(args.output)
        if not args.validate_only:
            timing(args.output, args.published, args.quick)
            if not args.published:
                profile(args.output)
        label = "published" if args.published else "candidate"
        import importlib.metadata

        import rexafs._core as core

        provenance = {
            "rexafs_version": importlib.metadata.version("rexafs"),
            "scipy_version": importlib.metadata.version("scipy"),
            "larch_version": importlib.metadata.version("xraylarch"),
            "python": sys.version,
            "platform": platform.platform(),
            "threadpools": threadpool_info(),
            "extension_sha256": study.sha(Path(core.__file__)),
            "extension_path": core.__file__,
            "script_sha256": study.sha(Path(__file__)),
            "head": subprocess.check_output(
                ["git", "rev-parse", "HEAD"], text=True
            ).strip(),
            "source_sha256": None
            if args.published
            else {
                str(p.relative_to(bench.ROOT)): study.sha(p)
                for p in [
                    bench.ROOT / "crates/rexafs/src/xafs/background.rs",
                    bench.ROOT / "crates/rexafs/src/xafs/background/fixed.rs",
                    bench.ROOT / "crates/rexafs/src/xafs/spline.rs",
                    bench.ROOT / "crates/rexafs/src/xafs/constants.rs",
                ]
            },
        }
        (args.output / f"provenance-{label}.json").write_text(
            json.dumps(provenance, indent=2) + "\n"
        )


if __name__ == "__main__":
    main()
