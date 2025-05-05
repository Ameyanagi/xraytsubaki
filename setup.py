#!/usr/bin/env python3
from setuptools import setup, find_packages
from setuptools_rust import Binding, RustExtension

setup(
    name="pyxraytsubaki",
    version="0.1.0",
    packages=find_packages(where="pyxraytsubaki/python"),
    package_dir={"": "pyxraytsubaki/python"},
    rust_extensions=[RustExtension("pyxraytsubaki.pyxraytsubaki", 
                                   path="pyxraytsubaki/Cargo.toml",
                                   binding=Binding.PyO3)],
    zip_safe=False,
    install_requires=["numpy>=1.20.0"],
)