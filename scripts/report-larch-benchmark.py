#!/usr/bin/env python3
"""Summarize retained benchmark samples and compare numerical output arrays."""

import argparse
import csv
import json
from pathlib import Path

import matplotlib
import numpy as np

matplotlib.use("Agg")
matplotlib.rcParams["svg.hashsalt"] = "rexafs-larch-benchmark"
import matplotlib.pyplot as plt


def error_metrics(actual, reference):
    assert actual.shape == reference.shape, (actual.shape, reference.shape)
    assert np.isfinite(actual).all() and np.isfinite(reference).all()
    delta = actual - reference
    return {
        "relative_l2": float(
            np.linalg.norm(delta) / max(np.linalg.norm(reference), 1e-30)
        ),
        "rmse": float(np.sqrt(np.mean(np.abs(delta) ** 2))),
        "max_abs": float(np.max(np.abs(delta))),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("results", type=Path)
    args = parser.parse_args()
    root = args.results
    records = []
    for file in sorted(root.glob("*.json")):
        record = json.loads(file.read_text())
        if isinstance(record, dict) and "timing_ms" in record:
            records.append(record)
    assert records, "No successful benchmark cases"
    keyed = {(r["dataset"], r["setup"], r["config"]): r for r in records}
    metrics = []
    for record in records:
        d, s, c = (record[k] for k in ("dataset", "setup", "config"))
        reference = keyed.get((d, s, "larch"))
        if reference is None:
            continue
        data = np.load(root / f"{d}--{s}--{c}.npz")
        ref = np.load(root / f"{d}--{s}--larch.npz")
        np.testing.assert_allclose(data["energy"], ref["energy"], rtol=0, atol=0)
        np.testing.assert_allclose(data["k"], ref["k"], rtol=0, atol=1e-12)
        np.testing.assert_allclose(data["r"], ref["r"], rtol=0, atol=1e-12)
        kmask = (ref["k"] >= 2) & (ref["k"] <= record["parameters"]["kmax"])
        rmask = (ref["r"] >= 1) & (ref["r"] <= 3)
        peak = np.flatnonzero(rmask)[np.argmax(data["chir_mag"][rmask])]
        refpeak = np.flatnonzero(rmask)[np.argmax(ref["chir_mag"][rmask])]
        row = {
            "dataset": d,
            "setup": s,
            "config": c,
            "points": record["source"]["points"],
            "pipeline_ms": record["timing_ms"]["pipeline"]["median"],
            "pipeline_p95_ms": record["timing_ms"]["pipeline"]["p95"],
            "spectra_per_second": 1000 / record["timing_ms"]["pipeline"]["median"],
            "first_call_pipeline_ms": record["first_call_ms"]["pipeline"],
            "speedup_vs_larch": reference["timing_ms"]["pipeline"]["median"]
            / record["timing_ms"]["pipeline"]["median"],
            "peak_rss_mib": record["peak_rss_bytes"] / 1024**2,
            "incremental_peak_rss_mib": record["incremental_peak_rss_bytes"] / 1024**2,
            "r_peak_shift_angstrom": float(data["r"][peak] - ref["r"][refpeak]),
            "r_peak_height_relative_difference": float(
                data["chir_mag"][peak] / ref["chir_mag"][refpeak] - 1
            ),
        }
        for key in ("construct", "normalize", "autobk", "fft"):
            row[f"{key}_ms"] = record["timing_ms"][key]["median"]
        for field in ("norm", "flat", "chi", "chir_mag"):
            actual, expected = data[field], ref[field]
            if field == "chi":
                actual, expected = actual[kmask], expected[kmask]
            for key, value in error_metrics(actual, expected).items():
                row[f"{field}_{key}"] = value
        if c != "larch":
            actual = data["chir_real"] + 1j * data["chir_imag"]
            expected = data["same_chi_larch_real"] + 1j * data["same_chi_larch_imag"]
            row["fft_same_chi_relative_l2"] = error_metrics(actual, expected)[
                "relative_l2"
            ]
        else:
            row["fft_same_chi_relative_l2"] = 0.0
        metrics.append(row)
    with (root / "metrics.csv").open("w") as f:
        writer = csv.DictWriter(f, fieldnames=list(metrics[0]), lineterminator="\n")
        writer.writeheader()
        writer.writerows(metrics)
    (root / "metrics.json").write_text(json.dumps(metrics, indent=2) + "\n")

    datasets = [d for d in ("cu", "ni", "ru") if (d, "standard", "larch") in keyed]
    colors = {
        "larch": "black",
        "shipped_default": "#D55E00",
        "direct_two_pass": "#009E73",
        "legacy_lm": "#0072B2",
        "dogleg": "#CC79A7",
    }
    figure, axes = plt.subplots(
        len(datasets),
        3,
        figsize=(13, 3.3 * len(datasets)),
        squeeze=False,
        constrained_layout=True,
    )
    for index, dataset in enumerate(datasets):
        ref = np.load(root / f"{dataset}--standard--larch.npz")
        for config, color in colors.items():
            path = root / f"{dataset}--standard--{config}.npz"
            if not path.exists():
                continue
            data = np.load(path)
            axes[index, 0].plot(
                data["k"],
                data["k"] ** 2 * data["chi"],
                label=config,
                color=color,
                lw=1.15,
            )
            axes[index, 1].plot(
                data["r"], data["chir_mag"], label=config, color=color, lw=1.15
            )
            axes[index, 2].plot(
                data["k"],
                data["k"] ** 2 * (data["chi"] - ref["chi"]),
                label=config,
                color=color,
                lw=1.15,
            )
        axes[index, 0].set(
            xlim=(2, 12),
            xlabel="k (Å⁻¹)",
            ylabel="k²χ(k) (Å⁻²)",
            title=f"{dataset.upper()}: χ(k)",
        )
        axes[index, 1].set(
            xlim=(0, 6),
            xlabel="R (Å, without phase correction)",
            ylabel="|χ(R)| (Å⁻³)",
            title="Fourier magnitude",
        )
        axes[index, 2].set(
            xlim=(2, 12),
            xlabel="k (Å⁻¹)",
            ylabel="k²[χ − χLarch] (Å⁻²)",
            title="Difference from Larch",
        )
        for ax in axes[index]:
            ax.grid(alpha=0.2)
    axes[0, 0].legend(fontsize=8)
    figure.savefig(root / "output-comparison.png", dpi=180)
    figure.savefig(root / "output-comparison.svg", metadata={"Date": None})
    plt.close(figure)

    configs = [
        "larch",
        "shipped_default",
        "direct_cached",
        "direct_uncached",
        "direct_two_pass",
        "legacy_lm",
        "dogleg",
    ]
    figure, axes = plt.subplots(1, 2, figsize=(13, 4.8), constrained_layout=True)
    for ax, setup in zip(axes, ("standard", "fine_fft")):
        selected = [r for r in metrics if r["dataset"] == "cu" and r["setup"] == setup]
        selected = {r["config"]: r for r in selected}
        present = [c for c in configs if c in selected]
        bottom = np.zeros(len(present))
        for stage, color in zip(
            ("construct", "normalize", "autobk", "fft"),
            ("#999999", "#56B4E9", "#D55E00", "#009E73"),
        ):
            times = np.array([selected[c][f"{stage}_ms"] for c in present])
            ax.barh(present, times, left=bottom, label=stage, color=color)
            bottom += times
        ax.invert_yaxis()
        for index, config in enumerate(present):
            ax.annotate(
                f"{selected[config]['pipeline_ms']:.3f}",
                (bottom[index], index),
                xytext=(4, 0),
                textcoords="offset points",
                va="center",
                fontsize=8,
            )
        ax.set_xlim(0, max(bottom) * 1.16)
        ax.set(xlabel="Milliseconds per fresh spectrum", title=f"Cu: {setup}")
        ax.grid(axis="x", alpha=0.2)
    axes[0].legend(loc="lower right", fontsize=8)
    figure.savefig(root / "stage-timing.png", dpi=180)
    figure.savefig(root / "stage-timing.svg", metadata={"Date": None})
    plt.close(figure)

    # Matplotlib emits trailing spaces in SVG paths; keep checked-in artifacts
    # friendly to repository whitespace checks without changing their geometry.
    for path in root.glob("*.svg"):
        path.write_text(
            "\n".join(line.rstrip() for line in path.read_text().splitlines()) + "\n"
        )

    lines = [
        "# Benchmark measurements",
        "",
        "Generated from retained raw JSON samples and NPZ arrays.",
        "",
        "| Dataset | Setup | Configuration | Median ms | p95 of round means (ms) | Speedup | χ(k) relative L2 | χ(R) magnitude relative L2 |",
        "|---|---|---|---:|---:|---:|---:|---:|",
    ]
    for r in metrics:
        lines.append(
            f"| {r['dataset']} | {r['setup']} | {r['config']} | {r['pipeline_ms']:.4f} | {r['pipeline_p95_ms']:.4f} | {r['speedup_vs_larch']:.2f}× | {100 * r['chi_relative_l2']:.3f}% | {100 * r['chir_mag_relative_l2']:.3f}% |"
        )
    lines += [
        "",
        "The p95 is over per-round means, not individual-spectrum tail latency. Speedup compares the complete public pipeline calls used here; the two APIs also perform different ancillary work. Agreement with Larch is not ground-truth accuracy.",
        "",
        "![Output comparison](output-comparison.png)",
        "![Stage timing](stage-timing.png)",
        "",
    ]
    (root / "measurements.md").write_text("\n".join(lines))
    print(f"Wrote {len(metrics)} comparisons to {root}")


if __name__ == "__main__":
    main()
