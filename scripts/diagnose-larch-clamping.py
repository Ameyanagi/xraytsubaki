#!/usr/bin/env python3
"""Isolate AUTOBK clamp choices; modified Larch calls are not speed benchmarks."""

import argparse
import hashlib
import importlib
import importlib.util
import itertools
import json
from pathlib import Path

spec = importlib.util.spec_from_file_location(
    "benchmark", Path(__file__).with_name("benchmark-larch.py")
)
if spec is None or spec.loader is None:
    raise RuntimeError("Cannot load the adjacent benchmark-larch.py harness")
bench = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bench)

import numpy as np
from scipy.interpolate import splev

autobk = importlib.import_module("larch.xafs.autobk")


def plot_comparison(output):
    import matplotlib

    matplotlib.use("Agg")
    matplotlib.rcParams["svg.hashsalt"] = "rexafs-clamping-diagnostics"
    import matplotlib.pyplot as plt

    figure, axes = plt.subplots(3, 2, figsize=(12, 9), constrained_layout=True)
    for row, dataset in enumerate(("cu", "ni", "ru")):
        with np.load(output / f"{dataset}.npz") as data:
            visible = (data["k"] >= 2) & (data["k"] <= 12)
            for column, enabled in enumerate((True, False)):
                ax = axes[row, column]
                for config, label, color, style in (
                    ("larch", "Larch", "black", "-"),
                    ("shipped_default", "rexafs fixed", "#D55E00", "-"),
                    ("direct_two_pass", "rexafs two-pass", "#009E73", "--"),
                ):
                    key = config if enabled else f"{config}_no_clamp"
                    if key == "larch":
                        key = "larch_standard"
                    ax.plot(
                        data["k"][visible],
                        data[key][visible],
                        label=label,
                        color=color,
                        ls=style,
                        lw=1.2,
                    )
                ax.set(
                    xlim=(2, 12),
                    xlabel="k (Å⁻¹)",
                    ylabel="χ(k)",
                    title=f"{dataset.upper()}: {'standard clamps' if enabled else 'clamps off in both packages'}",
                )
                ax.grid(alpha=0.2)
        # Same vertical scale for each dataset's on/off comparison.
        limits = [ax.get_ylim() for ax in axes[row]]
        for ax in axes[row]:
            ax.set_ylim(min(v[0] for v in limits), max(v[1] for v in limits))
    axes[0, 0].legend(fontsize=9)
    figure.suptitle(
        "Clamping changes the fit, but the Ru difference remains without it",
        fontsize=14,
    )
    figure.savefig(output / "clamps-on-off.png", dpi=180)
    figure.savefig(output / "clamps-on-off.svg", metadata={"Date": None})
    plt.close(figure)
    svg = output / "clamps-on-off.svg"
    svg.write_text(
        "\n".join(line.rstrip() for line in svg.read_text().splitlines()) + "\n"
    )


def linear_resampling(kraw, mu, knots, coefs, order, kout):
    return (
        splev(kraw, (knots, coefs, order)),
        np.interp(kout, kraw, mu) - splev(kout, (knots, coefs, order)),
    )


def relative_l2(actual, reference):
    if actual.shape != reference.shape:
        raise ValueError("Comparison arrays must have the same shape")
    if not np.isfinite(actual).all() or not np.isfinite(reference).all():
        raise ValueError("Non-finite comparison array")
    denominator = np.linalg.norm(reference)
    if not np.isfinite(denominator) or denominator <= 0:
        raise ValueError("Reference norm must be finite and positive")
    result = np.linalg.norm(actual - reference) / denominator
    if not np.isfinite(result):
        raise ValueError("Relative L2 calculation overflowed")
    return float(result)


def diagnostic(energy, mu, model, multiplier, shift, policy, enabled=True):
    """Hold MINPACK/settings fixed; vary model, scale, endpoints and update policy."""
    original = (autobk._resid, autobk.spline_eval, autobk.leastsq)
    details = {"passes": []}
    frozen = None

    def parts(
        vcoefs,
        ncoef,
        kraw,
        mu,
        chi_std,
        knots,
        order,
        kout,
        ftwin,
        nfft,
        irbkg,
        nclamp,
        clamp_lo,
        clamp_hi,
    ):
        coefs = np.full(ncoef, vcoefs[-1])
        coefs[: len(vcoefs)] = vcoefs
        _, chi = autobk.spline_eval(kraw, mu, knots, coefs, order, kout)
        if chi_std is not None:
            chi = chi - chi_std
        head = autobk.realimag(autobk.xftf_fast(chi * ftwin, nfft=nfft)[:irbkg])
        scale = multiplier * (0.1 + 10 * np.mean(head**2))
        high = chi[len(chi) - nclamp - shift : len(chi) - shift]
        ends = np.concatenate((abs(clamp_lo) * chi[:nclamp], abs(clamp_hi) * high))
        return head, ends, scale

    def residual(coefs, *args):
        head, ends, scale = parts(coefs, *args)
        if not enabled:
            return head
        return np.concatenate((head, (scale if frozen is None else frozen) * ends))

    def solve(function, initial, args, **kwargs):
        nonlocal frozen
        details["initial_scale"] = parts(initial, *args)[2]
        if policy != "dynamic":
            frozen = details["initial_scale"]
        result = original[2](function, initial, args, **kwargs)
        details["passes"].append({"status": int(result[4]), "nfev": result[2]["nfev"]})
        if result[4] not in (1, 2, 3, 4):
            raise RuntimeError(f"Diagnostic fit failed: {result[3]}")
        if policy == "two_pass":
            frozen = parts(result[0], *args)[2]
            result = original[2](function, result[0], args, **kwargs)
            details["passes"].append(
                {"status": int(result[4]), "nfev": result[2]["nfev"]}
            )
            if result[4] not in (1, 2, 3, 4):
                raise RuntimeError(f"Second diagnostic fit failed: {result[3]}")
        head, ends, final_scale = parts(result[0], *args)
        used_scale = final_scale if frozen is None else frozen
        details.update(
            final_dynamic_scale=final_scale,
            used_scale=used_scale,
            low_r_loss=float(head @ head),
            endpoint_loss=float((used_scale * ends) @ (used_scale * ends))
            if enabled
            else 0.0,
        )
        return result

    params = bench.parameters(energy, mu, "standard")
    # Same kmax. rbkg=1.05 only forces Larch's counts to rexafs' 9/35.
    params["rbkg"] = 1.05 if model == "aligned" else 1.0
    params["nclamp"] = 3 if enabled else 0
    try:
        autobk._resid = residual
        autobk.leastsq = solve
        if model == "aligned":
            autobk.spline_eval = linear_resampling
        result = bench.Pipeline(energy, mu, params, "larch").run()
    finally:
        autobk._resid, autobk.spline_eval, autobk.leastsq = original
    details.update(
        nknots=result.autobk_details.nknots,
        irbkg=result.autobk_details.irbkg,
        edge_step=float(result.edge_step),
    )
    return result.k, result.chi, details


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("baseline", type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    rows, public_rows, sources = [], [], {}
    for dataset in ("cu", "ni", "ru"):
        energy, mu, sources[dataset] = bench.load_input(dataset)
        with np.load(args.baseline / f"{dataset}--standard--larch.npz") as data:
            k, reference = data["k"], data["chi"]
        mask = (k >= 2) & (k <= 12)
        arrays = {"k": k, "larch_standard": reference}
        rx = {}
        for config in ("shipped_default", "direct_two_pass", "legacy_lm", "dogleg"):
            with np.load(args.baseline / f"{dataset}--standard--{config}.npz") as data:
                rx[config] = data["chi"]
                arrays[config] = data["chi"]
        # Public APIs: turn clamps off in BOTH packages, with one consistent
        # reference denominator per row, plus each package's own clamp effect.
        for config in ("larch", *rx):
            with np.load(args.baseline / f"{dataset}--no_clamp--{config}.npz") as data:
                no_clamp = data["chi"]
            with np.load(args.baseline / f"{dataset}--no_clamp--larch.npz") as data:
                larch_no_clamp = data["chi"]
            standard = reference if config == "larch" else rx[config]
            arrays[f"{config}_no_clamp"] = no_clamp
            public_rows.append(
                {
                    "dataset": dataset,
                    "config": config,
                    "standard_vs_standard_larch": relative_l2(
                        standard[mask], reference[mask]
                    ),
                    "no_clamp_vs_no_clamp_larch": relative_l2(
                        no_clamp[mask], larch_no_clamp[mask]
                    ),
                    "clamp_effect_vs_own_no_clamp": relative_l2(
                        standard[mask], no_clamp[mask]
                    ),
                    "clamp_effect_fixed_larch_denominator": float(
                        np.linalg.norm((standard - no_clamp)[mask])
                        / np.linalg.norm(reference[mask])
                    ),
                }
            )
        cases = [
            (m, a, s, p, True)
            for m, a, s, p in itertools.product(
                ("native", "aligned"), (1, 10), (0, 1), ("dynamic", "fixed", "two_pass")
            )
        ]
        cases += [(m, 1, 0, "dynamic", False) for m in ("native", "aligned")]
        for model, multiplier, shift, policy, enabled in cases:
            label = (
                f"{model}--x{multiplier}--shift{shift}--{policy}"
                if enabled
                else f"{model}--no_clamp"
            )
            actual_k, chi, details = diagnostic(
                energy, mu, model, multiplier, shift, policy, enabled
            )
            np.testing.assert_allclose(actual_k, k, rtol=0, atol=1e-12)
            arrays[label] = chi
            rows.append(
                {
                    "dataset": dataset,
                    "case": label,
                    "model": model,
                    "scale_multiplier": multiplier,
                    "endpoint_shift": shift,
                    "policy": policy,
                    "enabled": enabled,
                    **details,
                    "relative_l2_vs_standard_larch": relative_l2(
                        chi[mask], reference[mask]
                    ),
                    "relative_l2_vs_rexafs": {
                        c: relative_l2(chi[mask], value[mask])
                        for c, value in rx.items()
                    },
                }
            )
        # Verify the diagnostic machinery reproduces the unmodified package.
        np.testing.assert_allclose(
            arrays["native--x1--shift0--dynamic"], reference, rtol=1e-10, atol=1e-12
        )
        np.savez_compressed(args.output / f"{dataset}.npz", **arrays)
    report = {
        "purpose": "Controlled clamp diagnostics, not performance benchmarks or an accuracy ranking",
        "script_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "benchmark_script_sha256": hashlib.sha256(
            Path(bench.__file__).read_bytes()
        ).hexdigest(),
        "larch_autobk_sha256": hashlib.sha256(
            Path(autobk.__file__).read_bytes()
        ).hexdigest(),
        "versions": {
            p: bench.importlib.metadata.version(p)
            for p in ("rexafs", "xraylarch", "numpy", "scipy")
        },
        "sources": sources,
        "public_api": public_rows,
        "diagnostics": rows,
    }
    (args.output / "clamping.json").write_text(json.dumps(report, indent=2) + "\n")
    plot_comparison(args.output)
    print(
        f"Wrote {len(rows)} controlled fits and {len(public_rows)} public comparisons"
    )


if __name__ == "__main__":
    main()
