import { useMemo, useState, useCallback, useRef, useEffect } from "react";
import Plot from "react-plotly.js";
import {
  MousePointer2,
  Crosshair,
  ZoomIn,
  Move,
  Plus,
  X,
} from "lucide-react";
import { useSpectraStore } from "@/stores/spectra";
import { useWorkspaceStore } from "@/stores/workspace";
import { usePlotSpectrum, usePlotGroup } from "@/hooks/usePlot";
import type { PlotMode } from "@/backend/types";
import type { CursorTool, PlotGroup, PlotLayout } from "@/stores/workspace";

const MODE_LABELS: Record<PlotMode, string> = {
  mu: "\u03BC(E)",
  norm: "Norm",
  k: "\u03C7(k)",
  r: "\u03C7(R)",
};

const OVERLAYS_BY_MODE: Record<PlotMode, { id: string; label: string }[]> = {
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

const ALL_MODES: PlotMode[] = ["mu", "norm", "k", "r"];

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
  const { cursorTool, setCursorTool, pickTarget, setPickTarget, renderMode, setRenderMode } =
    useWorkspaceStore();

  const handleCursorTool = (tool: CursorTool) => {
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
    <div className="flex items-center gap-0.5 px-2 py-1 border-b border-slate-700 bg-slate-800 shrink-0">
      <div className="flex gap-px">
        {tools.map(({ tool, icon, label }) => (
          <button
            key={tool}
            className={`w-7 h-6 flex items-center justify-center rounded transition-colors ${
              cursorTool === tool
                ? "text-blue-400 bg-blue-500/15 border border-blue-500/50"
                : "text-slate-500 hover:text-slate-300 border border-transparent"
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

      <div className="flex border border-slate-600 rounded overflow-hidden">
        <button
          className={`px-2.5 py-0.5 text-[11px] transition-colors ${
            renderMode === "interactive"
              ? "text-blue-400 bg-blue-500/15"
              : "text-slate-500 hover:text-slate-300"
          }`}
          onClick={() => setRenderMode("interactive")}
        >
          Interactive
        </button>
        <button
          className={`px-2.5 py-0.5 text-[11px] border-l border-slate-600 transition-colors ${
            renderMode === "core"
              ? "text-blue-400 bg-blue-500/15"
              : "text-slate-500 hover:text-slate-300"
          }`}
          onClick={() => setRenderMode("core")}
        >
          Core
        </button>
      </div>
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
    <div
      className={`flex-1 grid gap-px bg-slate-700 min-h-0 ${gridClass[plotLayout]}`}
    >
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

  const toggleOverlay = useCallback((id: string) => {
    setEnabledOverlays((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }, []);

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
      className={`bg-slate-950 flex flex-col min-h-0 min-w-0 ${
        isActive ? "outline outline-1 outline-blue-500/50 -outline-offset-1 z-10" : ""
      }`}
      onClick={() => setActiveGroup(group.id)}
    >
      {/* Group header: tabs + actions */}
      <div className="flex items-center bg-slate-800/60 border-b border-slate-700 shrink-0 h-7 min-h-[28px]">
        <div className="flex flex-1 min-w-0 overflow-x-auto">
          {group.tabs.map((mode) => (
            <button
              key={mode}
              className={`flex items-center gap-1 px-3 h-7 text-[12px] whitespace-nowrap border-r border-slate-700 transition-colors shrink-0 ${
                group.activeMode === mode
                  ? "bg-slate-950 text-slate-200"
                  : "text-slate-400 hover:text-slate-200"
              }`}
              onClick={(e) => {
                e.stopPropagation();
                setGroupActiveMode(group.id, mode);
              }}
            >
              {MODE_LABELS[mode]}
              {group.tabs.length > 1 && (
                <span
                  className="ml-1 w-3.5 h-3.5 flex items-center justify-center rounded text-[10px] text-slate-500 hover:bg-slate-700 hover:text-slate-200 opacity-0 group-hover:opacity-100"
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
              className="w-7 h-7 flex items-center justify-center text-slate-500 hover:text-slate-300 hover:bg-slate-700/50 shrink-0"
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
              <div className="absolute top-full left-0 z-50 bg-slate-800 border border-slate-600 rounded-md shadow-xl min-w-[120px] py-1 text-xs">
                {availableModes.map((mode) => (
                  <button
                    key={mode}
                    className="w-full text-left px-3 py-1.5 text-slate-200 hover:bg-blue-500/20 transition-colors"
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
                className="w-5 h-5 flex items-center justify-center text-slate-500 hover:text-slate-200 rounded hover:bg-slate-700"
                title="Split right"
                onClick={(e) => {
                  e.stopPropagation();
                  splitGroup(group.id, "right");
                }}
              >
                <SplitRightIcon />
              </button>
              <button
                className="w-5 h-5 flex items-center justify-center text-slate-500 hover:text-slate-200 rounded hover:bg-slate-700"
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
              className="w-5 h-5 flex items-center justify-center text-slate-500 hover:text-slate-200 rounded hover:bg-slate-700"
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
      <div className="flex items-center gap-2.5 px-2 py-0.5 border-b border-slate-700 bg-slate-800/40 shrink-0 min-h-[22px]">
        {overlays.map((ov) => (
          <label
            key={ov.id}
            className={`flex items-center gap-1 text-[11px] cursor-pointer whitespace-nowrap ${
              enabledOverlays.has(ov.id) ? "text-slate-300" : "text-slate-500"
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

function GroupPlotArea({ mode, enabledOverlays }: { mode: PlotMode; enabledOverlays: Set<string> }) {
  const activeIndex = useSpectraStore((s) => s.activeIndex);
  const selectedIndices = useSpectraStore((s) => s.selectedIndices);
  const renderMode = useWorkspaceStore((s) => s.renderMode);
  const pickTarget = useWorkspaceStore((s) => s.pickTarget);

  const selectedArray = useMemo(() => Array.from(selectedIndices), [selectedIndices]);
  const useGroup = selectedArray.length > 1;

  // Each group independently fetches its own mode's data
  const { data: singlePlot } = usePlotSpectrum(!useGroup ? activeIndex : null, mode);
  const { data: groupPlot } = usePlotGroup(useGroup ? selectedArray : [], mode);

  const plotResult = useGroup ? groupPlot : singlePlot;

  const plotData = useMemo(() => {
    if (!plotResult?.traces) return [];
    return plotResult.traces
      .filter((trace) => {
        // Main traces (no overlay field) always shown
        if (!trace.overlay) return true;
        // Overlay traces shown only if enabled
        return enabledOverlays.has(trace.overlay);
      })
      .map((trace) => ({
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
        // Overlay traces: no hover, subtle opacity
        ...(trace.overlay
          ? { hoverinfo: "skip" as const, opacity: 0.8, showlegend: true }
          : {}),
      }));
  }, [enabledOverlays, plotResult]);

  const layout = useMemo(
    () => ({
      paper_bgcolor: "rgba(0,0,0,0)",
      plot_bgcolor: "rgba(15,23,42,0.3)",
      font: { color: "#94a3b8", size: 11 },
      xaxis: {
        title: { text: plotResult?.x_label ?? "", font: { color: "#94a3b8", size: 11 } },
        gridcolor: "#1e293b",
        zerolinecolor: "#334155",
      },
      yaxis: {
        title: { text: plotResult?.y_label ?? "", font: { color: "#94a3b8", size: 11 } },
        gridcolor: "#1e293b",
        zerolinecolor: "#334155",
      },
      margin: { l: 55, r: 10, t: 10, b: 40 },
      showlegend: plotData.length > 1,
      legend: { font: { color: "#94a3b8", size: 10 }, bgcolor: "rgba(0,0,0,0)" },
      autosize: true,
      // Enable crosshair cursor in pick mode
      ...(pickTarget ? { dragmode: false as const } : {}),
    }),
    [plotResult, plotData.length, pickTarget],
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

  if (renderMode === "core" && plotResult?.svgs && plotResult.svgs.length > 0) {
    // SVG from Rust backend — trusted source
    return (
      <div
        className="flex-1 overflow-auto p-1 min-h-0"
        dangerouslySetInnerHTML={{ __html: plotResult.svgs[0] }}
      />
    );
  }

  if (plotData.length === 0) {
    return (
      <div className="flex-1 flex items-center justify-center text-slate-600 text-xs min-h-0">
        {activeIndex !== null ? `${MODE_LABELS[mode]} \u2014 no data` : "Select a spectrum"}
      </div>
    );
  }

  return (
    <div className={`flex-1 min-h-0 ${pickTarget ? "cursor-crosshair" : ""}`}>
      <Plot
        data={plotData}
        layout={layout}
        config={{
          responsive: true,
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
    <svg width="12" height="12" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <rect x="1" y="1" width="12" height="12" rx="1" />
      <line x1="7" y1="1" x2="7" y2="13" />
    </svg>
  );
}

function SplitDownIcon() {
  return (
    <svg width="12" height="12" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <rect x="1" y="1" width="12" height="12" rx="1" />
      <line x1="1" y1="7" x2="13" y2="7" />
    </svg>
  );
}
