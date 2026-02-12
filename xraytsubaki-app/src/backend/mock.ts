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
  PlotTrace,
  WorkspaceData,
  FeffRunConfig,
  FeffRunResultDto,
  FeffFitConfig,
  FeffFitResultDto,
} from "./types";

// Numerical derivative: dy/dx
function derivative(x: number[], y: number[]): number[] {
  const d: number[] = [];
  for (let i = 0; i < y.length; i++) {
    if (i === 0) d.push((y[1] - y[0]) / (x[1] - x[0]));
    else if (i === y.length - 1) d.push((y[i] - y[i - 1]) / (x[i] - x[i - 1]));
    else d.push((y[i + 1] - y[i - 1]) / (x[i + 1] - x[i - 1]));
  }
  return d;
}

// Hanning window for k-space
function hanningWindow(kArr: number[], kmin: number, kmax: number, dk: number): number[] {
  return kArr.map((k) => {
    if (k < kmin - dk || k > kmax + dk) return 0;
    if (k < kmin + dk) return 0.5 * (1 + Math.cos(Math.PI * (k - kmin - dk) / (2 * dk)));
    if (k > kmax - dk) return 0.5 * (1 + Math.cos(Math.PI * (k - kmax + dk) / (2 * dk)));
    return 1;
  });
}

function fnv1a(input: string): number {
  let hash = 0x811c9dc5;
  for (let i = 0; i < input.length; i++) {
    hash ^= input.charCodeAt(i);
    hash = Math.imul(hash, 0x01000193);
  }
  return hash >>> 0;
}

function buildDeterministicRng(seed: number): () => number {
  let state = seed || 1;
  return () => {
    state = (Math.imul(state, 1664525) + 1013904223) >>> 0;
    return state / 0xffffffff;
  };
}

// Generate realistic Cu K-edge XANES/EXAFS data
function generateCuKedge(name: string, shift = 0, noise = 0.002): SpectrumData {
  const e0 = 8979.0 + shift;
  const nPre = 100;
  const nEdge = 50;
  const nPost = 350;
  const energy: number[] = [];
  const mu: number[] = [];
  const norm: number[] = [];
  const preEdge: number[] = [];
  const postEdge: number[] = [];

  // Pre-edge: linear slope
  for (let i = 0; i < nPre; i++) {
    const e = e0 - 200 + (i * 170) / nPre;
    energy.push(e);
    const muVal = 0.5 + 0.0002 * (e - e0 + 200) + (Math.random() - 0.5) * noise;
    mu.push(muVal);
    preEdge.push(0.5 + 0.0002 * (e - e0 + 200));
    postEdge.push(1.5 + 0.0001 * (e - e0));
    norm.push(0);
  }

  // Edge region: arctan step
  for (let i = 0; i < nEdge; i++) {
    const e = e0 - 30 + (i * 60) / nEdge;
    energy.push(e);
    const t = (e - e0) / 3;
    const step = 0.5 + Math.atan(t) / Math.PI;
    const muVal = 0.5 + step * 1.0 + (Math.random() - 0.5) * noise;
    mu.push(muVal);
    preEdge.push(0.5 + 0.0002 * (e - e0 + 200));
    postEdge.push(1.5 + 0.0001 * (e - e0));
    norm.push(step);
  }

  // Post-edge: damped EXAFS oscillations
  for (let i = 0; i < nPost; i++) {
    const e = e0 + 30 + (i * 770) / nPost;
    energy.push(e);
    const k = Math.sqrt(0.2625 * (e - e0));
    const exafs = 0.05 * Math.exp(-0.003 * k * k) * Math.sin(2 * 2.55 * k + 0.5);
    const muVal = 1.5 + 0.0001 * (e - e0) + exafs + (Math.random() - 0.5) * noise;
    mu.push(muVal);
    preEdge.push(0.5 + 0.0002 * (e - e0 + 200));
    postEdge.push(1.5 + 0.0001 * (e - e0));
    norm.push(1.0 + exafs / 1.0);
  }

  // k-space chi(k)
  const kArr: number[] = [];
  const chi: number[] = [];
  const chiKw: number[] = [];
  const kwin = hanningWindow(
    Array.from({ length: 200 }, (_, i) => i * 0.05 + 0.5),
    2, 12, 1,
  );
  for (let i = 0; i < 200; i++) {
    const k = i * 0.05 + 0.5;
    kArr.push(k);
    const chiVal =
      0.8 * Math.exp(-0.006 * k * k) * Math.sin(2 * 2.55 * k + 0.3) +
      0.3 * Math.exp(-0.01 * k * k) * Math.sin(2 * 3.61 * k - 0.2);
    chi.push(chiVal);
    chiKw.push(chiVal * k * k);
  }

  // R-space (simplified FT magnitude)
  const r: number[] = [];
  const chirMag: number[] = [];
  const chirRe: number[] = [];
  const chirIm: number[] = [];
  for (let i = 0; i < 200; i++) {
    const rVal = i * 0.03;
    r.push(rVal);
    const peak1 = 2.0 * Math.exp(-8 * (rVal - 1.5) * (rVal - 1.5));
    const peak2 = 1.2 * Math.exp(-6 * (rVal - 2.55) * (rVal - 2.55));
    const peak3 = 0.4 * Math.exp(-10 * (rVal - 3.6) * (rVal - 3.6));
    const mag = peak1 + peak2 + peak3;
    chirMag.push(mag);
    chirRe.push(mag * Math.cos(rVal * 5));
    chirIm.push(mag * Math.sin(rVal * 5));
  }

  return {
    index: 0,
    name,
    energy,
    mu,
    e0,
    norm,
    flat: norm,
    k: kArr,
    chi,
    chi_kweighted: chiKw,
    r,
    chir_mag: chirMag,
    chir_re: chirRe,
    chir_im: chirIm,
    q: null,
    chiq: null,
    kwin,
    pre_edge: preEdge,
    post_edge: postEdge,
  };
}

const MOCK_SPECTRA = [
  generateCuKedge("Cu_foil_001.dat", 0, 0.002),
  generateCuKedge("Cu_foil_002.dat", 0.3, 0.003),
  generateCuKedge("Cu_foil_003.dat", -0.2, 0.0025),
  generateCuKedge("Fe2O3_001.dat", 100, 0.004),
  generateCuKedge("Fe2O3_002.dat", 100.5, 0.0035),
];

MOCK_SPECTRA.forEach((s, i) => {
  s.index = i;
});

// Per-spectrum processing options (stored for re-apply)
interface StoredOptions {
  norm?: NormOptions;
  bg?: BgOptions;
  fft?: FFTOptions;
}

export class MockBackend implements XASBackend {
  private spectra = [...MOCK_SPECTRA];
  private options: Map<number, StoredOptions> = new Map();
  private fitResults = new Map<string, FeffFitResultDto>();

  async loadSpectraFromFiles(paths: string[]): Promise<LoadResult> {
    return { loaded: paths.length, errors: [] };
  }

  async loadWorkspace(_path: string): Promise<WorkspaceData> {
    return {
      version: "0.1.0",
      layout: null,
      tabs: [],
      spectra_source: null,
      spectra_count: this.spectra.length,
      processing: {},
      fits: {},
      plot_settings: {},
    };
  }

  async saveWorkspace(_path: string, _data: WorkspaceData): Promise<void> {}

  async getSpectrumList(): Promise<SpectrumMeta[]> {
    return this.spectra.map((s) => ({
      index: s.index,
      name: s.name,
      has_e0: s.e0 !== null,
      has_norm: s.norm !== null,
      has_chi: s.chi !== null,
      has_chir: s.chir_mag !== null,
    }));
  }

  async getSpectrumData(index: number): Promise<SpectrumData> {
    return this.spectra[index] ?? this.spectra[0];
  }

  async removeSpectra(indices: number[]): Promise<number> {
    this.spectra = this.spectra.filter((_, i) => !indices.includes(i));
    return this.spectra.length;
  }

  async findE0(index: number): Promise<number> {
    const spec = this.spectra[index];
    if (!spec) return 8979.0;
    // Simulate finding E0 from max first derivative
    const dmude = derivative(spec.energy!, spec.mu!);
    let maxIdx = 0;
    let maxVal = -Infinity;
    for (let i = 0; i < dmude.length; i++) {
      if (dmude[i] > maxVal) {
        maxVal = dmude[i];
        maxIdx = i;
      }
    }
    spec.e0 = spec.energy![maxIdx];
    return spec.e0;
  }

  async normalize(index: number, opts?: NormOptions): Promise<void> {
    const spec = this.spectra[index];
    if (!spec) return;
    const stored = this.options.get(index) ?? {};
    stored.norm = opts;
    this.options.set(index, stored);

    // Recompute normalization with updated pre/post edge range
    const e0 = opts?.e0 ?? spec.e0 ?? 8979.0;
    const preStart = opts?.pre_edge_start ?? -200;
    const preEnd = opts?.pre_edge_end ?? -30;
    const normStart = opts?.norm_start ?? 150;
    const normEnd = opts?.norm_end ?? 800;
    const energy = spec.energy!;
    const mu = spec.mu!;

    // Fit pre-edge line (linear) over [e0+preStart, e0+preEnd]
    let sx = 0, sy = 0, sxx = 0, sxy = 0, n = 0;
    for (let i = 0; i < energy.length; i++) {
      const rel = energy[i] - e0;
      if (rel >= preStart && rel <= preEnd) {
        sx += energy[i]; sy += mu[i]; sxx += energy[i] * energy[i]; sxy += energy[i] * mu[i]; n++;
      }
    }
    const preSlope = n > 1 ? (n * sxy - sx * sy) / (n * sxx - sx * sx) : 0;
    const preIntercept = n > 0 ? (sy - preSlope * sx) / n : 0;

    // Fit post-edge line over [e0+normStart, e0+normEnd]
    sx = sy = sxx = sxy = n = 0;
    for (let i = 0; i < energy.length; i++) {
      const rel = energy[i] - e0;
      if (rel >= normStart && rel <= normEnd) {
        sx += energy[i]; sy += mu[i]; sxx += energy[i] * energy[i]; sxy += energy[i] * mu[i]; n++;
      }
    }
    const postSlope = n > 1 ? (n * sxy - sx * sy) / (n * sxx - sx * sx) : 0;
    const postIntercept = n > 0 ? (sy - postSlope * sx) / n : 0;

    // Compute edge step at E0
    const preAtE0 = preSlope * e0 + preIntercept;
    const postAtE0 = postSlope * e0 + postIntercept;
    const edgeStep = Math.max(postAtE0 - preAtE0, 0.01);

    // Update arrays
    const newPre: number[] = [];
    const newPost: number[] = [];
    const newNorm: number[] = [];
    for (let i = 0; i < energy.length; i++) {
      const pre = preSlope * energy[i] + preIntercept;
      const post = postSlope * energy[i] + postIntercept;
      newPre.push(pre);
      newPost.push(post);
      newNorm.push((mu[i] - pre) / edgeStep);
    }
    spec.pre_edge = newPre;
    spec.post_edge = newPost;
    spec.norm = newNorm;
    spec.flat = newNorm;
    spec.e0 = e0;
  }

  async calcBackground(index: number, opts?: BgOptions): Promise<void> {
    const spec = this.spectra[index];
    if (!spec) return;
    const stored = this.options.get(index) ?? {};
    stored.bg = opts;
    this.options.set(index, stored);

    // Recompute k-space with updated parameters
    const kw = opts?.kweight ?? 2;
    const kmin = opts?.kmin ?? 0;
    const kmax = opts?.kmax ?? 15;

    const kArr: number[] = [];
    const chi: number[] = [];
    const chiKw: number[] = [];
    for (let i = 0; i < 200; i++) {
      const k = i * 0.05 + 0.5;
      kArr.push(k);
      const chiVal =
        0.8 * Math.exp(-0.006 * k * k) * Math.sin(2 * 2.55 * k + 0.3) +
        0.3 * Math.exp(-0.01 * k * k) * Math.sin(2 * 3.61 * k - 0.2);
      // Zero out chi outside kmin..kmax range
      const inRange = k >= kmin && k <= kmax ? 1 : 0;
      chi.push(chiVal * inRange);
      chiKw.push(chiVal * Math.pow(k, kw) * inRange);
    }

    const dk = 1;
    spec.k = kArr;
    spec.chi = chi;
    spec.chi_kweighted = chiKw;
    spec.kwin = hanningWindow(kArr, Math.max(kmin, 2), Math.min(kmax, 12), dk);
  }

  async fft(index: number, opts?: FFTOptions): Promise<void> {
    const spec = this.spectra[index];
    if (!spec) return;
    const stored = this.options.get(index) ?? {};
    stored.fft = opts;
    this.options.set(index, stored);

    // Recompute R-space with updated window parameters
    const kmin = opts?.kmin ?? 2;
    const kmax = opts?.kmax ?? 12;
    const dk = opts?.dk ?? 1;

    // Update the k-space window
    spec.kwin = hanningWindow(spec.k!, kmin, kmax, dk);

    // Simplified FT — scale peaks based on k-range
    const kRange = kmax - kmin;
    const scaleFactor = kRange / 10; // normalized to default range
    const r: number[] = [];
    const chirMag: number[] = [];
    const chirRe: number[] = [];
    const chirIm: number[] = [];
    for (let i = 0; i < 200; i++) {
      const rVal = i * 0.03;
      r.push(rVal);
      const peak1 = 2.0 * scaleFactor * Math.exp(-8 * (rVal - 1.5) * (rVal - 1.5));
      const peak2 = 1.2 * scaleFactor * Math.exp(-6 * (rVal - 2.55) * (rVal - 2.55));
      const peak3 = 0.4 * scaleFactor * Math.exp(-10 * (rVal - 3.6) * (rVal - 3.6));
      const mag = peak1 + peak2 + peak3;
      chirMag.push(mag);
      chirRe.push(mag * Math.cos(rVal * 5));
      chirIm.push(mag * Math.sin(rVal * 5));
    }
    spec.r = r;
    spec.chir_mag = chirMag;
    spec.chir_re = chirRe;
    spec.chir_im = chirIm;
  }

  async runPipeline(index: number, opts?: PipelineOptions): Promise<void> {
    await this.findE0(index);
    await this.normalize(index, opts?.norm);
    await this.calcBackground(index, opts?.bg);
    await this.fft(index, opts?.fft);
  }

  async batchProcess(indices: number[]): Promise<BatchResult> {
    for (const idx of indices) {
      await this.runPipeline(idx);
    }
    return { succeeded: indices.length, failed: 0, errors: [] };
  }

  async plotSpectrum(index: number, panels: string[]): Promise<PlotResult> {
    const spec = this.spectra[index] ?? this.spectra[0];
    const mode = panels[0] ?? "mu";
    return this.buildPlotResult(spec, mode);
  }

  async plotGroup(indices: number[], panels: string[]): Promise<PlotResult> {
    const mode = panels[0] ?? "mu";
    // Group plots: only main traces, no overlays (too noisy)
    const traces = indices.flatMap((idx) => {
      const spec = this.spectra[idx] ?? this.spectra[0];
      const result = this.buildPlotResult(spec, mode);
      return result.traces.filter((t) => !t.overlay);
    });
    const first = this.buildPlotResult(this.spectra[indices[0]] ?? this.spectra[0], mode);
    return { ...first, traces };
  }

  async plotSvg(_index: number, _panels: string[]): Promise<string[]> {
    return [];
  }

  async plotFit(fitId: string, panel: "k" | "r", includePaths = true): Promise<PlotResult> {
    const fit = this.fitResults.get(fitId);
    if (!fit) {
      throw new Error(`Fit result '${fitId}' not found`);
    }

    if (panel === "k") {
      const traces: PlotTrace[] = [
        { x: fit.k, y: fit.data_chi, label: "Data χ(k)", panel: "k" },
        { x: fit.k, y: fit.model_chi, label: "Model χ(k)", panel: "k" },
      ];
      if (includePaths) {
        for (const path of fit.path_contributions) {
          traces.push({
            x: fit.k,
            y: path.chi,
            label: `Path ${path.label}`,
            panel: "k",
            overlay: "path",
            dash: "dot",
            color: "#22c55e",
          });
        }
      }
      return {
        traces,
        svgs: [],
        x_label: "k (Å⁻¹)",
        y_label: "χ(k)",
      };
    }

    const dataMag = fit.data_chir_re.map((re, i) => {
      const im = fit.data_chir_im[i] ?? 0;
      return Math.sqrt(re * re + im * im);
    });
    const traces: PlotTrace[] = [
      { x: fit.r, y: dataMag, label: "Data |χ(R)|", panel: "r" },
      { x: fit.r, y: fit.model_chir_mag, label: "Model |χ(R)|", panel: "r" },
    ];
    if (includePaths) {
      for (const path of fit.path_contributions) {
        traces.push({
          x: fit.r,
          y: path.chir_mag,
          label: `Path ${path.label}`,
          panel: "r",
          overlay: "path",
          dash: "dot",
          color: "#f59e0b",
        });
      }
    }
    return {
      traces,
      svgs: [],
      x_label: "R (Å)",
      y_label: "|χ(R)|",
    };
  }

  async runFeffPaths(config: FeffRunConfig): Promise<FeffRunResultDto> {
    const workspace = config.workspace_dir.trim();
    if (!workspace) {
      throw new Error("Workspace directory is required");
    }
    if (!config.executable_path.trim()) {
      throw new Error("FEFF executable path is required");
    }

    const seed = fnv1a(JSON.stringify(config));
    const pathCount = 2 + (seed % 3);
    const normalizedWorkspace = workspace.endsWith("/") ? workspace.slice(0, -1) : workspace;
    const path_files = Array.from({ length: pathCount }, (_, i) => {
      const fileId = String(i + 1).padStart(4, "0");
      return `${normalizedWorkspace}/feff${fileId}.dat`;
    });

    return {
      mode: "Feff85LModules",
      workspace_dir: workspace,
      feffinp_path: config.feffinp?.trim() || `${normalizedWorkspace}/feff.inp`,
      modules: [
        { module: "rdinp", executable: config.executable_path },
        { module: "pot", executable: config.executable_path },
        { module: "xsph", executable: config.executable_path },
        { module: "pathfinder", executable: config.executable_path },
        { module: "genfmt", executable: config.executable_path },
        { module: "ff2x", executable: config.executable_path },
      ],
      logs: [
        `${normalizedWorkspace}/log1.dat`,
        `${normalizedWorkspace}/log2.dat`,
      ],
      path_files,
    };
  }

  async runFeffFit(config: FeffFitConfig): Promise<FeffFitResultDto> {
    const spectrum = this.spectra[config.data_index] ?? this.spectra[0];
    const k = spectrum.k ?? [];
    const dataChi = spectrum.chi ?? spectrum.chi_kweighted ?? [];
    const r = spectrum.r ?? [];
    const dataRe = spectrum.chir_re ?? [];
    const dataIm = spectrum.chir_im ?? [];

    if (k.length === 0 || dataChi.length === 0) {
      throw new Error("Selected spectrum has no χ(k) data. Run processing first.");
    }

    const seed = fnv1a(JSON.stringify(config));
    const rand = buildDeterministicRng(seed);
    const fitId = `mock-fit-${seed.toString(16)}`;

    const modelChi = dataChi.map((value, i) => {
      const phase = i * 0.02 + rand() * 0.5;
      return value * (0.96 + 0.06 * Math.sin(phase));
    });

    const modelRe = dataRe.map((value, i) => value * (0.95 + 0.05 * Math.cos(i * 0.015)));
    const modelIm = dataIm.map((value, i) => value * (0.95 + 0.05 * Math.sin(i * 0.018)));
    const modelMag = modelRe.map((value, i) => {
      const im = modelIm[i] ?? 0;
      return Math.sqrt(value * value + im * im);
    });

    const variables = (config.variables.length > 0
      ? config.variables
      : [
          { name: "amp", value: 1.0, vary: true, min: 0.0, max: 2.0 },
          { name: "de0", value: 0.0, vary: true, min: -10, max: 10 },
          { name: "sig2", value: 0.003, vary: true, min: 0.0, max: 0.02 },
          { name: "dr", value: 0.0, vary: true, min: -0.1, max: 0.1 },
        ]) as Array<{
      name: string;
      value: number;
      vary: boolean;
      min?: number;
      max?: number;
      expr?: string;
    }>;

    const variableResults = variables.map((variable) => {
      const jitter = variable.vary ? (rand() - 0.5) * 0.04 : 0;
      const value = variable.value * (1 + jitter);
      return {
        name: variable.name,
        value,
        stderr: variable.vary ? Math.max(Math.abs(value) * 0.02, 1e-6) : null,
        vary: variable.vary,
        init_value: variable.value,
      };
    });

    const activePaths = config.paths.filter((p) => p.use_path);
    const pathContributions = activePaths.map((path, idx) => {
      const weight = 0.35 / (idx + 1);
      return {
        label: path.label || `path-${idx + 1}`,
        chi: modelChi.map((v, i) => v * weight * (1 + 0.02 * Math.sin(i * 0.03 + idx))),
        chir_re: modelRe.map((v) => v * weight),
        chir_im: modelIm.map((v) => v * weight),
        chir_mag: modelMag.map((v) => v * weight),
      };
    });

    const fit: FeffFitResultDto = {
      id: fitId,
      chi_square: 2.0 + rand() * 8,
      reduced_chi_square: 1.0 + rand() * 2,
      r_factor: 0.01 + rand() * 0.03,
      n_vary: variableResults.filter((v) => v.vary).length,
      n_data: k.length,
      n_idp: Math.max(10, 80 + rand() * 40),
      variables: variableResults,
      correlation: null,
      k: [...k],
      data_chi: [...dataChi],
      model_chi: modelChi,
      r: [...r],
      data_chir_re: [...dataRe],
      data_chir_im: [...dataIm],
      model_chir_re: modelRe,
      model_chir_im: modelIm,
      model_chir_mag: modelMag,
      path_contributions: pathContributions,
      warnings: [],
    };

    this.fitResults.set(fit.id, fit);
    return fit;
  }

  async getFitResult(fitId: string): Promise<FeffFitResultDto> {
    const fit = this.fitResults.get(fitId);
    if (!fit) {
      throw new Error(`Fit result '${fitId}' not found`);
    }
    return fit;
  }

  async listFitResults(): Promise<string[]> {
    return Array.from(this.fitResults.keys()).sort();
  }

  async getWorkspacePath(): Promise<string | null> {
    return null;
  }

  private buildPlotResult(spec: SpectrumData, mode: string): PlotResult {
    switch (mode) {
      case "mu":
        return this.buildMuPlot(spec);
      case "norm":
        return this.buildNormPlot(spec);
      case "k":
        return this.buildKPlot(spec);
      case "r":
        return this.buildRPlot(spec);
      default:
        return { traces: [], svgs: [], x_label: "", y_label: "" };
    }
  }

  private buildMuPlot(spec: SpectrumData): PlotResult {
    const traces: PlotTrace[] = [
      { x: spec.energy!, y: spec.mu!, label: spec.name, panel: "mu" },
    ];
    // Overlay: derivative dμ/dE (scaled to fit on same axis)
    if (spec.energy && spec.mu) {
      const dmude = derivative(spec.energy, spec.mu);
      const maxDmu = Math.max(...dmude.map(Math.abs));
      const muRange = Math.max(...spec.mu) - Math.min(...spec.mu);
      const scale = maxDmu > 0 ? (muRange * 0.4) / maxDmu : 1;
      const base = Math.min(...spec.mu);
      traces.push({
        x: spec.energy,
        y: dmude.map((d) => d * scale + base),
        label: "dμ/dE",
        panel: "mu",
        overlay: "dmude",
        dash: "dot",
        color: "#ef4444",
      });
    }
    // Overlay: pre-edge line
    if (spec.energy && spec.pre_edge) {
      traces.push({
        x: spec.energy,
        y: spec.pre_edge,
        label: "Pre-edge",
        panel: "mu",
        overlay: "preedge",
        dash: "dash",
        color: "#f97316",
      });
    }
    // Overlay: post-edge line
    if (spec.energy && spec.post_edge) {
      traces.push({
        x: spec.energy,
        y: spec.post_edge,
        label: "Post-edge",
        panel: "mu",
        overlay: "postedge",
        dash: "dash",
        color: "#22c55e",
      });
    }
    // Overlay: E0 vertical marker
    if (spec.e0 !== null && spec.mu) {
      const yMin = Math.min(...spec.mu);
      const yMax = Math.max(...spec.mu);
      traces.push({
        x: [spec.e0, spec.e0],
        y: [yMin, yMax],
        label: `E0 = ${spec.e0.toFixed(1)} eV`,
        panel: "mu",
        overlay: "e0marker",
        dash: "dashdot",
        color: "#06b6d4",
      });
    }
    return { traces, svgs: [], x_label: "Energy (eV)", y_label: "\u03BC(E)" };
  }

  private buildNormPlot(spec: SpectrumData): PlotResult {
    const traces: PlotTrace[] = [
      { x: spec.energy!, y: spec.norm!, label: spec.name, panel: "norm" },
    ];
    // Overlay: flattened
    if (spec.energy && spec.flat) {
      traces.push({
        x: spec.energy,
        y: spec.flat,
        label: "Flattened",
        panel: "norm",
        overlay: "flattened",
        dash: "dot",
        color: "#a855f7",
      });
    }
    // Overlay: derivative of norm
    if (spec.energy && spec.norm) {
      const dnormde = derivative(spec.energy, spec.norm);
      const maxD = Math.max(...dnormde.map(Math.abs));
      const scale = maxD > 0 ? 0.4 / maxD : 1;
      traces.push({
        x: spec.energy,
        y: dnormde.map((d) => d * scale),
        label: "dNorm/dE",
        panel: "norm",
        overlay: "dnormde",
        dash: "dot",
        color: "#ef4444",
      });
    }
    // Overlay: pre-edge (normalized space: = 0 line essentially)
    if (spec.energy && spec.pre_edge && spec.post_edge) {
      const e0 = spec.e0 ?? spec.energy[Math.floor(spec.energy.length / 3)];
      const preAtE0 = spec.pre_edge[spec.energy.findIndex((e) => e >= e0)] ?? 0;
      const postAtE0 = spec.post_edge[spec.energy.findIndex((e) => e >= e0)] ?? 1;
      const step = Math.max(postAtE0 - preAtE0, 0.01);
      traces.push({
        x: spec.energy,
        y: spec.pre_edge.map((p) => (p - spec.pre_edge![0]) / step),
        label: "Pre-edge",
        panel: "norm",
        overlay: "preedge",
        dash: "dash",
        color: "#f97316",
      });
      traces.push({
        x: spec.energy,
        y: spec.post_edge.map((p) => (p - preAtE0) / step),
        label: "Post-edge",
        panel: "norm",
        overlay: "postedge",
        dash: "dash",
        color: "#22c55e",
      });
    }
    return { traces, svgs: [], x_label: "Energy (eV)", y_label: "Normalized \u03BC(E)" };
  }

  private buildKPlot(spec: SpectrumData): PlotResult {
    const traces: PlotTrace[] = [
      { x: spec.k!, y: spec.chi_kweighted!, label: spec.name, panel: "k" },
    ];
    // Overlay: |chi(k)| magnitude envelope
    if (spec.k && spec.chi_kweighted) {
      const mag = spec.chi_kweighted.map(Math.abs);
      traces.push({
        x: spec.k,
        y: mag,
        label: "|\u03C7(k)|",
        panel: "k",
        overlay: "chimag",
        dash: "dot",
        color: "#94a3b8",
      });
      // Negative envelope
      traces.push({
        x: spec.k,
        y: mag.map((v) => -v),
        label: "-|\u03C7(k)|",
        panel: "k",
        overlay: "chimag",
        dash: "dot",
        color: "#94a3b8",
      });
    }
    // Overlay: k-window
    if (spec.k && spec.kwin) {
      // Scale window to chi range for visibility
      const maxChi = Math.max(...spec.chi_kweighted!.map(Math.abs));
      traces.push({
        x: spec.k,
        y: spec.kwin.map((w) => w * maxChi),
        label: "Window",
        panel: "k",
        overlay: "window",
        dash: "dash",
        color: "#eab308",
      });
    }
    return { traces, svgs: [], x_label: "k (\u00C5\u207B\u00B9)", y_label: "k\u00B2\u03C7(k)" };
  }

  private buildRPlot(spec: SpectrumData): PlotResult {
    const traces: PlotTrace[] = [
      { x: spec.r!, y: spec.chir_mag!, label: spec.name, panel: "r" },
    ];
    // Overlay: Re[chi(R)]
    if (spec.r && spec.chir_re) {
      traces.push({
        x: spec.r,
        y: spec.chir_re,
        label: "Re[\u03C7(R)]",
        panel: "r",
        overlay: "chir_re",
        dash: "dash",
        color: "#3b82f6",
      });
    }
    // Overlay: Im[chi(R)]
    if (spec.r && spec.chir_im) {
      traces.push({
        x: spec.r,
        y: spec.chir_im,
        label: "Im[\u03C7(R)]",
        panel: "r",
        overlay: "chir_im",
        dash: "dash",
        color: "#ef4444",
      });
    }
    // Overlay: R-space window (simplified — not stored, use a default)
    if (spec.r) {
      const rmin = 1.0, rmax = 3.0, dr = 0.2;
      const maxMag = Math.max(...spec.chir_mag!);
      const rwin = spec.r.map((rv) => {
        if (rv < rmin - dr || rv > rmax + dr) return 0;
        if (rv < rmin + dr) return 0.5 * (1 + Math.cos(Math.PI * (rv - rmin - dr) / (2 * dr)));
        if (rv > rmax - dr) return 0.5 * (1 + Math.cos(Math.PI * (rv - rmax + dr) / (2 * dr)));
        return 1;
      });
      traces.push({
        x: spec.r,
        y: rwin.map((w) => w * maxMag),
        label: "Window",
        panel: "r",
        overlay: "window",
        dash: "dash",
        color: "#eab308",
      });
    }
    return { traces, svgs: [], x_label: "R (\u00C5)", y_label: "|\u03C7(R)| (\u00C5\u207B\u00B3)" };
  }
}
