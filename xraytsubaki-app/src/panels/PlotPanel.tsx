import { useMemo, useState, useCallback, useRef, useEffect } from "react";
import Plot from "react-plotly.js";
import { MousePointer2, Crosshair, ZoomIn, Move, Plus, X } from "lucide-react";
import { useWorkspaceStore } from "@/stores/workspace";
import { usePlotSpectrum, usePlotGroup, usePlotCore } from "@/hooks/usePlot";
import type { PipelineOptions, PlotMode } from "@/backend/types";
import type { CursorTool, PlotGroup, PlotLayout } from "@/stores/workspace";
import { addLog } from "@/panels/LogPanel";

const MODE_LABELS: Record<PlotMode, string> = {
  energy: "Energy",
  mu: "\u03BC(E)",
  norm: "Norm",
  k: "\u03C7(k)",
  r: "\u03C7(R)",
};

const OVERLAYS_BY_MODE: Record<PlotMode, { id: string; label: string }[]> = {
  energy: [
    { id: "flattened", label: "Flattened" },
    { id: "dmude", label: "d\u03BC/dE" },
    { id: "dnormde", label: "dNorm/dE" },
    { id: "preedge", label: "Pre-edge" },
    { id: "postedge", label: "Post-edge" },
    { id: "e0marker", label: "E0 marker" },
  ],
  mu: [
    { id: "dmude", label: "d\u03BC/dE" },
    { id: "preedge", label: "Pre-edge" },
    { id: "postedge", label: "Post-edge" },
    { id: "e0marker", label: "E0 marker" },
  ],
  norm: [
    { id: "flattened", label: "Flattened" },
    { id: "dnormde", label: "dNorm/dE" },
    { id: "preedge", label: "Pre-edge" },
    { id: "postedge", label: "Post-edge" },
  ],
  k: [
    { id: "chimag", label: "|\u03C7(k)|" },
    { id: "window", label: "Window" },
  ],
  r: [
    { id: "chir_re", label: "Re[\u03C7(R)]" },
    { id: "chir_im", label: "Im[\u03C7(R)]" },
    { id: "window", label: "Window" },
  ],
};

const ALL_MODES: PlotMode[] = ["energy", "k", "r"];
const AUTO_CORE_SPECTRA_THRESHOLD = 500;
const AUTO_CORE_POINT_THRESHOLD = 250_000;
const INTERACTIVE_MAX_POINTS_PER_TRACE = 20_000;
const MAIN_TRACE_COLORS: Record<PlotMode, string> = {
  energy: "#4dabf7",
  mu: "#4dabf7",
  norm: "#51cf66",
  k: "#ff8c42",
  r: "#b388ff",
};

function svgToDataUrl(svg: string): string {
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
}

function lttbDownsample(x: number[], y: number[], threshold: number): { x: number[]; y: number[] } {
  const n = Math.min(x.length, y.length);
  if (threshold <= 2 || n <= threshold) {
    return { x: x.slice(0, n), y: y.slice(0, n) };
  }

  const sampledX: number[] = [x[0]];
  const sampledY: number[] = [y[0]];
  const bucketSize = (n - 2) / (threshold - 2);
  let a = 0;

  for (let i = 0; i < threshold - 2; i++) {
    const rangeStart = Math.floor((i + 1) * bucketSize) + 1;
    const rangeEnd = Math.min(Math.floor((i + 2) * bucketSize) + 1, n - 1);

    let avgX = 0;
    let avgY = 0;
    let avgCount = 0;
    for (let j = rangeStart; j < rangeEnd; j++) {
      avgX += x[j];
      avgY += y[j];
      avgCount++;
    }
    if (avgCount > 0) {
      avgX /= avgCount;
      avgY /= avgCount;
    } else {
      avgX = x[n - 1];
      avgY = y[n - 1];
    }

    const bucketStart = Math.floor(i * bucketSize) + 1;
    const bucketEnd = Math.min(Math.floor((i + 1) * bucketSize) + 1, n - 1);

    let maxArea = -1;
    let maxIndex = bucketStart;
    for (let j = bucketStart; j < bucketEnd; j++) {
      const area = Math.abs((x[a] - avgX) * (y[j] - y[a]) - (x[a] - x[j]) * (avgY - y[a]));
      if (area > maxArea) {
        maxArea = area;
        maxIndex = j;
      }
    }

    sampledX.push(x[maxIndex]);
    sampledY.push(y[maxIndex]);
    a = maxIndex;
  }

  sampledX.push(x[n - 1]);
  sampledY.push(y[n - 1]);
  return { x: sampledX, y: sampledY };
}

/* ═══════════════════════════════════════════════════
   Main PlotPanel — toolbar + editor group grid
   ═══════════════════════════════════════════════════ */

export function PlotPanel() {
  return (
    <div className="flex-1 flex flex-col min-h-0">
      <PlotToolbar />
      <PlotGrid />
    </div>
  );
}

/* ─── Plot Toolbar: cursor tools + pick indicator + render toggle ─── */

function PlotToolbar() {
  const {
    cursorTool,
    setCursorTool,
    pickTarget,
    setPickTarget,
    renderMode,
    renderModeSource,
    setRenderMode,
    enableRenderModeAuto,
    coreFormat,
    setCoreFormat,
  } = useWorkspaceStore();

  const handleCursorTool = (tool: CursorTool) => {
    if (renderMode === "core" && tool !== "select") {
      setRenderMode("interactive");
    }
    setCursorTool(tool);
    if (tool !== "pick") setPickTarget(null);
  };

  const tools: { tool: CursorTool; icon: React.ReactNode; label: string }[] = [
    { tool: "select", icon: <MousePointer2 size={14} />, label: "Select" },
    { tool: "pick", icon: <Crosshair size={14} />, label: "Pick value" },
    { tool: "zoom", icon: <ZoomIn size={14} />, label: "Zoom" },
    { tool: "pan", icon: <Move size={14} />, label: "Pan" },
  ];

  return (
    <div className="flex items-center gap-0.5 px-2 py-1 bg-[#151515] shrink-0">
      <div className="flex gap-px">
        {tools.map(({ tool, icon, label }) => (
          <button
            key={tool}
            className={`w-7 h-6 flex items-center justify-center rounded transition-colors ${
              cursorTool === tool
                ? "text-blue-400 bg-blue-500/15 border border-blue-500/50"
                : "text-[#888] hover:text-[#d0d0d0] border border-transparent"
            }`}
            onClick={() => handleCursorTool(tool)}
            title={label}
          >
            {icon}
          </button>
        ))}
      </div>

      {pickTarget && (
        <span className="ml-2 text-[11px] text-blue-400 bg-blue-500/10 px-2 py-0.5 rounded">
          Picking: {pickTarget}
        </span>
      )}

      <div className="flex-1" />

      <div className="flex border border-[#343434] rounded overflow-hidden">
        <button
          className={`px-2.5 py-0.5 text-[11px] border-r border-slate-600 transition-colors ${
            renderModeSource === "auto"
              ? "text-emerald-300 bg-emerald-500/10"
              : "text-[#888] hover:text-[#d0d0d0]"
          }`}
          onClick={enableRenderModeAuto}
          title="Enable automatic render mode switching"
        >
          Auto
        </button>
        <button
          className={`px-2.5 py-0.5 text-[11px] transition-colors ${
            renderMode === "interactive"
              ? "text-blue-400 bg-blue-500/15"
              : "text-[#888] hover:text-[#d0d0d0]"
          }`}
          onClick={() => setRenderMode("interactive")}
        >
          Interactive
        </button>
        <button
          className={`px-2.5 py-0.5 text-[11px] border-l border-slate-600 transition-colors ${
            renderMode === "core"
              ? "text-blue-400 bg-blue-500/15"
              : "text-[#888] hover:text-[#d0d0d0]"
          }`}
          onClick={() => setRenderMode("core")}
        >
          Core
        </button>
      </div>

      {renderMode === "core" && (
        <div className="flex border border-[#343434] rounded overflow-hidden ml-1">
          <button
            className={`px-2.5 py-0.5 text-[11px] transition-colors ${
              coreFormat === "png"
                ? "text-blue-400 bg-blue-500/15"
                : "text-[#888] hover:text-[#d0d0d0]"
            }`}
            onClick={() => setCoreFormat("png")}
            title="Core render format: PNG (default)"
          >
            PNG
          </button>
          <button
            className={`px-2.5 py-0.5 text-[11px] border-l border-slate-600 transition-colors ${
              coreFormat === "svg"
                ? "text-blue-400 bg-blue-500/15"
                : "text-[#888] hover:text-[#d0d0d0]"
            }`}
            onClick={() => setCoreFormat("svg")}
            title="Core render format: SVG"
          >
            SVG
          </button>
        </div>
      )}
    </div>
  );
}

/* ─── Plot Grid: CSS grid of editor groups ─── */

function PlotGrid() {
  const { plotGroups, plotLayout } = useWorkspaceStore();

  const gridClass: Record<PlotLayout, string> = {
    "1x1": "grid-cols-1 grid-rows-1",
    "1x2": "grid-cols-2 grid-rows-1",
    "2x1": "grid-cols-1 grid-rows-2",
    "2x2": "grid-cols-2 grid-rows-2",
  };

  return (
    <div className={`flex-1 grid gap-2 bg-[#0d0d0d] p-2 min-h-0 ${gridClass[plotLayout]}`}>
      {plotGroups.map((group) => (
        <EditorGroup key={group.id} group={group} />
      ))}
    </div>
  );
}

/* ─── Editor Group: tab bar + overlay bar + plot area ─── */

function EditorGroup({ group }: { group: PlotGroup }) {
  const {
    activeGroupId,
    setActiveGroup,
    setGroupActiveMode,
    energyMain,
    setEnergyMain,
    splitGroup,
    closeGroup,
    addPlotTab,
    removePlotTab,
    plotGroups,
  } = useWorkspaceStore();

  const isActive = activeGroupId === group.id;
  const canClose = plotGroups.length > 1;
  const canSplit = plotGroups.length < 4;

  const [enabledOverlays, setEnabledOverlays] = useState<Set<string>>(new Set());
  const [showAddMenu, setShowAddMenu] = useState(false);
  const addMenuRef = useRef<HTMLDivElement>(null);

  const toggleOverlay = useCallback(
    (id: string) => {
      setEnabledOverlays((prev) => {
        const next = new Set(prev);
        const isEnabled = next.has(id);
        if (isEnabled) {
          next.delete(id);
        } else {
          next.add(id);
        }

        if (group.activeMode === "energy" && id === "flattened") {
          if (isEnabled) {
            if (energyMain === "flattened") {
              setEnergyMain("norm");
            }
          } else {
            setEnergyMain("flattened");
          }
        }
        return next;
      });
    },
    [energyMain, group.activeMode, setEnergyMain],
  );

  useEffect(() => {
    setEnabledOverlays(new Set());
  }, [group.activeMode]);

  // Close add menu on outside click
  useEffect(() => {
    if (!showAddMenu) return;
    const handleClick = (e: MouseEvent) => {
      if (addMenuRef.current && !addMenuRef.current.contains(e.target as Node)) {
        setShowAddMenu(false);
      }
    };
    document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, [showAddMenu]);

  const overlays = OVERLAYS_BY_MODE[group.activeMode] ?? [];
  const availableModes = ALL_MODES.filter((m) => !group.tabs.includes(m));

  return (
    <div
      className={`bg-[#0f1115] rounded-md flex flex-col min-h-0 min-w-0 ${
        isActive ? "outline outline-1 outline-blue-500/50 -outline-offset-1 z-10" : ""
      }`}
      onClick={() => setActiveGroup(group.id)}
    >
      {/* Group header: tabs + actions */}
      <div className="flex items-center bg-[#151515] shrink-0 h-7 min-h-[28px]">
        <div className="flex flex-1 min-w-0 overflow-x-auto">
          {group.tabs.map((mode) => (
            <button
              key={mode}
              className={`flex items-center gap-1 px-3 h-7 text-[12px] whitespace-nowrap transition-colors shrink-0 ${
                group.activeMode === mode
                  ? "bg-[#0f1115] text-[#e8e8e8]"
                  : "text-[#9a9a9a] hover:text-[#e8e8e8]"
              }`}
              onClick={(e) => {
                e.stopPropagation();
                setGroupActiveMode(group.id, mode);
              }}
            >
              {MODE_LABELS[mode]}
              {group.tabs.length > 1 && (
                <span
                  className="ml-1 w-3.5 h-3.5 flex items-center justify-center rounded text-[10px] text-[#777] hover:bg-[#2a2a2a] hover:text-[#e8e8e8] opacity-0 group-hover:opacity-100"
                  onClick={(e) => {
                    e.stopPropagation();
                    removePlotTab(group.id, mode);
                  }}
                  style={{ opacity: group.activeMode === mode ? 1 : undefined }}
                >
                  ×
                </span>
              )}
            </button>
          ))}
          {/* Add tab button with dropdown */}
          <div className="relative" ref={addMenuRef}>
            <button
              className="w-7 h-7 flex items-center justify-center text-[#777] hover:text-[#d0d0d0] hover:bg-[#242424] shrink-0"
              title="Add plot type"
              onClick={(e) => {
                e.stopPropagation();
                if (availableModes.length > 0) setShowAddMenu(!showAddMenu);
              }}
              disabled={availableModes.length === 0}
            >
              <Plus size={14} />
            </button>
            {showAddMenu && availableModes.length > 0 && (
              <div className="absolute top-full left-0 z-50 bg-[#1a1a1a] border border-[#343434] rounded-md shadow-xl min-w-[120px] py-1 text-xs">
                {availableModes.map((mode) => (
                  <button
                    key={mode}
                    className="w-full text-left px-3 py-1.5 text-[#e0e0e0] hover:bg-blue-500/20 transition-colors"
                    onClick={(e) => {
                      e.stopPropagation();
                      addPlotTab(group.id, mode);
                      setShowAddMenu(false);
                    }}
                  >
                    {MODE_LABELS[mode]}
                  </button>
                ))}
              </div>
            )}
          </div>
        </div>
        <div className="flex gap-px px-1 shrink-0">
          {canSplit && (
            <>
              <button
                className="w-5 h-5 flex items-center justify-center text-[#777] hover:text-[#e8e8e8] rounded hover:bg-[#2a2a2a]"
                title="Split right"
                onClick={(e) => {
                  e.stopPropagation();
                  splitGroup(group.id, "right");
                }}
              >
                <SplitRightIcon />
              </button>
              <button
                className="w-5 h-5 flex items-center justify-center text-[#777] hover:text-[#e8e8e8] rounded hover:bg-[#2a2a2a]"
                title="Split down"
                onClick={(e) => {
                  e.stopPropagation();
                  splitGroup(group.id, "down");
                }}
              >
                <SplitDownIcon />
              </button>
            </>
          )}
          {canClose && (
            <button
              className="w-5 h-5 flex items-center justify-center text-[#777] hover:text-[#e8e8e8] rounded hover:bg-[#2a2a2a]"
              title="Close group"
              onClick={(e) => {
                e.stopPropagation();
                closeGroup(group.id);
              }}
            >
              <X size={12} />
            </button>
          )}
        </div>
      </div>

      {/* Overlay checkboxes */}
      <div className="flex items-center gap-2.5 px-2 py-0.5 bg-[#131313] shrink-0 min-h-[22px]">
        {group.activeMode === "energy" && (
          <div className="flex items-center gap-1 mr-2">
            <button
              className={`px-2 py-0.5 rounded text-[11px] transition-colors ${
                energyMain === "mu"
                  ? "bg-blue-500/20 text-blue-300"
                  : "text-[#9a9a9a] hover:text-[#e8e8e8] hover:bg-[#1f1f1f]"
              }`}
              onClick={(e) => {
                e.stopPropagation();
                setEnergyMain("mu");
                setEnabledOverlays((prev) => {
                  const next = new Set(prev);
                  next.delete("flattened");
                  return next;
                });
              }}
              title="Show \u03BC(E) as main trace"
            >
              μ(E)
            </button>
            <button
              className={`px-2 py-0.5 rounded text-[11px] transition-colors ${
                energyMain === "norm" || energyMain === "flattened"
                  ? "bg-blue-500/20 text-blue-300"
                  : "text-[#9a9a9a] hover:text-[#e8e8e8] hover:bg-[#1f1f1f]"
              }`}
              onClick={(e) => {
                e.stopPropagation();
                setEnergyMain("norm");
              }}
              title="Show normalized μ(E) as main trace"
            >
              Norm
            </button>
          </div>
        )}
        {overlays.map((ov) => (
          <label
            key={ov.id}
            className={`flex items-center gap-1 text-[11px] cursor-pointer whitespace-nowrap ${
              enabledOverlays.has(ov.id) ? "text-[#d0d0d0]" : "text-[#777]"
            }`}
          >
            <input
              type="checkbox"
              checked={enabledOverlays.has(ov.id)}
              onChange={() => toggleOverlay(ov.id)}
              className="w-3 h-3 accent-blue-500 cursor-pointer"
            />
            {ov.label}
          </label>
        ))}
      </div>

      {/* Plot area — pass enabledOverlays */}
      <GroupPlotArea mode={group.activeMode} enabledOverlays={enabledOverlays} />
    </div>
  );
}

/* ─── Plot Area (Plotly.js or SVG) ─── */

function GroupPlotArea({
  mode,
  enabledOverlays,
}: {
  mode: PlotMode;
  enabledOverlays: Set<string>;
}) {
  const activeIndex = useWorkspaceStore((s) => s.activeIndex);
  const selectedIndices = useWorkspaceStore((s) => s.selectedIndices);
  const activeTabId = useWorkspaceStore((s) => s.activeTabId);
  const energyMain = useWorkspaceStore((s) => s.energyMain);
  const renderMode = useWorkspaceStore((s) => s.renderMode);
  const renderModeSource = useWorkspaceStore((s) => s.renderModeSource);
  const setAutoRenderMode = useWorkspaceStore((s) => s.setAutoRenderMode);
  const coreFormat = useWorkspaceStore((s) => s.coreFormat);
  const normOpts = useWorkspaceStore((s) => s.normOpts);
  const bgOpts = useWorkspaceStore((s) => s.bgOpts);
  const fftOpts = useWorkspaceStore((s) => s.fftOpts);
  const pickTarget = useWorkspaceStore((s) => s.pickTarget);
  const cursorTool = useWorkspaceStore((s) => s.cursorTool);
  const [plotContainerEl, setPlotContainerEl] = useState<HTMLDivElement | null>(null);
  const plotContainerRef = useCallback((node: HTMLDivElement | null) => {
    setPlotContainerEl((prev) => (prev === node ? prev : node));
  }, []);
  const [plotSize, setPlotSize] = useState({ width: 0, height: 0 });

  const selectedArray = useMemo(() => Array.from(selectedIndices), [selectedIndices]);
  const selectedCount = selectedArray.length;
  const coreIndex = activeIndex ?? (selectedArray.length > 0 ? selectedArray[0] : null);
  const useGroup = renderMode !== "core" && selectedArray.length > 1;
  const resolvedMode: PlotMode = mode === "energy" ? (energyMain === "mu" ? "mu" : "norm") : mode;
  const pipelineOptions = useMemo<PipelineOptions>(() => {
    if (resolvedMode === "r") {
      return {
        norm: normOpts,
        bg: bgOpts,
        fft: fftOpts,
      };
    }
    if (resolvedMode === "k") {
      return {
        norm: normOpts,
        bg: bgOpts,
      };
    }
    return {
      norm: normOpts,
    };
  }, [bgOpts, fftOpts, normOpts, resolvedMode]);

  // Each group independently fetches its own mode's data
  const { data: singlePlot, error: singlePlotError } = usePlotSpectrum(
    !useGroup ? coreIndex : null,
    resolvedMode,
    pipelineOptions,
    activeTabId,
  );
  const { data: groupPlot, error: groupPlotError } = usePlotGroup(
    useGroup ? selectedArray : [],
    resolvedMode,
    pipelineOptions,
    activeTabId,
  );
  const { data: coreAssets, error: corePlotError } = usePlotCore(
    renderMode === "core" && !useGroup ? coreIndex : null,
    renderMode === "core" ? [resolvedMode] : [],
    renderMode === "core" ? pipelineOptions : undefined,
  );

  const plotResult = useGroup ? groupPlot : singlePlot;
  const totalPoints = useMemo(
    () => (plotResult?.traces ?? []).reduce((sum, trace) => sum + trace.x.length, 0),
    [plotResult],
  );

  useEffect(() => {
    if (renderModeSource !== "auto") return;
    const shouldCoreBySpectra = selectedCount > AUTO_CORE_SPECTRA_THRESHOLD;
    const shouldCoreByPoints = totalPoints > AUTO_CORE_POINT_THRESHOLD;
    const nextMode = shouldCoreBySpectra || shouldCoreByPoints ? "core" : "interactive";
    if (nextMode !== renderMode) {
      setAutoRenderMode(nextMode);
    }
  }, [renderModeSource, selectedCount, totalPoints, renderMode, setAutoRenderMode]);

  useEffect(() => {
    if (!plotContainerEl) return;

    const observer = new ResizeObserver((entries) => {
      const entry = entries[0];
      if (!entry) return;
      const nextWidth = Math.max(0, Math.floor(entry.contentRect.width));
      const nextHeight = Math.max(0, Math.floor(entry.contentRect.height));
      setPlotSize((prev) =>
        prev.width === nextWidth && prev.height === nextHeight
          ? prev
          : { width: nextWidth, height: nextHeight },
      );
    });
    observer.observe(plotContainerEl);

    return () => observer.disconnect();
  }, [plotContainerEl]);

  const plotData = useMemo(() => {
    if (!plotResult?.traces) return [];
    let sourceTraces = plotResult.traces;

    if (mode === "energy") {
      if (energyMain === "flattened") {
        const flattened = sourceTraces.find((trace) => trace.overlay === "flattened");
        const overlaysOnly = sourceTraces.filter(
          (trace) => trace.overlay && trace.overlay !== "flattened",
        );
        if (flattened) {
          sourceTraces = [
            {
              ...flattened,
              overlay: undefined,
              dash: undefined,
              label: flattened.label || "Flattened",
            },
            ...overlaysOnly,
          ];
        } else {
          sourceTraces = overlaysOnly;
        }
      } else if (energyMain === "norm") {
        sourceTraces = sourceTraces.filter((trace) => trace.overlay !== "flattened");
      }
    }

    return sourceTraces
      .filter((trace) => {
        // Main traces (no overlay field) always shown
        if (!trace.overlay) return true;
        // Overlay traces shown only if enabled
        return enabledOverlays.has(trace.overlay);
      })
      .map((trace) => {
        const targetLen = Math.min(trace.x.length, trace.y.length);
        let x = trace.x.slice(0, targetLen);
        let y = trace.y.slice(0, targetLen);
        if (targetLen > INTERACTIVE_MAX_POINTS_PER_TRACE) {
          const downsampled = lttbDownsample(x, y, INTERACTIVE_MAX_POINTS_PER_TRACE);
          x = downsampled.x;
          y = downsampled.y;
        }

        return {
          x,
          y,
          name: trace.label,
          type: "scattergl" as const,
          mode: "lines" as const,
          line: {
            dash: (trace.dash ?? "solid") as Plotly.Dash,
            width: trace.overlay ? 1.8 : 2.4,
            color: trace.color ?? (trace.overlay ? "#a0a0a0" : MAIN_TRACE_COLORS[mode]),
          },
          // Overlay traces: no hover, subtle opacity
          ...(trace.overlay ? { hoverinfo: "skip" as const, opacity: 0.8, showlegend: true } : {}),
        };
      });
  }, [enabledOverlays, energyMain, mode, plotResult]);

  const bounds = useMemo(() => {
    let minX = Number.POSITIVE_INFINITY;
    let maxX = Number.NEGATIVE_INFINITY;
    let minY = Number.POSITIVE_INFINITY;
    let maxY = Number.NEGATIVE_INFINITY;

    for (const trace of plotData) {
      const xVals = Array.isArray(trace.x) ? trace.x : [];
      const yVals = Array.isArray(trace.y) ? trace.y : [];
      const n = Math.min(xVals.length, yVals.length);
      for (let i = 0; i < n; i++) {
        const x = xVals[i];
        const y = yVals[i];
        if (!Number.isFinite(x) || !Number.isFinite(y)) continue;
        if (x < minX) minX = x;
        if (x > maxX) maxX = x;
        if (y < minY) minY = y;
        if (y > maxY) maxY = y;
      }
    }

    if (
      !Number.isFinite(minX) ||
      !Number.isFinite(maxX) ||
      !Number.isFinite(minY) ||
      !Number.isFinite(maxY)
    ) {
      return null;
    }
    return { minX, maxX, minY, maxY };
  }, [plotData]);

  const layout = useMemo(
    () => ({
      paper_bgcolor: "rgba(0,0,0,0)",
      plot_bgcolor: "rgba(16,18,21,1)",
      font: { color: "#c7c7c7", size: 12 },
      xaxis: {
        title: { text: plotResult?.x_label ?? "", font: { color: "#d0d0d0", size: 12 } },
        gridcolor: "#2a2a2a",
        zerolinecolor: "#3a3a3a",
        automargin: true,
        tickfont: { color: "#c7c7c7", size: 11 },
        title_standoff: 10,
      },
      yaxis: {
        title: { text: plotResult?.y_label ?? "", font: { color: "#d0d0d0", size: 12 } },
        gridcolor: "#2a2a2a",
        zerolinecolor: "#3a3a3a",
        automargin: true,
        tickfont: { color: "#c7c7c7", size: 11 },
        title_standoff: 10,
        ...(bounds
          ? (() => {
              const ySpan = Math.max(bounds.maxY - bounds.minY, 1e-9);
              const yPad = ySpan * 0.08;
              return { range: [bounds.minY - yPad, bounds.maxY + yPad] as [number, number] };
            })()
          : {}),
      },
      margin: { l: 60, r: 14, t: 12, b: 58, pad: 4 },
      showlegend: plotData.length > 1,
      legend: { font: { color: "#c7c7c7", size: 10 }, bgcolor: "rgba(0,0,0,0)" },
      autosize: true,
      ...(plotSize.width > 0 ? { width: plotSize.width } : {}),
      ...(plotSize.height > 0 ? { height: plotSize.height } : {}),
      ...(pickTarget
        ? { dragmode: false as const }
        : cursorTool === "zoom"
          ? { dragmode: "zoom" as const }
          : cursorTool === "pan"
            ? { dragmode: "pan" as const }
            : { dragmode: false as const }),
    }),
    [bounds, cursorTool, pickTarget, plotData.length, plotResult, plotSize.height, plotSize.width],
  );

  // Handle Plotly click events for pick mode
  const handlePlotClick = useCallback(
    (data: Plotly.PlotMouseEvent) => {
      if (!pickTarget || !data.points || data.points.length === 0) return;
      const point = data.points[0];
      const xValue = point.x as number;

      // Dispatch picked value to workspace store
      useWorkspaceStore.getState().onPickValue(pickTarget, xValue);
    },
    [pickTarget],
  );

  const canUseCoreImage =
    renderMode === "core" &&
    !useGroup &&
    enabledOverlays.size === 0 &&
    mode !== "energy" &&
    cursorTool === "select";
  const plotError = useGroup ? groupPlotError : singlePlotError;
  const lastLoggedErrorRef = useRef<string | null>(null);

  useEffect(() => {
    const rawError = plotError ?? (canUseCoreImage ? corePlotError : null);
    if (!rawError) return;
    const message = rawError instanceof Error ? rawError.message : String(rawError);
    if (!message || message === lastLoggedErrorRef.current) return;
    addLog("error", `Plot update failed (${MODE_LABELS[mode]}): ${message}`);
    lastLoggedErrorRef.current = message;
  }, [canUseCoreImage, corePlotError, mode, plotError]);

  if (canUseCoreImage) {
    const preferredPng = coreAssets?.pngs?.[0];
    const preferredSvg = coreAssets?.svgs?.[0];

    if (coreFormat === "png" && preferredPng) {
      return (
        <div className="flex-1 overflow-auto p-1 min-h-0 flex items-center justify-center">
          <img
            src={preferredPng}
            alt={`${MODE_LABELS[mode]} core plot`}
            className="w-full h-full object-contain"
          />
        </div>
      );
    }
    if (coreFormat === "svg" && preferredSvg) {
      const svgDataUrl = svgToDataUrl(preferredSvg);
      return (
        <div className="flex-1 overflow-auto p-1 min-h-0 flex items-center justify-center">
          <img
            src={svgDataUrl}
            alt={`${MODE_LABELS[mode]} core plot`}
            className="w-full h-full object-contain"
          />
        </div>
      );
    }
    if (preferredPng) {
      return (
        <div className="flex-1 overflow-auto p-1 min-h-0 flex items-center justify-center">
          <img
            src={preferredPng}
            alt={`${MODE_LABELS[mode]} core plot`}
            className="w-full h-full object-contain"
          />
        </div>
      );
    }
    if (preferredSvg) {
      const svgDataUrl = svgToDataUrl(preferredSvg);
      return (
        <div className="flex-1 overflow-auto p-1 min-h-0 flex items-center justify-center">
          <img
            src={svgDataUrl}
            alt={`${MODE_LABELS[mode]} core plot`}
            className="w-full h-full object-contain"
          />
        </div>
      );
    }
  }

  const inlineError = plotError ?? (canUseCoreImage ? corePlotError : null);
  if (plotData.length === 0 && inlineError) {
    const message = inlineError instanceof Error ? inlineError.message : String(inlineError);
    return (
      <div className="flex-1 flex items-center justify-center text-red-300 text-xs min-h-0 px-3 text-center">
        Plot error: {message}
      </div>
    );
  }

  if (plotData.length === 0) {
    const title =
      mode === "energy"
        ? energyMain === "flattened"
          ? "Flattened"
          : energyMain === "norm"
            ? "Norm"
            : "\u03BC(E)"
        : MODE_LABELS[mode];
    return (
      <div className="flex-1 flex items-center justify-center text-slate-600 text-xs min-h-0">
        {activeIndex !== null ? `${title} \u2014 no data` : "Select a spectrum"}
      </div>
    );
  }

  return (
    <div
      ref={plotContainerRef}
      className={`flex-1 min-h-0 ${pickTarget ? "cursor-crosshair" : ""}`}
    >
      <Plot
        data={plotData}
        layout={layout}
        revision={plotSize.width * 10000 + plotSize.height}
        config={{
          responsive: true,
          staticPlot: renderMode === "core",
          displayModeBar: false,
          displaylogo: false,
        }}
        useResizeHandler
        style={{ width: "100%", height: "100%" }}
        onClick={pickTarget ? handlePlotClick : undefined}
      />
    </div>
  );
}

/* ─── Inline SVG Icons for split actions ─── */

function SplitRightIcon() {
  return (
    <svg
      width="12"
      height="12"
      viewBox="0 0 14 14"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <rect x="1" y="1" width="12" height="12" rx="1" />
      <line x1="7" y1="1" x2="7" y2="13" />
    </svg>
  );
}

function SplitDownIcon() {
  return (
    <svg
      width="12"
      height="12"
      viewBox="0 0 14 14"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <rect x="1" y="1" width="12" height="12" rx="1" />
      <line x1="1" y1="7" x2="13" y2="7" />
    </svg>
  );
}
