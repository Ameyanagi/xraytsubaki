import os

import numpy as np
from larch.math import utils
from larch import Group
from larch.xafs import pre_edge, preedge, xafsft, autobk, xftf, xftr
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
    
    



if __name__ == "__main__":
    
    generate_test_smooth()
    
    generate_preedge()
    
    generate_window_function()
    
    generate_autobk()
    
    generate_xftf()