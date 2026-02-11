import { useState, useCallback, useEffect, useRef } from "react";
import { Crosshair } from "lucide-react";
import {
  useSpectrumData,
  useFindE0,
  useNormalize,
  useCalcBackground,
  useFFT,
  useRunPipeline,
} from "@/hooks/useSpectra";
import { useSpectraStore } from "@/stores/spectra";
import { useWorkspaceStore } from "@/stores/workspace";
import type { NormOptions, BgOptions, FFTOptions } from "@/backend/types";
import type { ParamTab } from "@/stores/workspace";

const TAB_LABELS: Record<ParamTab, string> = {
  e0: "E0",
  norm: "Norm",
  bkg: "Bkg",
  fft: "FFT",
};

const TABS: ParamTab[] = ["e0", "norm", "bkg", "fft"];

export function ParameterPanel() {
  const activeIndex = useSpectraStore((s) => s.activeIndex);
  const { data: spectrum } = useSpectrumData(activeIndex);
  const { paramTab, setParamTab, pickTarget, setPickTarget, registerPickListener, unregisterPickListener } =
    useWorkspaceStore();

  // Parameter state — seeded from spectrum data
  const [normOpts, setNormOpts] = useState<NormOptions>({
    pre_edge_start: -200,
    pre_edge_end: -30,
    norm_start: 150,
    norm_end: 800,
  });
  const [bgOpts, setBgOpts] = useState<BgOptions>({
    rbkg: 1.0,
    kweight: 2,
    kmin: 0,
    kmax: 15,
  });
  const [fftOpts, setFftOpts] = useState<FFTOptions>({
    kmin: 2,
    kmax: 12,
    kweight: 2,
    dk: 1,
    window: "hanning",
  });

  // Seed E0 from spectrum when it changes
  const prevIndexRef = useRef<number | null>(null);
  useEffect(() => {
    if (activeIndex !== null && activeIndex !== prevIndexRef.current && spectrum?.e0) {
      setNormOpts((o) => ({ ...o, e0: spectrum.e0 ?? undefined }));
    }
    prevIndexRef.current = activeIndex;
  }, [activeIndex, spectrum?.e0]);

  // Register pick listeners for all pickable parameters
  useEffect(() => {
    const e0Val = spectrum?.e0 ?? 0;

    const listeners: [string, (v: number) => void][] = [
      ["E0", (v) => setNormOpts((o) => ({ ...o, e0: v }))],
      ["Pre-edge start", (v) => setNormOpts((o) => ({ ...o, pre_edge_start: Math.round(v - e0Val) }))],
      ["Pre-edge end", (v) => setNormOpts((o) => ({ ...o, pre_edge_end: Math.round(v - e0Val) }))],
      ["Norm start", (v) => setNormOpts((o) => ({ ...o, norm_start: Math.round(v - e0Val) }))],
      ["Norm end", (v) => setNormOpts((o) => ({ ...o, norm_end: Math.round(v - e0Val) }))],
    ];

    for (const [target, cb] of listeners) {
      registerPickListener(target, cb);
    }

    return () => {
      for (const [target] of listeners) {
        unregisterPickListener(target);
      }
    };
  }, [spectrum?.e0, registerPickListener, unregisterPickListener]);

  // Mutations
  const findE0 = useFindE0();
  const normalize = useNormalize();
  const calcBg = useCalcBackground();
  const fft = useFFT();
  const runPipeline = useRunPipeline();

  const isPending =
    findE0.isPending ||
    normalize.isPending ||
    calcBg.isPending ||
    fft.isPending ||
    runPipeline.isPending;

  const handleApply = useCallback(() => {
    if (activeIndex === null) return;
    switch (paramTab) {
      case "e0":
        findE0.mutate(activeIndex);
        break;
      case "norm":
        normalize.mutate({ index: activeIndex, opts: normOpts });
        break;
      case "bkg":
        calcBg.mutate({ index: activeIndex, opts: bgOpts });
        break;
      case "fft":
        fft.mutate({ index: activeIndex, opts: fftOpts });
        break;
    }
  }, [activeIndex, paramTab, normOpts, bgOpts, fftOpts, findE0, normalize, calcBg, fft]);

  const handleApplyAll = useCallback(() => {
    if (activeIndex === null) return;
    runPipeline.mutate({
      index: activeIndex,
      opts: {
        norm: normOpts,
        bg: bgOpts,
        fft: fftOpts,
      },
    });
  }, [activeIndex, normOpts, bgOpts, fftOpts, runPipeline]);

  const handlePick = useCallback(
    (label: string) => {
      setPickTarget(pickTarget === label ? null : label);
    },
    [pickTarget, setPickTarget],
  );

  if (activeIndex === null) {
    return (
      <div className="flex flex-col h-full">
        <div className="px-3 py-2 text-[11px] font-semibold text-slate-400 uppercase tracking-wider border-b border-slate-700">
          Parameters
        </div>
        <div className="flex-1 flex items-center justify-center p-4 text-slate-500 text-xs">
          Select a spectrum to edit parameters.
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="px-3 py-2 text-[11px] font-semibold text-slate-400 uppercase tracking-wider border-b border-slate-700">
        Parameters
        <span className="ml-2 normal-case tracking-normal font-normal text-slate-500">
          {spectrum?.name ?? `#${activeIndex}`}
        </span>
      </div>

      {/* Tabs: E0 | Norm | Bkg | FFT */}
      <div className="flex border-b border-slate-700 shrink-0">
        {TABS.map((tab) => (
          <button
            key={tab}
            className={`flex-1 py-1.5 text-center text-[11px] font-medium transition-colors border-b-2 ${
              paramTab === tab
                ? "text-blue-400 border-blue-500"
                : "text-slate-500 border-transparent hover:text-slate-300 hover:bg-slate-700/30"
            }`}
            onClick={() => setParamTab(tab)}
          >
            {TAB_LABELS[tab]}
          </button>
        ))}
      </div>

      {/* Tab content — all mounted, only active visible */}
      <div className="flex-1 overflow-y-auto">
        <div className={paramTab === "e0" ? "block p-3" : "hidden"}>
          <ParamRow label="E0" value={spectrum?.e0?.toFixed(2) ?? "\u2014"} unit="eV">
            <PickButton label="E0" active={pickTarget === "E0"} onClick={() => handlePick("E0")} />
          </ParamRow>
          <div className="mt-2">
            <ParamLabel label="Method" />
            <select className="w-full bg-slate-700 text-slate-200 text-xs px-2 py-1.5 rounded border border-slate-600 focus:border-blue-500 focus:outline-none">
              <option>Max 1st deriv</option>
              <option>Half step</option>
              <option>Max 2nd deriv</option>
            </select>
          </div>
        </div>

        <div className={paramTab === "norm" ? "block p-3 space-y-2" : "hidden"}>
          <ParamInput
            label="Pre start"
            value={normOpts.pre_edge_start}
            onChange={(v) => setNormOpts((o) => ({ ...o, pre_edge_start: v }))}
            unit="eV"
          >
            <PickButton
              label="Pre-edge start"
              active={pickTarget === "Pre-edge start"}
              onClick={() => handlePick("Pre-edge start")}
            />
          </ParamInput>
          <ParamInput
            label="Pre end"
            value={normOpts.pre_edge_end}
            onChange={(v) => setNormOpts((o) => ({ ...o, pre_edge_end: v }))}
            unit="eV"
          >
            <PickButton
              label="Pre-edge end"
              active={pickTarget === "Pre-edge end"}
              onClick={() => handlePick("Pre-edge end")}
            />
          </ParamInput>
          <div>
            <ParamLabel label="Norm order" />
            <select
              className="w-full bg-slate-700 text-slate-200 text-xs px-2 py-1.5 rounded border border-slate-600 focus:border-blue-500 focus:outline-none"
              value={normOpts.norm_polyorder ?? 2}
              onChange={(e) => setNormOpts((o) => ({ ...o, norm_polyorder: parseInt(e.target.value) }))}
            >
              <option value="1">1</option>
              <option value="2">2</option>
              <option value="3">3</option>
            </select>
          </div>
          <ParamInput
            label="Norm start"
            value={normOpts.norm_start}
            onChange={(v) => setNormOpts((o) => ({ ...o, norm_start: v }))}
            unit="eV"
          >
            <PickButton
              label="Norm start"
              active={pickTarget === "Norm start"}
              onClick={() => handlePick("Norm start")}
            />
          </ParamInput>
          <ParamInput
            label="Norm end"
            value={normOpts.norm_end}
            onChange={(v) => setNormOpts((o) => ({ ...o, norm_end: v }))}
            unit="eV"
          >
            <PickButton
              label="Norm end"
              active={pickTarget === "Norm end"}
              onClick={() => handlePick("Norm end")}
            />
          </ParamInput>
        </div>

        <div className={paramTab === "bkg" ? "block p-3 space-y-2" : "hidden"}>
          <ParamInput
            label="Rbkg"
            value={bgOpts.rbkg}
            onChange={(v) => setBgOpts((o) => ({ ...o, rbkg: v }))}
            unit={"\u00C5"}
          />
          <ParamInput
            label="k-weight"
            value={bgOpts.kweight}
            onChange={(v) => setBgOpts((o) => ({ ...o, kweight: v !== undefined ? Math.round(v) : undefined }))}
          />
          <ParamInput
            label="k-min"
            value={bgOpts.kmin}
            onChange={(v) => setBgOpts((o) => ({ ...o, kmin: v }))}
            unit={"\u00C5\u207B\u00B9"}
          />
          <ParamInput
            label="k-max"
            value={bgOpts.kmax}
            onChange={(v) => setBgOpts((o) => ({ ...o, kmax: v }))}
            unit={"\u00C5\u207B\u00B9"}
          />
          <div>
            <ParamLabel label="Clamps" />
            <select
              className="w-full bg-slate-700 text-slate-200 text-xs px-2 py-1.5 rounded border border-slate-600 focus:border-blue-500 focus:outline-none"
              defaultValue="low"
            >
              <option value="none">None</option>
              <option value="low">Low</option>
              <option value="high">High</option>
              <option value="both">Both</option>
            </select>
          </div>
        </div>

        <div className={paramTab === "fft" ? "block p-3 space-y-2" : "hidden"}>
          <ParamInput
            label="k-min"
            value={fftOpts.kmin}
            onChange={(v) => setFftOpts((o) => ({ ...o, kmin: v }))}
            unit={"\u00C5\u207B\u00B9"}
          />
          <ParamInput
            label="k-max"
            value={fftOpts.kmax}
            onChange={(v) => setFftOpts((o) => ({ ...o, kmax: v }))}
            unit={"\u00C5\u207B\u00B9"}
          />
          <ParamInput
            label="k-weight"
            value={fftOpts.kweight}
            onChange={(v) => setFftOpts((o) => ({ ...o, kweight: v }))}
          />
          <div>
            <ParamLabel label="Window" />
            <select
              className="w-full bg-slate-700 text-slate-200 text-xs px-2 py-1.5 rounded border border-slate-600 focus:border-blue-500 focus:outline-none"
              value={fftOpts.window ?? "hanning"}
              onChange={(e) => setFftOpts((o) => ({ ...o, window: e.target.value }))}
            >
              <option value="hanning">Hanning</option>
              <option value="kaiserbessel">Kaiser-Bessel</option>
              <option value="gaussian">Gaussian</option>
              <option value="sine">Sine</option>
            </select>
          </div>
          <ParamInput
            label="dk"
            value={fftOpts.dk}
            onChange={(v) => setFftOpts((o) => ({ ...o, dk: v }))}
            unit={"\u00C5\u207B\u00B9"}
          />
        </div>
      </div>

      {/* Action buttons */}
      <div className="px-3 py-2 border-t border-slate-700 shrink-0 flex gap-2">
        <button
          className="flex-1 py-1.5 text-xs bg-blue-600 hover:bg-blue-500 text-white rounded transition-colors disabled:opacity-50"
          onClick={handleApply}
          disabled={isPending}
        >
          {isPending ? "Processing..." : `Apply ${TAB_LABELS[paramTab]}`}
        </button>
        <button
          className="px-3 py-1.5 text-xs bg-slate-700 hover:bg-slate-600 text-slate-200 rounded transition-colors disabled:opacity-50"
          onClick={handleApplyAll}
          disabled={isPending}
          title="Run full pipeline: E0 → Norm → Bkg → FFT"
        >
          All
        </button>
      </div>
    </div>
  );
}

/* ─── Shared components ─── */

function ParamLabel({ label }: { label: string }) {
  return <div className="text-[11px] text-slate-400 mb-1">{label}</div>;
}

function ParamRow({
  label,
  value,
  unit,
  children,
}: {
  label: string;
  value: string;
  unit?: string;
  children?: React.ReactNode;
}) {
  return (
    <div className="flex items-center gap-2">
      <span className="text-[11px] text-slate-400 w-20 shrink-0">{label}</span>
      <span className="flex-1 text-xs text-slate-200">
        {value}
        {unit && <span className="text-slate-500 ml-1">{unit}</span>}
      </span>
      {children}
    </div>
  );
}

function ParamInput({
  label,
  value,
  onChange,
  unit,
  children,
}: {
  label: string;
  value: number | undefined;
  onChange: (v: number | undefined) => void;
  unit?: string;
  children?: React.ReactNode;
}) {
  const [text, setText] = useState(value?.toString() ?? "");

  useEffect(() => {
    setText(value?.toString() ?? "");
  }, [value]);

  const handleBlur = () => {
    const num = parseFloat(text);
    onChange(isNaN(num) ? undefined : num);
  };

  return (
    <div className="flex items-center gap-2">
      <label className="text-[11px] text-slate-400 w-20 shrink-0">{label}</label>
      <input
        type="text"
        className="flex-1 bg-slate-700 text-slate-200 text-xs px-2 py-1 rounded border border-slate-600 focus:border-blue-500 focus:outline-none min-w-0"
        value={text}
        onChange={(e) => setText(e.target.value)}
        onBlur={handleBlur}
        onKeyDown={(e) => e.key === "Enter" && handleBlur()}
        placeholder="auto"
      />
      {unit && <span className="text-[10px] text-slate-500 w-8 shrink-0">{unit}</span>}
      {children}
    </div>
  );
}

function PickButton({
  label,
  active,
  onClick,
}: {
  label: string;
  active: boolean;
  onClick: () => void;
}) {
  return (
    <button
      className={`w-6 h-6 flex items-center justify-center rounded border shrink-0 transition-colors ${
        active
          ? "border-blue-500 text-blue-400 bg-blue-500/15"
          : "border-slate-600 text-slate-500 hover:border-blue-500 hover:text-blue-400 hover:bg-blue-500/10"
      }`}
      title={`Pick ${label} from plot`}
      onClick={onClick}
    >
      <Crosshair size={12} />
    </button>
  );
}
