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

function parseDataUrl(dataUrl: string): { mime: string; bytes: Uint8Array } | null {
  const match = /^data:([^;]+);base64,(.+)$/i.exec(dataUrl.trim());
  if (!match) return null;
  const [, mime, base64] = match;
  try {
    const binary = atob(base64);
    const bytes = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i++) {
      bytes[i] = binary.charCodeAt(i);
    }
    return { mime, bytes };
  } catch {
    return null;
  }
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

  const handleExportPlot = async () => {
    if (activeIndex === null) return;

    const path = await save({
      defaultPath: `plot-${Date.now()}.png`,
      filters: [
        { name: "PNG", extensions: ["png"] },
        { name: "SVG", extensions: ["svg"] },
      ],
    });
    if (!path) return;

    const { plotMode, energyMain } = useWorkspaceStore.getState();
    const exportPanel =
      plotMode === "energy" ? (energyMain === "mu" ? "mu" : "norm") : plotMode;
    const core = await backend.plotCore(activeIndex, [exportPanel]);
    const asSvg = path.toLowerCase().endsWith(".svg");

    if (asSvg) {
      if (core.svgs.length === 0) {
        addLog("error", "Export failed: no SVG payload available");
        return;
      }
      const { writeTextFile } = await import("@tauri-apps/plugin-fs");
      await writeTextFile(path, core.svgs[0]);
      addLog("info", `Plot exported as SVG: ${fileName(path)}`);
      return;
    }

    if (core.pngs.length === 0) {
      addLog("error", "Export failed: no PNG payload available");
      return;
    }

    const parsed = parseDataUrl(core.pngs[0]);
    if (!parsed || !parsed.mime.includes("png")) {
      addLog("error", "Export failed: invalid PNG payload");
      return;
    }
    const { writeFile } = await import("@tauri-apps/plugin-fs");
    await writeFile(path, parsed.bytes);
    addLog("info", `Plot exported as PNG: ${fileName(path)}`);
  };

  return (
    <div className="flex items-center gap-1 h-10 px-3 bg-[#151515] shrink-0">
      <span className="text-[14px] font-semibold text-blue-400 mr-3 tracking-tight">
        xray<span className="font-normal text-[#a0a0a0]">tsubaki</span>
      </span>

      <ToolButton icon={<FolderOpen size={15} />} label="Open" onClick={handleOpen} />
      <ToolButton icon={<Save size={15} />} label="Save" onClick={handleSave} />
      <Divider />
      <ToolButton
        icon={<Play size={15} />}
        label="Process All"
        onClick={handleProcessAll}
        disabled={selectedIndices.size === 0 || batchProcess.isPending}
        variant="primary"
      />
      <ToolButton
        icon={<Zap size={15} />}
        label="Fit"
        onClick={handleRunFit}
        disabled={activeIndex === null}
        iconOnly
      />
      <Divider />
      <ToolButton icon={<Download size={15} />} label="Export" onClick={handleExportPlot} iconOnly />

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
        className="bg-[#242424] border border-[#343434] text-[#e0e0e0] text-[12px] px-2 py-0.5 rounded cursor-pointer focus:outline-none focus:border-blue-500 min-w-[110px]"
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
  variant = "ghost",
  iconOnly = false,
}: {
  icon: React.ReactNode;
  label: string;
  onClick: () => void;
  disabled?: boolean;
  variant?: "ghost" | "primary";
  iconOnly?: boolean;
}) {
  const base = iconOnly
    ? "w-8 h-8 justify-center"
    : "px-2.5 py-1.5 justify-start";
  const style =
    variant === "primary"
      ? "bg-[#5b9aff] text-white hover:bg-[#4a89ee]"
      : "text-[#d0d0d0] hover:bg-[#242424] hover:text-white";
  return (
    <button
      className={`flex items-center gap-1.5 rounded text-[12px] transition-colors disabled:opacity-40 disabled:cursor-not-allowed ${base} ${style}`}
      onClick={onClick}
      disabled={disabled}
      title={label}
    >
      {icon}
      {!iconOnly && <span className="font-medium">{label}</span>}
    </button>
  );
}

function Divider() {
  return <div className="w-px h-4 bg-[#343434] mx-1" />;
}
