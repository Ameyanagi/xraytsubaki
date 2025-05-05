import * as xray from '../src';

describe('XRayTsubaki TypeScript API', () => {
  it('should export all required classes and functions', () => {
    // XASSpectrum class
    expect(xray.XASSpectrum).toBeDefined();
    
    // XASGroup class
    expect(xray.XASGroup).toBeDefined();
    
    // XAFS Functions
    expect(xray.findE0).toBeDefined();
    expect(xray.preEdge).toBeDefined();
    expect(xray.autobk).toBeDefined();
    expect(xray.xftf).toBeDefined();
    expect(xray.xftr).toBeDefined();
    
    // Fitting classes
    expect(xray.FittingParameter).toBeDefined();
    expect(xray.FittingParameters).toBeDefined();
    expect(xray.SimplePath).toBeDefined();
    expect(xray.FittingDataset).toBeDefined();
    expect(xray.ExafsFitter).toBeDefined();
    
    // MultiSpectrum classes
    expect(xray.ParameterConstraint).toBeDefined();
    expect(xray.ConstrainedParameter).toBeDefined();
    expect(xray.ConstrainedParameters).toBeDefined();
    expect(xray.MultiSpectrumDataset).toBeDefined();
    expect(xray.MultiSpectrumFitter).toBeDefined();
  });
  
  it('should create instances of classes', () => {
    // Create a spectrum
    const spectrum = new xray.XASSpectrum();
    expect(spectrum).toBeInstanceOf(xray.XASSpectrum);
    
    // Create a group
    const group = new xray.XASGroup();
    expect(group).toBeInstanceOf(xray.XASGroup);
    
    // Create fitting components
    const param = new xray.FittingParameter('amp', 1.0);
    expect(param).toBeInstanceOf(xray.FittingParameter);
    
    const params = new xray.FittingParameters();
    expect(params).toBeInstanceOf(xray.FittingParameters);
    
    const path = new xray.SimplePath('path1', 6, 2.5);
    expect(path).toBeInstanceOf(xray.SimplePath);
    
    const dataset = new xray.FittingDataset(new Float64Array(10), new Float64Array(10));
    expect(dataset).toBeInstanceOf(xray.FittingDataset);
    
    const fitter = new xray.ExafsFitter();
    expect(fitter).toBeInstanceOf(xray.ExafsFitter);
    
    // Create multispectrum components
    const constrainedParam = new xray.ConstrainedParameter('amp', 1.0);
    expect(constrainedParam).toBeInstanceOf(xray.ConstrainedParameter);
    
    const constrainedParams = new xray.ConstrainedParameters();
    expect(constrainedParams).toBeInstanceOf(xray.ConstrainedParameters);
    
    const multiDataset = new xray.MultiSpectrumDataset();
    expect(multiDataset).toBeInstanceOf(xray.MultiSpectrumDataset);
    
    const multiFitter = new xray.MultiSpectrumFitter();
    expect(multiFitter).toBeInstanceOf(xray.MultiSpectrumFitter);
  });
});