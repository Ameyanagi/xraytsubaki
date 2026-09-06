"""Rust Spectrum API for X-ray absorption analysis. Energy is in eV."""
from ._core import Spectrum, PrePostEdge, AUTOBK, XrayFFTF, NormalizationMethod, BackgroundMethod, __version__
from . import io

__all__ = ["Spectrum", "PrePostEdge", "AUTOBK", "XrayFFTF", "NormalizationMethod", "BackgroundMethod", "io", "__version__"]
