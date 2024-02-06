from larch.math import utils
import numpy as np

spectrum = np.loadtxt("tests/testfiles/Ru1 RS0001-1 CpstarRu2 Ru  0001.dat")

print(utils.smooth(spectrum[:,0], np.log(spectrum[:,1]/spectrum[:,2])))

print("Hello World!")