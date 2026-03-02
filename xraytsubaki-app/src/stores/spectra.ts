import { create } from "zustand";
import type { BatchResult } from "@/backend/types";

export interface BatchProgressState {
  active: boolean;
  current: number;
  total: number;
  succeeded: number;
  failed: number;
}

interface SpectraState {
  // Selection
  selectedIndices: Set<number>;
  activeIndex: number | null;

  // Actions
  setActiveIndex: (index: number | null) => void;
  toggleSelection: (index: number) => void;
  selectRange: (from: number, to: number) => void;
  selectAll: (indices: number[]) => void;
  clearSelection: () => void;
  setSelectedIndices: (indices: Set<number>) => void;

  // Invalidation counter to trigger refetches
  spectraVersion: number;
  invalidateSpectra: () => void;

  // Batch processing progress
  batchProgress: BatchProgressState | null;
  startBatchProgress: (total: number) => void;
  updateBatchProgress: (progress: Omit<BatchProgressState, "active">) => void;
  finishBatchProgress: (result?: BatchResult) => void;
  clearBatchProgress: () => void;
}

export const useSpectraStore = create<SpectraState>((set) => ({
  selectedIndices: new Set(),
  activeIndex: null,

  setActiveIndex: (index) => set({ activeIndex: index }),

  toggleSelection: (index) =>
    set((state) => {
      const next = new Set(state.selectedIndices);
      if (next.has(index)) {
        next.delete(index);
      } else {
        next.add(index);
      }
      return { selectedIndices: next };
    }),

  selectRange: (from, to) =>
    set((state) => {
      const next = new Set(state.selectedIndices);
      const start = Math.min(from, to);
      const end = Math.max(from, to);
      for (let i = start; i <= end; i++) {
        next.add(i);
      }
      return { selectedIndices: next };
    }),

  selectAll: (indices) => set({ selectedIndices: new Set(indices) }),

  clearSelection: () => set({ selectedIndices: new Set() }),

  setSelectedIndices: (indices) => set({ selectedIndices: indices }),

  spectraVersion: 0,
  invalidateSpectra: () => set((state) => ({ spectraVersion: state.spectraVersion + 1 })),

  batchProgress: null,
  startBatchProgress: (total) =>
    set({
      batchProgress: {
        active: true,
        current: 0,
        total,
        succeeded: 0,
        failed: 0,
      },
    }),
  updateBatchProgress: (progress) =>
    set({
      batchProgress: {
        active: true,
        ...progress,
      },
    }),
  finishBatchProgress: (result) =>
    set((state) => {
      if (!state.batchProgress) return {};
      return {
        batchProgress: {
          ...state.batchProgress,
          active: false,
          current: state.batchProgress.total,
          succeeded: result?.succeeded ?? state.batchProgress.succeeded,
          failed: result?.failed ?? state.batchProgress.failed,
        },
      };
    }),
  clearBatchProgress: () => set({ batchProgress: null }),
}));
