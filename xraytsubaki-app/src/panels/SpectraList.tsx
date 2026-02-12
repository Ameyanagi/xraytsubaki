import { useCallback, useRef, useEffect } from "react";
import { Trash2, PlayCircle } from "lucide-react";
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
      clearSelection();
      setActiveIndex(null);
    }
  };

  const handleProcessSelected = () => {
    const indices = Array.from(selectedIndices);
    if (indices.length > 0) {
      batchProcess.mutate({ indices });
    }
  };

  if (isLoading) {
    return <div className="p-3 text-slate-400 text-sm">Loading spectra...</div>;
  }

  return (
    <div className="flex flex-col h-full">
      {/* Actions */}
      <div className="flex items-center gap-1 px-2 py-1.5 border-b border-slate-700">
        <button
          className="text-xs text-slate-400 hover:text-white px-1.5 py-0.5 rounded hover:bg-slate-700"
          onClick={handleSelectAll}
        >
          All
        </button>
        <button
          className="text-xs text-slate-400 hover:text-white px-1.5 py-0.5 rounded hover:bg-slate-700"
          onClick={clearSelection}
        >
          None
        </button>
        <div className="flex-1" />
        <button
          className="p-1 text-slate-400 hover:text-green-400 disabled:opacity-30"
          title="Process selected"
          onClick={handleProcessSelected}
          disabled={selectedIndices.size === 0 || batchProcess.isPending}
        >
          <PlayCircle size={14} />
        </button>
        <button
          className="p-1 text-slate-400 hover:text-red-400 disabled:opacity-30"
          title="Remove selected"
          onClick={handleDeleteSelected}
          disabled={selectedIndices.size === 0}
        >
          <Trash2 size={14} />
        </button>
      </div>

      {/* Spectrum list */}
      <div className="flex-1 overflow-y-auto">
        {!spectra || spectra.length === 0 ? (
          <div className="p-4 text-center text-slate-500 text-sm">
            No spectra loaded.
            <br />
            Use <kbd className="px-1 py-0.5 bg-slate-700 rounded text-xs">Cmd+O</kbd> to open files.
          </div>
        ) : (
          spectra.map((spec) => (
            <div
              key={spec.index}
              className={`flex items-center gap-2 px-2 py-1 cursor-pointer text-sm border-l-2 transition-colors ${
                activeIndex === spec.index
                  ? "bg-blue-900/30 border-blue-500 text-white"
                  : selectedIndices.has(spec.index)
                    ? "bg-slate-700/50 border-slate-500 text-slate-200"
                    : "border-transparent text-slate-300 hover:bg-slate-700/30"
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
              <div className="flex items-center gap-0.5">
                {spec.has_e0 && (
                  <span className="w-1.5 h-1.5 rounded-full bg-green-500" title="E0 found" />
                )}
                {spec.has_norm && (
                  <span className="w-1.5 h-1.5 rounded-full bg-blue-500" title="Normalized" />
                )}
                {spec.has_chi && (
                  <span className="w-1.5 h-1.5 rounded-full bg-yellow-500" title="Background" />
                )}
                {spec.has_chir && (
                  <span className="w-1.5 h-1.5 rounded-full bg-purple-500" title="FFT" />
                )}
              </div>
            </div>
          ))
        )}
      </div>
    </div>
  );
}
