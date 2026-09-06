#!/usr/bin/env python3
"""Controlled numerical ablations, separate from the public-API timing matrix."""

import argparse
import importlib
import importlib.util
import json
from pathlib import Path

spec = importlib.util.spec_from_file_location(
    "benchmark", Path(__file__).with_name("benchmark-larch.py")
)
bench = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bench)

import numpy as np
from larch.xafs.xafsft import ftwindow, xftf_fast
from scipy.interpolate import splev

autobk_module = importlib.import_module("larch.xafs.autobk")


def relative_l2(actual, expected):
    assert actual.shape == expected.shape
    assert np.isfinite(actual).all() and np.isfinite(expected).all()
    return float(np.linalg.norm(actual - expected) / np.linalg.norm(expected))


def linear_resampling(kraw, mu, knots, coefs, order, kout):
    """Diagnostic only: evaluate the same affine residual used by rexafs."""
    background = splev(kraw, (knots, coefs, order))
    chi = np.interp(kout, kraw, mu) - splev(kout, (knots, coefs, order))
    return background, chi


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("results", type=Path)
    args = parser.parse_args()
    rows = []
    original_spline_eval = autobk_module.spline_eval
    for dataset in ("cu", "ni", "ru"):
        energy, mu, _ = bench.load_input(dataset)
        # Keep k range identical. At kmax=12, rbkg=1.05 in Larch selects
        # nspl=9 and irbkg=35, matching rexafs' rounded choices at rbkg=1.
        # This deliberately changes the Larch input solely as a diagnostic;
        # it is not one of the matched-settings performance comparisons.
        rx = np.load(args.results / f"{dataset}--no_clamp--direct_cached.npz")
        mask = (rx["k"] >= 2) & (rx["k"] <= 12)
        for label, rbkg, linear in (
            ("same_settings", 1.0, False),
            ("matched_knots_and_cutoff", 1.05, False),
            ("matched_knots_cutoff_and_resampling", 1.05, True),
        ):
            params = bench.parameters(energy, mu, "no_clamp")
            params["rbkg"] = rbkg
            pipeline = bench.Pipeline(energy, mu, params, "larch")
            try:
                if linear:
                    autobk_module.spline_eval = linear_resampling
                output = pipeline.run()
            finally:
                autobk_module.spline_eval = original_spline_eval
            np.testing.assert_allclose(output.k, rx["k"], rtol=0, atol=1e-12)
            rows.append(
                {
                    "dataset": dataset,
                    "diagnostic": label,
                    "larch_rbkg": rbkg,
                    "larch_nknots": output.autobk_details.nknots,
                    "larch_irbkg": output.autobk_details.irbkg,
                    "chi_relative_l2_vs_rexafs_no_clamp": relative_l2(
                        output.chi[mask], rx["chi"][mask]
                    ),
                }
            )
            np.savez_compressed(
                args.results / f"diagnostic--{dataset}--{label}.npz",
                k=output.k,
                chi=output.chi,
            )

        data = np.load(args.results / f"{dataset}--standard--shipped_default.npz")
        # Larch xftf_prep builds its window on an extended grid, while rexafs
        # builds on the supplied grid. Hold chi and the FFT kernel constant,
        # and change only the window's construction domain.
        window = ftwindow(data["k"], xmin=2, xmax=12, dx=1, dx2=1, window="kaiser")
        reference = xftf_fast(data["chi"] * data["k"] ** 2 * window, 2048, 0.05)
        actual = data["chir_real"] + 1j * data["chir_imag"]
        original = data["same_chi_larch_real"] + 1j * data["same_chi_larch_imag"]
        rows.append(
            {
                "dataset": dataset,
                "diagnostic": "identical_chi_fft_window_domain",
                "public_xftf_relative_l2": relative_l2(actual, original),
                "matched_window_domain_relative_l2": relative_l2(
                    actual, reference[: len(actual)]
                ),
            }
        )
    (args.results / "parity-diagnostics.json").write_text(
        json.dumps(rows, indent=2) + "\n"
    )
    print(json.dumps(rows, indent=2))


if __name__ == "__main__":
    main()
