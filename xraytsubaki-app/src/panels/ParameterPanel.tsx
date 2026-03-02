import {
  forwardRef,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
  type RefObject,
} from "react";
import { Crosshair } from "lucide-react";
import {
  useSpectrumData,
  useFindE0,
  useNormalize,
  useCalcBackground,
  useFFT,
  useRunPipeline,
} from "@/hooks/useSpectra";
import { useWorkspaceStore } from "@/stores/workspace";
import type { ParamTab } from "@/stores/workspace";

const SECTION_ORDER: ParamTab[] = ["e0", "norm", "bkg", "fft"];
const SECTION_LABELS: Record<ParamTab, string> = {
  e0: "E0",
  norm: "Normalization",
  bkg: "Background",
  fft: "FFT",
};
const LIVE_DEBOUNCE_MS = 300;

interface SectionCardProps {
  title: string;
  active: boolean;
  onActivate: () => void;
  onApply: () => void;
  applying: boolean;
  children: ReactNode;
}

export function ParameterPanel() {
  const activeIndex = useWorkspaceStore((s) => s.activeIndex);
  const { data: spectrum } = useSpectrumData(activeIndex);

  const paramTab = useWorkspaceStore((s) => s.paramTab);
  const setParamTab = useWorkspaceStore((s) => s.setParamTab);

  const pickTarget = useWorkspaceStore((s) => s.pickTarget);
  const setPickTarget = useWorkspaceStore((s) => s.setPickTarget);
  const registerPickListener = useWorkspaceStore((s) => s.registerPickListener);
  const unregisterPickListener = useWorkspaceStore((s) => s.unregisterPickListener);

  const normOpts = useWorkspaceStore((s) => s.normOpts);
  const setNormOpts = useWorkspaceStore((s) => s.setNormOpts);
  const bgOpts = useWorkspaceStore((s) => s.bgOpts);
  const setBgOpts = useWorkspaceStore((s) => s.setBgOpts);
  const fftOpts = useWorkspaceStore((s) => s.fftOpts);
  const setFftOpts = useWorkspaceStore((s) => s.setFftOpts);

  const livePreview = useWorkspaceStore((s) => s.livePreview);
  const setLivePreview = useWorkspaceStore((s) => s.setLivePreview);

  const findE0 = useFindE0();
  const normalize = useNormalize();
  const calcBg = useCalcBackground();
  const fft = useFFT();
  const runPipeline = useRunPipeline();

  const e0Ref = useRef<HTMLDivElement>(null);
  const normRef = useRef<HTMLDivElement>(null);
  const bkgRef = useRef<HTMLDivElement>(null);
  const fftRef = useRef<HTMLDivElement>(null);

  const sectionRefs: Record<ParamTab, RefObject<HTMLDivElement | null>> = {
    e0: e0Ref,
    norm: normRef,
    bkg: bkgRef,
    fft: fftRef,
  };

  const lastNormApplyKeyRef = useRef<string | null>(null);
  const lastBgApplyKeyRef = useRef<string | null>(null);
  const lastFftApplyKeyRef = useRef<string | null>(null);

  const isPending =
    findE0.isPending ||
    normalize.isPending ||
    calcBg.isPending ||
    fft.isPending ||
    runPipeline.isPending;

  const normKey = useMemo(
    () => (activeIndex === null ? null : `${activeIndex}:norm:${JSON.stringify(normOpts)}`),
    [activeIndex, normOpts],
  );
  const bgKey = useMemo(
    () => (activeIndex === null ? null : `${activeIndex}:bkg:${JSON.stringify(bgOpts)}`),
    [activeIndex, bgOpts],
  );
  const fftKey = useMemo(
    () => (activeIndex === null ? null : `${activeIndex}:fft:${JSON.stringify(fftOpts)}`),
    [activeIndex, fftOpts],
  );

  // Seed E0 override from active spectrum once per selection change.
  const prevIndexRef = useRef<number | null>(null);
  useEffect(() => {
    if (activeIndex !== null && activeIndex !== prevIndexRef.current && spectrum?.e0) {
      setNormOpts((prev) => ({ ...prev, e0: spectrum.e0 ?? undefined }));
    }
    prevIndexRef.current = activeIndex;
  }, [activeIndex, setNormOpts, spectrum?.e0]);

  useEffect(() => {
    const e0Val = spectrum?.e0 ?? 0;
    const listeners: [string, (v: number) => void][] = [
      ["E0", (v) => setNormOpts((prev) => ({ ...prev, e0: v }))],
      [
        "Pre-edge start",
        (v) => setNormOpts((prev) => ({ ...prev, pre_edge_start: Math.round(v - e0Val) })),
      ],
      [
        "Pre-edge end",
        (v) => setNormOpts((prev) => ({ ...prev, pre_edge_end: Math.round(v - e0Val) })),
      ],
      [
        "Norm start",
        (v) => setNormOpts((prev) => ({ ...prev, norm_start: Math.round(v - e0Val) })),
      ],
      [
        "Norm end",
        (v) => setNormOpts((prev) => ({ ...prev, norm_end: Math.round(v - e0Val) })),
      ],
    ];

    for (const [target, cb] of listeners) {
      registerPickListener(target, cb);
    }

    return () => {
      for (const [target] of listeners) {
        unregisterPickListener(target);
      }
    };
  }, [registerPickListener, setNormOpts, spectrum?.e0, unregisterPickListener]);

  useEffect(() => {
    if (activeIndex === null) {
      lastNormApplyKeyRef.current = null;
      lastBgApplyKeyRef.current = null;
      lastFftApplyKeyRef.current = null;
      return;
    }

    lastNormApplyKeyRef.current = normKey;
    lastBgApplyKeyRef.current = bgKey;
    lastFftApplyKeyRef.current = fftKey;
  }, [activeIndex, normKey, bgKey, fftKey]);

  useEffect(() => {
    sectionRefs[paramTab]?.current?.scrollIntoView({ behavior: "smooth", block: "start" });
  }, [paramTab]);

  useEffect(() => {
    if (!livePreview || activeIndex === null || !normKey || isPending) return;
    if (normKey === lastNormApplyKeyRef.current) return;

    const timer = window.setTimeout(() => {
      lastNormApplyKeyRef.current = normKey;
      normalize.mutate({ index: activeIndex, opts: normOpts });
    }, LIVE_DEBOUNCE_MS);

    return () => window.clearTimeout(timer);
  }, [activeIndex, isPending, livePreview, normKey, normOpts, normalize]);

  useEffect(() => {
    if (!livePreview || activeIndex === null || !bgKey || isPending) return;
    if (bgKey === lastBgApplyKeyRef.current) return;

    const timer = window.setTimeout(() => {
      lastBgApplyKeyRef.current = bgKey;
      calcBg.mutate({ index: activeIndex, opts: bgOpts });
    }, LIVE_DEBOUNCE_MS);

    return () => window.clearTimeout(timer);
  }, [activeIndex, bgKey, bgOpts, calcBg, isPending, livePreview]);

  useEffect(() => {
    if (!livePreview || activeIndex === null || !fftKey || isPending) return;
    if (fftKey === lastFftApplyKeyRef.current) return;

    const timer = window.setTimeout(() => {
      lastFftApplyKeyRef.current = fftKey;
      fft.mutate({ index: activeIndex, opts: fftOpts });
    }, LIVE_DEBOUNCE_MS);

    return () => window.clearTimeout(timer);
  }, [activeIndex, fft, fftKey, fftOpts, isPending, livePreview]);

  const handlePick = useCallback(
    (label: string, section: ParamTab) => {
      setParamTab(section);
      setPickTarget(pickTarget === label ? null : label);
    },
    [pickTarget, setParamTab, setPickTarget],
  );

  const applySection = useCallback(
    (section: ParamTab) => {
      if (activeIndex === null) return;
      setParamTab(section);

      if (section === "e0") {
        findE0.mutate(activeIndex);
        return;
      }
      if (section === "norm") {
        lastNormApplyKeyRef.current = normKey;
        normalize.mutate({ index: activeIndex, opts: normOpts });
        return;
      }
      if (section === "bkg") {
        lastBgApplyKeyRef.current = bgKey;
        calcBg.mutate({ index: activeIndex, opts: bgOpts });
        return;
      }

      lastFftApplyKeyRef.current = fftKey;
      fft.mutate({ index: activeIndex, opts: fftOpts });
    },
    [
      activeIndex,
      bgKey,
      bgOpts,
      calcBg,
      fft,
      fftKey,
      findE0,
      normKey,
      normOpts,
      normalize,
      setParamTab,
    ],
  );

  const applyAll = useCallback(() => {
    if (activeIndex === null) return;

    lastNormApplyKeyRef.current = normKey;
    lastBgApplyKeyRef.current = bgKey;
    lastFftApplyKeyRef.current = fftKey;

    runPipeline.mutate({
      index: activeIndex,
      opts: {
        norm: normOpts,
        bg: bgOpts,
        fft: fftOpts,
      },
    });
  }, [activeIndex, bgKey, bgOpts, fftKey, fftOpts, normKey, normOpts, runPipeline]);

  if (activeIndex === null) {
    return (
      <div className="flex flex-col h-full">
        <div className="px-3 py-2 text-[12px] font-semibold text-[#b0b0b0] uppercase tracking-wider">
          Parameters
        </div>
        <div className="flex-1 flex items-center justify-center p-4 text-[#888] text-xs">
          Select a spectrum to edit parameters.
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full min-h-0">
      <div className="px-3 py-2 text-[12px] font-semibold text-[#b0b0b0] uppercase tracking-wider">
        Parameters
        <span className="ml-2 normal-case tracking-normal font-normal text-[#8a8a8a]">
          {spectrum?.name ?? `#${activeIndex}`}
        </span>
      </div>

      <div className="px-3 pb-2 flex gap-1.5 shrink-0">
        {SECTION_ORDER.map((section) => (
          <button
            key={section}
            className={`px-2 py-1 text-[11px] rounded border transition-colors ${
              paramTab === section
                ? "border-blue-500 text-blue-300 bg-blue-500/10"
                : "border-[#343434] text-[#9a9a9a] hover:text-[#e0e0e0] hover:bg-[#242424]"
            }`}
            onClick={() => {
              setParamTab(section);
              sectionRefs[section].current?.scrollIntoView({ behavior: "smooth", block: "start" });
            }}
          >
            {SECTION_LABELS[section]}
          </button>
        ))}
      </div>

      <div className="flex-1 overflow-y-auto px-3 pb-3 space-y-2.5">
        <SectionCard
          ref={sectionRefs.e0}
          title="E0"
          active={paramTab === "e0"}
          onActivate={() => setParamTab("e0")}
          onApply={() => applySection("e0")}
          applying={findE0.isPending}
        >
          <div className="grid grid-cols-2 gap-2">
            <DisplayField label="Current E0" value={spectrum?.e0?.toFixed(2) ?? "-"} unit="eV" />
            <SelectField
              label="Method"
              value="max-1st"
              onChange={() => undefined}
              options={[
                { value: "max-1st", label: "Max 1st deriv" },
                { value: "half-step", label: "Half step" },
                { value: "max-2nd", label: "Max 2nd deriv" },
              ]}
            />
            <CompactNumberField
              label="Override E0"
              value={normOpts.e0}
              onChange={(v) => setNormOpts((prev) => ({ ...prev, e0: v }))}
              unit="eV"
              right={
                <PickButton
                  label="E0"
                  active={pickTarget === "E0"}
                  onClick={() => handlePick("E0", "e0")}
                />
              }
            />
          </div>
        </SectionCard>

        <SectionCard
          ref={sectionRefs.norm}
          title="Normalization"
          active={paramTab === "norm"}
          onActivate={() => setParamTab("norm")}
          onApply={() => applySection("norm")}
          applying={normalize.isPending}
        >
          <div className="grid grid-cols-2 gap-2">
            <CompactNumberField
              label="Pre start"
              value={normOpts.pre_edge_start}
              onChange={(v) => setNormOpts((prev) => ({ ...prev, pre_edge_start: v }))}
              unit="eV"
              right={
                <PickButton
                  label="Pre-edge start"
                  active={pickTarget === "Pre-edge start"}
                  onClick={() => handlePick("Pre-edge start", "norm")}
                />
              }
            />
            <CompactNumberField
              label="Pre end"
              value={normOpts.pre_edge_end}
              onChange={(v) => setNormOpts((prev) => ({ ...prev, pre_edge_end: v }))}
              unit="eV"
              right={
                <PickButton
                  label="Pre-edge end"
                  active={pickTarget === "Pre-edge end"}
                  onClick={() => handlePick("Pre-edge end", "norm")}
                />
              }
            />
            <CompactNumberField
              label="Norm start"
              value={normOpts.norm_start}
              onChange={(v) => setNormOpts((prev) => ({ ...prev, norm_start: v }))}
              unit="eV"
              right={
                <PickButton
                  label="Norm start"
                  active={pickTarget === "Norm start"}
                  onClick={() => handlePick("Norm start", "norm")}
                />
              }
            />
            <CompactNumberField
              label="Norm end"
              value={normOpts.norm_end}
              onChange={(v) => setNormOpts((prev) => ({ ...prev, norm_end: v }))}
              unit="eV"
              right={
                <PickButton
                  label="Norm end"
                  active={pickTarget === "Norm end"}
                  onClick={() => handlePick("Norm end", "norm")}
                />
              }
            />
            <SelectField
              label="Norm order"
              value={String(normOpts.norm_polyorder ?? 2)}
              onChange={(value) =>
                setNormOpts((prev) => ({ ...prev, norm_polyorder: Number.parseInt(value, 10) }))
              }
              options={[
                { value: "1", label: "1" },
                { value: "2", label: "2" },
                { value: "3", label: "3" },
              ]}
            />
          </div>
        </SectionCard>

        <SectionCard
          ref={sectionRefs.bkg}
          title="Background"
          active={paramTab === "bkg"}
          onActivate={() => setParamTab("bkg")}
          onApply={() => applySection("bkg")}
          applying={calcBg.isPending}
        >
          <div className="grid grid-cols-2 gap-2">
            <CompactNumberField
              label="Rbkg"
              value={bgOpts.rbkg}
              onChange={(v) => setBgOpts((prev) => ({ ...prev, rbkg: v }))}
              unit="A"
            />
            <CompactNumberField
              label="k-weight"
              value={bgOpts.kweight}
              onChange={(v) =>
                setBgOpts((prev) => ({ ...prev, kweight: v === undefined ? undefined : Math.round(v) }))
              }
            />
            <CompactNumberField
              label="k-min"
              value={bgOpts.kmin}
              onChange={(v) => setBgOpts((prev) => ({ ...prev, kmin: v }))}
              unit="A^-1"
            />
            <CompactNumberField
              label="k-max"
              value={bgOpts.kmax}
              onChange={(v) => setBgOpts((prev) => ({ ...prev, kmax: v }))}
              unit="A^-1"
            />
          </div>
        </SectionCard>

        <SectionCard
          ref={sectionRefs.fft}
          title="FFT"
          active={paramTab === "fft"}
          onActivate={() => setParamTab("fft")}
          onApply={() => applySection("fft")}
          applying={fft.isPending}
        >
          <div className="grid grid-cols-2 gap-2">
            <CompactNumberField
              label="k-min"
              value={fftOpts.kmin}
              onChange={(v) => setFftOpts((prev) => ({ ...prev, kmin: v }))}
              unit="A^-1"
            />
            <CompactNumberField
              label="k-max"
              value={fftOpts.kmax}
              onChange={(v) => setFftOpts((prev) => ({ ...prev, kmax: v }))}
              unit="A^-1"
            />
            <CompactNumberField
              label="k-weight"
              value={fftOpts.kweight}
              onChange={(v) => setFftOpts((prev) => ({ ...prev, kweight: v }))}
            />
            <CompactNumberField
              label="dk"
              value={fftOpts.dk}
              onChange={(v) => setFftOpts((prev) => ({ ...prev, dk: v }))}
              unit="A^-1"
            />
            <SelectField
              label="Window"
              value={fftOpts.window ?? "hanning"}
              onChange={(value) => setFftOpts((prev) => ({ ...prev, window: value }))}
              options={[
                { value: "hanning", label: "Hanning" },
                { value: "kaiserbessel", label: "Kaiser-Bessel" },
                { value: "gaussian", label: "Gaussian" },
                { value: "sine", label: "Sine" },
              ]}
            />
          </div>
        </SectionCard>
      </div>

      <div className="px-3 py-2 shrink-0 flex items-center gap-2 border-t border-[#2c2c2c]">
        <label className="flex items-center gap-1 text-[12px] text-[#a0a0a0] cursor-pointer">
          <input
            type="checkbox"
            className="w-3 h-3 accent-blue-500"
            checked={livePreview}
            onChange={(e) => setLivePreview(e.target.checked)}
          />
          Live
        </label>
        <button
          className="flex-1 py-1.5 text-xs bg-blue-600 hover:bg-blue-500 text-white rounded transition-colors disabled:opacity-50"
          onClick={() => applySection(paramTab)}
          disabled={isPending}
        >
          {isPending ? "Processing..." : `Apply ${SECTION_LABELS[paramTab]}`}
        </button>
        <button
          className="px-3 py-1.5 text-xs bg-[#242424] hover:bg-[#2d2d2d] text-[#e0e0e0] rounded transition-colors disabled:opacity-50"
          onClick={applyAll}
          disabled={isPending}
          title="Run full pipeline: E0 -> Norm -> Bkg -> FFT"
        >
          Apply All
        </button>
      </div>
    </div>
  );
}

const SectionCard = forwardRef<HTMLDivElement, SectionCardProps>(
  ({ title, active, onActivate, onApply, applying, children }, ref) => (
    <section
      ref={ref}
      className={`rounded border p-2.5 space-y-2 transition-colors ${
        active ? "border-blue-500/60 bg-blue-500/5" : "border-[#343434] bg-[#151515]"
      }`}
      onClick={onActivate}
    >
      <div className="flex items-center justify-between gap-2">
        <h3 className="text-xs font-semibold uppercase tracking-wider text-[#b7b7b7]">{title}</h3>
        <button
          className="px-2 py-0.5 text-[11px] rounded border border-[#3a3a3a] text-[#b8b8b8] hover:text-white hover:bg-[#202020] transition-colors disabled:opacity-40"
          onClick={(e) => {
            e.stopPropagation();
            onApply();
          }}
          disabled={applying}
        >
          {applying ? "Running..." : "Apply"}
        </button>
      </div>
      {children}
    </section>
  ),
);
SectionCard.displayName = "SectionCard";

function DisplayField({
  label,
  value,
  unit,
}: {
  label: string;
  value: string;
  unit?: string;
}) {
  return (
    <div>
      <div className="text-[11px] text-[#8f8f8f] mb-1">{label}</div>
      <div className="h-8 px-2 rounded border border-[#343434] bg-[#111] text-[12px] text-[#d8d8d8] flex items-center">
        {value}
        {unit && <span className="ml-1 text-[#7a7a7a]">{unit}</span>}
      </div>
    </div>
  );
}

function CompactNumberField({
  label,
  value,
  onChange,
  unit,
  right,
}: {
  label: string;
  value: number | undefined;
  onChange: (value: number | undefined) => void;
  unit?: string;
  right?: ReactNode;
}) {
  const [text, setText] = useState(value?.toString() ?? "");

  useEffect(() => {
    setText(value?.toString() ?? "");
  }, [value]);

  const commit = () => {
    const trimmed = text.trim();
    if (!trimmed) {
      onChange(undefined);
      return;
    }
    const parsed = Number(trimmed);
    onChange(Number.isFinite(parsed) ? parsed : undefined);
  };

  return (
    <div>
      <div className="text-[11px] text-[#8f8f8f] mb-1">{label}</div>
      <div className="flex items-center gap-1.5">
        <input
          type="text"
          className="flex-1 h-8 px-2 bg-[#242424] text-[#e0e0e0] text-[12px] rounded border border-[#343434] focus:border-blue-500 focus:outline-none min-w-0"
          value={text}
          onChange={(e) => setText(e.target.value)}
          onBlur={commit}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              (e.currentTarget as HTMLInputElement).blur();
            }
          }}
          placeholder="auto"
        />
        {unit && <span className="text-[11px] text-[#787878] whitespace-nowrap">{unit}</span>}
        {right}
      </div>
    </div>
  );
}

function SelectField({
  label,
  value,
  onChange,
  options,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  options: { value: string; label: string }[];
}) {
  return (
    <div>
      <div className="text-[11px] text-[#8f8f8f] mb-1">{label}</div>
      <select
        className="w-full h-8 bg-[#242424] text-[#e0e0e0] text-[12px] px-2 rounded border border-[#343434] focus:border-blue-500 focus:outline-none"
        value={value}
        onChange={(e) => onChange(e.target.value)}
      >
        {options.map((option) => (
          <option key={option.value} value={option.value}>
            {option.label}
          </option>
        ))}
      </select>
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
      className={`w-8 h-8 flex items-center justify-center rounded border shrink-0 transition-colors ${
        active
          ? "border-blue-500 text-blue-400 bg-blue-500/15"
          : "border-[#3a3a3a] text-[#787878] hover:border-blue-500 hover:text-blue-400 hover:bg-blue-500/10"
      }`}
      title={`Pick ${label} from plot`}
      onClick={onClick}
    >
      <Crosshair size={12} />
    </button>
  );
}
