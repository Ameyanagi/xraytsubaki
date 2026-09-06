#!/usr/bin/env python3
"""Experimental AUTOBK clamp study; does not change either package's defaults.

Use the pinned Larch benchmark environment plus the local rexafs candidate wheel.
All experimental penalties share Larch's exact spline/interpolation/FFT model.
Ground truth is used for scoring and training-set selection, never by a fit.
"""

from __future__ import annotations

import argparse
import contextlib
import hashlib
import importlib
import importlib.util
import itertools
import json
import time
from pathlib import Path

spec = importlib.util.spec_from_file_location(
    "benchmark", Path(__file__).with_name("benchmark-larch.py")
)
if spec is None or spec.loader is None:
    raise RuntimeError("Cannot import benchmark-larch.py")
bench = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bench)

import numpy as np
from scipy.interpolate import CubicSpline
from scipy.optimize import leastsq
from threadpoolctl import threadpool_limits

ab = importlib.import_module("larch.xafs.autobk")
LAMBDAS = (0.0, 1e-6, 1e-5, 1e-4, 1e-3, 1e-2, 1e-1, 1.0)
ROOT = Path(__file__).resolve().parents[1]


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def fixed_name(value):
    return f"fixed_{value:g}"


def rmse(values):
    return float(np.sqrt(np.mean(np.square(values))))


def relative(actual, reference):
    if actual.shape != reference.shape:
        raise ValueError("Mismatched comparison shapes")
    if not np.isfinite(actual).all() or not np.isfinite(reference).all():
        raise ValueError("Non-finite comparison")
    norm = np.linalg.norm(reference)
    if not np.isfinite(norm) or norm <= 0:
        raise ValueError("Invalid reference norm")
    value = float(np.linalg.norm(actual - reference) / norm)
    if not np.isfinite(value):
        raise ValueError("Comparison overflow")
    return value


class Captured(Exception):
    pass


@contextlib.contextmanager
def replace_solver(function):
    # Sequential, process-local use only. Never leave Larch patched on failure.
    original = ab.leastsq
    ab.leastsq = function
    try:
        yield
    finally:
        ab.leastsq = original


def call_larch(energy, mu, p, step):
    group = bench.larch.Group()
    ab.autobk(
        energy,
        mu,
        group=group,
        edge_step=step,
        win="hanning",
        calc_uncertainties=False,
        **{
            key: p[key]
            for key in (
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
    return group


class Problem:
    """Affine Larch model chi = y + J c, with edge-step-normalized coefficients."""

    def __init__(self, energy, mu, p, step):
        if not np.isfinite(step) or step <= 0:
            raise ValueError("Need a positive finite edge step")
        captured = {}

        def capture(function, initial, args, **kwargs):
            captured.update(initial=initial.copy(), args=args, options=kwargs)
            raise Captured

        with replace_solver(capture):
            try:
                call_larch(energy, mu, p, step)
            except Captured:
                pass
        if not captured:
            raise RuntimeError("Larch did not supply a spline problem")
        self.args = captured["args"]
        self.initial = captured["initial"] / step
        self.options = captured["options"]
        self.step, self.p = step, p
        (
            self.ncoef,
            self.kraw,
            self.mu,
            standard,
            self.knots,
            self.order,
            self.k,
            self.window,
            self.nfft,
            self.irbkg,
            self.nclamp,
            self.lo,
            self.hi,
        ) = self.args
        if standard is not None or self.nclamp != 3:
            raise ValueError("Study expects no chi standard and three clamp points")
        zero = np.zeros(self.ncoef)
        self.y = (
            ab.spline_eval(self.kraw, self.mu, self.knots, zero, self.order, self.k)[1]
            / step
        )
        columns, raw_columns = [], []
        for index in range(len(self.initial)):
            coefs = zero.copy()
            coefs[index] = 1
            if index == len(self.initial) - 1:
                coefs[len(self.initial) :] = 1
            raw, chi = ab.spline_eval(
                self.kraw,
                np.zeros_like(self.mu),
                self.knots,
                coefs,
                self.order,
                self.k,
            )
            columns.append(chi)
            raw_columns.append(raw)
        self.j = np.column_stack(columns)
        self.raw_basis = np.column_stack(raw_columns)
        self.h = self.head(self.y)
        self.hj = np.column_stack([self.head(col) for col in columns])
        self.e, self.ej = self.ends(self.y), self.ends(self.j)
        self.initial_scale = self.dynamic_scale(self.initial)

    def head(self, chi):
        # Match _resid exactly: Larch's call uses xftf_fast's default kstep.
        return ab.realimag(
            ab.xftf_fast(chi * self.window, nfft=self.nfft)[: self.irbkg]
        )

    def ends(self, chi):
        return np.concatenate(
            (abs(self.lo) * chi[: self.nclamp], abs(self.hi) * chi[-self.nclamp :])
        )

    def dynamic_scale(self, coefs):
        raw_head = self.step * (self.h + self.hj @ coefs)
        return float(0.1 + 10 * np.mean(raw_head**2))

    def dynamic_residual(self, coefs):
        head, ends = self.h + self.hj @ coefs, self.e + self.ej @ coefs
        return np.concatenate((head, self.dynamic_scale(coefs) * ends))

    def dynamic_jacobian(self, coefs):
        head, ends = self.h + self.hj @ coefs, self.e + self.ej @ coefs
        gradient = 20 * self.step**2 * (head @ self.hj) / len(head)
        return np.vstack(
            (self.hj, self.dynamic_scale(coefs) * self.ej + np.outer(ends, gradient))
        )

    def fixed(self, penalty=None, scale=None, chi_data=None):
        if (penalty is None) == (scale is None):
            raise ValueError("Specify exactly one fixed penalty or residual scale")
        if penalty is not None:
            if not np.isfinite(penalty) or penalty < 0:
                raise ValueError("Penalty must be finite and nonnegative")
            # mean(low-R real/imag residual squared) + lambda * mean(high-end chi squared).
            # Include only active endpoint rows in the mean (lo=0 in this study).
            active = self.nclamp * (int(self.lo != 0) + int(self.hi != 0))
            scale = np.sqrt(penalty * len(self.h) / active)
        y = self.y if chi_data is None else np.asarray(chi_data)
        if y.shape != self.y.shape or not np.isfinite(y).all():
            raise ValueError("Invalid affine input")
        head = self.h if chi_data is None else self.head(y)
        ends = self.e if chi_data is None else self.ends(y)
        matrix = np.vstack((self.hj, scale * self.ej))
        rhs = -np.concatenate((head, scale * ends))
        column_norms = np.linalg.norm(matrix, axis=0)
        if np.any(column_norms == 0):
            raise RuntimeError("Zero design column")
        scaled = matrix / column_norms
        # Exactly one SVD least-squares call; no ridge search or nonlinear fallback.
        solved, _, rank, singular = np.linalg.lstsq(scaled, rhs, rcond=None)
        if rank != matrix.shape[1]:
            raise RuntimeError("Rank-deficient fixed-penalty problem")
        coefs = solved / column_norms
        residual = matrix @ coefs - rhs
        stationarity = np.linalg.norm(scaled.T @ residual)
        if not np.isfinite(coefs).all() or stationarity > 1e-9 * max(
            1, np.linalg.norm(rhs)
        ):
            raise RuntimeError("Invalid direct solution")
        return y + self.j @ coefs, {
            "coefficients": coefs.tolist(),
            "solves": 1,
            "condition": float(singular[0] / singular[-1]),
            "stationarity": float(stationarity),
            "scale": float(scale),
        }


def larch_reference(energy, mu, p, step):
    details = {}

    def logged(function, initial, args, **kwargs):
        begin = time.perf_counter_ns()
        result = leastsq(function, initial, args, **kwargs)
        details.update(
            status=int(result[4]),
            nfev=int(result[2]["nfev"]),
            optimizer_ms=(time.perf_counter_ns() - begin) / 1e6,
        )
        if result[4] not in (1, 2, 3, 4):
            raise RuntimeError(f"Larch failed: {result[3]}")
        return result

    with replace_solver(logged):
        begin = time.perf_counter_ns()
        result = call_larch(energy, mu, p, step)
        details["background_ms"] = (time.perf_counter_ns() - begin) / 1e6
    return result, details


def rexafs_direct(energy, mu, p, step):
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
    _, background, _ = bench.configure_rexafs(p, "direct_cached")
    spectrum = (
        bench.rexafs.Spectrum(energy, mu)
        .set_e0(p["e0"])
        .set_normalization_method(bench.rexafs.NormalizationMethod.PrePostEdge(norm))
        .set_background_method(background)
    )
    spectrum.normalize()
    begin = time.perf_counter_ns()
    spectrum.calc_background()
    elapsed = (time.perf_counter_ns() - begin) / 1e6
    return (
        np.asarray(spectrum.k()),
        np.asarray(spectrum.chi()),
        {"background_ms": elapsed},
    )


def validate_problem(problem, reference):
    # Independently check the affine model and the full scale derivative against
    # Larch residual evaluations away from the solution, where the missing term matters.
    coefs = problem.initial + np.linspace(-0.07, 0.09, len(problem.initial))
    np.testing.assert_allclose(
        problem.dynamic_residual(coefs),
        ab._resid(coefs * problem.step, *problem.args) / problem.step,
        rtol=2e-10,
        atol=2e-11,
    )
    epsilon = 1e-5
    numeric = np.column_stack(
        [
            (
                problem.dynamic_residual(coefs + epsilon * direction)
                - problem.dynamic_residual(coefs - epsilon * direction)
            )
            / (2 * epsilon)
            for direction in np.eye(len(coefs))
        ]
    )
    jac_error = relative(problem.dynamic_jacobian(coefs), numeric)
    if jac_error > 1e-7:
        raise RuntimeError(f"Dynamic Jacobian mismatch: {jac_error}")
    model_chi = problem.y + problem.j @ (
        reference.autobk_details.coefs[: len(coefs)] / problem.step
    )
    np.testing.assert_allclose(model_chi, reference.chi, rtol=2e-10, atol=2e-11)
    # Independent iterative solve of a FIXED quadratic checks the direct result.
    direct, _ = problem.fixed(penalty=1e-3)
    scale = np.sqrt(1e-3 * len(problem.h) / problem.nclamp)
    matrix = np.vstack((problem.hj, scale * problem.ej))
    offset = np.concatenate((problem.h, scale * problem.e))
    fit = leastsq(
        lambda c: offset + matrix @ c,
        problem.initial,
        Dfun=lambda _: matrix,
        full_output=True,
        ftol=1e-12,
        xtol=1e-12,
    )
    if fit[4] not in (1, 2, 3, 4):
        raise RuntimeError("Fixed-objective validation failed")
    np.testing.assert_allclose(
        direct, problem.y + problem.j @ fit[0], rtol=1e-8, atol=1e-10
    )
    return {
        "dynamic_jacobian_relative_l2": jac_error,
        "fixed_direct_vs_lm": "passed",
        "affine_vs_larch_residual": "passed",
        "affine_vs_larch_output": "passed",
    }


def scale_checks(energy, mu, p, step):
    """Change absorption units and offset without changing normalized truth."""
    original = Problem(energy, mu, p, step)
    fixed, _ = original.fixed(penalty=1e-3)
    larch, _ = larch_reference(energy, mu, p, step)
    rows = []
    mask = (original.k >= 2) & (original.k <= p["kmax"])
    for gain, offset in ((0.1, 0.0), (10.0, 0.0), (1.0, 2.0)):
        problem = Problem(energy, gain * mu + offset, p, gain * step)
        actual, _ = problem.fixed(penalty=1e-3)
        reference, _ = larch_reference(energy, gain * mu + offset, p, gain * step)
        error = relative(actual[mask], fixed[mask])
        if error > 1e-8:
            raise RuntimeError(f"Fixed penalty depends on absorption units: {error}")
        rows.append(
            {
                "gain": gain,
                "offset": offset,
                "fixed_relative_change": error,
                "larch_relative_change": relative(reference.chi[mask], larch.chi[mask]),
            }
        )
    return rows


FEFF_PATH = bench.DATA / "feff_ff2chi_larch_ref.txt"
_feff = np.loadtxt(FEFF_PATH)
FEFF_SIGNAL = CubicSpline(_feff[:, 0], _feff[:, 1])


def synthetic_functions(split, signal, background, amplitude):
    held = split == "test"

    def chi(k):
        turn_on = 1 - np.exp(-(k**2) / 0.6**2)
        if signal == "feff":
            # A retained FEFF-derived signal, never part of penalty selection.
            wave = FEFF_SIGNAL(np.clip(k, _feff[0, 0], _feff[-1, 0]))
        else:
            radius = (
                (2.37 if held else 2.15)
                if signal == "single"
                else (1.72 if held else 1.9)
            )
            phase = 0.9 if held else 0.1
            wave = (
                0.28 * np.sin(2 * radius * k + phase) * np.exp(-0.006 * k**2) / (k + 1)
            )
            if signal == "double":
                wave += (
                    0.17
                    * np.sin(2 * (3.25 if held else 3.0) * k + 1.3)
                    * np.exp(-0.01 * k**2)
                    / (k + 1)
                )
        return amplitude * turn_on * wave

    def bkg(k):
        x = k / 12
        value = 1 + 0.12 * x**2 - 0.035 * x**4
        if background == "bump":
            value += (0.11 if held else 0.08) * np.exp(
                -0.5 * ((k - (5.5 if held else 4.5)) / 2.0) ** 2
            )
        return value

    return chi, bkg


def synthetic_cases(quick=False):
    for split in ("train", "test"):
        signals = (
            ("single", "double") if split == "train" else ("single", "double", "feff")
        )
        for signal, bg, amplitude, noise, kmax in itertools.product(
            signals,
            ("polynomial", "bump"),
            (0.3, 1.0),
            (0.0, 0.0003, 0.003),
            (10.0, 12.0),
        ):
            for repeat in range(1 if noise == 0 or quick else 3):
                seed = (1000 if split == "train" else 5000) + repeat
                chi, bkg = synthetic_functions(split, signal, bg, amplitude)
                kraw = np.concatenate(
                    ([0.0], np.arange(0.02, 1, 0.02), np.arange(1.0, 16.01, 0.04))
                )
                energy = np.concatenate(
                    (np.arange(8800.0, 9000.0, 1.0), 9000 + kraw**2 / ab.ETOK)
                )
                mu = 0.2 + 0.0001 * (energy - 9000)
                above = energy >= 9000
                mu[above] += bkg(kraw) + chi(kraw)
                rng = np.random.default_rng(seed)
                mu += noise * rng.standard_normal(len(mu))
                p = bench.parameters(energy, mu, "standard")
                p.update(e0=9000.0, kmax=kmax)
                name = f"{split}-{signal}-{bg}-a{amplitude:g}-noise{noise:g}-k{kmax:g}-r{repeat}"
                meta = {
                    "split": split,
                    "signal": signal,
                    "background": bg,
                    "amplitude": amplitude,
                    "noise_sigma": noise,
                    "seed": seed,
                    "kmax": kmax,
                    "kind": "synthetic",
                }

                def truth(k, chi=chi, bkg=bkg):
                    # Includes the known pre-edge baseline in the raw background.
                    return chi(k), 0.2 + 0.0001 * k**2 / ab.ETOK + bkg(k)

                yield name, energy, mu, p, 1.0, truth, meta


def measured_cases():
    for dataset in ("cu", "ni", "ru"):
        energy, mu, source = bench.load_input(dataset)
        for setup, kmax in itertools.product(("standard", "fine_fft"), (10.0, 12.0)):
            p = bench.parameters(energy, mu, setup)
            p["kmax"] = kmax
            pipeline = bench.Pipeline(energy, mu, p, "larch")
            group = pipeline.create()
            pipeline.normalize(group)
            yield (
                f"{dataset}-{setup}-k{kmax:g}",
                energy,
                mu,
                p,
                float(group.edge_step),
                None,
                {
                    "kind": "measured",
                    "dataset": dataset,
                    "setup": setup,
                    "kmax": kmax,
                    "source": source,
                },
            )


def stress_cases():
    """Stronger-clamp sensitivity, excluded from penalty selection.

    Added after the primary pilot selected lambda=0. Paired inputs are reused
    so only high-clamp strength changes; these are labeled separately.
    """
    inputs = [
        case
        for case in synthetic_cases(quick=True)
        if case[-1]["split"] == "test"
        and case[-1]["amplitude"] == 1.0
        and case[-1]["noise_sigma"] in (0.0, 0.003)
        and case[-1]["kmax"] == 12
    ]
    inputs += [
        case
        for case in measured_cases()
        if case[-1]["setup"] == "standard" and case[-1]["kmax"] == 12
    ]
    for name, energy, mu, p, step, truth, meta in inputs:
        for strength in (10, 50):
            stress = dict(
                meta,
                split="stress",
                clamp_hi=strength,
                original_split=meta.get("split"),
            )
            yield (
                f"stress-hi{strength}-{name}",
                energy,
                mu,
                dict(p, clamp_hi=strength),
                step,
                truth,
                stress,
            )


def scores(k, actual, reference, p):
    mask = (k >= 2) & (k <= p["kmax"])
    end = (k >= p["kmax"] - 1) & (k <= p["kmax"])
    delta = actual - reference
    transform = lambda chi: ab.xftf_fast(
        chi
        * k**2
        * ab.ftwindow(k, xmin=2, xmax=p["kmax"], dx=1, dx2=1, window="hanning"),
        nfft=p["nfft"],
        kstep=p["kstep"],
    )
    r = np.arange(p["nfft"] // 2) * np.pi / (p["nfft"] * p["kstep"])
    rmask = (r >= 1) & (r <= 4)
    actual_ft, reference_ft = transform(actual), transform(reference)
    return {
        "chi_relative_l2": relative(actual[mask], reference[mask]),
        "chi_rmse": rmse(delta[mask]),
        "signal_projection_gain": float(
            actual[mask] @ reference[mask] / (reference[mask] @ reference[mask])
        ),
        "endpoint_rmse": rmse(delta[end]),
        "chi_k2_relative_l2": relative((actual * k**2)[mask], (reference * k**2)[mask]),
        "r_1_4_complex_relative_l2": relative(actual_ft[rmask], reference_ft[rmask]),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--quick", action="store_true")
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    rows, checks, invariance = [], [], []
    with threadpool_limits(limits=1):
        environment = bench.environment()
        environment.update(
            study_source_sha256=sha(Path(__file__)),
            background_rs_sha256=sha(ROOT / "crates/rexafs/src/xafs/background.rs"),
            feff_signal_sha256=sha(FEFF_PATH),
            candidate="local 0.1.1 wheel with automatic knot-count floor; clamp code unchanged",
        )
        cases = (
            list(synthetic_cases(args.quick))
            + list(measured_cases())
            + list(stress_cases())
        )
        for index, (name, energy, mu, p, step, truth, meta) in enumerate(cases):
            print(f"[{index + 1}/{len(cases)}] {name}", flush=True)
            begin = time.perf_counter_ns()
            problem = Problem(energy, mu, p, step)
            preparation_ms = (time.perf_counter_ns() - begin) / 1e6
            reference, ref_details = larch_reference(energy, mu, p, step)
            np.testing.assert_allclose(problem.k, reference.k, rtol=0, atol=1e-12)
            if index == 0 or meta["kind"] == "measured":
                checks.append({"case": name, **validate_problem(problem, reference)})
            if index == 0 or (
                meta["kind"] == "measured"
                and meta["setup"] == "standard"
                and p["kmax"] == 12
            ):
                invariance.append(
                    {"case": name, "checks": scale_checks(energy, mu, p, step)}
                )
            arrays = {
                "energy": energy,
                "mu": mu,
                "k": problem.k,
                "larch_dynamic": reference.chi,
            }
            methods = {"larch_dynamic": (reference.chi, ref_details)}
            for penalty in LAMBDAS:
                begin = time.perf_counter_ns()
                chi, details = problem.fixed(penalty=penalty)
                details["solve_ms"] = (time.perf_counter_ns() - begin) / 1e6
                methods[fixed_name(penalty)] = chi, details
            methods["frozen_initial"] = problem.fixed(scale=problem.initial_scale)
            rk, rchi, details = rexafs_direct(energy, mu, p, step)
            np.testing.assert_allclose(rk, problem.k, rtol=0, atol=1e-12)
            methods["rexafs_direct"] = rchi, details
            if truth:
                true_chi, _ = truth(problem.k)
                arrays["true_chi"] = true_chi
                raw_chi, true_background = truth(problem.kraw)
                arrays.update(
                    kraw=problem.kraw,
                    true_background=true_background,
                    raw_true_chi=raw_chi,
                )
            else:
                true_chi = reference.chi
            case_rows = []
            for method, (chi, details) in methods.items():
                if not np.isfinite(chi).all():
                    raise RuntimeError(f"Non-finite output: {name} {method}")
                arrays[method] = chi
                row = dict(
                    case=name,
                    method=method,
                    **meta,
                    **scores(problem.k, chi, true_chi, p),
                    details=details,
                )
                if truth and "coefficients" in details:
                    background = step * problem.raw_basis @ details["coefficients"]
                    rawmask = (problem.kraw >= 2) & (problem.kraw <= p["kmax"])
                    row["background_rmse"] = rmse(
                        (background - true_background)[rawmask] / step
                    )
                elif truth and method == "larch_dynamic":
                    d = reference.autobk_details
                    background = reference.bkg[d.iek0 : d.iemax + 1]
                    rawmask = (problem.kraw >= 2) & (problem.kraw <= p["kmax"])
                    row["background_rmse"] = rmse(
                        (background - true_background)[rawmask] / step
                    )
                rows.append(row)
                case_rows.append(row)
            np.savez_compressed(args.output / f"{name}.npz", **arrays)
            (args.output / f"{name}.json").write_text(
                json.dumps(
                    {
                        "parameters": p,
                        "edge_step": step,
                        "metadata": meta,
                        "initial_scale": problem.initial_scale,
                        "final_larch_scale": problem.dynamic_scale(
                            reference.autobk_details.coefs[: len(problem.initial)]
                            / step
                        ),
                        "nknots": len(problem.initial),
                        "irbkg": problem.irbkg,
                        "model_preparation_ms": preparation_ms,
                        "rows": case_rows,
                    },
                    indent=2,
                )
                + "\n"
            )
        train = {
            fixed_name(value): float(
                np.mean(
                    [
                        row["chi_relative_l2"]
                        for row in rows
                        if row.get("split") == "train"
                        and row["method"] == fixed_name(value)
                    ]
                )
            )
            for value in LAMBDAS
        }
        selected = min(train, key=train.get)
        summary = {
            "environment": environment,
            "checks": checks,
            "absorption_unit_checks": invariance,
            "selection": {
                "criterion": "mean training chi relative L2 on k=2..kmax",
                "candidates": train,
                "selected": selected,
                "test_used_for_selection": False,
            },
            "case_count": len(cases),
            "rows": rows,
        }
        (args.output / "study.json").write_text(json.dumps(summary, indent=2) + "\n")
        print(json.dumps(summary["selection"], indent=2))


if __name__ == "__main__":
    main()
