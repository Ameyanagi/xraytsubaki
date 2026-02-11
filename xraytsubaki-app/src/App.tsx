import { useState, useCallback, useEffect, useRef } from "react";
import { Folder, Search, Activity, Settings } from "lucide-react";
import { Toolbar } from "@/components/Toolbar";
import { StatusBar } from "@/components/StatusBar";
import { ErrorBoundary } from "@/components/ErrorBoundary";
import { SpectraList } from "@/panels/SpectraList";
import { PlotPanel } from "@/panels/PlotPanel";
import { ParameterPanel } from "@/panels/ParameterPanel";
import { LogPanel } from "@/panels/LogPanel";
import { FitPanel } from "@/panels/FitPanel";
import { useWorkspaceStore } from "@/stores/workspace";
import { useSpectraStore } from "@/stores/spectra";

export default function App() {
  const [activeSidebar, setActiveSidebar] = useState<string | null>("spectra");

  const tabs = useWorkspaceStore((s) => s.tabs);
  const activeTabId = useWorkspaceStore((s) => s.activeTabId);
  const setActiveTab = useWorkspaceStore((s) => s.setActiveTab);
  const removeTab = useWorkspaceStore((s) => s.removeTab);
  const setActiveIndex = useSpectraStore((s) => s.setActiveIndex);

  // Resizable panel widths
  const [leftWidth, setLeftWidth] = useState(200);
  const [rightWidth, setRightWidth] = useState(250);
  const [bottomHeight, setBottomHeight] = useState(120);

  const toggleSidebar = useCallback((panel: string) => {
    setActiveSidebar((current) => (current === panel ? null : panel));
  }, []);

  const handleTabClick = useCallback(
    (tabId: string, spectrumIndex: number) => {
      setActiveTab(tabId);
      setActiveIndex(spectrumIndex);
    },
    [setActiveTab, setActiveIndex],
  );

  // Right-click context menu state
  const [ctxMenu, setCtxMenu] = useState<{ x: number; y: number } | null>(null);

  const handleParamContextMenu = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    setCtxMenu({ x: e.clientX, y: e.clientY });
  }, []);

  // Close context menu on any click
  useEffect(() => {
    if (!ctxMenu) return;
    const close = () => setCtxMenu(null);
    document.addEventListener("click", close);
    return () => document.removeEventListener("click", close);
  }, [ctxMenu]);

  // Global keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      const isMod = e.metaKey || e.ctrlKey;
      if (!isMod) return;

      // Cmd+O — Open files
      if (e.key === "o") {
        e.preventDefault();
        // Trigger the toolbar open button by clicking it
        const openBtn = document.querySelector('[title="Open"]') as HTMLButtonElement;
        openBtn?.click();
      }
      // Cmd+S — Save workspace
      if (e.key === "s") {
        e.preventDefault();
        const saveBtn = document.querySelector('[title="Save"]') as HTMLButtonElement;
        saveBtn?.click();
      }
      // Cmd+Shift+P — Process All
      if (e.key === "p" && e.shiftKey) {
        e.preventDefault();
        const processBtn = document.querySelector('[title="Process All"]') as HTMLButtonElement;
        processBtn?.click();
      }
      // Cmd+1/2/3/4 — Switch plot mode
      if (e.key >= "1" && e.key <= "4") {
        e.preventDefault();
        const modes = ["mu", "norm", "k", "r"] as const;
        const idx = parseInt(e.key) - 1;
        if (idx < modes.length) {
          useWorkspaceStore.getState().setPlotMode(modes[idx]);
        }
      }
    };
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, []);

  return (
    <div className="flex flex-col h-screen overflow-hidden text-[13px] leading-normal">
      {/* ═══ Toolbar ═══ */}
      <Toolbar />

      {/* ═══ Main area ═══ */}
      <div className="flex flex-1 min-h-0">
        {/* Activity Bar */}
        <div className="w-[42px] shrink-0 bg-slate-800 border-r border-slate-700 flex flex-col items-center py-2 gap-0.5">
          <ActivityIcon
            icon={Folder}
            label="Spectra"
            active={activeSidebar === "spectra"}
            onClick={() => toggleSidebar("spectra")}
          />
          <ActivityIcon
            icon={Search}
            label="Search"
            active={activeSidebar === "search"}
            onClick={() => toggleSidebar("search")}
          />
          <ActivityIcon
            icon={Activity}
            label="Processing"
            active={activeSidebar === "processing"}
            onClick={() => toggleSidebar("processing")}
          />
          <div className="flex-1" />
          <ActivityIcon icon={Settings} label="Settings" onClick={() => {}} />
        </div>

        {/* Left Sidebar (resizable) */}
        {activeSidebar === "spectra" && (
          <>
            <div
              className="shrink-0 bg-slate-800 border-r border-slate-700 flex flex-col"
              style={{ width: leftWidth }}
            >
              <div className="px-3 py-2 text-[11px] font-semibold text-slate-400 uppercase tracking-wider">
                Spectra
              </div>
              <SpectraList />
            </div>
            <ResizeHandle
              direction="horizontal"
              onResize={(delta) => setLeftWidth((w) => Math.max(120, Math.min(400, w + delta)))}
            />
          </>
        )}

        {/* Center: analysis tabs + plot area */}
        <div className="flex-1 flex flex-col min-w-0">
          {/* Analysis tabs */}
          {tabs.length > 0 && (
            <div className="flex bg-slate-800/60 border-b border-slate-700 shrink-0">
              {tabs.map((tab) => (
                <div
                  key={tab.id}
                  className={`flex items-center gap-1.5 px-4 py-1.5 text-xs cursor-pointer border-r border-slate-700 transition-colors ${
                    activeTabId === tab.id
                      ? "bg-slate-950 text-slate-200 border-t-2 border-t-blue-500"
                      : "text-slate-400 hover:text-slate-200 border-t-2 border-t-transparent"
                  }`}
                  onClick={() => handleTabClick(tab.id, tab.spectrumIndex)}
                >
                  {tab.label}
                  <button
                    className="ml-1 w-4 h-4 flex items-center justify-center rounded text-slate-500 hover:bg-slate-700 hover:text-slate-200"
                    onClick={(e) => {
                      e.stopPropagation();
                      removeTab(tab.id);
                    }}
                  >
                    ×
                  </button>
                </div>
              ))}
            </div>
          )}

          {/* Plot panels (single default, splittable) */}
          <ErrorBoundary>
            <PlotPanel />
          </ErrorBoundary>
        </div>

        {/* Right Sidebar resize handle + panel */}
        <ResizeHandle
          direction="horizontal"
          onResize={(delta) => setRightWidth((w) => Math.max(180, Math.min(400, w - delta)))}
        />
        <div
          className="shrink-0 bg-slate-800 border-l border-slate-700 flex flex-col"
          style={{ width: rightWidth }}
          onContextMenu={handleParamContextMenu}
        >
          <ErrorBoundary>
            <ParameterPanel />
          </ErrorBoundary>
        </div>
      </div>

      {/* ═══ Bottom Panel (resizable) ═══ */}
      <ResizeHandle
        direction="vertical"
        onResize={(delta) => setBottomHeight((h) => Math.max(60, Math.min(400, h - delta)))}
      />
      <div className="shrink-0 border-t border-blue-500 flex" style={{ height: bottomHeight }}>
        <div className="flex-1 min-w-0">
          <LogPanel />
        </div>
        <div className="w-[280px] shrink-0 border-l border-slate-700">
          <FitPanel />
        </div>
      </div>

      {/* ═══ Status Bar ═══ */}
      <StatusBar />

      {/* ═══ Right-click Context Menu ═══ */}
      {ctxMenu && <ContextMenu x={ctxMenu.x} y={ctxMenu.y} onClose={() => setCtxMenu(null)} />}
    </div>
  );
}

/* ─── Resizable Panel Handle ─── */

function ResizeHandle({
  direction,
  onResize,
}: {
  direction: "horizontal" | "vertical";
  onResize: (delta: number) => void;
}) {
  const dragging = useRef(false);
  const lastPos = useRef(0);

  const handleMouseDown = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      dragging.current = true;
      lastPos.current = direction === "horizontal" ? e.clientX : e.clientY;

      const handleMouseMove = (ev: MouseEvent) => {
        if (!dragging.current) return;
        const currentPos = direction === "horizontal" ? ev.clientX : ev.clientY;
        const delta = currentPos - lastPos.current;
        lastPos.current = currentPos;
        onResize(delta);
      };

      const handleMouseUp = () => {
        dragging.current = false;
        document.removeEventListener("mousemove", handleMouseMove);
        document.removeEventListener("mouseup", handleMouseUp);
        document.body.style.cursor = "";
        document.body.style.userSelect = "";
      };

      document.addEventListener("mousemove", handleMouseMove);
      document.addEventListener("mouseup", handleMouseUp);
      document.body.style.cursor = direction === "horizontal" ? "col-resize" : "row-resize";
      document.body.style.userSelect = "none";
    },
    [direction, onResize],
  );

  return (
    <div
      className={
        direction === "horizontal"
          ? "w-1 shrink-0 cursor-col-resize hover:bg-blue-500/30 active:bg-blue-500/50 transition-colors"
          : "h-1 shrink-0 cursor-row-resize hover:bg-blue-500/30 active:bg-blue-500/50 transition-colors"
      }
      onMouseDown={handleMouseDown}
    />
  );
}

/* ─── Activity Bar Icon ─── */

function ActivityIcon({
  icon: Icon,
  label,
  active = false,
  onClick,
}: {
  icon: React.ComponentType<{ size?: number }>;
  label?: string;
  active?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      className={`relative w-9 h-9 flex items-center justify-center rounded cursor-pointer transition-colors ${
        active ? "text-slate-200" : "text-slate-500 hover:text-slate-300"
      }`}
      onClick={onClick}
      title={label}
    >
      {active && (
        <div className="absolute left-0 top-1.5 bottom-1.5 w-0.5 bg-blue-500 rounded-r" />
      )}
      <Icon size={20} />
    </button>
  );
}

/* ─── Right-click Context Menu ─── */

function ContextMenu({ x, y, onClose }: { x: number; y: number; onClose: () => void }) {
  const ref = useRef<HTMLDivElement>(null);

  // Adjust position if menu overflows viewport
  const [pos, setPos] = useState({ x, y });
  useEffect(() => {
    if (!ref.current) return;
    const rect = ref.current.getBoundingClientRect();
    setPos({
      x: rect.right > window.innerWidth ? x - rect.width : x,
      y: rect.bottom > window.innerHeight ? y - rect.height : y,
    });
  }, [x, y]);

  return (
    <div
      ref={ref}
      className="fixed z-50 bg-slate-800 border border-slate-600 rounded-md shadow-xl min-w-[180px] py-1 text-xs"
      style={{ left: pos.x, top: pos.y }}
    >
      <CtxItem label="Copy to marked" shortcut="⌘⇧C" onClick={onClose} />
      <CtxItem label="Copy to all" shortcut="⌘⇧A" onClick={onClose} />
      <div className="h-px bg-slate-700 my-1" />
      <CtxItem label="Paste from..." onClick={onClose} />
      <div className="h-px bg-slate-700 my-1" />
      <CtxItem label="Reset to defaults" onClick={onClose} />
    </div>
  );
}

function CtxItem({
  label,
  shortcut,
  onClick,
}: {
  label: string;
  shortcut?: string;
  onClick: () => void;
}) {
  return (
    <button
      className="w-full flex items-center gap-2 px-3 py-1.5 text-slate-200 hover:bg-blue-500/20 transition-colors text-left"
      onClick={onClick}
    >
      <span className="flex-1">{label}</span>
      {shortcut && <span className="text-slate-500">{shortcut}</span>}
    </button>
  );
}
