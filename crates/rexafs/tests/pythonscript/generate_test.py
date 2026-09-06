import os
import argparse

import numpy as np
from larch.math import utils
from larch import Group
from larch.xafs import pre_edge, preedge, xafsft, autobk, xftf, xftr
from larch.fitting import param, param_group
from larch.xafs import feffpath, path2chi, ff2chi
import json

current_dir = os.path.dirname(os.path.abspath(__file__))


def generate_test_smooth():
    test_filepath = os.path.join(current_dir, "../testfiles/Ru_QAS.dat")
    save_filepath = os.path.join(current_dir, "../testfiles/Ru_QAS_smooth_larch.txt")
    
    data = np.loadtxt(test_filepath)
    energy = data[:,0]
    i0 = data[:,1]
    it = data[:,2]
    mu = np.log(i0/it)
    
    smooth_mu = utils.smooth(energy, mu)
    
    np.savetxt(save_filepath, smooth_mu)


def generate_preedge():
    test_filepath = os.path.join(current_dir, "../testfiles/Ru_QAS.dat")
    save_filepath = os.path.join(current_dir, "../testfiles/Ru_QAS_preedge_larch.txt")
    
    group = Group()
    
    data = np.loadtxt(test_filepath)
    energy = data[:,0]
    i0 = data[:,1]
    it = data[:,2]
    mu = np.log(i0/it)
    
    group.mu = mu
    group.energy = energy
    
    pre_edge_dict = preedge(group.energy, group.mu)
    
    np.savetxt(save_filepath, np.array([energy, pre_edge_dict['norm']]).T)

def generate_window_function():
    
    test_dir = os.path.join(current_dir, "../testfiles/")
    x = np.linspace(0, 10, 11)
    
    window_list = ('Kaiser-Bessel', 'Hanning', 'Parzen', 'Welch', 'Gaussian', 'Sine')
    
    for window_name in window_list:
        window = xafsft.ftwindow(x, window=window_name)
        
        save_filepath = os.path.join(test_dir, "window_{}.txt".format(window_name))
        
        np.savetxt(save_filepath, np.array([x, window]).T)

def generate_autobk():
    test_filepath = os.path.join(current_dir, "../testfiles/Ru_QAS.dat")
    save_filepath = os.path.join(current_dir, "../testfiles/Ru_QAS_autobk_bkg_larch.txt")
    save_k_filepath = os.path.join(current_dir, "../testfiles/Ru_QAS_autobk_k_larch.txt")
    
    group = Group()
    
    data = np.loadtxt(test_filepath)
    energy = data[:,0]
    i0 = data[:,1]
    it = data[:,2]
    mu = np.log(i0/it)
    
    group.mu = mu
    group.energy = energy
    
    pre_edge(group)
    autobk(group)
    
    np.savetxt(save_filepath, np.vstack([group.energy, group.bkg]).T)
    np.savetxt(save_k_filepath, np.vstack([group.k, group.chi]).T)
    
def generate_xftf():
    test_filepath = os.path.join(current_dir, "../testfiles/Ru_QAS.dat")
    save_filepath = os.path.join(current_dir, "../testfiles/Ru_QAS_xftf_larch.txt")
    
    group = Group()
    
    data = np.loadtxt(test_filepath)
    energy = data[:,0]
    i0 = data[:,1]
    it = data[:,2]
    mu = np.log(i0/it)
    
    group.mu = mu
    group.energy = energy
    
    pre_edge(group)
    autobk(group, rbkg=1.4, kweight=2)
    xftf(group, window="hanning", dk=1, kmin=2, kmax=15, kweight=2)
    
    np.savetxt(save_filepath, np.array([group.r, group.chir_mag]).T)
    

def generate_feff_fitting_refs():
    test_dir = os.path.join(current_dir, "../testfiles/")
    feff_path1 = os.path.join(test_dir, "feffcu01.dat")
    feff_path2 = os.path.join(test_dir, "feff0002.dat")

    k = 0.05 * (np.arange(280) + 1.0)
    pars = param_group(
        amp=param(0.92, vary=False),
        de0=param(1.4, vary=False),
        sig2=param(0.0031, vary=False),
        dr=param(0.011, vary=False),
        amp2=param(0.35, vary=False),
        dr2=param(0.0025, vary=False),
    )

    path1 = feffpath(feff_path1, s02="amp", e0="de0", sigma2="sig2", deltar="dr")
    path2chi(path1, params=pars, k=k)
    np.savetxt(
        os.path.join(test_dir, "feff_path_chi_larch_ref.txt"),
        np.column_stack([path1.k, path1.chi]),
    )

    path2 = feffpath(feff_path2, s02="amp2", e0="de0", sigma2="sig2", deltar="dr2")
    out = Group()
    ff2chi([path1, path2], params=pars, k=k, group=out)
    np.savetxt(
        os.path.join(test_dir, "feff_ff2chi_larch_ref.txt"),
        np.column_stack([k, out.chi]),
    )

    fit_target_pars = param_group(
        amp=param(0.88, vary=False),
        de0=param(1.2, vary=False),
        sig2=param(0.0032, vary=False),
        dr=param(0.012, vary=False),
    )
    fit_target_path = feffpath(feff_path1, s02="amp", e0="de0", sigma2="sig2", deltar="dr")
    path2chi(fit_target_path, params=fit_target_pars, k=k)
    np.savetxt(
        os.path.join(test_dir, "feff_fit_target_larch.txt"),
        np.column_stack([k, fit_target_path.chi]),
    )

    



TARGETS = {
    "smooth": generate_test_smooth,
    "preedge": generate_preedge,
    "window": generate_window_function,
    "autobk": generate_autobk,
    "xftf": generate_xftf,
    "feff": generate_feff_fitting_refs,
}


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--target",
        choices=["all", *TARGETS.keys()],
        default="all",
        help="Select one fixture group to generate; default updates all fixture groups.",
    )
    args = parser.parse_args()

    if args.target == "all":
        for fn in TARGETS.values():
            fn()
    else:
        TARGETS[args.target]()
