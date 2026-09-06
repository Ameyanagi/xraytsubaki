#!/usr/bin/env python3
"""Report clamp-study arrays, decompose truth errors, and time/profile prototypes."""

import argparse
import cProfile
import csv
import importlib.util
import io
import itertools
import json
import pstats
import random
import time
import zipfile
from pathlib import Path

spec = importlib.util.spec_from_file_location(
    "study", Path(__file__).with_name("study-autobk-clamps.py")
)
if spec is None or spec.loader is None:
    raise RuntimeError("Cannot import study-autobk-clamps.py")
study = importlib.util.module_from_spec(spec)
spec.loader.exec_module(study)

import numpy as np
from threadpoolctl import threadpool_limits

METHODS = (
    "larch_dynamic",
    "fixed_0",
    "fixed_0.001",
    "fixed_1",
    "frozen_initial",
    "rexafs_direct",
)
LABELS = {
    "larch_dynamic": "Larch dynamic",
    "fixed_0": "Direct, no clamp",
    "fixed_0.001": "Fixed λ=0.001",
    "fixed_1": "Fixed λ=1",
    "frozen_initial": "Frozen initial scale",
    "rexafs_direct": "rexafs candidate",
}
COLORS = {
    "larch_dynamic": "#0072B2",
    "fixed_0": "#E69F00",
    "fixed_0.001": "#009E73",
    "fixed_1": "#CC79A7",
    "frozen_initial": "#999999",
    "rexafs_direct": "#D55E00",
}


def raw_bytes(output, name):
    """Read a fresh run or the lossless checked-in archive without extraction."""
    path = output / name
    if path.exists():
        return path.read_bytes()
    with zipfile.ZipFile(output / "raw-cases.zip") as archive:
        return archive.read(name)


def raw_arrays(output, name):
    return np.load(io.BytesIO(raw_bytes(output, name)))


def aggregate(rows):
    result = {}
    for method in METHODS:
        selected = [row for row in rows if row["method"] == method]
        if not selected:
            continue
        values = np.array([row["chi_relative_l2"] for row in selected])
        result[method] = {
            "count": len(selected),
            "chi_mean": float(np.mean(values)),
            "chi_median": float(np.median(values)),
            "chi_p95": float(np.percentile(values, 95)),
            "chi_rmse_mean": float(np.mean([row["chi_rmse"] for row in selected])),
            "endpoint_rmse_mean": float(
                np.mean([row["endpoint_rmse"] for row in selected])
            ),
            "r_1_4_error_mean": float(
                np.mean([row["r_1_4_complex_relative_l2"] for row in selected])
            ),
        }
    return result


def save_figure(figure, output, name):
    import matplotlib.pyplot as plt

    figure.savefig(output / f"{name}.png", dpi=170)
    figure.savefig(output / f"{name}.svg", metadata={"Date": None})
    plt.close(figure)
    path = output / f"{name}.svg"
    path.write_text(
        "\n".join(line.rstrip() for line in path.read_text().splitlines()) + "\n"
    )


def plots(output, rows):
    import matplotlib

    matplotlib.use("Agg")
    matplotlib.rcParams["svg.hashsalt"] = "rexafs-clamp-ground-truth-study"
    import matplotlib.pyplot as plt

    figure, axes = plt.subplots(3, 2, figsize=(12, 9), constrained_layout=True)
    for row, family in enumerate(("single", "double", "feff")):
        name = f"test-{family}-bump-a1-noise0-k12-r0"
        with raw_arrays(output, f"{name}.npz") as arrays:
            k = arrays["k"]
            mask = (k >= 2) & (k <= 12)
            truth = arrays["true_chi"]
            axes[row, 0].plot(
                k[mask], truth[mask], color="black", lw=1.8, label="Known true χ(k)"
            )
            for method in ("larch_dynamic", "fixed_0", "fixed_1", "rexafs_direct"):
                style = "--" if method == "fixed_0" else "-"
                axes[row, 0].plot(
                    k[mask],
                    arrays[method][mask],
                    color=COLORS[method],
                    ls=style,
                    lw=1,
                    label=LABELS[method],
                )
                axes[row, 1].plot(
                    k[mask],
                    (arrays[method] - truth)[mask],
                    color=COLORS[method],
                    ls=style,
                    lw=1,
                    label=LABELS[method],
                )
            for col, ax in enumerate(axes[row]):
                ax.set(
                    xlabel="k (Å⁻¹)",
                    ylabel="χ(k)" if col == 0 else "Recovered − true χ(k)",
                    title=f"{family.upper()}: {'recovered signal' if col == 0 else 'recovery error'}",
                    xlim=(2, 12),
                )
                ax.grid(alpha=0.2)
    axes[0, 0].legend(fontsize=8, ncol=2)
    figure.suptitle(
        "Held-out noiseless spectra with a known smooth background\nA stronger endpoint penalty helps some signals and harms others",
        fontsize=13,
    )
    save_figure(figure, output, "synthetic-chi")

    figure, axes = plt.subplots(3, 2, figsize=(12, 9), constrained_layout=True)
    for row, dataset in enumerate(("cu", "ni", "ru")):
        with raw_arrays(output, f"{dataset}-standard-k12.npz") as arrays:
            k = arrays["k"]
            mask = k >= 2
            for method in (
                "larch_dynamic",
                "fixed_0.001",
                "frozen_initial",
                "rexafs_direct",
            ):
                axes[row, 0].plot(
                    k[mask],
                    arrays[method][mask],
                    color=COLORS[method],
                    lw=1,
                    label=LABELS[method],
                    ls="--" if method == "fixed_0.001" else "-",
                )
                if method != "larch_dynamic":
                    axes[row, 1].plot(
                        k[mask],
                        (arrays[method] - arrays["larch_dynamic"])[mask],
                        color=COLORS[method],
                        lw=1,
                        label=LABELS[method],
                    )
            for col, ax in enumerate(axes[row]):
                ax.set(
                    xlabel="k (Å⁻¹)",
                    ylabel="χ(k)" if col == 0 else "Δχ(k) versus Larch",
                    title=f"{dataset.upper()}: {'measured scan' if col == 0 else 'agreement, not ground truth'}",
                    xlim=(2, 12),
                )
                ax.grid(alpha=0.2)
    axes[0, 0].legend(fontsize=8)
    figure.suptitle(
        "Measured Cu, Ni and Ru: identical settings and a shared edge step", fontsize=13
    )
    save_figure(figure, output, "measured-chi")

    figure, axes = plt.subplots(1, 2, figsize=(12, 4.5), constrained_layout=True)
    grid = [study.fixed_name(value) for value in study.LAMBDAS]
    for split, label in (
        ("train", "Training (select penalty here)"),
        ("test", "Held-out (evaluation only)"),
    ):
        vals = [
            100
            * np.mean(
                [
                    row["chi_relative_l2"]
                    for row in rows
                    if row.get("split") == split and row["method"] == method
                ]
            )
            for method in grid
        ]
        axes[0].plot(range(len(grid)), vals, marker="o", label=label)
    axes[0].set(
        xticks=range(len(grid)),
        xticklabels=[f"{v:g}" for v in study.LAMBDAS],
        xlabel="Fixed penalty λ (zero is an explicit control)",
        ylabel="Mean χ(k) error (%)",
        title="Training selected λ=0; retain the whole sweep",
    )
    axes[0].legend(fontsize=8)
    for method in ("larch_dynamic", "fixed_0", "fixed_1", "rexafs_direct"):
        vals = [
            100
            * np.mean(
                [
                    row["chi_relative_l2"]
                    for row in rows
                    if row.get("split") == "test"
                    and row["method"] == method
                    and row["noise_sigma"] == 0
                    and row["signal"] == family
                ]
            )
            for family in ("single", "double", "feff")
        ]
        axes[1].plot(
            ("Single shell", "Two shells", "FEFF-derived"),
            vals,
            marker="o",
            label=LABELS[method],
            color=COLORS[method],
        )
    axes[1].set(
        ylabel="Mean χ(k) error (%)",
        title="Noiseless held-out signals: no universal winner",
    )
    axes[1].legend(fontsize=8)
    for ax in axes:
        ax.grid(alpha=0.2)
    save_figure(figure, output, "penalty-sensitivity")


def decompose(output):
    rows = []
    for family, bg, kmax in itertools.product(
        ("single", "double", "feff"), ("polynomial", "bump"), (10, 12)
    ):
        name = f"test-{family}-{bg}-a1-noise0-k{kmax}-r0"
        metadata = json.loads(raw_bytes(output, f"{name}.json"))
        with raw_arrays(output, f"{name}.npz") as arrays:
            problem = study.Problem(
                arrays["energy"], arrays["mu"], metadata["parameters"], 1.0
            )
            zero = np.zeros(problem.ncoef)
            interpolate = lambda values, problem=problem, zero=zero: (
                study.ab.spline_eval(
                    problem.kraw, values, problem.knots, zero, problem.order, problem.k
                )[1]
            )
            signal = interpolate(arrays["raw_true_chi"])
            background = interpolate(arrays["true_background"])
            truth = arrays["true_chi"]
            mask = (problem.k >= 2) & (problem.k <= kmax)
            components = {"k": problem.k, "true_chi": truth}
            for penalty in (0.0, 0.001, 1.0):
                method = study.fixed_name(penalty)
                recovered_signal, _ = problem.fixed(penalty=penalty, chi_data=signal)
                leaked_background, _ = problem.fixed(
                    penalty=penalty, chi_data=background
                )
                reconstructed = recovered_signal + leaked_background
                np.testing.assert_allclose(
                    reconstructed, arrays[method], rtol=1e-9, atol=2e-11
                )
                interpolation_error = signal - truth
                signal_removed = recovered_signal - signal
                delta = reconstructed - truth
                np.testing.assert_allclose(
                    interpolation_error + signal_removed + leaked_background,
                    delta,
                    rtol=1e-9,
                    atol=2e-11,
                )
                rows.append(
                    {
                        "case": name,
                        "method": method,
                        "total_rmse": study.rmse(delta[mask]),
                        "interpolation_rmse": study.rmse(interpolation_error[mask]),
                        "signal_removed_rmse": study.rmse(signal_removed[mask]),
                        "background_leakage_rmse": study.rmse(leaked_background[mask]),
                    }
                )
                components[f"{method}_interpolation_error"] = interpolation_error
                components[f"{method}_signal_removed"] = signal_removed
                components[f"{method}_background_leakage"] = leaked_background
            np.savez_compressed(output / f"decomposition-{name}.npz", **components)
    (output / "error-decomposition.json").write_text(json.dumps(rows, indent=2) + "\n")


def timing_and_profiles(output):
    rows = []
    for dataset in ("cu", "ni", "ru"):
        name = f"{dataset}-standard-k12"
        metadata = json.loads(raw_bytes(output, f"{name}.json"))
        with raw_arrays(output, f"{name}.npz") as arrays:
            energy, mu = arrays["energy"], arrays["mu"]
        p, step = metadata["parameters"], metadata["edge_step"]
        prepared = study.Problem(energy, mu, p, step)

        def run(method, energy=energy, mu=mu, p=p, step=step, prepared=prepared):
            start = time.perf_counter_ns()
            if method == "larch_dynamic":
                result = study.call_larch(energy, mu, p, step)
            elif method == "rexafs_direct":
                return study.rexafs_direct(energy, mu, p, step)[2]["background_ms"]
            elif method == "fixed_prepared":
                result = prepared.fixed(penalty=0.001)
            else:
                penalty = 0 if method == "fixed_0" else 0.001
                result = study.Problem(energy, mu, p, step).fixed(penalty=penalty)
            elapsed = (time.perf_counter_ns() - start) / 1e6
            del result
            return elapsed

        methods = (
            "larch_dynamic",
            "fixed_0",
            "fixed_0.001",
            "fixed_prepared",
            "rexafs_direct",
        )
        for method in methods:
            for _ in range(3):
                run(method)
        samples = {method: [] for method in methods}
        order = list(methods)
        rng = random.Random(20260907)
        for _ in range(7):
            rng.shuffle(order)
            for method in order:
                samples[method].append(float(np.mean([run(method) for _ in range(20)])))
        for method, values in samples.items():
            rows.append(
                {
                    "dataset": dataset,
                    "method": method,
                    "samples_ms": values,
                    "median_ms": float(np.median(values)),
                    "p95_ms": float(np.percentile(values, 95)),
                }
            )
        if dataset == "cu":
            for method in methods:
                profiler = cProfile.Profile()
                profiler.enable()
                for _ in range(300):
                    run(method)
                profiler.disable()
                profiler.dump_stats(str(output / f"profile-cu-{method}.pstats"))
                with (output / f"profile-cu-{method}.txt").open("w") as stream:
                    pstats.Stats(profiler, stream=stream).strip_dirs().sort_stats(
                        "cumulative"
                    ).print_stats(35)
    (output / "timings.json").write_text(
        json.dumps(
            {
                "environment": study.bench.environment(),
                "rounds": 7,
                "fresh_fits_per_round": 20,
                "scope": "Background only, shared E0/edge step; fixed_0 and fixed_0.001 include model preparation. fixed_prepared excludes preparation and is not an end-to-end comparison. rexafs normalization/setup and output copying occur outside its internal stage timer.",
                "rows": rows,
            },
            indent=2,
        )
        + "\n"
    )


def floor_validation(output):
    """Repeat the original public pipeline settings, including own normalization."""
    baseline = study.ROOT / "doc/benchmarks/2026-09-06-larch/baseline-0.1.0"
    rows = []
    for dataset, setup in itertools.product(
        ("cu", "ni", "ru"), ("standard", "fine_fft")
    ):
        energy, mu, source = study.bench.load_input(dataset)
        p = study.bench.parameters(energy, mu, setup)
        arrays = {}
        with np.load(baseline / f"{dataset}--{setup}--larch.npz") as data:
            k, reference = data["k"], data["chi"]
            arrays.update(k=k, larch=reference)
        mask = (k >= 2) & (k <= p["kmax"])
        for config in ("shipped_default", "direct_cached"):
            pipeline = study.bench.Pipeline(energy, mu, p, config)
            actual = pipeline.output(pipeline.run())
            np.testing.assert_allclose(actual["k"], k, rtol=0, atol=1e-12)
            with np.load(baseline / f"{dataset}--{setup}--{config}.npz") as data:
                previous = data["chi"]
            arrays[config] = actual["chi"]
            arrays[f"previous_{config}"] = previous
            rows.append(
                {
                    "dataset": dataset,
                    "setup": setup,
                    "config": config,
                    "source": source,
                    "parameters": p,
                    "old_vs_larch_relative_l2": study.relative(
                        previous[mask], reference[mask]
                    ),
                    "new_vs_larch_relative_l2": study.relative(
                        actual["chi"][mask], reference[mask]
                    ),
                    "new_vs_old_relative_l2": study.relative(
                        actual["chi"][mask], previous[mask]
                    ),
                }
            )
        np.testing.assert_allclose(
            arrays["shipped_default"], arrays["direct_cached"], rtol=0, atol=1e-12
        )
        np.savez_compressed(output / f"floor-{dataset}-{setup}.npz", **arrays)
    (output / "floor-validation.json").write_text(
        json.dumps(
            {
                "environment": study.bench.environment(),
                "rows": rows,
                "note": "Original 0.1.0 public pipeline settings repeated with the local 0.1.1 floor candidate; clamp code unchanged. Default and strict direct outputs match in every case. This is output validation, not a new timing matrix.",
            },
            indent=2,
        )
        + "\n"
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    parser.add_argument("--timings", action="store_true")
    args = parser.parse_args()
    result = json.loads(raw_bytes(args.output, "study.json"))
    rows = result["rows"]
    groups = {
        "train": aggregate([row for row in rows if row.get("split") == "train"]),
        "test": aggregate([row for row in rows if row.get("split") == "test"]),
    }
    for sigma in (0.0, 0.0003, 0.003):
        groups[f"test-noise-{sigma:g}"] = aggregate(
            [
                row
                for row in rows
                if row.get("split") == "test" and row["noise_sigma"] == sigma
            ]
        )
    for family in ("single", "double", "feff"):
        groups[f"test-noiseless-{family}"] = aggregate(
            [
                row
                for row in rows
                if row.get("split") == "test"
                and row["noise_sigma"] == 0
                and row["signal"] == family
            ]
        )
    (args.output / "aggregate.json").write_text(json.dumps(groups, indent=2) + "\n")
    fields = sorted({key for row in rows for key in row} - {"details", "source"})
    with (args.output / "metrics.csv").open("w") as stream:
        writer = csv.DictWriter(
            stream, fieldnames=fields, extrasaction="ignore", lineterminator="\n"
        )
        writer.writeheader()
        writer.writerows(rows)
    plots(args.output, rows)
    with threadpool_limits(limits=1):
        decompose(args.output)
        floor_validation(args.output)
        if args.timings:
            timing_and_profiles(args.output)


if __name__ == "__main__":
    main()
