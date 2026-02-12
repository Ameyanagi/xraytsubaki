import { FolderOpen, Save, Play, Zap, Download } from "lucide-react";
import { useQueryClient } from "@tanstack/react-query";
import { open, save } from "@tauri-apps/plugin-dialog";
import { useLoadSpectra, useBatchProcess } from "@/hooks/useSpectra";
import { useWorkspaceStore } from "@/stores/workspace";
import { useSpectraStore } from "@/stores/spectra";
import { backend } from "@/backend/tauri";
import type { WorkspaceData } from "@/backend/types";
import {
  deserializeWorkspaceLayout,
  serializeWorkspaceLayout,
  writeWorkspaceLayoutToStorage,
} from "@/lib/workspace-serde";
import { addLog } from "@/panels/LogPanel";

const THEMES = [
  { value: "slate-pro", label: "Slate Pro" },
  { value: "vscode-dark", label: "VS Code Dark" },
  { value: "github-dark", label: "GitHub Dark" },
  { value: "darcula", label: "Darcula" },
  { value: "light", label: "Light" },
];

function isWorkspaceFile(path: string): boolean {
  return path.toLowerCase().endsWith(".xtw");
}

function fileName(path: string): string {
  const normalized = path.replaceAll("\\", "/");
  const idx = normalized.lastIndexOf("/");
  return idx >= 0 ? normalized.slice(idx + 1) : normalized;
}

export function Toolbar() {
  const queryClient = useQueryClient();

  const loadSpectra = useLoadSpectra();
  const batchProcess = useBatchProcess();
  const batchProgress = useSpectraStore((s) => s.batchProgress);
  const invalidateSpectra = useSpectraStore((s) => s.invalidateSpectra);
  const selectedIndices = useWorkspaceStore((s) => s.selectedIndices);
  const activeIndex = useWorkspaceStore((s) => s.activeIndex);

  const workspacePath = useWorkspaceStore((s) => s.workspacePath);
  const setWorkspacePath = useWorkspaceStore((s) => s.setWorkspacePath);
  const theme = useWorkspaceStore((s) => s.theme);
  const setTheme = useWorkspaceStore((s) => s.setTheme);
  const importTabsFromWorkspace = useWorkspaceStore((s) => s.importTabsFromWorkspace);
  const exportTabsForWorkspace = useWorkspaceStore((s) => s.exportTabsForWorkspace);
  const dockLayout = useWorkspaceStore((s) => s.dockLayout);
  const setDockLayout = useWorkspaceStore((s) => s.setDockLayout);
  const leftSidebarCollapsed = useWorkspaceStore((s) => s.leftSidebarCollapsed);
  const setLeftSidebarCollapsed = useWorkspaceStore((s) => s.setLeftSidebarCollapsed);
  const leftSidebarWidth = useWorkspaceStore((s) => s.leftSidebarWidth);
  const setLeftSidebarWidth = useWorkspaceStore((s) => s.setLeftSidebarWidth);

  const handleOpen = async () => {
    const files = await open({
      multiple: true,
      filters: [
        { name: "Workspace", extensions: ["xtw"] },
        { name: "XAS Data", extensions: ["dat", "txt", "xmu", "csv", "qas"] },
        { name: "All Files", extensions: ["*"] },
      ],
    });
    if (!files) return;

    const paths = Array.isArray(files) ? files : [files];
    if (paths.length === 1 && isWorkspaceFile(paths[0])) {
      try {
        const data = await backend.loadWorkspace(paths[0]);
        const layout = deserializeWorkspaceLayout(data.layout);

        setDockLayout(layout.dock);
        setLeftSidebarCollapsed(layout.left_sidebar.collapsed);
        setLeftSidebarWidth(layout.left_sidebar.width);
        writeWorkspaceLayoutToStorage(layout);

        setWorkspacePath(paths[0]);
        importTabsFromWorkspace(data.tabs);

        invalidateSpectra();
        await queryClient.invalidateQueries({ queryKey: ["spectrumList"] });
        await queryClient.invalidateQueries({ queryKey: ["spectrumData"] });

        addLog("info", `Workspace loaded: ${fileName(paths[0])}`);
      } catch (error) {
        addLog("error", `Workspace load failed: ${String(error)}`);
      }
      return;
    }

    loadSpectra.mutate(paths);
  };

  const handleSave = async () => {
    let path = workspacePath;
    if (!path) {
      const selected = await save({
        filters: [{ name: "xraytsubaki Workspace", extensions: ["xtw"] }],
      });
      if (!selected) return;
      path = selected;
    }

    const layoutPayload = serializeWorkspaceLayout(
      dockLayout,
      leftSidebarCollapsed,
      leftSidebarWidth,
    );

    const data: WorkspaceData = {
      version: "0.1.0",
      layout: layoutPayload,
      tabs: exportTabsForWorkspace(),
      spectra_source: null,
      spectra_count: 0,
      processing: {},
      fits: {},
      plot_settings: {},
    };

    try {
      await backend.saveWorkspace(path, data);
      setWorkspacePath(path);
      writeWorkspaceLayoutToStorage(layoutPayload);
      addLog("info", `Workspace saved: ${fileName(path)}`);
    } catch (error) {
      addLog("error", `Workspace save failed: ${String(error)}`);
    }
  };

  const handleProcessAll = () => {
    const indices = Array.from(selectedIndices);
    if (indices.length > 0) {
      batchProcess.mutate({ indices });
    }
  };

  const handleRunFit = () => {
    window.dispatchEvent(new Event("xraytsubaki:fit-run-request"));
  };

  const handleExportSvg = async () => {
    if (activeIndex === null) return;

    const path = await save({
      filters: [{ name: "SVG", extensions: ["svg"] }],
    });
    if (!path) return;

    const plotMode = useWorkspaceStore.getState().plotMode;
    const svgs = await backend.plotSvg(activeIndex, [plotMode]);
    if (svgs.length > 0) {
      const { writeTextFile } = await import("@tauri-apps/plugin-fs");
      await writeTextFile(path, svgs[0]);
    }
  };

  return (
    <div className="flex items-center gap-1 h-9 px-3 bg-slate-800 border-b border-slate-700 shrink-0">
      <span className="text-[13px] font-semibold text-blue-500 mr-3 tracking-tight">
        xray<span className="font-normal text-slate-400">tsubaki</span>
      </span>

      <ToolButton icon={<FolderOpen size={15} />} label="Open" onClick={handleOpen} />
      <ToolButton icon={<Save size={15} />} label="Save" onClick={handleSave} />
      <Divider />
      <ToolButton
        icon={<Play size={15} />}
        label="Process All"
        onClick={handleProcessAll}
        disabled={selectedIndices.size === 0 || batchProcess.isPending}
      />
      <ToolButton
        icon={<Zap size={15} />}
        label="Fit"
        onClick={handleRunFit}
        disabled={activeIndex === null}
      />
      <Divider />
      <ToolButton icon={<Download size={15} />} label="Export" onClick={handleExportSvg} />

      {loadSpectra.isPending && (
        <span className="ml-2 text-xs text-blue-400 animate-pulse">Loading...</span>
      )}
      {batchProgress?.active && (
        <span className="ml-2 text-xs text-blue-400 animate-pulse">
          Batch {batchProgress.current}/{batchProgress.total}
        </span>
      )}

      <div className="flex-1" />

      <select
        className="bg-slate-700 border border-slate-600 text-slate-300 text-[11px] px-2 py-0.5 rounded cursor-pointer focus:outline-none focus:border-blue-500 min-w-[110px]"
        value={theme}
        onChange={(e) => setTheme(e.target.value)}
      >
        {THEMES.map((t) => (
          <option key={t.value} value={t.value}>
            {t.label}
          </option>
        ))}
      </select>
    </div>
  );
}

function ToolButton({
  icon,
  label,
  onClick,
  disabled = false,
}: {
  icon: React.ReactNode;
  label: string;
  onClick: () => void;
  disabled?: boolean;
}) {
  return (
    <button
      className="flex items-center gap-1.5 px-2 py-1 rounded text-xs text-slate-300 hover:bg-slate-700 hover:text-white disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
      onClick={onClick}
      disabled={disabled}
      title={label}
    >
      {icon}
      <span>{label}</span>
    </button>
  );
}

function Divider() {
  return <div className="w-px h-4 bg-slate-600 mx-1" />;
}
