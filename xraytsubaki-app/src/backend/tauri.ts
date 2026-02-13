import { invoke } from "@tauri-apps/api/core";
import type { XASBackend } from "./interface";
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

export class TauriBackend implements XASBackend {
  async loadSpectraFromFiles(paths: string[]): Promise<LoadResult> {
    return invoke("load_spectra", { paths });
  }

  async loadWorkspace(path: string): Promise<WorkspaceData> {
    return invoke("load_workspace", { path });
  }

  async saveWorkspace(path: string, data: WorkspaceData): Promise<void> {
    return invoke("save_workspace", { path, data });
  }

  async getSpectrumList(): Promise<SpectrumMeta[]> {
    return invoke("get_spectrum_list");
  }

  async getSpectrumData(index: number): Promise<SpectrumData> {
    return invoke("get_spectrum_data", { index });
  }

  async removeSpectra(indices: number[]): Promise<number> {
    return invoke("remove_spectra", { indices });
  }

  async findE0(index: number): Promise<number> {
    return invoke("find_e0", { index });
  }

  async normalize(index: number, opts?: NormOptions): Promise<void> {
    return invoke("normalize", { index, opts: opts ?? null });
  }

  async calcBackground(index: number, opts?: BgOptions): Promise<void> {
    return invoke("calc_background", { index, opts: opts ?? null });
  }

  async fft(index: number, opts?: FFTOptions): Promise<void> {
    return invoke("fft", { index, opts: opts ?? null });
  }

  async runPipeline(index: number, opts?: PipelineOptions): Promise<void> {
    return invoke("run_pipeline", { index, opts: opts ?? null });
  }

  async batchProcess(indices: number[], opts?: PipelineOptions): Promise<BatchResult> {
    return invoke("batch_process", { indices, opts: opts ?? null });
  }

  async plotSpectrum(index: number, panels: string[], opts?: PipelineOptions): Promise<PlotResult> {
    return invoke("plot_spectrum", { index, panels, opts: opts ?? null });
  }

  async plotGroup(
    indices: number[],
    panels: string[],
    opts?: PipelineOptions,
  ): Promise<PlotResult> {
    return invoke("plot_group", { indices, panels, opts: opts ?? null });
  }

  async plotCore(index: number, panels: string[], opts?: PipelineOptions): Promise<PlotResult> {
    return invoke("plot_core", { index, panels, opts: opts ?? null });
  }

  async plotFit(fitId: string, panel: "k" | "r", includePaths = true): Promise<PlotResult> {
    return invoke("plot_fit", { fitId, panel, includePaths });
  }

  async runFeffPaths(config: FeffRunConfig): Promise<FeffRunResultDto> {
    return invoke("run_feff_paths", { config });
  }

  async runFeffFit(config: FeffFitConfig): Promise<FeffFitResultDto> {
    return invoke("run_feff_fit", { config });
  }

  async getFitResult(fitId: string): Promise<FeffFitResultDto> {
    return invoke("get_fit_result", { fitId });
  }

  async listFitResults(): Promise<string[]> {
    return invoke("list_fit_results");
  }

  async getWorkspacePath(): Promise<string | null> {
    return invoke("get_workspace_path");
  }
}

// Singleton backend instance — use mock when Tauri IPC is unavailable
import { MockBackend } from "./mock";
import type { XASBackend as IBackend } from "./interface";

function createBackend(): IBackend {
  if (typeof window !== "undefined" && "__TAURI_INTERNALS__" in window) {
    return new TauriBackend();
  }
  console.info("[xraytsubaki] Tauri not detected, using mock backend");
  return new MockBackend();
}

export const backend = createBackend();
