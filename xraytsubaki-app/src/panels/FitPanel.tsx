import { useCallback, useEffect, useMemo, useState } from "react";
import Plot from "react-plotly.js";
import { Plus, Play, Trash2 } from "lucide-react";
import { useWorkspaceStore } from "@/stores/workspace";
import {
  useFitPlot,
  useFitResult,
  useFitResultList,
  useRunFeffFit,
  useRunFeffPaths,
} from "@/hooks/useFit";
import type { FeffPathConfig, FitVariableConfig, FitTransformConfig } from "@/backend/types";
import { addLog } from "@/panels/LogPanel";

const DEFAULT_TRANSFORM: FitTransformConfig = {
  kmin: 3,
  kmax: 14,
  kweight: 2,
  dk: 4,
  rmin: 1.5,
  rmax: 3.0,
};

const DEFAULT_VARIABLES: FitVariableConfig[] = [
  { name: "amp", value: 1, vary: true, min: 0, max: 2 },
  { name: "de0", value: 0, vary: true, min: -10, max: 10 },
  { name: "sig2", value: 0.003, vary: true, min: 0, max: 0.02 },
  { name: "dr", value: 0, vary: true, min: -0.1, max: 0.1 },
];

function parseOptionalNumber(value: string): number | undefined {
  const trimmed = value.trim();
  if (trimmed === "") return undefined;
  const parsed = Number(trimmed);
  return Number.isFinite(parsed) ? parsed : undefined;
}

function pathFileName(path: string): string {
  const normalized = path.replaceAll("\\", "/");
  const idx = normalized.lastIndexOf("/");
  return idx >= 0 ? normalized.slice(idx + 1) : normalized;
}

export function FitPanel() {
  const activeIndex = useWorkspaceStore((s) => s.activeIndex);

  const runFeffPaths = useRunFeffPaths();
  const runFeffFit = useRunFeffFit();
  const { data: fitIds } = useFitResultList();

  const [selectedFitId, setSelectedFitId] = useState<string | null>(null);
  const [fitPanel, setFitPanel] = useState<"k" | "r">("r");

  const [runConfig, setRunConfig] = useState({
    executable_path: "",
    workspace_dir: "",
    feffinp: "",
    timeout_sec: "180",
    use_sfconv: false,
  });

  const [paths, setPaths] = useState<FeffPathConfig[]>([]);
  const [variables, setVariables] = useState<FitVariableConfig[]>(DEFAULT_VARIABLES);
  const [transform, setTransform] = useState<FitTransformConfig>(DEFAULT_TRANSFORM);

  useEffect(() => {
    if (!fitIds || fitIds.length === 0) {
      setSelectedFitId(null);
      return;
    }
    if (!selectedFitId || !fitIds.includes(selectedFitId)) {
      setSelectedFitId(fitIds[fitIds.length - 1]);
    }
  }, [fitIds, selectedFitId]);

  const selectedFit = useFitResult(selectedFitId);
  const fitPlot = useFitPlot(selectedFitId, fitPanel, true);

  const plotData = useMemo(() => {
    const traces = fitPlot.data?.traces ?? [];
    return traces.map((trace) => ({
      x: trace.x,
      y: trace.y,
      name: trace.label,
      type: "scattergl" as const,
      mode: "lines" as const,
      line: {
        dash: (trace.dash ?? "solid") as Plotly.Dash,
        width: trace.overlay ? 1.5 : 2,
        ...(trace.color ? { color: trace.color } : {}),
      },
      ...(trace.overlay ? { opacity: 0.8 } : {}),
    }));
  }, [fitPlot.data?.traces]);

  const runFit = useCallback(async () => {
    if (activeIndex === null) {
      addLog("warn", "Fit skipped: select a spectrum first");
      return;
    }

    const activePaths = paths.filter((path) => path.use_path);
    if (activePaths.length === 0) {
      addLog("warn", "Fit skipped: no active FEFF paths");
      return;
    }

    try {
      const result = await runFeffFit.mutateAsync({
        data_index: activeIndex,
        paths,
        variables,
        transform,
      });
      setSelectedFitId(result.id);
      addLog(
        "info",
        `Fit completed (${result.id}): R-factor=${result.r_factor.toFixed(4)}, χ²=${result.reduced_chi_square.toFixed(4)}`,
      );
    } catch (error) {
      addLog("error", `Fit failed: ${String(error)}`);
    }
  }, [activeIndex, paths, runFeffFit, transform, variables]);

  useEffect(() => {
    const onToolbarRequest = () => {
      if (!runFeffFit.isPending) {
        void runFit();
      }
    };
    window.addEventListener("xraytsubaki:fit-run-request", onToolbarRequest);
    return () => window.removeEventListener("xraytsubaki:fit-run-request", onToolbarRequest);
  }, [runFit, runFeffFit.isPending]);

  const runFeff = useCallback(async () => {
    try {
      const result = await runFeffPaths.mutateAsync({
        executable_path: runConfig.executable_path.trim() || undefined,
        workspace_dir: runConfig.workspace_dir.trim(),
        feffinp: runConfig.feffinp.trim() || undefined,
        timeout_sec: parseOptionalNumber(runConfig.timeout_sec),
        use_sfconv: runConfig.use_sfconv,
      });

      const generated = result.path_files.map((path) => ({
        label: pathFileName(path),
        feff_dat_path: path,
        use_path: true,
        s02: "amp",
        e0: "de0",
        deltar: "dr",
        sigma2: "sig2",
      }));
      setPaths(generated);
      addLog("info", `FEFF run completed: ${generated.length} path(s) detected`);
    } catch (error) {
      addLog("error", `FEFF run failed: ${String(error)}`);
    }
  }, [runConfig, runFeffPaths]);

  return (
    <div className="flex flex-col h-full min-h-0">
      <div className="px-3 py-2 border-b border-slate-700">
        <div className="text-sm font-medium text-slate-200">FEFF Fitting</div>
        <div className="text-xs text-slate-500 mt-0.5">
          Run FEFF, configure path/variable constraints, then fit in R-space. Leaving executable
          empty uses FEFF10 pipeline; specifying an executable uses FEFF85 module fallback.
        </div>
      </div>

      <div className="flex-1 overflow-y-auto p-3 space-y-3">
        <section className="rounded border border-slate-700 bg-slate-800/40 p-2.5 space-y-2">
          <div className="text-xs font-medium text-slate-300">1. FEFF Run</div>
          <div className="grid grid-cols-1 gap-1.5">
            <TextInput
              label="Executable (optional)"
              value={runConfig.executable_path}
              onChange={(value) => setRunConfig((prev) => ({ ...prev, executable_path: value }))}
              placeholder="FEFF10 default (leave empty)"
            />
            <TextInput
              label="Workspace dir"
              value={runConfig.workspace_dir}
              onChange={(value) => setRunConfig((prev) => ({ ...prev, workspace_dir: value }))}
              placeholder="/path/to/feff/workspace"
            />
            <TextInput
              label="feff.inp (optional)"
              value={runConfig.feffinp}
              onChange={(value) => setRunConfig((prev) => ({ ...prev, feffinp: value }))}
              placeholder="/path/to/feff.inp"
            />
            <TextInput
              label="Timeout (sec)"
              value={runConfig.timeout_sec}
              onChange={(value) => setRunConfig((prev) => ({ ...prev, timeout_sec: value }))}
              placeholder="180"
            />
            <label className="flex items-center gap-2 text-xs text-slate-300 py-1">
              <input
                type="checkbox"
                checked={runConfig.use_sfconv}
                onChange={(e) =>
                  setRunConfig((prev) => ({ ...prev, use_sfconv: e.target.checked }))
                }
                className="w-3.5 h-3.5 accent-blue-500"
              />
              <span>Enable SFCONV (FEFF10 mode only)</span>
            </label>
          </div>
          <button
            className="h-7 px-2.5 text-xs rounded bg-blue-600 hover:bg-blue-500 text-white disabled:opacity-50 disabled:cursor-not-allowed"
            onClick={() => void runFeff()}
            disabled={runFeffPaths.isPending}
          >
            {runFeffPaths.isPending ? "Running FEFF..." : "Run FEFF and Load Paths"}
          </button>
        </section>

        <section className="rounded border border-slate-700 bg-slate-800/40 p-2.5 space-y-2">
          <div className="flex items-center justify-between">
            <div className="text-xs font-medium text-slate-300">2. FEFF Paths</div>
            <button
              className="h-6 px-2 text-[11px] rounded border border-slate-600 text-slate-300 hover:bg-slate-700 flex items-center gap-1"
              onClick={() =>
                setPaths((prev) => [
                  ...prev,
                  {
                    label: `path-${prev.length + 1}`,
                    feff_dat_path: "",
                    use_path: true,
                    s02: "amp",
                    e0: "de0",
                    deltar: "dr",
                    sigma2: "sig2",
                  },
                ])
              }
            >
              <Plus size={12} />
              Add Path
            </button>
          </div>
          {paths.length === 0 ? (
            <div className="text-xs text-slate-500">
              No paths configured. Run FEFF above or add path rows manually.
            </div>
          ) : (
            <div className="space-y-1.5">
              {paths.map((path, index) => (
                <div key={`${path.feff_dat_path}-${index}`} className="rounded border border-slate-700 p-2 space-y-1.5">
                  <div className="flex items-center gap-2">
                    <input
                      type="checkbox"
                      checked={path.use_path}
                      onChange={(e) =>
                        setPaths((prev) =>
                          prev.map((item, i) =>
                            i === index ? { ...item, use_path: e.target.checked } : item,
                          ),
                        )
                      }
                      className="w-3.5 h-3.5 accent-blue-500"
                    />
                    <input
                      className="flex-1 bg-slate-900 border border-slate-700 rounded px-2 py-1 text-xs text-slate-200 focus:outline-none focus:border-blue-500"
                      value={path.label}
                      onChange={(e) =>
                        setPaths((prev) =>
                          prev.map((item, i) =>
                            i === index ? { ...item, label: e.target.value } : item,
                          ),
                        )
                      }
                      placeholder="Path label"
                    />
                    <button
                      className="w-6 h-6 rounded hover:bg-slate-700 text-slate-400 hover:text-rose-300 flex items-center justify-center"
                      onClick={() => setPaths((prev) => prev.filter((_, i) => i !== index))}
                      title="Remove path"
                    >
                      <Trash2 size={13} />
                    </button>
                  </div>
                  <input
                    className="w-full bg-slate-900 border border-slate-700 rounded px-2 py-1 text-xs text-slate-200 focus:outline-none focus:border-blue-500"
                    value={path.feff_dat_path}
                    onChange={(e) =>
                      setPaths((prev) =>
                        prev.map((item, i) =>
                          i === index ? { ...item, feff_dat_path: e.target.value } : item,
                        ),
                      )
                    }
                    placeholder="/path/to/feff0001.dat"
                  />
                  <div className="grid grid-cols-4 gap-1.5">
                    <CompactInput
                      label="s02"
                      value={path.s02}
                      onChange={(value) =>
                        setPaths((prev) =>
                          prev.map((item, i) => (i === index ? { ...item, s02: value } : item)),
                        )
                      }
                    />
                    <CompactInput
                      label="e0"
                      value={path.e0}
                      onChange={(value) =>
                        setPaths((prev) =>
                          prev.map((item, i) => (i === index ? { ...item, e0: value } : item)),
                        )
                      }
                    />
                    <CompactInput
                      label="deltar"
                      value={path.deltar}
                      onChange={(value) =>
                        setPaths((prev) =>
                          prev.map((item, i) =>
                            i === index ? { ...item, deltar: value } : item,
                          ),
                        )
                      }
                    />
                    <CompactInput
                      label="sigma2"
                      value={path.sigma2}
                      onChange={(value) =>
                        setPaths((prev) =>
                          prev.map((item, i) =>
                            i === index ? { ...item, sigma2: value } : item,
                          ),
                        )
                      }
                    />
                  </div>
                </div>
              ))}
            </div>
          )}
        </section>

        <section className="rounded border border-slate-700 bg-slate-800/40 p-2.5 space-y-2">
          <div className="text-xs font-medium text-slate-300">3. Variables</div>
          <div className="space-y-1">
            {variables.map((variable, index) => (
              <div key={`${variable.name}-${index}`} className="grid grid-cols-[1fr_90px_46px_80px_80px_24px] gap-1 items-center">
                <input
                  className="bg-slate-900 border border-slate-700 rounded px-2 py-1 text-xs text-slate-200 focus:outline-none focus:border-blue-500"
                  value={variable.name}
                  onChange={(e) =>
                    setVariables((prev) =>
                      prev.map((item, i) => (i === index ? { ...item, name: e.target.value } : item)),
                    )
                  }
                  placeholder="name"
                />
                <input
                  className="bg-slate-900 border border-slate-700 rounded px-2 py-1 text-xs text-slate-200 focus:outline-none focus:border-blue-500"
                  type="number"
                  value={Number.isFinite(variable.value) ? variable.value : 0}
                  onChange={(e) =>
                    setVariables((prev) =>
                      prev.map((item, i) =>
                        i === index ? { ...item, value: Number(e.target.value) || 0 } : item,
                      ),
                    )
                  }
                />
                <label className="text-[11px] text-slate-400 flex items-center gap-1">
                  <input
                    type="checkbox"
                    checked={variable.vary}
                    onChange={(e) =>
                      setVariables((prev) =>
                        prev.map((item, i) => (i === index ? { ...item, vary: e.target.checked } : item)),
                      )
                    }
                    className="w-3.5 h-3.5 accent-blue-500"
                  />
                  vary
                </label>
                <input
                  className="bg-slate-900 border border-slate-700 rounded px-2 py-1 text-xs text-slate-200 focus:outline-none focus:border-blue-500"
                  value={variable.min ?? ""}
                  onChange={(e) =>
                    setVariables((prev) =>
                      prev.map((item, i) =>
                        i === index ? { ...item, min: parseOptionalNumber(e.target.value) } : item,
                      ),
                    )
                  }
                  placeholder="min"
                />
                <input
                  className="bg-slate-900 border border-slate-700 rounded px-2 py-1 text-xs text-slate-200 focus:outline-none focus:border-blue-500"
                  value={variable.max ?? ""}
                  onChange={(e) =>
                    setVariables((prev) =>
                      prev.map((item, i) =>
                        i === index ? { ...item, max: parseOptionalNumber(e.target.value) } : item,
                      ),
                    )
                  }
                  placeholder="max"
                />
                <button
                  className="w-6 h-6 rounded hover:bg-slate-700 text-slate-400 hover:text-rose-300 flex items-center justify-center"
                  onClick={() => setVariables((prev) => prev.filter((_, i) => i !== index))}
                >
                  <Trash2 size={12} />
                </button>
              </div>
            ))}
          </div>
          <button
            className="h-6 px-2 text-[11px] rounded border border-slate-600 text-slate-300 hover:bg-slate-700"
            onClick={() =>
              setVariables((prev) => [...prev, { name: "", value: 0, vary: true }])
            }
          >
            Add Variable
          </button>
        </section>

        <section className="rounded border border-slate-700 bg-slate-800/40 p-2.5 space-y-2">
          <div className="text-xs font-medium text-slate-300">4. Transform</div>
          <div className="grid grid-cols-3 gap-1.5">
            <NumberInput
              label="kmin"
              value={transform.kmin}
              onChange={(value) => setTransform((prev) => ({ ...prev, kmin: value }))}
            />
            <NumberInput
              label="kmax"
              value={transform.kmax}
              onChange={(value) => setTransform((prev) => ({ ...prev, kmax: value }))}
            />
            <NumberInput
              label="kweight"
              value={transform.kweight}
              onChange={(value) => setTransform((prev) => ({ ...prev, kweight: value }))}
            />
            <NumberInput
              label="dk"
              value={transform.dk}
              onChange={(value) => setTransform((prev) => ({ ...prev, dk: value }))}
            />
            <NumberInput
              label="rmin"
              value={transform.rmin}
              onChange={(value) => setTransform((prev) => ({ ...prev, rmin: value }))}
            />
            <NumberInput
              label="rmax"
              value={transform.rmax}
              onChange={(value) => setTransform((prev) => ({ ...prev, rmax: value }))}
            />
          </div>
        </section>

        <button
          className="h-8 px-3 text-xs rounded bg-emerald-600 hover:bg-emerald-500 text-white disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-1.5"
          onClick={() => void runFit()}
          disabled={activeIndex === null || runFeffFit.isPending}
        >
          <Play size={13} />
          {runFeffFit.isPending ? "Running Fit..." : "Run Fit"}
        </button>

        <section className="rounded border border-slate-700 bg-slate-800/40 p-2.5 space-y-2">
          <div className="flex items-center gap-1.5">
            <span className="text-xs font-medium text-slate-300">Fit Results</span>
            <select
              className="ml-auto h-7 bg-slate-900 border border-slate-700 rounded px-2 text-xs text-slate-200 min-w-[190px]"
              value={selectedFitId ?? ""}
              onChange={(e) => setSelectedFitId(e.target.value || null)}
            >
              <option value="">Select fit</option>
              {(fitIds ?? []).map((id) => (
                <option key={id} value={id}>
                  {id}
                </option>
              ))}
            </select>
          </div>

          {selectedFit.data ? (
            <div className="grid grid-cols-3 gap-2 text-[11px] text-slate-400">
              <Stat label="R-factor" value={selectedFit.data.r_factor.toFixed(5)} />
              <Stat label="Reduced χ²" value={selectedFit.data.reduced_chi_square.toFixed(5)} />
              <Stat label="N varying" value={String(selectedFit.data.n_vary)} />
            </div>
          ) : (
            <div className="text-xs text-slate-500">No fit selected.</div>
          )}

          <div className="flex gap-1">
            <button
              className={`h-6 px-2 text-[11px] rounded border ${
                fitPanel === "k"
                  ? "border-blue-500 text-blue-300 bg-blue-500/10"
                  : "border-slate-600 text-slate-400 hover:text-slate-200"
              }`}
              onClick={() => setFitPanel("k")}
            >
              k-space
            </button>
            <button
              className={`h-6 px-2 text-[11px] rounded border ${
                fitPanel === "r"
                  ? "border-blue-500 text-blue-300 bg-blue-500/10"
                  : "border-slate-600 text-slate-400 hover:text-slate-200"
              }`}
              onClick={() => setFitPanel("r")}
            >
              R-space
            </button>
          </div>

          <div className="h-56 rounded border border-slate-700 bg-slate-900/60 overflow-hidden">
            {plotData.length > 0 ? (
              <Plot
                data={plotData}
                layout={{
                  paper_bgcolor: "rgba(0,0,0,0)",
                  plot_bgcolor: "rgba(15,23,42,0.2)",
                  font: { color: "#94a3b8", size: 11 },
                  xaxis: {
                    title: { text: fitPlot.data?.x_label ?? "", font: { color: "#94a3b8", size: 11 } },
                    gridcolor: "#1e293b",
                  },
                  yaxis: {
                    title: { text: fitPlot.data?.y_label ?? "", font: { color: "#94a3b8", size: 11 } },
                    gridcolor: "#1e293b",
                  },
                  margin: { l: 50, r: 10, t: 8, b: 35 },
                  showlegend: true,
                  legend: { font: { color: "#94a3b8", size: 10 }, bgcolor: "rgba(0,0,0,0)" },
                }}
                config={{ responsive: true, displayModeBar: false, displaylogo: false }}
                useResizeHandler
                style={{ width: "100%", height: "100%" }}
              />
            ) : (
              <div className="h-full flex items-center justify-center text-xs text-slate-500">
                Run a fit to view data/model traces.
              </div>
            )}
          </div>

          {selectedFit.data?.warnings.length ? (
            <div className="text-[11px] text-amber-300 space-y-0.5">
              {selectedFit.data.warnings.map((warning, index) => (
                <div key={`${warning}-${index}`}>• {warning}</div>
              ))}
            </div>
          ) : null}
        </section>
      </div>
    </div>
  );
}

function TextInput({
  label,
  value,
  onChange,
  placeholder,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
}) {
  return (
    <label className="space-y-1">
      <div className="text-[11px] text-slate-400">{label}</div>
      <input
        className="w-full bg-slate-900 border border-slate-700 rounded px-2 py-1 text-xs text-slate-200 focus:outline-none focus:border-blue-500"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
      />
    </label>
  );
}

function NumberInput({
  label,
  value,
  onChange,
}: {
  label: string;
  value: number;
  onChange: (value: number) => void;
}) {
  return (
    <label className="space-y-1">
      <div className="text-[11px] text-slate-400">{label}</div>
      <input
        type="number"
        className="w-full bg-slate-900 border border-slate-700 rounded px-2 py-1 text-xs text-slate-200 focus:outline-none focus:border-blue-500"
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
      />
    </label>
  );
}

function CompactInput({
  label,
  value,
  onChange,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
}) {
  return (
    <label className="space-y-1">
      <div className="text-[11px] text-slate-500">{label}</div>
      <input
        className="w-full bg-slate-900 border border-slate-700 rounded px-2 py-1 text-xs text-slate-200 focus:outline-none focus:border-blue-500"
        value={value}
        onChange={(e) => onChange(e.target.value)}
      />
    </label>
  );
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded bg-slate-900/60 border border-slate-700 px-2 py-1">
      <div className="text-slate-500">{label}</div>
      <div className="text-slate-200 font-medium">{value}</div>
    </div>
  );
}
