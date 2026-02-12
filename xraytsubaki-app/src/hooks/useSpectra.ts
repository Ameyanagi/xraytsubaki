import { useEffect } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { backend } from "@/backend/tauri";
import { useSpectraStore } from "@/stores/spectra";
import { addLog } from "@/panels/LogPanel";
import type {
  PipelineOptions,
  NormOptions,
  BgOptions,
  FFTOptions,
  BatchProgressEvent,
} from "@/backend/types";

export function useSpectrumList() {
  const version = useSpectraStore((s) => s.spectraVersion);
  return useQuery({
    queryKey: ["spectrumList", version],
    queryFn: () => backend.getSpectrumList(),
  });
}

export function useSpectrumData(index: number | null) {
  const version = useSpectraStore((s) => s.spectraVersion);
  return useQuery({
    queryKey: ["spectrumData", index, version],
    queryFn: () => backend.getSpectrumData(index!),
    enabled: index !== null,
  });
}

export function useBatchProgressEvents() {
  const updateBatchProgress = useSpectraStore((s) => s.updateBatchProgress);

  useEffect(() => {
    let isDisposed = false;
    let unlisten: (() => void) | undefined;

    async function setupListener() {
      if (typeof window === "undefined" || !("__TAURI_INTERNALS__" in window)) return;
      const { listen } = await import("@tauri-apps/api/event");
      if (isDisposed) return;
      unlisten = await listen<BatchProgressEvent>("batch-progress", (event) => {
        updateBatchProgress({
          current: event.payload.current,
          total: event.payload.total,
          succeeded: event.payload.succeeded,
          failed: event.payload.failed,
        });
      });
    }

    void setupListener();

    return () => {
      isDisposed = true;
      unlisten?.();
    };
  }, [updateBatchProgress]);
}

export function useLoadSpectra() {
  const queryClient = useQueryClient();
  const invalidate = useSpectraStore((s) => s.invalidateSpectra);

  return useMutation({
    mutationFn: (paths: string[]) => backend.loadSpectraFromFiles(paths),
    onSuccess: (result) => {
      invalidate();
      queryClient.invalidateQueries({ queryKey: ["spectrumList"] });
      addLog("info", `Loaded ${result.loaded} spectrum(s)`);
      if (result.errors.length > 0) {
        for (const err of result.errors) {
          addLog("error", `Failed to load ${err.path}: ${err.message}`);
        }
      }
    },
    onError: (err) => {
      addLog("error", `Load failed: ${err}`);
    },
  });
}

export function useRemoveSpectra() {
  const queryClient = useQueryClient();
  const invalidate = useSpectraStore((s) => s.invalidateSpectra);

  return useMutation({
    mutationFn: (indices: number[]) => backend.removeSpectra(indices),
    onSuccess: (_newCount, indices) => {
      invalidate();
      queryClient.invalidateQueries({ queryKey: ["spectrumList"] });
      addLog("info", `Removed ${indices.length} spectrum(s)`);
    },
    onError: (err) => {
      addLog("error", `Remove failed: ${err}`);
    },
  });
}

export function useFindE0() {
  const queryClient = useQueryClient();
  const invalidate = useSpectraStore((s) => s.invalidateSpectra);

  return useMutation({
    mutationFn: (index: number) => backend.findE0(index),
    onSuccess: (e0, index) => {
      invalidate();
      queryClient.invalidateQueries({ queryKey: ["spectrumData"] });
      queryClient.invalidateQueries({ queryKey: ["spectrumList"] });
      addLog("info", `E0 found for #${index}: ${e0.toFixed(2)} eV`);
    },
    onError: (err, index) => {
      addLog("error", `Find E0 failed for #${index}: ${err}`);
    },
  });
}

export function useNormalize() {
  const queryClient = useQueryClient();
  const invalidate = useSpectraStore((s) => s.invalidateSpectra);

  return useMutation({
    mutationFn: ({ index, opts }: { index: number; opts?: NormOptions }) =>
      backend.normalize(index, opts),
    onSuccess: (_, { index }) => {
      invalidate();
      queryClient.invalidateQueries({ queryKey: ["spectrumData"] });
      queryClient.invalidateQueries({ queryKey: ["spectrumList"] });
      addLog("info", `Normalized #${index}`);
    },
    onError: (err, { index }) => {
      addLog("error", `Normalize failed for #${index}: ${err}`);
    },
  });
}

export function useCalcBackground() {
  const queryClient = useQueryClient();
  const invalidate = useSpectraStore((s) => s.invalidateSpectra);

  return useMutation({
    mutationFn: ({ index, opts }: { index: number; opts?: BgOptions }) =>
      backend.calcBackground(index, opts),
    onSuccess: (_, { index }) => {
      invalidate();
      queryClient.invalidateQueries({ queryKey: ["spectrumData"] });
      queryClient.invalidateQueries({ queryKey: ["spectrumList"] });
      addLog("info", `Background removed for #${index}`);
    },
    onError: (err, { index }) => {
      addLog("error", `Background failed for #${index}: ${err}`);
    },
  });
}

export function useFFT() {
  const queryClient = useQueryClient();
  const invalidate = useSpectraStore((s) => s.invalidateSpectra);

  return useMutation({
    mutationFn: ({ index, opts }: { index: number; opts?: FFTOptions }) =>
      backend.fft(index, opts),
    onSuccess: (_, { index }) => {
      invalidate();
      queryClient.invalidateQueries({ queryKey: ["spectrumData"] });
      queryClient.invalidateQueries({ queryKey: ["spectrumList"] });
      addLog("info", `FFT completed for #${index}`);
    },
    onError: (err, { index }) => {
      addLog("error", `FFT failed for #${index}: ${err}`);
    },
  });
}

export function useRunPipeline() {
  const queryClient = useQueryClient();
  const invalidate = useSpectraStore((s) => s.invalidateSpectra);

  return useMutation({
    mutationFn: ({ index, opts }: { index: number; opts?: PipelineOptions }) =>
      backend.runPipeline(index, opts),
    onSuccess: (_, { index }) => {
      invalidate();
      queryClient.invalidateQueries({ queryKey: ["spectrumData"] });
      queryClient.invalidateQueries({ queryKey: ["spectrumList"] });
      addLog("info", `Pipeline completed for #${index}`);
    },
    onError: (err, { index }) => {
      addLog("error", `Pipeline failed for #${index}: ${err}`);
    },
  });
}

export function useBatchProcess() {
  const queryClient = useQueryClient();
  const invalidate = useSpectraStore((s) => s.invalidateSpectra);
  const startBatchProgress = useSpectraStore((s) => s.startBatchProgress);
  const finishBatchProgress = useSpectraStore((s) => s.finishBatchProgress);

  return useMutation({
    mutationFn: ({ indices, opts }: { indices: number[]; opts?: PipelineOptions }) =>
      backend.batchProcess(indices, opts),
    onMutate: ({ indices }) => {
      startBatchProgress(indices.length);
    },
    onSuccess: (result) => {
      invalidate();
      queryClient.invalidateQueries({ queryKey: ["spectrumData"] });
      queryClient.invalidateQueries({ queryKey: ["spectrumList"] });
      finishBatchProgress(result);
      addLog("info", `Batch: ${result.succeeded} succeeded, ${result.failed} failed`);
      if (result.errors.length > 0) {
        for (const err of result.errors) {
          addLog("error", `Batch error: ${err.name} — ${err.message}`);
        }
      }
    },
    onError: (err) => {
      finishBatchProgress();
      addLog("error", `Batch processing failed: ${err}`);
    },
  });
}
