#!/usr/bin/env python3
"""Regenerate Cu FEFF fit references for k-space, q-space and multi-k-weight fits.

These complement `generate_larch_fit_references.py` (R-space, single k-weight) and
are consumed by `tests/feff_larch_fitspace_parity.rs`.

Usage:
  uv run --project crates/xraytsubaki/tests/pythonscript \
      python crates/xraytsubaki/scripts/generate_larch_fitspace_references.py

All fits use the same Cu data, path and parameters as the R-space reference, but
pass an explicit scalar ``epsilon_k`` (the value recorded in ``metadata.json``) so
that Larch does not estimate the noise itself; this keeps the residual scaling
identical between Larch and the Rust implementation.
"""

from __future__ import annotations

import json
from datetime import datetime, timezone
from pathlib import Path

import numpy as np

import larch
from larch.fitting import guess, param, param_group
from larch.io import read_ascii
from larch.xafs import autobk, feffit, feffit_dataset, feffit_transform, feffpath

SOURCE_COMMIT = "d8678dd666fd95839fe9dc71b4dbe8bedec278ff"
CU_EPSILON_K = 0.0015509900716108595


def write_matrix(path: Path, header: str, columns: list[np.ndarray]) -> None:
    lens = {len(col) for col in columns}
    if len(lens) != 1:
        raise ValueError(f"length mismatch for {path}: {sorted(lens)}")
    np.savetxt(path, np.column_stack(columns), fmt="%.15e", header=header)


def collect_fit_params(out) -> dict[str, dict[str, float]]:
    params = {}
    for name in ("amp", "de0", "sig2", "dr"):
        p = out.params[name]
        params[name] = {"value": float(p.value), "stderr": float(p.stderr)}
    return params


def as_list(value) -> list[float]:
    if isinstance(value, (list, tuple, np.ndarray)):
        return [float(np.asarray(v, dtype=float).mean()) for v in value]
    return [float(np.asarray(value, dtype=float).mean())]


def make_params():
    return param_group(
        amp=param(1.0, vary=True),
        de0=guess(0.0),
        sig2=param(0.003, vary=True, min=0),
        dr=guess(0.0),
    )


def run_case(data, base: Path, name: str, **trans_kws) -> tuple[dict[str, object], object]:
    pars = make_params()
    path1 = feffpath(
        str(base / "feffit" / "Feff_Cu" / "feff0001.dat"),
        s02="amp",
        e0="de0",
        sigma2="sig2",
        deltar="dr",
    )
    trans = feffit_transform(
        kmin=3, kmax=16, dk=5, window="kaiser", rmin=1.4, rmax=3.0, **trans_kws
    )
    dset = feffit_dataset(data=data, transform=trans, pathlist=[path1], epsilon_k=CU_EPSILON_K)
    out = feffit(pars, dset)
    stats = {
        "fitspace": trans.fitspace,
        "kweights": as_list(trans.kweight),
        "chi_square": float(out.chi_square),
        "reduced_chi_square": float(out.chi2_reduced),
        "r_factor": float(out.rfactor),
        "n_idp": float(out.n_independent),
        "n_data": int(out.fit_details.ndata),
        "epsilon_k": as_list(dset.epsilon_k),
        "epsilon_r": as_list(dset.epsilon_r),
        "params": collect_fit_params(out),
    }
    print(name, json.dumps(stats, indent=1), flush=True)
    return stats, dset


def estimate_noise_reference(data, base: Path) -> dict[str, list[float]]:
    """Larch's high-R noise estimate (rmin=15, rmax=30) for kweights (1,2,3)."""
    path1 = feffpath(str(base / "feffit" / "Feff_Cu" / "feff0001.dat"))
    trans = feffit_transform(
        kmin=3, kmax=16, dk=5, window="kaiser", rmin=1.4, rmax=3.0, kweight=(1, 2, 3)
    )
    dset = feffit_dataset(data=data, transform=trans, pathlist=[path1])
    trans.make_karrays()
    ikmax = int(1.01 + max(data.k) / trans.kstep)
    chi = np.interp(trans.k_[:ikmax], data.k, data.chi)
    dset.estimate_noise(chi=chi, rmin=15.0, rmax=30.0)
    return {"epsilon_k": as_list(dset.epsilon_k), "epsilon_r": as_list(dset.epsilon_r)}


def main() -> None:
    script_dir = Path(__file__).resolve().parent
    crate_dir = script_dir.parent
    repo_root = crate_dir.parent.parent
    fixture_root = repo_root / "crates/xraytsubaki/tests/testfiles/xraylarch_d867"
    out_dir = crate_dir / "tests" / "testfiles" / "larch_fit_refs"
    out_dir.mkdir(parents=True, exist_ok=True)

    data = read_ascii(str(fixture_root / "xafsdata" / "cu_150k.xmu"), labels="energy mutrans")
    autobk(data.energy, data.mutrans, group=data, rbkg=1.1, kw=2, clamp_hi=50)

    refs = {}
    refs["cu_kspace_kw2"], _ = run_case(data, fixture_root, "cu_kspace_kw2", kw=2, fitspace="k")
    refs["cu_qspace_kw2"], dset_q = run_case(
        data, fixture_root, "cu_qspace_kw2", kw=2, fitspace="q"
    )
    refs["cu_rspace_kw123"], _ = run_case(
        data, fixture_root, "cu_rspace_kw123", kweight=(1, 2, 3), fitspace="r"
    )
    refs["cu_kspace_kw123"], _ = run_case(
        data, fixture_root, "cu_kspace_kw123", kweight=(1, 2, 3), fitspace="k"
    )

    nq = min(len(dset_q.data.q), len(dset_q.data.chiq_re), len(dset_q.model.chiq_re))
    write_matrix(
        out_dir / "cu_fit_qspace.txt",
        "q data_chiq_re data_chiq_im model_chiq_re model_chiq_im",
        [
            np.asarray(dset_q.data.q)[:nq],
            np.asarray(dset_q.data.chiq_re)[:nq],
            np.asarray(dset_q.data.chiq_im)[:nq],
            np.asarray(dset_q.model.chiq_re)[:nq],
            np.asarray(dset_q.model.chiq_im)[:nq],
        ],
    )

    meta = {
        "generated_at_utc": datetime.now(timezone.utc).isoformat(),
        "xraylarch_version": getattr(larch, "__version__", "unknown"),
        "xraylarch_source_commit": SOURCE_COMMIT,
        "input_epsilon_k": CU_EPSILON_K,
        "noise_estimate_kw123": estimate_noise_reference(data, fixture_root),
        "references": refs,
    }
    (out_dir / "fitspace_metadata.json").write_text(json.dumps(meta, indent=2) + "\n")
    print(f"Wrote references to {out_dir}")


if __name__ == "__main__":
    main()
