#!/usr/bin/env python
"""
Generate test data for XAS fitting with xraylarch
This script creates example feffpaths and fits using xraylarch, 
then saves the results to be used for testing the Rust implementation.
"""

import os
import numpy as np
from larch import Group
from larch.xafs import pre_edge, autobk, feffit_dataset, feffit_transform
from larch.xafs import feffit, feffit_report, feffpath
from larch.io import read_ascii

current_dir = os.path.dirname(os.path.abspath(__file__))
test_files_dir = os.path.join(current_dir, "../testfiles")
fit_results_dir = os.path.join(test_files_dir, "fit_results")

# Create output directory if it doesn't exist
if not os.path.exists(fit_results_dir):
    os.makedirs(fit_results_dir)

def create_simple_feff_path():
    """Create a simple single-path feff model and fit"""
    # Read data
    data_file = os.path.join(test_files_dir, "Ru_QAS.dat")
    data = read_ascii(data_file, labels='energy i0 itrans')
    data.mu = -np.log(data.itrans/data.i0)
    
    # Process data
    dat = Group(energy=data.energy, mu=data.mu)
    pre_edge(dat, e0=22117.0, pre1=-200, pre2=-75, norm1=150, norm2=600)
    autobk(dat, rbkg=1.1, kweight=2, kmin=0, kmax=15)
    
    # Setup transform parameters
    trans = feffit_transform(kmin=3, kmax=14, kweight=2, dk=3, window='hanning',
                            rmin=1.4, rmax=3.0)
    
    # Create feff path parameters
    path_params = Group()
    from larch.fitting import param
    path_params.deltar = param(name='deltar', vary=True, value=0.0)
    path_params.e0 = param(name='e0', vary=True, value=0.0)
    path_params.s02 = param(name='s02', vary=True, value=1.0)
    path_params.sigma2 = param(name='sigma2', vary=True, value=0.003)
    
    # Create a simple Ru-O path with distance ~2.0 Å
    feff_path = feffpath(reff=2.0, degen=6, 
                        s02='s02', e0='e0', 
                        deltar='deltar', sigma2='sigma2')
    
    # Create dataset
    dset = feffit_dataset(data=dat, pathlist=[feff_path], transform=trans)
    
    # Run the fit
    fit = feffit(path_params, dset)
    
    # Save results
    np.savetxt(os.path.join(fit_results_dir, 'simple_fit_params.dat'), 
               np.array([
                   fit.params.s02.value, fit.params.s02.stderr,
                   fit.params.e0.value, fit.params.e0.stderr,
                   fit.params.deltar.value, fit.params.deltar.stderr,
                   fit.params.sigma2.value, fit.params.sigma2.stderr
               ]).reshape(1, -1),
               header="s02 s02_err e0 e0_err deltar deltar_err sigma2 sigma2_err")
    
    # Save data and model
    fit_data = np.column_stack((
        dset.data.k,
        dset.data.chi * dset.data.k**2,
        dset.model.chi * dset.model.k**2
    ))
    np.savetxt(os.path.join(fit_results_dir, 'simple_fit_k_chi.dat'), fit_data,
               header="k chi_k2_data chi_k2_model")
    
    # Save r-space data and model
    r_data = np.column_stack((
        dset.data.r,
        np.abs(dset.data.chir_mag),
        np.abs(dset.model.chir_mag)
    ))
    np.savetxt(os.path.join(fit_results_dir, 'simple_fit_r_chir.dat'), r_data,
               header="r chir_mag_data chir_mag_model")
    
    # Save fit statistics
    with open(os.path.join(fit_results_dir, 'simple_fit_stats.dat'), 'w') as f:
        f.write(f"# nvarys {fit.nvarys}\n")
        f.write(f"# npts {fit.npts}\n")
        f.write(f"# nfree {fit.nfree}\n")
        f.write(f"# chi_square {fit.chi_square}\n")
        f.write(f"# reduced_chi_square {fit.chi_reduced}\n")
        f.write(f"# r_factor {fit.rfactor}\n")
        f.write(f"# akaike_info_criterion {fit.aic}\n")
        f.write(f"# bayesian_info_criterion {fit.bic}\n")
    
    # Print report for reference
    print(feffit_report(fit))
    return fit

def create_multiple_shells_fit():
    """Create a multi-shell feff model and fit with multiple paths"""
    # Read data
    data_file = os.path.join(test_files_dir, "Ru_QAS.dat")
    data = read_ascii(data_file, labels='energy i0 itrans')
    data.mu = -np.log(data.itrans/data.i0)
    
    # Process data
    dat = Group(energy=data.energy, mu=data.mu)
    pre_edge(dat, e0=22117.0, pre1=-200, pre2=-75, norm1=150, norm2=600)
    autobk(dat, rbkg=1.1, kweight=2, kmin=0, kmax=15)
    
    # Setup transform parameters
    trans = feffit_transform(kmin=3, kmax=14, kweight=2, dk=3, window='hanning',
                            rmin=1.4, rmax=4.5)
    
    # Create path parameters
    path_params = Group()
    from larch.fitting import param
    
    # Amplitude and energy shift
    path_params.amp = param(name='amp', vary=True, value=1.0)
    path_params.del_e0 = param(name='del_e0', vary=True, value=0.0)
    
    # First shell (Ru-O) parameters
    path_params.dr_1 = param(name='dr_1', vary=True, value=0.0)
    path_params.ss_1 = param(name='ss_1', vary=True, value=0.003)
    
    # Second shell (Ru-Ru) parameters
    path_params.dr_2 = param(name='dr_2', vary=True, value=0.0)
    path_params.ss_2 = param(name='ss_2', vary=True, value=0.006)
    
    # Create paths
    path1 = feffpath(reff=2.0, degen=6, 
                    s02='amp', e0='del_e0', 
                    deltar='dr_1', sigma2='ss_1')
    
    path2 = feffpath(reff=3.5, degen=12, 
                    s02='amp', e0='del_e0', 
                    deltar='dr_2', sigma2='ss_2')
    
    # Create dataset
    dset = feffit_dataset(data=dat, pathlist=[path1, path2], transform=trans)
    
    # Run the fit
    fit = feffit(path_params, dset)
    
    # Save results
    np.savetxt(os.path.join(fit_results_dir, 'multi_shell_fit_params.dat'), 
               np.array([
                   fit.params.amp.value, fit.params.amp.stderr,
                   fit.params.del_e0.value, fit.params.del_e0.stderr,
                   fit.params.dr_1.value, fit.params.dr_1.stderr,
                   fit.params.ss_1.value, fit.params.ss_1.stderr,
                   fit.params.dr_2.value, fit.params.dr_2.stderr,
                   fit.params.ss_2.value, fit.params.ss_2.stderr
               ]).reshape(1, -1),
               header="amp amp_err e0 e0_err dr_1 dr_1_err ss_1 ss_1_err dr_2 dr_2_err ss_2 ss_2_err")
    
    # Save data and model
    fit_data = np.column_stack((
        dset.data.k,
        dset.data.chi * dset.data.k**2,
        dset.model.chi * dset.model.k**2
    ))
    np.savetxt(os.path.join(fit_results_dir, 'multi_shell_fit_k_chi.dat'), fit_data,
               header="k chi_k2_data chi_k2_model")
    
    # Save r-space data and model
    r_data = np.column_stack((
        dset.data.r,
        np.abs(dset.data.chir_mag),
        np.abs(dset.model.chir_mag)
    ))
    np.savetxt(os.path.join(fit_results_dir, 'multi_shell_fit_r_chir.dat'), r_data,
               header="r chir_mag_data chir_mag_model")
    
    # Save individual path contributions
    path1_data = np.column_stack((
        dset.pathlist[0].k,
        dset.pathlist[0].chi * dset.pathlist[0].k**2
    ))
    np.savetxt(os.path.join(fit_results_dir, 'multi_shell_path1_k_chi.dat'), path1_data,
               header="k chi_k2_path1")
    
    path2_data = np.column_stack((
        dset.pathlist[1].k,
        dset.pathlist[1].chi * dset.pathlist[1].k**2
    ))
    np.savetxt(os.path.join(fit_results_dir, 'multi_shell_path2_k_chi.dat'), path2_data,
               header="k chi_k2_path2")
    
    # Save fit statistics
    with open(os.path.join(fit_results_dir, 'multi_shell_fit_stats.dat'), 'w') as f:
        f.write(f"# nvarys {fit.nvarys}\n")
        f.write(f"# npts {fit.npts}\n")
        f.write(f"# nfree {fit.nfree}\n")
        f.write(f"# chi_square {fit.chi_square}\n")
        f.write(f"# reduced_chi_square {fit.chi_reduced}\n")
        f.write(f"# r_factor {fit.rfactor}\n")
        f.write(f"# akaike_info_criterion {fit.aic}\n")
        f.write(f"# bayesian_info_criterion {fit.bic}\n")
    
    # Print report for reference
    print(feffit_report(fit))
    return fit

if __name__ == "__main__":
    print("Generating simple single-path fit test data...")
    create_simple_feff_path()
    
    print("\nGenerating multiple-shell fit test data...")
    create_multiple_shells_fit()
    
    print(f"\nTest data saved to {fit_results_dir}")