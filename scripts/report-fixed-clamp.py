#!/usr/bin/env python3
"""Render the production fixed-lambda timing matrix and measured chi comparison."""

import argparse
import csv
import json
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
matplotlib.rcParams["svg.hashsalt"] = "rexafs-fixed-20260907"
import matplotlib.pyplot as plt
import numpy as np


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    out = args.output
    current = json.loads((out / "timing-candidate.json").read_text())
    published = json.loads((out / "timing-published.json").read_text())
    a, b = np.load(out / "chi-candidate.npz"), np.load(out / "chi-published.npz")
    rows = []
    for data, arrays in ((current, a), (published, b)):
        for row in data["rows"]:
            prefix = f"{row['dataset']}__{row['setup']}__"
            k, ref = arrays[prefix + "k"], arrays[prefix + "larch"]
            mask = (k >= 2) & (k <= row["parameters"]["kmax"])
            chi = arrays[prefix + row["mode"]]
            rows.append(
                {
                    "dataset": row["dataset"],
                    "setup": row["setup"],
                    "mode": row["mode"],
                    "median_ms": row["background_ms"]["median"],
                    "p10_ms": row["background_ms"]["p10"],
                    "p90_ms": row["background_ms"]["p90"],
                    "chi_relative_l2_percent_k2_kmax": float(
                        100
                        * np.linalg.norm((chi - ref)[mask])
                        / np.linalg.norm(ref[mask])
                    ),
                }
            )
    with (out / "matrix.csv").open("w", newline="") as stream:
        writer = csv.DictWriter(stream, fieldnames=list(rows[0]), lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)
    fig, axes = plt.subplots(3, 2, figsize=(12, 9), constrained_layout=True)
    for i, dataset in enumerate(("cu", "ni", "ru")):
        prefix = f"{dataset}__standard__"
        k = a[prefix + "k"]
        mask = k >= 2
        ref, chi, old = (
            a[prefix + "larch"],
            a[prefix + "fixed_0.001_cached"],
            b[prefix + "published_0.1.0_cached"],
        )
        axes[i, 0].plot(
            k[mask],
            old[mask],
            color="#e28e2c",
            lw=1,
            alpha=0.8,
            label="Published rexafs 0.1.0",
        )
        axes[i, 0].plot(k[mask], ref[mask], color="#264b70", lw=1.6, label="Larch")
        axes[i, 0].plot(
            k[mask],
            chi[mask],
            color="#24936e",
            lw=1.2,
            ls="--",
            label="New rexafs, λ=0.001",
        )
        error = 100 * np.linalg.norm((chi - ref)[mask]) / np.linalg.norm(ref[mask])
        axes[i, 0].set(
            title=f"{dataset.capitalize()} — χ(k)", ylabel="χ(k)", xlim=(2, 12)
        )
        axes[i, 1].plot(k[mask], (chi - ref)[mask], color="#24936e", lw=1.2)
        axes[i, 1].axhline(0, color="#264b70", lw=0.6)
        axes[i, 1].set(
            title=f"New rexafs − Larch: relative L2 = {error:.6f}%",
            ylabel="Δχ(k)",
            xlim=(2, 12),
        )
        axes[i, 1].ticklabel_format(axis="y", style="sci", scilimits=(0, 0))
        for ax in axes[i]:
            ax.grid(alpha=0.2)
            ax.set_xlabel("k (Å⁻¹)")
    axes[0, 0].legend(fontsize=8)
    fig.suptitle(
        "Production fixed-λ implementation versus Larch\nShared input, E₀ and edge step; CODATA 2022 in the new implementation",
        fontsize=13,
    )
    for suffix in ("png", "svg"):
        fig.savefig(
            out / f"measured-chi.{suffix}",
            dpi=180,
            metadata={"Date": None} if suffix == "svg" else None,
        )
    plt.close(fig)
    labels = ["Cu", "Ni", "Ru", "Cu 8,192", "Cu 32,768"]
    datasets = ["cu", "ni", "ru", "cu_dense_8192", "cu_dense_32768"]
    fig, ax = plt.subplots(figsize=(10, 5), constrained_layout=True)
    x = np.arange(len(datasets))
    for offset, mode, label, color in (
        (-0.25, "larch", "Larch", "#264b70"),
        (0, "fixed_0.001_cached", "Rexafs λ=0.001, cached", "#24936e"),
        (0.25, "fixed_0.001_uncached", "Rexafs λ=0.001, uncached", "#8abf9e"),
    ):
        values = [
            next(
                r["background_ms"]["median"]
                for r in current["rows"]
                if r["dataset"] == d and r["setup"] == "standard" and r["mode"] == mode
            )
            for d in datasets
        ]
        ax.bar(x + offset, values, width=0.24, label=label, color=color)
    ax.set(
        yscale="log",
        ylabel="Median AUTOBK time (ms, log scale)",
        xticks=x,
        xticklabels=labels,
        title="Mac measurements under background load — standard grid\n140 fresh fits per cell; cached and uncached geometry shown separately",
    )
    ax.legend()
    ax.grid(axis="y", alpha=0.2)
    for suffix in ("png", "svg"):
        fig.savefig(
            out / f"speed.{suffix}",
            dpi=180,
            metadata={"Date": None} if suffix == "svg" else None,
        )
    plt.close(fig)
    for path in out.glob("*.svg"):
        path.write_text(
            "\n".join(line.rstrip() for line in path.read_text().splitlines()).rstrip()
            + "\n"
        )
    a.close()
    b.close()


if __name__ == "__main__":
    main()
