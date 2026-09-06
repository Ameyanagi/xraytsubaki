"""Rust-powered X-ray absorption analysis. Energy is in eV."""
from dataclasses import dataclass
from os import PathLike, fspath
from typing import Sequence

import numpy as np
from numpy.typing import ArrayLike, NDArray

from . import _core
from ._core import __version__

__all__ = ["process", "ProcessedSpectrum", "process_qas_batch", "BatchResult", "BatchFailure",
           "run_pipeline_arrays", "run_batch_qas_trans", "__version__"]


@dataclass(frozen=True)
class ProcessedSpectrum:
    """Owned arrays: k in inverse angstroms; r in angstroms, not phase corrected."""
    e0: float
    k: NDArray[np.float64]
    chi: NDArray[np.float64]
    r: NDArray[np.float64]
    chir_mag: NDArray[np.float64]
    chir_re: NDArray[np.float64]
    chir_im: NDArray[np.float64]


def process(energy: ArrayLike, mu: ArrayLike, *, e0: float | None = None) -> ProcessedSpectrum:
    """Normalize, remove background, and Fourier transform one spectrum.

    Inputs must be finite 1-D arrays of equal length, with strictly increasing
    energy in eV. E0 is found automatically unless supplied. Raises ValueError
    for invalid input/normalization and RuntimeError for processing failures.
    """
    energy_array, mu_array = np.asarray(energy, dtype=np.float64), np.asarray(mu, dtype=np.float64)
    if energy_array.ndim != 1 or mu_array.ndim != 1:
        raise ValueError("energy and mu must be one-dimensional arrays")
    return ProcessedSpectrum(**_core.process(energy_array, mu_array, e0))


@dataclass(frozen=True)
class BatchFailure:
    index: int
    category: str
    message: str


@dataclass(frozen=True)
class BatchResult:
    """Successful pipeline count and failures indexed by original input order."""
    processed_count: int
    errors: list[BatchFailure]


def process_qas_batch(paths: Sequence[str | PathLike[str]]) -> BatchResult:
    """Process QAS transmission files independently, continuing after failures."""
    count, errors = _core.run_batch_qas_trans([fspath(path) for path in paths])
    return BatchResult(count, [BatchFailure(*error) for error in errors])


def run_pipeline_arrays(energy: ArrayLike, mu: ArrayLike) -> dict:
    """Compatibility entry point; prefer process() and its typed result."""
    return vars(process(energy, mu)).copy()


def run_batch_qas_trans(paths: Sequence[str | PathLike[str]]) -> tuple[int, list[tuple[int, str, str]]]:
    """Compatibility tuple; count now means successfully completed spectra."""
    result = process_qas_batch(paths)
    return result.processed_count, [(e.index, e.category, e.message) for e in result.errors]
