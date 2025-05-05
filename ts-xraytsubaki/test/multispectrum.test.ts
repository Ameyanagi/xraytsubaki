import { 
  XASSpectrum,
  FittingParameter, 
  SimplePath, 
  FittingDataset,
  MultiSpectrumDataset,
  ParameterConstraint,
  ConstrainedParameter,
  ConstrainedParameters,
  MultiSpectrumFitter
} from '../src';
import * as path from 'path';
import * as fs from 'fs';

describe('Multi-Spectrum Fitting', () => {
  // Test constants
  const TEST_DATA_DIR = path.join(__dirname, '../../crates/xraytsubaki/tests/testfiles');
  const SYNTHETIC_SPECTRUM1 = path.join(TEST_DATA_DIR, 'fit_results/synthetic_spectrum_1.dat');
  const SYNTHETIC_SPECTRUM2 = path.join(TEST_DATA_DIR, 'fit_results/synthetic_spectrum_2.dat');
  const SYNTHETIC_SPECTRUM3 = path.join(TEST_DATA_DIR, 'fit_results/synthetic_spectrum_3.dat');
  
  function loadOrCreateTestData(index: number): { k: Float64Array, chi: Float64Array } {
    const filePath = path.join(TEST_DATA_DIR, `fit_results/synthetic_spectrum_${index}.dat`);
    
    // Load synthetic spectrum if it exists
    if (fs.existsSync(filePath)) {
      const data = fs.readFileSync(filePath, 'utf-8');
      const lines = data.split('\n');
      
      const k: number[] = [];
      const chi: number[] = [];
      
      for (const line of lines) {
        const trimmed = line.trim();
        if (trimmed === '' || trimmed.startsWith('#')) continue;
        
        const parts = trimmed.split(/\s+/);
        if (parts.length >= 2) {
          k.push(parseFloat(parts[0]));
          chi.push(parseFloat(parts[1]));
        }
      }
      
      return {
        k: new Float64Array(k),
        chi: new Float64Array(chi)
      };
    }
    
    // Create synthetic EXAFS data with slightly different parameters for each spectrum
    const k = new Float64Array(100);
    const chi = new Float64Array(100);
    
    // Vary parameters slightly based on index
    const s02Base = 0.9;
    const sigma2Base = 0.003;
    const rBase = 2.5;
    const e0Base = 0;
    
    // Modify parameters based on spectrum index
    const s02 = s02Base - 0.05 * (index - 1);  // 0.9, 0.85, 0.8
    const sigma2 = sigma2Base + 0.001 * (index - 1);  // 0.003, 0.004, 0.005
    const r = rBase + 0.1 * (index - 1);  // 2.5, 2.6, 2.7
    const e0 = e0Base + 1.0 * (index - 1);  // 0, 1, 2
    
    for (let i = 0; i < 100; i++) {
      k[i] = 0.05 * (i + 1);
      
      // Simple EXAFS model with one path
      const n = 6;
      const amplitude = s02 * n * Math.exp(-2 * sigma2 * k[i] * k[i]) / (k[i] * r * r);
      const phase = 2 * k[i] * (r + 0.01 * e0) + 0.1 * k[i];
      
      chi[i] = amplitude * Math.sin(phase);
    }
    
    return { k, chi };
  }
  
  it('should create constrained parameters', () => {
    const param = new ConstrainedParameter('amp', 1.0);
    expect(param.getName()).toBe('amp');
    expect(param.getValue()).toBe(1.0);
    expect(param.getConstraint()).toBeNull();
    
    // Set reference constraint
    param.referTo('amp_ref');
    expect(param.getConstraint()).toBeDefined();
    expect(param.getConstraint()?.type).toBe('reference');
    expect(param.getConstraint()?.reference).toBe('amp_ref');
    
    // Reset and set scale constraint
    param.resetConstraint();
    param.scaleFrom('amp_ref', 0.8);
    expect(param.getConstraint()).toBeDefined();
    expect(param.getConstraint()?.type).toBe('scale');
    expect(param.getConstraint()?.reference).toBe('amp_ref');
    expect(param.getConstraint()?.factor).toBe(0.8);
    
    // Reset and set offset constraint
    param.resetConstraint();
    param.offsetFrom('e0_ref', 2.0);
    expect(param.getConstraint()).toBeDefined();
    expect(param.getConstraint()?.type).toBe('offset');
    expect(param.getConstraint()?.reference).toBe('e0_ref');
    expect(param.getConstraint()?.offset).toBe(2.0);
  });

  it('should create constrained parameter set', () => {
    const params = new ConstrainedParameters();
    
    // Add parameters for spectrum 1
    params.add('s02_1', 0.9, 0.7, 1.1);
    params.add('e0_1', 0.0, -5.0, 5.0);
    params.add('sigma2_1', 0.003, 0.001, 0.01);
    params.add('delr_1', 0.0, -0.1, 0.1);
    
    // Add parameters for spectrum 2 with constraints
    const s02_2 = params.add('s02_2', 0.85);
    s02_2.scaleFrom('s02_1', 0.9);  // s02_2 = 0.9 * s02_1
    
    const e0_2 = params.add('e0_2', 1.0);
    e0_2.offsetFrom('e0_1', 1.0);  // e0_2 = e0_1 + 1.0
    
    const sigma2_2 = params.add('sigma2_2', 0.004);
    sigma2_2.referTo('sigma2_1');  // sigma2_2 = sigma2_1
    
    params.add('delr_2', 0.05, -0.1, 0.1);
    
    // Check parameter count
    expect(params.size()).toBe(8);
    
    // Check constraints
    expect(params.get('s02_2')?.getConstraint()?.type).toBe('scale');
    expect(params.get('e0_2')?.getConstraint()?.type).toBe('offset');
    expect(params.get('sigma2_2')?.getConstraint()?.type).toBe('reference');
    
    // Update a reference parameter and check the constrained values
    params.set('s02_1', 1.0);
    params.updateConstraints();
    
    // s02_2 should be 0.9 * s02_1 = 0.9 * 1.0 = 0.9
    expect(params.get('s02_2')?.getValue()).toBeCloseTo(0.9, 6);
  });

  it('should create a multi-spectrum dataset', () => {
    // Create three datasets
    const data1 = loadOrCreateTestData(1);
    const data2 = loadOrCreateTestData(2);
    const data3 = loadOrCreateTestData(3);
    
    const dataset1 = new FittingDataset(data1.k, data1.chi);
    dataset1.setKRange(2.0, 12.0);
    dataset1.setKWeight(2);
    
    const dataset2 = new FittingDataset(data2.k, data2.chi);
    dataset2.setKRange(2.0, 12.0);
    dataset2.setKWeight(2);
    
    const dataset3 = new FittingDataset(data3.k, data3.chi);
    dataset3.setKRange(2.0, 12.0);
    dataset3.setKWeight(2);
    
    // Create multi-spectrum dataset
    const multiDataset = new MultiSpectrumDataset();
    multiDataset.addDataset('spectrum1', dataset1);
    multiDataset.addDataset('spectrum2', dataset2);
    multiDataset.addDataset('spectrum3', dataset3);
    
    expect(multiDataset.size()).toBe(3);
    expect(multiDataset.getDataset('spectrum1')).toBe(dataset1);
    expect(multiDataset.getDataset('spectrum2')).toBe(dataset2);
    expect(multiDataset.getDataset('spectrum3')).toBe(dataset3);
  });

  it('should fit multiple spectra with constraints', () => {
    // Create three datasets
    const data1 = loadOrCreateTestData(1);
    const data2 = loadOrCreateTestData(2);
    const data3 = loadOrCreateTestData(3);
    
    const dataset1 = new FittingDataset(data1.k, data1.chi);
    dataset1.setKRange(2.0, 12.0);
    dataset1.setKWeight(2);
    
    const dataset2 = new FittingDataset(data2.k, data2.chi);
    dataset2.setKRange(2.0, 12.0);
    dataset2.setKWeight(2);
    
    const dataset3 = new FittingDataset(data3.k, data3.chi);
    dataset3.setKRange(2.0, 12.0);
    dataset3.setKWeight(2);
    
    // Create multi-spectrum dataset
    const multiDataset = new MultiSpectrumDataset();
    multiDataset.addDataset('spectrum1', dataset1);
    multiDataset.addDataset('spectrum2', dataset2);
    multiDataset.addDataset('spectrum3', dataset3);
    
    // Create constrained parameters
    const params = new ConstrainedParameters();
    
    // Add parameters for spectrum 1
    params.add('s02_1', 0.9, 0.7, 1.1);
    params.add('e0_1', 0.0, -5.0, 5.0);
    params.add('sigma2_1', 0.003, 0.001, 0.01);
    params.add('delr_1', 0.0, -0.1, 0.1);
    
    // Add parameters for spectrum 2 with constraints
    const s02_2 = params.add('s02_2', 0.85);
    s02_2.scaleFrom('s02_1', 0.95);  // s02_2 = 0.95 * s02_1
    
    const e0_2 = params.add('e0_2', 1.0);
    e0_2.offsetFrom('e0_1', 1.0);  // e0_2 = e0_1 + 1.0
    
    params.add('sigma2_2', 0.004, 0.001, 0.01);
    params.add('delr_2', 0.05, -0.1, 0.2);
    
    // Add parameters for spectrum 3 with constraints
    const s02_3 = params.add('s02_3', 0.8);
    s02_3.scaleFrom('s02_1', 0.9);  // s02_3 = 0.9 * s02_1
    
    const e0_3 = params.add('e0_3', 2.0);
    e0_3.offsetFrom('e0_1', 2.0);  // e0_3 = e0_1 + 2.0
    
    params.add('sigma2_3', 0.005, 0.001, 0.01);
    params.add('delr_3', 0.1, 0.0, 0.3);
    
    // Create paths for each spectrum
    const path1 = new SimplePath('path1_1', 6, 2.5);
    path1.setS02('s02_1');
    path1.setE0('e0_1');
    path1.setSigma2('sigma2_1');
    path1.setDelr('delr_1');
    
    const path2 = new SimplePath('path1_2', 6, 2.6);
    path2.setS02('s02_2');
    path2.setE0('e0_2');
    path2.setSigma2('sigma2_2');
    path2.setDelr('delr_2');
    
    const path3 = new SimplePath('path1_3', 6, 2.7);
    path3.setS02('s02_3');
    path3.setE0('e0_3');
    path3.setSigma2('sigma2_3');
    path3.setDelr('delr_3');
    
    // Create multi-spectrum fitter
    const fitter = new MultiSpectrumFitter();
    
    // Add paths for each spectrum
    fitter.addPath('spectrum1', path1);
    fitter.addPath('spectrum2', path2);
    fitter.addPath('spectrum3', path3);
    
    // Perform the fit
    const result = fitter.fit(multiDataset, params);
    
    // Check results
    expect(result).toBeDefined();
    expect(result.success).toBe(true);
    expect(result.nfev).toBeGreaterThan(0);
    expect(result.message).toBeDefined();
    
    // Check parameter values
    expect(result.params.get('s02_1')?.getValue()).toBeCloseTo(0.9, 1);
    expect(result.params.get('sigma2_1')?.getValue()).toBeCloseTo(0.003, 3);
    expect(result.params.get('e0_1')?.getValue()).toBeCloseTo(0, 0);
    
    // Check that constraints were maintained
    const s02_1 = result.params.get('s02_1')?.getValue() || 0;
    const e0_1 = result.params.get('e0_1')?.getValue() || 0;
    
    expect(result.params.get('s02_2')?.getValue()).toBeCloseTo(0.95 * s02_1, 6);
    expect(result.params.get('s02_3')?.getValue()).toBeCloseTo(0.9 * s02_1, 6);
    expect(result.params.get('e0_2')?.getValue()).toBeCloseTo(e0_1 + 1.0, 6);
    expect(result.params.get('e0_3')?.getValue()).toBeCloseTo(e0_1 + 2.0, 6);
    
    // Check fit quality for each spectrum
    expect(result.redchi).toBeDefined();
    expect(result.redchi).toBeLessThan(10);  // A reasonable fit should have a relatively low reduced chi-squared
    
    // Check that the result includes best fits for each spectrum
    expect(result.best_fits).toBeDefined();
    expect(result.best_fits['spectrum1']).toBeDefined();
    expect(result.best_fits['spectrum2']).toBeDefined();
    expect(result.best_fits['spectrum3']).toBeDefined();
    expect(result.best_fits['spectrum1'].length).toBe(data1.chi.length);
    expect(result.best_fits['spectrum2'].length).toBe(data2.chi.length);
    expect(result.best_fits['spectrum3'].length).toBe(data3.chi.length);
  });
});