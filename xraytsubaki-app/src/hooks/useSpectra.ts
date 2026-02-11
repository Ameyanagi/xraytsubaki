import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { backend } from "@/backend/tauri";
import { useSpectraStore } from "@/stores/spectra";
import { addLog } from "@/panels/LogPanel";
import type { PipelineOptions, NormOptions, BgOptions, FFTOptions } from "@/backend/types";

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

  return useMutation({
    mutationFn: ({ indices, opts }: { indices: number[]; opts?: PipelineOptions }) =>
      backend.batchProcess(indices, opts),
    onSuccess: (result) => {
      invalidate();
      queryClient.invalidateQueries({ queryKey: ["spectrumData"] });
      queryClient.invalidateQueries({ queryKey: ["spectrumList"] });
      addLog("info", `Batch: ${result.succeeded} succeeded, ${result.failed} failed`);
      if (result.errors.length > 0) {
        for (const err of result.errors) {
          addLog("error", `Batch error: ${err.name} — ${err.message}`);
        }
      }
    },
    onError: (err) => {
      addLog("error", `Batch processing failed: ${err}`);
    },
  });
}
