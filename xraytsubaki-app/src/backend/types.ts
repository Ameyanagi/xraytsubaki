import type { LayoutBase } from "rc-dock";

// --- Load / Spectrum Metadata ---

export interface LoadResult {
  loaded: number;
  errors: LoadError[];
}

export interface LoadError {
  path: string;
  message: string;
}

export interface SpectrumMeta {
  index: number;
  name: string;
  has_e0: boolean;
  has_norm: boolean;
  has_chi: boolean;
  has_chir: boolean;
}

// --- Spectrum Data ---

export interface SpectrumData {
  index: number;
  name: string;
  energy: number[] | null;
  mu: number[] | null;
  e0: number | null;
  norm: number[] | null;
  flat: number[] | null;
  k: number[] | null;
  chi: number[] | null;
  chi_kweighted: number[] | null;
  r: number[] | null;
  chir_mag: number[] | null;
  chir_re: number[] | null;
  chir_im: number[] | null;
  q: number[] | null;
  chiq: number[] | null;
  kwin: number[] | null;
  pre_edge: number[] | null;
  post_edge: number[] | null;
}

// --- Processing Options ---

export interface NormOptions {
  e0?: number;
  pre_edge_start?: number;
  pre_edge_end?: number;
  norm_start?: number;
  norm_end?: number;
  norm_polyorder?: number;
}

export interface BgOptions {
  rbkg?: number;
  kmin?: number;
  kmax?: number;
  kweight?: number;
}

export interface FFTOptions {
  kmin?: number;
  kmax?: number;
  kweight?: number;
  dk?: number;
  window?: string;
}

export interface PipelineOptions {
  norm?: NormOptions;
  bg?: BgOptions;
  fft?: FFTOptions;
}

// --- Batch Processing ---

export interface BatchResult {
  succeeded: number;
  failed: number;
  errors: BatchError[];
}

export interface BatchError {
  index: number;
  name: string;
  message: string;
}

export interface BatchProgressEvent {
  current: number;
  total: number;
  succeeded: number;
  failed: number;
  index: number;
  name: string;
}

// --- Plotting ---

export interface PlotTrace {
  x: number[];
  y: number[];
  label: string;
  panel: string;
  /** If set, this trace is an overlay identified by this key (e.g. "preedge", "dmude"). */
  overlay?: string;
  /** Dash style hint: "solid" | "dash" | "dot" | "dashdot". Default solid. */
  dash?: string;
  /** Optional explicit color for overlay traces. */
  color?: string;
}

export interface PlotResult {
  traces: PlotTrace[];
  svgs: string[];
  x_label: string;
  y_label: string;
}

// --- Workspace ---

export interface LeftSidebarLayoutState {
  collapsed: boolean;
  width: number;
}

export interface WorkspaceLayoutPayload {
  dock: LayoutBase;
  left_sidebar: LeftSidebarLayoutState;
}

export interface WorkspaceData {
  version: string;
  layout: WorkspaceLayoutPayload | null;
  tabs: unknown[];
  spectra_source: string | null;
  spectra_count: number;
  processing: Record<number, ProcessingState>;
  fits: Record<string, unknown>;
  plot_settings: Record<string, unknown>;
}

export interface ProcessingState {
  e0?: number;
  norm_options?: NormOptions;
  bg_options?: BgOptions;
  fft_options?: FFTOptions;
}

// --- FEFF Fitting ---

export interface FeffFitConfig {
  paths: FeffPathConfig[];
  variables: FitVariableConfig[];
  transform: FitTransformConfig;
  data_index: number;
}

export interface FeffPathConfig {
  label: string;
  feff_dat_path: string;
  use_path: boolean;
  s02: string;
  e0: string;
  deltar: string;
  sigma2: string;
}

export interface FitVariableConfig {
  name: string;
  value: number;
  vary: boolean;
  min?: number;
  max?: number;
  expr?: string;
}

export interface FitTransformConfig {
  kmin: number;
  kmax: number;
  kweight: number;
  dk: number;
  rmin: number;
  rmax: number;
}

export interface FeffFitResultDto {
  id: string;
  chi_square: number;
  reduced_chi_square: number;
  r_factor: number;
  n_vary: number;
  n_data: number;
  n_idp: number;
  variables: FitVariableResult[];
  correlation: number[][] | null;
  k: number[];
  data_chi: number[];
  model_chi: number[];
  r: number[];
  data_chir_re: number[];
  data_chir_im: number[];
  model_chir_re: number[];
  model_chir_im: number[];
  model_chir_mag: number[];
  path_contributions: PathContributionDto[];
  warnings: string[];
}

export interface FitVariableResult {
  name: string;
  value: number;
  stderr: number | null;
  vary: boolean;
  init_value: number;
}

export interface PathContributionDto {
  label: string;
  chi: number[];
  chir_re: number[];
  chir_im: number[];
  chir_mag: number[];
}

// --- Plot Mode ---

export type PlotMode = "mu" | "norm" | "k" | "r";
export type RenderMode = "interactive" | "core";
