import { useCallback, useRef, useEffect } from "react";
import { Trash2, PlayCircle, CheckCircle2, Activity, Circle } from "lucide-react";
import { useSpectrumList, useRemoveSpectra, useBatchProcess } from "@/hooks/useSpectra";
import { useWorkspaceStore } from "@/stores/workspace";
import { addLog } from "@/panels/LogPanel";

export function SpectraList() {
  const { data: spectra, isLoading } = useSpectrumList();
  const removeSpectra = useRemoveSpectra();
  const batchProcess = useBatchProcess();
  const {
    selectedIndices,
    activeIndex,
    setActiveIndex,
    toggleSelection,
    selectRange,
    selectAll,
    clearSelection,
  } = useWorkspaceStore();
  const openSpectrumTab = useWorkspaceStore((s) => s.openSpectrumTab);
  const lastClickedRef = useRef<number | null>(null);
  const didAutoSelect = useRef(false);

  // Auto-select first spectrum when list first loads
  useEffect(() => {
    if (didAutoSelect.current || !spectra || spectra.length === 0) return;
    if (activeIndex === null) {
      setActiveIndex(spectra[0].index);
      toggleSelection(spectra[0].index);
      addLog("info", `${spectra.length} spectrum(s) available`);
      didAutoSelect.current = true;
    }
  }, [spectra, activeIndex, setActiveIndex, toggleSelection]);

  const handleClick = useCallback(
    (index: number, e: React.MouseEvent) => {
      if (e.shiftKey && lastClickedRef.current !== null) {
        selectRange(lastClickedRef.current, index);
      } else if (e.metaKey || e.ctrlKey) {
        toggleSelection(index);
      } else {
        clearSelection();
        toggleSelection(index);
        setActiveIndex(index);
      }
      lastClickedRef.current = index;
    },
    [selectRange, toggleSelection, clearSelection, setActiveIndex],
  );

  const handleDoubleClick = useCallback(
    (index: number, name: string) => {
      openSpectrumTab(index, name);
    },
    [openSpectrumTab],
  );

  const handleSelectAll = () => {
    if (spectra) selectAll(spectra.map((s) => s.index));
  };

  const handleDeleteSelected = () => {
    const indices = Array.from(selectedIndices);
    if (indices.length > 0) {
      removeSpectra.mutate(indices);
    }
  };

  const handleProcessSelected = () => {
    const indices = Array.from(selectedIndices);
    if (indices.length > 0) {
      batchProcess.mutate({ indices });
    }
  };

  if (isLoading) {
    return <div className="p-3 text-[#a0a0a0] text-sm">Loading spectra...</div>;
  }

  return (
    <div className="flex flex-col h-full">
      {/* Actions */}
      <div className="flex items-center gap-1 px-2 py-1.5">
        <button
          className="text-xs text-[#a0a0a0] hover:text-white px-1.5 py-0.5 rounded hover:bg-[#242424]"
          onClick={handleSelectAll}
        >
          All
        </button>
        <button
          className="text-xs text-[#a0a0a0] hover:text-white px-1.5 py-0.5 rounded hover:bg-[#242424]"
          onClick={clearSelection}
        >
          None
        </button>
        <div className="flex-1" />
        <button
          className="p-1 text-[#a0a0a0] hover:text-green-400 disabled:opacity-30"
          title="Process selected"
          onClick={handleProcessSelected}
          disabled={selectedIndices.size === 0 || batchProcess.isPending}
        >
          <PlayCircle size={14} />
        </button>
        <button
          className="p-1 text-[#a0a0a0] hover:text-red-400 disabled:opacity-30"
          title="Remove selected"
          onClick={handleDeleteSelected}
          disabled={selectedIndices.size === 0 || removeSpectra.isPending}
        >
          <Trash2 size={14} />
        </button>
      </div>

      {/* Spectrum list */}
      <div className="flex-1 overflow-y-auto">
        {!spectra || spectra.length === 0 ? (
          <div className="p-4 text-center text-[#888] text-sm">
            No spectra loaded.
            <br />
            Use <kbd className="px-1 py-0.5 bg-[#242424] rounded text-xs">Cmd+O</kbd> to open files.
          </div>
        ) : (
          spectra.map((spec) => {
            const stage = spec.has_chir
              ? { text: "FFT ready", icon: CheckCircle2, className: "text-emerald-400" }
              : spec.has_chi
                ? { text: "Background", icon: Activity, className: "text-amber-300" }
                : spec.has_norm
                  ? { text: "Normalized", icon: Activity, className: "text-blue-300" }
                  : spec.has_e0
                    ? { text: "E0 found", icon: Activity, className: "text-cyan-300" }
                    : { text: "Raw", icon: Circle, className: "text-[#777]" };
            const StageIcon = stage.icon;

            return (
              <div
                key={spec.index}
                className={`flex items-center gap-2 px-2 py-1.5 cursor-pointer text-[13px] transition-colors ${
                  activeIndex === spec.index
                    ? "bg-blue-600/20 text-white"
                    : selectedIndices.has(spec.index)
                      ? "bg-[#242424] text-[#e0e0e0]"
                      : "text-[#d0d0d0] hover:bg-[#202020]"
                }`}
                onClick={(e) => handleClick(spec.index, e)}
                onDoubleClick={() => handleDoubleClick(spec.index, spec.name)}
              >
                <input
                  type="checkbox"
                  checked={selectedIndices.has(spec.index)}
                  onChange={() => toggleSelection(spec.index)}
                  className="accent-blue-500 cursor-pointer"
                  onClick={(e) => e.stopPropagation()}
                />
                <span className="truncate flex-1">{spec.name}</span>
                <span className={`inline-flex items-center gap-1 text-[11px] ${stage.className}`}>
                  <StageIcon size={12} />
                  {stage.text}
                </span>
              </div>
            );
          })
        )}
      </div>
    </div>
  );
}
