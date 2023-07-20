import os

import numpy as np
from larch.math import utils

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


if __name__ == "__main__":
    
    generate_test_smooth()