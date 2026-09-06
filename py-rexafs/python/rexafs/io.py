"""Native spectrum readers."""
from os import fspath
from . import _core

def read_qas_transmission(path):
    return _core.read_qas_transmission(fspath(path))
