import type {
  LoadResult,
  SpectrumMeta,
  SpectrumData,
  NormOptions,
  BgOptions,
  FFTOptions,
  PipelineOptions,
  BatchResult,
  PlotResult,
  WorkspaceData,
  FeffRunConfig,
  FeffRunResultDto,
  FeffFitConfig,
  FeffFitResultDto,
} from "./types";

export interface XASBackend {
  // File operations
  loadSpectraFromFiles(paths: string[]): Promise<LoadResult>;
  loadWorkspace(path: string): Promise<WorkspaceData>;
  saveWorkspace(path: string, data: WorkspaceData): Promise<void>;

  // Spectrum access
  getSpectrumList(): Promise<SpectrumMeta[]>;
  getSpectrumData(index: number): Promise<SpectrumData>;
  removeSpectra(indices: number[]): Promise<number>;

  // Processing (individual)
  findE0(index: number): Promise<number>;
  normalize(index: number, opts?: NormOptions): Promise<void>;
  calcBackground(index: number, opts?: BgOptions): Promise<void>;
  fft(index: number, opts?: FFTOptions): Promise<void>;
  runPipeline(index: number, opts?: PipelineOptions): Promise<void>;

  // Processing (batch)
  batchProcess(indices: number[], opts?: PipelineOptions): Promise<BatchResult>;

  // Plotting
  plotSpectrum(index: number, panels: string[]): Promise<PlotResult>;
  plotGroup(indices: number[], panels: string[]): Promise<PlotResult>;
  plotSvg(index: number, panels: string[]): Promise<string[]>;
  plotFit(fitId: string, panel: "k" | "r", includePaths?: boolean): Promise<PlotResult>;

  // Fitting
  runFeffPaths(config: FeffRunConfig): Promise<FeffRunResultDto>;
  runFeffFit(config: FeffFitConfig): Promise<FeffFitResultDto>;
  getFitResult(fitId: string): Promise<FeffFitResultDto>;
  listFitResults(): Promise<string[]>;

  // Workspace
  getWorkspacePath(): Promise<string | null>;
}
