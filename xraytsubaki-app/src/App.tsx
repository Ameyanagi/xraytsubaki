import { useState, useCallback, useEffect, useRef, useMemo } from "react";
import { DockLayout } from "rc-dock";
import type { LayoutBase, LayoutData, TabBase, TabData } from "rc-dock";
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
import { useBatchProgressEvents } from "@/hooks/useSpectra";
import {
  createDefaultDockLayout,
  readWorkspaceLayoutFromStorage,
  sanitizeDockLayout,
  serializeWorkspaceLayout,
  writeWorkspaceLayoutToStorage,
} from "@/lib/workspace-serde";

const LEFT_SIDEBAR_MIN = 160;
const LEFT_SIDEBAR_MAX = 420;

export default function App() {
  useBatchProgressEvents();

  const [activeSidebar, setActiveSidebar] = useState<string>("spectra");
  const [ctxMenu, setCtxMenu] = useState<{ x: number; y: number } | null>(null);

  const tabs = useWorkspaceStore((s) => s.tabs);
  const activeTabId = useWorkspaceStore((s) => s.activeTabId);
  const setActiveTab = useWorkspaceStore((s) => s.setActiveTab);
  const removeTab = useWorkspaceStore((s) => s.removeTab);
  const leftSidebarCollapsed = useWorkspaceStore((s) => s.leftSidebarCollapsed);
  const setLeftSidebarCollapsed = useWorkspaceStore((s) => s.setLeftSidebarCollapsed);
  const leftSidebarWidth = useWorkspaceStore((s) => s.leftSidebarWidth);
  const setLeftSidebarWidth = useWorkspaceStore((s) => s.setLeftSidebarWidth);
  const dockLayout = useWorkspaceStore((s) => s.dockLayout);
  const setDockLayout = useWorkspaceStore((s) => s.setDockLayout);

  const setActiveIndex = useSpectraStore((s) => s.setActiveIndex);

  const defaultDockLayout = useMemo(() => createDefaultDockLayout(), []);
  const dockRef = useRef<DockLayout | null>(null);
  const lastAppliedDockKeyRef = useRef<string | null>(null);
  const restoredLayoutRef = useRef(false);

  const handleSidebarToggle = useCallback(
    (panel: string) => {
      if (activeSidebar === panel && !leftSidebarCollapsed) {
        setLeftSidebarCollapsed(true);
        return;
      }
      setActiveSidebar(panel);
      setLeftSidebarCollapsed(false);
    },
    [activeSidebar, leftSidebarCollapsed, setLeftSidebarCollapsed],
  );

  const handleTabClick = useCallback(
    (tabId: string, spectrumIndex: number) => {
      setActiveTab(tabId);
      setActiveIndex(spectrumIndex);
    },
    [setActiveTab, setActiveIndex],
  );

  const handleParamContextMenu = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    setCtxMenu({ x: e.clientX, y: e.clientY });
  }, []);

  useEffect(() => {
    if (restoredLayoutRef.current) return;

    const storedLayout = readWorkspaceLayoutFromStorage();
    if (storedLayout) {
      setLeftSidebarCollapsed(storedLayout.left_sidebar.collapsed);
      setLeftSidebarWidth(storedLayout.left_sidebar.width);
      setDockLayout(storedLayout.dock);
    }

    restoredLayoutRef.current = true;
  }, [setDockLayout, setLeftSidebarCollapsed, setLeftSidebarWidth]);

  useEffect(() => {
    if (!dockRef.current || !dockLayout) return;

    const sanitized = sanitizeDockLayout(dockLayout);
    const key = JSON.stringify(sanitized);
    if (key === lastAppliedDockKeyRef.current) return;

    dockRef.current.loadLayout(sanitized);
    lastAppliedDockKeyRef.current = key;
  }, [dockLayout]);

  useEffect(() => {
    const currentLayout = dockRef.current?.saveLayout() ?? dockLayout ?? defaultDockLayout;
    const payload = serializeWorkspaceLayout(currentLayout, leftSidebarCollapsed, leftSidebarWidth);
    writeWorkspaceLayoutToStorage(payload);
  }, [defaultDockLayout, dockLayout, leftSidebarCollapsed, leftSidebarWidth]);

  useEffect(() => {
    if (!ctxMenu) return;

    const close = () => setCtxMenu(null);
    document.addEventListener("click", close);
    return () => document.removeEventListener("click", close);
  }, [ctxMenu]);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      const isMod = e.metaKey || e.ctrlKey;
      if (!isMod) return;

      if (e.key === "o") {
        e.preventDefault();
        const openBtn = document.querySelector('[title="Open"]') as HTMLButtonElement;
        openBtn?.click();
      }

      if (e.key === "s") {
        e.preventDefault();
        const saveBtn = document.querySelector('[title="Save"]') as HTMLButtonElement;
        saveBtn?.click();
      }

      if (e.key === "p" && e.shiftKey) {
        e.preventDefault();
        const processBtn = document.querySelector('[title="Process All"]') as HTMLButtonElement;
        processBtn?.click();
      }

      if (e.key >= "1" && e.key <= "4") {
        e.preventDefault();
        const modes = ["mu", "norm", "k", "r"] as const;
        const idx = parseInt(e.key, 10) - 1;
        if (idx < modes.length) {
          useWorkspaceStore.getState().setPlotMode(modes[idx]);
        }
      }
    };

    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, []);

  const loadDockTab = useCallback(
    (tab: TabBase): TabData => {
      switch (tab.id) {
        case "plot":
          return {
            id: "plot",
            group: "workspace",
            title: "Plot",
            closable: false,
            cached: true,
            content: (
              <ErrorBoundary>
                <PlotPanel />
              </ErrorBoundary>
            ),
          };
        case "parameters":
          return {
            id: "parameters",
            group: "workspace",
            title: "Parameters",
            closable: false,
            cached: true,
            content: (
              <div className="h-full bg-slate-800" onContextMenu={handleParamContextMenu}>
                <ErrorBoundary>
                  <ParameterPanel />
                </ErrorBoundary>
              </div>
            ),
          };
        case "log":
          return {
            id: "log",
            group: "workspace",
            title: "Log",
            closable: false,
            cached: true,
            content: <LogPanel />,
          };
        case "fit":
          return {
            id: "fit",
            group: "workspace",
            title: "Fit",
            closable: false,
            cached: true,
            content: <FitPanel />,
          };
        default: {
          const id = typeof tab.id === "string" ? tab.id : "unknown";
          return {
            id,
            group: "workspace",
            title: id,
            closable: false,
            content: (
              <div className="h-full flex items-center justify-center text-xs text-slate-500">
                Unknown panel: {id}
              </div>
            ),
          };
        }
      }
    },
    [handleParamContextMenu],
  );

  const handleDockLayoutChange = useCallback(
    (layout: LayoutBase) => {
      const sanitized = sanitizeDockLayout(layout);
      const key = JSON.stringify(sanitized);
      lastAppliedDockKeyRef.current = key;
      setDockLayout(sanitized);

      writeWorkspaceLayoutToStorage(
        serializeWorkspaceLayout(sanitized, leftSidebarCollapsed, leftSidebarWidth),
      );
    },
    [leftSidebarCollapsed, leftSidebarWidth, setDockLayout],
  );

  return (
    <div className="flex flex-col h-screen overflow-hidden text-[13px] leading-normal">
      <Toolbar />

      <div className="flex flex-1 min-h-0">
        <div className="w-[42px] shrink-0 bg-slate-800 border-r border-slate-700 flex flex-col items-center py-2 gap-0.5">
          <ActivityIcon
            icon={Folder}
            label="Spectra"
            active={!leftSidebarCollapsed && activeSidebar === "spectra"}
            onClick={() => handleSidebarToggle("spectra")}
          />
          <ActivityIcon
            icon={Search}
            label="Search"
            active={!leftSidebarCollapsed && activeSidebar === "search"}
            onClick={() => handleSidebarToggle("search")}
          />
          <ActivityIcon
            icon={Activity}
            label="Processing"
            active={!leftSidebarCollapsed && activeSidebar === "processing"}
            onClick={() => handleSidebarToggle("processing")}
          />
          <div className="flex-1" />
          <ActivityIcon
            icon={Settings}
            label="Settings"
            active={!leftSidebarCollapsed && activeSidebar === "settings"}
            onClick={() => handleSidebarToggle("settings")}
          />
        </div>

        {!leftSidebarCollapsed && (
          <>
            <div
              className="shrink-0 bg-slate-800 border-r border-slate-700 flex flex-col"
              style={{ width: leftSidebarWidth }}
            >
              <SidebarHeader panel={activeSidebar} />
              <SidebarContent panel={activeSidebar} />
            </div>
            <ResizeHandle
              onResize={(delta) =>
                setLeftSidebarWidth(
                  Math.max(LEFT_SIDEBAR_MIN, Math.min(LEFT_SIDEBAR_MAX, leftSidebarWidth + delta)),
                )
              }
            />
          </>
        )}

        <div className="flex-1 flex flex-col min-w-0 min-h-0">
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

          <div className="dock-host flex-1 min-h-0 min-w-0 bg-slate-950">
            <DockLayout
              ref={dockRef}
              defaultLayout={defaultDockLayout as unknown as LayoutData}
              loadTab={loadDockTab}
              onLayoutChange={handleDockLayoutChange}
              style={{ width: "100%", height: "100%" }}
            />
          </div>
        </div>
      </div>

      <StatusBar />

      {ctxMenu && <ContextMenu x={ctxMenu.x} y={ctxMenu.y} onClose={() => setCtxMenu(null)} />}
    </div>
  );
}

function ResizeHandle({ onResize }: { onResize: (delta: number) => void }) {
  const dragging = useRef(false);
  const lastPos = useRef(0);

  const handleMouseDown = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      dragging.current = true;
      lastPos.current = e.clientX;

      const handleMouseMove = (ev: MouseEvent) => {
        if (!dragging.current) return;
        const delta = ev.clientX - lastPos.current;
        lastPos.current = ev.clientX;
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
      document.body.style.cursor = "col-resize";
      document.body.style.userSelect = "none";
    },
    [onResize],
  );

  return (
    <div
      className="w-1 shrink-0 cursor-col-resize hover:bg-blue-500/30 active:bg-blue-500/50 transition-colors"
      onMouseDown={handleMouseDown}
    />
  );
}

function SidebarHeader({ panel }: { panel: string }) {
  const title =
    panel === "spectra"
      ? "Spectra"
      : panel === "search"
        ? "Search"
        : panel === "processing"
          ? "Processing"
          : "Settings";

  return (
    <div className="px-3 py-2 text-[11px] font-semibold text-slate-400 uppercase tracking-wider">
      {title}
    </div>
  );
}

function SidebarContent({ panel }: { panel: string }) {
  if (panel === "spectra") {
    return <SpectraList />;
  }

  const text =
    panel === "search"
      ? "Search tools are not implemented yet."
      : panel === "processing"
        ? "Processing queue view is not implemented yet."
        : "Settings panel is not implemented yet.";

  return <div className="px-3 py-2 text-xs text-slate-500">{text}</div>;
}

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
      {active && <div className="absolute left-0 top-1.5 bottom-1.5 w-0.5 bg-blue-500 rounded-r" />}
      <Icon size={20} />
    </button>
  );
}

function ContextMenu({ x, y, onClose }: { x: number; y: number; onClose: () => void }) {
  const ref = useRef<HTMLDivElement>(null);
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
