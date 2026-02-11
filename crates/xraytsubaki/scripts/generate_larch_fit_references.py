#!/usr/bin/env python3
"""Regenerate Cu/ZnSe FEFF fit reference curves using XrayLarch.

Usage:
  uv run --with xraylarch python crates/xraytsubaki/scripts/generate_larch_fit_references.py
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


def write_matrix(path: Path, header: str, columns: list[np.ndarray]) -> None:
    lens = {len(col) for col in columns}
    if len(lens) != 1:
        raise ValueError(f"length mismatch for {path}: {sorted(lens)}")
    matrix = np.column_stack(columns)
    np.savetxt(path, matrix, fmt="%.15e", header=header)


def full_chir_mag(dset) -> tuple[np.ndarray, np.ndarray]:
    """Return full (un-windowed) |chi(R)| from current data/model chi(k)."""
    data_grid = getattr(dset.data, "r", None)
    model_grid = getattr(dset.model, "r", None)
    r = np.asarray(data_grid if data_grid is not None else model_grid, dtype=float)
    if r.size == 0:
        raise ValueError("missing R grid on feffit dataset")

    data_ft = np.fft.fft(np.asarray(dset.data.chi, dtype=float))
    model_ft = np.fft.fft(np.asarray(dset.model.chi, dtype=float))
    n = min(r.size, data_ft.size, model_ft.size)
    return np.abs(data_ft[:n]), np.abs(model_ft[:n])


def collect_fit_params(out) -> dict[str, dict[str, float]]:
    selected = ("amp", "de0", "sig2", "dr")
    params = {}
    for name in selected:
        p = out.params[name]
        params[name] = {
            "value": float(p.value),
            "stderr": float(p.stderr),
        }
    return params


def epsilon_k_mean(dset) -> float:
    eps = getattr(dset, "epsilon_k", None)
    if eps is None:
        raise ValueError("missing epsilon_k in larch dataset")
    arr = np.asarray(eps, dtype=float)
    return float(arr.mean())


def run_cu(base: Path, out_dir: Path) -> dict[str, object]:
    data = read_ascii(str(base / "xafsdata" / "cu_150k.xmu"), labels="energy mutrans")
    autobk(data.energy, data.mutrans, group=data, rbkg=1.1, kw=2, clamp_hi=50)

    pars = param_group(
        amp=param(1.0, vary=True),
        de0=guess(0.0),
        sig2=param(0.003, vary=True, min=0),
        dr=guess(0.0),
    )
    path1 = feffpath(
        str(base / "feffit" / "Feff_Cu" / "feff0001.dat"),
        s02="amp",
        e0="de0",
        sigma2="sig2",
        deltar="dr",
    )
    trans = feffit_transform(kmin=3, kmax=16, kw=2, dk=5, window="kaiser", rmin=1.4, rmax=3.0)
    dset = feffit_dataset(data=data, transform=trans, pathlist=[path1])
    out = feffit(pars, dset)

    model_k = np.asarray(dset.model.k, dtype=float)
    data_k = np.asarray(dset.data.k, dtype=float)
    data_chi = np.interp(model_k, data_k, np.asarray(dset.data.chi, dtype=float))
    model_chi = np.asarray(dset.model.chi, dtype=float)
    model_kwin = np.asarray(dset.model.kwin, dtype=float)
    write_matrix(
        out_dir / "cu_fit_kspace.txt",
        "k data_chi model_chi kwin",
        [model_k, data_chi, model_chi, model_kwin],
    )
    data_mag_full, model_mag_full = full_chir_mag(dset)

    n_r = min(len(dset.data.r), len(data_mag_full), len(model_mag_full))
    write_matrix(
        out_dir / "cu_fit_rspace.txt",
        "r data_re data_im data_mag model_re model_im model_mag",
        [
            np.asarray(dset.data.r)[:n_r],
            np.asarray(dset.data.chir_re)[:n_r],
            np.asarray(dset.data.chir_im)[:n_r],
            data_mag_full[:n_r],
            np.asarray(dset.model.chir_re)[:n_r],
            np.asarray(dset.model.chir_im)[:n_r],
            model_mag_full[:n_r],
        ],
    )

    return {
        "chi_square": float(out.chi_square),
        "reduced_chi_square": float(out.chi2_reduced),
        "r_factor": float(out.rfactor),
        "n_idp": float(out.n_independent),
        "epsilon_k": epsilon_k_mean(dset),
        "params": collect_fit_params(out),
    }


def run_znse(base: Path, out_dir: Path) -> dict[str, object]:
    data = read_ascii(
        str(base / "xafsdata" / "znse_zn_xafs.001"),
        labels="energy dwelltime i0 i1",
    )
    data.mu = -np.log(data.i1 / data.i0)
    autobk(data, e0=9666.0, rbkg=1.25, kweight=2)

    pars = param_group(
        amp=guess(1.0),
        de0=guess(0.1),
        sig2=param(0.006, vary=True, min=0),
        dr=guess(0.0),
    )
    path1 = feffpath(
        str(base / "feffit" / "Feff_ZnSe" / "feff_znse.dat"),
        degen=4.0,
        s02="amp",
        e0="de0",
        sigma2="sig2",
        deltar="dr",
    )
    trans = feffit_transform(rmin=1.5, rmax=3.0, kmin=3, kmax=13, kw=2, dk=4, window="kaiser")
    dset = feffit_dataset(data=data, transform=trans, pathlist=[path1])
    out = feffit(pars, dset)

    model_k = np.asarray(dset.model.k, dtype=float)
    data_k = np.asarray(dset.data.k, dtype=float)
    data_chi = np.interp(model_k, data_k, np.asarray(dset.data.chi, dtype=float))
    model_chi = np.asarray(dset.model.chi, dtype=float)
    model_kwin = np.asarray(dset.model.kwin, dtype=float)
    write_matrix(
        out_dir / "znse_fit_kspace.txt",
        "k data_chi model_chi kwin",
        [model_k, data_chi, model_chi, model_kwin],
    )
    data_mag_full, model_mag_full = full_chir_mag(dset)

    n_r = min(len(dset.data.r), len(data_mag_full), len(model_mag_full))
    write_matrix(
        out_dir / "znse_fit_rspace.txt",
        "r data_re data_im data_mag model_re model_im model_mag",
        [
            np.asarray(dset.data.r)[:n_r],
            np.asarray(dset.data.chir_re)[:n_r],
            np.asarray(dset.data.chir_im)[:n_r],
            data_mag_full[:n_r],
            np.asarray(dset.model.chir_re)[:n_r],
            np.asarray(dset.model.chir_im)[:n_r],
            model_mag_full[:n_r],
        ],
    )

    return {
        "chi_square": float(out.chi_square),
        "reduced_chi_square": float(out.chi2_reduced),
        "r_factor": float(out.rfactor),
        "n_idp": float(out.n_independent),
        "epsilon_k": epsilon_k_mean(dset),
        "params": collect_fit_params(out),
    }


def main() -> None:
    script_dir = Path(__file__).resolve().parent
    crate_dir = script_dir.parent
    repo_root = crate_dir.parent.parent
    fixture_root_rel = Path("crates/xraytsubaki/tests/testfiles/xraylarch_d867")
    fixture_root = repo_root / fixture_root_rel
    out_dir = crate_dir / "tests" / "testfiles" / "larch_fit_refs"
    out_dir.mkdir(parents=True, exist_ok=True)

    cu_stats = run_cu(fixture_root, out_dir)
    znse_stats = run_znse(fixture_root, out_dir)

    meta = {
        "generated_at_utc": datetime.now(timezone.utc).isoformat(),
        "xraylarch_version": getattr(larch, "__version__", "unknown"),
        "xraylarch_source_commit": SOURCE_COMMIT,
        "fixtures_root": fixture_root_rel.as_posix(),
        "references": {
            "cu": cu_stats,
            "znse": znse_stats,
        },
    }
    (out_dir / "README.md").write_text(
        "# XrayLarch Fit References\n\n"
        "Generated by `crates/xraytsubaki/scripts/generate_larch_fit_references.py`\n"
        f"using XrayLarch fixtures from commit `{SOURCE_COMMIT}`.\n"
    )
    (out_dir / "metadata.json").write_text(json.dumps(meta, indent=2) + "\n")

    print(f"Wrote references to {out_dir}")


if __name__ == "__main__":
    main()
